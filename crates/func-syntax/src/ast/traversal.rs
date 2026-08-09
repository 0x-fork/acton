use tree_sitter::Node;

pub use ton_syntax::ast::PreorderTraverse;

/// Additional traversal operations for Tree-sitter nodes.
pub trait NodeTraversalExt<'tree> {
    /// Traverses this node and its descendants in preorder.
    fn traverse(&self) -> PreorderTraverse<'tree>;
}

impl<'tree> NodeTraversalExt<'tree> for Node<'tree> {
    fn traverse(&self) -> PreorderTraverse<'tree> {
        PreorderTraverse::new(self.walk())
    }
}

/// Finds the nearest ancestor with the requested Tree-sitter kind.
#[must_use]
pub fn find_parent_by_kind<'tree>(node: &Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut parent = node.parent();
    while let Some(candidate) = parent {
        if candidate.kind() == kind {
            return Some(candidate);
        }
        parent = candidate.parent();
    }
    None
}
