use crate::ast::{AstNode, AstNodeBytesKind, InvalidNodeKindError, TryFromNode};
use crate::impl_ast_node;
use tree_sitter::Node;

/// A `FunC` identifier, including backtick-escaped identifiers.
#[derive(Clone, Copy, Debug)]
pub struct Ident<'tree>(pub Node<'tree>);
impl_ast_node!(Ident, "identifier");

/// A type identifier.
#[derive(Clone, Copy, Debug)]
pub struct TypeIdent<'tree>(pub Node<'tree>);
impl_ast_node!(TypeIdent, "type_identifier");

/// An integer or hexadecimal number literal.
#[derive(Clone, Copy, Debug)]
pub struct NumberLit<'tree>(pub Node<'tree>);
impl_ast_node!(NumberLit, "number_literal");

/// A quoted number literal with a `FunC` conversion suffix.
#[derive(Clone, Copy, Debug)]
pub struct NumberStringLit<'tree>(pub Node<'tree>);
impl_ast_node!(NumberStringLit, "number_string_literal");

/// A quoted slice literal with a `FunC` conversion suffix.
#[derive(Clone, Copy, Debug)]
pub struct SliceStringLit<'tree>(pub Node<'tree>);
impl_ast_node!(SliceStringLit, "slice_string_literal");

/// A plain quoted string literal.
#[derive(Clone, Copy, Debug)]
pub struct StringLit<'tree>(pub Node<'tree>);
impl_ast_node!(StringLit, "string_literal");

/// An underscore placeholder.
#[derive(Clone, Copy, Debug)]
pub struct Underscore<'tree>(pub Node<'tree>);
impl_ast_node!(Underscore, "underscore");

/// A source comment accepted by the `FunC` grammar.
#[derive(Clone, Copy, Debug)]
pub struct Comment<'tree>(pub Node<'tree>);
impl_ast_node!(Comment, "comment");

/// A version expression used by `#pragma version`.
#[derive(Clone, Copy, Debug)]
pub struct VersionIdent<'tree>(pub Node<'tree>);
impl_ast_node!(VersionIdent, "version_identifier");

impl<'tree> NumberLit<'tree> {
    /// Returns the string-form number literal nested in this number, if any.
    #[must_use]
    pub fn string_value(&self) -> Option<NumberStringLit<'tree>> {
        self.0.named_child(0).map(NumberStringLit::from)
    }
}

/// A declaration name which may be an identifier or `_`.
#[derive(Clone, Copy, Debug)]
pub enum Name<'tree> {
    /// A named declaration.
    Ident(Ident<'tree>),
    /// An ignored declaration.
    Underscore(Underscore<'tree>),
}

impl<'tree> Name<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Ident(node) => node.0,
            Self::Underscore(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for Name<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"identifier" => Ok(Self::Ident(Ident(node))),
            b"underscore" => Ok(Self::Underscore(Underscore(node))),
            _ => Err(InvalidNodeKindError {
                expected: "identifier or underscore",
                actual: node.kind().to_owned(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for Name<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}
