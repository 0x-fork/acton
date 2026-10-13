use super::{BlockId, BlockSignature};
use crate::cell::*;
use crate::dict::Dict;
use crate::error::Error;
use crate::models::shard::ValidatorBaseInfo;

/// Typed block proof.
#[derive(Clone, Debug)]
pub struct BlockProof {
    /// Id of the related block.
    pub proof_for: BlockId,
    /// Merkle proof cell.
    pub root: Cell,
    /// Optional masterchain signatures in the ordinary or Simplex format.
    pub signatures: Option<BlockSignatures>,
}

impl BlockProof {
    const TAG: u8 = 0xc3;
}

impl Store for BlockProof {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let child_cell = match &self.signatures {
            Some(signatures) => {
                let mut builder = CellBuilder::new();
                ok!(signatures.store_into(&mut builder, context));
                Some(ok!(builder.build_ext(context)))
            }
            None => None,
        };

        ok!(builder.store_u8(Self::TAG));
        ok!(self.proof_for.store_into(builder, context));
        ok!(builder.store_reference(self.root.clone()));
        match child_cell {
            Some(cell) => {
                ok!(builder.store_bit_one());
                builder.store_reference(cell)
            }
            None => builder.store_bit_zero(),
        }
    }
}

impl<'a> Load<'a> for BlockProof {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_u8() {
            Ok(Self::TAG) => {}
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        }

        Ok(Self {
            proof_for: ok!(BlockId::load_from(slice)),
            root: ok!(slice.load_reference_cloned()),
            signatures: if ok!(slice.load_bit()) {
                match slice.load_reference() {
                    Ok(cell) => Some(ok!(cell.parse::<BlockSignatures>())),
                    Err(e) => return Err(e),
                }
            } else {
                None
            },
        })
    }
}

/// Masterchain block signatures, selected by their TL-B constructor.
///
/// Loading preserves the signature data without verifying signatures, validator
/// membership, declared counts or weights, or the candidate's relation to a block.
/// Applications are responsible for these checks before trusting a proof.
#[derive(Debug, Clone)]
pub enum BlockSignatures {
    /// Ordinary block signatures (`block_signatures_ordinary#11`).
    Ordinary(OrdinaryBlockSignatures),
    /// Simplex finalization signatures (`block_signatures_simplex#12`).
    Simplex(SimplexBlockSignatures),
}

impl Store for BlockSignatures {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::Ordinary(signatures) => signatures.store_into(builder, context),
            Self::Simplex(signatures) => signatures.store_into(builder, context),
        }
    }
}

impl<'a> Load<'a> for BlockSignatures {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match ok!(slice.get_u8(0)) {
            0x11 => OrdinaryBlockSignatures::load_from(slice).map(Self::Ordinary),
            0x12 => SimplexBlockSignatures::load_from(slice).map(Self::Simplex),
            _ => Err(Error::InvalidTag),
        }
    }
}

/// Ordinary masterchain block signatures with the `#11` TL-B constructor.
///
/// This model stores the declared signature count and signer weight unchanged;
/// loading does not authenticate the signatures or validate those declarations.
#[derive(Debug, Clone, Store, Load)]
#[tlb(tag = "#11")]
pub struct OrdinaryBlockSignatures {
    /// Brief validator basic info.
    pub validator_info: ValidatorBaseInfo,
    /// Total number of signatures.
    pub signature_count: u32,
    /// Declared total weight of the validators whose signatures are included.
    pub total_weight: u64,
    /// Block signatures from all signers.
    pub signatures: Dict<u16, BlockSignature>,
}

/// Simplex masterchain finalization signatures with the `#12` TL-B constructor.
///
/// The candidate is retained as an opaque cell chain. Loading does not parse its
/// TL payload, bind it to a block, or authenticate the signatures and their weight.
#[derive(Debug, Clone, Store, Load)]
#[tlb(tag = "#12")]
pub struct SimplexBlockSignatures {
    /// Brief validator basic info.
    pub validator_info: ValidatorBaseInfo,
    /// Declared number of signatures in the dictionary.
    pub signature_count: u32,
    /// Declared total weight of the validators whose signatures are included.
    pub total_weight: u64,
    /// Finalization signatures indexed within the signature set.
    pub signatures: Dict<u16, BlockSignature>,
    /// Identifier of the Simplex consensus session which produced the signatures.
    pub session_id: HashBytes,
    /// Slot finalized within the consensus session.
    pub slot: u32,
    /// Reference to the original boxed TL candidate bytes encoded as a snake.
    ///
    /// Candidate hashing uses the bytes concatenated from this chain, without a
    /// comment prefix. The cell's representation hash and BoC are not that hash.
    pub candidate_data: Cell,
}
