use expect_test::expect;
use func_resolver::{ProjectFile, ProjectIndexBuilder, ProjectSource};
use func_ty::TypeDb;
use std::path::Path;
use std::sync::Arc;

#[test]
fn infers_forward_constant_chains_and_cross_file_references() {
    let library = "const first = second; const second = 42; int helper() { return first; } const result = helper();";
    let main = "#include \"lib.fc\"; int main() { return helper(); }";
    let project = ProjectIndexBuilder::new()
        .include_stubs(false)
        .add_file(ProjectFile::workspace(
            "/project/lib.fc",
            ProjectSource::Text(Arc::from(library)),
        ))
        .add_file(ProjectFile::workspace(
            "/project/main.fc",
            ProjectSource::Text(Arc::from(main)),
        ))
        .build()
        .expect("project should build");
    let types = TypeDb::new(&project);
    let library_file = project
        .file_by_path(Path::new("/project/lib.fc"))
        .expect("library should exist");
    let main_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("main file should exist");
    let actual = (
        types
            .symbol_type(library_file.index().symbols[0].id)
            .expect("constant should have a type")
            .to_string(),
        types
            .type_at(
                &project,
                main_id,
                main.find("helper").expect("helper should exist"),
            )
            .expect("helper should have a type")
            .to_string(),
        types
            .symbol_type(library_file.index().symbols[3].id)
            .expect("call result should have a type")
            .to_string(),
    );

    expect![[r#"
        (
            "int",
            "() -> int",
            "int",
        )
    "#]]
    .assert_debug_eq(&actual);
}

#[test]
fn leaves_cyclic_untyped_constants_unknown() {
    let source = "const left = right; const right = left;";
    let project = ProjectIndexBuilder::new()
        .include_stubs(false)
        .add_file(ProjectFile::workspace(
            "/project/main.fc",
            ProjectSource::Text(Arc::from(source)),
        ))
        .build()
        .expect("project should build");
    let types = TypeDb::new(&project);
    let file = project
        .file_by_path(Path::new("/project/main.fc"))
        .expect("file should exist");
    let actual = file
        .index()
        .symbols
        .iter()
        .map(|symbol| {
            types
                .symbol_type(symbol.id)
                .expect("symbol should have a type")
                .to_string()
        })
        .collect::<Vec<_>>();

    expect![[r#"
        [
            "unknown",
            "unknown",
        ]
    "#]]
    .assert_debug_eq(&actual);
}
