#![cfg(feature = "models")]

use anyhow::Result;
use ed25519_dalek::{Signer, SigningKey};
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, CellFamily, HashBytes};
use rston::dict::Dict;
use rston::error::Error;
use rston::models::block::{
    Block, BlockId, BlockSignature, ShardIdent, Signature, SimplexBlockSignatures,
};
use rston::models::config::{CatchainConfig, ValidatorDescription, ValidatorSet};
use rston::models::shard::ValidatorBaseInfo;
use serde::Deserialize;

#[derive(Deserialize)]
struct Vector {
    name: String,
    candidate: String,
    data_to_sign: String,
}

#[derive(Clone)]
struct Fixture {
    block: BlockId,
    validator_info: ValidatorBaseInfo,
    validators: Vec<ValidatorDescription>,
    signatures: SimplexBlockSignatures,
}

impl Fixture {
    fn verify(&self) -> Result<u64, rston::error::Error> {
        self.signatures
            .check_signatures(&self.block, &self.validator_info, &self.validators)
    }
}

fn vectors() -> Vec<Vector> {
    serde_json::from_str(include_str!("fixtures/simplex/candidates.json")).unwrap()
}

fn ordinary_fixture() -> Result<Fixture> {
    synthetic_fixture(&ordinary_vector())
}

fn ordinary_vector() -> Vector {
    vectors()
        .into_iter()
        .find(|vector| vector.name == "ordinary_without_parent")
        .unwrap()
}

fn snake(bytes: &[u8], chunk_size: usize) -> Result<Cell> {
    let mut tail = None;
    for chunk in bytes.chunks(chunk_size).rev() {
        let mut builder = CellBuilder::new();
        builder.store_raw(chunk, chunk.len() as u16 * 8)?;
        if let Some(cell) = tail {
            builder.store_reference(cell)?;
        }
        tail = Some(builder.build()?);
    }
    Ok(tail.unwrap_or_else(Cell::empty_cell))
}

fn synthetic_fixture(vector: &Vector) -> Result<Fixture> {
    let mut validators = Vec::new();
    let mut signatures = Dict::new();
    let payload = hex::decode(&vector.data_to_sign)?;
    for (index, (seed, weight)) in [(0x11, 40), (0x22, 35), (0x33, 25)].into_iter().enumerate() {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let public_key = key.verifying_key().to_bytes();
        validators.push(ValidatorDescription {
            public_key: HashBytes(public_key),
            weight,
            adnl_addr: Some(HashBytes([seed + 1; 32])),
            prev_total_weight: 0,
        });
        if index < 2 {
            signatures.set(
                index as u16,
                BlockSignature {
                    node_id_short: HashBytes(tl_proto::hash(
                        tycho_crypto::tl::PublicKey::Ed25519 { key: &public_key },
                    )),
                    signature: Signature(key.sign(&payload).to_bytes()),
                },
            )?;
        }
    }
    let validator_info = ValidatorBaseInfo {
        validator_list_hash_short: ValidatorSet::compute_subset_hash_short(&validators, 17),
        catchain_seqno: 17,
    };
    Ok(Fixture {
        block: BlockId {
            shard: ShardIdent::MASTERCHAIN,
            seqno: 42,
            root_hash: HashBytes([0x41; 32]),
            file_hash: HashBytes([0x42; 32]),
        },
        validator_info,
        validators,
        signatures: SimplexBlockSignatures {
            validator_info,
            signature_count: 2,
            total_weight: 75,
            signatures,
            session_id: HashBytes([0x51; 32]),
            slot: 77,
            candidate_data: snake(&hex::decode(&vector.candidate)?, 127)?,
        },
    })
}

#[test]
fn matches_go_payloads_and_verifies_all_candidate_variants() -> Result<()> {
    for vector in vectors() {
        let mut fixture = synthetic_fixture(&vector)?;
        let payload = fixture.signatures.build_data_for_sign(&fixture.block)?;
        anyhow::ensure!(
            payload == hex::decode(&vector.data_to_sign)?,
            "{}: TL payload differs from tonutils-go",
            vector.name
        );
        assert_eq!(fixture.verify(), Ok(75), "{}", vector.name);

        fixture.signatures.candidate_data = snake(&hex::decode(&vector.candidate)?, 10)?;
        anyhow::ensure!(
            fixture.signatures.build_data_for_sign(&fixture.block)? == payload,
            "cell boundaries changed the signed payload"
        );
        assert_eq!(fixture.verify(), Ok(75), "{} rechunked", vector.name);
    }
    Ok(())
}

