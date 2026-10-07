//! Project-wide declaration tables, module exports, aliases, and reference
//! resolution.
//!
//! A table entry holds one symbol per meaning plus an optional alias, so a
//! same-named type and value stay separate unless their declarations merge.
//! Aliases and qualified references resolve on demand through one explicit
//! task stack; cycles become unresolved targets rather than recursion.

use super::collect::{AliasBind, AliasKind, DeclKind, Facts, Host, NameTarget, RefBase, ScopeKind};
use super::ids::{
    AliasId, DeclId, ModuleId, RefId, RequestId, ScopeId, SymbolId, TableId, TypeParameterListId,
};
use super::index::{self, Index, Site, Target, Token};
use super::instances::Instance;
use super::targets::{Meanings, Reach, Slots, each};
use super::{FileFacts, ModuleResolution, RequestKind, meaning};
use crate::Span;
use crate::references::project::{self, Program};
use crate::references::source::FileId;
use crate::references::{ProjectError, ProjectErrorKind};
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

const GLOBAL: TableId = TableId::new_const(0);

/// TypeScript symbol flags that decide which declarations may share a name.
mod flags {
    pub(super) const FUNCTION_SCOPED: u32 = 1;
    pub(super) const BLOCK_SCOPED: u32 = 1 << 1;
    pub(super) const FUNCTION: u32 = 1 << 2;
    pub(super) const CLASS: u32 = 1 << 3;
    pub(super) const INTERFACE: u32 = 1 << 4;
    pub(super) const CONST_ENUM: u32 = 1 << 5;
    pub(super) const REGULAR_ENUM: u32 = 1 << 6;
    pub(super) const VALUE_MODULE: u32 = 1 << 7;
    pub(super) const NAMESPACE_MODULE: u32 = 1 << 8;
    pub(super) const TYPE_ALIAS: u32 = 1 << 9;
    pub(super) const TYPE_PARAMETER: u32 = 1 << 10;
    pub(super) const PARAMETER: u32 = 1 << 11;
    pub(super) const EXPRESSION: u32 = 1 << 12;
    /// A source file or ambient module, which TypeScript marks `ValueModule`.
    pub(super) const MODULE: u32 = 1 << 13;
    /// A function declaration with a body.
    pub(super) const IMPLEMENTED: u32 = 1 << 14;
    pub(super) const AMBIENT_CLASS: u32 = 1 << 15;
    pub(super) const CONCRETE_CLASS: u32 = 1 << 16;
    pub(super) const ENUM: u32 = CONST_ENUM | REGULAR_ENUM;
    pub(super) const NAMESPACE: u32 = VALUE_MODULE | NAMESPACE_MODULE;
    pub(super) const VALUE: u32 =
        FUNCTION_SCOPED | BLOCK_SCOPED | FUNCTION | CLASS | ENUM | VALUE_MODULE | EXPRESSION;
    pub(super) const TYPE: u32 = CLASS | INTERFACE | ENUM | TYPE_ALIAS | TYPE_PARAMETER;
}

fn symbol_flags(kind: DeclKind) -> u32 {
    use flags::*;
    match kind {
        DeclKind::Var => FUNCTION_SCOPED,
        DeclKind::Parameter => FUNCTION_SCOPED | PARAMETER,
        DeclKind::Let => BLOCK_SCOPED,
        DeclKind::Function => FUNCTION,
        DeclKind::Class => CLASS,
        DeclKind::Interface => INTERFACE,
        DeclKind::TypeAlias => TYPE_ALIAS,
        DeclKind::Enum { constant: true } => CONST_ENUM,
        DeclKind::Enum { constant: false } => REGULAR_ENUM,
        DeclKind::Namespace {
            instance: Instance::NonInstantiated,
        } => NAMESPACE_MODULE,
        DeclKind::Namespace { .. } => VALUE_MODULE,
        DeclKind::TypeParameter => TYPE_PARAMETER,
        DeclKind::Expression => EXPRESSION,
    }
}

/// The meanings a symbol with `flags` has of its own: the one rule for a
/// symbol's meanings, which its declarations' kinds and alias expansion use.
/// A module is a value and a namespace. Contextual meanings stay with their
/// rule: any export of a shorthand ambient module has every meaning, and a
/// type-only route restricts the meanings it reaches (see `targets`).
fn meanings(flags: u32) -> u8 {
    let mut meanings = 0;
    if flags & (flags::VALUE | flags::MODULE) != 0 {
        meanings |= meaning::VALUE;
    }
    if flags & flags::TYPE != 0 {
        meanings |= meaning::TYPE;
    }
    if flags & (flags::NAMESPACE | flags::ENUM | flags::MODULE) != 0 {
        meanings |= meaning::NAMESPACE;
    }
    meanings
}

impl DeclKind {
    /// The meanings one declaration of this kind gives its symbol.
    pub(super) fn meanings(self) -> u8 {
        meanings(symbol_flags(self))
    }

    /// TypeScript's `getDeclarationSpaces`: the value, type, and namespace
    /// spaces a declaration occupies, which decide whether exported and local
    /// declarations of one name conflict (TS2395). Unlike its lookup
    /// meanings, an enum occupies no namespace space, so an exported enum and
    /// a local type-only namespace merge.
    pub(super) fn declaration_spaces(self) -> u8 {
        use meaning::{NAMESPACE, TYPE, VALUE};
        match self {
            DeclKind::Var
            | DeclKind::Parameter
            | DeclKind::Let
            | DeclKind::Function
            | DeclKind::Expression => VALUE,
            DeclKind::Class | DeclKind::Enum { .. } => VALUE | TYPE,
            DeclKind::Interface | DeclKind::TypeAlias | DeclKind::TypeParameter => TYPE,
            DeclKind::Namespace {
                instance: Instance::NonInstantiated,
            } => NAMESPACE,
            DeclKind::Namespace { .. } => NAMESPACE | VALUE,
        }
    }
}

/// Flags of existing declarations that a new declaration cannot share a name
/// with, as in TypeScript's binder.
fn excludes(kind: DeclKind) -> u32 {
    use flags::*;
    match kind {
        DeclKind::Var => VALUE & !FUNCTION_SCOPED,
        DeclKind::Parameter | DeclKind::Let | DeclKind::Expression => VALUE,
        DeclKind::Function => VALUE & !(FUNCTION | VALUE_MODULE | CLASS),
        DeclKind::Class => (VALUE | TYPE) & !(VALUE_MODULE | INTERFACE | FUNCTION),
        DeclKind::Interface => TYPE & !(INTERFACE | CLASS),
        DeclKind::Enum { constant: false } => (VALUE | TYPE) & !(REGULAR_ENUM | VALUE_MODULE),
        DeclKind::Enum { constant: true } => (VALUE | TYPE) & !CONST_ENUM,
        DeclKind::Namespace {
            instance: Instance::NonInstantiated,
        } => 0,
        DeclKind::Namespace { .. } => VALUE & !(FUNCTION | CLASS | REGULAR_ENUM | VALUE_MODULE),
        DeclKind::TypeAlias => TYPE,
        DeclKind::TypeParameter => TYPE & !TYPE_PARAMETER,
    }
}

/// Whether compatible declarations form one symbol (TypeScript declaration
/// merging) rather than separate same-named symbols of other meanings.
fn merges(existing: u32, kind: DeclKind) -> bool {
    use flags::*;
    let new = symbol_flags(kind);
    let namespace_like = FUNCTION | CLASS | ENUM | NAMESPACE;
    match kind {
        DeclKind::Var => existing & FUNCTION_SCOPED != 0,
        DeclKind::Function => existing & (FUNCTION | CLASS | NAMESPACE) != 0,
        DeclKind::Class => existing & (INTERFACE | FUNCTION | NAMESPACE) != 0,
        DeclKind::Interface => existing & (INTERFACE | CLASS) != 0,
        DeclKind::Enum { .. } => existing & (new | NAMESPACE) != 0,
        DeclKind::Namespace { .. } => existing & namespace_like != 0,
        DeclKind::TypeParameter => existing & TYPE_PARAMETER != 0,
        DeclKind::Parameter | DeclKind::Let | DeclKind::TypeAlias | DeclKind::Expression => false,
    }
}

