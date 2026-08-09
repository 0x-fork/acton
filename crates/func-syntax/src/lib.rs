//! Parsing and typed syntax trees for the `FunC` language.
//!
//! This crate wraps the `FunC` Tree-sitter grammar with an error-tolerant typed
//! AST. Parsing succeeds even when the source contains syntax errors; inspect
//! [`SourceFile::has_errors`] or [`SourceFile::errors`] to retrieve them.

pub mod ast;

pub use ast::*;
pub use ton_syntax::errors::{ParseError, ParseErrorKind, Span};
pub use ton_syntax::impl_ast_node;
use tree_sitter::{Language, Tree};

/// Parses `FunC` source code into a [`SourceFile`].
///
/// # Errors
///
/// Returns an error when the Tree-sitter parser cannot be initialized.
pub fn parse(code: &str) -> anyhow::Result<SourceFile> {
    parse_with_old_tree(code, None)
}

/// Parses `FunC` source code and optionally reuses an existing syntax tree.
///
/// Callers must apply the corresponding [`tree_sitter::InputEdit`] values to
/// `old_tree` before passing it when the new source changes byte ranges.
///
/// # Errors
///
/// Returns an error when the Tree-sitter parser cannot be initialized.
pub fn parse_with_old_tree(code: &str, old_tree: Option<&Tree>) -> anyhow::Result<SourceFile> {
    let tree = ton_syntax::parser::parse_with_old_tree(
        code,
        old_tree,
        tree_sitter_func::LANGUAGE.into(),
        "FunC",
    )?;
    Ok(SourceFile::new(tree, code.to_owned()))
}

/// Returns the Tree-sitter language used to parse `FunC`.
#[must_use]
pub fn language() -> Language {
    tree_sitter_func::LANGUAGE.into()
}
