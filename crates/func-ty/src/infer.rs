use crate::{PrimitiveTy, Ty};
use func_resolver::{
    FileId, LocalDefKind, LocalId, ProjectIndex, ResolvedTarget, Span, SymbolId, SymbolKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tree_sitter::Node;

#[derive(Debug, Clone, Default)]
pub struct TypeDb {
    symbol_types: BTreeMap<SymbolId, Ty>,
    local_types: BTreeMap<LocalId, Ty>,
}

impl TypeDb {
    #[must_use]
    pub fn new(project: &ProjectIndex) -> Self {
        let mut db = Self::default();
        for (&file_id, file) in project.files() {
            let source = file.source().source.as_ref();
            for symbol in &file.index().symbols {
                let ty = declared_symbol_type(file.source().tree.root_node(), &symbol.kind, source);
                db.symbol_types.insert(symbol.id, ty);
            }
            if let Some(resolution) = project.resolution(file_id) {
                for local in &resolution.locals {
                    let ty = if local.kind == LocalDefKind::TypeParameter {
                        Ty::TypeParameter(local.name.clone())
                    } else {
                        local
                            .type_span
                            .and_then(|span| node_for_span(file.source().tree.root_node(), span))
                            .and_then(|node| convert_type(node, source))
                            .unwrap_or(Ty::Unknown)
                    };
                    db.local_types.insert(local.id, ty);
                }
            }
        }

        let constant_count = project
            .files()
            .values()
            .flat_map(|file| &file.index().symbols)
            .filter(|symbol| matches!(symbol.kind, SymbolKind::Constant { .. }))
            .count();
        for _ in 0..constant_count {
            let mut changed = false;
            for (&file_id, file) in project.files() {
                let root = file.source().tree.root_node();
                let source = file.source().source.as_ref();
                for symbol in &file.index().symbols {
                    let SymbolKind::Constant {
                        type_span: None,
                        value_span: Some(value_span),
                    } = symbol.kind
                    else {
                        continue;
                    };
                    let Some(value) = node_for_span(root, value_span) else {
                        continue;
                    };
                    let Some(inferred) = infer_expression(project, &db, file_id, value, source)
                    else {
                        continue;
                    };
                    if inferred.is_unknown() || db.symbol_types.get(&symbol.id) == Some(&inferred) {
                        continue;
                    }
                    db.symbol_types.insert(symbol.id, inferred);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        db
    }

    #[must_use]
    pub fn symbol_type(&self, symbol_id: SymbolId) -> Option<&Ty> {
        self.symbol_types.get(&symbol_id)
    }

    #[must_use]
    pub fn local_type(&self, local_id: LocalId) -> Option<&Ty> {
        self.local_types.get(&local_id)
    }

    #[must_use]
    pub fn target_type(&self, target: ResolvedTarget) -> Option<&Ty> {
        match target {
            ResolvedTarget::Symbol(symbol) => self.symbol_type(symbol),
            ResolvedTarget::Local(local) => self.local_type(local),
        }
    }

    #[must_use]
    pub fn type_at(&self, project: &ProjectIndex, file_id: FileId, offset: usize) -> Option<Ty> {
        if let Some(target) = project.target_at(file_id, offset)
            && let Some(ty) = self.target_type(target)
        {
            return Some(ty.clone());
        }
        let file = project.file(file_id)?;
        let root = file.source().tree.root_node();
        let node = root.descendant_for_byte_range(offset, offset)?;
        infer_expression(project, self, file_id, node, file.source().source.as_ref())
    }
}

#[must_use]
pub fn convert_type(node: Node<'_>, source: &str) -> Option<Ty> {
    match node.kind() {
        "primitive_type" => {
            PrimitiveTy::parse(node.utf8_text(source.as_bytes()).ok()?).map(Ty::Primitive)
        }
        "var_type" => Some(Ty::Var),
        "hole_type" => Some(Ty::Hole),
        "type_identifier" => Some(Ty::TypeParameter(Arc::from(
            node.utf8_text(source.as_bytes()).ok()?.trim_matches('`'),
        ))),
        "tensor_type" => Some(Ty::Tensor(field_types(node, "types", source))),
        "tuple_type" => Some(Ty::Tuple(field_types(node, "types", source))),
        "function_type" => {
            let mut cursor = node.walk();
            let mut types = node
                .named_children(&mut cursor)
                .filter_map(|child| convert_type(child, source));
            let input = types.next()?;
            let output = types.next()?;
            Some(Ty::Function {
                parameters: tensor_parameters(input),
                return_ty: Box::new(output),
            })
        }
        _ => {
            let mut cursor = node.walk();
            let mut children = node.named_children(&mut cursor);
            let first = children.next()?;
            children
                .next()
                .is_none()
                .then(|| convert_type(first, source))
                .flatten()
        }
    }
}

fn field_types(node: Node<'_>, field: &str, source: &str) -> Vec<Ty> {
    let mut cursor = node.walk();
    let mut seen = BTreeSet::new();
    node.children_by_field_name(field, &mut cursor)
        .filter(|child| child.is_named() && seen.insert((child.start_byte(), child.end_byte())))
        .map(|child| convert_type(child, source).unwrap_or(Ty::Unknown))
        .collect()
}

fn tensor_parameters(input: Ty) -> Vec<Ty> {
    match input {
        Ty::Tensor(parameters) => parameters,
        other => vec![other],
    }
}

fn declared_symbol_type(root: Node<'_>, kind: &SymbolKind, source: &str) -> Ty {
    match kind {
        SymbolKind::Function {
            parameters,
            return_type_span,
            ..
        } => {
            let parameters = parameters
                .iter()
                .map(|parameter| {
                    parameter
                        .type_span
                        .and_then(|span| node_for_span(root, span))
                        .and_then(|node| convert_type(node, source))
                        .unwrap_or(Ty::Unknown)
                })
                .collect();
            let return_ty = return_type_span
                .and_then(|span| node_for_span(root, span))
                .and_then(|node| convert_type(node, source))
                .unwrap_or(Ty::Unknown);
            Ty::Function {
                parameters,
                return_ty: Box::new(return_ty),
            }
        }
        SymbolKind::GlobalVariable { type_span } => type_span
            .and_then(|span| node_for_span(root, span))
            .and_then(|node| convert_type(node, source))
            .unwrap_or(Ty::Unknown),
        SymbolKind::Constant {
            type_span,
            value_span: _,
        } => type_span
            .and_then(|span| node_for_span(root, span))
            .and_then(|node| convert_type(node, source))
            .unwrap_or(Ty::Unknown),
    }
}

fn infer_expression(
    project: &ProjectIndex,
    db: &TypeDb,
    file_id: FileId,
    node: Node<'_>,
    source: &str,
) -> Option<Ty> {
    match node.kind() {
        "number_literal" | "number_string_literal" => Some(Ty::INT),
        "string_literal" | "slice_string_literal" => Some(Ty::SLICE),
        "tensor_expression" => Some(Ty::Tensor(field_expression_types(
            project,
            db,
            file_id,
            node,
            "expressions",
            source,
        ))),
        "typed_tuple" => Some(Ty::Tuple(field_expression_types(
            project,
            db,
            file_id,
            node,
            "expressions",
            source,
        ))),
        "identifier" | "type_identifier" => project
            .target_at(file_id, node.start_byte())
            .and_then(|target| db.target_type(target).cloned()),
        "function_application" => {
            let callee = node.child_by_field_name("callee")?;
            let target = project.target_at(file_id, callee.start_byte())?;
            let Ty::Function { return_ty, .. } = db.target_type(target)? else {
                return None;
            };
            Some(return_ty.as_ref().clone())
        }
        _ => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find_map(|child| infer_expression(project, db, file_id, child, source))
        }
    }
}

fn field_expression_types(
    project: &ProjectIndex,
    db: &TypeDb,
    file_id: FileId,
    node: Node<'_>,
    field: &str,
    source: &str,
) -> Vec<Ty> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor)
        .filter(Node::is_named)
        .map(|child| infer_expression(project, db, file_id, child, source).unwrap_or(Ty::Unknown))
        .collect()
}

fn node_for_span(root: Node<'_>, span: Span) -> Option<Node<'_>> {
    let mut node = root.descendant_for_byte_range(span.start(), span.end())?;
    while let Some(parent) = node.parent() {
        if parent.start_byte() != span.start() || parent.end_byte() != span.end() {
            break;
        }
        node = parent;
    }
    Some(node)
}