/// One name in a table: at most one declared symbol per meaning, plus an
/// optional import or export alias. A stored symbol may since have merged
/// into another, so readers canonicalize it with `find`.
#[derive(Debug, Default)]
struct Entry {
    slots: Slots,
    alias: Option<AliasRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AliasRef {
    file: FileId,
    id: AliasId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RefRef {
    file: FileId,
    id: RefId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Task {
    Alias(AliasRef),
    Ref(RefRef),
}

/// A task that another task waits for. `fill` marks an alias consulted only
/// for meanings that same-named local declarations lack: TypeScript resolves
/// such a name to its locals without following the alias, so a cycle through
/// that edge is cut there rather than leaving every member unresolved.
#[derive(Debug, Clone, Copy)]
struct Need {
    task: Task,
    fill: bool,
}

impl Need {
    fn required(task: Task) -> Self {
        Self { task, fill: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Resolution {
    Found(Meanings),
    GlobalThis,
    NotReference(&'static str),
    Unresolved(String),
}

#[derive(Debug, Clone, Default)]
enum State {
    #[default]
    Unvisited,
    InProgress,
    Done(Resolution),
}

enum Step<T> {
    Done(T),
    Need(Need),
}

/// The result of probing one table for a name.
enum Probe {
    Found(Resolution),
    Need(Need),
    Missing,
}

#[derive(Debug, Default)]
struct Symbol {
    flags: u32,
    sites: Vec<Site>,
    /// Type parameter lists that declare this type parameter.
    lists: Vec<(FileId, TypeParameterListId)>,
    exports: Option<TableId>,
    members: Option<TableId>,
    module: Option<ModuleId>,
    enum_members: Vec<String>,
}

#[derive(Debug)]
struct Module {
    exports: TableId,
    stars: Vec<(ModuleId, bool)>,
    symbol: SymbolId,
    shorthand: bool,
    /// A declaration file or ambient module, which may describe CommonJS and
    /// so has a synthetic default export.
    declaration: bool,
}

/// What a module request denotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestTarget {
    Module(ModuleId),
    Script,
    /// A tolerated failure: names imported through the request are
    /// unresolved, and an augmentation of it binds nothing.
    Ignored,
}

/// Where a scope's names are stored.
#[derive(Debug, Clone, Copy)]
enum Storage {
    /// The global table: a script's root, or `declare global`.
    Global,
    /// The member table shared by a class or interface's declarations.
    Members(DeclId),
    /// The scope's own table, which also sees the exports of a host.
    Own(Option<Host>),
}

struct Linker<'p> {
    program: &'p Program,
    tables: Vec<HashMap<String, Entry>>,
    scope_tables: Vec<Vec<Option<TableId>>>,
    symbols: Vec<Symbol>,
    parents: Vec<SymbolId>,
    /// The symbol each declaration created or merged into. It may since have
    /// merged into another, so readers canonicalize it with `find`.
    decl_symbols: Vec<Vec<Option<SymbolId>>>,
    modules: Vec<Module>,
    file_modules: Vec<Option<ModuleId>>,
    ambient: HashMap<String, ModuleId>,
    /// Pattern ambient modules, by name, in the order they were declared.
    patterns: Vec<(String, ModuleId)>,
    /// Parallel to each file's requests.
    request_modules: Vec<Vec<RequestTarget>>,
    alias_states: Vec<Vec<State>>,
    ref_states: Vec<Vec<State>>,
    export_memo: HashMap<(ModuleId, String), Resolution>,
    conflicts: Vec<(TableId, String, FileId, Span)>,
    /// The task `force` is running, for `probe_entry` to consult `cuts`.
    running: Option<Task>,
    /// Fill edges that close a cycle: the waiting task resolves to its local
    /// declarations without the alias.
    cuts: HashSet<(Task, Task)>,
    /// The waiting tasks of `cuts`, whose lookups are specific to them.
    cut_tasks: HashSet<Task>,
}

type Result<T> = std::result::Result<T, ProjectError>;

pub(super) fn link(program: &Program) -> Result<Index> {
    let mut linker = Linker::new(program);
    linker.check_mixed_exports()?;
    linker.create_modules();
    linker.resolve_requests()?;
    linker.collect_stars();
    linker.declare_all(false)?;
    linker.declare_all(true)?;
    linker.check_hoisting()?;
    linker.check_merges()?;
    let index = linker.resolve_and_index();
    linker.check_conflicts()?;
    Ok(index)
}

impl<'p> Linker<'p> {
    fn new(program: &'p Program) -> Self {
        let files = &program.files;
        Self {
            program,
            tables: vec![HashMap::new()],
            scope_tables: files
                .iter()
                .map(|f| vec![None; f.facts.facts.scopes.len()])
                .collect(),
            symbols: Vec::new(),
            parents: Vec::new(),
            decl_symbols: files
                .iter()
                .map(|f| vec![None; f.facts.facts.decls.len()])
                .collect(),
            modules: Vec::new(),
            file_modules: vec![None; files.len()],
            ambient: HashMap::new(),
            patterns: Vec::new(),
            request_modules: Vec::new(),
            alias_states: files
                .iter()
                .map(|f| vec![State::Unvisited; f.facts.facts.aliases.len()])
                .collect(),
            ref_states: files
                .iter()
                .map(|f| vec![State::Unvisited; f.facts.facts.refs.len()])
                .collect(),
            export_memo: HashMap::new(),
            conflicts: Vec::new(),
            running: None,
            cuts: HashSet::new(),
            cut_tasks: HashSet::new(),
        }
    }

    fn file_ids(&self) -> impl Iterator<Item = FileId> + use<'p> {
        self.program.sources.ids()
    }
    fn file(&self, file: FileId) -> &'p FileFacts {
        &self.program.files[file.index()].facts
    }
    fn facts(&self, file: FileId) -> &'p Facts {
        &self.program.files[file.index()].facts.facts
    }
    fn search(&self, file: FileId) -> bool {
        self.program.files[file.index()].search
    }
    fn checks_bindings(&self, file: FileId) -> bool {
        self.program.files[file.index()]
            .diagnostics
            .checks_bindings()
    }
    fn error(
        &self,
        kind: ProjectErrorKind,
        file: FileId,
        span: Span,
        message: String,
    ) -> ProjectError {
        let source = self.program.sources.get(file);
        ProjectError {
            kind,
            message,
            path: Some(source.path.clone()),
            position: Some(source.text.position(span.start)),
        }
    }

    fn table(&mut self) -> TableId {
        self.tables.push(HashMap::new());
        TableId::new(self.tables.len() - 1)
    }

    fn new_symbol(&mut self, flags: u32, site: Option<Site>) -> SymbolId {
        let id = SymbolId::new(self.symbols.len());
        self.symbols.push(Symbol {
            flags,
            sites: site.into_iter().collect(),
            ..Symbol::default()
        });
        self.parents.push(id);
        id
    }

    /// The canonical symbol after declaration merges.
    fn find(&self, mut id: SymbolId) -> SymbolId {
        while self.parents[id.index()] != id {
            id = self.parents[id.index()];
        }
        id
    }
    fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[self.find(id).index()]
    }

    fn new_module(&mut self, site: Site, shorthand: bool, declaration: bool) -> ModuleId {
        let exports = self.table();
        let symbol = self.new_symbol(flags::MODULE, Some(site));
        let id = ModuleId::new(self.modules.len());
        self.symbols[symbol.index()].module = Some(id);
        self.modules.push(Module {
            exports,
            stars: Vec::new(),
            symbol,
            shorthand,
            declaration,
        });
        id
    }

    fn create_modules(&mut self) {
        for file in self.file_ids() {
            if self.file(file).module {
                let site = Site {
                    file,
                    span: Span { start: 0, end: 0 },
                    name: self.program.sources.get(file).display.clone(),
                };
                let declaration = self.file(file).declaration;
                self.file_modules[file.index()] = Some(self.new_module(site, false, declaration));
            }
            for ambient in &self.facts(file).ambient_modules {
                let site = Site {
                    file,
                    span: ambient.span,
                    name: ambient.name.clone(),
                };
                match self.ambient.get(&ambient.name) {
                    Some(&module) => {
                        let symbol = self.modules[module.index()].symbol;
                        self.symbols[symbol.index()].sites.push(site);
                        self.modules[module.index()].shorthand &= ambient.shorthand;
                    }
                    None => {
                        let module = self.new_module(site, ambient.shorthand, true);
                        self.ambient.insert(ambient.name.clone(), module);
                        if super::super::pattern::parse(&ambient.name).is_some() {
                            self.patterns.push((ambient.name.clone(), module));
                        }
                    }
                }
            }
        }
    }

    fn resolve_requests(&mut self) -> Result<()> {
        for file in self.file_ids() {
            let program_file = &self.program.files[file.index()];
            let mut modules = Vec::with_capacity(program_file.facts.requests.len());
            for (index, request) in program_file.facts.requests.iter().enumerate() {
                let resolution = &program_file.modules[index];
                let relative = project::is_relative_name(&request.specifier);
                let ambient = (!relative)
                    .then(|| self.ambient.get(&request.specifier).copied())
                    .flatten();
                let span = request.span;
                let mut target = match (ambient, resolution) {
                    (Some(module), _) => RequestTarget::Module(module),
                    (None, ModuleResolution::File(target)) => {
                        match self.file_modules[target.index()] {
                            Some(module) => RequestTarget::Module(module),
                            None => RequestTarget::Script,
                        }
                    }
                    (None, ModuleResolution::Unchecked) => RequestTarget::Ignored,
                    (None, ModuleResolution::Missing) => {
                        // Pattern ambient modules also match relative names.
                        match self.pattern(&request.specifier) {
                            Some(module) => RequestTarget::Module(module),
                            // TypeScript reports no errors in declaration
                            // files under skipLibCheck, and a comment
                            // directive may suppress one; only names imported
                            // through the request stay unresolved.
                            None if program_file.diagnostics.tolerates_missing_input(
                                &self.program.sources.get(file).text,
                                span.start,
                            ) =>
                            {
                                RequestTarget::Ignored
                            }
                            None => {
                                return Err(self.error(
                                    ProjectErrorKind::UnresolvedImport,
                                    file,
                                    span,
                                    format!("cannot resolve module \"{}\"", request.specifier),
                                ));
                            }
                        }
                    }
                    (None, ModuleResolution::Unsupported(_))
                        if program_file.diagnostics.tolerates_unsupported_input() =>
                    {
                        RequestTarget::Ignored
                    }
                    (None, ModuleResolution::Unsupported(reason)) => {
                        return Err(self.error(
                            ProjectErrorKind::Unsupported,
                            file,
                            span,
                            format!("module \"{}\": {reason}", request.specifier),
                        ));
                    }
                };
                if request.kind == RequestKind::Augmentation {
                    match target {
                        RequestTarget::Module(module) => {
                            let symbol = self.modules[module.index()].symbol;
                            let site = Site {
                                file,
                                span,
                                name: request.specifier.clone(),
                            };
                            self.symbols[symbol.index()].sites.push(site);
                        }
                        // Like an augmentation of a tolerated missing module,
                        // a dependency's augmentation of a script binds
                        // nothing.
                        RequestTarget::Script if !self.checks_bindings(file) => {
                            target = RequestTarget::Ignored;
                        }
                        RequestTarget::Script => {
                            return Err(self.error(
                                ProjectErrorKind::UnresolvedImport,
                                file,
                                span,
                                format!(
                                    "cannot augment \"{}\", which is not a module",
                                    request.specifier
                                ),
                            ));
                        }
                        RequestTarget::Ignored => {}
                    }
                }
                modules.push(target);
            }
            self.request_modules.push(modules);
        }
        Ok(())
    }

    /// The module a request names, or why names imported through it are
    /// unresolved.
    fn request_module(
        &self,
        file: FileId,
        request: RequestId,
    ) -> std::result::Result<ModuleId, &'static str> {
        match self.request_modules[file.index()][request.index()] {
            RequestTarget::Module(module) => Ok(module),
            RequestTarget::Script => Err("imported file is not a module"),
            RequestTarget::Ignored => Err("module is not resolved"),
        }
    }

