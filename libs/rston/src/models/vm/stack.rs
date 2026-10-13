//! TVM stack values and their cell codec for get-method arguments and results.

use super::snake_string::{
    build_snake_bytes_cell, parse_snake_bytes_cell, parse_snake_bytes_from_slice,
    parse_snake_string_from_slice,
};
use crate::cell::{Cell, CellBuilder, CellContext, CellFamily, CellSlice, Load, Store};
use crate::dict::RawDict;
use crate::error::Error;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::ops::{Deref, DerefMut};

/// Ordered TVM values used for both stacks and nested tuples.
/// Stack index zero is the bottom; the final element is the top.
#[derive(Default, Debug, Clone, Eq)]
pub struct Tuple(
    /// Values in stack order, or field order for a nested tuple.
    pub Vec<TupleItem>,
);

impl Tuple {
    /// Create an empty tuple.
    #[must_use]
    pub const fn empty() -> Tuple {
        Tuple(vec![])
    }

    /// Unwrap an empty tuple.
    ///
    /// ```text
    /// (()) -> ()
    /// ```
    #[must_use]
    pub fn unwrap_empty(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.first()
            && item.is_empty()
        {
            return Tuple(vec![]);
        }

        (*self).clone()
    }

    /// Unwrap a single item tuple.
    ///
    /// ```text
    /// ((x)) -> (x)
    /// ```
    #[must_use]
    pub fn unwrap_single(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.first()
            && item.len() == 1
        {
            return Tuple(vec![item[0].clone()]);
        }

        (*self).clone()
    }

    /// Return the first nested tuple's values when present, otherwise clone this tuple.
    #[must_use]
    pub fn unwrap_tuple(&self) -> Tuple {
        if let Some(TupleItem::Tuple(item)) = &self.0.first() {
            return Tuple(item.0.clone());
        }

        (*self).clone()
    }

    /// Push a boolean value to the tuple.
    ///
    /// In TVM `true` is represented as `-1` and `false` is represented as `0`.
    pub fn push_bool(&mut self, v: bool) {
        self.push(TupleItem::Int(if v {
            BigInt::from(-1)
        } else {
            BigInt::ZERO
        }));
    }

    /// Compare values, treating UTF-8 snake cells and slices with equal text as equal.
    /// Other values use their normal structural equality.
    #[must_use]
    pub fn equal_to(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter().zip(other.iter()).all(|(a, b)| a.equal_to(b))
    }

    /// Encode a complete `VmStack` cell using the default cell context.
    pub fn serialize(&self) -> Result<Cell, Error> {
        CellBuilder::build_from(self)
    }

    /// Read a `VmStack`, allowing exotic root cells and unused trailing data.
    pub fn deserialize(src: &Cell) -> Result<Tuple, Error> {
        Self::load_from(&mut src.as_slice_allow_exotic())
    }

    /// Parse a snake string from a cell.
    ///
    /// If the slice is not a snake string, returns `None`.
    /// This is tricky since we cannot be sure that the slice is a snake string and
    /// not some other data with 8-bit encoding that forms a valid UTF-8 string.
    #[must_use]
    pub fn parse_snake_string(cell: &Cell) -> Option<String> {
        String::from_utf8(parse_snake_bytes_cell(cell)?).ok()
    }

    /// Read byte-aligned data along the cell's first-reference chain.
    ///
    /// Other references are ignored. Returns `None` if data is not byte-aligned
    /// or cannot be read.
    #[must_use]
    pub fn parse_snake_bytes(cell: &Cell) -> Option<Vec<u8>> {
        parse_snake_bytes_cell(cell)
    }

    /// Parse a snake string from a cell slice (parser).
    ///
    /// If the slice is not a snake string, returns `None`.
    /// This is tricky since we cannot be sure that the slice is a snake string and
    /// not some other data with 8-bit encoding that forms a valid UTF-8 string.
    #[must_use]
    pub fn parse_snake_string_slice(parser: &mut CellSlice<'_>) -> Option<String> {
        parse_snake_string_from_slice(parser)
    }

