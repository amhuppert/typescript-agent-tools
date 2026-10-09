//! Project-wide declaration tables, module exports, aliases, and reference
//! resolution.
//!
//! A table entry holds one symbol per meaning plus an optional alias, so a
//! same-named type and value stay separate unless their declarations merge.
//! Aliases and qualified references resolve on demand through one explicit
//! task stack; cycles become unresolved targets rather than recursion.

use super::collect::{
    AliasBind, AliasKind, DeclKind, Facts, Host, MisplacedModule, NameTarget, RefBase, ScopeKind,
};
use super::ids::{
    AliasId, DeclId, ModuleId, RefId, RequestId, ScopeId, SymbolId, TableId, TypeParameterListId,
};
use super::index::{self, Index, Site, Target, Token};
use super::instances::Instance;
use super::store::{
    AliasRef, Direction, GLOBAL, RequestTarget, StarRequest, Store, Symbol, TwoModules,
};
use super::targets::{Meanings, Reach, Slots, each};
use super::{FileFacts, ModuleResolution, RequestKind, meaning};
use crate::Span;
use crate::analysis::{InputError, InputErrorKind};
use crate::project;
use crate::project::program::Program;
use crate::project::sources::FileId;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

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
    /// A source file or ambient module. Its symbol also carries
    /// `VALUE_MODULE`, as TypeScript marks it, so declarations merge with it
    /// or conflict with it as with a namespace.
    pub(super) const MODULE: u32 = 1 << 13;
    /// A function declaration with a body.
    pub(super) const IMPLEMENTED: u32 = 1 << 14;
    pub(super) const AMBIENT_CLASS: u32 = 1 << 15;
    pub(super) const CONCRETE_CLASS: u32 = 1 << 16;
    /// A value that is not a reference target (`ValueTarget`): a JSON
    /// module's value or property, or a library interface's member, all
    /// properties or methods to TypeScript. Only a checker merge that meets
    /// one gives it a symbol (`Linker::value_symbol`).
    pub(super) const PROPERTY: u32 = 1 << 17;
    pub(super) const ENUM: u32 = CONST_ENUM | REGULAR_ENUM;
    pub(super) const NAMESPACE: u32 = VALUE_MODULE | NAMESPACE_MODULE;
    pub(super) const VALUE: u32 = FUNCTION_SCOPED
        | BLOCK_SCOPED
        | FUNCTION
        | CLASS
        | ENUM
        | VALUE_MODULE
        | EXPRESSION
        | PROPERTY;
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

/// TypeScript's `getExcludedSymbolFlags`: the flags a symbol with `flags`
/// cannot share a name with, each kind's `*Excludes` combined.
fn excluded(flags: u32) -> u32 {
    use flags::*;
    let kinds = [
        (FUNCTION_SCOPED, VALUE & !FUNCTION_SCOPED),
        (BLOCK_SCOPED | PARAMETER | EXPRESSION, VALUE),
        (FUNCTION, VALUE & !(FUNCTION | VALUE_MODULE | CLASS)),
        (
            CLASS,
            (VALUE | TYPE) & !(VALUE_MODULE | INTERFACE | FUNCTION),
        ),
        (INTERFACE, TYPE & !(INTERFACE | CLASS)),
        (
            REGULAR_ENUM,
            (VALUE | TYPE) & !(REGULAR_ENUM | VALUE_MODULE),
        ),
        (CONST_ENUM, (VALUE | TYPE) & !CONST_ENUM),
        (
            VALUE_MODULE,
            VALUE & !(FUNCTION | CLASS | REGULAR_ENUM | VALUE_MODULE),
        ),
        (TYPE_ALIAS, TYPE),
        (TYPE_PARAMETER, TYPE & !TYPE_PARAMETER),
        (PROPERTY, VALUE & !PROPERTY),
    ];
    kinds
        .into_iter()
        .filter(|&(kind, _)| flags & kind != 0)
        .fold(0, |all, (_, excludes)| all | excludes)
}

/// Whether a symbol with `existing` flags and a declaration or symbol with
/// `new` flags conflict: TypeScript's exclusion test
/// (`getExcludedSymbolFlags`), which its binder (`declareSymbol`) and its
/// checker (`mergeSymbol`) apply alike, plus the pairs its binder merges but
/// its checker rejects (`invalid_merge`).
fn conflicts(existing: u32, new: u32, repeated_list: bool) -> bool {
    existing & excluded(new) != 0 || invalid_merge(existing, new, repeated_list)
}

/// Whether compatible declarations form one symbol (TypeScript declaration
/// merging) rather than separate same-named symbols of other meanings: a
/// `new` declaration or symbol of any kind that merges with `existing`.
fn merges(existing: u32, new: u32) -> bool {
    use flags::*;
    if new & PARAMETER != 0 {
        return false;
    }
    let namespace_like = FUNCTION | CLASS | ENUM | NAMESPACE;
    let rules = [
        (FUNCTION_SCOPED, FUNCTION_SCOPED),
        (FUNCTION, FUNCTION | CLASS | NAMESPACE),
        (CLASS, INTERFACE | FUNCTION | NAMESPACE),
        (INTERFACE, INTERFACE | CLASS),
        (REGULAR_ENUM, REGULAR_ENUM | NAMESPACE),
        (CONST_ENUM, CONST_ENUM | NAMESPACE),
        (NAMESPACE, namespace_like),
        (TYPE_PARAMETER, TYPE_PARAMETER),
    ];
    rules
        .into_iter()
        .any(|(kind, with)| new & kind != 0 && existing & with != 0)
}

/// The distinct symbols of `slots`, in slot order.
fn distinct(slots: Slots) -> SmallVec<[SymbolId; 3]> {
    let mut distinct = SmallVec::new();
    for symbol in slots.symbols() {
        if !distinct.contains(&symbol) {
            distinct.push(symbol);
        }
    }
    distinct
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
    /// A value that is not a reference target but whose identity and
    /// properties linking still needs (`ValueTarget`). It has only the value
    /// meaning; `type_only` marks a type-only route, after which only a type
    /// context may use it, as for a restricted symbol (`Meanings::select`).
    Value {
        target: ValueTarget,
        type_only: bool,
    },
    NotReference(&'static str),
    Unresolved(String),
}

/// A value that linking follows but that is not a reference target: a JSON
/// module's value or a value an object key reaches in it, or a member of the
/// library interface that is such a value's apparent type. TypeScript gives
/// each a property symbol (the file's `export=`, a transient property of the
/// value's type, an interface member), which star exports compare and whose
/// type's properties a later name selects (`Linker::value_property`). The
/// index reports it as not a reference target (`Linker::target`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ValueTarget {
    /// A value of a JSON file, by its index among the file's values
    /// (`super::JsonModule`): `JsonModule::ROOT` is the file's value.
    Json { file: FileId, value: u32 },
    /// A member of a global library interface, by the interface's canonical
    /// symbol when found. Boxed, so a resolution, which every reference's
    /// state holds, is no larger for it.
    Member { interface: SymbolId, name: Box<str> },
}

/// Whether a use with `mask` in a type context or not may select a value
/// reached through a type-only route or not: the rule `Meanings::select`
/// applies to a symbol's value meaning.
fn selects_value(mask: u8, type_only: bool, type_context: bool) -> bool {
    mask & meaning::VALUE != 0 && !(type_only && mask == meaning::VALUE && !type_context)
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

/// What a name denotes where a checker merge meets it, TypeScript's one
/// symbol: declared symbols per meaning beside an optional alias.
#[derive(Debug, Clone, Copy, Default)]
struct Held {
    slots: Slots,
    alias: Option<AliasRef>,
}

/// Where the symbols a checker merge merges into came from, which decides
/// TypeScript's response to a conflict.
#[derive(Debug, Clone, Copy)]
enum Origin {
    /// Declared symbols or none: a conflict keeps them (`return target`).
    Declared,
    /// A pure alias, resolved (`resolveSymbol`): a conflict is reported at
    /// the alias, and the source replaces it (`return source`).
    Alias(AliasRef),
    /// An unresolved or circular alias, which the source replaces without
    /// an error (`resolvedTarget == unknownSymbol`).
    Unresolved,
    /// A name the augmented module's stars supply: the source becomes the
    /// module's own export either way.
    Star(ModuleId),
}

/// A table merge a checker merge queues (`mergeSymbolTable`): each entry of
/// `from` merges into `into`, where the augmented module's `stars` may
/// supply the names `into` lacks.
#[derive(Debug, Clone, Copy)]
struct TableMerge {
    into: TableId,
    from: TableId,
    direction: Direction,
    stars: Option<ModuleId>,
}

/// Where a scope's names are stored.
#[derive(Debug, Clone, Copy)]
enum Storage {
    /// A script's top level: its own table, which merges into the global
    /// table (`merge_globals`), where its names are looked up.
    Script,
    /// The member table shared by a class or interface's declarations.
    Members(DeclId),
    /// The scope's own table, which also sees the exports of a host.
    Own(Option<Host>),
}

/// Where one of a script's ambient module declarations binds, as
/// TypeScript's binder gives each file's declarations of one name one symbol
/// in the file's locals: the module of that name for the first file in
/// program order that declares it, and for any later file a fragment, its
/// own symbol and export table, which the checker merges into the module
/// (`merge_globals`).
#[derive(Debug, Clone, Copy)]
enum AmbientHost {
    Module(ModuleId),
    /// An index into `Linker::fragments`.
    Fragment(usize),
}

/// A later file's declarations of an ambient module (`AmbientHost`).
#[derive(Debug)]
struct Fragment {
    module: ModuleId,
    symbol: SymbolId,
    /// The fragment's `export *` declarations, which the module gains when
    /// the fragment merges into it.
    stars: Vec<StarRequest>,
}

struct Linker<'p> {
    program: &'p Program,
    /// Tables, symbols, modules, and request targets, whose writes
    /// invalidate resolutions made while merges still change them.
    store: Store,
    scope_tables: Vec<Vec<Option<TableId>>>,
    /// The symbol each declaration created or merged into. It may since have
    /// merged into another, so readers canonicalize it with `find`.
    decl_symbols: Vec<Vec<Option<SymbolId>>>,
    file_modules: Vec<Option<ModuleId>>,
    ambient: HashMap<String, ModuleId>,
    /// By file, parallel to its ambient module declarations.
    ambient_hosts: Vec<Vec<AmbientHost>>,
    /// Later files' ambient module declarations, in program order.
    fragments: Vec<Fragment>,
    /// Pattern ambient modules, by name, in the order they were declared.
    patterns: Vec<(String, ModuleId)>,
    /// Requests a pattern ambient module satisfies, which an augmentation of
    /// the same specifier may redirect to a module of its own
    /// (`exact_module`).
    pattern_requests: Vec<(FileId, RequestId)>,
    /// Modules of augmented specifiers and the patterns they extend one way,
    /// in the order they were made.
    pattern_augmentations: Vec<(ModuleId, ModuleId)>,
    /// The `export *` declarations of each module augmentation symbol, which
    /// the module it augments gains when it merges.
    augmentation_stars: HashMap<SymbolId, Vec<StarRequest>>,
    /// Parallel to each file's requests: the symbol of each module
    /// augmentation, one per file and module name.
    augmentations: Vec<Vec<Option<SymbolId>>>,
    /// Each augmentation symbol with its file and first request, in program
    /// order, until it is merged.
    augmentation_order: Vec<(FileId, RequestId, SymbolId)>,
    /// By file: the symbol of its `declare global` blocks, as TypeScript's
    /// binder gives a file's global augmentations one symbol, whose exports
    /// `merge_globals` merges into the global table.
    global_augmentations: Vec<Option<SymbolId>>,
    /// By file: its UMD globals (`export as namespace`), the binder's
    /// `GlobalExports` table, which `merge_globals` adds to the global table.
    global_exports: Vec<Vec<(String, AliasRef)>>,
    alias_states: Vec<Vec<State>>,
    ref_states: Vec<Vec<State>>,
    export_memo: HashMap<(ModuleId, String), Resolution>,
    /// Tasks resolved while augmentations still change tables, which
    /// `reset_resolutions` discards once the store has changed.
    journal: Option<Vec<Task>>,
    /// The value each value symbol stands for (`value_symbol`), and the
    /// symbol of each value a checker merge met.
    values: HashMap<SymbolId, ValueTarget>,
    value_symbols: HashMap<ValueTarget, SymbolId>,
    conflicts: Vec<(TableId, String, FileId, Span)>,
    /// The task `force` is running, for `probe_entry` to consult `cuts`.
    running: Option<Task>,
    /// Fill edges that close a cycle: the waiting task resolves to its local
    /// declarations without the alias.
    cuts: HashSet<(Task, Task)>,
    /// The waiting tasks of `cuts`, whose lookups are specific to them.
    cut_tasks: HashSet<Task>,
}

