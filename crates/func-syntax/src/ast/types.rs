use crate::ast::{
    AstChildren, AstNode, AstNodeBytesKind, InvalidNodeKindError, RawNode, TryFromNode, TypeIdent,
};
use crate::impl_ast_node;
use tree_sitter::Node;

/// Any named `FunC` type node.
#[derive(Clone, Copy, Debug)]
pub enum Type<'tree> {
    Function(FunctionType<'tree>),
    Primitive(PrimitiveType<'tree>),
    Var(VarType<'tree>),
    Hole(HoleType<'tree>),
    Ident(TypeIdent<'tree>),
    Tensor(TensorType<'tree>),
    Tuple(TupleType<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Type<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Function(node) => node.0,
            Self::Primitive(node) => node.0,
            Self::Var(node) => node.0,
            Self::Hole(node) => node.0,
            Self::Ident(node) => node.0,
            Self::Tensor(node) => node.0,
            Self::Tuple(node) => node.0,
            Self::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> From<Node<'tree>> for Type<'tree> {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"function_type" => Self::Function(FunctionType(node)),
            b"primitive_type" => Self::Primitive(PrimitiveType(node)),
            b"var_type" => Self::Var(VarType(node)),
            b"hole_type" => Self::Hole(HoleType(node)),
            b"type_identifier" => Self::Ident(TypeIdent(node)),
            b"tensor_type" => Self::Tensor(TensorType(node)),
            b"tuple_type" => Self::Tuple(TupleType(node)),
            _ => Self::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for Type<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let result = Self::from(node);
        if matches!(result, Self::Unmapped(_)) {
            Err(InvalidNodeKindError {
                expected: "FunC type",
                actual: node.kind().to_owned(),
            })
        } else {
            Ok(result)
        }
    }
}

impl<'tree> AstNode<'tree> for Type<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// A right-associative function type such as `int -> int -> int`.
#[derive(Clone, Copy, Debug)]
pub struct FunctionType<'tree>(pub Node<'tree>);
impl_ast_node!(FunctionType, "function_type");

/// A built-in `FunC` type such as `int`, `cell`, or `slice`.
#[derive(Clone, Copy, Debug)]
pub struct PrimitiveType<'tree>(pub Node<'tree>);
impl_ast_node!(PrimitiveType, "primitive_type");

/// The inferred `var` type.
#[derive(Clone, Copy, Debug)]
pub struct VarType<'tree>(pub Node<'tree>);
impl_ast_node!(VarType, "var_type");

/// An underscore type placeholder.
#[derive(Clone, Copy, Debug)]
pub struct HoleType<'tree>(pub Node<'tree>);
impl_ast_node!(HoleType, "hole_type");

/// A tensor type enclosed in parentheses.
#[derive(Clone, Copy, Debug)]
pub struct TensorType<'tree>(pub Node<'tree>);
impl_ast_node!(TensorType, "tensor_type");

/// A tuple type enclosed in square brackets.
#[derive(Clone, Copy, Debug)]
pub struct TupleType<'tree>(pub Node<'tree>);
impl_ast_node!(TupleType, "tuple_type");

impl<'tree> FunctionType<'tree> {
    /// Returns the input and output types in source order.
    #[must_use]
    pub fn types(&self) -> AstChildren<'tree, Type<'tree>> {
        AstChildren::new(self.0)
    }

    /// Returns the function input type.
    #[must_use]
    pub fn input(&self) -> Option<Type<'tree>> {
        self.types().next()
    }

    /// Returns the function output type.
    #[must_use]
    pub fn output(&self) -> Option<Type<'tree>> {
        self.types().nth(1)
    }
}

impl<'tree> TensorType<'tree> {
    /// Returns the tensor element types.
    #[must_use]
    pub fn types(&self) -> AstChildren<'tree, Type<'tree>> {
        AstChildren::new(self.0)
    }
}

impl<'tree> TupleType<'tree> {
    /// Returns the tuple element types.
    #[must_use]
    pub fn types(&self) -> AstChildren<'tree, Type<'tree>> {
        AstChildren::new(self.0)
    }
}