    /// Read byte-aligned data from this slice and its first-reference chain.
    ///
    /// Advances the supplied slice through its data and first reference.
    /// Other references are ignored. Returns `None` if data is not byte-aligned
    /// or cannot be read; a failed read can leave the slice partially consumed.
    #[must_use]
    pub fn parse_snake_bytes_slice(parser: &mut CellSlice<'_>) -> Option<Vec<u8>> {
        parse_snake_bytes_from_slice(parser)
    }

    /// Push a snake string to the tuple as a TVM slice.
    ///
    /// If the string is too long, it will be split into multiple cells automatically.
    pub fn push_string_slice(&mut self, s: &str) {
        self.push_bytes(s.as_bytes());
    }

    /// Push a snake string to the tuple as a TVM cell.
    ///
    /// If the string is too long, it will be split into multiple cells automatically.
    pub fn push_string(&mut self, s: &str) {
        self.push(TupleItem::Cell(build_snake_bytes_cell(s.as_bytes())));
    }

    /// Push bytes as a TVM slice, splitting long data into a snake cell chain.
    ///
    /// If the array is too long, it will be split into multiple cells automatically.
    pub fn push_bytes(&mut self, bytes: &[u8]) {
        self.push(TupleItem::Slice(build_snake_bytes_cell(bytes)));
    }
}

