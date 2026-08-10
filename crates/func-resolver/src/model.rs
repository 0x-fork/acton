use std::path::PathBuf;
use std::sync::Arc;

pub type FileId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    #[must_use]
    pub const fn contains(self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    #[must_use]
    pub const fn contains_span(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

impl<'tree> From<tree_sitter::Node<'tree>> for Span {
    fn from(node: tree_sitter::Node<'tree>) -> Self {
        Self::new(node.start_byte(), node.end_byte())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId {
    pub file_id: FileId,
    pub local_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalId {
    pub file_id: FileId,
    pub local_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResolvedTarget {
    Symbol(SymbolId),
    Local(LocalId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclarationLocation {
    pub file_id: FileId,
    pub declaration_span: Span,
    pub name_span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileSourceKind {
    Workspace,
    Stubs,
    Stdlib,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: Option<Arc<str>>,
    pub name_span: Option<Span>,
    pub declaration_span: Span,
    pub type_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameter {
    pub name: Arc<str>,
    pub name_span: Span,
    pub declaration_span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodId {
    pub specifier_span: Span,
    pub value_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function {
        parameters: Vec<Parameter>,
        type_parameters: Vec<TypeParameter>,
        return_type_span: Option<Span>,
        body_span: Option<Span>,
        method_id: Option<MethodId>,
    },
    Constant {
        type_span: Option<Span>,
        value_span: Option<Span>,
    },
    GlobalVariable {
        type_span: Option<Span>,
    },
}

impl SymbolKind {
    #[must_use]
    pub const fn category_order(&self) -> u8 {
        match self {
            Self::Function { .. } => 0,
            Self::GlobalVariable { .. } => 1,
            Self::Constant { .. } => 2,
        }
    }

    #[must_use]
    pub const fn method_id(&self) -> Option<&MethodId> {
        match self {
            Self::Function { method_id, .. } => method_id.as_ref(),
            Self::Constant { .. } | Self::GlobalVariable { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: Arc<str>,
    pub name_span: Span,
    pub declaration_span: Span,
    pub kind: SymbolKind,
    pub doc: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub path: Arc<str>,
    pub path_span: Span,
    pub declaration_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    pub import: Import,
    pub path: PathBuf,
    pub target: Option<FileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocalDefKind {
    Parameter,
    TypeParameter,
    Variable,
    CatchParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDef {
    pub id: LocalId,
    pub owner: Option<SymbolId>,
    pub name: Arc<str>,
    pub name_span: Span,
    pub declaration_span: Span,
    pub scope_span: Span,
    pub visible_from: usize,
    pub type_span: Option<Span>,
    pub kind: LocalDefKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameUse {
    pub name: Arc<str>,
    pub span: Span,
    pub target: ResolvedTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionError {
    pub name: Arc<str>,
    pub span: Span,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileResolution {
    pub locals: Vec<LocalDef>,
    pub uses: Vec<NameUse>,
    pub unresolved: Vec<ResolutionError>,
}
