use crate::{
    FileId, FileResolution, LocalDef, LocalDefKind, LocalId, NameUse, ProjectIndex,
    ResolutionError, ResolvedTarget, Span, Symbol, SymbolId, SymbolKind, normalize_name,
};
use func_syntax::{AstNode, AstNodeBytesKind, NodeTraversalExt};
use std::collections::BTreeMap;
use std::sync::Arc;
use tree_sitter::Node;

pub(crate) fn resolve_file(project: &ProjectIndex, file_id: FileId) -> FileResolution {
    let Some(file) = project.file(file_id) else {
        return FileResolution::default();
    };
    let source = file.source().source.as_ref();
    let root = file.source().root().syntax();
    let mut locals = collect_locals(file_id, project, root, source);
    locals.sort_by_key(|local| (local.name_span.start(), local.id.local_id));

    let declaration_targets = declaration_targets(project, file_id, &locals);
    let visible_symbols = project.visible_symbols(file_id);
    let mut uses = Vec::new();
    let mut unresolved = Vec::new();

    for node in root.traverse().filter(Node::is_named) {
        if !matches!(node.kind_bytes(), b"identifier" | b"type_identifier")
            || is_duplicate_alias_node(node)
        {
            continue;
        }
        let span = Span::from(node);
        if declaration_targets.contains_key(&span) {
            continue;
        }
        let raw_name = node.utf8_text(source.as_bytes()).unwrap_or("");
        let name: Arc<str> = Arc::from(normalize_name(raw_name));
        if name.is_empty() || name.as_ref() == "_" {
            continue;
        }

        let type_position = node.kind() == "type_identifier";
        let target = resolve_local(&locals, &name, span.start(), type_position)
            .map(|local| ResolvedTarget::Local(local.id))
            .or_else(|| {
                (!type_position)
                    .then(|| resolve_symbol(&visible_symbols, &name))
                    .flatten()
                    .map(|symbol| ResolvedTarget::Symbol(symbol.id))
            });
        if let Some(target) = target {
            uses.push(NameUse { name, span, target });
        } else {
            unresolved.push(ResolutionError { name, span });
        }
    }

    FileResolution {
        locals,
        uses,
        unresolved,
    }
}

fn declaration_targets(
    project: &ProjectIndex,
    file_id: FileId,
    locals: &[LocalDef],
) -> BTreeMap<Span, ResolvedTarget> {
    let mut targets = BTreeMap::new();
    if let Some(file) = project.file(file_id) {
        for symbol in &file.index().symbols {
            targets.insert(symbol.name_span, ResolvedTarget::Symbol(symbol.id));
        }
    }
    for local in locals {
        targets.insert(local.name_span, ResolvedTarget::Local(local.id));
    }
    targets
}