impl Deref for Tuple {
    type Target = Vec<TupleItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Tuple {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PartialEq for Tuple {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

/// Parsed data from a TVM continuation (`VmCont`).
///
/// Stores the code cell, optionally the captured stack, and the save list from `VmControlData`.
/// Encoding emits a standard continuation with codepage zero. Decoding normalizes
/// supported envelope and push-int forms; this is not a lossless `VmCont` model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContData {
    /// The code cell (`VmCellSlice`) of the continuation.
    pub code: Cell,
    /// The captured stack from `VmControlData` (set by SETCONTARGS).
    pub stack: Option<Tuple>,
    /// The save list (`VmSaveList`) — maps register indices to saved `VmStackValues`.
    /// Stored as raw hashmap cell for round-trip serialization.
    /// Key registers: c0 = return cont, c1 = alt return cont (RETALT), etc.
    pub savelist: Option<Cell>,
    /// Arity metadata from `VmControlData` (set by SETCONTARGS). When present,
    /// the continuation expects exactly this many arguments at invocation time;
    /// must be preserved across parse/serialize or call semantics change.
    pub nargs: Option<u16>,
}

impl ContData {
    /// Create a `ContData` with just a code cell and no captured stack.
    #[must_use]
    pub const fn from_code(code: Cell) -> Self {
        Self {
            code,
            stack: None,
            savelist: None,
            nargs: None,
        }
    }
}

/// Represents a stack value in TVM
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TupleItem {
    /// The TVM null value, distinct from an empty tuple or empty cell.
    #[default]
    Null,
    /// A signed integer. Encoding selects int64 or int257 and rejects larger values.
    Int(BigInt),
    /// TVM's invalid-integer sentinel, separate from a numeric zero.
    Nan,
    /// Continuation data normalized to code, captured stack, save list, and arity.
    Cont(ContData),
    /// An immutable cell passed by reference in the stack encoding.
    Cell(Cell),
    /// A materialized cell containing the selected slice bits and references.
    /// Original slice bounds and the original backing cell are not retained.
    Slice(Cell),
    /// Builder contents represented by their finalized cell.
    Builder(Cell),
    /// Nested tuple values, encoded with the tuple's 16-bit length field.
    Tuple(Tuple),
}

impl TupleItem {
    /// Build the two-level tuple layout used by Tolk BigArray values.
    ///
    /// # Panics
    /// Panics when more than 65,025 values are supplied.
    #[must_use]
    pub fn big_array_from_items(v: Vec<TupleItem>) -> Self {
        const BIN_SIZE: usize = 255;
        const MAX_SIZE: usize = BIN_SIZE * BIN_SIZE;

        let size = v.len();
        assert!(
            size <= MAX_SIZE,
            "BigArray supports at most {MAX_SIZE} items, got {size}"
        );

        // BigArray layout in stack tuple:
        // [topLevel: array<array<T>>, size: int]
        // topLevel stores bins of up to 255 items and keeps only used bins.
        let mut bins = vec![Vec::<TupleItem>::new(); size.div_ceil(BIN_SIZE)];
        for (index, value) in v.into_iter().enumerate() {
            let bin_idx = index / BIN_SIZE;
            bins[bin_idx].push(value);
        }

        let top_level = bins
            .into_iter()
            .map(|bin| Self::Tuple(Tuple(bin)))
            .collect::<Vec<_>>();

        Self::Tuple(Tuple(vec![
            Self::Tuple(Tuple(top_level)),
            Self::Int(BigInt::from(size)),
        ]))
    }

    /// Build a BigArray of integers with the same size bound as `big_array_from_items`.
    pub fn big_array_from_vec(v: Vec<BigInt>) -> Self {
        Self::big_array_from_items(v.into_iter().map(Self::Int).collect())
    }

    fn equal_to(&self, other: &Self) -> bool {
        // Since strings can be build in different cells, we need to compare string values.
        if let TupleItem::Cell(left) | TupleItem::Slice(left) = self
            && let TupleItem::Cell(right) | TupleItem::Slice(right) = other
            && let Some(left_str) = Tuple::parse_snake_string(left)
            && let Some(right_str) = Tuple::parse_snake_string(right)
        {
            return left_str == right_str;
        }

        self == other
    }

    /// Unwrap a single item tuple.
    ///
    /// ```text
    /// (x) -> x
    /// ```
    #[must_use]
    pub fn unwrap_single(&self) -> TupleItem {
        let TupleItem::Tuple(items) = self else {
            return (*self).clone();
        };

        if items.len() == 1 {
            return items[0].clone();
        }

        (*self).clone()
    }
}

/// Serialize a tuple item to a cell builder
impl Store for TupleItem {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            TupleItem::Null => {
                builder.store_small_uint(0x00, 8)?;
            }
            TupleItem::Int(value) => {
                // Check if value fits in int64
                if value <= &BigInt::from(9223372036854775807i64)
                    && value >= &BigInt::from(-9223372036854775808i64)
                {
                    builder.store_small_uint(0x01, 8)?;
                    let int64 = value.to_i64().ok_or(Error::IntOverflow)?;
                    builder.store_u64(int64 as u64)?;
                    return Ok(());
                }

                // Use int257 for larger values
                builder.store_small_uint(0x02, 8)?;
                builder.store_uint(0, 7)?;
                builder.store_bigint(value, 257, true)?;
            }
            TupleItem::Cont(cont) => {
                // vm_stk_cont#06 cont:VmCont = VmStackValue;
                builder.store_small_uint(0x06, 8)?;
                cont.store_into(builder, context)?;
            }
            TupleItem::Nan => {
                builder.store_small_uint(0x02, 8)?;
                builder.store_small_uint(0xff, 8)?;
            }
            TupleItem::Cell(cell) => {
                builder.store_small_uint(0x03, 8)?;
                builder.store_reference(cell.clone())?;
            }
            TupleItem::Slice(cell) => {
                builder.store_small_uint(0x04, 8)?;
                builder.store_uint(0, 10)?;
                builder.store_uint(u64::from(cell.bit_len()), 10)?;
                builder.store_uint(0, 3)?;
                builder.store_uint(u64::from(cell.reference_count()), 3)?;
                builder.store_reference(cell.clone())?;
            }
            TupleItem::Builder(cell) => {
                builder.store_small_uint(0x05, 8)?;
                builder.store_reference(cell.clone())?;
            }
            TupleItem::Tuple(items) => {
                let mut head: Option<Cell> = None;
                let mut tail: Option<Cell> = None;

                for (i, item) in items.iter().enumerate() {
                    std::mem::swap(&mut head, &mut tail);

                    if i > 1 {
                        let mut bc = CellBuilder::new();
                        if let Some(tail) = tail.as_ref() {
                            bc.store_reference(tail.clone())?;
                        }
                        if let Some(head) = head.as_ref() {
                            bc.store_reference(head.clone())?;
                        }
                        head = Some(bc.build_ext(context)?);
                    }

                    let mut bc = CellBuilder::new();
                    item.store_into(&mut bc, context)?;
                    tail = Some(bc.build_ext(context)?);
                }

                builder.store_small_uint(0x07, 8)?;
                builder.store_u16(items.len() as u16)?;
                if let Some(h) = &head {
                    builder.store_reference(h.clone())?;
                }
                if let Some(t) = &tail {
                    builder.store_reference(t.clone())?;
                }
            }
        }
        Ok(())
    }
}

