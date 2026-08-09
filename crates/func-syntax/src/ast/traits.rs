use crate::ast::{Block, FunctionBody, Ident, Parameters, Type};

pub use ton_syntax::ast::{
    AstNode, AstNodeBytesKind, HasName, HasTreeSitterKind, InvalidNodeKindError, TryFromNode,
};

/// An AST node that carries a `FunC` type annotation.
pub trait HasType<'tree> {
    /// Returns the annotated type when it is present in the source.
    fn type_hint(&self) -> Option<Type<'tree>>;
}

/// A function-like `FunC` declaration.
pub trait FunctionLike<'tree>: HasName<'tree, Name = Ident<'tree>> + AstNode<'tree> {
    /// Returns the declared return type.
    fn return_type(&self) -> Option<Type<'tree>>;

    /// Returns the declaration's parameter list.
    fn parameters(&self) -> Option<Parameters<'tree>>;

    /// Returns the block or assembly body when the declaration has one.
    fn body(&self) -> Option<FunctionBody<'tree>>;

    /// Returns a block body, excluding assembly declarations.
    fn block_body(&self) -> Option<Block<'tree>> {
        match self.body()? {
            FunctionBody::Block(block) => Some(block),
            FunctionBody::Asm(_) => None,
        }
    }
}
