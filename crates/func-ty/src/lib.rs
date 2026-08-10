//! Declaration types and lightweight expression inference for `FunC` editor
//! features.
//!
//! This crate does not validate programs or replace the compiler's type
//! checker.

mod infer;
mod ty;

pub use infer::*;
pub use ty::*;