/// Read the encoded slice bounds and materialize the selected bits and references.
/// The original backing cell and its offsets are not retained.
/// Reversed bit bounds produce empty data; reversed reference bounds select no references.
pub fn parse_vm_cell_slice(parser: &mut CellSlice<'_>) -> Result<Cell, Error> {
    let start_bits = parser.load_uint(10)? as u16;
    let end_bits = parser.load_uint(10)? as u16;
    let start_refs = parser.load_uint(3)? as u8;
    let end_refs = parser.load_uint(3)? as u8;

    let cell_ref = parser.load_reference_cloned()?;

    let mut parser = cell_ref.as_slice_allow_exotic();
    parser.skip_first(start_bits, start_refs)?;

    let root_data_size = end_bits.saturating_sub(start_bits);
    let mut root_bits = vec![0u8; root_data_size.div_ceil(8) as usize];
    parser.load_raw(&mut root_bits, root_data_size)?;

    let mut builder = CellBuilder::new();
    builder.store_raw(&root_bits, root_data_size)?;

    for _ in start_refs..end_refs {
        let next_ref = parser.load_reference_cloned()?;
        builder.store_reference(next_ref)?;
    }

    let final_cell = builder.build()?;

    Ok(final_cell)
}

/// Parse `VmStack` from a cell slice.
///
/// ```text
/// vm_stack#_ depth:(## 24) stack:(VmStackList depth) = VmStack;
/// vm_stk_cons#_ {n:#} rest:^(VmStackList n) tos:VmStackValue = VmStackList (n + 1);
/// vm_stk_nil#_ = VmStackList 0;
/// ```
impl<'a> Load<'a> for Tuple {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let size = parser.load_uint(24)? as usize;
        if size == 0 {
            return Ok(Tuple::empty());
        }

        let mut result: Vec<TupleItem> = Vec::with_capacity(size);

        // First entry: rest reference + tos inline in current parser
        let next_ref = parser.load_reference_cloned()?;
        let item = TupleItem::load_from(parser)?;
        result.insert(0, item);

        // Remaining entries from referenced cells
        let mut cur_cell = next_ref;
        for _ in 1..size {
            let mut cs = cur_cell.as_slice_allow_exotic();
            let nr = cs.load_reference_cloned()?;
            let item = TupleItem::load_from(&mut cs)?;
            result.insert(0, item);
            cur_cell = nr;
        }

        Ok(Tuple(result))
    }
}

/// Serialize `VmStack` into a cell builder.
impl Store for Tuple {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        builder.store_uint(self.len() as u64, 24)?;
        serialize_tuple_tail(&self.0, builder, context)
    }
}

/// Parsed `VmControlData` fields.
struct VmControlData {
    nargs: Option<u16>,
    stack: Option<Tuple>,
    savelist: Option<Cell>,
}

