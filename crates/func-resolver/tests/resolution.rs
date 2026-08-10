use expect_test::expect;
use func_resolver::{
    FileId, ProjectFile, ProjectIndex, ProjectIndexBuilder, ProjectSource, ResolvedTarget,
};
use std::path::Path;
use std::sync::Arc;

fn project(files: &[(&str, &str)]) -> ProjectIndex {
    files
        .iter()
        .fold(ProjectIndexBuilder::new(), |builder, (path, source)| {
            builder.add_file(ProjectFile::workspace(
                *path,
                ProjectSource::Text(Arc::from(*source)),
            ))
        })
        .build()
        .expect("project should build")
}

fn target_at(project: &ProjectIndex, path: &str, source: &str, needle: &str) -> String {
    let file_id = project.file_id(Path::new(path)).expect("file should exist");
    let offset = source.find(needle).expect("needle should exist");
    describe_target(
        project,
        file_id,
        project
            .target_at(file_id, offset)
            .unwrap_or_else(|| panic!("target `{needle}` should resolve")),
    )
}

fn describe_target(project: &ProjectIndex, file_id: FileId, target: ResolvedTarget) -> String {
    match target {
        ResolvedTarget::Symbol(symbol_id) => {
            let symbol = project.symbol(symbol_id).expect("symbol should exist");
            format!("symbol:{}@{}", symbol.name, symbol_id.file_id)
        }
        ResolvedTarget::Local(local_id) => {
            let local = project
                .resolution(file_id)
                .and_then(|resolution| resolution.locals.iter().find(|local| local.id == local_id))
                .expect("local should exist");
            format!("local:{:?}:{}", local.kind, local.name)
        }
    }
}

#[test]
fn resolves_locals_and_included_top_level_symbols() {
    let library = r"
        int helper(int value) { return value; }
        const int answer = 42;
    ";
    let main = r#"
        #include "lib.fc";
        forall X -> X choose(X value) {
            int outer = value;
            {
                int outer = answer;
                helper(outer);
            }
            return outer;
        }
    "#;
    let project = project(&[("/project/lib.fc", library), ("/project/main.fc", main)]);

    let actual = [
        target_at(&project, "/project/main.fc", main, "helper(outer)"),
        target_at(&project, "/project/main.fc", main, "answer;"),
        target_at(&project, "/project/main.fc", main, "value;"),
        target_at(&project, "/project/main.fc", main, "outer);"),
        target_at(&project, "/project/main.fc", main, "outer;\n        }"),
        target_at(&project, "/project/main.fc", main, "X choose"),
    ];

    expect![[r#"
        [
            "symbol:helper@0",
            "symbol:answer@0",
            "local:Parameter:value",
            "local:Variable:outer",
            "local:Variable:outer",
            "local:TypeParameter:X",
        ]
    "#]]
    .assert_debug_eq(&actual);
}

#[test]
fn resolves_transitive_includes_and_stubs() {
    let first = "#include \"middle.fc\";\nint main() { leaf(); return preload_int(); }";
    let middle = "#include \"leaf.fc\";";
    let leaf = "int leaf() { return 1; }";
    let project = project(&[
        ("/project/first.fc", first),
        ("/project/middle.fc", middle),
        ("/project/leaf.fc", leaf),
    ]);

    let actual = [
        target_at(&project, "/project/first.fc", first, "leaf()"),
        target_at(&project, "/project/first.fc", first, "preload_int"),
    ];

    expect![[r#"
        [
            "symbol:leaf@1",
            "symbol:preload_int@3",
        ]
    "#]]
    .assert_debug_eq(&actual);
}

#[test]
fn records_references_without_declarations() {
    let source = "int helper(int value) { return value; } int main() { return helper(helper(1)); }";
    let project = project(&[("/project/main.fc", source)]);
    let file_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("file should exist");
    let declaration = source.find("helper").expect("helper should exist");
    let target = project
        .target_at(file_id, declaration)
        .expect("helper should resolve");
    let references = project
        .references_to(target)
        .into_iter()
        .map(|(_, span)| &source[span.start()..span.end()])
        .collect::<Vec<_>>();

    expect![[r#"
        [
            "helper",
            "helper",
        ]
    "#]]
    .assert_debug_eq(&references);
}

#[test]
fn limits_nested_variables_to_their_block() {
    let source = "int main() { { int hidden = 1; hidden; } hidden; return 0; }";
    let project = project(&[("/project/main.fc", source)]);
    let file_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("file should exist");
    let resolution = project
        .resolution(file_id)
        .expect("file should be resolved");
    let uses = resolution
        .uses
        .iter()
        .filter(|usage| usage.name.as_ref() == "hidden")
        .count();
    let unresolved = resolution
        .unresolved
        .iter()
        .filter(|usage| usage.name.as_ref() == "hidden")
        .count();

    expect![[r"
        (
            1,
            1,
        )
    "]]
    .assert_debug_eq(&(uses, unresolved));
}

#[test]
fn indexes_function_metadata_and_documentation() {
    let source =
        ";;; Returns the input\nforall X -> X identity(X value) method_id(0x100) { return value; }";
    let project = project(&[("/project/main.fc", source)]);
    let file = project
        .file_by_path(Path::new("/project/main.fc"))
        .expect("file should exist");
    let symbol = &file.index().symbols[0];
    let func_resolver::SymbolKind::Function {
        parameters,
        type_parameters,
        method_id,
        ..
    } = &symbol.kind
    else {
        panic!("expected function");
    };
    let actual = (
        symbol.name.as_ref(),
        symbol.doc.as_ref(),
        parameters[0].name.as_deref(),
        type_parameters[0].name.as_ref(),
        method_id.is_some(),
        method_id
            .and_then(|method_id| method_id.value_span)
            .map(|span| &source[span.start()..span.end()]),
    );

    expect![[r#"
        (
            "identity",
            "Returns the input",
            Some(
                "value",
            ),
            "X",
            true,
            Some(
                "0x100",
            ),
        )
    "#]]
    .assert_debug_eq(&actual);
}

#[test]
fn exposes_local_kinds_for_inspections_and_semantic_tokens() {
    let source = "forall X -> int main(X value) { int local = 1; return local; }";
    let project = project(&[("/project/main.fc", source)]);
    let file_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("file should exist");
    let kinds = project
        .resolution(file_id)
        .expect("file should be resolved")
        .locals
        .iter()
        .map(|local| local.kind)
        .collect::<Vec<_>>();

    expect![[r"
        [
            TypeParameter,
            Parameter,
            Variable,
        ]
    "]]
    .assert_debug_eq(&kinds);
}
