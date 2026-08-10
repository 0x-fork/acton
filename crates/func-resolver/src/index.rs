use crate::resolve::resolve_file;
use crate::{
    DeclarationLocation, FileId, FileResolution, FileSourceKind, Import, LocalDef, LocalId,
    MethodId, Parameter, ResolvedImport, ResolvedTarget, Span, Symbol, SymbolId, SymbolKind,
    TypeParameter,
};
use anyhow::Context;
use func_syntax::{AstNode, FunctionLike, HasName, HasType, Parameters, SourceFile, TopLevel};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

const STUBS_PATH: &str = "/__func_builtin__/stubs.fc";

#[derive(Debug, Clone)]
pub enum ProjectSource {
    Text(Arc<str>),
    Parsed(SourceFile),
}

pub trait ProjectSourceProvider {
    fn source(&self, path: &Path) -> anyhow::Result<Option<ProjectSource>>;
}

#[derive(Debug, Clone)]
pub struct ProjectFile {
    pub path: PathBuf,
    pub source: ProjectSource,
    pub source_kind: FileSourceKind,
}

impl ProjectFile {
    #[must_use]
    pub fn workspace(path: impl Into<PathBuf>, source: ProjectSource) -> Self {
        Self {
            path: path.into(),
            source,
            source_kind: FileSourceKind::Workspace,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileIndex {
    pub id: FileId,
    pub path: PathBuf,
    pub source_kind: FileSourceKind,
    pub imports: Vec<Import>,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    index: Arc<FileIndex>,
    source: SourceFile,
}

impl FileInfo {
    #[must_use]
    pub const fn index(&self) -> &Arc<FileIndex> {
        &self.index
    }

    #[must_use]
    pub const fn source(&self) -> &SourceFile {
        &self.source
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.index.path
    }

    #[must_use]
    pub fn symbol_at(&self, offset: usize) -> Option<&Symbol> {
        self.index
            .symbols
            .iter()
            .find(|symbol| symbol.name_span.contains(offset))
    }
}

#[derive(Debug, Default)]
pub struct ProjectIndexBuilder {
    files: Vec<ProjectFile>,
    include_stubs: bool,
}

impl ProjectIndexBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            files: Vec::new(),
            include_stubs: true,
        }
    }

    #[must_use]
    pub const fn include_stubs(mut self, include: bool) -> Self {
        self.include_stubs = include;
        self
    }

    #[must_use]
    pub fn add_file(mut self, file: ProjectFile) -> Self {
        self.files.push(file);
        self
    }

    pub fn build(self) -> anyhow::Result<ProjectIndex> {
        ProjectIndex::build(self.files, self.include_stubs)
    }

    pub fn build_with_provider(
        roots: impl IntoIterator<Item = PathBuf>,
        provider: &dyn ProjectSourceProvider,
        include_stubs: bool,
    ) -> anyhow::Result<ProjectIndex> {
        let mut files = Vec::new();
        let mut queued = BTreeSet::new();
        let mut queue = VecDeque::new();
        for root in roots {
            queue.push_back(normalize_path(&root));
        }

        while let Some(path) = queue.pop_front() {
            let path = normalize_path(&path);
            if !queued.insert(path.clone()) {
                continue;
            }
            let Some(source) = provider.source(&path)? else {
                continue;
            };
            let parsed = parse_project_source(source)?;
            for import in parsed.imports() {
                let Some(path_node) = import.path() else {
                    continue;
                };
                let import_text = path_node.text(parsed.source.as_ref());
                let import_path = unquote(import_text);
                queue.push_back(normalize_path(&resolve_include_path(&path, import_path)));
            }
            files.push(ProjectFile::workspace(path, ProjectSource::Parsed(parsed)));
        }

        ProjectIndex::build(files, include_stubs)
    }
}

#[derive(Debug, Clone)]
pub struct ProjectIndex {
    files: BTreeMap<FileId, Arc<FileInfo>>,
    path_to_file_id: BTreeMap<PathBuf, FileId>,
    imports: BTreeMap<FileId, Vec<ResolvedImport>>,
    resolutions: BTreeMap<FileId, Arc<FileResolution>>,
    stubs_file_id: Option<FileId>,
}

