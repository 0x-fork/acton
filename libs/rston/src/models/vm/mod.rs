//! VM output actions, stack models, and cell encoding for get-method calls.
//!
//! Stack support is part of the `models` feature and provides the cell format
//! for get-method arguments and results.
//! This module operates on cells and integers; it does not execute TVM code.
//! [`Tuple`] represents both a stack and a nested tuple. Stack order runs from
//! bottom at index zero to top at the last index.
//!
//! Slice values contain a materialized cell. Continuations use [`ContData`],
//! which normalizes supported forms and rejects unsupported control-flow forms.
//! Decoding allows unused trailing data. This is not a strict validator for
//! arbitrary TL-B input or a lossless continuation codec.

pub use self::out_actions::*;

mod out_actions;

/// Ordered values, continuation data, and their TL-B cell codec.
pub mod stack;

mod snake_string;

pub use stack::{ContData, Tuple, TupleItem};
