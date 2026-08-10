use anyhow::Result;
use expect_test::expect;
use func_resolver::{FileSourceKind, ProjectIndexBuilder, ProjectSource, ProjectSourceProvider};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct MemoryProvider {
    sources: BTreeMap<PathBuf, Arc<str>>,
}

impl ProjectSourceProvider for MemoryProvider {
    fn source(&self, path: &Path) -> Result<Option<ProjectSource>> {
        Ok(self.sources.get(path).cloned().map(ProjectSource::Text))
    }
}

#[test]
fn discovers_transitive_includes_once_even_with_a_cycle() {
    let provider = MemoryProvider {
        sources: BTreeMap::from([
            (
                PathBuf::from("/project/main.fc"),
                Arc::from("#include \"lib.fc\"; int main() { return helper(); }"),
            ),
            (
                PathBuf::from("/project/lib.fc"),
                Arc::from("#include \"main.fc\"; int helper() { return 1; }"),
            ),
        ]),
    };
    let project = ProjectIndexBuilder::build_with_provider(
        [PathBuf::from("/project/main.fc")],
        &provider,
        false,
    )
    .expect("project should build");
    let main_id = project
        .file_id(Path::new("/project/main.fc"))
        .expect("file should exist");
    let source = project
        .file(main_id)
        .expect("file should exist")
        .source()
        .source
        .as_ref();
    let target = project
        .target_at(main_id, source.find("helper").expect("helper should exist"))
        .expect("helper should resolve");

    expect![[r#"
        (
            2,
            Some(
                "helper",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        project.files().len(),
        project
            .declaration(target)
            .and_then(|location| project.file(location.file_id))
            .and_then(|file| file.symbol_at(file.index().symbols[0].name_span.start()))
            .map(|symbol| symbol.name.as_ref()),
    ));
}

#[test]
fn indexes_the_real_editor_stubs() {
    let project = ProjectIndexBuilder::new()
        .build()
        .expect("project should build");
    let stubs = project
        .files()
        .values()
        .find(|file| file.index().source_kind == FileSourceKind::Stubs)
        .expect("stubs should be indexed");
    let selected = stubs
        .index()
        .symbols
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.name.as_ref(),
                "muldiv"
                    | "null?"
                    | "throw"
                    | "load_int"
                    | "store_uint"
                    | "at"
                    | "touch"
                    | "run_method3"
            )
        })
        .map(|symbol| (symbol.name.as_ref(), symbol.doc.as_ref()))
        .collect::<Vec<_>>();

    expect![[r#"
        [
            (
                "muldiv",
                "",
            ),
            (
                "null?",
                "Checks whether [x] is a _Null_, and returns `-1` or `0` accordingly.",
            ),
            (
                "throw",
                "Throws exception [`excno`] with parameter zero.\n\nIn other words, it transfers control to the continuation in `c2`,\npushing `0` and [`excno`] into it's stack, and discarding the old stack altogether.",
            ),
            (
                "load_int",
                "Loads a signed [`len`]-bit integer from slice [`s`].",
            ),
            (
                "load_int",
                "Loads a signed [`len`]-bit integer from slice [`s`].",
            ),
            (
                "store_uint",
                "Stores a unsigned [`len`]-bit integer [`x`] into [`b`] for `0 ≤ len ≤ 256`.",
            ),
            (
                "at",
                "Returns the [`index`]-th element of tuple [`t`].",
            ),
            (
                "touch",
                "Moves a variable [x] to the top of the stack.",
            ),
            (
                "touch",
                "Moves a variable [x] to the top of the stack.",
            ),
            (
                "run_method3",
                "",
            ),
        ]
    "#]]
    .assert_debug_eq(&selected);
}