/// Parse `VmControlData`, returning the captured stack and save list.
///
/// ```text
/// vm_ctl_data$_ nargs:(Maybe uint13) stack:(Maybe VmStack)
///               save:VmSaveList cp:(Maybe int16) = VmControlData;
/// _ cregs:(HashmapE 4 VmStackValue) = VmSaveList;
/// ```
fn parse_vm_control_data(parser: &mut CellSlice<'_>) -> Result<VmControlData, Error> {
    // nargs:(Maybe uint13)
    let nargs = if parser.load_bit()? {
        Some(parser.load_uint(13)? as u16)
    } else {
        None
    };

    // stack:(Maybe VmStack)
    let stack = if parser.load_bit()? {
        Some(Tuple::load_from(parser)?)
    } else {
        None
    };

    // save:VmSaveList = HashmapE 4 VmStackValue
    let savelist = if parser.load_bit()? {
        Some(parser.load_reference_cloned()?)
    } else {
        None
    };

    // cp:(Maybe int16)
    if parser.load_bit()? {
        parser.load_uint(16)?;
    }

    Ok(VmControlData {
        nargs,
        stack,
        savelist,
    })
}

/// Encode normalized continuation data as `vmc_std$00` with codepage zero.
impl Store for ContData {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        // vmc_std$00
        builder.store_uint(0b00, 2)?;

        // VmControlData:
        // nargs:(Maybe uint13) — preserve the continuation's arity metadata.
        // SETCONTARGS-produced continuations carry this and it changes call semantics,
        // so we must round-trip it rather than dropping it.
        if let Some(nargs) = self.nargs {
            builder.store_bit(true)?;
            builder.store_uint(u64::from(nargs), 13)?;
        } else {
            builder.store_bit(false)?;
        }

        // stack:(Maybe VmStack)
        if let Some(stack) = &self.stack {
            builder.store_bit(true)?;
            stack.store_into(builder, context)?;
        } else {
            builder.store_bit(false)?;
        }

        // save:VmSaveList = HashmapE 4 VmStackValue
        if let Some(savelist) = &self.savelist {
            builder.store_bit(true)?;
            builder.store_reference(savelist.clone())?;
        } else {
            builder.store_bit(false)?;
        }

        // cp:(Maybe int16) — always serialize as codepage 0
        builder.store_bit(true)?;
        builder.store_uint(0, 16)?;

        // code:VmCellSlice
        let code = &self.code;
        builder.store_uint(0, 10)?; // st_bits
        builder.store_uint(u64::from(code.bit_len()), 10)?; // end_bits
        builder.store_uint(0, 3)?; // st_ref
        builder.store_uint(u64::from(code.reference_count()), 3)?; // end_ref
        builder.store_reference(code.clone())?;

        Ok(())
    }
}

/// Normalize standard, envelope, and push-int continuations into captured data.
/// Other constructors return `Error::InvalidTag`.
#[allow(clippy::branches_sharing_code)]
impl<'a> Load<'a> for ContData {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let first_bit = parser.load_bit()?;

