use crate::ast::node::{AstChildren, AstFieldChildren, RawNode, field_children};
use crate::ast::{
    AstNode, AstNodeBytesKind, HasName, HasType, Ident, InvalidNodeKindError, NumberLit,
    SliceStringLit, StringLit, TryFromNode, Type, Underscore,
};
use crate::impl_ast_node;
use tree_sitter::Node;

/// Any named `FunC` expression node.
///
/// `FunC`'s grammar keeps operators anonymous. For an operator expression, its
/// containing statement or parenthesized expression therefore exposes the
/// named operands as several [`Expr`] values in source order.
#[derive(Clone, Copy, Debug)]
pub enum Expr<'tree> {
    FunctionApplication(FunctionApplication<'tree>),
    MethodCall(MethodCall<'tree>),
    LocalVarsDeclaration(LocalVarsDeclaration<'tree>),
    Parenthesized(ParenthesizedExpr<'tree>),
    Tensor(TensorExpr<'tree>),
    Tuple(TypedTuple<'tree>),
    Number(NumberLit<'tree>),
    String(StringLit<'tree>),
    SliceString(SliceStringLit<'tree>),
    Ident(Ident<'tree>),
    Underscore(Underscore<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Expr<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::FunctionApplication(node) => node.0,
            Self::MethodCall(node) => node.0,
            Self::LocalVarsDeclaration(node) => node.0,
            Self::Parenthesized(node) => node.0,
            Self::Tensor(node) => node.0,
            Self::Tuple(node) => node.0,
            Self::Number(node) => node.0,
            Self::String(node) => node.0,
            Self::SliceString(node) => node.0,
            Self::Ident(node) => node.0,
            Self::Underscore(node) => node.0,
            Self::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> From<Node<'tree>> for Expr<'tree> {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"function_application" => Self::FunctionApplication(FunctionApplication(node)),
            b"method_call" => Self::MethodCall(MethodCall(node)),
            b"local_vars_declaration" => Self::LocalVarsDeclaration(LocalVarsDeclaration(node)),
            b"parenthesized_expression" => Self::Parenthesized(ParenthesizedExpr(node)),
            b"tensor_expression" => Self::Tensor(TensorExpr(node)),
            b"typed_tuple" => Self::Tuple(TypedTuple(node)),
            b"number_literal" => Self::Number(NumberLit(node)),
            b"string_literal" => Self::String(StringLit(node)),
            b"slice_string_literal" => Self::SliceString(SliceStringLit(node)),
            b"identifier" => Self::Ident(Ident(node)),
            b"underscore" => Self::Underscore(Underscore(node)),
            _ => Self::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for Expr<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let result = Self::from(node);
        if matches!(result, Self::Unmapped(_)) {
            Err(InvalidNodeKindError {
                expected: "FunC expression",
                actual: node.kind().to_owned(),
            })
        } else {
            Ok(result)
        }
    }
}

impl<'tree> AstNode<'tree> for Expr<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// A direct function application.
#[derive(Clone, Copy, Debug)]
pub struct FunctionApplication<'tree>(pub Node<'tree>);
impl_ast_node!(FunctionApplication, "function_application");

impl<'tree> FunctionApplication<'tree> {
    /// Returns the expression being called.
    #[must_use]
    pub fn callee(&self) -> Option<Expr<'tree>> {
        self.0.child_by_field_name("callee").map(Expr::from)
    }

    /// Returns application arguments in source order.
    #[must_use]
    pub fn arguments(&self) -> AstFieldChildren<'tree, Expr<'tree>> {
        field_children(self.0, "arguments")
    }
}

/// A `.` or `~` method-call suffix.
#[derive(Clone, Copy, Debug)]
pub struct MethodCall<'tree>(pub Node<'tree>);
impl_ast_node!(MethodCall, "method_call");

impl<'tree> MethodCall<'tree> {
    /// Returns the called method name.
    #[must_use]
    pub fn method_name(&self) -> Option<Ident<'tree>> {
        self.0.field("method_name")
    }

    /// Returns the argument expression.
    #[must_use]
    pub fn arguments(&self) -> Option<Expr<'tree>> {
        self.0.field("arguments")
    }

    /// Returns `true` for a modifying `~` call and `false` for a `.` call.
    #[must_use]
    pub fn is_modifying(&self, source: &str) -> bool {
        self.0
            .child(0)
            .and_then(|node| node.utf8_text(source.as_bytes()).ok())
            == Some("~")
    }
}

/// A local variable declaration used as the left side of an assignment.
#[derive(Clone, Copy, Debug)]
pub struct LocalVarsDeclaration<'tree>(pub Node<'tree>);
impl_ast_node!(LocalVarsDeclaration, "local_vars_declaration");

impl<'tree> LocalVarsDeclaration<'tree> {
    /// Returns the declaration pattern.
    #[must_use]
    pub fn lhs(&self) -> Option<VarDeclLhs<'tree>> {
        self.0.field("lhs")
    }
}

/// A local declaration pattern.
#[derive(Clone, Copy, Debug)]
pub enum VarDeclLhs<'tree> {
    Tensor(TensorVarsDeclaration<'tree>),
    Tuple(TupleVarsDeclaration<'tree>),
    Var(VarDeclaration<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> VarDeclLhs<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Tensor(node) => node.0,
            Self::Tuple(node) => node.0,
            Self::Var(node) => node.0,
            Self::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> From<Node<'tree>> for VarDeclLhs<'tree> {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"tensor_vars_declaration" => Self::Tensor(TensorVarsDeclaration(node)),
            b"tuple_vars_declaration" => Self::Tuple(TupleVarsDeclaration(node)),
            b"var_declaration" => Self::Var(VarDeclaration(node)),
            _ => Self::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for VarDeclLhs<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let result = Self::from(node);
        if matches!(result, Self::Unmapped(_)) {
            Err(InvalidNodeKindError {
                expected: "FunC variable declaration",
                actual: node.kind().to_owned(),
            })
        } else {
            Ok(result)
        }
    }
}

impl<'tree> AstNode<'tree> for VarDeclLhs<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// A parenthesized variable pattern.
#[derive(Clone, Copy, Debug)]
pub struct TensorVarsDeclaration<'tree>(pub Node<'tree>);
impl_ast_node!(TensorVarsDeclaration, "tensor_vars_declaration");

impl<'tree> TensorVarsDeclaration<'tree> {
    /// Returns nested variable declaration patterns.
    #[must_use]
    pub fn variables(&self) -> AstFieldChildren<'tree, VarDeclLhs<'tree>> {
        field_children(self.0, "vars")
    }
}

/// A square-bracketed variable pattern.
#[derive(Clone, Copy, Debug)]
pub struct TupleVarsDeclaration<'tree>(pub Node<'tree>);
impl_ast_node!(TupleVarsDeclaration, "tuple_vars_declaration");

impl<'tree> TupleVarsDeclaration<'tree> {
    /// Returns nested variable declaration patterns.
    #[must_use]
    pub fn variables(&self) -> AstFieldChildren<'tree, VarDeclLhs<'tree>> {
        field_children(self.0, "vars")
    }
}

/// A single typed local variable declaration.
#[derive(Clone, Copy, Debug)]
pub struct VarDeclaration<'tree>(pub Node<'tree>);
impl_ast_node!(VarDeclaration, "var_declaration");

impl<'tree> HasName<'tree> for VarDeclaration<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Self::Name> {
        self.0.field("name")
    }
}

impl<'tree> HasType<'tree> for VarDeclaration<'tree> {
    fn type_hint(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }
}

