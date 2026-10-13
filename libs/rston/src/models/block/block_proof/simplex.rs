use sha2::{Digest, Sha256};
use tl_proto::{BoxedConstructor, BoxedWrapper, TlRead, TlWrite};

use super::{BlockId, Error, SimplexBlockSignatures, ValidatorBaseInfo};
use crate::models::block::BlockSignatureExt;
use crate::models::config::{ValidatorDescription, ValidatorSet};

impl SimplexBlockSignatures {
    /// Builds the boxed `consensus.dataToSign` payload for a finalization vote.
    ///
    /// The snake must contain one complete ordinary or empty TL candidate for
    /// `block_id`. Its original bytes are hashed, regardless of cell boundaries.
    /// It may contain at most 160 bytes in 16 ordinary, byte-aligned cells.
    /// Each cell may reference only its next chunk.
    /// This method does not verify signatures or authenticate the session identifier.
    pub fn build_data_for_sign(&self, block_id: &BlockId) -> Result<Vec<u8>, Error> {
        // The supported candidate constructors occupy at most 160 bytes.
        // TON's CellString writer limits a snake to 16 cells.
        const MAX_CANDIDATE_BYTES: usize = 160;
        const MAX_CANDIDATE_CELLS: usize = 16;

        let mut data = Vec::with_capacity(MAX_CANDIDATE_BYTES);
        let mut slice = self.candidate_data.as_slice()?;
        for depth in 0..MAX_CANDIDATE_CELLS {
            let bits = slice.size_bits();
            let new_len = data.len() + usize::from(bits / 8);
            if bits % 8 != 0 || slice.size_refs() > 1 || new_len > MAX_CANDIDATE_BYTES {
                return Err(Error::InvalidData);
            }

            let offset = data.len();
            data.resize(new_len, 0);
            slice.load_raw(&mut data[offset..], bits)?;
            if slice.size_refs() == 0 {
                break;
            }
            if depth + 1 == MAX_CANDIDATE_CELLS {
                return Err(Error::InvalidData);
            }
            slice = slice.load_reference_as_slice()?;
        }

        let mut remaining = data.as_slice();
        let candidate =
            CandidateHashData::read_from(&mut remaining).map_err(|_| Error::InvalidData)?;
        let candidate_block = match candidate {
            CandidateHashData::Ordinary { block, .. } | CandidateHashData::Empty { block, .. } => {
                block
            }
        };
        if !remaining.is_empty()
            || candidate_block.workchain != block_id.shard.workchain()
            || candidate_block.shard != block_id.shard.prefix()
            || candidate_block.seqno != block_id.seqno
            || candidate_block.root_hash != block_id.root_hash.0
            || candidate_block.file_hash != block_id.file_hash.0
        {
            return Err(Error::InvalidData);
        }

        let vote = tl_proto::serialize(FinalizeVote {
            id: BoxedWrapper(CandidateId {
                slot: self.slot,
                hash: Sha256::digest(&data).into(),
            }),
        });
        Ok(tl_proto::serialize(DataToSign {
            session_id: self.session_id.0,
            data: &vote,
        }))
    }

    /// Verifies Simplex finalization signatures and returns the signed weight.
    ///
    /// The caller supplies an independently authenticated block identifier and
    /// validator metadata, plus the ordered validator subset for that block.
    /// Its public keys must be unique, its weights positive, and its total weight
    /// representable as `u64`.
    /// Use [`ValidatorSet::compute_subset`] to select it from trusted configuration.
    /// This method does not verify a Merkle proof or establish configuration trust.
    ///
    /// Checks include the candidate's full block identifier, validator metadata
    /// and subset hash, contiguous dictionary indexes, count, Ed25519 signatures,
    /// declared signer weight, and a quorum strictly greater than two thirds.
    /// Duplicate or unknown signers are rejected. Weight arithmetic cannot wrap.
    ///
    /// Malformed data, metadata mismatches, and insufficient weight return
    /// [`Error::InvalidData`]. Failed Ed25519 checks return [`Error::InvalidSignature`].
    /// Cell decoding errors are propagated.
    pub fn check_signatures(
        &self,
        block_id: &BlockId,
        validator_info: &ValidatorBaseInfo,
        validators: &[ValidatorDescription],
    ) -> Result<u64, Error> {
        if validators.is_empty()
            || self.signature_count == 0
            || self.signature_count as usize > validators.len()
            || self.validator_info != *validator_info
            || ValidatorSet::compute_subset_hash_short(validators, validator_info.catchain_seqno)
                != validator_info.validator_list_hash_short
        {
            return Err(Error::InvalidData);
        }

        let mut public_keys =
            ahash::HashSet::with_capacity_and_hasher(validators.len(), Default::default());
        let mut total_weight = 0u64;
        for validator in validators {
            if validator.weight == 0 || !public_keys.insert(validator.public_key) {
                return Err(Error::InvalidData);
            }
            total_weight = total_weight
                .checked_add(validator.weight)
                .ok_or(Error::InvalidData)?;
        }

        let mut count = 0usize;
        for key in self.signatures.keys() {
            if usize::from(key?) != count || count >= self.signature_count as usize {
                return Err(Error::InvalidData);
            }
            count += 1;
        }
        if count != self.signature_count as usize {
            return Err(Error::InvalidData);
        }

        let data = self.build_data_for_sign(block_id)?;
        let signed_weight = self.signatures.check_signatures(validators, &data)?;
        if signed_weight != self.total_weight
            || u128::from(signed_weight) * 3 <= u128::from(total_weight) * 2
        {
            return Err(Error::InvalidData);
        }
        Ok(signed_weight)
    }
}

#[derive(TlRead, TlWrite)]
struct BlockIdExt {
    workchain: i32,
    shard: u64,
    seqno: u32,
    root_hash: [u8; 32],
    file_hash: [u8; 32],
}

#[derive(TlRead, TlWrite)]
struct CandidateId {
    slot: u32,
    hash: [u8; 32],
}

impl BoxedConstructor for CandidateId {
    const TL_ID: u32 = tl_proto::id!(
        "consensus.candidateId",
        scheme = "models/block/block_proof/simplex.tl"
    );
}

#[derive(TlRead, TlWrite)]
#[tl(boxed, scheme = "models/block/block_proof/simplex.tl")]
enum CandidateParent {
    #[tl(id = "consensus.candidateParent")]
    Candidate { id: BoxedWrapper<CandidateId> },
    #[tl(id = "consensus.candidateWithoutParents")]
    WithoutParents,
}

#[derive(TlRead, TlWrite)]
#[tl(boxed, scheme = "models/block/block_proof/simplex.tl")]
enum CandidateHashData {
    #[tl(id = "consensus.candidateHashDataOrdinary")]
    Ordinary {
        block: BlockIdExt,
        collated_file_hash: [u8; 32],
        parent: CandidateParent,
    },
    #[tl(id = "consensus.candidateHashDataEmpty")]
    Empty {
        block: BlockIdExt,
        parent: CandidateId,
    },
}

#[derive(TlWrite)]
#[tl(
    boxed,
    id = "consensus.simplex.finalizeVote",
    scheme = "models/block/block_proof/simplex.tl"
)]
struct FinalizeVote {
    id: BoxedWrapper<CandidateId>,
}

#[derive(TlWrite)]
#[tl(
    boxed,
    id = "consensus.dataToSign",
    scheme = "models/block/block_proof/simplex.tl"
)]
struct DataToSign<'tl> {
    session_id: [u8; 32],
    data: &'tl [u8],
}
