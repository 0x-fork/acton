use crate::ast::node::{AstChildren, AstFieldChildren, RawNode, field_children};
use crate::ast::{AstNode, AstNodeBytesKind, EmptyStmt, Expr, InvalidNodeKindError, TryFromNode};
use crate::impl_ast_node;
use tree_sitter::Node;

/// Any named `FunC` statement node.
#[derive(Clone, Copy, Debug)]
pub enum Stmt<'tree> {
    Block(Block<'tree>),
    Return(ReturnStmt<'tree>),
    Expr(ExprStmt<'tree>),
    Empty(EmptyStmt<'tree>),
    Repeat(RepeatStmt<'tree>),
    If(IfStmt<'tree>),
    DoWhile(DoWhileStmt<'tree>),
    While(WhileStmt<'tree>),
    TryCatch(TryCatchStmt<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Stmt<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Block(node) => node.0,
            Self::Return(node) => node.0,
            Self::Expr(node) => node.0,
            Self::Empty(node) => node.0,
            Self::Repeat(node) => node.0,
            Self::If(node) => node.0,
            Self::DoWhile(node) => node.0,
            Self::While(node) => node.0,
            Self::TryCatch(node) => node.0,
            Self::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> From<Node<'tree>> for Stmt<'tree> {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"block_statement" => Self::Block(Block(node)),
            b"return_statement" => Self::Return(ReturnStmt(node)),
            b"expression_statement" => Self::Expr(ExprStmt(node)),
            b"empty_statement" => Self::Empty(EmptyStmt(node)),
            b"repeat_statement" => Self::Repeat(RepeatStmt(node)),
            b"if_statement" => Self::If(IfStmt(node)),
            b"do_while_statement" => Self::DoWhile(DoWhileStmt(node)),
            b"while_statement" => Self::While(WhileStmt(node)),
            b"try_catch_statement" => Self::TryCatch(TryCatchStmt(node)),
            _ => Self::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for Stmt<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let result = Self::from(node);
        if matches!(result, Self::Unmapped(_)) {
            Err(InvalidNodeKindError {
                expected: "FunC statement",
                actual: node.kind().to_owned(),
            })
        } else {
            Ok(result)
        }
    }
}

impl<'tree> AstNode<'tree> for Stmt<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// A braced sequence of statements.
#[derive(Clone, Copy, Debug)]
pub struct Block<'tree>(pub Node<'tree>);
impl_ast_node!(Block, "block_statement");

impl<'tree> Block<'tree> {
    /// Returns statements in source order.
    #[must_use]
    pub fn statements(&self) -> AstChildren<'tree, Stmt<'tree>> {
        AstChildren::new(self.0)
    }
}

/// A `return` statement.
#[derive(Clone, Copy, Debug)]
pub struct ReturnStmt<'tree>(pub Node<'tree>);
impl_ast_node!(ReturnStmt, "return_statement");

impl<'tree> ReturnStmt<'tree> {
    /// Returns the named parts of the returned expression.
    #[must_use]
    pub fn expressions(&self) -> AstChildren<'tree, Expr<'tree>> {
        AstChildren::new(self.0)
    }
}

/// An expression statement.
#[derive(Clone, Copy, Debug)]
pub struct ExprStmt<'tree>(pub Node<'tree>);
impl_ast_node!(ExprStmt, "expression_statement");

impl<'tree> ExprStmt<'tree> {
    /// Returns the named expression parts in source order.
    #[must_use]
    pub fn expressions(&self) -> AstChildren<'tree, Expr<'tree>> {
        AstChildren::new(self.0)
    }
}

/// A `repeat` loop.
#[derive(Clone, Copy, Debug)]
pub struct RepeatStmt<'tree>(pub Node<'tree>);
impl_ast_node!(RepeatStmt, "repeat_statement");

impl<'tree> RepeatStmt<'tree> {
    /// Returns the named parts of the repeat count expression.
    #[must_use]
    pub fn count(&self) -> AstFieldChildren<'tree, Expr<'tree>> {
        field_children(self.0, "count")
    }

    /// Returns the loop body.
    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }
}

/// An `if`, `ifnot`, `elseif`, or `elseifnot` chain.
#[derive(Clone, Copy, Debug)]
pub struct IfStmt<'tree>(pub Node<'tree>);
impl_ast_node!(IfStmt, "if_statement");

impl<'tree> IfStmt<'tree> {
    /// Returns all named condition parts in source order.
    ///
    /// The grammar flattens `elseif` branches into the same node, so this may
    /// include conditions from several branches.
    #[must_use]
    pub fn conditions(&self) -> AstFieldChildren<'tree, Expr<'tree>> {
        field_children(self.0, "condition")
    }

    /// Returns the consequent blocks for all branches in source order.
    #[must_use]
    pub fn consequents(&self) -> AstFieldChildren<'tree, Block<'tree>> {
        field_children(self.0, "consequent")
    }

    /// Returns the final `else` block when present.
    #[must_use]
    pub fn alternative(&self) -> Option<Block<'tree>> {
        field_children(self.0, "alternative").last()
    }
}

/// A `do ... until` loop.
#[derive(Clone, Copy, Debug)]
pub struct DoWhileStmt<'tree>(pub Node<'tree>);
impl_ast_node!(DoWhileStmt, "do_while_statement");

impl<'tree> DoWhileStmt<'tree> {
    /// Returns the loop body.
    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }

    /// Returns the named parts of the postcondition.
    #[must_use]
    pub fn postcondition(&self) -> AstFieldChildren<'tree, Expr<'tree>> {
        field_children(self.0, "postcondition")
    }
}

/// A `while` loop.
#[derive(Clone, Copy, Debug)]
pub struct WhileStmt<'tree>(pub Node<'tree>);
impl_ast_node!(WhileStmt, "while_statement");

impl<'tree> WhileStmt<'tree> {
    /// Returns the named parts of the precondition.
    #[must_use]
    pub fn precondition(&self) -> AstFieldChildren<'tree, Expr<'tree>> {
        field_children(self.0, "precondition")
    }

    /// Returns the loop body.
    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }
}

/// A `try ... catch` statement.
#[derive(Clone, Copy, Debug)]
pub struct TryCatchStmt<'tree>(pub Node<'tree>);
impl_ast_node!(TryCatchStmt, "try_catch_statement");

impl<'tree> TryCatchStmt<'tree> {
    /// Returns the protected block.
    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("body")
    }

    /// Returns the catch clause.
    #[must_use]
    pub fn catch_clause(&self) -> Option<CatchClause<'tree>> {
        AstChildren::new(self.0).first()
    }
}

/// A catch expression and body.
#[derive(Clone, Copy, Debug)]
pub struct CatchClause<'tree>(pub Node<'tree>);
impl_ast_node!(CatchClause, "catch_clause");

impl<'tree> CatchClause<'tree> {
    /// Returns named catch-expression parts.
    #[must_use]
    pub fn expression(&self) -> AstFieldChildren<'tree, Expr<'tree>> {
        field_children(self.0, "catch_expr")
    }

    /// Returns the catch body.
    #[must_use]
    pub fn body(&self) -> Option<Block<'tree>> {
        self.0.field("catch_body")
    }
}