#[test]
fn verifies_mainnet_signatures_with_the_configured_validator_subset() -> Result<()> {
    let signatures = Boc::decode(include_bytes!("fixtures/simplex/mainnet-signatures.boc"))?
        .parse::<SimplexBlockSignatures>()?;
    let validators = Boc::decode(include_bytes!("fixtures/simplex/mainnet-validator-set.boc"))?
        .parse::<ValidatorSet>()?;
    let catchain = Boc::decode(include_bytes!(
        "fixtures/simplex/mainnet-catchain-config.boc"
    ))?
    .parse::<CatchainConfig>()?;
    let block = BlockId {
        shard: ShardIdent::MASTERCHAIN,
        seqno: 98841714,
        root_hash: "aab0320a4e3a90f3155f3e8b3a9b9506822e8663ab6493b45d059de9aa67e98f".parse()?,
        file_hash: "a00ed9197c5f968b8ef5f693f531d0564bb594e573b08dd7892c5f39e942b762".parse()?,
    };
    let validator_info = ValidatorBaseInfo {
        validator_list_hash_short: 569050026,
        catchain_seqno: 871222,
    };
    let (subset, hash) = validators
        .compute_subset(block.shard, &catchain, validator_info.catchain_seqno)
        .unwrap();
    assert_eq!(hash, validator_info.validator_list_hash_short);
    assert_eq!(
        signatures.check_signatures(&block, &validator_info, &subset),
        Ok(302390044705312660),
    );
    Ok(())
}

#[test]
fn rejects_every_block_identifier_mismatch() -> Result<()> {
    let fixture = ordinary_fixture()?;
    let expected_blocks = [
        (
            "workchain",
            BlockId {
                shard: ShardIdent::BASECHAIN,
                ..fixture.block
            },
        ),
        (
            "seqno",
            BlockId {
                seqno: 43,
                ..fixture.block
            },
        ),
        (
            "root_hash",
            BlockId {
                root_hash: HashBytes([0x99; 32]),
                ..fixture.block
            },
        ),
        (
            "file_hash",
            BlockId {
                file_hash: HashBytes([0x99; 32]),
                ..fixture.block
            },
        ),
    ];
    for (field, block) in expected_blocks {
        let result = fixture.signatures.build_data_for_sign(&block).map(|_| ());
        assert_eq!(result, Err(Error::InvalidData), "{field}");
    }

    // The candidate's bare block ID starts after its four-byte constructor.
    // Set both workchains to zero so that only the shard prefix differs.
    let mut candidate = hex::decode(&ordinary_vector().candidate)?;
    candidate[4..8].copy_from_slice(&0i32.to_le_bytes());
    let mut signatures = fixture.signatures;
    signatures.candidate_data = snake(&candidate, 127)?;
    let block = BlockId {
        shard: ShardIdent::new(0, 0x4000000000000000).unwrap(),
        ..fixture.block
    };
    let result = signatures.build_data_for_sign(&block).map(|_| ());
    assert_eq!(result, Err(Error::InvalidData), "shard");
    Ok(())
}

