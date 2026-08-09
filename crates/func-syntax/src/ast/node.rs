use crate::ast::{
    AstNode, Comment, ConstantDeclarations, Function, GlobalVarDeclarations, Import,
    NodeTraversalExt, Pragma, Root, TopLevel, TryFromNode,
};
use crate::{ParseError, language};
use std::sync::Arc;
use tree_sitter::{Node, Tree};

pub use ton_syntax::ast::{AstChildren, RawNode, SyntaxNodeChildren};
use ton_syntax::errors::collect_errors;

/// An owning parsed `FunC` source file.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// The Tree-sitter syntax tree.
    pub tree: Tree,
    /// The source text used to construct the tree.
    pub source: Arc<str>,
}

ton_syntax::impl_source_file_basics!(SourceFile, ParseError, collect_errors, language);

impl SourceFile {
    /// Returns the typed root node.
    #[must_use]
    pub fn root(&self) -> Root<'_> {
        Root::try_from_node(self.tree.root_node())
            .expect("the FunC grammar always produces a source_file root")
    }

    /// Returns all top-level items in source order.
    #[must_use]
    pub fn top_levels(&self) -> AstChildren<'_, TopLevel<'_>> {
        self.root().items()
    }

    /// Returns all include directives in source order.
    pub fn imports(&self) -> impl Iterator<Item = Import<'_>> {
        self.top_levels().filter_map(|item| match item {
            TopLevel::Import(import) => Some(import),
            _ => None,
        })
    }

    /// Returns all function declarations in source order.
    pub fn functions(&self) -> impl Iterator<Item = Function<'_>> {
        self.top_levels().filter_map(|item| match item {
            TopLevel::Function(function) => Some(function),
            _ => None,
        })
    }

    /// Returns all pragma directives in source order.
    pub fn pragmas(&self) -> impl Iterator<Item = Pragma<'_>> {
        self.top_levels().filter_map(|item| match item {
            TopLevel::Pragma(pragma) => Some(pragma),
            _ => None,
        })
    }

    /// Returns all groups of global variable declarations.
    pub fn global_var_declarations(&self) -> impl Iterator<Item = GlobalVarDeclarations<'_>> {
        self.top_levels().filter_map(|item| match item {
            TopLevel::GlobalVars(declarations) => Some(declarations),
            _ => None,
        })
    }

    /// Returns all groups of constant declarations.
    pub fn constant_declarations(&self) -> impl Iterator<Item = ConstantDeclarations<'_>> {
        self.top_levels().filter_map(|item| match item {
            TopLevel::Constants(declarations) => Some(declarations),
            _ => None,
        })
    }

    /// Returns all comments in source order, including comments inside declarations.
    pub fn comments(&self) -> impl Iterator<Item = Comment<'_>> {
        self.root()
            .syntax()
            .traverse()
            .filter_map(|node| Comment::try_from_node(node).ok())
    }

    /// Finds the top-level item covering the byte range.
    #[must_use]
    pub fn find_top_level_at(&self, start: usize, end: usize) -> Option<TopLevel<'_>> {
        if start > end {
            return None;
        }

        self.top_levels().find(|item| {
            let node = item.syntax();
            node.start_byte() <= start && start < node.end_byte() && end <= node.end_byte()
        })
    }
}

/// Owned iterator over typed children carrying a particular Tree-sitter field.
pub type AstFieldChildren<'tree, N> = std::vec::IntoIter<N>;

/// Returns all children assigned to `field_name`, converted to an AST type.
///
/// Tree-sitter requires a temporary cursor for field iteration. This helper
/// collects the typically small result so callers receive an owning iterator.
pub(crate) fn field_children<'tree, N>(
    node: Node<'tree>,
    field_name: &str,
) -> AstFieldChildren<'tree, N>
where
    N: TryFromNode<'tree>,
{
    let mut cursor = node.walk();
    node.children_by_field_name(field_name, &mut cursor)
        .filter_map(|child| N::try_from_node(child).ok())
        .collect::<Vec<_>>()
        .into_iter()
}
