use crate::ast::node::{AstChildren, AstFieldChildren, RawNode, field_children};
use crate::ast::{
    AstNode, AstNodeBytesKind, Block, Expr, FunctionLike, HasName, HasType, Ident,
    InvalidNodeKindError, Name, NumberLit, StringLit, TryFromNode, Type, TypeIdent, VersionIdent,
};
use crate::impl_ast_node;
use tree_sitter::Node;

/// The root node of a `FunC` source file.
#[derive(Clone, Copy, Debug)]
pub struct Root<'tree>(pub Node<'tree>);
impl_ast_node!(Root, "source_file");

impl<'tree> Root<'tree> {
    /// Returns top-level items in source order.
    #[must_use]
    pub fn items(&self) -> AstChildren<'tree, TopLevel<'tree>> {
        AstChildren::new(self.0)
    }
}

/// Any top-level `FunC` item.
#[derive(Clone, Copy, Debug)]
pub enum TopLevel<'tree> {
    Function(Function<'tree>),
    GlobalVars(GlobalVarDeclarations<'tree>),
    Import(Import<'tree>),
    Pragma(Pragma<'tree>),
    Constants(ConstantDeclarations<'tree>),
    Empty(EmptyStmt<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> TopLevel<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Function(node) => node.0,
            Self::GlobalVars(node) => node.0,
            Self::Import(node) => node.0,
            Self::Pragma(node) => node.0,
            Self::Constants(node) => node.0,
            Self::Empty(node) => node.0,
            Self::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> From<Node<'tree>> for TopLevel<'tree> {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"function_declaration" => Self::Function(Function(node)),
            b"global_var_declarations" => Self::GlobalVars(GlobalVarDeclarations(node)),
            b"import_directive" => Self::Import(Import(node)),
            b"pragma_directive" => Self::Pragma(Pragma(node)),
            b"constant_declarations" => Self::Constants(ConstantDeclarations(node)),
            b"empty_statement" => Self::Empty(EmptyStmt(node)),
            _ => Self::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for TopLevel<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let result = Self::from(node);
        if matches!(result, Self::Unmapped(_)) {
            Err(InvalidNodeKindError {
                expected: "FunC top-level item",
                actual: node.kind().to_owned(),
            })
        } else {
            Ok(result)
        }
    }
}

impl<'tree> AstNode<'tree> for TopLevel<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// A `#include` directive.
#[derive(Clone, Copy, Debug)]
pub struct Import<'tree>(pub Node<'tree>);
impl_ast_node!(Import, "import_directive");

impl<'tree> Import<'tree> {
    /// Returns the quoted include path.
    #[must_use]
    pub fn path(&self) -> Option<StringLit<'tree>> {
        self.0.field("path")
    }
}

/// A `#pragma` directive.
#[derive(Clone, Copy, Debug)]
pub struct Pragma<'tree>(pub Node<'tree>);
impl_ast_node!(Pragma, "pragma_directive");

impl<'tree> Pragma<'tree> {
    /// Returns the anonymous pragma-key node.
    #[must_use]
    pub fn key(&self) -> Option<RawNode<'tree>> {
        self.0.child_by_field_name("key").map(RawNode::new)
    }

    /// Returns the version constraint, if this pragma has one.
    #[must_use]
    pub fn value(&self) -> Option<VersionIdent<'tree>> {
        self.0.field("value")
    }
}

/// A comma-separated group of global variable declarations.
#[derive(Clone, Copy, Debug)]
pub struct GlobalVarDeclarations<'tree>(pub Node<'tree>);
impl_ast_node!(GlobalVarDeclarations, "global_var_declarations");

impl<'tree> GlobalVarDeclarations<'tree> {
    /// Returns declarations in source order.
    #[must_use]
    pub fn declarations(&self) -> AstFieldChildren<'tree, GlobalVar<'tree>> {
        field_children(self.0, "decls")
    }
}

/// A single global variable declaration.
#[derive(Clone, Copy, Debug)]
pub struct GlobalVar<'tree>(pub Node<'tree>);
impl_ast_node!(GlobalVar, "global_var_declaration");

impl<'tree> HasName<'tree> for GlobalVar<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Self::Name> {
        self.0.field("name")
    }
}

