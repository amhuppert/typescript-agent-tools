//! TypeScript's resolveNameHelper/resolveEntityName/resolveAlias read one bound
//! declaration graph. Loading and semantic queries run this same task machine;
//! only their cache storage and immutable source-fact projections differ.

pub(in crate::project) mod names;

pub(super) use super::inputs::{BindingFiles as Files, BindingInputs as Inputs, ProjectedHosts};

use super::LookupFailure;
use super::collect::{AliasBind, AliasKind, Host, RefBase, ScopeKind};
use super::ids::{DeclId, ModuleId, RefId, RequestId, ScopeId, SymbolId, TableId};
use super::link::{
    Resolution, ResolutionFailure, ValueOrigin, ValueTarget, flags, mask_resolution, meanings,
    no_property, restrict, selects_value,
};
use super::meaning;
use super::semantic::TypeBindingFailure;
use super::store::{AliasRef, GLOBAL, RequestTarget, Symbol};
use super::targets::{Meanings, Reach, Slots, each};
use crate::project::sources::FileId;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct RefRef {
    pub(super) file: FileId,
    pub(super) id: RefId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Task {
    Alias(AliasRef),
    Ref(RefRef),
}

/// `resolveNameHelper`'s scope decisions, independent of the result domain.
/// Native headers and navigation tables consume this same ordered recipe.
#[derive(Clone, Copy)]
enum NameTable {
    Global,
    Locals(ScopeId),
    Members(DeclId),
    Exports { host: Host, external: bool },
}

impl NameTable {
    /// `resolveNameHelper` narrows raw module exports to ModuleMember. The
    /// default-local test uses the original meaning before this restriction.
    fn lookup_meaning(self, meaning: u32) -> u32 {
        match self {
            Self::Members(_) => meaning & flags::TYPE_PARAMETER,
            Self::Exports { .. } => meaning & flags::MODULE_MEMBER,
            _ => meaning,
        }
    }
}
struct NameScope {
    index: Option<ScopeId>,
    infer: Option<DeclId>,
    enum_: Option<DeclId>,
    arguments: bool,
    tables: SmallVec<[NameTable; 2]>,
}

/// A task that another task waits for. `fill` marks an alias consulted only
/// for meanings that same-named local declarations lack: TypeScript resolves
/// such a name to its locals without following the alias, so a cycle through
/// that edge is cut there rather than leaving every member unresolved.
#[derive(Debug, Clone, Copy)]
pub(super) struct Need {
    pub(super) task: Task,
    fill: bool,
    /// The enclosing lexical reference's raw value identity, when lookup
    /// pauses for an alias. Force still follows the same task and cut edge.
    value_origin: Option<ValueOrigin>,
}

impl Need {
    fn required(task: Task) -> Self {
        Self {
            task,
            fill: false,
            value_origin: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) enum State {
    #[default]
    Unvisited,
    InProgress,
    Done(Resolution),
}

pub(super) enum Step<T> {
    Done(T),
    Need(Need),
    Failed(LookupFailure),
}

impl<T> Step<T> {
    pub(super) fn map<U>(self, map: impl FnOnce(T) -> U) -> Step<U> {
        match self {
            Self::Done(value) => Step::Done(map(value)),
            Self::Need(need) => Step::Need(need),
            Self::Failed(failure) => Step::Failed(failure),
        }
    }
}

/// The result of probing one table for a name.
pub(super) enum Probe<T = Resolution> {
    Found(T),
    Need(Need),
    Missing,
}

/// `getExportsOfModule` retains the selected export symbol, independently of
/// the meanings obtained by resolving it. In particular, Alias-only entity
/// lookup observes this symbol even when a declaration also supplies TYPE.
#[derive(Clone)]
pub(super) struct ExportLookup {
    pub(super) resolution: Resolution,
    alias: Option<AliasRef>,
}

impl ExportLookup {
    fn direct(resolution: Resolution) -> Self {
        Self {
            resolution,
            alias: None,
        }
    }

    fn restrict(self, type_only: bool) -> Self {
        Self {
            resolution: restrict(self.resolution, type_only),
            ..self
        }
    }
}

struct LexicalLookup {
    resolution: Resolution,
    value_origin: Option<ValueOrigin>,
}

/// A lookup whose meaning includes Alias can select the raw symbol without
/// asking for its target. This is not a completed alias resolution.
enum NameLookup {
    Resolved(LexicalLookup),
    Symbol(ValueOrigin),
}

/// `getSymbol` accepts an actual SymbolFlags mask. Lexical navigation keeps
/// its three meaning projection, while checker queries may ask a narrower
/// native family (for example Interface). That eligibility must be applied
/// before a scope stops, including after alias resolution.
#[derive(Clone, Copy)]
pub(super) enum LookupMeaning {
    Projected(u8),
    Native(u32),
}

impl LookupMeaning {
    pub(in crate::project::binding) fn native(self) -> u32 {
        match self {
            Self::Native(flags) => flags,
            Self::Projected(mask) => {
                (if mask & meaning::VALUE != 0 {
                    flags::VALUE
                } else {
                    0
                }) | (if mask & meaning::TYPE != 0 {
                    flags::TYPE
                } else {
                    0
                }) | (if mask & meaning::NAMESPACE != 0 {
                    flags::NAMESPACE | flags::ENUM
                } else {
                    0
                })
            }
        }
    }
    fn projection(self) -> u8 {
        match self {
            Self::Projected(mask) => mask,
            Self::Native(flags) => meanings(flags),
        }
    }

    fn accepts(self, symbol_flags: u32) -> bool {
        match self {
            Self::Projected(mask) => meanings(symbol_flags) & mask != 0,
            Self::Native(mask) => symbol_flags & mask != 0,
        }
    }

    fn restricted(self, projection: u8) -> Self {
        match self {
            Self::Projected(_) => Self::Projected(projection),
            Self::Native(mask) => Self::Native(
                mask & (if projection & meaning::VALUE != 0 {
                    flags::VALUE
                } else {
                    0
                } | if projection & meaning::TYPE != 0 {
                    flags::TYPE
                } else {
                    0
                } | if projection & meaning::NAMESPACE != 0 {
                    flags::NAMESPACE
                } else {
                    0
                }),
            ),
        }
    }
}

impl From<u8> for LookupMeaning {
    fn from(mask: u8) -> Self {
        Self::Projected(mask)
    }
}

struct EntryProbe {
    probe: Probe,
    value_origin: Option<ValueOrigin>,
    type_alias_origin: Option<AliasRef>,
}

/// Where a scope's names are stored.
#[derive(Debug, Clone, Copy)]
pub(super) enum Storage {
    /// A script's top level: its own table, which merges into the global
    /// table (`merge_globals`), where its names are looked up.
    Script,
    /// The member table shared by a class or interface's declarations.
    Members(DeclId),
    /// The scope's own table, which also sees the exports of a host.
    Own(Option<Host>),
}

impl<'a> Inputs<'a> {
    fn symbol(&self, id: SymbolId) -> &Symbol {
        self.store.symbol(id)
    }
    /// The module a request names, or why names imported through it are
    /// unresolved.
    pub(super) fn request_module(
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

    /// `resolveNameHelper` uses the same complete current-header precheck as
    /// semantic name lookup. Scope/Host recipes never leave binding.
    fn scope_export_lookup(
        &self,
        file: FileId,
        host: Host,
        external: bool,
        name: &str,
        meaning: u32,
    ) -> Result<super::ScopeExportRead, LookupFailure> {
        let inputs = self
            .native_inputs
            .ok_or(LookupFailure::Binding(TypeBindingFailure::NotRetained))?;
        let header = inputs
            .raw_host(file, host)
            .ok_or(LookupFailure::Binding(TypeBindingFailure::NotRetained))?;
        self.native_facts
            .ok_or(LookupFailure::Binding(TypeBindingFailure::NotRetained))?
            .scope_exports(super::BoundSymbol(header), external, name, meaning)
    }
    fn current_projection_flags(&self, symbol: SymbolId) -> Result<u32, LookupFailure> {
        let header = self
            .store
            .raw_projection_header(symbol)
            .ok_or(LookupFailure::Binding(TypeBindingFailure::NotRetained))?;
        Ok(self
            .native_facts
            .ok_or(LookupFailure::Binding(TypeBindingFailure::NotRetained))?
            .flags(super::BoundSymbol(header))?
            .bits)
    }

    /// The export table of a host whose members a scope sees, if it exists.
    pub(super) fn visible_exports(&self, file: FileId, host: Host) -> Option<TableId> {
        match host {
            Host::Namespace(decl) => self.decl_symbols[file.index()][decl.index()]
                .and_then(|symbol| self.symbol(symbol).exports),
            Host::Augmentation(request) => {
                self.symbol(
                    self.augmentations[file.index()][request.index()]
                        .expect("an augmentation request has a symbol"),
                )
                .exports
            }
            Host::Global => self.global_augmentations[file.index()]
                .and_then(|symbol| self.symbol(symbol).exports),
            Host::File | Host::AmbientModule(_) => self
                .host_symbol(file, host)
                .and_then(|symbol| self.symbol(symbol).exports),
        }
    }
    /// Whether a module's export table, completed as `probe_export` completes
    /// it, has an entry for `name`.
    pub(super) fn exports_have(&self, module: ModuleId, name: &str) -> bool {
        let data = self.store.module(module);
        self.store.table(data.exports).contains_key(name)
            || data.pattern.is_some_and(|pattern| {
                let exports = self.store.module(pattern).exports;
                self.store.table(exports).contains_key(name)
            })
    }
    /// A module's star exports, completed as `probe_export` completes its
    /// exports, in declaration order.
    pub(super) fn module_stars(&self, module: ModuleId) -> SmallVec<[(ModuleId, bool); 2]> {
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
}

/// Caches whose semantics are common to both stages. A load journal is present
/// only while declaration merges can invalidate answers; queries see frozen data.
#[derive(Default)]
pub(super) struct Runtime {
    pub(super) export_memo: HashMap<(ModuleId, String), ExportLookup>,
    pub(super) journal: Option<Vec<Task>>,
    pub(super) running: Option<Task>,
    pub(super) cuts: HashSet<(Task, Task)>,
    pub(super) cut_tasks: HashSet<Task>,
}
impl Runtime {
    pub(super) fn loading() -> Self {
        Self {
            journal: Some(Vec::new()),
            ..Self::default()
        }
    }
}
#[derive(Clone, Copy)]
enum StateView<'a> {
    Unvisited,
    InProgress,
    Done(&'a Resolution),
}
impl State {
    fn view(&self) -> StateView<'_> {
        match self {
            Self::Unvisited => StateView::Unvisited,
            Self::InProgress => StateView::InProgress,
            Self::Done(value) => StateView::Done(value),
        }
    }
}
pub(super) enum States<'a> {
    Loading {
        aliases: &'a mut Vec<Vec<State>>,
        refs: &'a mut Vec<Vec<State>>,
        alias_origins: &'a mut Vec<Vec<Option<ValueOrigin>>>,
        ref_origins: &'a mut Vec<Vec<Option<ValueOrigin>>>,
        ref_type_alias_origins: &'a mut Vec<Vec<Option<AliasRef>>>,
    },
}
impl States<'_> {
    fn state(&self, task: Task) -> StateView<'_> {
        match self {
            Self::Loading { aliases, refs, .. } => match task {
                Task::Alias(a) => aliases[a.file.index()][a.id.index()].view(),
                Task::Ref(r) => refs[r.file.index()][r.id.index()].view(),
            },
        }
    }
    fn set_state(&mut self, task: Task, state: State) {
        if !matches!(state, State::Done(_)) {
            self.set_origin(task, None);
            if let Task::Ref(reference) = task {
                self.set_type_alias(reference, None);
            }
        }
        match self {
            Self::Loading { aliases, refs, .. } => match task {
                Task::Alias(a) => aliases[a.file.index()][a.id.index()] = state,
                Task::Ref(r) => refs[r.file.index()][r.id.index()] = state,
            },
        }
    }
    fn set_origin(&mut self, task: Task, origin: Option<ValueOrigin>) {
        match self {
            Self::Loading {
                alias_origins,
                ref_origins,
                ..
            } => match task {
                Task::Alias(a) => alias_origins[a.file.index()][a.id.index()] = origin,
                Task::Ref(r) => ref_origins[r.file.index()][r.id.index()] = origin,
            },
        }
    }
    fn set_type_alias(&mut self, reference: RefRef, alias: Option<AliasRef>) {
        match self {
            Self::Loading {
                ref_type_alias_origins,
                ..
            } => ref_type_alias_origins[reference.file.index()][reference.id.index()] = alias,
        }
    }
}

pub(super) struct Resolver<'a> {
    pub(super) inputs: Inputs<'a>,
    pub(super) states: States<'a>,
    pub(super) runtime: &'a mut Runtime,
}
impl<'a> Resolver<'a> {
    fn find(&self, symbol: SymbolId) -> SymbolId {
        self.inputs.store.find(symbol)
    }
    fn symbol(&self, symbol: SymbolId) -> &Symbol {
        self.inputs.store.symbol(symbol)
    }
    fn meanings_of(&self, symbol: SymbolId) -> u8 {
        meanings(self.symbol(symbol).flags)
    }
    fn value_of(&self, symbol: SymbolId) -> Option<&ValueTarget> {
        self.inputs.values.get(&self.find(symbol))
    }
    fn state(&self, task: Task) -> StateView<'_> {
        self.states.state(task)
    }
    fn set_state(&mut self, task: Task, state: State) {
        if let Some(journal) = &mut self.runtime.journal {
            journal.push(task);
        }
        self.states.set_state(task, state);
    }
    /// Resolve `task` and everything it needs with an explicit stack. Each
    /// task on the stack needs the one above it, so a task that needs one
    /// still in progress closes a cycle through every task between them. All
    /// of them are unresolved, whichever task resolution happened to start
    /// from; deciding only the task that detects the cycle would let the
    /// others' results depend on file order.
    pub(super) fn force(&mut self, task: Task) -> Result<Resolution, LookupFailure> {
        // Each frame records whether a fill edge from the frame below pushed it.
        let mut stack = vec![(task, false)];
        while let Some(&(top, _)) = stack.last() {
            if let StateView::Done(_) = self.state(top) {
                stack.pop();
                continue;
            }
            self.set_state(top, State::InProgress);
            self.runtime.running = Some(top);
            let step = match top {
                Task::Alias(alias) => self.try_alias(alias),
                Task::Ref(reference) => self.try_ref(reference),
            };
            self.runtime.running = None;
            match step {
                Step::Done(resolution) => {
                    self.set_state(top, State::Done(resolution));
                    stack.pop();
                }
                Step::Need(need) if matches!(self.state(need.task), StateView::InProgress) => {
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
                        let (file, span) = match need.task {
                            Task::Alias(alias) => (
                                alias.file,
                                self.inputs.file(alias.file).aliases()[alias.id.index()].span,
                            ),
                            Task::Ref(reference) => {
                                let data = self.inputs.reference(reference);
                                (reference.file, data.span)
                            }
                        };
                        for (member, _) in stack.drain(start..) {
                            self.set_state(
                                member,
                                State::Done(Resolution::Unresolved(
                                    ResolutionFailure::from("circular alias")
                                        .at_dependency(file, span),
                                )),
                            );
                        }
                        continue;
                    };
                    for &(tail, head) in &fills {
                        self.runtime.cuts.insert((stack[tail].0, head));
                        self.runtime.cut_tasks.insert(stack[tail].0);
                    }
                    // Rerun from the lowest cut tail; tasks above it resume
                    // when it needs them again.
                    for (member, _) in stack.drain(lowest + 1..) {
                        self.set_state(member, State::Unvisited);
                    }
                }
                Step::Need(need) => stack.push((need.task, need.fill)),
                Step::Failed(failure) => {
                    // No failed acquisition is a completed name resolution.
                    // Reset suspended frames and their origin attachments so
                    // a later demand cannot observe an abandoned cycle.
                    let abandoned: HashSet<_> = stack.iter().map(|&(task, _)| task).collect();
                    for (task, _) in stack {
                        self.set_state(task, State::Unvisited);
                    }
                    self.runtime.cuts.retain(|(tail, head)| {
                        !abandoned.contains(tail) && !abandoned.contains(head)
                    });
                    self.runtime.cut_tasks =
                        self.runtime.cuts.iter().map(|(tail, _)| *tail).collect();
                    return Err(failure);
                }
            }
        }
        match self.state(task) {
            StateView::Done(resolution) => Ok(resolution.clone()),
            _ => unreachable!("forced task is resolved"),
        }
    }

    fn done(&self, task: Task) -> Option<&Resolution> {
        match self.state(task) {
            StateView::Done(resolution) => Some(resolution),
            _ => None,
        }
    }

    /// `probe` for a lexical lookup, to which export specifiers are invisible
    /// even in the export table of an enclosing namespace.
    fn probe_entry(
        &self,
        table: TableId,
        name: &str,
        requested: LookupMeaning,
        type_context: bool,
        lexical: bool,
    ) -> EntryProbe {
        let mask = requested.projection();
        let Some(entry) = self.inputs.store.table(table).get(name) else {
            return EntryProbe {
                probe: Probe::Missing,
                value_origin: None,
                type_alias_origin: None,
            };
        };
        let mut found = self.select_lookup_meanings(
            Meanings::new(entry.slots.map(|symbol| self.find(symbol))),
            requested,
            type_context,
        );
        let mut value_origin = found.targets().value().map(ValueOrigin::Direct);
        let alias = entry.alias.filter(|alias| {
            !lexical || !self.inputs.file(alias.file).aliases()[alias.id.index()].export_only
        });
        if let Some(alias) = alias {
            let task = Task::Alias(alias);
            let cut = !found.is_empty()
                && self
                    .runtime
                    .running
                    .is_some_and(|running| self.runtime.cuts.contains(&(running, task)));
            if found.needs(mask) && !cut {
                let resolved = self.done(task);
                if value_origin.is_none()
                    && mask & meaning::VALUE != 0
                    && resolved.is_some_and(|resolution| match resolution {
                        Resolution::Found(targets) => targets.targets().value().is_some(),
                        Resolution::Value { .. }
                        | Resolution::NotReference(_)
                        | Resolution::Unresolved(_) => true,
                    })
                {
                    value_origin = Some(ValueOrigin::Alias(alias));
                }
                match resolved {
                    None => {
                        return EntryProbe {
                            probe: Probe::Need(Need {
                                task,
                                fill: !found.is_empty(),
                                value_origin: None,
                            }),
                            value_origin: value_origin.or_else(|| {
                                (mask & meaning::VALUE != 0).then_some(ValueOrigin::Alias(alias))
                            }),
                            type_alias_origin: Some(alias),
                        };
                    }
                    Some(Resolution::Found(targets)) => {
                        found.fill_missing(
                            self.select_lookup_meanings(*targets, requested, type_context),
                            mask,
                            type_context,
                        );
                    }
                    // A value supplies no other meaning.
                    Some(Resolution::Value { type_only, .. })
                        if !selects_value(mask, *type_only, type_context) => {}
                    Some(other) if found.is_empty() => {
                        // `getSymbolFlags` uses All after alias resolution
                        // failure to suppress cascading lookup errors.
                        return EntryProbe {
                            probe: Probe::Found(other.clone()),
                            value_origin,
                            type_alias_origin: Some(alias),
                        };
                    }
                    Some(_) => {}
                }
            }
        }
        EntryProbe {
            probe: if found.is_empty() {
                Probe::Missing
            } else {
                Probe::Found(Resolution::Found(found))
            },
            value_origin,
            type_alias_origin: alias,
        }
    }

    /// `getSymbol`'s direct/aliased target flag eligibility. Filtering happens
    /// at each table probe; a rejected local must not hide an eligible outer
    /// declaration. The alias task cache stores targets independently of the
    /// caller's meaning, so every observation applies this same decision.
    fn select_lookup_meanings(
        &self,
        found: Meanings,
        requested: LookupMeaning,
        type_context: bool,
    ) -> Meanings {
        let selected = found.select(requested.projection(), type_context);
        match requested {
            LookupMeaning::Projected(_) => selected,
            LookupMeaning::Native(_) => {
                selected.retain(|symbol| requested.accepts(self.symbol(symbol).flags))
            }
        }
    }

    fn probe_export_entry(&self, table: TableId, name: &str) -> Probe<ExportLookup> {
        let entry = self.probe_entry(table, name, meaning::ALL.into(), true, false);
        match entry.probe {
            Probe::Found(resolution) => Probe::Found(ExportLookup {
                resolution,
                alias: entry.type_alias_origin,
            }),
            Probe::Need(need) => Probe::Need(need),
            Probe::Missing => Probe::Missing,
        }
    }

    /// Native and lexical answers use the same scope-kind decisions. Only
    /// converting a table identity or a found symbol differs between domains.
    fn walk_lookup_scopes<T>(
        &self,
        file: FileId,
        scope: ScopeId,
        mut visit: impl FnMut(NameScope) -> Option<T>,
    ) -> Option<T> {
        for index in self.inputs.files.lookup_scope_chain(file, scope) {
            if let Some(result) = visit(self.inputs.files.name_scope(file, index)) {
                return Some(result);
            }
        }
        None
    }

    fn projected_name_table(&self, file: FileId, table: NameTable) -> Option<TableId> {
        match table {
            NameTable::Global => Some(GLOBAL),
            NameTable::Locals(scope) => self.inputs.scope_tables[file.index()][scope.index()],
            NameTable::Members(decl) => self.inputs.decl_symbols[file.index()][decl.index()]
                .and_then(|symbol| self.symbol(symbol).members),
            NameTable::Exports { host, .. } => self.inputs.visible_exports(file, host),
        }
    }

    /// `resolveNameHelper` permits only actual type parameters from class or
    /// interface member tables. Alias-only lookup uses no declaration meaning.
    fn lexical_table_mask(
        &self,
        file: FileId,
        scope: Option<ScopeId>,
        table: TableId,
        name: &str,
        mask: u8,
    ) -> u8 {
        if scope.is_some_and(|scope| {
            matches!(
                self.inputs.file(file).scopes()[scope.index()].kind,
                ScopeKind::Members { .. }
            )
        }) {
            if self
                .inputs
                .store
                .entry(table, name)
                .and_then(|entry| entry.member)
                .is_some_and(|symbol| self.symbol(symbol).flags & flags::TYPE_PARAMETER != 0)
            {
                mask & meaning::TYPE
            } else {
                0
            }
        } else {
            mask
        }
    }

    /// `resolveNameHelper` with Alias meaning. Direct declarations do not
    /// stop this lookup and selecting an alias does not resolve its target.
    fn lookup_alias(
        &self,
        file: FileId,
        scope: ScopeId,
        name: &str,
    ) -> Result<Option<AliasRef>, LookupFailure> {
        match self.lookup_name(file, scope, name, 0u8.into(), true, true, false, true) {
            Step::Done(NameLookup::Symbol(ValueOrigin::Alias(alias))) => Ok(Some(alias)),
            Step::Done(NameLookup::Resolved(_)) => Ok(None),
            Step::Failed(failure) => Err(failure),
            Step::Done(NameLookup::Symbol(ValueOrigin::Direct(_))) | Step::Need(_) => {
                unreachable!("Alias-only lookup cannot select or resolve a declaration")
            }
        }
    }

    /// Lexical lookup from `scope` outward to the global table.
    fn lookup(
        &self,
        file: FileId,
        scope: ScopeId,
        name: &str,
        mask: u8,
        type_context: bool,
    ) -> Step<LexicalLookup> {
        self.lookup_name(
            file,
            scope,
            name,
            mask.into(),
            type_context,
            false,
            false,
            false,
        )
        .map(|found| match found {
            NameLookup::Resolved(found) => found,
            NameLookup::Symbol(_) => {
                unreachable!("projected target lookup does not select raw aliases")
            }
        })
    }

    /// `resolveNameHelper`: Alias is an independent lookup meaning. The same
    /// scope walk and special-name gates serve raw-symbol and target queries.
    #[expect(
        clippy::too_many_arguments,
        reason = "ports resolveNameHelper's lookup inputs"
    )]
    fn lookup_name(
        &self,
        file: FileId,
        scope: ScopeId,
        name: &str,
        requested: LookupMeaning,
        type_context: bool,
        include_alias: bool,
        include_export_value: bool,
        raw_symbols: bool,
    ) -> Step<NameLookup> {
        let mask = requested.projection();
        let facts = self.inputs.file(file);
        let mut value_origin = None;
        let found = self.walk_lookup_scopes(file, scope, |plan| {
            let index = plan.index;
            // An infer constraint sees only its own declaration's name.
            if let Some(decl) = plan.infer {
                let own = &facts.decls()[decl.index()];
                if own.name == name && requested.accepts(flags::TYPE_PARAMETER) {
                    if let Some(symbol) = self.inputs.decl_symbols[file.index()][decl.index()] {
                        return Some(Step::Done(NameLookup::Resolved(LexicalLookup {
                            resolution: Resolution::Found(Meanings::new(Slots::only(
                                meaning::TYPE,
                                self.find(symbol),
                            ))),
                            value_origin: None,
                        })));
                    }
                }
            }
            // Enum initializers see the enum's members, which are not targets.
            if mask != 0
                && let Some(decl) = plan.enum_
            {
                let member = self.inputs.decl_symbols[file.index()][decl.index()]
                    .and_then(|symbol| self.symbol(symbol).exports)
                    .and_then(|table| self.inputs.store.entry(table, name))
                    .and_then(|entry| entry.member.or_else(|| entry.slots.symbols().next()));
                if let Some(member) = member {
                    if self.symbol(member).flags & flags::ENUM_MEMBER != 0
                        && requested.accepts(flags::ENUM_MEMBER)
                    {
                        let origin = ValueOrigin::Direct(member);
                        if raw_symbols {
                            return Some(Step::Done(NameLookup::Symbol(origin)));
                        }
                        return Some(Step::Done(NameLookup::Resolved(LexicalLookup {
                            resolution: Resolution::NotReference("enum member"),
                            value_origin: Some(origin),
                        })));
                    }
                }
            }
            for table_recipe in plan.tables {
                let Some(table) = self.projected_name_table(file, table_recipe) else {
                    continue;
                };
                let table_requested = if matches!(table_recipe, NameTable::Exports { .. }) {
                    LookupMeaning::Native(table_recipe.lookup_meaning(requested.native()))
                } else {
                    requested
                };
                // Alias has no declaration slot, but it participates in the
                // same class-member-table eligibility gate.
                let lookup_mask = self.lexical_table_mask(
                    file,
                    index,
                    table,
                    name,
                    table_requested.projection() | if include_alias { 8 } else { 0 },
                );
                if let NameTable::Exports { host, external } = table_recipe {
                    match self.inputs.scope_export_lookup(
                        file,
                        host,
                        external,
                        name,
                        requested.native(),
                    ) {
                        Ok(super::ScopeExportRead::Skip | super::ScopeExportRead::Table(false)) => {
                            continue;
                        }
                        Ok(super::ScopeExportRead::Table(true)) => {}
                        Ok(super::ScopeExportRead::NamedDefault {
                            projection: symbol,
                            flags,
                        }) => {
                            let origin = ValueOrigin::Direct(self.find(symbol));
                            if raw_symbols {
                                return Some(Step::Done(NameLookup::Symbol(origin)));
                            }
                            let found =
                                Meanings::new(Slots::only(meanings(flags.bits), self.find(symbol)));
                            return Some(Step::Done(NameLookup::Resolved(LexicalLookup {
                                resolution: Resolution::Found(self.select_lookup_meanings(
                                    found,
                                    requested,
                                    type_context,
                                )),
                                value_origin: value_origin.or(Some(origin)),
                            })));
                        }
                        Err(failure) => return Some(Step::Failed(failure)),
                    }
                }
                if lookup_mask == 0 {
                    continue;
                }
                if let Some(local) = self
                    .inputs
                    .store
                    .entry(table, name)
                    .and_then(|entry| entry.member)
                    .filter(|&symbol| self.symbol(symbol).export_symbol.is_some())
                {
                    if raw_symbols {
                        let flags = match self.inputs.current_projection_flags(local) {
                            Ok(flags) => flags,
                            Err(failure) => return Some(Step::Failed(failure)),
                        };
                        if table_requested.restricted(lookup_mask).accepts(flags)
                            || include_export_value && flags & flags::EXPORT_VALUE != 0
                        {
                            return Some(Step::Done(NameLookup::Symbol(ValueOrigin::Direct(
                                local,
                            ))));
                        }
                        // Projected slots here point at exports. The native
                        // locals entry has no requested meaning; try exports
                        // through the same scope traversal below.
                        continue;
                    }
                    let flags = self.symbol(local).flags;
                    if lookup_mask & meaning::VALUE != 0 && flags & flags::EXPORT_VALUE != 0 {
                        value_origin = value_origin.or(Some(ValueOrigin::Direct(local)));
                    }
                }
                if include_alias {
                    if let Some(entry) = self.inputs.store.entry(table, name) {
                        let direct = self.select_lookup_meanings(
                            Meanings::new(entry.slots.map(|symbol| self.find(symbol))),
                            table_requested.restricted(lookup_mask & meaning::ALL),
                            type_context,
                        );
                        if let Some(symbol) = direct.targets().symbols().next() {
                            return Some(Step::Done(NameLookup::Symbol(ValueOrigin::Direct(
                                symbol,
                            ))));
                        }
                        if lookup_mask & 8 != 0
                            && let Some(alias) = entry.alias
                            && (matches!(table_recipe, NameTable::Exports { .. })
                                || !self.inputs.file(alias.file).aliases()[alias.id.index()]
                                    .export_only)
                        {
                            return Some(Step::Done(NameLookup::Symbol(ValueOrigin::Alias(alias))));
                        }
                    }
                    continue;
                }
                let probed = self.probe_entry(
                    table,
                    name,
                    table_requested.restricted(lookup_mask),
                    type_context,
                    !matches!(table_recipe, NameTable::Exports { .. }),
                );
                value_origin = value_origin.or(probed.value_origin);
                match probed.probe {
                    Probe::Found(resolution) => {
                        return Some(Step::Done(NameLookup::Resolved(LexicalLookup {
                            resolution,
                            value_origin,
                        })));
                    }
                    Probe::Need(mut need) => {
                        need.value_origin = value_origin;
                        return Some(Step::Need(need));
                    }
                    Probe::Missing => {}
                }
            }
            if plan.arguments && name == "arguments" && requested.accepts(flags::FUNCTION_SCOPED) {
                return Some(Step::Done(NameLookup::Resolved(LexicalLookup {
                    resolution: Resolution::NotReference("implicit arguments object"),
                    value_origin: None,
                })));
            }
            None
        });
        match found {
            Some(found) => found,
            None => Step::Done(NameLookup::Resolved(LexicalLookup {
                resolution: Resolution::Unresolved(format!("cannot find `{name}`").into()),
                value_origin,
            })),
        }
    }

    /// An export of a module, following `export =`, local exports, and star
    /// re-exports. Conflicting star exports are ambiguous.
    fn resolve_export(&mut self, module: ModuleId, name: &str) -> Step<ExportLookup> {
        // A task with cuts sees a lookup no other task shares.
        let shared = !self
            .runtime
            .running
            .is_some_and(|task| self.runtime.cut_tasks.contains(&task));
        if !shared {
            return self.resolve_export_uncached(module, name);
        }
        if let Some(found) = self.runtime.export_memo.get(&(module, name.to_string())) {
            return Step::Done(found.clone());
        }
        let step = self.resolve_export_uncached(module, name);
        if let Step::Done(found) = &step {
            self.runtime
                .export_memo
                .insert((module, name.to_string()), found.clone());
        }
        step
    }

    fn resolve_export_uncached(&mut self, module: ModuleId, name: &str) -> Step<ExportLookup> {
        let mut current = module;
        let mut type_only = false;
        let mut seen = HashSet::from([module]);
        loop {
            if let Some(file) = self.inputs.store.module(current).json {
                return Step::Done(ExportLookup::direct(self.json_export(file, name)));
            }
            // A shorthand module has no actual export alias declaration.
            if self.inputs.store.module(current).shorthand {
                let module = self.inputs.store.module(current);
                let mut slots = Slots::only(meaning::ALL, module.symbol);
                for meaning in each(meaning::ALL) {
                    if let Some(symbol) = module.merged.get(meaning) {
                        slots.set(meaning, symbol);
                    }
                }
                return Step::Done(ExportLookup::direct(Resolution::Found(
                    Meanings::new(slots).restrict(type_only),
                )));
            }
            if name != "export=" {
                match self.probe_export(current, name) {
                    Probe::Found(found) => return Step::Done(found.restrict(type_only)),
                    Probe::Need(task) => return Step::Need(task),
                    Probe::Missing => {}
                }
            }
            if !self.inputs.exports_have(current, "export=") || name == "export=" {
                break;
            }
            let export = match self.probe_export(current, "export=") {
                Probe::Need(task) => return Step::Need(task),
                Probe::Found(found) => found,
                Probe::Missing => unreachable!("entry exists"),
            };
            let found = match export.resolution {
                Resolution::Found(found) => found,
                Resolution::Value {
                    target,
                    type_only: value_type_only,
                } => {
                    let type_only = type_only || value_type_only;
                    if name == "default" {
                        return Step::Done(ExportLookup {
                            resolution: Resolution::Value { target, type_only },
                            alias: export.alias,
                        });
                    }
                    return Step::Done(ExportLookup::direct(
                        self.value_property(&target, name, type_only)
                            .unwrap_or_else(|| no_property(name)),
                    ));
                }
                resolution => {
                    return Step::Done(ExportLookup {
                        resolution,
                        alias: export.alias,
                    });
                }
            };
            if name == "default" {
                return Step::Done(ExportLookup {
                    resolution: Resolution::Found(found.restrict(type_only)),
                    alias: export.alias,
                });
            }
            type_only |= found.members_type_only(Reach::ExportAssignment);
            let target = found
                .container()
                .and_then(|container| self.symbol(container).module);
            if let Some(target) = target {
                if !seen.insert(target) {
                    return Step::Done(ExportLookup::direct(Resolution::Unresolved(
                        "circular export =".into(),
                    )));
                }
                current = target;
                continue;
            }
            return self
                .member(found, name, meaning::ALL, true)
                .map(|found| found.restrict(type_only));
        }
        if name == "default" {
            return Step::Done(ExportLookup::direct(Resolution::Unresolved(
                "no default export".into(),
            )));
        }
        self.star_export(current, name, type_only)
    }

    /// `getExportsOfModule` keeps the raw selected symbol alongside resolved
    /// meanings. Pattern augmentation fills missing meanings and alias flags;
    /// a later route does not replace an already selected alias declaration.
    pub(super) fn probe_export(&self, module: ModuleId, name: &str) -> Probe<ExportLookup> {
        let data = self.inputs.store.module(module);
        let own = self.probe_export_entry(data.exports, name);
        let Some(pattern) = data.pattern else {
            return own;
        };
        let pattern = self.probe_export_entry(self.inputs.store.module(pattern).exports, name);
        match (own, pattern) {
            (Probe::Need(need), _) | (_, Probe::Need(need)) => Probe::Need(need),
            (Probe::Missing, pattern) => pattern,
            (Probe::Found(mut own), Probe::Found(pattern)) => {
                if let (Resolution::Found(found), Resolution::Found(more)) =
                    (&mut own.resolution, pattern.resolution)
                {
                    found.fill_missing(more, meaning::ALL, true);
                    own.alias = own.alias.or(pattern.alias);
                }
                Probe::Found(own)
            }
            (own, _) => own,
        }
    }

    /// A name reached through `export *`. Each module is searched by its most
    /// accessible route, and routes to the same declarations are one target;
    /// different declarations make the name ambiguous.
    pub(super) fn star_export(
        &self,
        module: ModuleId,
        name: &str,
        type_only: bool,
    ) -> Step<ExportLookup> {
        let mut best: HashMap<ModuleId, bool> = HashMap::from([(module, type_only)]);
        let mut stack: Vec<(ModuleId, bool)> = self
            .inputs
            .module_stars(module)
            .iter()
            .rev()
            .map(|&(star, star_type_only)| (star, type_only || star_type_only))
            .collect();
        let mut found: Vec<(Meanings, Option<AliasRef>)> = Vec::new();
        // Values, each once with whether every route to it is type-only.
        let mut values: Vec<(ValueTarget, bool, Option<AliasRef>)> = Vec::new();
        // Routes whose export of `name` does not resolve. They still export
        // the name, as TypeScript counts them: alone, the failure is the
        // result; beside another route, the name is ambiguous (TS2308).
        let mut failed: Vec<(ModuleId, ExportLookup)> = Vec::new();
        while let Some((star, route_type_only)) = stack.pop() {
            match best.get(&star) {
                Some(&previous) if previous <= route_type_only => continue,
                _ => {
                    best.insert(star, route_type_only);
                }
            }
            if self.inputs.store.module(star).shorthand {
                continue;
            }
            match self.probe_export(star, name) {
                Probe::Need(task) => return Step::Need(task),
                Probe::Found(ExportLookup {
                    resolution: Resolution::Found(targets),
                    alias,
                }) => {
                    let targets = targets.restrict(route_type_only);
                    match found
                        .iter_mut()
                        .find(|(other, _)| other.targets() == targets.targets())
                    {
                        Some((existing, _)) => existing.merge_routes(targets),
                        None => found.push((targets, alias)),
                    }
                }
                Probe::Found(ExportLookup {
                    resolution: Resolution::Value { target, type_only },
                    alias,
                }) => {
                    let type_only = type_only || route_type_only;
                    match values
                        .iter_mut()
                        .find(|(other, _, _)| self.same_value(other, &target))
                    {
                        Some((_, existing, _)) => *existing &= type_only,
                        None => values.push((target, type_only, alias)),
                    }
                }
                Probe::Found(resolution) => {
                    if !failed.iter().any(|(module, _)| *module == star) {
                        failed.push((star, resolution));
                    }
                }
                Probe::Missing => {
                    for &(next, next_type_only) in self.inputs.module_stars(star).iter().rev() {
                        stack.push((next, next_type_only || route_type_only));
                    }
                }
            }
        }
        Step::Done(match (found.as_slice(), values.as_slice(), failed.len()) {
            ([], [], 0) => ExportLookup::direct(Resolution::Unresolved(
                format!("`{name}` is not exported").into(),
            )),
            ([(targets, alias)], [], 0) => ExportLookup {
                resolution: Resolution::Found(*targets),
                alias: *alias,
            },
            ([], [(target, type_only, alias)], 0) => ExportLookup {
                resolution: Resolution::Value {
                    target: target.clone(),
                    type_only: *type_only,
                },
                alias: *alias,
            },
            ([], [], 1) => failed.pop().unwrap().1,
            _ => ExportLookup::direct(Resolution::Unresolved(
                format!("`{name}` is ambiguous among star exports").into(),
            )),
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
    ) -> Step<ExportLookup> {
        let Some(container) = qualifier.container() else {
            return Step::Done(ExportLookup::direct(Resolution::NotReference("property")));
        };
        // A value a merge met has its type's properties, as a value does.
        if let Some(target) = self.value_of(container) {
            let type_only = qualifier.members_type_only(Reach::ExportAssignment);
            return Step::Done(ExportLookup::direct(
                self.value_property(target, name, type_only)
                    .unwrap_or(Resolution::NotReference("property")),
            ));
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
            let entry = self.probe_entry(exports, name, mask.into(), type_context, false);
            match entry.probe {
                Probe::Found(resolution) => {
                    // `getExportsOfModule`/`getPropertyOfType` share native
                    // class-static and enum-member storage. The legacy lexical
                    // product excludes those selected members while admitting
                    // genuine namespace exports on a merged class or enum.
                    if let Resolution::Found(found) = &resolution
                        && let Some(reason) = found.targets().symbols().find_map(|member| {
                            let flags = self.symbol(member).flags;
                            if flags & flags::ENUM_MEMBER != 0 {
                                Some("enum member")
                            } else if flags
                                & (flags::PROPERTY | flags::METHOD | flags::GETTER | flags::SETTER)
                                != 0
                            {
                                Some("property")
                            } else {
                                None
                            }
                        })
                    {
                        return Step::Done(ExportLookup::direct(Resolution::NotReference(reason)));
                    }
                    return Step::Done(ExportLookup {
                        resolution,
                        alias: entry.type_alias_origin,
                    });
                }
                Probe::Need(task) => return Step::Need(task),
                Probe::Missing => {}
            }
        }
        let member_kinds =
            flags::CLASS | flags::FUNCTION | flags::FUNCTION_SCOPED | flags::BLOCK_SCOPED;
        Step::Done(ExportLookup::direct(if symbol.flags & flags::ENUM != 0 {
            Resolution::NotReference("enum member")
        } else if symbol.flags & member_kinds != 0
            || value.is_some()
            // A namespace that exports nothing has no export table either.
            || symbol.exports.is_none() && symbol.flags & flags::NAMESPACE == 0
        {
            Resolution::NotReference("property")
        } else {
            Resolution::Unresolved(format!("namespace has no exported member `{name}`").into())
        }))
    }

    /// An export of a module named by a qualifier. A shorthand ambient
    /// module does not list its exports.
    fn module_member(&mut self, module: ModuleId, name: &str) -> Step<ExportLookup> {
        if self.inputs.store.module(module).shorthand {
            return Step::Done(ExportLookup::direct(Resolution::Unresolved(
                format!("`{name}` is a member of a shorthand ambient module").into(),
            )));
        }
        self.resolve_export(module, name)
    }

    /// A member of `globalThis`: a global type, or a global value other than
    /// a block-scoped variable.
    fn global_member(&self, name: &str, mask: u8, type_context: bool) -> Step<ExportLookup> {
        let entry = self.probe_entry(GLOBAL, name, mask.into(), type_context, false);
        let found = match entry.probe {
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
        Step::Done(ExportLookup {
            resolution: match found {
                Some(found) => Resolution::Found(found),
                None => Resolution::Unresolved(format!("`{name}` is not a global property").into()),
            },
            alias: entry.type_alias_origin,
        })
    }

    /// The value of a JSON module: its only export, `export=`
    /// (`bindSourceFileIfExternalModule`).
    pub(super) fn json_value(file: FileId) -> Resolution {
        Resolution::Value {
            target: ValueTarget::Json {
                file,
                value: super::JsonFacts::ROOT,
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
            value: super::JsonFacts::ROOT,
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
        let json = self.inputs.file(file).json().expect("a JSON module's file");
        if let Some(child) = json.property(at, name) {
            return Some(value(ValueTarget::Json { file, value: child }));
        }
        let interface = json.shape(at).interface()?;
        let entry = self.inputs.store.table(GLOBAL).get(interface)?;
        let symbol = self.find(entry.slots.get(meaning::TYPE)?);
        let members = self.inputs.store.interface_members(symbol)?;
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
        if let Some(file) = self.inputs.store.module(module).json {
            return Step::Done(Self::json_value(file));
        }
        if !self.inputs.exports_have(module, "export=") {
            let symbol = self.inputs.store.module(module).symbol;
            let mut slots = Slots::only(self.meanings_of(symbol), symbol);
            for meaning in each(meaning::ALL) {
                if let (None, Some(merged)) = (
                    slots.get(meaning),
                    self.inputs.store.module(module).merged.get(meaning),
                ) {
                    slots.set(meaning, merged);
                }
            }
            return Step::Done(Resolution::Found(Meanings::new(slots)));
        }
        match self.probe_export(module, "export=") {
            Probe::Need(task) => Step::Need(task),
            Probe::Found(found) => Step::Done(found.resolution),
            Probe::Missing => unreachable!("entry exists"),
        }
    }

    fn try_alias(&mut self, alias: AliasRef) -> Step<Resolution> {
        let facts = self.inputs.file(alias.file);
        let data = &facts.aliases()[alias.id.index()];
        let mut value_origin = None;
        let step = match &data.kind {
            AliasKind::Import { request, name } => {
                match self.inputs.request_module(alias.file, *request) {
                    Ok(module) => match self
                        .resolve_export(module, name)
                        .map(|found| found.resolution)
                    {
                        // A default import of a declaration module without a
                        // default export or an `__esModule` marker is its module
                        // object (synthetic default).
                        Step::Done(Resolution::Unresolved(failure))
                            if failure.dependency.is_none()
                                && name == "default"
                                && self.inputs.store.module(module).declaration
                                && !["default", "__esModule"]
                                    .iter()
                                    .any(|name| self.inputs.exports_have(module, name))
                                && matches!(data.bind, AliasBind::Lexical { .. }) =>
                        {
                            self.module_object(module)
                        }
                        step => step,
                    },
                    Err(why) => Step::Done(Resolution::Unresolved(
                        ResolutionFailure::from(why)
                            .at_dependency(alias.file, facts.requests()[request.index()].span),
                    )),
                }
            }
            AliasKind::Namespace { request } => {
                match self.inputs.request_module(alias.file, *request) {
                    Ok(module) => self.module_object(module),
                    Err(why) => Step::Done(Resolution::Unresolved(
                        ResolutionFailure::from(why)
                            .at_dependency(alias.file, facts.requests()[request.index()].span),
                    )),
                }
            }
            AliasKind::SelfModule => match self.inputs.file_modules[alias.file.index()] {
                Some(module) => self.module_object(module),
                None => Step::Done(Resolution::Unresolved("file is not a module".into())),
            },
            AliasKind::Lexical { scope, name } => {
                match self.lookup(alias.file, *scope, name, meaning::ALL, false) {
                    Step::Done(found) => {
                        value_origin = found.value_origin;
                        Step::Done(found.resolution)
                    }
                    Step::Need(need) => {
                        value_origin = need.value_origin;
                        Step::Need(need)
                    }
                    Step::Failed(failure) => Step::Failed(failure),
                }
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
        let type_only = data.type_only;
        self.states.set_origin(Task::Alias(alias), value_origin);
        match step {
            Step::Done(resolution) => Step::Done(restrict(
                match resolution {
                    Resolution::Unresolved(failure) => {
                        Resolution::Unresolved(failure.at_dependency(alias.file, data.span))
                    }
                    other => other,
                },
                type_only,
            )),
            need => need,
        }
    }

    fn try_ref(&mut self, reference: RefRef) -> Step<Resolution> {
        let data = self.inputs.reference(reference);
        let name = data.name;
        let step = match data.base {
            // Lookup already selects the meanings the reference needs from
            // each table it probes, by the same rule as `mask_resolution`.
            RefBase::Lexical(scope) => {
                return match self.lookup(
                    reference.file,
                    scope,
                    name,
                    data.meaning,
                    data.type_context,
                ) {
                    Step::Done(found) => {
                        let alias = if data.meaning & meaning::TYPE != 0 {
                            match self.lookup_alias(reference.file, scope, name) {
                                Ok(alias) => alias,
                                Err(failure) => return Step::Failed(failure),
                            }
                        } else {
                            None
                        };
                        self.states
                            .set_origin(Task::Ref(reference), found.value_origin);
                        self.states.set_type_alias(reference, alias);
                        Step::Done(found.resolution)
                    }
                    Step::Need(need) => {
                        self.states
                            .set_origin(Task::Ref(reference), need.value_origin);
                        Step::Need(need)
                    }
                    Step::Failed(failure) => Step::Failed(failure),
                };
            }
            RefBase::Member(base) => {
                let base = Task::Ref(RefRef {
                    file: reference.file,
                    id: base,
                });
                // `globalThis` now has its actual initialized header. Its
                // lexical property policy remains the existing global-member
                // operation, including the block-scoped value restriction.
                if self.done(base).is_some_and(|resolution| matches!(resolution,
                    Resolution::Found(found) if found.targets().symbols().any(|symbol| self.inputs.store.is_global_this(symbol)))) {
                    return match self.global_member(name, data.meaning, data.type_context) {
                        Step::Done(found) => {
                            self.states.set_type_alias(reference, if data.meaning & meaning::TYPE != 0 { found.alias } else { None });
                            Step::Done(found.resolution)
                        }
                        Step::Need(need) => Step::Need(need),
                        Step::Failed(failure) => Step::Failed(failure),
                    };
                }
                let qualifier = match self.done(base) {
                    None => return Step::Need(Need::required(base)),
                    Some(Resolution::Found(found)) => *found,
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
                    Some(Resolution::Unresolved(failure)) if failure.dependency.is_some() => {
                        return Step::Done(Resolution::Unresolved(failure.clone()));
                    }
                    Some(Resolution::Unresolved(_)) if data.property => {
                        return Step::Done(Resolution::NotReference("property"));
                    }
                    Some(Resolution::Unresolved(_)) => {
                        return Step::Done(Resolution::Unresolved(
                            format!("the qualifier of `{name}` is unresolved").into(),
                        ));
                    }
                };
                match self.member(qualifier, name, data.meaning, data.type_context) {
                    Step::Done(found) => {
                        self.states.set_type_alias(
                            reference,
                            if data.meaning & meaning::TYPE != 0 {
                                found.alias
                            } else {
                                None
                            },
                        );
                        Step::Done(restrict(
                            found.resolution,
                            qualifier.members_type_only(Reach::Qualified),
                        ))
                    }
                    Step::Need(need) => Step::Need(need),
                    Step::Failed(failure) => Step::Failed(failure),
                }
            }
            RefBase::Module(request) => match self.inputs.request_module(reference.file, request) {
                // An import type's qualifier names a property of a JSON
                // module's value, with no synthetic default.
                Ok(module) => match self.inputs.store.module(module).json {
                    Some(file) => {
                        let value = ValueTarget::Json {
                            file,
                            value: super::JsonFacts::ROOT,
                        };
                        Step::Done(
                            self.value_property(&value, name, false)
                                .unwrap_or_else(|| no_property(name)),
                        )
                    }
                    None => match self.module_member(module, name) {
                        Step::Done(found) => {
                            self.states.set_type_alias(
                                reference,
                                if data.meaning & meaning::TYPE != 0 {
                                    found.alias
                                } else {
                                    None
                                },
                            );
                            Step::Done(found.resolution)
                        }
                        Step::Need(need) => Step::Need(need),
                        Step::Failed(failure) => Step::Failed(failure),
                    },
                },
                Err(why) => Step::Done(Resolution::Unresolved(
                    ResolutionFailure::from(why).at_dependency(
                        reference.file,
                        self.inputs.file(reference.file).requests()[request.index()].span,
                    ),
                )),
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
}

impl Files<'_> {
    /// `resolveNameHelper`'s ancestor schedule. Result policy and table
    /// representation do not change which scope or final globals are visited.
    fn lookup_scope_chain(
        &self,
        file: FileId,
        scope: ScopeId,
    ) -> impl Iterator<Item = Option<ScopeId>> + '_ {
        let mut current = Some(scope);
        let mut saw_global = false;
        let mut finished = false;
        std::iter::from_fn(move || {
            if let Some(index) = current {
                current = self.file(file).scopes()[index.index()].parent;
                saw_global |= matches!(self.storage(file, index), Storage::Script);
                Some(Some(index))
            } else if !finished && !saw_global {
                finished = true;
                Some(None)
            } else {
                None
            }
        })
    }
    /// Where a scope stores its names. Every scope kind is decided here, for
    /// both declaration and lookup.
    pub(super) fn storage(&self, file: FileId, scope: ScopeId) -> Storage {
        match self.file(file).scopes()[scope.index()].kind {
            ScopeKind::Root if !self.file(file).module() => Storage::Script,
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
    /// `resolveNameHelper`: infer and enum introductions precede table
    /// lookup; implicit arguments follows all tables in its function scope.
    fn name_scope(&self, file: FileId, index: Option<ScopeId>) -> NameScope {
        let mut plan = NameScope {
            index,
            infer: None,
            enum_: None,
            arguments: false,
            tables: SmallVec::new(),
        };
        let Some(index) = index else {
            plan.tables.push(NameTable::Global);
            return plan;
        };
        let kind = self.file(file).scopes()[index.index()].kind;
        match kind {
            ScopeKind::InferConstraint { decl } => plan.infer = Some(decl),
            ScopeKind::Enum { decl } => plan.enum_ = Some(decl),
            ScopeKind::Signature { arguments } => plan.arguments = arguments,
            _ => {}
        }
        match self.storage(file, index) {
            Storage::Script => plan.tables.push(NameTable::Global),
            Storage::Members(decl) => plan.tables.push(NameTable::Members(decl)),
            Storage::Own(host) => {
                plan.tables.push(NameTable::Locals(index));
                // SourceFile Locals contain paired export locals with only
                // ExportValue/None flags. `resolveNameHelper` then visits raw
                // Exports; this lookup step does not change declaration storage.
                let host = host.or_else(|| matches!(kind, ScopeKind::Root).then_some(Host::File));
                if let Some(host) = host {
                    let external = match kind {
                        ScopeKind::Root
                        | ScopeKind::AmbientModule { .. }
                        | ScopeKind::Augmentation { .. } => true,
                        ScopeKind::Namespace { decl } => {
                            self.file(file).decls()[decl.index()].ambient
                        }
                        _ => false,
                    };
                    plan.tables.push(NameTable::Exports { host, external });
                }
            }
        }
        plan
    }
}