    /// The best-matching pattern ambient module, as TypeScript's
    /// `resolveExternalModule` chooses it.
    fn pattern(&self, specifier: &str) -> Option<ModuleId> {
        let patterns = self
            .patterns
            .iter()
            .map(|(name, module)| (name.as_str(), *module));
        super::super::pattern::best_match(patterns, specifier).map(|(module, _)| module)
    }

    fn host_module(&self, file: FileId, host: Host) -> Option<ModuleId> {
        match host {
            Host::File => self.file_modules[file.index()],
            Host::AmbientModule(index) => {
                let name = &self.facts(file).ambient_modules[index.index()].name;
                self.ambient.get(name).copied()
            }
            Host::Augmentation(request) => self.request_module(file, request).ok(),
            Host::Namespace(_) => None,
        }
    }

    fn collect_stars(&mut self) {
        for file in self.file_ids() {
            for star in &self.facts(file).stars {
                let (Some(host), Ok(target)) = (
                    self.host_module(file, star.host),
                    self.request_module(file, star.request),
                ) else {
                    continue;
                };
                self.modules[host.index()]
                    .stars
                    .push((target, star.type_only));
            }
        }
    }

    /// Where a scope stores its names. Every scope kind is decided here, for
    /// both declaration and lookup.
    fn storage(&self, file: FileId, scope: ScopeId) -> Storage {
        match self.facts(file).scopes[scope.index()].kind {
            ScopeKind::Root if !self.file(file).module => Storage::Global,
            ScopeKind::Global => Storage::Global,
            ScopeKind::Members { decl } => Storage::Members(decl),
            ScopeKind::Namespace { decl } => Storage::Own(Some(Host::Namespace(decl))),
            ScopeKind::AmbientModule { module } => Storage::Own(Some(Host::AmbientModule(module))),
            ScopeKind::Augmentation { request } => Storage::Own(Some(Host::Augmentation(request))),
            ScopeKind::Root
            | ScopeKind::Block
            | ScopeKind::Catch
            | ScopeKind::Signature { .. }
            | ScopeKind::Body
            | ScopeKind::ExpressionName
            | ScopeKind::TypeParameters
            | ScopeKind::Infer
            | ScopeKind::InferConstraint { .. }
            | ScopeKind::Enum { .. } => Storage::Own(None),
        }
    }

    /// The table a scope declares into, created on first use.
    fn scope_table(&mut self, file: FileId, scope: ScopeId) -> TableId {
        match self.storage(file, scope) {
            Storage::Global => GLOBAL,
            Storage::Members(decl) => {
                let symbol = self.decl_symbol(file, decl);
                self.member_table(symbol)
            }
            Storage::Own(_) => {
                if let Some(table) = self.scope_tables[file.index()][scope.index()] {
                    return table;
                }
                let table = self.table();
                self.scope_tables[file.index()][scope.index()] = Some(table);
                table
            }
        }
    }

    fn decl_symbol(&self, file: FileId, decl: DeclId) -> SymbolId {
        self.find(self.decl_symbols[file.index()][decl.index()].expect("declared before use"))
    }

    fn member_table(&mut self, symbol: SymbolId) -> TableId {
        let symbol = self.find(symbol);
        if let Some(table) = self.symbols[symbol.index()].members {
            return table;
        }
        let table = self.table();
        self.symbols[symbol.index()].members = Some(table);
        table
    }

    fn export_table(&mut self, symbol: SymbolId) -> TableId {
        let symbol = self.find(symbol);
        if let Some(table) = self.symbols[symbol.index()].exports {
            return table;
        }
        let table = self.table();
        self.symbols[symbol.index()].exports = Some(table);
        table
    }

    /// The export table of a host, created on first use. Only an ignored
    /// module augmentation has none, and its declarations bind nothing:
    /// exporting files and ambient modules always have modules, and
    /// `resolve_requests` either rejects an augmentation of anything else or
    /// ignores it.
    fn host_table(&mut self, file: FileId, host: Host) -> Option<TableId> {
        let table = match host {
            Host::Namespace(decl) => {
                let symbol = self.decl_symbol(file, decl);
                Some(self.export_table(symbol))
            }
            _ => self
                .host_module(file, host)
                .map(|module| self.modules[module.index()].exports),
        };
        debug_assert!(
            table.is_some()
                || matches!(host, Host::Augmentation(request)
                    if self.request_modules[file.index()][request.index()] == RequestTarget::Ignored),
            "only an ignored augmentation lacks an export table"
        );
        table
    }

    /// The export table of a host whose members a scope sees, if it exists.
    fn visible_exports(&self, file: FileId, host: Host) -> Option<TableId> {
        match host {
            Host::Namespace(decl) => self.decl_symbols[file.index()][decl.index()]
                .and_then(|symbol| self.symbol(symbol).exports),
            _ => self
                .host_module(file, host)
                .map(|module| self.modules[module.index()].exports),
        }
    }