#[test]
fn rejects_malformed_candidates_and_snakes() -> Result<()> {
    let vector = ordinary_vector();
    let original = hex::decode(&vector.candidate)?;
    let mut cases = vec![
        ("empty", Vec::new()),
        ("truncated", original[..119].to_vec()),
    ];
    let mut unknown = original.clone();
    unknown[..4].fill(0xff);
    cases.push(("unknown constructor", unknown));
    let mut parent = original.clone();
    parent[116..].fill(0xff);
    cases.push(("unknown parent", parent));
    let mut trailing = original.clone();
    trailing.extend_from_slice(&[0; 4]);
    cases.push(("trailing bytes", trailing));
    cases.push(("oversized", vec![0; 161]));

    for (name, bytes) in cases {
        let mut fixture = synthetic_fixture(&vector)?;
        fixture.signatures.candidate_data = snake(&bytes, 127)?;
        assert_eq!(fixture.verify(), Err(Error::InvalidData), "{name}");
    }

    let mut branching = CellBuilder::new();
    branching.store_raw(&original, original.len() as u16 * 8)?;
    branching.store_reference(Cell::empty_cell())?;
    branching.store_reference(Cell::empty_cell())?;

    let mut unaligned = CellBuilder::new();
    unaligned.store_raw(&original, original.len() as u16 * 8)?;
    unaligned.store_bit_one()?;

    let mut exotic = CellBuilder::new();
    exotic.set_exotic(true);
    exotic.store_u8(2)?; // Library-reference cell, not a candidate byte chunk.
    exotic.store_raw(&[0; 32], 256)?;

    let mut too_deep = snake(&original, 127)?;
    for _ in 0..16 {
        let mut builder = CellBuilder::new();
        builder.store_reference(too_deep)?;
        too_deep = builder.build()?;
    }

    let malformed_snakes = [
        ("branching", branching.build()?, Error::InvalidData),
        ("unaligned", unaligned.build()?, Error::InvalidData),
        ("exotic", exotic.build()?, Error::UnexpectedExoticCell),
        ("too deep", too_deep, Error::InvalidData),
    ];
    for (shape, cell, error) in malformed_snakes {
        let mut fixture = ordinary_fixture()?;
        fixture.signatures.candidate_data = cell;
        assert_eq!(fixture.verify(), Err(error), "{shape}");
    }
    Ok(())
}

#[test]
fn rejects_changed_session_slot_and_candidate() -> Result<()> {
    let fixture = ordinary_fixture()?;
    let mut changed_session = fixture.clone();
    changed_session.signatures.session_id.0[0] ^= 1;

    let mut changed_slot = fixture.clone();
    changed_slot.signatures.slot += 1;

    // Collated-file hash follows the candidate constructor and bare block ID.
    // Changing it preserves the block ID but changes the signed candidate hash.
    let mut candidate = hex::decode(&ordinary_vector().candidate)?;
    candidate[84] ^= 1;
    let mut changed_candidate = fixture;
    changed_candidate.signatures.candidate_data = snake(&candidate, 127)?;

    let cases = [
        ("session", changed_session),
        ("slot", changed_slot),
        ("candidate", changed_candidate),
    ];
    for (name, fixture) in cases {
        assert_eq!(fixture.verify(), Err(Error::InvalidSignature), "{name}");
    }
    Ok(())
}

#[test]
fn rejects_invalid_ed25519_signature() -> Result<()> {
    let mut fixture = ordinary_fixture()?;
    let mut signature = fixture.signatures.signatures.get(0u16)?.unwrap();
    signature.signature.0[0] ^= 1;
    fixture.signatures.signatures.set(0u16, signature)?;

    assert_eq!(fixture.verify(), Err(Error::InvalidSignature));
    Ok(())
}

#[test]
fn rejects_signatures_over_the_ordinary_block_payload() -> Result<()> {
    let mut fixture = ordinary_fixture()?;
    let legacy_payload = Block::build_data_for_sign(&fixture.block);
    for (index, seed) in [0x11, 0x22].into_iter().enumerate() {
        let mut signature = fixture.signatures.signatures.get(index as u16)?.unwrap();
        let key = SigningKey::from_bytes(&[seed; 32]);
        signature.signature = Signature(key.sign(&legacy_payload).to_bytes());
        fixture.signatures.signatures.set(index as u16, signature)?;
    }

    assert_eq!(fixture.verify(), Err(Error::InvalidSignature));
    Ok(())
}

#[test]
fn rejects_duplicate_and_unknown_signers() -> Result<()> {
    let fixture = ordinary_fixture()?;
    let mut duplicate = fixture.clone();
    let signature = duplicate.signatures.signatures.get(0u16)?.unwrap();
    duplicate.signatures.signatures.set(1u16, signature)?;
    assert_eq!(duplicate.verify(), Err(Error::InvalidData));

    let mut unknown = fixture;
    let mut signature = unknown.signatures.signatures.get(0u16)?.unwrap();
    signature.node_id_short = HashBytes([0xaa; 32]);
    unknown.signatures.signatures.set(0u16, signature)?;
    assert_eq!(unknown.verify(), Err(Error::InvalidData));
    Ok(())
}

