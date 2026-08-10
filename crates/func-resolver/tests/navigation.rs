use expect_test::expect;
use func_resolver::{ProjectFile, ProjectIndexBuilder, ProjectSource};
use std::path::Path;
use std::sync::Arc;

#[test]
fn exposes_declarations_references_and_imports_for_navigation() {
    let library = "int helper(int value) { return value; }";
    let main = "#include \"lib.fc\"; int main() { int local = helper(1); return local; }";
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
    let main_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("file should exist");
    let helper_target = project
        .target_at(main_id, main.find("helper").expect("helper should exist"))
        .expect("helper should resolve");
    let local_target = project
        .target_at(main_id, main.rfind("local").expect("local should exist"))
        .expect("local should resolve");
    let helper_declaration = project
        .declaration(helper_target)
        .expect("helper declaration should exist");
    let local_declaration = project
        .declaration(local_target)
        .expect("local declaration should exist");
    let import = project
        .import_at(main_id, main.find("lib.fc").expect("import should exist"))
        .expect("import should resolve");
    let actual = (
        &library[helper_declaration.name_span.start()..helper_declaration.name_span.end()],
        &main[local_declaration.name_span.start()..local_declaration.name_span.end()],
        project.references_to(helper_target).len(),
        import.path.as_path(),
        import.target,
    );

    expect![[r#"
        (
            "helper",
            "local",
            1,
            "/project/lib.fc",
            Some(
                0,
            ),
        )
    "#]]
    .assert_debug_eq(&actual);
}