    fn declare_all(&mut self, deferred: bool) -> Result<()> {
        for file in self.file_ids() {
            let facts = self.facts(file);
            for (index, decl) in facts.decls.iter().enumerate() {
                if decl.deferred == deferred {
                    self.declare(file, DeclId::new(index))?;
                }
            }
            for (index, alias) in facts.aliases.iter().enumerate() {
                if alias.deferred == deferred {
                    self.bind_alias(file, AliasId::new(index))?;
                }
            }
        }
        Ok(())
    }

    fn declare(&mut self, file: FileId, index: DeclId) -> Result<()> {
        let decl = &self.facts(file).decls[index.index()];
        let site = Site {
            file,
            span: decl.anchor,
            name: decl.name.clone(),
        };
        let mut flags = symbol_flags(decl.kind);
        if decl.kind == DeclKind::Function && decl.implementation {
            flags |= flags::IMPLEMENTED;
        }
        if decl.kind == DeclKind::Class {
            flags |= if decl.ambient {
                flags::AMBIENT_CLASS
            } else {
                flags::CONCRETE_CLASS
            };
        }
        let declaring = Declaring {
            kind: decl.kind,
            flags,
            list: decl.list.map(|list| (file, list)),
            site,
        };
        let symbol = match (decl.lexical, &decl.export) {
            (Some(scope), _) => {
                let table = self.scope_table(file, scope);
                let signature = self.signature_table(file, scope, decl.kind);
                self.declare_in(table, signature, &decl.name, declaring)?
            }
            (None, Some((host, name))) => match self.host_table(file, *host) {
                Some(table) => self.declare_in(table, None, name, declaring)?,
                // An ignored augmentation binds nothing.
                None => self.new_symbol(declaring.flags, Some(declaring.site)),
            },
            (None, None) => unreachable!("collect binds a declaration lexically or as an export"),
        };
        self.decl_symbols[file.index()][index.index()] = Some(symbol);
        if !decl.members.is_empty() {
            let canonical = self.find(symbol);
            self.symbols[canonical.index()]
                .enum_members
                .extend(decl.members.iter().cloned());
        }
        if let (Some(_), Some((host, name))) = (decl.lexical, &decl.export) {
            if let Some(table) = self.host_table(file, *host) {
                self.export_symbol(table, name, symbol, decl.kind.meanings(), file, decl.anchor)?;
            }
        }
        Ok(())
    }

    /// Declarations of one name in one container must be all exported or
    /// all local (TypeScript's TS2395), where bindings are checked.
    fn check_mixed_exports(&self) -> Result<()> {
        for file in self.file_ids().filter(|&file| self.checks_bindings(file)) {
            let facts = self.facts(file);
            if let Some(&name) = facts.mixed_exports.first() {
                let name = &facts.names[name.index()];
                return Err(self.error(
                    ProjectErrorKind::Binding,
                    file,
                    name.span,
                    format!(
                        "declarations of one name must be all exported or all local: `{}`",
                        name.text
                    ),
                ));
            }
        }
        Ok(())
    }