impl ProjectIndex {
    fn build(mut project_files: Vec<ProjectFile>, include_stubs: bool) -> anyhow::Result<Self> {
        if include_stubs {
            project_files.push(ProjectFile {
                path: PathBuf::from(STUBS_PATH),
                source: ProjectSource::Text(Arc::from(include_str!("../assets/stubs.fc"))),
                source_kind: FileSourceKind::Stubs,
            });
        }

        project_files.sort_by(|left, right| {
            left.source_kind
                .cmp(&right.source_kind)
                .then_with(|| left.path.cmp(&right.path))
        });
        project_files.dedup_by(|left, right| left.path == right.path);

        let mut files = BTreeMap::new();
        let mut path_to_file_id = BTreeMap::new();
        let mut stubs_file_id = None;

        for (index, project_file) in project_files.into_iter().enumerate() {
            let file_id = FileId::try_from(index).context("too many FunC source files")?;
            let path = normalize_path(&project_file.path);
            let source = parse_project_source(project_file.source)?;
            let index = Arc::new(index_file(
                file_id,
                path.clone(),
                project_file.source_kind,
                &source,
            ));
            if project_file.source_kind == FileSourceKind::Stubs {
                stubs_file_id = Some(file_id);
            }
            path_to_file_id.insert(path, file_id);
            files.insert(file_id, Arc::new(FileInfo { index, source }));
        }

        let imports = files
            .iter()
            .map(|(&file_id, file)| {
                let imports = file
                    .index
                    .imports
                    .iter()
                    .cloned()
                    .map(|import| {
                        let path = resolve_include_path(file.path(), &import.path);
                        let path = normalize_path(&path);
                        let target = path_to_file_id.get(&path).copied();
                        ResolvedImport {
                            import,
                            path,
                            target,
                        }
                    })
                    .collect();
                (file_id, imports)
            })
            .collect();

        let mut project = Self {
            files,
            path_to_file_id,
            imports,
            resolutions: BTreeMap::new(),
            stubs_file_id,
        };
        let resolutions = project
            .files
            .keys()
            .copied()
            .map(|file_id| (file_id, Arc::new(resolve_file(&project, file_id))))
            .collect();
        project.resolutions = resolutions;
        Ok(project)
    }

    #[must_use]
    pub const fn files(&self) -> &BTreeMap<FileId, Arc<FileInfo>> {
        &self.files
    }

    #[must_use]
    pub fn file(&self, file_id: FileId) -> Option<&Arc<FileInfo>> {
        self.files.get(&file_id)
    }

    #[must_use]
    pub fn file_by_path(&self, path: &Path) -> Option<&Arc<FileInfo>> {
        self.path_to_file_id
            .get(&normalize_path(path))
            .and_then(|file_id| self.files.get(file_id))
    }

    #[must_use]
    pub fn file_id(&self, path: &Path) -> Option<FileId> {
        self.path_to_file_id.get(&normalize_path(path)).copied()
    }

    #[must_use]
    pub fn imports(&self, file_id: FileId) -> &[ResolvedImport] {
        self.imports.get(&file_id).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn resolution(&self, file_id: FileId) -> Option<&Arc<FileResolution>> {
        self.resolutions.get(&file_id)
    }

    #[must_use]
    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.files
            .get(&id.file_id)?
            .index
            .symbols
            .get(usize::try_from(id.local_id).ok()?)
    }

    #[must_use]
    pub fn local(&self, id: LocalId) -> Option<&LocalDef> {
        self.resolution(id.file_id)?
            .locals
            .iter()
            .find(|local| local.id == id)
    }

    #[must_use]
    pub fn declaration(&self, target: ResolvedTarget) -> Option<DeclarationLocation> {
        match target {
            ResolvedTarget::Symbol(id) => {
                let symbol = self.symbol(id)?;
                Some(DeclarationLocation {
                    file_id: id.file_id,
                    declaration_span: symbol.declaration_span,
                    name_span: symbol.name_span,
                })
            }
            ResolvedTarget::Local(id) => {
                let local = self.local(id)?;
                Some(DeclarationLocation {
                    file_id: id.file_id,
                    declaration_span: local.declaration_span,
                    name_span: local.name_span,
                })
            }
        }
    }

    #[must_use]
    pub fn import_at(&self, file_id: FileId, offset: usize) -> Option<&ResolvedImport> {
        self.imports(file_id)
            .iter()
            .find(|import| import.import.path_span.contains(offset))
    }

