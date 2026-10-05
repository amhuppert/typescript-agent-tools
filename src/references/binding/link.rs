//! Project-wide declaration tables, module exports, aliases, and reference
//! resolution.
//!
//! A table entry holds one symbol per meaning plus an optional alias, so a
//! same-named type and value stay separate unless their declarations merge.
//! Aliases and qualified references resolve on demand through one explicit
//! task stack; cycles become unresolved targets rather than recursion.

use super::collect::{AliasBind, AliasKind, DeclKind, Facts, Host, NameTarget, RefBase, ScopeKind};
use super::index::{Index, Occurrence, OccurrenceRef, Site, Strings, SymbolId, Target};
use super::{FileFacts, ModuleResolution, RequestKind, meaning};
use crate::Span;
use crate::references::project::Program;
use crate::references::source::{FileId, token_span};
use crate::references::{ProjectError, ProjectErrorKind};
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

type Slots = [Option<SymbolId>; 3];
type TableId = usize;
type ModuleId = usize;
const GLOBAL: TableId = 0;

const BITS: [u8; 3] = [meaning::VALUE, meaning::TYPE, meaning::NAMESPACE];
const GLOBAL_THIS: &str = "built-in globalThis";

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
        DeclKind::Namespace { instantiated: true } => VALUE_MODULE,
        DeclKind::Namespace {
            instantiated: false,
        } => NAMESPACE_MODULE,
        DeclKind::TypeParameter => TYPE_PARAMETER,
        DeclKind::Expression => EXPRESSION,
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
        DeclKind::Namespace { instantiated: true } => {
            VALUE & !(FUNCTION | CLASS | REGULAR_ENUM | VALUE_MODULE)
        }
        DeclKind::Namespace {
            instantiated: false,
        } => 0,
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

#[derive(Debug, Default)]
struct Entry {
    slots: Slots,
    alias: Option<AliasRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AliasRef {
    file: FileId,
    index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RefRef {
    file: FileId,
    index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Task {
    Alias(AliasRef),
    Ref(RefRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Resolution {
    Found { slots: Slots, type_only: bool },
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
    Need(Task),
}

/// The result of checking one table for a name.
enum Check {
    Found(Resolution),
    Need(Task),
    Missing,
}

#[derive(Debug)]
struct Symbol {
    flags: u32,
    sites: Vec<Site>,
    /// Type parameter lists that declare this type parameter.
    lists: Vec<(FileId, u32)>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestTarget {
    Module(ModuleId),
    Script,
    Ignored,
}

struct Linker<'p> {
    program: &'p Program,
    tables: Vec<HashMap<String, Entry>>,
    scope_tables: Vec<Vec<Option<TableId>>>,
    symbols: Vec<Symbol>,
    parents: Vec<u32>,
    decl_symbols: Vec<Vec<Option<SymbolId>>>,
    modules: Vec<Module>,
    file_modules: Vec<Option<ModuleId>>,
    ambient: HashMap<String, ModuleId>,
    patterns: Vec<(String, String, ModuleId)>,
    targets: Vec<Vec<RequestTarget>>,
    alias_states: Vec<Vec<State>>,
    ref_states: Vec<Vec<State>>,
    export_memo: HashMap<(ModuleId, String), Resolution>,
    conflicts: Vec<(TableId, String, FileId, Span)>,
}

type Result<T> = std::result::Result<T, ProjectError>;

pub(super) fn link(program: &Program) -> Result<Index> {
    let mut linker = Linker::new(program);
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
            targets: Vec::new(),
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
        }
    }

    fn file_ids(&self) -> impl Iterator<Item = FileId> + use<> {
        (0..self.program.files.len() as u32).map(FileId)
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
    fn text(&self, file: FileId) -> &'p str {
        self.program.sources.get(file).text.as_str()
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
        self.tables.len() - 1
    }

    fn new_symbol(&mut self, flags: u32, site: Option<Site>) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            flags,
            sites: site.into_iter().collect(),
            lists: Vec::new(),
            exports: None,
            members: None,
            module: None,
            enum_members: Vec::new(),
        });
        self.parents.push(id.0);
        id
    }

