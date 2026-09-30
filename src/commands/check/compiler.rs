use crate::commands::check::pos;
use acton_config::config::ActonConfig;
use std::fmt::Write;
use std::path::Path;
use std::time::Instant;
use tolk_compiler::Compiler;
use tolk_linter::Rule;
use tolk_linter::diagnostic::{Annotation, Diagnostic, Severity};
use tolk_resolver::{FileDb, Span};
use tree_sitter::Point;

pub(super) fn check_with_compiler(
    root: &Path,
    file_db: &FileDb,
    acton_config: &ActonConfig,
    all_diagnostics: &mut Vec<Diagnostic>,
) -> anyhow::Result<bool> {
    let now = Instant::now();

    let mappings = acton_config.mappings();
    let compiler = Compiler::new().with_mappings(&mappings);
    let compiler_errors = compiler.check(root)?;
    log::debug!(
        "Run compiler check took {:?}, found {} errors in {}",
        now.elapsed(),
        compiler_errors.len(),
        root.display()
    );

    let has_compiler_errors = compiler_errors.is_empty();

    for compiler_error in compiler_errors {
        let path = compiler_error
            .range
            .as_ref()
            .map_or(root, |range| Path::new(&range.file_name));
        let file_info = match file_db.process(path) {
            Ok(file_info) => file_info,
            Err(error) => {
                log::warn!("Cannot process file for compiler error {error}");
                continue;
            }
        };
        let mut annotations = Vec::new();
        if let Some(range) = &compiler_error.range {
            annotations.push(compiler_annotation(
                range,
                file_info.source().source.as_ref(),
                None,
                true,
            ));
        }
        let mut diagnostic = Diagnostic {
            file_id: file_info.id(),
            severity: Severity::Error,
            code: Some("C001".to_owned()),
            rule: Rule::CompilerError,
            name: "compiler-error",
            message: compiler_error.message,
            annotations,
            fixes: vec![],
            help: compiler_error.in_function,
        };
        let mut related_diagnostics = Vec::new();
        for note in compiler_error.secondary_locations {
            if let Some(range) = &note.range
                && let Ok(related_file) = file_db.process(Path::new(&range.file_name))
            {
                let annotation = compiler_annotation(
                    range,
                    related_file.source().source.as_ref(),
                    Some(note.note.clone()),
                    related_file.id() != file_info.id(),
                );
                if related_file.id() == file_info.id() {
                    diagnostic.annotations.push(annotation);
                } else {
                    related_diagnostics.push(Diagnostic {
                        file_id: related_file.id(),
                        severity: Severity::Help,
                        code: Some("C001".to_owned()),
                        rule: Rule::CompilerError,
                        name: "compiler-error",
                        message: note.note,
                        annotations: vec![annotation],
                        fixes: vec![],
                        help: None,
                    });
                }
            } else {
                let _ = write!(diagnostic.message, "\nnote: {}", note.note);
                if let Some(range) = note.range {
                    let _ = write!(
                        diagnostic.message,
                        " ({}:{}:{})",
                        range.file_name, range.start_line_no, range.start_char_no
                    );
                }
            }
        }
        all_diagnostics.push(diagnostic);
        all_diagnostics.extend(related_diagnostics);
    }
    Ok(has_compiler_errors)
}

/// Converts native one-based byte columns into a source annotation. Invalid
/// token ranges can split UTF-8 characters, so endpoints are clamped before rendering.
fn compiler_annotation(
    range: &tolk_compiler::CompilerErrorRange,
    source: &str,
    message: Option<String>,
    is_primary: bool,
) -> Annotation {
    let offset = |line: usize, column: usize| {
        pos::byte_offset_from_point(
            &Point {
                row: line.saturating_sub(1),
                column: column.saturating_sub(1),
            },
            source,
        )
        .min(source.len())
    };
    let mut start = offset(range.start_line_no, range.start_char_no);
    while !source.is_char_boundary(start) {
        start = start.saturating_sub(1);
    }
    let mut end = offset(range.end_line_no, range.end_char_no).max(start);
    while !source.is_char_boundary(end) {
        end += 1;
    }
    Annotation {
        span: Span {
            start: start as u32,
            end: end as u32,
        },
        message,
        is_primary,
        tags: vec![],
    }
}