#[test]
fn rejects_validator_metadata_mismatches() -> Result<()> {
    let fixture = ordinary_fixture()?;
    let mut wrong_seqno = fixture.clone();
    wrong_seqno.signatures.validator_info.catchain_seqno += 1;

    let mut wrong_hash = fixture.clone();
    wrong_hash
        .signatures
        .validator_info
        .validator_list_hash_short ^= 1;

    let mut inconsistent_subset = fixture;
    inconsistent_subset.validator_info.validator_list_hash_short ^= 1;
    inconsistent_subset.signatures.validator_info = inconsistent_subset.validator_info;

    let cases = [
        ("catchain seqno", wrong_seqno),
        ("validator hash", wrong_hash),
        ("subset hash", inconsistent_subset),
    ];
    for (name, fixture) in cases {
        assert_eq!(fixture.verify(), Err(Error::InvalidData), "{name}");
    }
    Ok(())
}

#[test]
fn rejects_incomplete_validator_subsets() -> Result<()> {
    let mut empty = ordinary_fixture()?;
    empty.validators.clear();
    assert_eq!(empty.verify(), Err(Error::InvalidData));

    let mut missing = ordinary_fixture()?;
    missing.validators.pop();
    assert_eq!(missing.verify(), Err(Error::InvalidData));
    Ok(())
}

#[test]
fn rejects_incorrect_signature_counts() -> Result<()> {
    let mut fixture = ordinary_fixture()?;
    for count in [0, 1, 3, u32::MAX] {
        fixture.signatures.signature_count = count;
        assert_eq!(fixture.verify(), Err(Error::InvalidData), "count {count}");
    }
    Ok(())
}

#[test]
fn rejects_incorrect_declared_weight() -> Result<()> {
    let mut fixture = ordinary_fixture()?;
    fixture.signatures.total_weight = 74; // The two included signers weigh 40 + 35.
    assert_eq!(fixture.verify(), Err(Error::InvalidData));
    Ok(())
}

#[test]
fn rejects_dictionary_gaps_and_missing_signatures() -> Result<()> {
    let mut gap = ordinary_fixture()?;
    let signature = gap.signatures.signatures.remove(1u16)?.unwrap();
    gap.signatures.signatures.set(2u16, signature)?;
    assert_eq!(gap.verify(), Err(Error::InvalidData));

    let mut empty = ordinary_fixture()?;
    empty.signatures.signatures = Dict::new();
    assert_eq!(empty.verify(), Err(Error::InvalidData));
    Ok(())
}

#[test]
fn enforces_strict_quorum_without_weight_overflow() -> Result<()> {
    let cases = [
        ("exactly two thirds", [1, 1, 1], 2, Err(Error::InvalidData)),
        ("more than two thirds", [2, 1, 1], 3, Ok(3)),
        (
            "large weights",
            [u64::MAX - 2, 1, 1],
            u64::MAX - 1,
            Ok(u64::MAX - 1),
        ),
        (
            "total overflow",
            [u64::MAX, 1, 1],
            u64::MAX,
            Err(Error::InvalidData),
        ),
        ("zero weight", [0, 1, 1], 1, Err(Error::InvalidData)),
    ];
    for (name, weights, declared_weight, expected) in cases {
        let mut fixture = ordinary_fixture()?;
        for (validator, weight) in fixture.validators.iter_mut().zip(weights) {
            validator.weight = weight;
        }
        fixture.validator_info.validator_list_hash_short = ValidatorSet::compute_subset_hash_short(
            &fixture.validators,
            fixture.validator_info.catchain_seqno,
        );
        fixture.signatures.validator_info = fixture.validator_info;
        fixture.signatures.total_weight = declared_weight;
        assert_eq!(fixture.verify(), expected, "{name}");
    }
    Ok(())
}

#[test]
fn rejects_duplicate_validators_in_the_trusted_subset() -> Result<()> {
    let mut fixture = ordinary_fixture()?;
    fixture.validators[2] = fixture.validators[0].clone();
    fixture.validator_info.validator_list_hash_short = ValidatorSet::compute_subset_hash_short(
        &fixture.validators,
        fixture.validator_info.catchain_seqno,
    );
    fixture.signatures.validator_info = fixture.validator_info;
    assert_eq!(fixture.verify(), Err(Error::InvalidData));
    Ok(())
}
