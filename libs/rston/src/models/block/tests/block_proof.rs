use expect_test::expect;
use sha2::{Digest, Sha256};

use super::*;
use crate::dict::Dict;
use crate::models::shard::ValidatorBaseInfo;

const SIMPLEX_SIGNATURES: &[u8] =
    include_bytes!("../../../../tests/fixtures/simplex/mainnet-signatures.boc");

#[test]
fn simplex_signatures_fixture() {
    let cell = Boc::decode(SIMPLEX_SIGNATURES).unwrap();
    let mut slice = cell.as_slice().unwrap();
    let BlockSignatures::Simplex(mut signatures) = BlockSignatures::load_from(&mut slice).unwrap()
    else {
        panic!("expected Simplex signatures");
    };
    assert!(slice.is_empty());

    let mut dictionary = Dict::new();
    for (index, entry) in signatures.signatures.iter().enumerate() {
        let (key, signature) = entry.unwrap();
        assert_eq!(key as usize, index);
        dictionary.set(key, signature).unwrap();
    }
    signatures.signatures = dictionary;

    let mut candidate = signatures.candidate_data.as_slice().unwrap();
    let head_bits = candidate.size_bits();
    let mut candidate_bytes = vec![0; head_bits as usize / 8];
    candidate.load_raw(&mut candidate_bytes, head_bits).unwrap();
    let mut tail = candidate.load_reference_as_slice().unwrap();
    assert!(candidate.is_empty());
    let tail_bits = tail.size_bits();
    let mut tail_bytes = vec![0; tail_bits as usize / 8];
    tail.load_raw(&mut tail_bytes, tail_bits).unwrap();
    assert!(tail.is_empty());
    candidate_bytes.extend_from_slice(&tail_bytes);

    expect![[r#"
        validator_list_hash_short: 569050026
        catchain_seqno: 871222
        signature_count: 68
        total_weight: 302390044705312660
        session_id: b2f3724bba0903eea9781179ce2c1405dc2c6e0813c5a75ce7b0f0ee9ccc5991
        slot: 454
        candidate_bits: 1016 + 264
        candidate_bytes: 160
        candidate_sha256: 4533e402b8a3bdb0c86de4f78841945c94cc7febc4d0d9fe073fcabc7d54cc8c
        cell_hash: a3dc62ec4b813552cbe002d9780570adff633be59f6e38e0d98e0bca05a97fda
    "#]]
    .assert_eq(&format!(
        concat!(
            "validator_list_hash_short: {}\n",
            "catchain_seqno: {}\n",
            "signature_count: {}\n",
            "total_weight: {}\n",
            "session_id: {}\n",
            "slot: {}\n",
            "candidate_bits: {} + {}\n",
            "candidate_bytes: {}\n",
            "candidate_sha256: {}\n",
            "cell_hash: {}\n",
        ),
        signatures.validator_info.validator_list_hash_short,
        signatures.validator_info.catchain_seqno,
        signatures.signature_count,
        signatures.total_weight,
        signatures.session_id,
        signatures.slot,
        head_bits,
        tail_bits,
        candidate_bytes.len(),
        hex::encode(Sha256::digest(&candidate_bytes)),
        cell.repr_hash(),
    ));

    assert_eq!(serialize_any(&signatures), cell);
    assert_eq!(serialize_any(BlockSignatures::Simplex(signatures)), cell);
}

#[test]
fn ordinary_signatures_fixture() {
    let proof_cell = Boc::decode(include_bytes!("mc_block_proof.boc")).unwrap();
    let proof = proof_cell.parse::<BlockProof>().unwrap();
    let Some(BlockSignatures::Ordinary(signatures)) = proof.signatures else {
        panic!("expected ordinary signatures");
    };
    let signature_cell = proof_cell.reference_cloned(1).unwrap();

    assert_eq!(serialize_any(&signatures), signature_cell);
    let parsed = signature_cell.parse::<OrdinaryBlockSignatures>().unwrap();
    assert_eq!(serialize_any(parsed), signature_cell);
    assert_eq!(
        serialize_any(BlockSignatures::Ordinary(signatures)),
        signature_cell,
    );
}

#[test]
fn simplex_block_proof_roundtrip() {
    let signatures_cell = Boc::decode(SIMPLEX_SIGNATURES).unwrap();
    let proof = BlockProof {
        proof_for: BlockId {
            shard: ShardIdent::MASTERCHAIN,
            seqno: 98841714,
            root_hash: "aab0320a4e3a90f3155f3e8b3a9b9506822e8663ab6493b45d059de9aa67e98f"
                .parse()
                .unwrap(),
            file_hash: "a00ed9197c5f968b8ef5f693f531d0564bb594e573b08dd7892c5f39e942b762"
                .parse()
                .unwrap(),
        },
        root: CellBuilder::build_from(42u32).unwrap(),
        signatures: Some(signatures_cell.parse().unwrap()),
    };
    let proof_cell = serialize_any(&proof);
    assert_eq!(proof_cell.as_slice().unwrap().get_u8(0), Ok(0xc3));
    assert_eq!(proof_cell.reference_cloned(1).unwrap(), signatures_cell);

    let parsed = Boc::decode(Boc::encode(&proof_cell))
        .unwrap()
        .parse::<BlockProof>()
        .unwrap();
    assert_eq!(parsed.proof_for, proof.proof_for);
    assert_eq!(parsed.root, proof.root);
    assert!(matches!(
        parsed.signatures,
        Some(BlockSignatures::Simplex(_))
    ));
    assert_eq!(serialize_any(parsed), proof_cell);
}

#[test]
fn empty_signature_dictionaries_roundtrip() {
    let validator_info = ValidatorBaseInfo {
        validator_list_hash_short: u32::MAX,
        catchain_seqno: u32::MAX,
    };
    let variants = [
        BlockSignatures::Ordinary(OrdinaryBlockSignatures {
            validator_info,
            signature_count: 0,
            total_weight: 0,
            signatures: Dict::new(),
        }),
        BlockSignatures::Simplex(SimplexBlockSignatures {
            validator_info,
            signature_count: 0,
            total_weight: 0,
            signatures: Dict::new(),
            session_id: HashBytes([0xff; 32]),
            slot: u32::MAX,
            candidate_data: Cell::empty_cell(),
        }),
    ];

    for signatures in variants {
        let cell = serialize_any(signatures);
        let mut slice = cell.as_slice().unwrap();
        let parsed = BlockSignatures::load_from(&mut slice).unwrap();
        assert!(slice.is_empty());
        assert_eq!(serialize_any(parsed), cell);
    }
}

#[test]
fn signature_tags_and_truncated_data() {
    let cell = CellBuilder::build_from(0x13u8).unwrap();
    assert!(matches!(
        cell.parse::<BlockSignatures>(),
        Err(Error::InvalidTag)
    ));

    let mut short_tag = CellBuilder::new();
    short_tag.store_small_uint(0x11, 7).unwrap();
    let cells = [
        Cell::empty_cell(),
        short_tag.build().unwrap(),
        CellBuilder::build_from(0x11u8).unwrap(),
        CellBuilder::build_from(0x12u8).unwrap(),
    ];
    for cell in cells {
        assert!(matches!(
            cell.parse::<BlockSignatures>(),
            Err(Error::CellUnderflow)
        ));
    }

    let cell = Boc::decode(SIMPLEX_SIGNATURES).unwrap();
    for references in 0..2 {
        let mut slice = cell.as_slice().unwrap();
        slice.only_first(cell.bit_len(), references).unwrap();
        assert!(matches!(
            BlockSignatures::load_from(&mut slice),
            Err(Error::CellUnderflow)
        ));
    }

    for bits in [72, 104, 168, 169, 425, 456] {
        let mut slice = cell.as_slice().unwrap();
        slice.only_first(bits, cell.reference_count()).unwrap();
        assert!(matches!(
            BlockSignatures::load_from(&mut slice),
            Err(Error::CellUnderflow)
        ));
    }
}

#[test]
fn signatures_inside_a_slice() {
    let cells = [
        Boc::decode(include_bytes!("mc_block_proof.boc"))
            .unwrap()
            .reference_cloned(1)
            .unwrap(),
        Boc::decode(SIMPLEX_SIGNATURES).unwrap(),
    ];

    for cell in cells {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(5, 3).unwrap();
        builder.store_reference(Cell::empty_cell()).unwrap();
        cell.parse::<BlockSignatures>()
            .unwrap()
            .store_into(&mut builder, Cell::empty_context())
            .unwrap();
        builder.store_u8(0xab).unwrap();
        let wrapper = builder.build().unwrap();

        let mut slice = wrapper.as_slice().unwrap();
        assert_eq!(slice.load_small_uint(3), Ok(5));
        slice.load_reference().unwrap();
        let signatures = BlockSignatures::load_from(&mut slice).unwrap();
        assert_eq!(slice.load_u8(), Ok(0xab));
        assert!(slice.is_empty());
        assert_eq!(serialize_any(signatures), cell);
    }
}