impl<'tree> HasType<'tree> for GlobalVar<'tree> {
    fn type_hint(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }
}

/// A comma-separated group of constant declarations.
#[derive(Clone, Copy, Debug)]
pub struct ConstantDeclarations<'tree>(pub Node<'tree>);
impl_ast_node!(ConstantDeclarations, "constant_declarations");

impl<'tree> ConstantDeclarations<'tree> {
    /// Returns declarations in source order.
    #[must_use]
    pub fn declarations(&self) -> AstFieldChildren<'tree, Constant<'tree>> {
        field_children(self.0, "decls")
    }
}

/// A single constant declaration.
#[derive(Clone, Copy, Debug)]
pub struct Constant<'tree>(pub Node<'tree>);
impl_ast_node!(Constant, "constant_declaration");

impl<'tree> Constant<'tree> {
    /// Returns the constant value wrapper.
    #[must_use]
    pub fn value(&self) -> Option<ConstantValue<'tree>> {
        self.0.field("value")
    }
}

impl<'tree> HasName<'tree> for Constant<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Self::Name> {
        self.0.field("name")
    }
}

impl<'tree> HasType<'tree> for Constant<'tree> {
    fn type_hint(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }
}

/// The expression wrapper on the right side of a constant declaration.
#[derive(Clone, Copy, Debug)]
pub struct ConstantValue<'tree>(pub Node<'tree>);
impl_ast_node!(ConstantValue, "constant_declaration_value");

impl<'tree> ConstantValue<'tree> {
    /// Returns named expression parts in source order.
    #[must_use]
    pub fn expressions(&self) -> AstChildren<'tree, Expr<'tree>> {
        AstChildren::new(self.0)
    }
}

/// A `FunC` function declaration or prototype.
#[derive(Clone, Copy, Debug)]
pub struct Function<'tree>(pub Node<'tree>);
impl_ast_node!(Function, "function_declaration");

impl<'tree> Function<'tree> {
    /// Returns generic type parameters.
    #[must_use]
    pub fn type_parameters(&self) -> Option<TypeParameters<'tree>> {
        self.0.field("type_parameters")
    }

    /// Returns function specifiers.
    #[must_use]
    pub fn specifiers(&self) -> Option<Specifiers<'tree>> {
        self.0.field("specifiers")
    }
}

impl<'tree> HasName<'tree> for Function<'tree> {
    type Name = Ident<'tree>;

    fn name(&self) -> Option<Self::Name> {
        self.0.field("name")
    }
}

impl<'tree> FunctionLike<'tree> for Function<'tree> {
    fn return_type(&self) -> Option<Type<'tree>> {
        self.0.field("return_type")
    }

    fn parameters(&self) -> Option<Parameters<'tree>> {
        self.0.field("parameters")
    }

    fn body(&self) -> Option<FunctionBody<'tree>> {
        self.0
            .child_by_field_name("body")
            .or_else(|| self.0.child_by_field_name("asm_body"))
            .and_then(|node| FunctionBody::try_from_node(node).ok())
    }
}

