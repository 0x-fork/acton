use expect_test::expect;
use func_syntax::{AstNode, HasName, NodeTraversalExt, ParseError, parse, parse_with_old_tree};
use serde_json::Value;
use std::collections::BTreeSet;
use tree_sitter::{InputEdit, Point};

const REPRESENTATIVE_SOURCE: &str = include_str!("fixtures/representative.fc");

#[test]
fn parses_representative_func_source() {
    let file = parse(REPRESENTATIVE_SOURCE).expect("source should parse");

    assert!(!file.has_errors(), "{:?}", file.errors());
    assert_eq!(file.imports().count(), 1);
    assert_eq!(file.functions().count(), 7);
    assert_eq!(file.pragmas().count(), 2);
    assert_eq!(file.global_var_declarations().count(), 1);
    assert_eq!(file.constant_declarations().count(), 1);
}

#[test]
fn reports_recoverable_syntax_errors() {
    let file = parse("int broken( { return 1;").expect("parser should recover");
    let actual = format_errors(&file.errors());

    expect![[r"
        Missing at 0:11-0:11: syntax error: missing `)`. Valid here: end, identifier, #include, ;, #pragma, version, not-version, allow-post-modification, compute-asm-ltr, global, ,, const, =, impure, inline, inline_ref.
        Missing at 0:23-0:23: syntax error: missing `}`. Valid here: end, identifier, #include, ;, #pragma, version, not-version, allow-post-modification, compute-asm-ltr, global, ,, const, =, impure, inline, inline_ref."]]
    .assert_eq(&actual);
}

#[test]
fn reuses_an_existing_tree() {
    let first_source = "int answer() { return 41; }";
    let second_source = "int long_answer() { return 41; }";
    let first = parse(first_source).expect("initial source should parse");
    let mut edited_tree = first.tree;
    edited_tree.edit(&InputEdit {
        start_byte: 4,
        old_end_byte: 4,
        new_end_byte: 9,
        start_position: Point::new(0, 4),
        old_end_position: Point::new(0, 4),
        new_end_position: Point::new(0, 9),
    });

    let second = parse_with_old_tree(second_source, Some(&edited_tree))
        .expect("updated source should parse");

    assert!(!second.has_errors());
    expect![[r#"
        [
            "long_answer",
        ]
    "#]]
    .assert_debug_eq(
        &second
            .functions()
            .filter_map(|function| function.name())
            .map(|name| name.text(second_source))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn reports_unicode_syntax_errors_without_panicking() {
    let source = format!("int broken() {{ {} }}", "ж".repeat(40));
    let file = parse(&source).expect("parser should recover");

    expect![[r"
        [
            Unexpected,
            Unexpected,
        ]
    "]]
    .assert_debug_eq(
        &file
            .errors()
            .iter()
            .map(|error| &error.kind)
            .collect::<Vec<_>>(),
    );
}

#[test]
fn representative_source_exercises_the_named_grammar_nodes() {
    let file = parse(REPRESENTATIVE_SOURCE).expect("source should parse");
    assert!(!file.has_errors(), "{:?}", file.errors());

    let seen = file
        .root()
        .syntax()
        .traverse()
        .filter(tree_sitter::Node::is_named)
        .map(|node| node.kind().to_owned())
        .collect::<BTreeSet<_>>();
    let declared = serde_json::from_str::<Vec<Value>>(tree_sitter_func::NODE_TYPES)
        .expect("node-types.json should be valid")
        .into_iter()
        .filter(|node| node["named"] == true)
        .filter_map(|node| node["type"].as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let missing = declared.difference(&seen).cloned().collect::<Vec<_>>();

    expect![[r"
        []
    "]]
    .assert_debug_eq(&missing);
}

fn format_errors(errors: &[ParseError]) -> String {
    errors
        .iter()
        .map(|error| {
            format!(
                "{:?} at {}:{}-{}:{}: {}",
                error.kind,
                error.span.start.row,
                error.span.start.column,
                error.span.end.row,
                error.span.end.column,
                error.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