fn collect_locals(
    file_id: FileId,
    project: &ProjectIndex,
    root: Node<'_>,
    source: &str,
) -> Vec<LocalDef> {
    let Some(file) = project.file(file_id) else {
        return Vec::new();
    };
    let mut locals = Vec::new();
    for symbol in &file.index().symbols {
        let SymbolKind::Function {
            parameters,
            type_parameters,
            body_span,
            ..
        } = &symbol.kind
        else {
            continue;
        };
        let scope_span = body_span.unwrap_or(symbol.declaration_span);
        for parameter in parameters {
            let (Some(name), Some(name_span)) = (&parameter.name, parameter.name_span) else {
                continue;
            };
            push_local(
                &mut locals,
                file_id,
                Some(symbol.id),
                name.clone(),
                name_span,
                parameter.declaration_span,
                scope_span,
                scope_span.start(),
                parameter.type_span,
                LocalDefKind::Parameter,
            );
        }
        for parameter in type_parameters {
            push_local(
                &mut locals,
                file_id,
                Some(symbol.id),
                parameter.name.clone(),
                parameter.name_span,
                parameter.declaration_span,
                symbol.declaration_span,
                symbol.declaration_span.start(),
                Some(parameter.name_span),
                LocalDefKind::TypeParameter,
            );
        }
    }

    for node in root.traverse().filter(Node::is_named) {
        match node.kind_bytes() {
            b"var_declaration" => {
                let Some(name_node) = node.child_by_field_name("name") else {
                    continue;
                };
                let Some(scope_node) = local_scope(node) else {
                    continue;
                };
                let name: Arc<str> = Arc::from(normalize_name(
                    name_node.utf8_text(source.as_bytes()).unwrap_or(""),
                ));
                push_local(
                    &mut locals,
                    file_id,
                    owner_symbol(file.index().symbols.as_slice(), node.start_byte()),
                    name,
                    name_node.into(),
                    node.into(),
                    scope_node.into(),
                    name_node.end_byte(),
                    node.child_by_field_name("type").map(Into::into),
                    LocalDefKind::Variable,
                );
            }
            b"catch_clause" => {
                let Some(expression) = node.child_by_field_name("catch_expr") else {
                    continue;
                };
                let Some(body) = node.child_by_field_name("catch_body") else {
                    continue;
                };
                for identifier in expression_children(expression) {
                    if identifier.kind() != "identifier" || is_duplicate_alias_node(identifier) {
                        continue;
                    }
                    let name: Arc<str> = Arc::from(normalize_name(
                        identifier.utf8_text(source.as_bytes()).unwrap_or(""),
                    ));
                    push_local(
                        &mut locals,
                        file_id,
                        owner_symbol(file.index().symbols.as_slice(), node.start_byte()),
                        name,
                        identifier.into(),
                        expression.into(),
                        body.into(),
                        body.start_byte(),
                        None,
                        LocalDefKind::CatchParameter,
                    );
                }
            }
            _ => {}
        }
    }
    locals
}

fn expression_children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    NodeTraversalExt::traverse(&node).skip(1)
}

#[allow(clippy::too_many_arguments)]
fn push_local(
    locals: &mut Vec<LocalDef>,
    file_id: FileId,
    owner: Option<SymbolId>,
    name: Arc<str>,
    name_span: Span,
    declaration_span: Span,
    scope_span: Span,
    visible_from: usize,
    type_span: Option<Span>,
    kind: LocalDefKind,
) {
    if name.is_empty() || name.as_ref() == "_" {
        return;
    }
    let local_id = u32::try_from(locals.len()).expect("a file cannot contain more locals");
    locals.push(LocalDef {
        id: LocalId { file_id, local_id },
        owner,
        name,
        name_span,
        declaration_span,
        scope_span,
        visible_from,
        type_span,
        kind,
    });
}

fn owner_symbol(symbols: &[Symbol], offset: usize) -> Option<SymbolId> {
    symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Function { .. }))
        .find(|symbol| symbol.declaration_span.contains(offset))
        .map(|symbol| symbol.id)
}

fn local_scope(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "block_statement" {
            if parent
                .parent()
                .is_some_and(|owner| owner.kind() == "do_while_statement")
            {
                return parent.parent();
            }
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn resolve_local<'a>(
    locals: &'a [LocalDef],
    name: &str,
    offset: usize,
    type_position: bool,
) -> Option<&'a LocalDef> {
    locals
        .iter()
        .filter(|local| local.name.as_ref() == name)
        .filter(|local| local.scope_span.contains(offset) && local.visible_from <= offset)
        .filter(|local| matches!(local.kind, LocalDefKind::TypeParameter) == type_position)
        .min_by_key(|local| (local.scope_span.len(), usize::MAX - local.name_span.start()))
}

fn resolve_symbol<'a>(symbols: &[&'a Symbol], name: &str) -> Option<&'a Symbol> {
    symbols
        .iter()
        .copied()
        .find(|symbol| symbol.name.as_ref() == name)
}

fn is_duplicate_alias_node(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == node.kind()
            && parent.start_byte() == node.start_byte()
            && parent.end_byte() == node.end_byte()
    })
}