        if !first_bit {
            // Tags starting with 0: vmc_std (00) or vmc_envelope (01)
            let second_bit = parser.load_bit()?;

            if !second_bit {
                // vmc_std$00 cdata:VmControlData code:VmCellSlice
                let cdata = parse_vm_control_data(parser)?;
                let code = parse_vm_cell_slice(parser)?;
                Ok(ContData {
                    code,
                    stack: cdata.stack,
                    savelist: cdata.savelist,
                    nargs: cdata.nargs,
                })
            } else {
                // vmc_envelope$01 cdata:VmControlData next:^VmCont
                let cdata = parse_vm_control_data(parser)?;
                let next = parser.load_reference_cloned()?;
                let mut next_parser = next.as_slice_allow_exotic();
                let inner = ContData::load_from(&mut next_parser)?;

                let merged_stack = match (cdata.stack, inner.stack) {
                    (Some(outer), Some(inner)) => {
                        let mut combined = inner;
                        combined.0.extend(outer.0);
                        Some(combined)
                    }
                    (s @ Some(_), None) | (None, s @ Some(_)) => s,
                    (None, None) => None,
                };

                let merged_savelist = match (cdata.savelist, inner.savelist) {
                    (Some(outer), Some(inner_sl)) => {
                        let mut merged: RawDict<4> = Some(inner_sl).into();
                        let outer_dict: RawDict<4> = Some(outer).into();
                        for entry in outer_dict.iter() {
                            let (key, value): (_, CellSlice<'_>) = entry?;
                            merged.set_ext(key.as_data_slice(), &value, Cell::empty_context())?;
                        }
                        merged.into_root()
                    }
                    (s @ Some(_), None) | (None, s @ Some(_)) => s,
                    (None, None) => None,
                };

                // vmc_envelope carries its own arity metadata. Prefer the envelope's
                // nargs (it's the outer invocation), and fall back to the wrapped
                // continuation's nargs if the envelope didn't set one.
                let merged_nargs = cdata.nargs.or(inner.nargs);

                Ok(ContData {
                    code: inner.code,
                    stack: merged_stack,
                    savelist: merged_savelist,
                    nargs: merged_nargs,
                })
            }
        } else {
            // Tags starting with 1
            let second_bit = parser.load_bit()?;

            if !second_bit {
                // Tags starting with 10
                let third_bit = parser.load_bit()?;

                if !third_bit {
                    // Tags starting with 100. Both variants are control-flow terminators
                    // (`vmc_quit` with an exit code, `vmc_quit_exc`) that cannot be re-emitted
                    // as a plain `vmc_std` without losing their semantics. Reject them rather
                    // than silently collapsing to an empty `ContData`.
                    parser.load_bit()?;
                    Err(Error::InvalidTag)
                } else {
                    // Tags starting with 101
                    parser.load_bit()?;

                    // Repeat and undefined 1011 forms cannot be represented by ContData.
                    Err(Error::InvalidTag)
                }
            } else {
                // Tags starting with 11
                let third_bit = parser.load_bit()?;

                if !third_bit {
                    // Tags starting with 110
                    let fourth_bit = parser.load_bit()?;

                    if !fourth_bit {
                        // Tags starting with 1100. All four variants encode loops that cannot
                        // be re-emitted as `vmc_std` without losing semantics, so reject them
                        // up-front rather than collapsing them to their `body` continuation.
                        parser.load_uint(2)?;
                        Err(Error::InvalidTag)
                    } else {
                        // Tags starting with 1101 — undefined
                        Err(Error::InvalidTag)
                    }
                } else {
                    // Tags starting with 111
                    let fourth_bit = parser.load_bit()?;

                    if fourth_bit {
                        // vmc_pushint$1111 value:int32 next:^VmCont
                        let int_to_push = parser.load_bigint(32, true)?; // value
                        let next = parser.load_reference_cloned()?;
                        let mut np = next.as_slice_allow_exotic();
                        let mut cont = ContData::load_from(&mut np)?;
                        // `vmc_pushint` pushes `value` BEFORE running `next`. When nested
                        // (outer pushes A, inner pushes B, ...), the outer push happens first
                        // temporally, so in the flattened captured stack `value` must end up
                        // BELOW everything `next` contributed. In this representation index 0
                        // is the bottom of the stack and the last element is the top, so the
                        // outer value has to be inserted at index 0 rather than pushed at end —
                        // otherwise nested `vmc_pushint` continuations end up with their
                        // captured arguments in reverse order.
                        let mut stack = cont.stack.unwrap_or_else(Tuple::empty);
                        stack.0.insert(0, TupleItem::Int(int_to_push));
                        cont.stack = Some(stack);
                        Ok(cont)
                    } else {
                        // Tags starting with 1110 — undefined
                        Err(Error::InvalidTag)
                    }
                }
            }
        }
    }
}