    #[must_use]
    pub fn target_at(&self, file_id: FileId, offset: usize) -> Option<ResolvedTarget> {
        if let Some(symbol) = self.file(file_id)?.symbol_at(offset) {
            return Some(ResolvedTarget::Symbol(symbol.id));
        }
        let resolution = self.resolution(file_id)?;
        if let Some(local) = resolution
            .locals
            .iter()
            .find(|local| local.name_span.contains(offset))
        {
            return Some(ResolvedTarget::Local(local.id));
        }
        resolution
            .uses
            .iter()
            .find(|usage| usage.span.contains(offset))
            .map(|usage| usage.target)
    }

    #[must_use]
    pub fn references_to(&self, target: ResolvedTarget) -> Vec<(FileId, Span)> {
        self.resolutions
            .iter()
            .flat_map(|(&file_id, resolution)| {
                resolution
                    .uses
                    .iter()
                    .filter(move |usage| usage.target == target)
                    .map(move |usage| (file_id, usage.span))
            })
            .collect()
    }

    #[must_use]
    pub fn visible_file_ids(&self, file_id: FileId) -> Vec<FileId> {
        let mut result = Vec::new();
        let mut visited = BTreeSet::new();
        visited.insert(file_id);
        result.push(file_id);

        if let Some(stubs) = self.stubs_file_id
            && visited.insert(stubs)
        {
            result.push(stubs);
        }

        let mut queue = VecDeque::new();
        for import in self.imports(file_id) {
            if let Some(target) = import.target {
                queue.push_back(target);
            }
        }
        while let Some(next) = queue.pop_front() {
            if !visited.insert(next) {
                continue;
            }
            result.push(next);
            for import in self.imports(next) {
                if let Some(target) = import.target {
                    queue.push_back(target);
                }
            }
        }
        result
    }

    #[must_use]
    pub fn visible_symbols(&self, file_id: FileId) -> Vec<&Symbol> {
        self.visible_file_ids(file_id)
            .into_iter()
            .filter_map(|id| self.file(id))
            .flat_map(|file| {
                let mut symbols = file.index.symbols.iter().collect::<Vec<_>>();
                symbols.sort_by_key(|symbol| (symbol.kind.category_order(), symbol.id.local_id));
                symbols
            })
            .collect()
    }
}

fn parse_project_source(source: ProjectSource) -> anyhow::Result<SourceFile> {
    match source {
        ProjectSource::Text(text) => func_syntax::parse(&text),
        ProjectSource::Parsed(source) => Ok(source),
    }
}