/// A parenthesized expression.
#[derive(Clone, Copy, Debug)]
pub struct ParenthesizedExpr<'tree>(pub Node<'tree>);
impl_ast_node!(ParenthesizedExpr, "parenthesized_expression");

impl<'tree> ParenthesizedExpr<'tree> {
    /// Returns the named expression parts in source order.
    #[must_use]
    pub fn expressions(&self) -> AstChildren<'tree, Expr<'tree>> {
        AstChildren::new(self.0)
    }
}

/// A parenthesized tensor expression.
#[derive(Clone, Copy, Debug)]
pub struct TensorExpr<'tree>(pub Node<'tree>);
impl_ast_node!(TensorExpr, "tensor_expression");

impl<'tree> TensorExpr<'tree> {
    /// Returns tensor elements in source order.
    #[must_use]
    pub fn expressions(&self) -> AstFieldChildren<'tree, Expr<'tree>> {
        field_children(self.0, "expressions")
    }
}

/// A square-bracketed typed tuple expression.
#[derive(Clone, Copy, Debug)]
pub struct TypedTuple<'tree>(pub Node<'tree>);
impl_ast_node!(TypedTuple, "typed_tuple");

impl<'tree> TypedTuple<'tree> {
    /// Returns tuple elements in source order.
    #[must_use]
    pub fn expressions(&self) -> AstFieldChildren<'tree, Expr<'tree>> {
        field_children(self.0, "expressions")
    }
}
