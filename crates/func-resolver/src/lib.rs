//! Project indexing and name resolution for `FunC` source code.
//!
//! The resolver is independent of LSP and native filesystem APIs. Editors can
//! build an index from parsed in-memory documents, while native clients can use
//! [`ProjectSourceProvider`] to discover transitive `#include` dependencies.

mod index;
mod model;
mod resolve;

pub use index::*;
pub use model::*;