/// A function block or assembly body.
#[derive(Clone, Copy, Debug)]
pub enum FunctionBody<'tree> {
    Block(Block<'tree>),
    Asm(AsmFunctionBody<'tree>),
}

impl<'tree> FunctionBody<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Block(node) => node.0,
            Self::Asm(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for FunctionBody<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"block_statement" => Ok(Self::Block(Block(node))),
            b"asm_function_body" => Ok(Self::Asm(AsmFunctionBody(node))),
            _ => Err(InvalidNodeKindError {
                expected: "FunC function body",
                actual: node.kind().to_owned(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for FunctionBody<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// An assembly function body.
#[derive(Clone, Copy, Debug)]
pub struct AsmFunctionBody<'tree>(pub Node<'tree>);
impl_ast_node!(AsmFunctionBody, "asm_function_body");

impl<'tree> AsmFunctionBody<'tree> {
    /// Returns stack-parameter identifiers in source order.
    #[must_use]
    pub fn identifiers(&self) -> AstChildren<'tree, Ident<'tree>> {
        AstChildren::new(self.0)
    }

    /// Returns stack-result indices in source order.
    #[must_use]
    pub fn result_indices(&self) -> AstChildren<'tree, NumberLit<'tree>> {
        AstChildren::new(self.0)
    }

    /// Returns assembly instruction strings in source order.
    #[must_use]
    pub fn instructions(&self) -> AstChildren<'tree, StringLit<'tree>> {
        AstChildren::new(self.0)
    }
}

/// A regular or relaxed function parameter list.
#[derive(Clone, Copy, Debug)]
pub enum Parameters<'tree> {
    Regular(ParameterList<'tree>),
    Relaxed(RelaxedParameterList<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Parameters<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Regular(node) => node.0,
            Self::Relaxed(node) => node.0,
            Self::Unmapped(node) => node.0,
        }
    }

    /// Returns typed parameters in source order.
    #[must_use]
    pub fn declarations(&self) -> AstChildren<'tree, Parameter<'tree>> {
        AstChildren::new(self.syntax())
    }
}

impl<'tree> From<Node<'tree>> for Parameters<'tree> {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"parameter_list" => Self::Regular(ParameterList(node)),
            b"parameter_list_relaxed" => Self::Relaxed(RelaxedParameterList(node)),
            _ => Self::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for Parameters<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let result = Self::from(node);
        if matches!(result, Self::Unmapped(_)) {
            Err(InvalidNodeKindError {
                expected: "FunC parameter list",
                actual: node.kind().to_owned(),
            })
        } else {
            Ok(result)
        }
    }
}

impl<'tree> AstNode<'tree> for Parameters<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// A typed parameter list.
#[derive(Clone, Copy, Debug)]
pub struct ParameterList<'tree>(pub Node<'tree>);
impl_ast_node!(ParameterList, "parameter_list");

impl<'tree> ParameterList<'tree> {
    /// Returns parameters in source order.
    #[must_use]
    pub fn declarations(&self) -> AstChildren<'tree, Parameter<'tree>> {
        AstChildren::new(self.0)
    }
}

/// A prototype parameter list which may contain untyped names.
#[derive(Clone, Copy, Debug)]
pub struct RelaxedParameterList<'tree>(pub Node<'tree>);
impl_ast_node!(RelaxedParameterList, "parameter_list_relaxed");

impl<'tree> RelaxedParameterList<'tree> {
    /// Returns typed parameters in source order.
    #[must_use]
    pub fn declarations(&self) -> AstChildren<'tree, Parameter<'tree>> {
        AstChildren::new(self.0)
    }

    /// Returns untyped names in source order.
    #[must_use]
    pub fn names(&self) -> AstFieldChildren<'tree, Name<'tree>> {
        field_children(self.0, "name")
    }
}

/// A typed function parameter.
#[derive(Clone, Copy, Debug)]
pub struct Parameter<'tree>(pub Node<'tree>);
impl_ast_node!(Parameter, "parameter_declaration");

impl<'tree> Parameter<'tree> {
    /// Returns the parameter name or underscore.
    #[must_use]
    pub fn name(&self) -> Option<Name<'tree>> {
        self.0
            .child_by_field_name("name")
            .and_then(|node| Name::try_from_node(node).ok())
    }
}

impl<'tree> HasType<'tree> for Parameter<'tree> {
    fn type_hint(&self) -> Option<Type<'tree>> {
        self.0.field("type")
    }
}

/// A `forall ... ->` type-parameter list.
#[derive(Clone, Copy, Debug)]
pub struct TypeParameters<'tree>(pub Node<'tree>);
impl_ast_node!(TypeParameters, "type_parameters");

impl<'tree> TypeParameters<'tree> {
    /// Returns type parameters in source order.
    #[must_use]
    pub fn declarations(&self) -> AstChildren<'tree, TypeParameter<'tree>> {
        AstChildren::new(self.0)
    }
}

/// A generic type parameter.
#[derive(Clone, Copy, Debug)]
pub struct TypeParameter<'tree>(pub Node<'tree>);
impl_ast_node!(TypeParameter, "type_parameter");

impl<'tree> HasName<'tree> for TypeParameter<'tree> {
    type Name = TypeIdent<'tree>;

    fn name(&self) -> Option<Self::Name> {
        self.0.field("name")
    }
}