fn index_file(
    file_id: FileId,
    path: PathBuf,
    source_kind: FileSourceKind,
    source: &SourceFile,
) -> FileIndex {
    let text = source.source.as_ref();
    let imports = source
        .imports()
        .filter_map(|import| {
            let path = import.path()?;
            Some(Import {
                path: Arc::from(unquote(path.text(text))),
                path_span: path.0.into(),
                declaration_span: import.0.into(),
            })
        })
        .collect();
    let mut symbols = Vec::new();

    for top_level in source.top_levels() {
        match top_level {
            TopLevel::Function(function) => {
                let Some(name) = function.name() else {
                    continue;
                };
                let type_parameters = function
                    .type_parameters()
                    .map(|parameters| {
                        parameters
                            .declarations()
                            .filter_map(|parameter| {
                                let name = parameter.name()?;
                                Some(TypeParameter {
                                    name: Arc::from(normalize_name(name.text(text))),
                                    name_span: name.0.into(),
                                    declaration_span: parameter.0.into(),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let parameters = function
                    .parameters()
                    .map(|parameters| index_parameters(parameters, &type_parameters, text))
                    .unwrap_or_default();
                let method_id = function.specifiers().and_then(|specifiers| {
                    specifiers.items().find_map(|specifier| {
                        let func_syntax::Specifier::MethodId(method_id) = specifier else {
                            return None;
                        };
                        Some(MethodId {
                            specifier_span: method_id.0.into(),
                            value_span: method_id.value().map(|value| value.syntax().into()),
                        })
                    })
                });
                let doc = documentation_before(function.0, text);
                push_symbol(
                    &mut symbols,
                    file_id,
                    Arc::from(normalize_name(name.text(text))),
                    name.0.into(),
                    function.0.into(),
                    SymbolKind::Function {
                        parameters,
                        type_parameters,
                        return_type_span: function.return_type().map(|node| node.syntax().into()),
                        body_span: function.body().map(|body| body.syntax().into()),
                        method_id,
                    },
                    doc,
                );
            }
            TopLevel::GlobalVars(group) => {
                for declaration in group.declarations() {
                    let Some(name) = declaration.name() else {
                        continue;
                    };
                    push_symbol(
                        &mut symbols,
                        file_id,
                        Arc::from(normalize_name(name.text(text))),
                        name.0.into(),
                        declaration.0.into(),
                        SymbolKind::GlobalVariable {
                            type_span: declaration.type_hint().map(|node| node.syntax().into()),
                        },
                        Arc::from(""),
                    );
                }
            }
            TopLevel::Constants(group) => {
                for declaration in group.declarations() {
                    let Some(name) = declaration.name() else {
                        continue;
                    };
                    push_symbol(
                        &mut symbols,
                        file_id,
                        Arc::from(normalize_name(name.text(text))),
                        name.0.into(),
                        declaration.0.into(),
                        SymbolKind::Constant {
                            type_span: declaration.type_hint().map(|node| node.syntax().into()),
                            value_span: declaration.value().map(|node| node.0.into()),
                        },
                        documentation_before(declaration.0, text),
                    );
                }
            }
            TopLevel::Import(_)
            | TopLevel::Pragma(_)
            | TopLevel::Empty(_)
            | TopLevel::Unmapped(_) => {}
        }
    }

    FileIndex {
        id: file_id,
        path,
        source_kind,
        imports,
        symbols,
    }
}

fn index_parameters(
    parameters: Parameters<'_>,
    type_parameters: &[TypeParameter],
    source: &str,
) -> Vec<Parameter> {
    let mut result = parameters
        .declarations()
        .map(|parameter| {
            let name = parameter.name();
            let type_node = parameter.type_hint();
            let inferred_name = name
                .is_none()
                .then_some(type_node)
                .flatten()
                .and_then(|node| {
                    (node.syntax().kind() == "type_identifier"
                        && !type_parameters.iter().any(|parameter| {
                            parameter.name.as_ref() == normalize_name(node.syntax().text(source))
                        }))
                    .then_some(node.syntax())
                });
            Parameter {
                name: name
                    .map(|name| Arc::from(normalize_name(name.syntax().text(source))))
                    .or_else(|| {
                        inferred_name.map(|name| Arc::from(normalize_name(name.text(source))))
                    }),
                name_span: name
                    .map(|name| name.syntax().into())
                    .or_else(|| inferred_name.map(Into::into)),
                declaration_span: parameter.0.into(),
                type_span: inferred_name
                    .is_none()
                    .then_some(type_node)
                    .flatten()
                    .map(|node| node.syntax().into()),
            }
        })
        .collect::<Vec<_>>();
    if let Parameters::Relaxed(relaxed) = parameters {
        result.extend(relaxed.names().map(|name| Parameter {
            name: Some(Arc::from(normalize_name(name.syntax().text(source)))),
            name_span: Some(name.syntax().into()),
            declaration_span: name.syntax().into(),
            type_span: None,
        }));
        result.sort_by_key(|parameter| parameter.declaration_span.start());
    }
    result
}

fn push_symbol(
    symbols: &mut Vec<Symbol>,
    file_id: FileId,
    name: Arc<str>,
    name_span: Span,
    declaration_span: Span,
    kind: SymbolKind,
    doc: Arc<str>,
) {
    let local_id = u32::try_from(symbols.len()).expect("a file cannot contain more symbols");
    symbols.push(Symbol {
        id: SymbolId { file_id, local_id },
        name,
        name_span,
        declaration_span,
        kind,
        doc,
    });
}

fn documentation_before(node: tree_sitter::Node<'_>, source: &str) -> Arc<str> {
    let mut comments = Vec::new();
    let mut current = node.prev_named_sibling();
    let mut next_row = node.start_position().row;
    while let Some(comment) = current {
        if comment.kind() != "comment" || comment.end_position().row + 1 < next_row {
            break;
        }
        let raw = comment.utf8_text(source.as_bytes()).unwrap_or("");
        if !raw.trim_start().starts_with(";;;") {
            break;
        }
        comments.push(raw.trim_start_matches(';').trim().to_owned());
        next_row = comment.start_position().row;
        current = comment.prev_named_sibling();
    }
    comments.reverse();
    Arc::from(comments.join("\n"))
}

fn resolve_include_path(file: &Path, include: &str) -> PathBuf {
    file.parent().unwrap_or_else(|| Path::new("")).join(include)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

pub(crate) fn normalize_name(value: &str) -> &str {
    value.trim_matches('`')
}
