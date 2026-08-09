use expect_test::expect;
use func_syntax::{
    AsmFunctionBody, AstNode, Block, CatchClause, Comment, Constant, ConstantDeclarations,
    ConstantValue, DoWhileStmt, EmptyStmt, ExprStmt, Function, FunctionApplication, FunctionBody,
    FunctionType, GlobalVar, GlobalVarDeclarations, HoleType, Ident, IfStmt, Import,
    ImpureSpecifier, InlineSpecifier, LocalVarsDeclaration, MethodCall, MethodIdSpecifier,
    MethodIdValue, Name, NumberLit, NumberStringLit, Parameter, ParameterList, ParenthesizedExpr,
    Pragma, PrimitiveType, RelaxedParameterList, RepeatStmt, ReturnStmt, Root, SliceStringLit,
    Specifiers, StringLit, TensorExpr, TensorType, TensorVarsDeclaration, TryCatchStmt, TupleType,
    TupleVarsDeclaration, TypeIdent, TypeParameter, TypeParameters, TypedTuple, Underscore,
    VarDeclaration, VarType, VersionIdent, WhileStmt, parse,
};
use func_syntax::{HasTreeSitterKind, NodeTraversalExt, TryFromNode};
use tree_sitter::Node;

const SOURCE: &str = include_str!("fixtures/representative.fc");

#[test]
fn every_named_grammar_node_has_a_typed_wrapper() {
    let file = parse(SOURCE).expect("source should parse");
    assert!(!file.has_errors(), "{:?}", file.errors());

    for node in file.root().syntax().traverse().filter(Node::is_named) {
        assert_typed(node);
    }
}

#[test]
fn semantic_enums_reject_unrelated_nodes() {
    let file = parse("int main() { return 0; }").expect("source should parse");
    let root = file.root().syntax();
    let actual = [
        format!("{:?}", Name::try_from_node(root).unwrap_err()),
        format!("{:?}", FunctionBody::try_from_node(root).unwrap_err()),
        format!("{:?}", MethodIdValue::try_from_node(root).unwrap_err()),
    ];

    expect![[r#"
        [
            "InvalidNodeKindError { expected: \"identifier or underscore\", actual: \"source_file\" }",
            "InvalidNodeKindError { expected: \"FunC function body\", actual: \"source_file\" }",
            "InvalidNodeKindError { expected: \"FunC method identifier\", actual: \"source_file\" }",
        ]
    "#]]
    .assert_debug_eq(&actual);
}

fn assert_typed(node: Node<'_>) {
    match node.kind() {
        Root::TREE_SITTER_KIND => cast::<Root>(node),
        AsmFunctionBody::TREE_SITTER_KIND => cast::<AsmFunctionBody>(node),
        Block::TREE_SITTER_KIND => cast::<Block>(node),
        CatchClause::TREE_SITTER_KIND => cast::<CatchClause>(node),
        Constant::TREE_SITTER_KIND => cast::<Constant>(node),
        ConstantValue::TREE_SITTER_KIND => cast::<ConstantValue>(node),
        ConstantDeclarations::TREE_SITTER_KIND => cast::<ConstantDeclarations>(node),
        DoWhileStmt::TREE_SITTER_KIND => cast::<DoWhileStmt>(node),
        EmptyStmt::TREE_SITTER_KIND => cast::<EmptyStmt>(node),
        ExprStmt::TREE_SITTER_KIND => cast::<ExprStmt>(node),
        FunctionApplication::TREE_SITTER_KIND => cast::<FunctionApplication>(node),
        Function::TREE_SITTER_KIND => cast::<Function>(node),
        FunctionType::TREE_SITTER_KIND => cast::<FunctionType>(node),
        GlobalVar::TREE_SITTER_KIND => cast::<GlobalVar>(node),
        GlobalVarDeclarations::TREE_SITTER_KIND => cast::<GlobalVarDeclarations>(node),
        HoleType::TREE_SITTER_KIND => cast::<HoleType>(node),
        IfStmt::TREE_SITTER_KIND => cast::<IfStmt>(node),
        Import::TREE_SITTER_KIND => cast::<Import>(node),
        InlineSpecifier::TREE_SITTER_KIND => cast::<InlineSpecifier>(node),
        LocalVarsDeclaration::TREE_SITTER_KIND => cast::<LocalVarsDeclaration>(node),
        MethodCall::TREE_SITTER_KIND => cast::<MethodCall>(node),
        MethodIdSpecifier::TREE_SITTER_KIND => cast::<MethodIdSpecifier>(node),
        NumberLit::TREE_SITTER_KIND => cast::<NumberLit>(node),
        Parameter::TREE_SITTER_KIND => cast::<Parameter>(node),
        ParameterList::TREE_SITTER_KIND => cast::<ParameterList>(node),
        RelaxedParameterList::TREE_SITTER_KIND => cast::<RelaxedParameterList>(node),
        ParenthesizedExpr::TREE_SITTER_KIND => cast::<ParenthesizedExpr>(node),
        Pragma::TREE_SITTER_KIND => cast::<Pragma>(node),
        PrimitiveType::TREE_SITTER_KIND => cast::<PrimitiveType>(node),
        RepeatStmt::TREE_SITTER_KIND => cast::<RepeatStmt>(node),
        ReturnStmt::TREE_SITTER_KIND => cast::<ReturnStmt>(node),
        Specifiers::TREE_SITTER_KIND => cast::<Specifiers>(node),
        TensorExpr::TREE_SITTER_KIND => cast::<TensorExpr>(node),
        TensorType::TREE_SITTER_KIND => cast::<TensorType>(node),
        TensorVarsDeclaration::TREE_SITTER_KIND => cast::<TensorVarsDeclaration>(node),
        TryCatchStmt::TREE_SITTER_KIND => cast::<TryCatchStmt>(node),
        TupleType::TREE_SITTER_KIND => cast::<TupleType>(node),
        TupleVarsDeclaration::TREE_SITTER_KIND => cast::<TupleVarsDeclaration>(node),
        TypeIdent::TREE_SITTER_KIND => cast::<TypeIdent>(node),
        TypeParameter::TREE_SITTER_KIND => cast::<TypeParameter>(node),
        TypeParameters::TREE_SITTER_KIND => cast::<TypeParameters>(node),
        TypedTuple::TREE_SITTER_KIND => cast::<TypedTuple>(node),
        VarDeclaration::TREE_SITTER_KIND => cast::<VarDeclaration>(node),
        WhileStmt::TREE_SITTER_KIND => cast::<WhileStmt>(node),
        Comment::TREE_SITTER_KIND => cast::<Comment>(node),
        Ident::TREE_SITTER_KIND => cast::<Ident>(node),
        ImpureSpecifier::TREE_SITTER_KIND => cast::<ImpureSpecifier>(node),
        NumberStringLit::TREE_SITTER_KIND => cast::<NumberStringLit>(node),
        SliceStringLit::TREE_SITTER_KIND => cast::<SliceStringLit>(node),
        StringLit::TREE_SITTER_KIND => cast::<StringLit>(node),
        Underscore::TREE_SITTER_KIND => cast::<Underscore>(node),
        VarType::TREE_SITTER_KIND => cast::<VarType>(node),
        VersionIdent::TREE_SITTER_KIND => cast::<VersionIdent>(node),
        kind => panic!("missing typed wrapper for {kind}"),
    }
}

fn cast<'tree, N>(node: Node<'tree>)
where
    N: TryFromNode<'tree>,
    N::Error: std::fmt::Debug,
{
    N::try_from_node(node).expect("node should match its typed wrapper");
}