/// A group of function specifiers.
#[derive(Clone, Copy, Debug)]
pub struct Specifiers<'tree>(pub Node<'tree>);
impl_ast_node!(Specifiers, "specifiers_list");

impl<'tree> Specifiers<'tree> {
    /// Returns specifiers in source order.
    #[must_use]
    pub fn items(&self) -> AstChildren<'tree, Specifier<'tree>> {
        AstChildren::new(self.0)
    }
}

/// A function declaration specifier.
#[derive(Clone, Copy, Debug)]
pub enum Specifier<'tree> {
    Impure(ImpureSpecifier<'tree>),
    Inline(InlineSpecifier<'tree>),
    MethodId(MethodIdSpecifier<'tree>),
    Unmapped(RawNode<'tree>),
}

impl<'tree> Specifier<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Impure(node) => node.0,
            Self::Inline(node) => node.0,
            Self::MethodId(node) => node.0,
            Self::Unmapped(node) => node.0,
        }
    }
}

impl<'tree> From<Node<'tree>> for Specifier<'tree> {
    fn from(node: Node<'tree>) -> Self {
        match node.kind_bytes() {
            b"impure" => Self::Impure(ImpureSpecifier(node)),
            b"inline" => Self::Inline(InlineSpecifier(node)),
            b"method_id" => Self::MethodId(MethodIdSpecifier(node)),
            _ => Self::Unmapped(RawNode::new(node)),
        }
    }
}

impl<'tree> TryFromNode<'tree> for Specifier<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        let result = Self::from(node);
        if matches!(result, Self::Unmapped(_)) {
            Err(InvalidNodeKindError {
                expected: "FunC function specifier",
                actual: node.kind().to_owned(),
            })
        } else {
            Ok(result)
        }
    }
}

impl<'tree> AstNode<'tree> for Specifier<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// The `impure` function specifier.
#[derive(Clone, Copy, Debug)]
pub struct ImpureSpecifier<'tree>(pub Node<'tree>);
impl_ast_node!(ImpureSpecifier, "impure");

/// The `inline` or `inline_ref` function specifier.
#[derive(Clone, Copy, Debug)]
pub struct InlineSpecifier<'tree>(pub Node<'tree>);
impl_ast_node!(InlineSpecifier, "inline");

/// A `method_id` function specifier.
#[derive(Clone, Copy, Debug)]
pub struct MethodIdSpecifier<'tree>(pub Node<'tree>);
impl_ast_node!(MethodIdSpecifier, "method_id");

impl<'tree> MethodIdSpecifier<'tree> {
    /// Returns the explicit method identifier value.
    #[must_use]
    pub fn value(&self) -> Option<MethodIdValue<'tree>> {
        self.0
            .child_by_field_name("value")
            .and_then(|node| MethodIdValue::try_from_node(node).ok())
    }
}

/// An explicit numeric or string method identifier.
#[derive(Clone, Copy, Debug)]
pub enum MethodIdValue<'tree> {
    Number(NumberLit<'tree>),
    String(StringLit<'tree>),
}

impl<'tree> MethodIdValue<'tree> {
    /// Returns the underlying Tree-sitter node.
    #[must_use]
    pub const fn syntax(&self) -> Node<'tree> {
        match self {
            Self::Number(node) => node.0,
            Self::String(node) => node.0,
        }
    }
}

impl<'tree> TryFromNode<'tree> for MethodIdValue<'tree> {
    type Error = InvalidNodeKindError;

    fn try_from_node(node: Node<'tree>) -> Result<Self, Self::Error> {
        match node.kind_bytes() {
            b"number_literal" => Ok(Self::Number(NumberLit(node))),
            b"string_literal" => Ok(Self::String(StringLit(node))),
            _ => Err(InvalidNodeKindError {
                expected: "FunC method identifier",
                actual: node.kind().to_owned(),
            }),
        }
    }
}

impl<'tree> AstNode<'tree> for MethodIdValue<'tree> {
    fn syntax(&self) -> Node<'tree> {
        self.syntax()
    }
}

/// An empty top-level semicolon.
#[derive(Clone, Copy, Debug)]
pub struct EmptyStmt<'tree>(pub Node<'tree>);
impl_ast_node!(EmptyStmt, "empty_statement");
