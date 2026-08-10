use expect_test::expect;
use func_resolver::{ProjectFile, ProjectIndexBuilder, ProjectSource};
use std::path::Path;
use std::sync::Arc;

fn resolution(source: &str) -> func_resolver::FileResolution {
    let project = ProjectIndexBuilder::new()
        .include_stubs(false)
        .add_file(ProjectFile::workspace(
            "/project/main.fc",
            ProjectSource::Text(Arc::from(source)),
        ))
        .build()
        .expect("project should build");
    let file_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("file should exist");
    project
        .resolution(file_id)
        .expect("file should be resolved")
        .as_ref()
        .clone()
}

#[test]
fn keeps_do_body_variables_visible_in_the_until_condition() {
    let source = "int main() { do { int done = 1; } until done; return 0; }";
    let actual = resolution(source);
    let uses = actual
        .uses
        .iter()
        .filter(|usage| usage.name.as_ref() == "done")
        .count();
    let unresolved = actual
        .unresolved
        .iter()
        .filter(|usage| usage.name.as_ref() == "done")
        .count();

    expect![[r"
        (
            1,
            0,
        )
    "]]
    .assert_debug_eq(&(uses, unresolved));
}

#[test]
fn limits_catch_parameters_to_the_catch_body() {
    let source = "int main() { try { return 0; } catch (code, value) { code + value; } code; }";
    let actual = resolution(source);
    let locals = actual
        .locals
        .iter()
        .filter(|local| local.kind == func_resolver::LocalDefKind::CatchParameter)
        .map(|local| local.name.as_ref())
        .collect::<Vec<_>>();
    let unresolved_code = actual
        .unresolved
        .iter()
        .filter(|usage| usage.name.as_ref() == "code")
        .count();

    expect![[r#"
        (
            [
                "code",
                "value",
            ],
            1,
        )
    "#]]
    .assert_debug_eq(&(locals, unresolved_code));
}