type Result<T> = std::result::Result<T, InputError>;

/// Why a JSON module's value is not a reference target.
const JSON_VALUE: &str = "JSON module value";

pub(super) fn link(program: &Program) -> Result<Index> {
    let mut linker = Linker::new(program);
    linker.check_mixed_exports()?;
    linker.check_module_declarations()?;
    linker.create_modules();
    linker.create_augmentations();
    linker.resolve_requests()?;
    linker.declare_all()?;
    linker.merge_augmentations()?;
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
            store: Store::new(),
            scope_tables: files
                .iter()
                .map(|f| vec![None; f.facts.facts.scopes.len()])
                .collect(),
            decl_symbols: files
                .iter()
                .map(|f| vec![None; f.facts.facts.decls.len()])
                .collect(),
            file_modules: vec![None; files.len()],
            ambient: HashMap::new(),
            ambient_hosts: Vec::with_capacity(files.len()),
            fragments: Vec::new(),
            patterns: Vec::new(),
            pattern_requests: Vec::new(),
            pattern_augmentations: Vec::new(),
            augmentation_stars: HashMap::new(),
            augmentations: Vec::new(),
            augmentation_order: Vec::new(),
            global_augmentations: vec![None; files.len()],
            global_exports: vec![Vec::new(); files.len()],
            alias_states: files
                .iter()
                .map(|f| vec![State::Unvisited; f.facts.facts.aliases.len()])
                .collect(),
            ref_states: files
                .iter()
                .map(|f| vec![State::Unvisited; f.facts.facts.refs.len()])
                .collect(),
            export_memo: HashMap::new(),
            journal: Some(Vec::new()),
            values: HashMap::new(),
            value_symbols: HashMap::new(),
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
    /// Whether a diagnostic at `offset` of `file` fails the load: the one
    /// reporting decision (`Diagnostics::reports`), which every binding
    /// error and missing input here goes through.
    fn reports(&self, file: FileId, offset: usize) -> bool {
        self.program.files[file.index()]
            .diagnostics
            .reports(&self.program.sources.get(file).text, offset)
    }
    fn error(&self, kind: InputErrorKind, file: FileId, span: Span, message: String) -> InputError {
        let source = self.program.sources.get(file);
        InputError {
            kind,
            message,
            path: Some(source.path.clone()),
            position: Some(source.text.position(span.start)),
        }
    }

    /// The canonical symbol after merges both ways.
    fn find(&self, id: SymbolId) -> SymbolId {
        self.store.find(id)
    }
    fn symbol(&self, id: SymbolId) -> &Symbol {
        self.store.symbol(id)
    }

    fn new_module(&mut self, site: Option<Site>, shorthand: bool, declaration: bool) -> ModuleId {
        let flags = flags::MODULE | flags::VALUE_MODULE;
        self.store.new_module(flags, site, shorthand, declaration)
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
                let module = self.new_module(Some(site), false, declaration);
                if self.file(file).json.is_some() {
                    self.store.set_json(module, file);
                }
                self.file_modules[file.index()] = Some(module);
            }
            let mut own: HashMap<&'p str, AmbientHost> = HashMap::new();
            let mut hosts = Vec::new();
            for ambient in &self.facts(file).ambient_modules {
                let site = Site {
                    file,
                    span: ambient.span,
                    name: ambient.name.clone(),
                };
                let host = match own.get(ambient.name.as_str()) {
                    Some(&AmbientHost::Module(module)) => {
                        self.store.redeclare_module(module, site);
                        AmbientHost::Module(module)
                    }
                    Some(&AmbientHost::Fragment(index)) => {
                        let symbol = self.fragments[index].symbol;
                        self.store
                            .declare_into(symbol, flags::MODULE | flags::VALUE_MODULE, site);
                        AmbientHost::Fragment(index)
                    }
                    None => match self.ambient.get(&ambient.name) {
                        // The first declaration decides whether the module is
                        // shorthand.
                        None => {
                            let module = self.new_module(Some(site), ambient.shorthand, true);
                            self.ambient.insert(ambient.name.clone(), module);
                            if super::super::pattern::parse(&ambient.name).is_some() {
                                self.patterns.push((ambient.name.clone(), module));
                            }
                            AmbientHost::Module(module)
                        }
                        Some(&module) => {
                            let flags = flags::MODULE | flags::VALUE_MODULE;
                            let symbol = self.store.new_symbol(flags, Some(site));
                            self.store.export_table(symbol);
                            self.fragments.push(Fragment {
                                module,
                                symbol,
                                stars: Vec::new(),
                            });
                            AmbientHost::Fragment(self.fragments.len() - 1)
                        }
                    },
                };
                own.insert(&ambient.name, host);
                hosts.push(host);
            }
            self.ambient_hosts.push(hosts);
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
                let augmentation = matches!(request.kind, RequestKind::Augmentation { .. });
                let target = match (ambient, resolution) {
                    (Some(module), _) => RequestTarget::Module(module),
                    (None, ModuleResolution::File(target)) => {
                        match self.file_modules[target.index()] {
                            Some(module) => RequestTarget::Module(module),
                            // `merge_augmentations` decides what a module's
                            // augmentation merges into; a script is no target.
                            None if augmentation => {
                                self.augmentation_target_failed(file, index, true)?
                            }
                            None => RequestTarget::Script,
                        }
                    }
                    (None, ModuleResolution::Unchecked) => RequestTarget::Ignored,
                    (None, ModuleResolution::Missing | ModuleResolution::NotAdmitted(_)) => {
                        // Pattern ambient modules also match relative names.
                        match self.pattern(&request.specifier) {
                            Some(module) => {
                                self.pattern_requests.push((file, RequestId::new(index)));
                                RequestTarget::Module(module)
                            }
                            None if augmentation => {
                                self.augmentation_target_failed(file, index, false)?
                            }
                            // Where the error is not reported, only names
                            // imported through the request stay unresolved.
                            None if !self.reports(file, span.start) => RequestTarget::Ignored,
                            None => {
                                let message = match resolution {
                                    ModuleResolution::NotAdmitted(message) => message.clone(),
                                    _ => format!("cannot resolve module \"{}\"", request.specifier),
                                };
                                return Err(self.error(
                                    InputErrorKind::UnresolvedImport,
                                    file,
                                    span,
                                    message,
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
                            InputErrorKind::Unsupported,
                            file,
                            span,
                            format!("module \"{}\": {reason}", request.specifier),
                        ));
                    }
                };
                modules.push(target);
            }
            self.store.push_requests(modules);
        }
        Ok(())
    }

    /// The one decision for an augmentation whose target is missing (TS2664)
    /// or a script (TS2306), as TypeScript's `mergeModuleAugmentation` makes
    /// it: neither is reported where the augmentation's parent is ambient (a
    /// declaration file, or a script's ambient module), which passes no
    /// module-not-found message, nor where the reporting decision does not
    /// report it (`reports`); a tolerated failure binds nothing. Elsewhere
    /// it is an `UnresolvedImport`.
    fn augmentation_target_failed(
        &self,
        file: FileId,
        request: usize,
        script: bool,
    ) -> Result<RequestTarget> {
        let program_file = &self.program.files[file.index()];
        let request = &program_file.facts.requests[request];
        let tolerated = request.kind == (RequestKind::Augmentation { ambient: true })
            || !self.reports(file, request.span.start);
        if tolerated {
            return Ok(RequestTarget::Ignored);
        }
        let message = if script {
            format!(
                "cannot augment \"{}\", which is not a module",
                request.specifier
            )
        } else {
            format!("cannot resolve module \"{}\"", request.specifier)
        };
        Err(self.error(
            InputErrorKind::UnresolvedImport,
            file,
            request.span,
            message,
        ))
    }

    /// The module a request names, or why names imported through it are
    /// unresolved.
    fn request_module(
        &self,
        file: FileId,
        request: RequestId,
    ) -> std::result::Result<ModuleId, &'static str> {
        match self.store.request(file, request) {
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

    /// The symbol whose exports a host's export declarations bind into, as
    /// the binder declares them: a file's or ambient module's module symbol,
    /// a later file's ambient module fragment, a module augmentation's own
    /// symbol, or a namespace's.
    fn host_symbol(&self, file: FileId, host: Host) -> Option<SymbolId> {
        match host {
            Host::File => {
                self.file_modules[file.index()].map(|module| self.store.module(module).symbol)
            }
            Host::AmbientModule(index) => {
                Some(match self.ambient_hosts[file.index()][index.index()] {
                    AmbientHost::Module(module) => self.store.module(module).symbol,
                    AmbientHost::Fragment(index) => self.fragments[index].symbol,
                })
            }
            Host::Augmentation(request) => Some(self.augmentation(file, request)),
            Host::Namespace(decl) => self.decl_symbols[file.index()][decl.index()],
            Host::Global => self.global_augmentations[file.index()],
        }
    }

    /// Record each `export *` where its host's merges take it: on a file's
    /// or first-declared ambient module, which no merge combines, at once;
    /// on a later file's ambient module fragment or a module augmentation,
    /// when it merges (`merge_globals`, `merge_module_augmentation`). The
    /// module a star reaches is read through its request when used.
    fn collect_stars(&mut self) {
        for file in self.file_ids() {
            for star in &self.facts(file).stars {
                let request = StarRequest {
                    file,
                    request: star.request,
                    type_only: star.type_only,
                };
                match star.host {
                    Host::File => {
                        if let Some(module) = self.file_modules[file.index()] {
                            self.store.add_stars(module, &[request]);
                        }
                    }
                    Host::AmbientModule(index) => match self.ambient_hosts[file.index()]
                        [index.index()]
                    {
                        AmbientHost::Module(module) => self.store.add_stars(module, &[request]),
                        AmbientHost::Fragment(index) => self.fragments[index].stars.push(request),
                    },
                    Host::Augmentation(augmented) => {
                        let symbol = self.augmentation(file, augmented);
                        self.augmentation_stars
                            .entry(symbol)
                            .or_default()
                            .push(request);
                    }
                    Host::Namespace(_) | Host::Global => {}
                }
            }
        }
    }

    /// Where a scope stores its names. Every scope kind is decided here, for
    /// both declaration and lookup.
    fn storage(&self, file: FileId, scope: ScopeId) -> Storage {
        match self.facts(file).scopes[scope.index()].kind {
            ScopeKind::Root if !self.file(file).module => Storage::Script,
            ScopeKind::Global => Storage::Own(Some(Host::Global)),
            ScopeKind::Members { decl } => Storage::Members(decl),
            ScopeKind::Namespace { decl } => Storage::Own(Some(Host::Namespace(decl))),
            ScopeKind::AmbientModule { module } => Storage::Own(Some(Host::AmbientModule(module))),
            ScopeKind::Augmentation { request } => Storage::Own(Some(Host::Augmentation(request))),
            ScopeKind::Root
            | ScopeKind::Unmerged
            | ScopeKind::ScriptGlobal
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
            Storage::Members(decl) => {
                let symbol = self.decl_symbol(file, decl);
                self.store.member_table(symbol)
            }
            Storage::Script | Storage::Own(_) => {
                if let Some(table) = self.scope_tables[file.index()][scope.index()] {
                    return table;
                }
                let table = self.store.new_table();
                self.scope_tables[file.index()][scope.index()] = Some(table);
                table
            }
        }
    }

    fn decl_symbol(&self, file: FileId, decl: DeclId) -> SymbolId {
        self.find(self.decl_symbols[file.index()][decl.index()].expect("declared before use"))
    }

    /// The export table of a host, created on first use. A module
    /// augmentation's is its own symbol's until it merges, then that of what
    /// it merged into, as TypeScript reads `getMergedSymbol`.
    fn host_table(&mut self, file: FileId, host: Host) -> TableId {
        match host {
            Host::Namespace(decl) => {
                let symbol = self.decl_symbol(file, decl);
                self.store.export_table(symbol)
            }
            Host::Augmentation(request) => {
                let symbol = self.augmentation(file, request);
                self.store.export_table(symbol)
            }
            Host::Global => {
                let symbol = match self.global_augmentations[file.index()] {
                    Some(symbol) => symbol,
                    None => {
                        let symbol = self.store.new_symbol(flags::NAMESPACE_MODULE, None);
                        self.global_augmentations[file.index()] = Some(symbol);
                        symbol
                    }
                };
                self.store.export_table(symbol)
            }
            Host::File | Host::AmbientModule(_) => {
                let symbol = self
                    .host_symbol(file, host)
                    .expect("exporting files and ambient modules have symbols");
                self.store.export_table(symbol)
            }
        }
    }

    /// The export table of a host whose members a scope sees, if it exists.
    fn visible_exports(&self, file: FileId, host: Host) -> Option<TableId> {
        match host {
            Host::Namespace(decl) => self.decl_symbols[file.index()][decl.index()]
                .and_then(|symbol| self.symbol(symbol).exports),
            Host::Augmentation(request) => self.symbol(self.augmentation(file, request)).exports,
            Host::Global => self.global_augmentations[file.index()]
                .and_then(|symbol| self.symbol(symbol).exports),
            Host::File | Host::AmbientModule(_) => self
                .host_symbol(file, host)
                .and_then(|symbol| self.symbol(symbol).exports),
        }
    }

    /// Bind every declaration and alias, module augmentations into their own
    /// symbols and a script's top level into its own table. UMD globals join
    /// the global table as it is built (`merge_globals`).
    fn declare_all(&mut self) -> Result<()> {
        for file in self.file_ids() {
            let facts = self.facts(file);
            for index in 0..facts.decls.len() {
                self.declare(file, DeclId::new(index))?;
            }
            for index in 0..facts.aliases.len() {
                self.bind_alias(file, AliasId::new(index))?;
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
            (None, Some((host, name))) => {
                let table = self.host_table(file, *host);
                self.declare_in(table, None, name, declaring)?
            }
            (None, None) => unreachable!("collect binds a declaration lexically or as an export"),
        };
        self.decl_symbols[file.index()][index.index()] = Some(symbol);
        if !decl.members.is_empty() {
            self.store.add_enum_members(symbol, &decl.members);
        }
        if let Some(members) = &decl.interface_members {
            self.store.add_interface_members(symbol, members);
        }
        if let (Some(_), Some((host, name))) = (decl.lexical, &decl.export) {
            let table = self.host_table(file, *host);
            self.export_symbol(table, name, symbol, decl.kind.meanings(), file, decl.anchor)?;
        }
        Ok(())
    }

    /// Declarations of one name in one container must be all exported or
    /// all local (TypeScript's TS2395), reported at each declaration.
    fn check_mixed_exports(&self) -> Result<()> {
        for file in self.file_ids() {
            let facts = self.facts(file);
            let mut mixed = facts
                .mixed_exports
                .iter()
                .map(|&name| &facts.names[name.index()]);
            if let Some(name) = mixed.find(|name| self.reports(file, name.span.start)) {
                return Err(self.error(
                    InputErrorKind::Binding,
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

    /// Module declarations TypeScript rejects, where reported: a
    /// string-named module outside an ambient context (TS1035), a `declare
    /// global` block outside the top level of a module or of a script's
    /// top-level ambient module (TS2669), another string-named module nested
    /// where it is neither an augmentation nor an ambient module (TS2435),
    /// a script's ambient module with a relative name (TS2436), and a UMD
    /// global outside the top level of a declaration file's module (TS1314,
    /// TS1315, TS1316). `collect` keeps the names of each but a script's
    /// ambient module to their blocks, as TypeScript merges them nowhere,
    /// and declares no misplaced UMD global.
    fn check_module_declarations(&self) -> Result<()> {
        for file in self.file_ids() {
            let mut misplaced = self.facts(file).misplaced_modules.iter();
            if let Some(&(span, misplaced)) =
                misplaced.find(|(span, _)| self.reports(file, span.start))
            {
                let message = match misplaced {
                    MisplacedModule::Quoted => "only ambient modules can use quoted names",
                    MisplacedModule::Global => {
                        "a global augmentation must be at the top level of a module or directly \
                         inside an ambient module declaration"
                    }
                    MisplacedModule::Nested => {
                        "an ambient module cannot be nested in another module or namespace"
                    }
                    MisplacedModule::Relative => {
                        "an ambient module declaration cannot specify a relative module name"
                    }
                    MisplacedModule::UmdNested => "a UMD global can only be declared at top level",
                    MisplacedModule::UmdScript => "a UMD global can only be declared in a module",
                    MisplacedModule::UmdImplementation => {
                        "a UMD global can only be declared in a declaration file"
                    }
                };
                return Err(self.error(InputErrorKind::Binding, file, span, message.into()));
            }
        }
        Ok(())
    }

    // Module augmentations

    /// Give each file's augmentations of one module name one symbol, as
    /// TypeScript's binder does (`declareModuleSymbol` into the file's
    /// locals): a namespace, or a value module when any of its blocks creates
    /// values. Its declarations bind into its own export table by the
    /// ordinary binder rules, and `merge_augmentations` merges the symbol
    /// into what it augments.
    fn create_augmentations(&mut self) {
        for file in self.file_ids() {
            let facts = self.file(file);
            let mut by_name: HashMap<&'p str, SymbolId> = HashMap::new();
            let mut symbols = vec![None; facts.requests.len()];
            for (index, request) in facts.requests.iter().enumerate() {
                if !matches!(request.kind, RequestKind::Augmentation { .. }) {
                    continue;
                }
                let id = RequestId::new(index);
                let flags = if facts.facts.instantiated_augmentations.contains(&id) {
                    flags::VALUE_MODULE
                } else {
                    flags::NAMESPACE_MODULE
                };
                let site = Site {
                    file,
                    span: request.span,
                    name: request.specifier.clone(),
                };
                let symbol = match by_name.get(request.specifier.as_str()) {
                    Some(&symbol) => {
                        self.store.declare_into(symbol, flags, site);
                        symbol
                    }
                    None => {
                        let symbol = self.store.new_symbol(flags, Some(site));
                        self.store.export_table(symbol);
                        by_name.insert(&request.specifier, symbol);
                        self.augmentation_order.push((file, id, symbol));
                        symbol
                    }
                };
                symbols[index] = Some(symbol);
            }
            self.augmentations.push(symbols);
        }
    }

    /// The symbol of a module augmentation request.
    fn augmentation(&self, file: FileId, request: RequestId) -> SymbolId {
        self.augmentations[file.index()][request.index()]
            .expect("an augmentation request has a symbol")
    }

    /// Merge the global table and every module augmentation, after all
    /// declarations are bound, in TypeScript's `initializeChecker` order: the
    /// global table, including ambient modules declared in several files
    /// (`merge_globals`), then module augmentations
    /// (`mergeModuleAugmentation`), in program order and once per file and
    /// module name, at its first block. An augmentation of a specifier that
    /// only a pattern ambient module matches may make a module of that
    /// specifier at its turn (`exact_module`), which absorbs the pattern one
    /// way once every augmentation has merged and until then exposes its
    /// exports through it. Finding what a declaration merges with resolves
    /// aliases while later merges still change tables; the store marks every
    /// write, so a resolution made before one is redone (`force`), and every
    /// resolution is redone once merging ends, when the store is frozen.
    fn merge_augmentations(&mut self) -> Result<()> {
        self.collect_stars();
        self.merge_globals()?;
        for (file, request, augmentation) in std::mem::take(&mut self.augmentation_order) {
            self.merge_module_augmentation(file, request, augmentation)?;
        }
        self.absorb_patterns()?;
        self.reset_resolutions();
        self.journal = None;
        self.store.freeze();
        Ok(())
    }

    /// TypeScript's `initializeChecker` builds the global table before it
    /// merges module augmentations. File by file in program order, a
    /// script's declarations merge into it (`mergeGlobalSymbol`, the
    /// checker's `mergeSymbol`), and then a UMD global (`export as
    /// namespace`) enters it only if no global has its name yet. Then each
    /// file's `declare global` blocks merge into it as a module augmentation
    /// does (`mergeModuleAugmentation` with `mergeSymbolTable`). Last, the
    /// ambient modules it deferred (`ambientModuleSymbols`): in program order,
    /// each later file's declarations of an ambient module name, a fragment
    /// (`AmbientHost`), merge into the module of that name by
    /// `mergeGlobalSymbol` as a script's other declarations do, so its first
    /// declaration still decides whether the module is shorthand
    /// (`isShorthandAmbientModuleSymbol`). Declarations of one table, also
    /// one file's ambient modules of one name, were bound by the binder's
    /// rule (`combine`), as TypeScript's binder binds them.
    fn merge_globals(&mut self) -> Result<()> {
        for file in self.file_ids() {
            if !self.file(file).module {
                let script = self.scope_tables[file.index()].first().copied().flatten();
                if let Some(from) = script {
                    self.run_merges(vec![TableMerge {
                        into: GLOBAL,
                        from,
                        direction: Direction::Both,
                        stars: None,
                    }])?;
                }
            }
            for (name, alias) in std::mem::take(&mut self.global_exports[file.index()]) {
                let taken = self
                    .store
                    .entry(GLOBAL, &name)
                    .is_some_and(|entry| !entry.slots.is_empty() || entry.alias.is_some());
                if !taken {
                    self.store.set_alias(GLOBAL, &name, Some(alias));
                }
            }
        }
        for file in self.file_ids() {
            let Some(symbol) = self.global_augmentations[file.index()] else {
                continue;
            };
            let from = self.store.export_table(symbol);
            self.run_merges(vec![TableMerge {
                into: GLOBAL,
                from,
                direction: Direction::Both,
                stars: None,
            }])?;
        }
        for index in 0..self.fragments.len() {
            let Fragment { module, symbol, .. } = self.fragments[index];
            let stars = std::mem::take(&mut self.fragments[index].stars);
            let target = self.store.module(module).symbol;
            let target = Slots::only(self.meanings_of(target), target);
            self.mark_merged_into(target);
            let mut work = Vec::new();
            self.merge_symbol(target, symbol, Direction::Both, None, &mut work)?;
            self.run_merges(work)?;
            self.store.add_stars(module, &stars);
        }
        Ok(())
    }

    /// TypeScript's `mergeModuleAugmentation` for one file's augmentations
    /// of one module name: the augmented module (`exact_module`), or the
    /// target of its `export =` (`resolveExternalModuleSymbol`), which must
    /// be a namespace, module, or enum (TS2671), merges with the
    /// augmentation's symbol (`mergeSymbol`). Its exclusion test fails for a
    /// target that also declares a variable or const enum when the
    /// augmentation creates values: TS2649 beside a namespace, TS2567 for a
    /// const enum. A failure is a `Binding` error where reported and
    /// otherwise merges nothing, as does an ignored request.
    fn merge_module_augmentation(
        &mut self,
        file: FileId,
        request: RequestId,
        augmentation: SymbolId,
    ) -> Result<()> {
        let Ok(module) = self.request_module(file, request) else {
            return Ok(());
        };
        let module = self.exact_module(file, request, module, augmentation);
        if let Some(stars) = self.augmentation_stars.remove(&augmentation) {
            self.store.add_stars(module, &stars);
        }
        let data = &self.file(file).requests[request.index()];
        // A JSON module's `export=` is its value, a property.
        let json = self.store.module(module).json.is_some();
        let target = if json || self.exports_have(module, "export=") {
            let resolution = if let Some(file) = self.store.module(module).json {
                Self::json_value(file)
            } else {
                self.settle(|linker| match linker.probe_export(module, "export=") {
                    Probe::Found(resolution) => Step::Done(resolution),
                    Probe::Need(need) => Step::Need(need),
                    Probe::Missing => Step::Done(Resolution::Unresolved("no export =".into())),
                })
            };
            let target = match resolution {
                Resolution::Found(found) => found.targets().map(|symbol| self.find(symbol)),
                _ => Slots::default(),
            };
            if self.union_flags(target) & (flags::NAMESPACE | flags::ENUM | flags::MODULE) == 0 {
                if !self.reports(file, data.span.start) {
                    return Ok(());
                }
                return Err(self.error(
                    InputErrorKind::Binding,
                    file,
                    data.span,
                    format!(
                        "cannot augment \"{}\" because it resolves to a non-module entity",
                        data.specifier
                    ),
                ));
            }
            target
        } else {
            let symbol = self.store.module(module).symbol;
            Slots::only(self.meanings_of(symbol), symbol)
        };
        if self.excluding(target, augmentation).is_some() {
            let name = format!("\"{}\"", data.specifier);
            let values = self.union_flags(target) & flags::NAMESPACE_MODULE != 0;
            let sources = self.sites_of(Slots::only(meaning::NAMESPACE, augmentation));
            return self.reject(&name, &sources, &self.sites_of(target), values);
        }
        self.mark_merged_into(target);
        // `export *` supplies names only to a module.
        let stars = target
            .container()
            .and_then(|container| self.store.symbol(container).module);
        let mut work = Vec::new();
        self.merge_symbol(target, augmentation, Direction::Both, stars, &mut work)?;
        self.run_merges(work)
    }

    /// The module an augmentation through `request` merges into, `module` as
    /// the request now reaches it, unless only a pattern ambient module
    /// matches the specifier and no checker merge has merged into that
    /// pattern yet (`Module::merged_into`): TypeScript then merges the
    /// pattern one way (`mergeModuleAugmentation` with `unidirectional`) into
    /// a module of the exact specifier (`patternAmbientModuleAugmentations`),
    /// which `resolveExternalModule` returns for it from then on, while other
    /// specifiers keep the pattern. It is made at this augmentation's turn,
    /// so earlier merges saw the pattern; once a merge changed the pattern,
    /// TypeScript's resolved module is no longer the pattern's own symbol,
    /// and the augmentation merges into the pattern both ways. A pattern with
    /// `export =` is augmented through its target instead. The specifier's
    /// module has the pattern's exports from the start, as TypeScript's does:
    /// until it absorbs the pattern (`absorb_patterns`), its exports and
    /// stars complete the module's own (`probe_export`, `module_stars`). Its
    /// first value declaration, which decides whether it is shorthand, is the
    /// augmentation's when it declares values, and otherwise the pattern's.
    fn exact_module(
        &mut self,
        file: FileId,
        request: RequestId,
        module: ModuleId,
        augmentation: SymbolId,
    ) -> ModuleId {
        let pattern = module;
        let matched = self.pattern_requests.contains(&(file, request))
            && self.patterns.iter().any(|&(_, module)| module == pattern);
        if !matched
            || self.store.module(pattern).merged_into
            || self.exports_have(pattern, "export=")
        {
            return module;
        }
        let shorthand = self.store.module(pattern).shorthand
            && self.store.symbol(augmentation).flags & flags::VALUE_MODULE == 0;
        let exact = self.new_module(None, shorthand, true);
        self.store.set_pattern(exact, Some(pattern));
        self.pattern_augmentations.push((exact, pattern));
        let specifier = &self.file(file).requests[request.index()].specifier;
        for &(other, other_request) in &self.pattern_requests {
            if self.file(other).requests[other_request.index()].specifier == *specifier {
                self.store
                    .set_request(other, other_request, RequestTarget::Module(exact));
            }
        }
        exact
    }

    /// Record that a checker merge merges into each module among `targets`
    /// (`Module::merged_into`).
    fn mark_merged_into(&mut self, targets: Slots) {
        for symbol in distinct(targets) {
            if let Some(module) = self.store.symbol(symbol).module {
                self.store.set_merged_into(module);
            }
        }
    }

    /// Extend each augmented specifier's module one way with its pattern's
    /// stars, declarations, and exports (`mergeSymbol` with
    /// `unidirectional`), after every augmentation of the specifier merged
    /// into it, so that no augmentation changes the pattern's own symbols;
    /// the module's exports then no longer need the pattern to complete
    /// them.
    fn absorb_patterns(&mut self) -> Result<()> {
        for (module, pattern) in std::mem::take(&mut self.pattern_augmentations) {
            let stars = self.store.module(pattern).stars.clone();
            self.store.add_stars(module, &stars);
            let target = self.store.module(module).symbol;
            let source = self.store.module(pattern).symbol;
            let target = Slots::only(self.meanings_of(target), target);
            let mut work = Vec::new();
            self.merge_symbol(target, source, Direction::OneWay, None, &mut work)?;
            self.run_merges(work)?;
            self.store.set_pattern(module, None);
        }
        Ok(())
    }

    /// Merge the tables a checker merge queued (`mergeSymbolTable`), each
    /// entry of `from` into `into`, depth first as TypeScript recurses: the
    /// tables an entry's merge queues merge before the next entry. An
    /// explicit stack keeps deep namespaces off the call stack. Merging both
    /// ways leaves `from` as it is: nothing reaches it through the merged
    /// identity, and conflicts recorded against it are still checked. A pair
    /// of tables merges once, so tables that reach each other, as a namespace
    /// exporting an alias of itself can, still finish.
    fn run_merges(&mut self, work: Vec<TableMerge>) -> Result<()> {
        let mut merged = HashSet::new();
        let mut stack: Vec<(TableMerge, std::vec::IntoIter<(String, Held)>)> = Vec::new();
        let mut queued = work;
        loop {
            for merge in queued.drain(..).rev() {
                if merge.into == merge.from || !merged.insert((merge.into, merge.from)) {
                    continue;
                }
                let mut entries: Vec<(String, Held)> = self
                    .store
                    .table(merge.from)
                    .iter()
                    .map(|(name, entry)| {
                        let held = Held {
                            slots: entry.slots,
                            alias: entry.alias,
                        };
                        (name.clone(), held)
                    })
                    .collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                stack.push((merge, entries.into_iter()));
            }
            let Some((merge, entries)) = stack.last_mut() else {
                return Ok(());
            };
            let merge = *merge;
            match entries.next() {
                Some((name, source)) => self.merge_entry(
                    merge.into,
                    &name,
                    source,
                    merge.direction,
                    merge.stars,
                    &mut queued,
                )?,
                None => {
                    stack.pop();
                }
            }
        }
    }

    /// TypeScript's `mergeSymbolTable` and `mergeSymbol` for one name: the
    /// one checker merge, for the global table, ambient modules declared in
    /// several files, module augmentations, the namespaces nested in them,
    /// and pattern ambient modules. `source` is what a table declares as
    /// `name`; it merges into what the entry of `into` denotes, which
    /// TypeScript holds as one symbol and this table as symbols per meaning
    /// beside an optional alias:
    ///
    /// - Declared symbols, with any alias the binder merged beside them,
    ///   merge as they are.
    /// - A pure alias is resolved first (`resolveSymbol`); the source merges
    ///   with its targets, which replace the alias without its type-only
    ///   restriction. A value the alias resolves to, such as a JSON module's
    ///   value or property, is a target as well (`value_symbol`), a property
    ///   whose flags the exclusion test reads. The source alone replaces an
    ///   unresolved or circular alias.
    /// - A name that only the augmented module's `export *` supplies merges
    ///   with the symbols, or value, the stars resolve it to, which become the
    ///   module's own export (the `resolvedExports` merge).
    /// - Otherwise the source's symbols become the entry with their identity.
    ///
    /// Each source symbol merges with the targets it may merge with
    /// (`merges`) or stays a separate symbol of its other meanings. A source
    /// alias stays the merged entry's alias, which resolves the meanings the
    /// entry lacks, and becomes an alias declaration of the entry's symbols
    /// (`Symbol::aliases`): TypeScript's merged symbol has the alias among its
    /// declarations, but what the alias names is not one of them. An
    /// excluded pair, or an alias meeting an alias, rejects the whole merge
    /// (`conflicts`): a `Binding` error where TypeScript reports one in a
    /// checked file, and otherwise TypeScript's choice of one side.
    fn merge_entry(
        &mut self,
        into: TableId,
        name: &str,
        source: Held,
        direction: Direction,
        stars: Option<ModuleId>,
        work: &mut Vec<TableMerge>,
    ) -> Result<()> {
        let held = self.held(into, name);
        let source_slots = source.slots.map(|symbol| self.find(symbol));
        let alias_clash = source.alias.is_some() && held.alias.is_some();
        let (targets, origin) = if !held.slots.is_empty() || alias_clash {
            (held.slots, Origin::Declared)
        } else if let Some(alias) = held.alias {
            match self.force(Task::Alias(alias)) {
                Resolution::Found(found) => (
                    found.targets().map(|symbol| self.find(symbol)),
                    Origin::Alias(alias),
                ),
                Resolution::Value { target, .. } => (
                    Slots::only(meaning::VALUE, self.value_symbol(target)),
                    Origin::Alias(alias),
                ),
                _ => (Slots::default(), Origin::Unresolved),
            }
        } else if let Some(module) = stars.filter(|&module| !self.module_stars(module).is_empty()) {
            match self.settle(|linker| linker.star_export(module, name, false)) {
                Resolution::Found(found) => (
                    found.targets().map(|symbol| self.find(symbol)),
                    Origin::Star(module),
                ),
                Resolution::Value { target, .. } => (
                    Slots::only(meaning::VALUE, self.value_symbol(target)),
                    Origin::Star(module),
                ),
                _ => (Slots::default(), Origin::Declared),
            }
        } else {
            (Slots::default(), Origin::Declared)
        };
        let sources = distinct(source_slots);
        if alias_clash
            || sources
                .iter()
                .any(|&symbol| self.excluding(targets, symbol).is_some())
        {
            let mut source_sites = self.sites_of(source_slots);
            source_sites.extend(source.alias.map(|alias| self.alias_site(alias)));
            let (target_sites, values) = match origin {
                // TypeScript reports a conflict with an alias at the alias,
                // and one with a star export at the export the star supplies.
                Origin::Alias(alias) => (vec![self.alias_site(alias)], false),
                Origin::Star(module) => (self.star_export_sites(module, name), false),
                _ => {
                    let mut sites = self.sites_of(targets);
                    sites.extend(held.alias.map(|alias| self.alias_site(alias)));
                    let namespace = self.union_flags(targets) & flags::NAMESPACE_MODULE != 0;
                    (sites, namespace && !alias_clash)
                }
            };
            self.reject(&format!("`{name}`"), &source_sites, &target_sites, values)?;
            // TypeScript keeps a declared target; a resolved alias or star
            // export gives way to the source.
            if matches!(origin, Origin::Alias(_) | Origin::Star(_)) {
                self.store.set_alias(into, name, source.alias);
                for meaning in each(meaning::ALL) {
                    if let Some(symbol) = source_slots.get(meaning) {
                        self.store.hold(into, name, meaning, symbol);
                    }
                }
            }
            return Ok(());
        }
        self.mark_merged_into(targets);
        let reached = targets
            .symbols()
            .fold(0, |meanings, symbol| meanings | self.meanings_of(symbol));
        if !matches!(origin, Origin::Declared) {
            // TypeScript stores the merged symbol where the alias was.
            self.store.set_alias(into, name, None);
            for meaning in each(meaning::ALL) {
                if let Some(symbol) = targets.get(meaning) {
                    self.store.hold(into, name, meaning, symbol);
                }
            }
        }
        for symbol in sources {
            let brought = self.meanings_of(symbol);
            let current = self.held(into, name).slots;
            let merged = self.merge_symbol(current, symbol, direction, None, work)?;
            self.occupy(into, name, merged);
            self.debug_assert_placed(into, name, brought, merged);
        }
        if let Some(alias) = source.alias {
            self.store.set_alias(into, name, Some(alias));
            for symbol in distinct(self.held(into, name).slots) {
                self.store.add_alias(symbol, alias);
            }
            // The merged symbol is the alias's own, so TS2440 applies to it
            // as to a same-table alias; one way, the alias keeps its own.
            if direction == Direction::Both && !self.held(into, name).slots.is_empty() {
                let site = self.alias_site(alias);
                self.conflicts
                    .push((into, name.to_string(), site.0, site.1));
            }
        }
        let merged = self.held(into, name).slots;
        let meanings = each(meaning::ALL)
            .filter(|&meaning| merged.get(meaning).is_some())
            .fold(0, |meanings, meaning| meanings | meaning);
        if reached != 0 && meanings & !reached != 0 {
            self.propagate(targets, merged);
        }
        Ok(())
    }

    /// The symbol `source` forms with `targets` in a checker merge whose
    /// exclusion test passed: every target it may merge with (`merges`)
    /// becomes one symbol that gains it (`merge_into`); otherwise it stays a
    /// separate symbol of other meanings.
    fn merge_symbol(
        &mut self,
        targets: Slots,
        source: SymbolId,
        direction: Direction,
        stars: Option<ModuleId>,
        work: &mut Vec<TableMerge>,
    ) -> Result<SymbolId> {
        let source = self.find(source);
        let flags = self.store.symbol(source).flags;
        let with: SmallVec<[SymbolId; 3]> = distinct(targets.map(|symbol| self.find(symbol)))
            .into_iter()
            .filter(|&target| target != source && merges(self.store.symbol(target).flags, flags))
            .collect();
        let Some((&first, rest)) = with.split_first() else {
            return Ok(source);
        };
        for &other in rest {
            self.merge_symbols(first, other)?;
        }
        let first = self.find(first);
        self.merge_into(first, source, direction, stars, work)?;
        Ok(self.find(first))
    }

    /// TypeScript's `mergeSymbol` once it merges: `target` gains `source`'s
    /// flags, declarations, type parameter lists, and enum members
    /// (`transfer`), and their member and export tables merge entry by entry
    /// (`mergeSymbolTable`), the exports with `stars` of the augmented
    /// module. Both directions make one identity (`recordMergedSymbol`); one
    /// way leaves `source` as it was.
    fn merge_into(
        &mut self,
        target: SymbolId,
        source: SymbolId,
        direction: Direction,
        stars: Option<ModuleId>,
        work: &mut Vec<TableMerge>,
    ) -> Result<()> {
        let (target, source) = match direction {
            Direction::Both => match self.unite(target, source)? {
                Some(pair) => pair,
                None => return Ok(()),
            },
            Direction::OneWay => (self.find(target), self.find(source)),
        };
        if target == source {
            return Ok(());
        }
        let (members, exports) = self.store.transfer(target, source, direction);
        if let Some(from) = members {
            let into = self.store.member_table(target);
            work.push(TableMerge {
                into,
                from,
                direction,
                stars: None,
            });
        }
        if let Some(from) = exports {
            let into = self.store.export_table(target);
            work.push(TableMerge {
                into,
                from,
                direction,
                stars,
            });
        }
        Ok(())
    }

    /// Give every entry that holds a symbol of `reached` the meanings of
    /// `merged` it lacks. TypeScript's merged symbol has the meanings of
    /// every declaration merged into it wherever it is found
    /// (`getMergedSymbol`); here an entry holds a symbol per meaning, so each
    /// entry that holds a reached symbol, in the tables recorded as its homes,
    /// gains the
    /// merged entry's other meanings, which may be separate symbols, such as
    /// an interface beside a constant. A module symbol keeps them for its
    /// module object and shorthand exports, which no entry holds.
    fn propagate(&mut self, reached: Slots, merged: Slots) {
        let reached = distinct(reached.map(|symbol| self.find(symbol)));
        for &symbol in &reached {
            for table in self.store.symbol(symbol).homes.clone() {
                let entries: Vec<(String, Slots)> = self
                    .store
                    .table(table)
                    .iter()
                    .map(|(name, entry)| (name, entry.slots.map(|other| self.find(other))))
                    .filter(|(_, held)| held.symbols().any(|other| reached.contains(&other)))
                    .map(|(name, held)| (name.clone(), held))
                    .collect();
                for (name, held) in entries {
                    for meaning in each(meaning::ALL) {
                        let free = held
                            .get(meaning)
                            .is_none_or(|other| self.meanings_of(other) & meaning == 0);
                        if let (true, Some(symbol)) = (free, merged.get(meaning)) {
                            self.store.hold(table, &name, meaning, symbol);
                        }
                    }
                }
            }
            if let Some(module) = self.store.symbol(symbol).module {
                let own = self.meanings_of(symbol);
                for meaning in each(meaning::ALL).filter(|&meaning| own & meaning == 0) {
                    if let Some(other) = merged.get(meaning) {
                        self.store.set_merged(module, meaning, other);
                    }
                }
            }
        }
    }

    /// Report a checker merge TypeScript rejects, where it reports one of
    /// the declarations it names (`reports`): TS2649 (`values`) at the
    /// source's first declaration only, TS2300, TS2451, or TS2567 at the
    /// declarations of both sides.
    fn reject(
        &self,
        name: &str,
        sources: &[(FileId, Span)],
        targets: &[(FileId, Span)],
        values: bool,
    ) -> Result<()> {
        let (reported, message) = if values {
            (
                &sources[..sources.len().min(1)],
                format!(
                    "cannot augment {name} with values, because it resolves to a non-module entity"
                ),
            )
        } else {
            (sources, format!("duplicate declaration of {name}"))
        };
        let others = if values { &[][..] } else { targets };
        match reported
            .iter()
            .chain(others)
            .find(|&&(file, span)| self.reports(file, span.start))
        {
            Some(&(file, span)) => Err(self.error(InputErrorKind::Binding, file, span, message)),
            None => Ok(()),
        }
    }

    /// The declared symbols and alias of `table`'s entry for `name`, with
    /// symbols canonical.
    fn held(&self, table: TableId, name: &str) -> Held {
        self.store
            .entry(table, name)
            .map(|entry| Held {
                slots: entry.slots.map(|symbol| self.find(symbol)),
                alias: entry.alias,
            })
            .unwrap_or_default()
    }

    /// The flags of every symbol in `slots`: TypeScript's one symbol.
    fn union_flags(&self, slots: Slots) -> u32 {
        slots
            .symbols()
            .fold(0, |all, symbol| all | self.symbol(symbol).flags)
    }

    /// The first symbol of `targets` whose flags exclude `source`'s, or that
    /// it may not merge with (`conflicts`).
    fn excluding(&self, targets: Slots, source: SymbolId) -> Option<SymbolId> {
        let flags = self.symbol(source).flags;
        let source = self.find(source);
        targets
            .symbols()
            .map(|symbol| self.find(symbol))
            .find(|&target| {
                target != source && conflicts(self.store.symbol(target).flags, flags, false)
            })
    }

    /// The declaration sites of the symbols in `slots`.
    fn sites_of(&self, slots: Slots) -> Vec<(FileId, Span)> {
        distinct(slots.map(|symbol| self.find(symbol)))
            .into_iter()
            .flat_map(|symbol| self.store.symbol(symbol).sites.iter())
            .map(|site| (site.file, site.span))
            .collect()
    }

    /// The declarations of the export that `module`'s stars supply as
    /// `name`, where TypeScript reports a conflict with it: the first in star
    /// order that has an entry for the name (`getExportsOfModule`), its
    /// declared symbols' sites and its alias.
    fn star_export_sites(&self, module: ModuleId, name: &str) -> Vec<(FileId, Span)> {
        let mut seen = HashSet::from([module]);
        let mut stack: Vec<ModuleId> = self
            .module_stars(module)
            .iter()
            .rev()
            .map(|&(star, _)| star)
            .collect();
        while let Some(star) = stack.pop() {
            if !seen.insert(star) {
                continue;
            }
            let exports = self.store.module(star).exports;
            if let Some(entry) = self.store.entry(exports, name) {
                let mut sites = self.sites_of(entry.slots);
                sites.extend(entry.alias.map(|alias| self.alias_site(alias)));
                return sites;
            }
            stack.extend(self.module_stars(star).iter().rev().map(|&(next, _)| next));
        }
        Vec::new()
    }

    /// The symbol a checker merge holds for a value it meets
    /// (`ValueTarget`): one per value, a property with the value meaning, so
    /// that the merge reads its exclusion flags and keeps it beside the
    /// declarations it merges with, while queries still report it as no
    /// reference target (`target`). A library member is keyed by its
    /// interface's canonical symbol.
    fn value_symbol(&mut self, target: ValueTarget) -> SymbolId {
        let target = match target {
            ValueTarget::Member { interface, name } => ValueTarget::Member {
                interface: self.find(interface),
                name,
            },
            json => json,
        };
        if let Some(&symbol) = self.value_symbols.get(&target) {
            return symbol;
        }
        let symbol = self.store.new_symbol(flags::PROPERTY, None);
        self.values.insert(symbol, target.clone());
        self.value_symbols.insert(target, symbol);
        symbol
    }

    /// The value a value symbol stands for (`value_symbol`).
    fn value_of(&self, symbol: SymbolId) -> Option<&ValueTarget> {
        self.values.get(&self.find(symbol))
    }

    fn alias_site(&self, alias: AliasRef) -> (FileId, Span) {
        (
            alias.file,
            self.facts(alias.file).aliases[alias.id.index()].span,
        )
    }

    /// Every meaning a declaration or merged symbol brings is placed, merged,
    /// or rejected: the entry holds `symbol` in each of `meanings`.
    fn debug_assert_placed(&self, table: TableId, name: &str, meanings: u8, symbol: SymbolId) {
        if cfg!(debug_assertions) {
            let held = self.held(table, name).slots;
            let symbol = self.find(symbol);
            for meaning in each(meanings) {
                assert_eq!(
                    held.get(meaning),
                    Some(symbol),
                    "`{name}` is neither placed, merged, nor rejected in meaning {meaning}"
                );
            }
        }
    }

    /// Run `step` to completion, resolving first each task it needs.
    fn settle(&mut self, mut step: impl FnMut(&mut Self) -> Step<Resolution>) -> Resolution {
        if self.store.take_changed() {
            self.reset_resolutions();
        }
        loop {
            match step(self) {
                Step::Done(resolution) => return resolution,
                Step::Need(need) => {
                    self.force(need.task);
                }
            }
        }
    }

    /// Discard every resolution made while augmentations change tables, so
    /// that each is redone against the tables as they now are.
    fn reset_resolutions(&mut self) {
        self.store.take_changed();
        let tasks = self
            .journal
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        for task in tasks {
            match task {
                Task::Alias(a) => {
                    self.alias_states[a.file.index()][a.id.index()] = State::Unvisited
                }
                Task::Ref(r) => self.ref_states[r.file.index()][r.id.index()] = State::Unvisited,
            }
        }
        self.export_memo.clear();
        self.cuts.clear();
        self.cut_tasks.clear();
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
                        .and_then(|table| self.store.table(table).get(&decl.name))
                        .and_then(|entry| entry.slots.value())
                        .is_some_and(|symbol| self.symbol(symbol).flags & flags::BLOCK_SCOPED != 0);
                    if blocked && self.reports(file, decl.anchor.start) {
                        return Err(self.error(
                            InputErrorKind::Binding,
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
    /// parameters differ (TS2428). TypeScript checks a declaration against
    /// the symbol it denotes (`getSymbolOfDeclaration`, the view a merge gave
    /// it): that symbol's own declarations, including those a one-way merge
    /// copied into it, but not what its alias declarations name. So each
    /// view is checked from the declarations that select it and that the
    /// binding diagnostic policy covers, and an error is reported at one of
    /// them; a declaration copied one way into another view does not select
    /// that view.
    fn check_merges(&self) -> Result<()> {
        let decl = |(file, index): (FileId, DeclId)| &self.facts(file).decls[index.index()];
        let merged = |kind: DeclKind| {
            matches!(
                kind,
                DeclKind::Class
                    | DeclKind::Interface
                    | DeclKind::Function
                    | DeclKind::Namespace { .. }
            )
        };
        // The declarations these checks read, by the site of their name.
        let mut declarations: HashMap<(FileId, usize), DeclId> = HashMap::new();
        for file in self.file_ids() {
            for (index, decl) in self.facts(file).decls.iter().enumerate() {
                if merged(decl.kind) {
                    declarations.insert((file, decl.anchor.start), DeclId::new(index));
                }
            }
        }
        // The views that checked declarations select, in program order.
        let mut views: Vec<SymbolId> = Vec::new();
        let mut seen = HashSet::new();
        for file in self.file_ids() {
            for (index, decl) in self.facts(file).decls.iter().enumerate() {
                if merged(decl.kind)
                    && self.decl_symbols[file.index()][index].is_some()
                    && self.reports(file, decl.anchor.start)
                {
                    let view = self.decl_symbol(file, DeclId::new(index));
                    if seen.insert(view) {
                        views.push(view);
                    }
                }
            }
        }
        for view in views {
            let symbol = self.symbol(view);
            if symbol.sites.len() < 2 {
                continue;
            }
            let mut list: Vec<(FileId, DeclId)> = Vec::new();
            for site in &symbol.sites {
                if let Some(&decl) = declarations.get(&(site.file, site.span.start)) {
                    if !list.contains(&(site.file, decl)) {
                        list.push((site.file, decl));
                    }
                }
            }
            if list.len() < 2 {
                continue;
            }
            let selects = |&(file, index): &(FileId, DeclId)| {
                self.reports(file, decl((file, index)).anchor.start)
                    && self.decl_symbol(file, index) == view
            };
            let anchor = list.iter().copied().find(|&(file, index)| {
                let decl = decl((file, index));
                !decl.ambient && matches!(decl.kind, DeclKind::Class | DeclKind::Function)
            });
            for &(file, index) in list.iter().filter(|key| selects(key)) {
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
                if !instantiated || namespace.ambient {
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
                    InputErrorKind::Binding,
                    file,
                    namespace.anchor,
                    format!("{message}: `{}`", namespace.name),
                ));
            }
            let generic: Vec<((FileId, DeclId), Vec<TypeParameter<'_>>)> = list
                .iter()
                .copied()
                .filter_map(|key| {
                    let parameters = decl(key).type_parameters.as_deref()?;
                    let parameters = parameters
                        .iter()
                        .map(|(name, default)| TypeParameter {
                            symbol: self.type_parameter(key, name),
                            name,
                            default: *default,
                        })
                        .collect();
                    Some((key, parameters))
                })
                .collect();
            if !identical_type_parameters(generic.iter().map(|(_, list)| list.as_slice())) {
                // TypeScript reports every declaration of the view.
                if let Some(&((file, index), _)) = generic.iter().find(|(key, _)| selects(key)) {
                    let declaration = decl((file, index));
                    return Err(self.error(
                        InputErrorKind::Binding,
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

    /// The type parameter `name` of a class or interface declaration: what
    /// the members of the declaration's own symbol hold, which declarations
    /// merged both ways share and a one-way merge keeps apart.
    fn type_parameter(&self, (file, decl): (FileId, DeclId), name: &str) -> Option<SymbolId> {
        let owner = self.decl_symbol(file, decl);
        let members = self.store.symbol(owner).members?;
        let entry = self.store.table(members).get(name)?;
        entry
            .slots
            .get(meaning::TYPE)
            .map(|symbol| self.find(symbol))
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
        let entry = self.store.table(table).get(name);
        let has_alias = entry.is_some_and(|entry| entry.alias.is_some());
        let mut slots = entry.map(|entry| entry.slots).unwrap_or_default();
        if slots.value().is_none() {
            if let Some(parameter) = parameters
                .and_then(|parameters| self.store.table(parameters).get(name))
                .and_then(|entry| entry.slots.value())
            {
                slots.set(meaning::VALUE, parameter);
            }
        }
        let (file, span) = (declaring.site.file, declaring.site.span);
        let brought = meanings(declaring.flags);
        let symbol = self.combine(slots, name, declaring)?;
        self.occupy(table, name, symbol);
        self.debug_assert_placed(table, name, brought, symbol);
        if has_alias {
            self.conflicts.push((table, name.to_string(), file, span));
        }
        Ok(symbol)
    }

    /// The symbol a declaration of `name` forms with the same-named symbols
    /// it meets in one table, `existing`: one it merges with, or a new one,
    /// as TypeScript's binder decides (`declareSymbol`) by the one exclusion
    /// and merge test (`conflicts`, `merges`). Excluded pairs fail where
    /// bindings are checked (TS2300, TS2451, TS2567); declarations of other
    /// meanings stay separate symbols. Module augmentations merge by the
    /// checker's rule instead (`merge_entry`).
    fn combine(&mut self, existing: Slots, name: &str, declaring: Declaring) -> Result<SymbolId> {
        let Declaring {
            flags: declared,
            list,
            site,
        } = declaring;
        let mut merge_with: SmallVec<[SymbolId; 3]> = SmallVec::new();
        for symbol in distinct(existing.map(|symbol| self.find(symbol))) {
            let flags = self.store.symbol(symbol).flags;
            let repeated = list.is_some_and(|list| self.store.symbol(symbol).lists.contains(&list));
            if conflicts(flags, declared, repeated) {
                if let Some((file, span)) = self.reported(symbol, (site.file, site.span)) {
                    return Err(self.error(
                        InputErrorKind::Binding,
                        file,
                        span,
                        format!("duplicate declaration of `{name}`"),
                    ));
                }
                merge_with.push(symbol);
            } else if merges(flags, declared) {
                merge_with.push(symbol);
            }
        }
        let symbol = match merge_with.split_first() {
            None => self.store.new_symbol(declared, Some(site)),
            Some((&first, rest)) => {
                for &other in rest {
                    self.merge_symbols(first, other)?;
                }
                let first = self.find(first);
                self.store.declare_into(first, declared, site);
                first
            }
        };
        if let Some(list) = list {
            self.store.add_list(symbol, list);
        }
        Ok(symbol)
    }

    /// `symbol` takes each of its meanings in `table`'s entry for `name` that
    /// is free, held by a symbol now merged into it, or held only by the
    /// context of a symbol without that meaning of its own: any export of a
    /// shorthand ambient module has every meaning, but a declaration merged
    /// beside the module takes its own.
    fn occupy(&mut self, table: TableId, name: &str, symbol: SymbolId) {
        let symbol = self.find(symbol);
        let held = self.held(table, name).slots;
        for meaning in each(self.meanings_of(symbol)) {
            let free = held
                .get(meaning)
                .is_none_or(|other| other == symbol || self.meanings_of(other) & meaning == 0);
            if free {
                self.store.hold(table, name, meaning, symbol);
            }
        }
    }

    /// Where a binder conflict between `symbol` and a new declaration at
    /// `new` is reported: TypeScript's binder reports every declaration of
    /// both, so the new one if it is reported (`reports`), or else the first
    /// reported declaration of `symbol`. Where none is, the declarations
    /// merge, as this tool binds what TypeScript does not report.
    fn reported(&self, symbol: SymbolId, new: (FileId, Span)) -> Option<(FileId, Span)> {
        std::iter::once(new)
            .chain(
                self.symbol(symbol)
                    .sites
                    .iter()
                    .map(|site| (site.file, site.span)),
            )
            .find(|&(file, span)| self.reports(file, span.start))
    }

    /// The meanings of a symbol and every declaration merged into it.
    fn meanings_of(&self, symbol: SymbolId) -> u8 {
        meanings(self.symbol(symbol).flags)
    }

    /// `Store::unite`, where two modules are unsupported.
    fn unite(&mut self, target: SymbolId, other: SymbolId) -> Result<Option<(SymbolId, SymbolId)>> {
        self.store
            .unite(target, other)
            .map_err(|TwoModules(other)| {
                let site = &self.symbol(other).sites[0];
                self.error(
                    InputErrorKind::Unsupported,
                    site.file,
                    site.span,
                    "merging two modules is unsupported".into(),
                )
            })
    }

    /// Merge `other` into `target` as TypeScript's binder does for
    /// declarations of one table, which are one symbol there: their export
    /// and member tables combine, and symbols that meet in one meaning merge
    /// in turn.
    fn merge_symbols(&mut self, target: SymbolId, other: SymbolId) -> Result<()> {
        let mut work = vec![(target, other)];
        while let Some((target, other)) = work.pop() {
            let Some((target, other)) = self.unite(target, other)? else {
                continue;
            };
            let (members, exports) = self.store.transfer(target, other, Direction::Both);
            if let Some(from) = exports {
                let into = self.store.export_table(target);
                self.merge_tables(from, into, &mut work)?;
            }
            if let Some(from) = members {
                let into = self.store.member_table(target);
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
        let entries: Vec<(String, super::store::Entry)> = self.store.drain(from);
        for (name, entry) in entries {
            for meaning in each(meaning::ALL) {
                let held = self
                    .store
                    .entry(into, &name)
                    .and_then(|destination| destination.slots.get(meaning));
                match (held, entry.slots.get(meaning)) {
                    (Some(a), Some(b)) => work.push((a, b)),
                    (None, Some(b)) => self.store.hold(into, &name, meaning, b),
                    _ => {}
                }
            }
            let Some(alias) = entry.alias else {
                continue;
            };
            let Some(kept) = self.held(into, &name).alias else {
                self.store.set_alias(into, &name, Some(alias));
                continue;
            };
            // TypeScript reports both declarations; the first export stays
            // where neither is reported.
            if let Some(checked) = [alias, kept].into_iter().find(|alias| {
                let span = self.facts(alias.file).aliases[alias.id.index()].span;
                self.reports(alias.file, span.start)
            }) {
                let span = self.facts(checked.file).aliases[checked.id.index()].span;
                return Err(self.error(
                    InputErrorKind::Binding,
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
        let held = self.held(table, name);
        let mut merge = Vec::new();
        for meaning in each(meanings) {
            match held.slots.get(meaning) {
                None => self.store.hold(table, name, meaning, symbol),
                Some(other) => merge.push(other),
            }
        }
        for other in merge {
            if other == symbol {
                continue;
            }
            if let Some((file, span)) = self.reported(other, (file, span)) {
                return Err(self.error(
                    InputErrorKind::Binding,
                    file,
                    span,
                    format!("duplicate export `{name}`"),
                ));
            }
        }
        if held.alias.is_some() {
            self.conflicts.push((table, name.to_string(), file, span));
        }
        Ok(())
    }

    fn bind_alias(&mut self, file: FileId, index: AliasId) -> Result<()> {
        let alias = &self.facts(file).aliases[index.index()];
        let reference = AliasRef { file, id: index };
        let span = alias.span;
        let bind = |linker: &mut Self, table: TableId, name: &str| -> Result<()> {
            let held = linker.held(table, name);
            if let Some(kept) = held.alias {
                let kept_span = linker.facts(kept.file).aliases[kept.id.index()].span;
                // TypeScript's binder reports both declarations.
                if let Some((file, span)) = [(file, span), (kept.file, kept_span)]
                    .into_iter()
                    .find(|&(file, span)| linker.reports(file, span.start))
                {
                    return Err(linker.error(
                        InputErrorKind::Binding,
                        file,
                        span,
                        format!("duplicate import or export `{name}`"),
                    ));
                }
                return Ok(());
            }
            linker.store.set_alias(table, name, Some(reference));
            if !held.slots.is_empty() {
                linker.conflicts.push((table, name.to_string(), file, span));
            }
            Ok(())
        };
        match &alias.bind {
            AliasBind::Lexical { scope, name } => {
                let table = self.scope_table(file, *scope);
                bind(self, table, name)
            }
            AliasBind::Export { host, name } => {
                let table = self.host_table(file, *host);
                bind(self, table, name)
            }
            AliasBind::Both { scope, host, name } => {
                let table = self.scope_table(file, *scope);
                bind(self, table, name)?;
                let table = self.host_table(file, *host);
                bind(self, table, name)
            }
            // A UMD global binds into its file's own table, which joins the
            // global table as `merge_globals` builds it.
            AliasBind::Global { name } => {
                let exports = &mut self.global_exports[file.index()];
                if !exports.iter().any(|(other, _)| other == name) {
                    exports.push((name.clone(), reference));
                    return Ok(());
                }
                if self.reports(file, span.start) {
                    return Err(self.error(
                        InputErrorKind::Binding,
                        file,
                        span,
                        format!("duplicate import or export `{name}`"),
                    ));
                }
                Ok(())
            }
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
        if let Some(journal) = &mut self.journal {
            journal.push(task);
        }
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
        if self.store.take_changed() {
            self.reset_resolutions();
        }
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
        let Some(entry) = self.store.table(table).get(name) else {
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
                    // A value supplies no other meaning.
                    Some(Resolution::Value { type_only, .. })
                        if !selects_value(mask, *type_only, type_context) => {}
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
            Storage::Script => tables.push(GLOBAL),
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
            if let Some(file) = self.store.module(current).json {
                return Step::Done(self.json_export(file, name));
            }
            // A shorthand module lists no exports, so each has whatever
            // meaning its use asks for, unlike the module's own meanings.
            if self.store.module(current).shorthand {
                let module = self.store.module(current);
                let mut slots = Slots::only(meaning::ALL, module.symbol);
                for meaning in each(meaning::ALL) {
                    if let Some(symbol) = module.merged.get(meaning) {
                        slots.set(meaning, symbol);
                    }
                }
                return Step::Done(Resolution::Found(Meanings::new(slots).restrict(type_only)));
            }
            // Explicit exports, including type exports beside `export =`.
            if name != "export=" {
                match self.probe_export(current, name) {
                    Probe::Found(resolution) => return Step::Done(restrict(resolution, type_only)),
                    Probe::Need(task) => return Step::Need(task),
                    Probe::Missing => {}
                }
            }
            if !self.exports_have(current, "export=") || name == "export=" {
                break;
            }
            let found = match self.probe_export(current, "export=") {
                Probe::Need(task) => return Step::Need(task),
                Probe::Found(Resolution::Found(found)) => found,
                // An `export =` of a value exposes its type's properties
                // (`getExternalModuleMember`).
                Probe::Found(Resolution::Value {
                    target,
                    type_only: value_type_only,
                }) => {
                    let type_only = type_only || value_type_only;
                    if name == "default" {
                        return Step::Done(Resolution::Value { target, type_only });
                    }
                    return Step::Done(
                        self.value_property(&target, name, type_only)
                            .unwrap_or_else(|| no_property(name)),
                    );
                }
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

    /// `probe` of a module's export table for every meaning of `name`. An
    /// augmented specifier's module that has not yet absorbed its pattern
    /// has the pattern's exports in the meanings it lacks, as TypeScript's
    /// module for the specifier has them from its first augmentation on.
    fn probe_export(&self, module: ModuleId, name: &str) -> Probe {
        let data = self.store.module(module);
        let own = self.probe(data.exports, name, meaning::ALL, true);
        let Some(pattern) = data.pattern else {
            return own;
        };
        let pattern = self.probe(self.store.module(pattern).exports, name, meaning::ALL, true);
        match (own, pattern) {
            (Probe::Need(need), _) | (_, Probe::Need(need)) => Probe::Need(need),
            (Probe::Missing, pattern) => pattern,
            (Probe::Found(Resolution::Found(mut found)), Probe::Found(Resolution::Found(more))) => {
                found.fill_missing(more, meaning::ALL, true);
                Probe::Found(Resolution::Found(found))
            }
            (own, _) => own,
        }
    }

    /// Whether a module's export table, completed as `probe_export` completes
    /// it, has an entry for `name`.
    fn exports_have(&self, module: ModuleId, name: &str) -> bool {
        let data = self.store.module(module);
        self.store.table(data.exports).contains_key(name)
            || data.pattern.is_some_and(|pattern| {
                let exports = self.store.module(pattern).exports;
                self.store.table(exports).contains_key(name)
            })
    }

    /// A module's star exports, completed as `probe_export` completes its
    /// exports, in declaration order.
    fn module_stars(&self, module: ModuleId) -> SmallVec<[(ModuleId, bool); 2]> {
        let data = self.store.module(module);
        let pattern = data
            .pattern
            .map(|pattern| self.store.module(pattern).stars.iter());
        data.stars
            .iter()
            .chain(pattern.into_iter().flatten())
            .filter_map(|star| match self.store.request(star.file, star.request) {
                RequestTarget::Module(target) => Some((target, star.type_only)),
                RequestTarget::Script | RequestTarget::Ignored => None,
            })
            .collect()
    }

    /// A name reached through `export *`. Each module is searched by its most
    /// accessible route, and routes to the same declarations are one target;
    /// different declarations make the name ambiguous.
    fn star_export(&self, module: ModuleId, name: &str, type_only: bool) -> Step<Resolution> {
        let mut best: HashMap<ModuleId, bool> = HashMap::from([(module, type_only)]);
        let mut stack: Vec<(ModuleId, bool)> = self
            .module_stars(module)
            .iter()
            .rev()
            .map(|&(star, star_type_only)| (star, type_only || star_type_only))
            .collect();
        let mut found: Vec<Meanings> = Vec::new();
        // Values, each once with whether every route to it is type-only.
        let mut values: Vec<(ValueTarget, bool)> = Vec::new();
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
            if self.store.module(star).shorthand {
                continue;
            }
            match self.probe_export(star, name) {
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
                Probe::Found(Resolution::Value { target, type_only }) => {
                    let type_only = type_only || route_type_only;
                    match values
                        .iter_mut()
                        .find(|(other, _)| self.same_value(other, &target))
                    {
                        Some((_, existing)) => *existing &= type_only,
                        None => values.push((target, type_only)),
                    }
                }
                Probe::Found(resolution) => {
                    if !failed.iter().any(|(module, _)| *module == star) {
                        failed.push((star, resolution));
                    }
                }
                Probe::Missing => {
                    for &(next, next_type_only) in self.module_stars(star).iter().rev() {
                        stack.push((next, next_type_only || route_type_only));
                    }
                }
            }
        }
        Step::Done(match (found.as_slice(), values.as_slice(), failed.len()) {
            ([], [], 0) => Resolution::Unresolved(format!("`{name}` is not exported")),
            ([targets], [], 0) => Resolution::Found(*targets),
            ([], [(target, type_only)], 0) => Resolution::Value {
                target: target.clone(),
                type_only: *type_only,
            },
            ([], [], 1) => failed.pop().unwrap().1,
            _ => Resolution::Unresolved(format!("`{name}` is ambiguous among star exports")),
        })
    }

    /// Whether two values are one TypeScript symbol. A library member is one
    /// symbol per interface and name, though TypeScript instantiates an
    /// array's members per element type (see docs/references.md).
    fn same_value(&self, a: &ValueTarget, b: &ValueTarget) -> bool {
        match (a, b) {
            (
                ValueTarget::Member { interface, name },
                ValueTarget::Member {
                    interface: other,
                    name: other_name,
                },
            ) => self.find(*interface) == self.find(*other) && name == other_name,
            _ => a == b,
        }
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
        // A value a merge met has its type's properties, as a value does.
        if let Some(target) = self.value_of(container) {
            let type_only = qualifier.members_type_only(Reach::ExportAssignment);
            return Step::Done(
                self.value_property(target, name, type_only)
                    .unwrap_or(Resolution::NotReference("property")),
            );
        }
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
        if self.store.module(module).shorthand {
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

    /// The value of a JSON module: its only export, `export=`
    /// (`bindSourceFileIfExternalModule`).
    fn json_value(file: FileId) -> Resolution {
        Resolution::Value {
            target: ValueTarget::Json {
                file,
                value: super::JsonModule::ROOT,
            },
            type_only: false,
        }
    }

    /// An export of a JSON module as an import or export specifier names it
    /// (`getExternalModuleMember`): `default` names the value, as its
    /// synthetic default, even beside a `"default"` key, and any other name a
    /// property of the value (`value_property`).
    fn json_export(&self, file: FileId, name: &str) -> Resolution {
        if matches!(name, "default" | "export=") {
            return Self::json_value(file);
        }
        let value = ValueTarget::Json {
            file,
            value: super::JsonModule::ROOT,
        };
        self.value_property(&value, name, false)
            .unwrap_or_else(|| no_property(name))
    }

    /// The property `name` of a value's type, as TypeScript selects it for
    /// an import from a module whose `export =` is the value or for a
    /// qualified name (`getPropertyOfType`, without `Object`'s members):
    /// another value, or `None` when the type has no such property. An
    /// object's properties are its keys. A string's, number's, boolean's, or
    /// array's are the members of its global library interface, from every
    /// declaration of it; when one extends other types, whose members it has
    /// too, a name it does not declare is not decided here. A library
    /// member's own type is not inferred, so its properties are not either.
    fn value_property(
        &self,
        target: &ValueTarget,
        name: &str,
        type_only: bool,
    ) -> Option<Resolution> {
        let value = |target| Resolution::Value { target, type_only };
        let (file, at) = match target {
            ValueTarget::Json { file, value } => (*file, *value),
            ValueTarget::Member { .. } => {
                return Some(Resolution::NotReference(
                    "property of a library member, whose type is not inferred",
                ));
            }
        };
        let json = self.file(file).json.as_ref().expect("a JSON module's file");
        if let Some(child) = json.property(at, name) {
            return Some(value(ValueTarget::Json { file, value: child }));
        }
        let interface = json.shape(at).interface()?;
        let entry = self.store.table(GLOBAL).get(interface)?;
        let symbol = self.find(entry.slots.get(meaning::TYPE)?);
        let members = self.store.interface_members(symbol)?;
        if members.names.iter().any(|member| member == name) {
            return Some(value(ValueTarget::Member {
                interface: symbol,
                name: name.into(),
            }));
        }
        members.inherits.then_some(Resolution::NotReference(
            "possibly inherited member of a library interface",
        ))
    }

    /// What a namespace import, `import = require`, or UMD global denotes:
    /// the module's `export =` target, or the module itself.
    fn module_object(&self, module: ModuleId) -> Step<Resolution> {
        if let Some(file) = self.store.module(module).json {
            return Step::Done(Self::json_value(file));
        }
        if !self.exports_have(module, "export=") {
            let symbol = self.store.module(module).symbol;
            let mut slots = Slots::only(self.meanings_of(symbol), symbol);
            for meaning in each(meaning::ALL) {
                if let (None, Some(merged)) = (
                    slots.get(meaning),
                    self.store.module(module).merged.get(meaning),
                ) {
                    slots.set(meaning, merged);
                }
            }
            return Step::Done(Resolution::Found(Meanings::new(slots)));
        }
        match self.probe_export(module, "export=") {
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
                                && self.store.module(module).declaration
                                && !["default", "__esModule"]
                                    .iter()
                                    .any(|name| self.exports_have(module, name))
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
                    // A missing property of a value is still a property, as
                    // the semantic oracle normalizes it (`unresolvedProperty`).
                    Some(Resolution::Value { target, type_only }) => {
                        let property = self
                            .value_property(target, name, *type_only)
                            .unwrap_or(Resolution::NotReference("property"));
                        return Step::Done(mask_resolution(
                            property,
                            data.meaning,
                            data.type_context,
                            name,
                        ));
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
                // An import type's qualifier names a property of a JSON
                // module's value, with no synthetic default.
                Ok(module) => match self.store.module(module).json {
                    Some(file) => {
                        let value = ValueTarget::Json {
                            file,
                            value: super::JsonModule::ROOT,
                        };
                        Step::Done(
                            self.value_property(&value, name, false)
                                .unwrap_or_else(|| no_property(name)),
                        )
                    }
                    None => self.module_member(module, name),
                },
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
                        NameTarget::Unresolved(why) => Target::Unresolved(index.intern(why)),
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
        let aliased: Vec<SymbolId> = self
            .store
            .canonical_symbols()
            .filter(|(_, symbol)| !symbol.aliases.is_empty())
            .map(|(id, _)| id)
            .collect();
        let projected: HashMap<SymbolId, Vec<Site>> = aliased
            .into_iter()
            .map(|symbol| (symbol, self.projected_sites(symbol)))
            .collect();
        index.finish(&self.program.sources, |symbol| {
            projected
                .get(&symbol)
                .cloned()
                .unwrap_or_else(|| self.symbol(symbol).sites.clone())
        })
    }

    /// The declarations a group lists for `symbol`: its own, and those of
    /// what its alias declarations name that it may merge with, as
    /// TypeScript's queries follow an alias among a symbol's declarations to
    /// its target. One way, as the target's group does not list `symbol`'s.
    fn projected_sites(&mut self, symbol: SymbolId) -> Vec<Site> {
        let symbol = self.find(symbol);
        let mut sites = self.symbol(symbol).sites.clone();
        let mut reached = vec![symbol];
        let mut stack = vec![symbol];
        while let Some(owner) = stack.pop() {
            let flags = self.symbol(owner).flags;
            for alias in self.symbol(owner).aliases.clone() {
                let Resolution::Found(found) = self.force(Task::Alias(alias)) else {
                    continue;
                };
                for target in distinct(found.targets().map(|target| self.find(target))) {
                    let other = self.symbol(target).flags;
                    if reached.contains(&target)
                        || conflicts(flags, other, false)
                        || !merges(flags, other)
                    {
                        continue;
                    }
                    reached.push(target);
                    sites.extend(self.symbol(target).sites.iter().cloned());
                    stack.push(target);
                }
            }
        }
        sites
    }

    fn target(&self, resolution: Resolution, index: &mut index::Builder) -> Target {
        match resolution {
            // A name that denotes a value, alone or beside declarations of
            // other meanings that a merge gave it, denotes no reference
            // target, as TypeScript's one symbol for them is a property.
            Resolution::Found(found) => {
                if let Some(value) = found.targets().symbols().find_map(|s| self.value_of(s)) {
                    return Target::NotReference(value_reason(value));
                }
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
            Resolution::Value { target, .. } => Target::NotReference(value_reason(&target)),
            Resolution::NotReference(why) => Target::NotReference(why),
            Resolution::Unresolved(why) => Target::Unresolved(index.intern(&why)),
        }
    }

    /// An import alias may not share a meaning with a local declaration.
    fn check_conflicts(&mut self) -> Result<()> {
        for (table, name, file, span) in std::mem::take(&mut self.conflicts) {
            let Some(entry) = self.store.table(table).get(&name) else {
                continue;
            };
            let (Some(alias), slots) = (entry.alias, entry.slots) else {
                continue;
            };
            // An export specifier of a local names that local; any other
            // alias conflicts on a shared meaning even when it resolves back
            // to the local itself, as an import cycling through its own
            // module does (TypeScript's TS2440).
            let local = matches!(
                self.facts(alias.file).aliases[alias.id.index()].kind,
                AliasKind::Lexical { .. }
            );
            let overlap = match self.force(Task::Alias(alias)) {
                Resolution::Found(found) => {
                    let targets = found.targets();
                    each(meaning::ALL).any(|meaning| {
                        match (slots.get(meaning), targets.get(meaning)) {
                            (Some(a), Some(b)) => !local || self.find(a) != self.find(b),
                            _ => false,
                        }
                    })
                }
                // A value, which no declaration is, has only the value meaning.
                Resolution::Value { .. } => slots.get(meaning::VALUE).is_some(),
                _ => false,
            };
            if overlap && self.reports(file, span.start) {
                return Err(self.error(
                    InputErrorKind::Binding,
                    file,
                    span,
                    format!("import or export `{name}` conflicts with a local declaration"),
                ));
            }
        }
        Ok(())
    }
}

/// A declaration being added to a table.
struct Declaring {
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

/// One type parameter of a class or interface declaration.
struct TypeParameter<'a> {
    /// The symbol it declares, which declarations merged both ways share.
    symbol: Option<SymbolId>,
    name: &'a str,
    default: bool,
}

/// TypeScript's check of merged class and interface type parameters
/// (`areTypeParametersIdentical`), apart from constraints and defaults: the
/// merged list is each distinct type parameter symbol in declaration order,
/// and every declaration repeats the names of a prefix of it that includes
/// each parameter no declaration gives a default. A one-way merge keeps the
/// source's parameters apart, so a pattern's `X<T>` and its augmentation's
/// `X<T>` make two.
fn identical_type_parameters<'a>(
    lists: impl Iterator<Item = &'a [TypeParameter<'a>]> + Clone,
) -> bool {
    let mut merged: Vec<(&TypeParameter<'a>, bool)> = Vec::new();
    for list in lists.clone() {
        for parameter in list {
            let same = |(other, _): &&mut (&TypeParameter<'a>, bool)| match parameter.symbol {
                Some(symbol) => other.symbol == Some(symbol),
                None => other.symbol.is_none() && other.name == parameter.name,
            };
            match merged.iter_mut().find(same) {
                Some((_, default)) => *default |= parameter.default,
                None => merged.push((parameter, parameter.default)),
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
                .all(|(parameter, (merged, _))| parameter.name == merged.name)
    })
}

/// Mark a resolution reached through a type-only route.
fn restrict(resolution: Resolution, type_only: bool) -> Resolution {
    match resolution {
        Resolution::Found(found) => Resolution::Found(found.restrict(type_only)),
        Resolution::Value {
            target,
            type_only: own,
        } => Resolution::Value {
            target,
            type_only: own || type_only,
        },
        other => other,
    }
}

/// Why a value is not a reference target.
fn value_reason(target: &ValueTarget) -> &'static str {
    match target {
        ValueTarget::Json { value, .. } if *value == super::JsonModule::ROOT => JSON_VALUE,
        ValueTarget::Json { .. } => "JSON property",
        ValueTarget::Member { .. } => "member of a library interface",
    }
}

/// A name that a value's type does not have as a property.
fn no_property(name: &str) -> Resolution {
    Resolution::Unresolved(format!("the value has no property `{name}`"))
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
        Resolution::Value { type_only, .. } if !selects_value(mask, type_only, type_context) => {
            Resolution::Unresolved(format!("`{name}` is a value with no matching meaning"))
        }
        other => other,
    }
}
