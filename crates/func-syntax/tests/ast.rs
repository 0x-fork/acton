use expect_test::expect;
use func_syntax::{
    AstNode, Expr, FunctionBody, FunctionLike, HasName, HasType, NumberStringLit, Specifier, Stmt,
    TopLevel, Type, VersionIdent, Walker, parse,
};
use std::fmt::Write;

#[test]
fn exposes_top_level_declarations() {
    let source = r#"
        #include "stdlib.fc";
        global int counter, slice owner;
        const int op = 1, name = "counter";
        int add(int x, int y) inline method_id(7) { return x + y; }
    "#;
    let file = parse(source).expect("source should parse");
    assert!(!file.has_errors(), "{:?}", file.errors());

    let mut actual = String::new();
    for item in file.top_levels() {
        match item {
            TopLevel::Import(import) => {
                writeln!(actual, "import {}", import.path().unwrap().text(source)).unwrap();
            }
            TopLevel::GlobalVars(group) => {
                for declaration in group.declarations() {
                    writeln!(
                        actual,
                        "global {} {}",
                        declaration.type_hint().map_or("", |ty| ty.text(source)),
                        declaration.name().unwrap().text(source)
                    )
                    .unwrap();
                }
            }
            TopLevel::Constants(group) => {
                for declaration in group.declarations() {
                    writeln!(
                        actual,
                        "const {} {} = {}",
                        declaration.type_hint().map_or("", |ty| ty.text(source)),
                        declaration.name().unwrap().text(source),
                        declaration.value().unwrap().text(source)
                    )
                    .unwrap();
                }
            }
            TopLevel::Function(function) => {
                writeln!(
                    actual,
                    "function {}: {} ({} params)",
                    function.name().unwrap().text(source),
                    function.return_type().unwrap().text(source),
                    function.parameters().unwrap().declarations().count()
                )
                .unwrap();
                for specifier in function.specifiers().unwrap().items() {
                    match specifier {
                        Specifier::Inline(node) => {
                            writeln!(actual, "  specifier {}", node.text(source)).unwrap();
                        }
                        Specifier::MethodId(node) => {
                            writeln!(actual, "  specifier {}", node.text(source)).unwrap();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    expect![[r#"
        import "stdlib.fc"
        global int counter
        global slice owner
        const int op = 1
        const  name = "counter"
        function add: int (2 params)
          specifier inline
          specifier method_id(7)
    "#]]
    .assert_eq(&actual);
}

#[test]
fn exposes_types_function_bodies_and_statements() {
    let source = r#"
        forall X -> (X, X) duplicate(X value) impure {
            var local = value;
            if (local) { return (local, local); }
            return value;
        }
        int add(int x, int y) asm(x y -> 0) "ADD";
    "#;
    let file = parse(source).expect("source should parse");
    assert!(!file.has_errors(), "{:?}", file.errors());

    let mut functions = file.functions();
    let duplicate = functions.next().unwrap();
    let Type::Tensor(return_type) = duplicate.return_type().unwrap() else {
        panic!("expected tensor return type");
    };
    assert_eq!(return_type.types().count(), 2);
    assert_eq!(
        duplicate.type_parameters().unwrap().declarations().count(),
        1
    );

    let FunctionBody::Block(body) = duplicate.body().unwrap() else {
        panic!("expected block body");
    };
    let statements = body.statements().collect::<Vec<_>>();
    assert_eq!(statements.len(), 3);
    let Stmt::Expr(local) = statements[0] else {
        panic!("expected local declaration statement");
    };
    assert!(matches!(
        local.expressions().next(),
        Some(Expr::LocalVarsDeclaration(_))
    ));
    assert!(matches!(statements[1], Stmt::If(_)));
    assert!(matches!(statements[2], Stmt::Return(_)));

    let add = functions.next().unwrap();
    let FunctionBody::Asm(body) = add.body().unwrap() else {
        panic!("expected assembly body");
    };
    assert_eq!(body.identifiers().count(), 2);
    assert_eq!(body.result_indices().count(), 1);
    assert_eq!(body.instructions().next().unwrap().text(source), "\"ADD\"");
}

#[test]
fn finds_covering_top_level_item() {
    let source = "int first() { return 1; }\nint second() { return 2; }\n";
    let file = parse(source).expect("source should parse");
    let offset = source.find("return 2").unwrap();
    let item = file
        .find_top_level_at(offset, offset + "return 2".len())
        .unwrap();

    let TopLevel::Function(function) = item else {
        panic!("expected function");
    };
    assert_eq!(function.name().unwrap().text(source), "second");

    let first_end = source.find('\n').unwrap();
    assert!(file.find_top_level_at(first_end, first_end).is_none());
    assert!(file.find_top_level_at(first_end + 1, first_end).is_none());
}

#[test]
fn exposes_comments_in_source_order() {
    let source = ";; header\nint main() { ;; body\n return 0; }\n;; footer";
    let file = parse(source).expect("source should parse");
    let comments = file
        .comments()
        .map(|comment| comment.text(source))
        .collect::<Vec<_>>();

    expect![[r#"
        [
            ";; header",
            ";; body",
            ";; footer",
        ]
    "#]]
    .assert_debug_eq(&comments);
}

#[test]
fn walker_visits_nested_identifiers_in_source_order() {
    let source = "int add(int x, int y) { var result = x + y; return result; }";
    let file = parse(source).expect("source should parse");
    let mut walker = IdentifierCollector {
        source,
        names: Vec::new(),
    };

    walker.visit_source_file(&file);

    expect![[r#"
        [
            "add",
            "x",
            "y",
            "result",
            "x",
            "y",
            "result",
        ]
    "#]]
    .assert_debug_eq(&walker.names);
}

#[test]
fn walker_preserves_elseif_source_order() {
    let source = "int choose(int c1, int c2) { if c1 { first(); } elseif c2 { second(); } }";
    let file = parse(source).expect("source should parse");
    let mut walker = IdentifierCollector {
        source,
        names: Vec::new(),
    };

    walker.visit_source_file(&file);

    expect![[r#"
        [
            "choose",
            "c1",
            "c2",
            "c1",
            "first",
            "c2",
            "second",
        ]
    "#]]
    .assert_debug_eq(&walker.names);
}

#[test]
fn walker_visits_pragma_and_number_string_values() {
    let source = "#pragma version >=0.4.0\nconst hash = \"ABCD\"H;";
    let file = parse(source).expect("source should parse");
    let mut walker = LiteralCollector {
        source,
        values: Vec::new(),
    };

    walker.visit_source_file(&file);

    expect![[r#"
        [
            ">=0.4.0",
            "\"ABCD\"H",
        ]
    "#]]
    .assert_debug_eq(&walker.values);
}

struct IdentifierCollector<'source> {
    source: &'source str,
    names: Vec<&'source str>,
}

impl<'tree> Walker<'tree> for IdentifierCollector<'_> {
    type Result = ();

    fn default_result(&self) -> Self::Result {}

    fn visit_ident(&mut self, ident: &func_syntax::Ident<'tree>) -> Self::Result {
        self.names.push(ident.text(self.source));
    }
}

struct LiteralCollector<'source> {
    source: &'source str,
    values: Vec<&'source str>,
}

impl<'tree> Walker<'tree> for LiteralCollector<'_> {
    type Result = ();

    fn default_result(&self) -> Self::Result {}

    fn visit_number_string(&mut self, value: &NumberStringLit<'tree>) -> Self::Result {
        self.values.push(value.text(self.source));
    }

    fn visit_version(&mut self, version: &VersionIdent<'tree>) -> Self::Result {
        self.values.push(version.text(self.source));
    }
}