impl<'a> Load<'a> for TupleItem {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let kind = parser.load_small_uint(8)?;

        match kind {
            0 => Ok(TupleItem::Null),
            1 => {
                let value = parser.load_u64()? as i64;
                Ok(TupleItem::Int(BigInt::from(value)))
            }
            2 => {
                if parser.load_uint(7)? == 0 {
                    let value = parser.load_bigint(257, true)?;
                    Ok(TupleItem::Int(value))
                } else {
                    parser.load_bit()?;
                    Ok(TupleItem::Nan)
                }
            }
            3 => {
                let cell = parser.load_reference_cloned()?;
                Ok(TupleItem::Cell(cell))
            }
            4 => Ok(TupleItem::Slice(parse_vm_cell_slice(parser)?)),
            5 => {
                let cell = parser.load_reference_cloned()?;
                Ok(TupleItem::Builder(cell))
            }
            7 => {
                let length = parser.load_u16()? as usize;
                let mut items: Vec<TupleItem> = Vec::with_capacity(length);

                if length > 1 {
                    let head_ref = parser.load_reference_cloned()?;
                    let tail_ref = parser.load_reference_cloned()?;

                    let mut tail_parser = tail_ref.as_slice_allow_exotic();
                    items.insert(0, TupleItem::load_from(&mut tail_parser)?);

                    let mut head_refs = vec![head_ref];
                    let mut current_parser = head_refs[0].as_slice_allow_exotic();

                    for _ in 0..length - 2 {
                        let old_head = current_parser.load_reference_cloned()?;
                        let new_tail = current_parser.load_reference_cloned()?;

                        let mut new_tail_parser = new_tail.as_slice_allow_exotic();
                        items.insert(0, TupleItem::load_from(&mut new_tail_parser)?);

                        head_refs.push(old_head);
                        current_parser = head_refs[head_refs.len() - 1].as_slice_allow_exotic();
                    }

                    items.insert(0, TupleItem::load_from(&mut current_parser)?);
                } else if length == 1 {
                    let ref_cell = parser.load_reference_cloned()?;
                    let mut item_parser = ref_cell.as_slice_allow_exotic();
                    items.push(TupleItem::load_from(&mut item_parser)?);
                }

                Ok(TupleItem::Tuple(Tuple(items)))
            }
            6 => {
                // vm_stk_cont#06 cont:VmCont = VmStackValue;
                let cont = ContData::load_from(parser)?;
                Ok(TupleItem::Cont(cont))
            }
            _ => Err(Error::InvalidTag),
        }
    }
}

