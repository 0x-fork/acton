use expect_test::expect;
use func_resolver::{ProjectFile, ProjectIndex, ProjectIndexBuilder, ProjectSource};
use func_ty::{Ty, TypeDb};
use std::path::Path;
use std::sync::Arc;

fn project(source: &str) -> ProjectIndex {
    ProjectIndexBuilder::new()
        .include_stubs(false)
        .add_file(ProjectFile::workspace(
            "/project/main.fc",
            ProjectSource::Text(Arc::from(source)),
        ))
        .build()
        .expect("project should build")
}

fn type_at(project: &ProjectIndex, types: &TypeDb, source: &str, needle: &str) -> Ty {
    let file_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("file should exist");
    let offset = source.find(needle).expect("needle should exist");
    types
        .type_at(project, file_id, offset)
        .unwrap_or(Ty::Unknown)
}

#[test]
fn converts_declared_func_types() {
    let source = r#"
        global cell storage;
        const slice label = "value";
        (int, slice) split(int value, [cell, slice] data) { return (value, label); }
    "#;
    let project = project(source);
    let types = TypeDb::new(&project);
    let file = project
        .file_by_path(Path::new("/project/main.fc"))
        .expect("file should exist");
    let actual = file
        .index()
        .symbols
        .iter()
        .map(|symbol| {
            (
                symbol.name.as_ref(),
                types
                    .symbol_type(symbol.id)
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();

    expect![[r#"
        [
            (
                "storage",
                "cell",
            ),
            (
                "label",
                "slice",
            ),
            (
                "split",
                "(int, [cell, slice]) -> (int, slice)",
            ),
        ]
    "#]]
    .assert_debug_eq(&actual);
}

#[test]
fn infers_literal_constants_and_call_results() {
    let source = r#"
        const number = 42;
        const text = "hello";
        int helper(int value) { return value; }
        int main() { return helper(number); }
    "#;
    let project = project(source);
    let types = TypeDb::new(&project);
    let actual = [
        type_at(&project, &types, source, "number ="),
        type_at(&project, &types, source, "text ="),
        type_at(&project, &types, source, "helper(number)"),
        type_at(&project, &types, source, "number);"),
    ];

    expect![[r"
        [
            Primitive(
                Int,
            ),
            Primitive(
                Slice,
            ),
            Function {
                parameters: [
                    Primitive(
                        Int,
                    ),
                ],
                return_ty: Primitive(
                    Int,
                ),
            },
            Primitive(
                Int,
            ),
        ]
    "]]
    .assert_debug_eq(&actual);
}

#[test]
fn exposes_parameter_and_type_parameter_types() {
    let source = "forall X -> X identity(X value) { return value; }";
    let project = project(source);
    let types = TypeDb::new(&project);
    let file_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("file should exist");
    let resolution = project
        .resolution(file_id)
        .expect("file should be resolved");
    let actual = resolution
        .locals
        .iter()
        .map(|local| {
            (
                local.name.as_ref(),
                types
                    .local_type(local.id)
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();

    expect![[r#"
        [
            (
                "X",
                "X",
            ),
            (
                "value",
                "X",
            ),
        ]
    "#]]
    .assert_debug_eq(&actual);
}

#[test]
fn preserves_holes_and_unknown_relaxed_parameters() {
    let source = "_ transform(value) { return value; }";
    let project = project(source);
    let types = TypeDb::new(&project);
    let file = project
        .file_by_path(Path::new("/project/main.fc"))
        .expect("file should exist");
    let symbol = &file.index().symbols[0];
    let locals = project
        .resolution(file.index().id)
        .expect("file should be resolved")
        .locals
        .iter()
        .map(|local| {
            types
                .local_type(local.id)
                .expect("local should have a type")
                .to_string()
        })
        .collect::<Vec<_>>();

    expect![[r#"
        (
            "(unknown) -> _",
            [
                "unknown",
            ],
        )
    "#]]
    .assert_debug_eq(&(
        types
            .symbol_type(symbol.id)
            .expect("symbol should have a type")
            .to_string(),
        locals,
    ));
}