    /// A `var` hoisted out of a block may not share its name with that
    /// block's `let`, `const`, or `using` (TypeScript's TS2481).
    fn check_hoisting(&self) -> Result<()> {
        for file in self.file_ids() {
            let facts = self.facts(file);
            for decl in facts.decls.iter().filter(|decl| !decl.hoisted.is_empty()) {
                for &scope in &decl.hoisted {
                    if facts.scopes[scope.index()].kind == ScopeKind::Catch {
                        continue;
                    }
                    let blocked = self.scope_tables[file.index()][scope.index()]
                        .and_then(|table| self.tables[table.index()].get(&decl.name))
                        .and_then(|entry| entry.slots.value())
                        .is_some_and(|symbol| self.symbol(symbol).flags & flags::BLOCK_SCOPED != 0);
                    if blocked && self.checks_bindings(file) {
                        return Err(self.error(
                            ProjectErrorKind::Binding,
                            file,
                            decl.anchor,
                            format!(
                                "`var {}` is hoisted past a block-scoped declaration of the same name",
                                decl.name
                            ),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Merged declarations TypeScript's checker rejects: an instantiated
    /// namespace before, or in another file from, the class or function it
    /// merges with (TS2433, TS2434), and classes and interfaces whose type
    /// parameters differ (TS2428). Only declarations covered by the binding diagnostic policy are checked.
    fn check_merges(&self) -> Result<()> {
        let mut declarations: HashMap<SymbolId, Vec<(FileId, DeclId)>> = HashMap::new();
        for file in self.file_ids() {
            for (index, symbol) in self.decl_symbols[file.index()].iter().enumerate() {
                if let Some(symbol) = symbol {
                    declarations
                        .entry(self.find(*symbol))
                        .or_default()
                        .push((file, DeclId::new(index)));
                }
            }
        }
        let decl = |(file, index): (FileId, DeclId)| &self.facts(file).decls[index.index()];
        for list in declarations.values().filter(|list| list.len() > 1) {
            let anchor = list.iter().copied().find(|&(file, index)| {
                let decl = decl((file, index));
                !decl.ambient && matches!(decl.kind, DeclKind::Class | DeclKind::Function)
            });
            for &(file, index) in list {
                let namespace = decl((file, index));
                // TypeScript's `isInstantiatedModule` with
                // `shouldPreserveConstEnums`: a namespace of const enums only
                // counts when the configuration keeps const enums.
                let instantiated = match namespace.kind {
                    DeclKind::Namespace { instance } => {
                        instance == Instance::Instantiated
                            || instance == Instance::ConstEnumOnly
                                && self.program.preserve_const_enums
                    }
                    _ => false,
                };
                if !instantiated || namespace.ambient || !self.checks_bindings(file) {
                    continue;
                }
                let Some((anchor_file, anchor_index)) = anchor else {
                    continue;
                };
                let target = decl((anchor_file, anchor_index));
                let message = if anchor_file != file {
                    "a namespace cannot be in another file from the class or function it merges with"
                } else if namespace.anchor.start < target.anchor.start {
                    "a namespace cannot precede the class or function it merges with"
                } else {
                    continue;
                };
                return Err(self.error(
                    ProjectErrorKind::Binding,
                    file,
                    namespace.anchor,
                    format!("{message}: `{}`", namespace.name),
                ));
            }
            let generic: Vec<_> = list
                .iter()
                .copied()
                .filter_map(|key| Some((key, decl(key).type_parameters.as_deref()?)))
                .collect();
            if !identical_type_parameters(generic.iter().map(|&(_, list)| list)) {
                // TypeScript reports every declaration.
                if let Some(&((file, index), _)) = generic
                    .iter()
                    .find(|((file, _), _)| self.checks_bindings(*file))
                {
                    let declaration = decl((file, index));
                    return Err(self.error(
                        ProjectErrorKind::Binding,
                        file,
                        declaration.anchor,
                        format!(
                            "all declarations of `{}` must have identical type parameters",
                            declaration.name
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    /// The parameter table a body-level `var` may redeclare into. Collect
    /// records a function's parameters before its body's declarations, so
    /// the table exists by the time a body declaration asks for it.
    fn signature_table(&self, file: FileId, scope: ScopeId, kind: DeclKind) -> Option<TableId> {
        if kind.meanings() & meaning::VALUE == 0 {
            return None;
        }
        let scopes = &self.facts(file).scopes;
        let ScopeKind::Body = scopes[scope.index()].kind else {
            return None;
        };
        let parent = scopes[scope.index()].parent?;
        matches!(scopes[parent.index()].kind, ScopeKind::Signature { .. })
            .then(|| self.scope_tables[file.index()][parent.index()])
            .flatten()
    }

    /// Declare into `table`, merging with compatible same-named declarations.
    fn declare_in(
        &mut self,
        table: TableId,
        parameters: Option<TableId>,
        name: &str,
        declaring: Declaring,
    ) -> Result<SymbolId> {
        let Declaring {
            kind,
            flags: declared,
            list,
            site,
        } = declaring;
        let entry = self.tables[table.index()].get(name);
        let has_alias = entry.is_some_and(|entry| entry.alias.is_some());
        let mut slots = entry.map(|entry| entry.slots).unwrap_or_default();
        if slots.value().is_none() {
            if let Some(parameter) = parameters
                .and_then(|parameters| self.tables[parameters.index()].get(name))
                .and_then(|entry| entry.slots.value())
            {
                slots.set(meaning::VALUE, parameter);
            }
        }
        let mut existing: SmallVec<[SymbolId; 3]> = SmallVec::new();
        for symbol in slots.symbols() {
            let symbol = self.find(symbol);
            if !existing.contains(&symbol) {
                existing.push(symbol);
            }
        }
        let mut merge_with: SmallVec<[SymbolId; 3]> = SmallVec::new();
        for &symbol in &existing {
            let flags = self.symbols[symbol.index()].flags;
            let repeated =
                list.is_some_and(|list| self.symbols[symbol.index()].lists.contains(&list));
            if flags & excludes(kind) != 0 || invalid_merge(flags, declared, repeated) {
                if !self.lenient(symbol, site.file) {
                    return Err(self.error(
                        ProjectErrorKind::Binding,
                        site.file,
                        site.span,
                        format!("duplicate declaration of `{name}`"),
                    ));
                }
                merge_with.push(symbol);
            } else if merges(flags, kind) {
                merge_with.push(symbol);
            }
        }
        let symbol = match merge_with.split_first() {
            None => self.new_symbol(declared, Some(site.clone())),
            Some((&first, rest)) => {
                for &other in rest {
                    self.merge_symbols(first, other)?;
                }
                let first = self.find(first);
                let target = &mut self.symbols[first.index()];
                target.flags |= declared;
                target.sites.push(site.clone());
                first
            }
        };
        self.symbols[symbol.index()].lists.extend(list);
        let meanings = self.meanings_of(symbol);
        // The symbol takes each of its meanings that is free or held by a
        // symbol now merged into it.
        let held = self.tables[table.index()]
            .get(name)
            .map(|entry| entry.slots.map(|other| self.find(other)))
            .unwrap_or_default();
        let entry = self.tables[table.index()]
            .entry(name.to_string())
            .or_default();
        for meaning in each(meanings) {
            if held.get(meaning).is_none_or(|other| other == symbol) {
                entry.slots.set(meaning, symbol);
            }
        }
        if has_alias {
            self.conflicts
                .push((table, name.to_string(), site.file, site.span));
        }
        Ok(symbol)
    }

    /// Declarations reached only through packages are merged as TypeScript's
    /// binder does when library checking is skipped.
    fn lenient(&self, symbol: SymbolId, file: FileId) -> bool {
        !self.checks_bindings(file)
            && self
                .symbol(symbol)
                .sites
                .iter()
                .all(|site| !self.checks_bindings(site.file))
    }

    /// The meanings of a symbol and every declaration merged into it.
    fn meanings_of(&self, symbol: SymbolId) -> u8 {
        meanings(self.symbol(symbol).flags)
    }

    /// Merge `other` into `target`, combining namespace exports and shared
    /// type parameters.
    fn merge_symbols(&mut self, target: SymbolId, other: SymbolId) -> Result<()> {
        let mut work = vec![(target, other)];
        while let Some((target, other)) = work.pop() {
            let (target, other) = (self.find(target), self.find(other));
            if target == other {
                continue;
            }
            self.parents[other.index()] = target;
            let moved = std::mem::take(&mut self.symbols[other.index()]);
            let symbol = &mut self.symbols[target.index()];
            symbol.flags |= moved.flags;
            symbol.sites.extend(moved.sites);
            symbol.lists.extend(moved.lists);
            symbol.enum_members.extend(moved.enum_members);
            if let Some(from) = moved.exports {
                let into = self.export_table(target);
                self.merge_tables(from, into, &mut work)?;
            }
            if let Some(from) = moved.members {
                let into = self.member_table(target);
                self.merge_tables(from, into, &mut work)?;
            }
        }
        Ok(())
    }

    /// Move the entries of `from` into `into`. Symbols that meet in one
    /// meaning are queued to merge in turn.
    fn merge_tables(
        &mut self,
        from: TableId,
        into: TableId,
        work: &mut Vec<(SymbolId, SymbolId)>,
    ) -> Result<()> {
        let entries: Vec<(String, Entry)> = self.tables[from.index()].drain().collect();
        for (name, entry) in entries {
            let destination = self.tables[into.index()].entry(name.clone()).or_default();
            for meaning in each(meaning::ALL) {
                match (destination.slots.get(meaning), entry.slots.get(meaning)) {
                    (Some(a), Some(b)) => work.push((a, b)),
                    (None, Some(b)) => destination.slots.set(meaning, b),
                    _ => {}
                }
            }
            let Some(alias) = entry.alias else {
                continue;
            };
            let Some(kept) = destination.alias else {
                destination.alias = Some(alias);
                continue;
            };
            // TypeScript reports both declarations; the first export stays
            // where neither file's bindings are checked.
            if let Some(checked) = [kept, alias]
                .into_iter()
                .find(|alias| self.checks_bindings(alias.file))
            {
                let span = self.facts(checked.file).aliases[checked.id.index()].span;
                return Err(self.error(
                    ProjectErrorKind::Binding,
                    checked.file,
                    span,
                    format!("merged declarations both export `{name}`"),
                ));
            }
        }
        Ok(())
    }

    /// Make an already declared symbol visible under an export name.
    fn export_symbol(
        &mut self,
        table: TableId,
        name: &str,
        symbol: SymbolId,
        meanings: u8,
        file: FileId,
        span: Span,
    ) -> Result<()> {
        let symbol = self.find(symbol);
        let mut merge = Vec::new();
        let entry = self.tables[table.index()]
            .entry(name.to_string())
            .or_default();
        for meaning in each(meanings) {
            match entry.slots.get(meaning) {
                None => entry.slots.set(meaning, symbol),
                Some(other) => merge.push(other),
            }
        }
        let has_alias = entry.alias.is_some();
        for other in merge {
            let other = self.find(other);
            if other != symbol && !self.lenient(other, file) {
                return Err(self.error(
                    ProjectErrorKind::Binding,
                    file,
                    span,
                    format!("duplicate export `{name}`"),
                ));
            }
        }
        if has_alias {
            self.conflicts.push((table, name.to_string(), file, span));
        }
        Ok(())
    }

    fn bind_alias(&mut self, file: FileId, index: AliasId) -> Result<()> {
        let alias = &self.facts(file).aliases[index.index()];
        let reference = AliasRef { file, id: index };
        let span = alias.span;
        let bind = |linker: &mut Self, table: Option<TableId>, name: &str| -> Result<()> {
            // An ignored augmentation binds nothing.
            let Some(table) = table else {
                return Ok(());
            };
            let entry = linker.tables[table.index()]
                .entry(name.to_string())
                .or_default();
            if entry.alias.is_some() {
                if linker.checks_bindings(file) {
                    return Err(linker.error(
                        ProjectErrorKind::Binding,
                        file,
                        span,
                        format!("duplicate import or export `{name}`"),
                    ));
                }
                return Ok(());
            }
            entry.alias = Some(reference);
            if !entry.slots.is_empty() {
                linker.conflicts.push((table, name.to_string(), file, span));
            }
            Ok(())
        };
        match &alias.bind {
            AliasBind::Lexical { scope, name } => {
                let table = self.scope_table(file, *scope);
                bind(self, Some(table), name)
            }
            AliasBind::Export { host, name } => {
                let table = self.host_table(file, *host);
                bind(self, table, name)
            }
            AliasBind::Both { scope, host, name } => {
                let table = self.scope_table(file, *scope);
                bind(self, Some(table), name)?;
                let table = self.host_table(file, *host);
                bind(self, table, name)
            }
            AliasBind::Global { name } => bind(self, Some(GLOBAL), name),
        }
    }

    // Resolution

    fn state(&self, task: Task) -> &State {
        match task {
            Task::Alias(a) => &self.alias_states[a.file.index()][a.id.index()],
            Task::Ref(r) => &self.ref_states[r.file.index()][r.id.index()],
        }
    }
    fn set_state(&mut self, task: Task, state: State) {
        match task {
            Task::Alias(a) => self.alias_states[a.file.index()][a.id.index()] = state,
            Task::Ref(r) => self.ref_states[r.file.index()][r.id.index()] = state,
        }
    }

    /// Resolve `task` and everything it needs with an explicit stack. Each
    /// task on the stack needs the one above it, so a task that needs one
    /// still in progress closes a cycle through every task between them. All
    /// of them are unresolved, whichever task resolution happened to start
    /// from; deciding only the task that detects the cycle would let the
    /// others' results depend on file order.
    fn force(&mut self, task: Task) -> Resolution {
        // Each frame records whether a fill edge from the frame below pushed it.
        let mut stack = vec![(task, false)];
        while let Some(&(top, _)) = stack.last() {
            if let State::Done(_) = self.state(top) {
                stack.pop();
                continue;
            }
            self.set_state(top, State::InProgress);
            self.running = Some(top);
            let step = match top {
                Task::Alias(alias) => self.try_alias(alias),
                Task::Ref(reference) => self.try_ref(reference),
            };
            self.running = None;
            match step {
                Step::Done(resolution) => {
                    self.set_state(top, State::Done(resolution));
                    stack.pop();
                }
                Step::Need(need) if matches!(self.state(need.task), State::InProgress) => {
                    let start = stack
                        .iter()
                        .position(|&(task, _)| task == need.task)
                        .expect("a task in progress is on the stack");
                    // The cycle's edges: frame to frame, then `top` back to `start`.
                    let mut fills = (start + 1..stack.len())
                        .filter(|&index| stack[index].1)
                        .map(|index| (index - 1, stack[index].0))
                        .collect::<Vec<_>>();
                    if need.fill {
                        fills.push((stack.len() - 1, need.task));
                    }
                    let Some(&(lowest, _)) = fills.first() else {
                        for (member, _) in stack.drain(start..) {
                            self.set_state(
                                member,
                                State::Done(Resolution::Unresolved("circular alias".into())),
                            );
                        }
                        continue;
                    };
                    for &(tail, head) in &fills {
                        self.cuts.insert((stack[tail].0, head));
                        self.cut_tasks.insert(stack[tail].0);
                    }
                    // Rerun from the lowest cut tail; tasks above it resume
                    // when it needs them again.
                    for (member, _) in stack.drain(lowest + 1..) {
                        self.set_state(member, State::Unvisited);
                    }
                }
                Step::Need(need) => stack.push((need.task, need.fill)),
            }
        }
        match self.state(task) {
            State::Done(resolution) => resolution.clone(),
            _ => unreachable!("forced task is resolved"),
        }
    }

    fn done(&self, task: Task) -> Option<&Resolution> {
        match self.state(task) {
            State::Done(resolution) => Some(resolution),
            _ => None,
        }
    }

    /// Probe one table for `name` in `mask`. A mask of every meaning returns
    /// the first entry's meanings, as for export specifiers.
    fn probe(&self, table: TableId, name: &str, mask: u8, type_context: bool) -> Probe {
        self.probe_entry(table, name, mask, type_context, false)
    }

    /// `probe` for a lexical lookup, to which export specifiers are invisible
    /// even in the export table of an enclosing namespace.
    fn probe_lexical(&self, table: TableId, name: &str, mask: u8, type_context: bool) -> Probe {
        self.probe_entry(table, name, mask, type_context, true)
    }

    fn probe_entry(
        &self,
        table: TableId,
        name: &str,
        mask: u8,
        type_context: bool,
        lexical: bool,
    ) -> Probe {
        let Some(entry) = self.tables[table.index()].get(name) else {
            return Probe::Missing;
        };
        let mut found =
            Meanings::new(entry.slots.map(|symbol| self.find(symbol))).select(mask, type_context);
        let alias = entry.alias.filter(|alias| {
            !lexical || !self.facts(alias.file).aliases[alias.id.index()].export_only
        });
        if let Some(alias) = alias {
            let task = Task::Alias(alias);
            let cut = !found.is_empty()
                && self
                    .running
                    .is_some_and(|running| self.cuts.contains(&(running, task)));
            if found.needs(mask) && !cut {
                match self.done(task) {
                    None => {
                        return Probe::Need(Need {
                            task,
                            fill: !found.is_empty(),
                        });
                    }
                    Some(Resolution::Found(targets)) => {
                        found.fill_missing(*targets, mask, type_context)
                    }
                    Some(other) if found.is_empty() => return Probe::Found(other.clone()),
                    Some(_) => {}
                }
            }
        }
        if found.is_empty() {
            Probe::Missing
        } else {
            Probe::Found(Resolution::Found(found))
        }
    }

    /// Tables consulted for a scope: its locals, then any export table whose
    /// members are visible inside it.
    fn lookup_tables(&self, file: FileId, scope: ScopeId) -> SmallVec<[TableId; 2]> {
        let mut tables = SmallVec::new();
        match self.storage(file, scope) {
            Storage::Global => tables.push(GLOBAL),
            Storage::Members(decl) => {
                if let Some(symbol) = self.decl_symbols[file.index()][decl.index()] {
                    tables.extend(self.symbol(symbol).members);
                }
            }
            Storage::Own(host) => {
                tables.extend(self.scope_tables[file.index()][scope.index()]);
                tables.extend(host.and_then(|host| self.visible_exports(file, host)));
            }
        }
        tables
    }

    /// Lexical lookup from `scope` outward to the global table.
    fn lookup(
        &self,
        file: FileId,
        scope: ScopeId,
        name: &str,
        mask: u8,
        type_context: bool,
    ) -> Step<Resolution> {
        let facts = self.facts(file);
        let mut current = Some(scope);
        let mut global = false;
        while let Some(index) = current {
            let scope = &facts.scopes[index.index()];
            // An infer constraint sees only its own declaration's name.
            if let ScopeKind::InferConstraint { decl } = scope.kind {
                let own = &facts.decls[decl.index()];
                if own.name == name && mask & meaning::TYPE != 0 {
                    if let Some(symbol) = self.decl_symbols[file.index()][decl.index()] {
                        return Step::Done(Resolution::Found(Meanings::new(Slots::only(
                            meaning::TYPE,
                            self.find(symbol),
                        ))));
                    }
                }
            }
            // Enum initializers see the enum's members, which are not targets.
            if let ScopeKind::Enum { decl } = scope.kind {
                let member = self.decl_symbols[file.index()][decl.index()].is_some_and(|symbol| {
                    self.symbol(symbol).enum_members.iter().any(|m| m == name)
                });
                if member {
                    return Step::Done(Resolution::NotReference("enum member"));
                }
            }
            for table in self.lookup_tables(file, index) {
                global |= table == GLOBAL;
                match self.probe_lexical(table, name, mask, type_context) {
                    Probe::Found(resolution) => return Step::Done(resolution),
                    Probe::Need(task) => return Step::Need(task),
                    Probe::Missing => {}
                }
            }
            // A non-arrow function binds `arguments` around its parameters
            // and body, before any outer declaration.
            if scope.kind == (ScopeKind::Signature { arguments: true })
                && name == "arguments"
                && mask & meaning::VALUE != 0
            {
                return Step::Done(Resolution::NotReference("implicit arguments object"));
            }
            current = scope.parent;
        }
        if !global {
            match self.probe_lexical(GLOBAL, name, mask, type_context) {
                Probe::Found(resolution) => return Step::Done(resolution),
                Probe::Need(task) => return Step::Need(task),
                Probe::Missing => {}
            }
        }
        Step::Done(match name {
            "globalThis" => Resolution::GlobalThis,
            "undefined" => Resolution::NotReference("built-in value"),
            _ => Resolution::Unresolved(format!("cannot find `{name}`")),
        })
    }

    /// An export of a module, following `export =`, local exports, and star
    /// re-exports. Conflicting star exports are ambiguous.
    fn resolve_export(&mut self, module: ModuleId, name: &str) -> Step<Resolution> {
        // A task with cuts sees a lookup no other task shares.
        let shared = !self
            .running
            .is_some_and(|task| self.cut_tasks.contains(&task));
        if !shared {
            return self.resolve_export_uncached(module, name);
        }
        if let Some(resolution) = self.export_memo.get(&(module, name.to_string())) {
            return Step::Done(resolution.clone());
        }
        let step = self.resolve_export_uncached(module, name);
        if let Step::Done(resolution) = &step {
            self.export_memo
                .insert((module, name.to_string()), resolution.clone());
        }
        step
    }

    fn resolve_export_uncached(&mut self, module: ModuleId, name: &str) -> Step<Resolution> {
        let mut current = module;
        let mut type_only = false;
        let mut seen = HashSet::from([module]);
        loop {
            // A shorthand module lists no exports, so each has whatever
            // meaning its use asks for, unlike the module's own meanings.
            if self.modules[current.index()].shorthand {
                let symbol = self.modules[current.index()].symbol;
                return Step::Done(Resolution::Found(
                    Meanings::new(Slots::only(meaning::ALL, symbol)).restrict(type_only),
                ));
            }
            let exports = self.modules[current.index()].exports;
            // Explicit exports, including type exports beside `export =`.
            if name != "export=" {
                match self.probe(exports, name, meaning::ALL, true) {
                    Probe::Found(resolution) => return Step::Done(restrict(resolution, type_only)),
                    Probe::Need(task) => return Step::Need(task),
                    Probe::Missing => {}
                }
            }
            if !self.tables[exports.index()].contains_key("export=") || name == "export=" {
                break;
            }
            let found = match self.probe(exports, "export=", meaning::ALL, true) {
                Probe::Need(task) => return Step::Need(task),
                Probe::Found(Resolution::Found(found)) => found,
                Probe::Found(other) => return Step::Done(other),
                Probe::Missing => unreachable!("entry exists"),
            };
            if name == "default" {
                return Step::Done(Resolution::Found(found.restrict(type_only)));
            }
            type_only |= found.members_type_only(Reach::ExportAssignment);
            // An `export =` of another module object exposes its exports.
            let target = found
                .container()
                .and_then(|container| self.symbol(container).module);
            if let Some(target) = target {
                if !seen.insert(target) {
                    return Step::Done(Resolution::Unresolved("circular export =".into()));
                }
                current = target;
                continue;
            }
            return match self.member(found, name, meaning::ALL, true) {
                Step::Done(resolution) => Step::Done(restrict(resolution, type_only)),
                need => need,
            };
        }
        if name == "default" {
            return Step::Done(Resolution::Unresolved("no default export".into()));
        }
        self.star_export(current, name, type_only)
    }

    /// A name reached through `export *`. Each module is searched by its most
    /// accessible route, and routes to the same declarations are one target;
    /// different declarations make the name ambiguous.
    fn star_export(&self, module: ModuleId, name: &str, type_only: bool) -> Step<Resolution> {
        let mut best: HashMap<ModuleId, bool> = HashMap::from([(module, type_only)]);
        let mut stack: Vec<(ModuleId, bool)> = self.modules[module.index()]
            .stars
            .iter()
            .rev()
            .map(|&(star, star_type_only)| (star, type_only || star_type_only))
            .collect();
        let mut found: Vec<Meanings> = Vec::new();
        // Routes whose export of `name` does not resolve. They still export
        // the name, as TypeScript counts them: alone, the failure is the
        // result; beside another route, the name is ambiguous (TS2308).
        let mut failed: Vec<(ModuleId, Resolution)> = Vec::new();
        while let Some((star, route_type_only)) = stack.pop() {
            match best.get(&star) {
                Some(&previous) if previous <= route_type_only => continue,
                _ => {
                    best.insert(star, route_type_only);
                }
            }
            if self.modules[star.index()].shorthand {
                continue;
            }
            match self.probe(self.modules[star.index()].exports, name, meaning::ALL, true) {
                Probe::Need(task) => return Step::Need(task),
                Probe::Found(Resolution::Found(targets)) => {
                    let targets = targets.restrict(route_type_only);
                    match found
                        .iter_mut()
                        .find(|other| other.targets() == targets.targets())
                    {
                        Some(existing) => existing.merge_routes(targets),
                        None => found.push(targets),
                    }
                }
                Probe::Found(resolution) => {
                    if !failed.iter().any(|(module, _)| *module == star) {
                        failed.push((star, resolution));
                    }
                }
                Probe::Missing => {
                    for &(next, next_type_only) in self.modules[star.index()].stars.iter().rev() {
                        stack.push((next, next_type_only || route_type_only));
                    }
                }
            }
        }
        Step::Done(match (found.as_slice(), failed.len()) {
            ([], 0) => Resolution::Unresolved(format!("`{name}` is not exported")),
            ([targets], 0) => Resolution::Found(*targets),
            ([], 1) => failed.pop().unwrap().1,
            _ => Resolution::Unresolved(format!("`{name}` is ambiguous among star exports")),
        })
    }

    /// A member of the namespace-like target of `qualifier`: an export of a
    /// module or a namespace. Other targets have properties or members, which
    /// are not reference targets.
    fn member(
        &mut self,
        qualifier: Meanings,
        name: &str,
        mask: u8,
        type_context: bool,
    ) -> Step<Resolution> {
        let Some(container) = qualifier.container() else {
            return Step::Done(Resolution::NotReference("property"));
        };
        if let Some(module) = self.symbol(container).module {
            return self.module_member(module, name);
        }
        // A same-named value beside a namespace has properties of its own.
        let value = qualifier
            .targets()
            .value()
            .filter(|&value| self.find(value) != self.find(container));
        let symbol = self.symbol(container);
        if let Some(exports) = symbol.exports {
            match self.probe(exports, name, mask, type_context) {
                Probe::Found(resolution) => return Step::Done(resolution),
                Probe::Need(task) => return Step::Need(task),
                Probe::Missing => {}
            }
        }
        let member_kinds =
            flags::CLASS | flags::FUNCTION | flags::FUNCTION_SCOPED | flags::BLOCK_SCOPED;
        Step::Done(if symbol.flags & flags::ENUM != 0 {
            Resolution::NotReference("enum member")
        } else if symbol.flags & member_kinds != 0
            || value.is_some()
            // A namespace that exports nothing has no export table either.
            || symbol.exports.is_none() && symbol.flags & flags::NAMESPACE == 0
        {
            Resolution::NotReference("property")
        } else {
            Resolution::Unresolved(format!("namespace has no exported member `{name}`"))
        })
    }

    /// An export of a module named by a qualifier. A shorthand ambient
    /// module does not list its exports.
    fn module_member(&mut self, module: ModuleId, name: &str) -> Step<Resolution> {
        if self.modules[module.index()].shorthand {
            return Step::Done(Resolution::Unresolved(format!(
                "`{name}` is a member of a shorthand ambient module"
            )));
        }
        self.resolve_export(module, name)
    }

    /// A member of `globalThis`: a global type, or a global value other than
    /// a block-scoped variable.
    fn global_member(&self, name: &str, mask: u8, type_context: bool) -> Step<Resolution> {
        let found = match self.probe(GLOBAL, name, mask, type_context) {
            Probe::Need(task) => return Step::Need(task),
            Probe::Found(Resolution::Found(found)) => Some(found),
            _ => None,
        };
        let found = found
            .map(|found| match found.targets().value() {
                Some(value) if self.symbol(value).flags & flags::BLOCK_SCOPED != 0 => {
                    found.without_value()
                }
                _ => found,
            })
            .filter(|found| !found.is_empty());
        Step::Done(match found {
            Some(found) => Resolution::Found(found),
            None => Resolution::Unresolved(format!("`{name}` is not a global property")),
        })
    }

    /// What a namespace import, `import = require`, or UMD global denotes:
    /// the module's `export =` target, or the module itself.
    fn module_object(&self, module: ModuleId) -> Step<Resolution> {
        let exports = self.modules[module.index()].exports;
        if !self.tables[exports.index()].contains_key("export=") {
            let symbol = self.modules[module.index()].symbol;
            return Step::Done(Resolution::Found(Meanings::new(Slots::only(
                self.meanings_of(symbol),
                symbol,
            ))));
        }
        match self.probe(exports, "export=", meaning::ALL, true) {
            Probe::Need(task) => Step::Need(task),
            Probe::Found(resolution) => Step::Done(resolution),
            Probe::Missing => unreachable!("entry exists"),
        }
    }

    fn try_alias(&mut self, alias: AliasRef) -> Step<Resolution> {
        let facts = self.facts(alias.file);
        let data = &facts.aliases[alias.id.index()];
        let step = match &data.kind {
            AliasKind::Import { request, name } => {
                match self.request_module(alias.file, *request) {
                    Ok(module) => match self.resolve_export(module, name) {
                        // A default import of a declaration module without a
                        // default export or an `__esModule` marker is its module
                        // object (synthetic default).
                        Step::Done(Resolution::Unresolved(_))
                            if name == "default"
                                && self.modules[module.index()].declaration
                                && !["default", "__esModule"].iter().any(|name| {
                                    self.tables[self.modules[module.index()].exports.index()]
                                        .contains_key(*name)
                                })
                                && matches!(data.bind, AliasBind::Lexical { .. }) =>
                        {
                            self.module_object(module)
                        }
                        step => step,
                    },
                    Err(why) => Step::Done(Resolution::Unresolved(why.into())),
                }
            }
            AliasKind::Namespace { request } => match self.request_module(alias.file, *request) {
                Ok(module) => self.module_object(module),
                Err(why) => Step::Done(Resolution::Unresolved(why.into())),
            },
            AliasKind::SelfModule => match self.file_modules[alias.file.index()] {
                Some(module) => self.module_object(module),
                None => Step::Done(Resolution::Unresolved("file is not a module".into())),
            },
            AliasKind::Lexical { scope, name } => {
                self.lookup(alias.file, *scope, name, meaning::ALL, false)
            }
            AliasKind::Entity { reference, expand } => {
                let task = Task::Ref(RefRef {
                    file: alias.file,
                    id: *reference,
                });
                match self.done(task) {
                    Some(Resolution::Found(found)) if *expand => Step::Done(Resolution::Found(
                        found.expanded(|symbol| self.meanings_of(symbol)),
                    )),
                    Some(resolution) => Step::Done(resolution.clone()),
                    None => Step::Need(Need::required(task)),
                }
            }
        };
        match step {
            Step::Done(resolution) => Step::Done(restrict(resolution, data.type_only)),
            need => need,
        }
    }

    fn try_ref(&mut self, reference: RefRef) -> Step<Resolution> {
        let facts = self.facts(reference.file);
        let data = &facts.refs[reference.id.index()];
        let name = facts.names[data.token.index()].text.as_str();
        let step = match data.base {
            // Lookup already selects the meanings the reference needs from
            // each table it probes, by the same rule as `mask_resolution`.
            RefBase::Lexical(scope) => {
                return self.lookup(reference.file, scope, name, data.meaning, data.type_context);
            }
            RefBase::Member(base) => {
                let base = Task::Ref(RefRef {
                    file: reference.file,
                    id: base,
                });
                let qualifier = match self.done(base) {
                    None => return Step::Need(Need::required(base)),
                    Some(Resolution::Found(found)) => *found,
                    Some(Resolution::GlobalThis) => {
                        return self.global_member(name, data.meaning, data.type_context);
                    }
                    Some(Resolution::NotReference(_)) => {
                        return Step::Done(Resolution::NotReference("property"));
                    }
                    // A member of an unknown receiver is a property access,
                    // unless its qualified name can only name a namespace.
                    Some(Resolution::Unresolved(_)) if data.property => {
                        return Step::Done(Resolution::NotReference("property"));
                    }
                    Some(Resolution::Unresolved(_)) => {
                        return Step::Done(Resolution::Unresolved(format!(
                            "the qualifier of `{name}` is unresolved"
                        )));
                    }
                };
                match self.member(qualifier, name, data.meaning, data.type_context) {
                    Step::Done(resolution) => Step::Done(restrict(
                        resolution,
                        qualifier.members_type_only(Reach::Qualified),
                    )),
                    need => need,
                }
            }
            RefBase::Module(request) => match self.request_module(reference.file, request) {
                Ok(module) => self.module_member(module, name),
                Err(why) => Step::Done(Resolution::Unresolved(why.into())),
            },
        };
        match step {
            Step::Done(found) => Step::Done(mask_resolution(
                found,
                data.meaning,
                data.type_context,
                name,
            )),
            need => need,
        }
    }

    /// Resolve every searched occurrence and freeze the index.
    fn resolve_and_index(&mut self) -> Index {
        let mut index = index::Builder::default();
        for file in self.file_ids() {
            let mut tokens = Vec::new();
            if self.search(file) {
                let names = &self.facts(file).names;
                tokens.reserve(names.len());
                for name in names {
                    let target = match name.target {
                        NameTarget::Decl(decl) => {
                            Target::Symbols(SmallVec::from_elem(self.decl_symbol(file, decl), 1))
                        }
                        NameTarget::Ref(reference) => {
                            let resolution = self.force(Task::Ref(RefRef {
                                file,
                                id: reference,
                            }));
                            self.target(resolution, &mut index)
                        }
                        NameTarget::Alias(alias) => {
                            let resolution = self.force(Task::Alias(AliasRef { file, id: alias }));
                            self.target(resolution, &mut index)
                        }
                        NameTarget::NotReference(why) => Target::NotReference(why),
                    };
                    tokens.push(Token {
                        span: name.span,
                        name: &name.text,
                        roles: name.roles,
                        candidate: name.candidate,
                        target,
                    });
                }
            }
            index.add_file(file, tokens);
        }
        index.finish(&self.program.sources, |symbol| {
            self.symbol(symbol).sites.clone()
        })
    }

    fn target(&self, resolution: Resolution, index: &mut index::Builder) -> Target {
        match resolution {
            Resolution::Found(found) => {
                let mut symbols: SmallVec<[SymbolId; 2]> = SmallVec::new();
                for symbol in found.targets().symbols() {
                    let symbol = self.find(symbol);
                    if !symbols.contains(&symbol) {
                        symbols.push(symbol);
                    }
                }
                Target::Symbols(symbols)
            }
            Resolution::GlobalThis => Target::NotReference("built-in globalThis"),
            Resolution::NotReference(why) => Target::NotReference(why),
            Resolution::Unresolved(why) => Target::Unresolved(index.intern(&why)),
        }
    }

    /// An import alias may not share a meaning with a local declaration.
    fn check_conflicts(&mut self) -> Result<()> {
        for (table, name, file, span) in std::mem::take(&mut self.conflicts) {
            let Some(entry) = self.tables[table.index()].get(&name) else {
                continue;
            };
            let (Some(alias), slots) = (entry.alias, entry.slots) else {
                continue;
            };
            if let Resolution::Found(found) = self.force(Task::Alias(alias)) {
                let targets = found.targets();
                // An export specifier of a local names that local; any other
                // alias conflicts on a shared meaning even when it resolves
                // back to the local itself, as an import cycling through its
                // own module does (TypeScript's TS2440).
                let local = matches!(
                    self.facts(alias.file).aliases[alias.id.index()].kind,
                    AliasKind::Lexical { .. }
                );
                let overlap = each(meaning::ALL).any(|meaning| {
                    match (slots.get(meaning), targets.get(meaning)) {
                        (Some(a), Some(b)) => !local || self.find(a) != self.find(b),
                        _ => false,
                    }
                });
                if overlap && self.checks_bindings(file) {
                    return Err(self.error(
                        ProjectErrorKind::Binding,
                        file,
                        span,
                        format!("import or export `{name}` conflicts with a local declaration"),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A declaration being added to a table.
struct Declaring {
    kind: DeclKind,
    /// `symbol_flags` plus implementation and ambient-class facts.
    flags: u32,
    list: Option<(FileId, TypeParameterListId)>,
    site: Site,
}

/// Pairs that TypeScript's binder merges but its checker rejects: two
/// function implementations, a function with a non-ambient class, and one
/// type parameter list repeating a name.
fn invalid_merge(existing: u32, declared: u32, repeated_list: bool) -> bool {
    use flags::*;
    let function = declared & FUNCTION != 0;
    (function && declared & IMPLEMENTED != 0 && existing & IMPLEMENTED != 0)
        || (function && existing & CONCRETE_CLASS != 0)
        || (declared & CONCRETE_CLASS != 0 && existing & FUNCTION != 0)
        || repeated_list
}

/// TypeScript's check of merged class and interface type parameters, apart
/// from constraints and defaults: the merged list is each distinct name in
/// declaration order, and every declaration repeats a prefix of it that
/// includes each parameter no declaration gives a default.
fn identical_type_parameters<'a>(
    lists: impl Iterator<Item = &'a [(String, bool)]> + Clone,
) -> bool {
    let mut merged: Vec<(&str, bool)> = Vec::new();
    for list in lists.clone() {
        for (name, default) in list {
            match merged.iter_mut().find(|(merged, _)| merged == name) {
                Some((_, merged)) => *merged |= default,
                None => merged.push((name, *default)),
            }
        }
    }
    let required = merged
        .iter()
        .rposition(|&(_, default)| !default)
        .map_or(0, |index| index + 1);
    lists.into_iter().all(|list| {
        (required..=merged.len()).contains(&list.len())
            && list
                .iter()
                .zip(&merged)
                .all(|((name, _), (merged, _))| name == merged)
    })
}

/// Mark a resolution reached through a type-only route.
fn restrict(resolution: Resolution, type_only: bool) -> Resolution {
    match resolution {
        Resolution::Found(found) => Resolution::Found(found.restrict(type_only)),
        other => other,
    }
}

/// Select the meanings a reference needs, retaining each target's own access.
fn mask_resolution(resolution: Resolution, mask: u8, type_context: bool, name: &str) -> Resolution {
    match resolution {
        Resolution::Found(found) => {
            let selected = found.select(mask, type_context);
            if selected.is_empty() {
                Resolution::Unresolved(format!("`{name}` has no matching meaning"))
            } else {
                Resolution::Found(selected)
            }
        }
        other => other,
    }
}