fn serialize_tuple_tail(
    src: &[TupleItem],
    builder: &mut CellBuilder,
    context: &dyn CellContext,
) -> Result<(), Error> {
    if !src.is_empty() {
        // rest:^(VmStackList n)
        let mut tail_builder = CellBuilder::new();
        serialize_tuple_tail(&src[..src.len() - 1], &mut tail_builder, context)?;
        let tail_cell = tail_builder.build_ext(context)?;
        builder.store_reference(tail_cell)?;

        // tos
        src[src.len() - 1].store_into(builder, context)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::needless_pass_by_value)]
    fn roundtrip_test(item: TupleItem) {
        let mut builder = CellBuilder::new();
        item.store_into(&mut builder, Cell::empty_context())
            .unwrap();
        let cell = builder.build().unwrap();

        let mut parser = cell.as_slice_allow_exotic();
        let deserialized = TupleItem::load_from(&mut parser).unwrap();

        assert_eq!(item, deserialized);
    }

    fn roundtrip_tuple_test(items: Vec<TupleItem>) {
        let serialized = CellBuilder::build_from(Tuple(items.clone())).unwrap();
        let deserialized = Tuple::deserialize(&serialized).unwrap();

        assert_eq!(Tuple(items), deserialized);
    }

    #[test]
    fn test_null_roundtrip() {
        roundtrip_test(TupleItem::Null);
    }

    #[test]
    fn test_small_int_roundtrip() {
        roundtrip_test(TupleItem::Int(BigInt::from(42)));
        roundtrip_test(TupleItem::Int(BigInt::from(-123)));
    }

    #[test]
    fn test_large_int_roundtrip() {
        roundtrip_test(TupleItem::Int(BigInt::from(2u64).pow(100)));
    }

    #[test]
    fn test_nan_roundtrip() {
        roundtrip_test(TupleItem::Nan);
    }

    #[test]
    fn test_cell_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(42, 8).unwrap();
        let cell = builder.build().unwrap();

        roundtrip_test(TupleItem::Cell(cell));
    }

    #[test]
    fn test_slice_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(42, 8).unwrap();
        builder.store_small_uint(43, 8).unwrap();
        let cell = builder.build().unwrap();

        roundtrip_test(TupleItem::Slice(cell));
    }

    #[test]
    fn test_builder_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(42, 8).unwrap();
        let cell = builder.build().unwrap();

        roundtrip_test(TupleItem::Builder(cell));
    }

    #[test]
    fn test_empty_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(Tuple(vec![])));
    }

    #[test]
    fn test_single_item_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(Tuple(vec![TupleItem::Int(BigInt::from(
            42,
        ))])));
    }

    #[test]
    fn test_multiple_item_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(Tuple(vec![
            TupleItem::Int(BigInt::from(42)),
            TupleItem::Null,
            TupleItem::Nan,
        ])));
    }

    #[test]
    fn test_nested_tuple_roundtrip() {
        let nested = TupleItem::Tuple(Tuple(vec![
            TupleItem::Int(BigInt::from(1)),
            TupleItem::Tuple(Tuple(vec![
                TupleItem::Int(BigInt::from(2)),
                TupleItem::Tuple(Tuple(vec![TupleItem::Int(BigInt::from(3))])),
            ])),
        ]));

        roundtrip_test(nested);
    }

    #[test]
    fn test_serialize_deserialize_empty_tuple() {
        roundtrip_tuple_test(vec![]);
    }

    #[test]
    fn test_serialize_deserialize_simple_tuple() {
        roundtrip_tuple_test(vec![
            TupleItem::Int(BigInt::from(42)),
            TupleItem::Null,
            TupleItem::Nan,
        ]);
    }

    #[test]
    fn test_serialize_deserialize_complex_tuple() {
        let mut cell_builder = CellBuilder::new();
        cell_builder.store_small_uint(0xAB, 8).unwrap();
        let test_cell = cell_builder.build().unwrap();

        roundtrip_tuple_test(vec![
            TupleItem::Int(BigInt::from(12345)),
            TupleItem::Cell(test_cell.clone()),
            TupleItem::Tuple(Tuple(vec![
                TupleItem::Slice(test_cell.clone()),
                TupleItem::Int(BigInt::from(-9876)),
            ])),
            TupleItem::Builder(test_cell),
        ]);
    }

    #[test]
    fn test_serialize_deserialize_deeply_nested() {
        let mut nested = TupleItem::Int(BigInt::from(0));

        for i in 1..10 {
            nested = TupleItem::Tuple(Tuple(vec![TupleItem::Int(BigInt::from(i)), nested]));
        }

        roundtrip_tuple_test(vec![nested]);
    }

    #[test]
    fn test_int_boundary_values() {
        // Test i64 boundaries
        roundtrip_test(TupleItem::Int(BigInt::from(i64::MAX)));
        roundtrip_test(TupleItem::Int(BigInt::from(i64::MIN)));

        // Test values just outside i64 range (should use int257 encoding)
        roundtrip_test(TupleItem::Int(BigInt::from(i64::MAX) + BigInt::from(1)));
        roundtrip_test(TupleItem::Int(BigInt::from(i64::MIN) - BigInt::from(1)));
    }
}