    /// The canonical symbol after declaration merges.
    fn find(&self, mut id: SymbolId) -> SymbolId {
        while self.parents[id.0 as usize] != id.0 {
            id = SymbolId(self.parents[id.0 as usize]);
        }
        id
    }
    fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[self.find(id).0 as usize]
    }

    fn new_module(&mut self, site: Site, shorthand: bool, declaration: bool) -> ModuleId {
        let exports = self.table();
        let symbol = self.new_symbol(flags::MODULE, Some(site));
        let id = self.modules.len();
        self.symbols[symbol.0 as usize].module = Some(id);
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
                    span: token_span(self.text(file), ambient.span),
                    name: ambient.name.clone(),
                };
                match self.ambient.get(&ambient.name) {
                    Some(&module) => {
                        let symbol = self.modules[module].symbol;
                        self.symbols[symbol.0 as usize].sites.push(site);
                        self.modules[module].shorthand &= ambient.shorthand;
                    }
                    None => {
                        let module = self.new_module(site, ambient.shorthand, true);
                        self.ambient.insert(ambient.name.clone(), module);
                        if let Some((prefix, suffix)) = ambient.name.split_once('*') {
                            self.patterns.push((prefix.into(), suffix.into(), module));
                        }
                    }
                }
            }
        }
    }

    fn resolve_requests(&mut self) -> Result<()> {
        for file in self.file_ids() {
            let program_file = &self.program.files[file.index()];
            let mut targets = Vec::with_capacity(program_file.facts.requests.len());
            for (index, request) in program_file.facts.requests.iter().enumerate() {
                let resolution = program_file
                    .modules
                    .get(index)
                    .cloned()
                    .unwrap_or(ModuleResolution::Missing);
                let relative = is_relative(&request.specifier);
                let ambient = (!relative)
                    .then(|| self.ambient.get(&request.specifier).copied())
                    .flatten();
                let span = token_span(self.text(file), request.span);
                let target = match (ambient, resolution) {
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
                            None if program_file.unchecked
                                || program_file
                                    .suppressions
                                    .covers(&self.program.sources.get(file).text, span.start) =>
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
                    (None, ModuleResolution::Unsupported(_)) if !self.search(file) => {
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
                    if let RequestTarget::Module(module) = target {
                        let symbol = self.modules[module].symbol;
                        let site = Site {
                            file,
                            span,
                            name: request.specifier.clone(),
                        };
                        self.symbols[symbol.0 as usize].sites.push(site);
                    } else if self.search(file) && target != RequestTarget::Ignored {
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
                }
                targets.push(target);
            }
            self.targets.push(targets);
        }
        Ok(())
    }

    /// Whether declarations or aliases may lack their export host: a package
    /// file, or a module augmentation whose missing target is tolerated.
    fn tolerated(&self, file: FileId, host: Host) -> bool {
        !self.search(file)
            || matches!(host, Host::Augmentation(request)
                if self.targets[file.index()][request as usize] == RequestTarget::Ignored)
    }

    /// The pattern ambient module with the longest matching prefix.
    fn pattern(&self, specifier: &str) -> Option<ModuleId> {
        self.patterns
            .iter()
            .filter(|(prefix, suffix, _)| {
                specifier.len() >= prefix.len() + suffix.len()
                    && specifier.starts_with(prefix.as_str())
                    && specifier.ends_with(suffix.as_str())
            })
            .max_by_key(|(prefix, _, _)| prefix.len())
            .map(|(_, _, module)| *module)
    }

    fn host_module(&self, file: FileId, host: Host) -> Option<ModuleId> {
        match host {
            Host::File => self.file_modules[file.index()],
            Host::AmbientModule(index) => {
                let name = &self.facts(file).ambient_modules[index as usize].name;
                self.ambient.get(name).copied()
            }
            Host::Augmentation(request) => match self.targets[file.index()][request as usize] {
                RequestTarget::Module(module) => Some(module),
                _ => None,
            },
            Host::Namespace(_) => None,
        }
    }

    fn collect_stars(&mut self) {
        for file in self.file_ids() {
            for star in &self.facts(file).stars {
                let (Some(host), RequestTarget::Module(target)) = (
                    self.host_module(file, star.host),
                    self.targets[file.index()][star.request as usize],
                ) else {
                    continue;
                };
                self.modules[host].stars.push((target, star.type_only));
            }
        }
    }

    /// The table of a scope, created on first use.
    fn scope_table(&mut self, file: FileId, scope: u32) -> TableId {
        let facts = self.facts(file);
        match facts.scopes[scope as usize].kind {
            ScopeKind::Root if !self.file(file).module => return GLOBAL,
            ScopeKind::Global => return GLOBAL,
            ScopeKind::Members { decl } => {
                let symbol = self.decl_symbol(file, decl);
                return self.member_table(symbol);
            }
            _ => {}
        }
        if let Some(table) = self.scope_tables[file.index()][scope as usize] {
            return table;
        }
        let table = self.table();
        self.scope_tables[file.index()][scope as usize] = Some(table);
        table
    }

    fn decl_symbol(&self, file: FileId, decl: u32) -> SymbolId {
        self.find(self.decl_symbols[file.index()][decl as usize].expect("declared before use"))
    }

    fn member_table(&mut self, symbol: SymbolId) -> TableId {
        let symbol = self.find(symbol);
        if let Some(table) = self.symbols[symbol.0 as usize].members {
            return table;
        }
        let table = self.table();
        self.symbols[symbol.0 as usize].members = Some(table);
        table
    }

    fn export_table(&mut self, symbol: SymbolId) -> TableId {
        let symbol = self.find(symbol);
        if let Some(table) = self.symbols[symbol.0 as usize].exports {
            return table;
        }
        let table = self.table();
        self.symbols[symbol.0 as usize].exports = Some(table);
        table
    }

    fn host_table(&mut self, file: FileId, host: Host) -> Option<TableId> {
        match host {
            Host::Namespace(decl) => {
                let symbol = self.decl_symbol(file, decl);
                Some(self.export_table(symbol))
            }
            _ => self
                .host_module(file, host)
                .map(|module| self.modules[module].exports),
        }
    }

    fn declare_all(&mut self, deferred: bool) -> Result<()> {
        for file in self.file_ids() {
            let facts = self.facts(file);
            for (index, decl) in facts.decls.iter().enumerate() {
                if decl.deferred == deferred {
                    self.declare(file, index as u32)?;
                }
            }
            for (index, alias) in facts.aliases.iter().enumerate() {
                if alias.deferred == deferred {
                    self.bind_alias(file, index as u32)?;
                }
            }
        }
        Ok(())
    }

    fn declare(&mut self, file: FileId, index: u32) -> Result<()> {
        let decl = &self.facts(file).decls[index as usize];
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
            list: (decl.list != u32::MAX).then_some((file, decl.list)),
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
                // A tolerated augmentation of an absent module binds nothing.
                None if self.tolerated(file, *host) => {
                    self.new_symbol(declaring.flags, Some(declaring.site))
                }
                None => {
                    return Err(self.error(
                        ProjectErrorKind::Binding,
                        file,
                        decl.anchor,
                        format!("no module to export `{}` from", decl.name),
                    ));
                }
            },
            (None, None) => self.new_symbol(declaring.flags, Some(declaring.site)),
        };
        self.decl_symbols[file.index()][index as usize] = Some(symbol);
        if !decl.members.is_empty() {
            let canonical = self.find(symbol);
            self.symbols[canonical.0 as usize]
                .enum_members
                .extend(decl.members.iter().cloned());
        }
        if let (Some(_), Some((host, name))) = (decl.lexical, &decl.export) {
            let table = self.host_table(file, *host);
            if table.is_none() && self.tolerated(file, *host) {
                return Ok(());
            }
            let table = table.ok_or_else(|| {
                self.error(
                    ProjectErrorKind::Binding,
                    file,
                    decl.anchor,
                    format!("no module to export `{}` from", decl.name),
                )
            })?;
            let name = name.clone();
            self.export_symbol(
                table,
                &name,
                symbol,
                decl.kind.meanings(),
                file,
                decl.anchor,
            )?;
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
                    if facts.scopes[scope as usize].kind == ScopeKind::Catch {
                        continue;
                    }
                    let blocked = self.scope_tables[file.index()][scope as usize]
                        .and_then(|table| self.tables[table].get(&decl.name))
                        .and_then(|entry| entry.slots[0])
                        .is_some_and(|symbol| self.symbol(symbol).flags & flags::BLOCK_SCOPED != 0);
                    if blocked && self.search(file) {
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
    /// parameters differ (TS2428). Only searched declarations are checked.
    fn check_merges(&self) -> Result<()> {
        let mut declarations: HashMap<SymbolId, Vec<(FileId, u32)>> = HashMap::new();
        for file in self.file_ids() {
            for (index, symbol) in self.decl_symbols[file.index()].iter().enumerate() {
                if let Some(symbol) = symbol {
                    declarations
                        .entry(self.find(*symbol))
                        .or_default()
                        .push((file, index as u32));
                }
            }
        }
        let decl = |(file, index): (FileId, u32)| &self.facts(file).decls[index as usize];
        for list in declarations.values().filter(|list| list.len() > 1) {
            let anchor = list.iter().copied().find(|&(file, index)| {
                let decl = decl((file, index));
                !decl.ambient && matches!(decl.kind, DeclKind::Class | DeclKind::Function)
            });
            for &(file, index) in list {
                let namespace = decl((file, index));
                let instantiated = namespace.kind == (DeclKind::Namespace { instantiated: true });
                if !instantiated || namespace.ambient || !self.search(file) {
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
                if let Some(&((file, index), _)) =
                    generic.iter().find(|((file, _), _)| self.search(*file))
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

    /// The parameter table a body-level `var` may redeclare into.
    fn signature_table(&self, file: FileId, scope: u32, kind: DeclKind) -> Option<TableId> {
        if kind.meanings() & meaning::VALUE == 0 {
            return None;
        }
        let scopes = &self.facts(file).scopes;
        let ScopeKind::Body = scopes[scope as usize].kind else {
            return None;
        };
        let parent = scopes[scope as usize].parent?;
        matches!(scopes[parent as usize].kind, ScopeKind::Signature { .. })
            .then(|| self.scope_tables[file.index()][parent as usize])
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
        let mut existing: SmallVec<[SymbolId; 3]> = SmallVec::new();
        let entry = self.tables[table].get(name);
        let mut slots = entry.map(|entry| entry.slots).unwrap_or_default();
        if let Some(parameter) = parameters
            .and_then(|parameters| self.tables[parameters].get(name))
            .and_then(|entry| entry.slots[0])
        {
            slots[0] = slots[0].or(Some(parameter));
        }
        let has_alias = entry.is_some_and(|entry| entry.alias.is_some());
        for symbol in slots.into_iter().flatten() {
            let symbol = self.find(symbol);
            if !existing.contains(&symbol) {
                existing.push(symbol);
            }
        }
        let mut merge_with: SmallVec<[SymbolId; 3]> = SmallVec::new();
        for &symbol in &existing {
            let flags = self.symbols[symbol.0 as usize].flags;
            let repeated =
                list.is_some_and(|list| self.symbols[symbol.0 as usize].lists.contains(&list));
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
                    self.merge_symbols(first, other, &site)?;
                }
                let target = &mut self.symbols[first.0 as usize];
                target.flags |= declared;
                target.sites.push(site.clone());
                first
            }
        };
        self.symbols[symbol.0 as usize].lists.extend(list);
        let meanings = kind.meanings() | self.meanings_of(symbol);
        let entry = self.tables[table].entry(name.to_string()).or_default();
        for (index, bit) in BITS.iter().enumerate() {
            if meanings & bit != 0 {
                let occupied = entry.slots[index].map(|s| s.0);
                let replace = match occupied {
                    None => true,
                    Some(other) => merge_with.iter().any(|m| m.0 == other) || other == symbol.0,
                };
                if replace {
                    entry.slots[index] = Some(symbol);
                }
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
        !self.search(file)
            && self
                .symbol(symbol)
                .sites
                .iter()
                .all(|site| !self.search(site.file))
    }

    fn meanings_of(&self, symbol: SymbolId) -> u8 {
        let flags = self.symbol(symbol).flags;
        let mut meanings = 0;
        if flags & flags::VALUE != 0 {
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

    /// Merge `other` into `target`, combining namespace exports and shared
    /// type parameters.
    fn merge_symbols(&mut self, target: SymbolId, other: SymbolId, site: &Site) -> Result<()> {
        let mut work = vec![(target, other)];
        while let Some((target, other)) = work.pop() {
            let (target, other) = (self.find(target), self.find(other));
            if target == other {
                continue;
            }
            self.parents[other.0 as usize] = target.0;
            let moved = std::mem::replace(
                &mut self.symbols[other.0 as usize],
                Symbol {
                    flags: 0,
                    sites: Vec::new(),
                    lists: Vec::new(),
                    exports: None,
                    members: None,
                    module: None,
                    enum_members: Vec::new(),
                },
            );
            let symbol = &mut self.symbols[target.0 as usize];
            symbol.flags |= moved.flags;
            symbol.sites.extend(moved.sites);
            symbol.lists.extend(moved.lists);
            symbol.enum_members.extend(moved.enum_members);
            for (from, slot) in [(moved.exports, 0), (moved.members, 1)] {
                let Some(from) = from else { continue };
                let into = if slot == 0 {
                    self.export_table(target)
                } else {
                    self.member_table(target)
                };
                let entries: Vec<(String, Entry)> = self.tables[from].drain().collect();
                for (name, entry) in entries {
                    let destination = self.tables[into].entry(name.clone()).or_default();
                    for index in 0..3 {
                        match (destination.slots[index], entry.slots[index]) {
                            (Some(a), Some(b)) => work.push((a, b)),
                            (None, Some(b)) => destination.slots[index] = Some(b),
                            _ => {}
                        }
                    }
                    if entry.alias.is_some() {
                        if destination.alias.is_some() {
                            return Err(self.error(
                                ProjectErrorKind::Binding,
                                site.file,
                                site.span,
                                format!("merged declarations both export `{name}`"),
                            ));
                        }
                        destination.alias = entry.alias;
                    }
                }
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
        let entry = self.tables[table].entry(name.to_string()).or_default();
        for (index, bit) in BITS.iter().enumerate() {
            if meanings & bit == 0 {
                continue;
            }
            match entry.slots[index] {
                None => entry.slots[index] = Some(symbol),
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

    fn bind_alias(&mut self, file: FileId, index: u32) -> Result<()> {
        let alias = &self.facts(file).aliases[index as usize];
        let reference = AliasRef { file, index };
        let span = alias.span;
        let host = match &alias.bind {
            AliasBind::Export { host, .. } | AliasBind::Both { host, .. } => Some(*host),
            _ => None,
        };
        let tolerated = host.is_some_and(|host| self.tolerated(file, host));
        let bind = |linker: &mut Self, table: Option<TableId>, name: &str| -> Result<()> {
            if table.is_none() && tolerated {
                return Ok(());
            }
            let table = table.ok_or_else(|| {
                linker.error(
                    ProjectErrorKind::Binding,
                    file,
                    span,
                    format!("no module to export `{name}` from"),
                )
            })?;
            let entry = linker.tables[table].entry(name.to_string()).or_default();
            if entry.alias.is_some() {
                if linker.search(file) {
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
            if entry.slots.iter().any(Option::is_some) {
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
            Task::Alias(a) => &self.alias_states[a.file.index()][a.index as usize],
            Task::Ref(r) => &self.ref_states[r.file.index()][r.index as usize],
        }
    }
    fn set_state(&mut self, task: Task, state: State) {
        match task {
            Task::Alias(a) => self.alias_states[a.file.index()][a.index as usize] = state,
            Task::Ref(r) => self.ref_states[r.file.index()][r.index as usize] = state,
        }
    }

    /// Resolve `task` and everything it needs with an explicit stack. A task
    /// that needs one of its own dependents is circular and unresolved.
    fn force(&mut self, task: Task) -> Resolution {
        let mut stack = vec![task];
        while let Some(&top) = stack.last() {
            if let State::Done(_) = self.state(top) {
                stack.pop();
                continue;
            }
            self.set_state(top, State::InProgress);
            let step = match top {
                Task::Alias(alias) => self.try_alias(alias),
                Task::Ref(reference) => self.try_ref(reference),
            };
            match step {
                Step::Done(resolution) => {
                    self.set_state(top, State::Done(resolution));
                    stack.pop();
                }
                Step::Need(dependency) => {
                    if let State::InProgress = self.state(dependency) {
                        self.set_state(
                            top,
                            State::Done(Resolution::Unresolved("circular alias".into())),
                        );
                        stack.pop();
                    } else {
                        stack.push(dependency);
                    }
                }
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

    /// Check one table for `name` in `mask`. A mask of every meaning returns
    /// the first entry's meanings, as for export specifiers.
    fn check(&self, table: TableId, name: &str, mask: u8, type_context: bool) -> Check {
        self.check_entry(table, name, mask, type_context, false)
    }

    /// `check` for a lexical lookup, to which export specifiers are invisible
    /// even in the export table of an enclosing namespace.
    fn check_lexical(&self, table: TableId, name: &str, mask: u8, type_context: bool) -> Check {
        self.check_entry(table, name, mask, type_context, true)
    }

    fn check_entry(
        &self,
        table: TableId,
        name: &str,
        mask: u8,
        type_context: bool,
        lexical: bool,
    ) -> Check {
        let Some(entry) = self.tables[table].get(name) else {
            return Check::Missing;
        };
        let mut slots: Slots = [None; 3];
        for (index, bit) in BITS.iter().enumerate() {
            if mask & bit != 0 {
                slots[index] = entry.slots[index].map(|symbol| self.find(symbol));
            }
        }
        let mut type_only = false;
        let alias = entry.alias.filter(|alias| {
            !lexical || !self.facts(alias.file).aliases[alias.index as usize].export_only
        });
        if let Some(alias) = alias {
            let needed = BITS
                .iter()
                .enumerate()
                .any(|(index, bit)| mask & bit != 0 && slots[index].is_none());
            if needed {
                match self.done(Task::Alias(alias)) {
                    None => return Check::Need(Task::Alias(alias)),
                    Some(Resolution::Found {
                        slots: targets,
                        type_only: restricted,
                    }) => {
                        for (index, bit) in BITS.iter().enumerate() {
                            // Only a value lookup is blocked; an all-meaning
                            // alias keeps the restriction for its own uses.
                            let value_blocked =
                                *restricted && mask == meaning::VALUE && !type_context;
                            if mask & bit != 0 && slots[index].is_none() && !value_blocked {
                                slots[index] = targets[index].map(|symbol| self.find(symbol));
                                type_only |= *restricted && slots[index].is_some();
                            }
                        }
                    }
                    Some(other) => {
                        if slots.iter().all(Option::is_none) {
                            return Check::Found(other.clone());
                        }
                    }
                }
            }
        }
        if slots.iter().any(Option::is_some) {
            Check::Found(Resolution::Found { slots, type_only })
        } else {
            Check::Missing
        }
    }

    /// Tables consulted for a scope: its locals, then any export table whose
    /// members are visible inside it.
    fn lookup_tables(&self, file: FileId, scope: u32) -> SmallVec<[TableId; 2]> {
        let facts = self.facts(file);
        let own = self.scope_tables[file.index()][scope as usize];
        let mut tables = SmallVec::new();
        match facts.scopes[scope as usize].kind {
            ScopeKind::Root if !self.file(file).module => tables.push(GLOBAL),
            ScopeKind::Global => tables.push(GLOBAL),
            ScopeKind::Members { decl } => {
                if let Some(symbol) = self.decl_symbols[file.index()][decl as usize] {
                    tables.extend(self.symbol(symbol).members);
                }
            }
            ScopeKind::Namespace { decl } => {
                tables.extend(own);
                if let Some(symbol) = self.decl_symbols[file.index()][decl as usize] {
                    tables.extend(self.symbol(symbol).exports);
                }
            }
            ScopeKind::AmbientModule { .. } | ScopeKind::Augmentation { .. } => {
                tables.extend(own);
                let host = match facts.scopes[scope as usize].kind {
                    ScopeKind::AmbientModule { module } => Host::AmbientModule(module),
                    ScopeKind::Augmentation { request } => Host::Augmentation(request),
                    _ => unreachable!(),
                };
                if let Some(module) = self.host_module(file, host) {
                    tables.push(self.modules[module].exports);
                }
            }
            _ => tables.extend(own),
        }
        tables
    }

    /// Lexical lookup from `scope` outward to the global table.
    fn lookup(
        &self,
        file: FileId,
        scope: u32,
        name: &str,
        mask: u8,
        type_context: bool,
    ) -> Step<Resolution> {
        let facts = self.facts(file);
        let mut current = Some(scope);
        let mut global = false;
        while let Some(index) = current {
            let scope = &facts.scopes[index as usize];
            // An infer constraint sees only its own declaration's name.
            if let ScopeKind::InferConstraint { decl } = scope.kind {
                let own = &facts.decls[decl as usize];
                if own.name == name && mask & meaning::TYPE != 0 {
                    if let Some(symbol) = self.decl_symbols[file.index()][decl as usize] {
                        return Step::Done(Resolution::Found {
                            slots: [None, Some(self.find(symbol)), None],
                            type_only: false,
                        });
                    }
                }
            }
            // Enum initializers see the enum's members, which are not targets.
            if let ScopeKind::Enum { decl } = scope.kind {
                let member = self.decl_symbols[file.index()][decl as usize].is_some_and(|symbol| {
                    self.symbol(symbol).enum_members.iter().any(|m| m == name)
                });
                if member {
                    return Step::Done(Resolution::NotReference("enum member"));
                }
            }
            for table in self.lookup_tables(file, index) {
                global |= table == GLOBAL;
                match self.check_lexical(table, name, mask, type_context) {
                    Check::Found(resolution) => return Step::Done(resolution),
                    Check::Need(task) => return Step::Need(task),
                    Check::Missing => {}
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
            match self.check_lexical(GLOBAL, name, mask, type_context) {
                Check::Found(resolution) => return Step::Done(resolution),
                Check::Need(task) => return Step::Need(task),
                Check::Missing => {}
            }
        }
        Step::Done(match name {
            "globalThis" => Resolution::NotReference(GLOBAL_THIS),
            "undefined" => Resolution::NotReference("built-in value"),
            _ => Resolution::Unresolved(format!("cannot find `{name}`")),
        })
    }

    fn module_slots(&self, module: ModuleId) -> Slots {
        let symbol = Some(self.modules[module].symbol);
        [symbol, None, symbol]
    }

    /// An export of a module, following `export =`, local exports, and star
    /// re-exports. Conflicting star exports are ambiguous.
    fn resolve_export(&mut self, module: ModuleId, name: &str) -> Step<Resolution> {
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

    fn resolve_export_uncached(&self, module: ModuleId, name: &str) -> Step<Resolution> {
        let mut current = module;
        let mut type_only = false;
        let mut seen = HashSet::from([module]);
        loop {
            if self.modules[current].shorthand {
                return Step::Done(Resolution::Found {
                    slots: [Some(self.modules[current].symbol); 3],
                    type_only,
                });
            }
            let exports = self.modules[current].exports;
            // Explicit exports, including type exports beside `export =`.
            if name != "export=" {
                match self.check(exports, name, meaning::ALL, true) {
                    Check::Found(resolution) => return Step::Done(restrict(resolution, type_only)),
                    Check::Need(task) => return Step::Need(task),
                    Check::Missing => {}
                }
            }
            if !self.tables[exports].contains_key("export=") || name == "export=" {
                break;
            }
            let (slots, restricted) = match self.check(exports, "export=", meaning::ALL, true) {
                Check::Need(task) => return Step::Need(task),
                Check::Found(Resolution::Found { slots, type_only }) => (slots, type_only),
                Check::Found(other) => return Step::Done(other),
                Check::Missing => unreachable!("entry exists"),
            };
            type_only |= restricted;
            if name == "default" {
                return Step::Done(Resolution::Found { slots, type_only });
            }
            // An `export =` of another module object exposes its exports.
            let target = slots[2]
                .or(slots[0])
                .and_then(|container| self.symbol(container).module);
            if let Some(target) = target {
                if !seen.insert(target) {
                    return Step::Done(Resolution::Unresolved("circular export =".into()));
                }
                current = target;
                continue;
            }
            return Step::Done(
                match self.namespace_member(slots, name, meaning::ALL, true) {
                    Check::Need(task) => return Step::Need(task),
                    Check::Found(resolution) => restrict(resolution, type_only),
                    Check::Missing => Resolution::Unresolved(format!("`{name}` is not exported")),
                },
            );
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
        let mut stack: Vec<(ModuleId, bool)> = self.modules[module]
            .stars
            .iter()
            .rev()
            .map(|&(star, star_type_only)| (star, type_only || star_type_only))
            .collect();
        let mut found: Vec<(Slots, bool)> = Vec::new();
        while let Some((star, route_type_only)) = stack.pop() {
            match best.get(&star) {
                Some(&previous) if previous <= route_type_only => continue,
                _ => {
                    best.insert(star, route_type_only);
                }
            }
            if self.modules[star].shorthand {
                continue;
            }
            match self.check(self.modules[star].exports, name, meaning::ALL, true) {
                Check::Need(task) => return Step::Need(task),
                Check::Found(Resolution::Found { slots, type_only }) => {
                    let restricted = type_only || route_type_only;
                    match found.iter_mut().find(|(other, _)| *other == slots) {
                        Some((_, existing)) => *existing &= restricted,
                        None => found.push((slots, restricted)),
                    }
                }
                Check::Found(_) => {}
                Check::Missing => {
                    for &(next, next_type_only) in self.modules[star].stars.iter().rev() {
                        stack.push((next, next_type_only || route_type_only));
                    }
                }
            }
        }
        Step::Done(match found.as_slice() {
            [] => Resolution::Unresolved(format!("`{name}` is not exported")),
            [(slots, type_only)] => Resolution::Found {
                slots: *slots,
                type_only: *type_only,
            },
            _ => Resolution::Unresolved(format!("`{name}` is ambiguous among star exports")),
        })
    }

    /// A member of the namespace-like target among `slots`: a namespace's
    /// exports or a module's exports. Other targets have properties or
    /// members, which are not reference targets.
    fn namespace_member(&self, slots: Slots, name: &str, mask: u8, type_context: bool) -> Check {
        let Some(container) = slots[2].or(slots[0]) else {
            return Check::Found(Resolution::NotReference("property"));
        };
        // A same-named value beside a namespace has properties of its own.
        let value = slots[0].filter(|&value| self.find(value) != self.find(container));
        let symbol = self.symbol(container);
        if symbol.module.is_some() {
            // Modules are resolved through `resolve_export`, which needs `&mut`.
            return Check::Missing;
        }
        if let Some(exports) = symbol.exports {
            match self.check(exports, name, mask, type_context) {
                Check::Missing => {}
                other => return other,
            }
        }
        let member_kinds =
            flags::CLASS | flags::FUNCTION | flags::FUNCTION_SCOPED | flags::BLOCK_SCOPED;
        Check::Found(if symbol.flags & flags::ENUM != 0 {
            Resolution::NotReference("enum member")
        } else if symbol.flags & member_kinds != 0 || symbol.exports.is_none() || value.is_some() {
            Resolution::NotReference("property")
        } else {
            Resolution::Unresolved(format!("namespace has no exported member `{name}`"))
        })
    }

    /// A member of `globalThis`: a global type, or a global value other than
    /// a block-scoped variable.
    fn global_member(&self, name: &str, mask: u8, type_context: bool) -> Step<Resolution> {
        let mut slots = match self.check(GLOBAL, name, mask, type_context) {
            Check::Need(task) => return Step::Need(task),
            Check::Found(Resolution::Found { slots, .. }) => slots,
            _ => [None; 3],
        };
        if slots[0].is_some_and(|value| self.symbol(value).flags & flags::BLOCK_SCOPED != 0) {
            slots[0] = None;
        }
        Step::Done(if slots.iter().any(Option::is_some) {
            Resolution::Found {
                slots,
                type_only: false,
            }
        } else {
            Resolution::Unresolved(format!("`{name}` is not a global property"))
        })
    }

    /// Every meaning of the symbols found for some meanings, as when
    /// `import M = N` finds `N` as a namespace and aliases all of it.
    fn all_meanings(&self, slots: Slots) -> Slots {
        let mut all = slots;
        for symbol in slots.into_iter().flatten() {
            let meanings = self.meanings_of(symbol);
            for (index, bit) in BITS.iter().enumerate() {
                if meanings & bit != 0 && all[index].is_none() {
                    all[index] = Some(symbol);
                }
            }
        }
        all
    }

    /// What a namespace import, `import = require`, or UMD global denotes:
    /// the module's `export =` target, or the module itself.
    fn module_object(&self, module: ModuleId) -> Step<Resolution> {
        let exports = self.modules[module].exports;
        if !self.tables[exports].contains_key("export=") {
            return Step::Done(Resolution::Found {
                slots: self.module_slots(module),
                type_only: false,
            });
        }
        match self.check(exports, "export=", meaning::ALL, true) {
            Check::Need(task) => Step::Need(task),
            Check::Found(resolution) => Step::Done(resolution),
            Check::Missing => unreachable!("entry exists"),
        }
    }

    fn try_alias(&mut self, alias: AliasRef) -> Step<Resolution> {
        let facts = self.facts(alias.file);
        let data = &facts.aliases[alias.index as usize];
        let restrict = |resolution: Resolution| match resolution {
            Resolution::Found { slots, type_only } => Resolution::Found {
                slots,
                type_only: type_only || data.type_only,
            },
            other => other,
        };
        let step = match &data.kind {
            AliasKind::Import { request, name } => {
                match self.targets[alias.file.index()][*request as usize] {
                    RequestTarget::Module(module) => match self.resolve_export(module, name) {
                        // A default import of a declaration module without
                        // a default export or an `__esModule` marker is its
                        // module object (synthetic default).
                        Step::Done(Resolution::Unresolved(_))
                            if name == "default"
                                && self.modules[module].declaration
                                && !["default", "__esModule"].iter().any(|name| {
                                    self.tables[self.modules[module].exports].contains_key(*name)
                                })
                                && matches!(data.bind, AliasBind::Lexical { .. }) =>
                        {
                            self.module_object(module)
                        }
                        step => step,
                    },
                    RequestTarget::Script => Step::Done(Resolution::Unresolved(
                        "imported file is not a module".into(),
                    )),
                    RequestTarget::Ignored => {
                        Step::Done(Resolution::Unresolved("module is not resolved".into()))
                    }
                }
            }
            AliasKind::Namespace { request } => {
                match self.targets[alias.file.index()][*request as usize] {
                    RequestTarget::Module(module) => self.module_object(module),
                    RequestTarget::Script => Step::Done(Resolution::Unresolved(
                        "imported file is not a module".into(),
                    )),
                    RequestTarget::Ignored => {
                        Step::Done(Resolution::Unresolved("module is not resolved".into()))
                    }
                }
            }
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
                    index: *reference,
                });
                match self.done(task) {
                    Some(Resolution::Found { slots, type_only }) if *expand => {
                        Step::Done(Resolution::Found {
                            slots: self.all_meanings(*slots),
                            type_only: *type_only,
                        })
                    }
                    Some(resolution) => Step::Done(resolution.clone()),
                    None => Step::Need(task),
                }
            }
        };
        match step {
            Step::Done(resolution) => Step::Done(restrict(resolution)),
            need => need,
        }
    }

    fn try_ref(&mut self, reference: RefRef) -> Step<Resolution> {
        let facts = self.facts(reference.file);
        let data = &facts.refs[reference.index as usize];
        let name = facts.names[data.token as usize].text.as_str();
        let found = match data.base {
            RefBase::Lexical(scope) => {
                return self.lookup(reference.file, scope, name, data.meaning, data.type_context);
            }
            RefBase::Member(base) => {
                let base = Task::Ref(RefRef {
                    file: reference.file,
                    index: base,
                });
                let slots = match self.done(base) {
                    None => return Step::Need(base),
                    Some(Resolution::Found { slots, .. }) => *slots,
                    Some(Resolution::NotReference(GLOBAL_THIS)) => {
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
                let module = slots[2]
                    .or(slots[0])
                    .and_then(|container| self.symbol(container).module);
                match module {
                    Some(module) if self.modules[module].shorthand => Resolution::Unresolved(
                        format!("`{name}` is a member of a shorthand ambient module"),
                    ),
                    Some(module) => match self.resolve_export(module, name) {
                        Step::Need(task) => return Step::Need(task),
                        Step::Done(resolution) => resolution,
                    },
                    None => {
                        match self.namespace_member(slots, name, data.meaning, data.type_context) {
                            Check::Need(task) => return Step::Need(task),
                            Check::Found(resolution) => resolution,
                            Check::Missing => {
                                Resolution::Unresolved(format!("`{name}` is not exported"))
                            }
                        }
                    }
                }
            }
            RefBase::Module(request) => {
                match self.targets[reference.file.index()][request as usize] {
                    RequestTarget::Module(module) if self.modules[module].shorthand => {
                        Resolution::Unresolved(format!(
                            "`{name}` is a member of a shorthand ambient module"
                        ))
                    }
                    RequestTarget::Module(module) => match self.resolve_export(module, name) {
                        Step::Need(task) => return Step::Need(task),
                        Step::Done(resolution) => resolution,
                    },
                    _ => Resolution::Unresolved("module is not resolved".into()),
                }
            }
        };
        Step::Done(mask_resolution(
            found,
            data.meaning,
            data.type_context,
            name,
        ))
    }

    /// Resolve every searched occurrence and freeze the index.
    fn resolve_and_index(&mut self) -> Index {
        let mut index = Index {
            files: Vec::with_capacity(self.program.files.len()),
            ..Index::default()
        };
        let mut strings = Strings::default();
        for file in self.file_ids() {
            if !self.search(file) {
                index.files.push(Vec::new());
                continue;
            }
            let facts = self.facts(file);
            let mut occurrences = Vec::with_capacity(facts.names.len());
            for name in &facts.names {
                let target = match name.target {
                    NameTarget::Decl(decl) => {
                        Target::Symbols(SmallVec::from_elem(self.decl_symbol(file, decl), 1))
                    }
                    NameTarget::Ref(reference) => {
                        let resolution = self.force(Task::Ref(RefRef {
                            file,
                            index: reference,
                        }));
                        self.target(resolution, &mut strings)
                    }
                    NameTarget::Alias(alias) => {
                        let resolution = self.force(Task::Alias(AliasRef { file, index: alias }));
                        self.target(resolution, &mut strings)
                    }
                    NameTarget::NotReference(why) => Target::NotReference(why),
                };
                occurrences.push((name, target));
            }
            occurrences.sort_by_key(|(name, _)| name.span.start);
            let mut records = Vec::with_capacity(occurrences.len());
            for (position, (name, target)) in occurrences.into_iter().enumerate() {
                let reference = OccurrenceRef {
                    file,
                    index: position as u32,
                };
                if let Target::Symbols(symbols) = &target {
                    for symbol in symbols {
                        index.by_symbol.entry(*symbol).or_default().push(reference);
                    }
                    if name.candidate {
                        index
                            .by_name
                            .entry(name.text.clone())
                            .or_default()
                            .push(reference);
                    }
                } else if name.candidate {
                    index
                        .by_name
                        .entry(name.text.clone())
                        .or_default()
                        .push(reference);
                }
                records.push(Occurrence {
                    span: name.span,
                    name: strings.intern(&name.text),
                    roles: name.roles,
                    target,
                });
            }
            index.files.push(records);
        }
        let symbols: Vec<SymbolId> = index.by_symbol.keys().copied().collect();
        for symbol in symbols {
            index
                .sites
                .insert(symbol, self.symbol(symbol).sites.clone());
        }
        index.strings = strings.finish();
        index
    }

    fn target(&self, resolution: Resolution, strings: &mut Strings) -> Target {
        match resolution {
            Resolution::Found { slots, .. } => {
                let mut symbols: SmallVec<[SymbolId; 2]> = SmallVec::new();
                for symbol in slots.into_iter().flatten() {
                    let symbol = self.find(symbol);
                    if !symbols.contains(&symbol) {
                        symbols.push(symbol);
                    }
                }
                Target::Symbols(symbols)
            }
            Resolution::NotReference(why) => Target::NotReference(why),
            Resolution::Unresolved(why) => Target::Unresolved(strings.intern(&why)),
        }
    }

    /// An import alias may not share a meaning with a local declaration.
    fn check_conflicts(&mut self) -> Result<()> {
        for (table, name, file, span) in std::mem::take(&mut self.conflicts) {
            let Some(entry) = self.tables[table].get(&name) else {
                continue;
            };
            let (Some(alias), slots) = (entry.alias, entry.slots) else {
                continue;
            };
            if let Resolution::Found { slots: targets, .. } = self.force(Task::Alias(alias)) {
                let overlap = (0..3).any(|index| {
                    matches!((slots[index], targets[index]), (Some(a), Some(b)) if self.find(a) != self.find(b))
                });
                if overlap && self.search(file) {
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
    list: Option<(FileId, u32)>,
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
        Resolution::Found {
            slots,
            type_only: inner,
        } => Resolution::Found {
            slots,
            type_only: inner || type_only,
        },
        other => other,
    }
}

/// Restrict a resolution to the meaning a reference needs.
fn mask_resolution(resolution: Resolution, mask: u8, type_context: bool, name: &str) -> Resolution {
    match resolution {
        Resolution::Found { slots, type_only } => {
            let mut masked = [None; 3];
            for (index, bit) in BITS.iter().enumerate() {
                if mask & bit != 0 && !(type_only && *bit == meaning::VALUE && !type_context) {
                    masked[index] = slots[index];
                }
            }
            if masked.iter().any(Option::is_some) {
                Resolution::Found {
                    slots: masked,
                    type_only,
                }
            } else {
                Resolution::Unresolved(format!("`{name}` has no matching meaning"))
            }
        }
        other => other,
    }
}

fn is_relative(specifier: &str) -> bool {
    specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier == "."
        || specifier == ".."
        || specifier.starts_with('/')
}
