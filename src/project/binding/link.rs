//! Project-wide declaration tables, module exports, aliases, and reference
//! resolution.
//!
//! A table entry holds one symbol per meaning plus an optional alias, so a
//! same-named type and value stay separate unless their declarations merge.
//! Aliases and qualified references resolve on demand through one explicit
//! task stack; cycles become unresolved targets rather than recursion.

mod raw;

use super::collect::{
    AliasBind, AliasKind, BindingEntry, DeclKind, Facts, Host, MisplacedModule, NameTarget,
    ScopeKind,
};
use super::ids::{
    AliasId, DeclId, ModuleId, NameId, RequestId, ScopeId, SymbolId, TableId, TypeParameterListId,
};
use super::index::{self, Index, Site, Target, Token};
use super::instances::Instance;
use super::member::{MemberKey, MemberKind};
use super::resolution::{ExportLookup, Probe, RefRef, State, Step, Storage, Task};
use super::retained::Linked;
use super::store::{
    AliasRef, BindingTable, DeclaredSymbol, Direction, GLOBAL, RequestTarget, StarRequest, Store,
    Symbol, TwoModules,
};
use super::targets::{Meanings, Slots, each};
use super::{FileFacts, ModuleResolution, RequestKind, meaning};
use crate::Span;
use crate::analysis::{InputError, InputErrorKind};
use crate::project;
use crate::project::program::Program;
use crate::project::sources::FileId;
use crate::syntax_kind::SyntaxKind;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

/// TypeScript symbol flags that decide which declarations may share a name.
pub(super) mod flags {
    pub(in crate::project::binding) const FUNCTION_SCOPED: u32 = 1;
    pub(in crate::project::binding) const BLOCK_SCOPED: u32 = 1 << 1;
    pub(in crate::project::binding) const FUNCTION: u32 = 1 << 2;
    pub(in crate::project::binding) const CLASS: u32 = 1 << 3;
    pub(in crate::project::binding) const INTERFACE: u32 = 1 << 4;
    pub(in crate::project::binding) const CONST_ENUM: u32 = 1 << 5;
    pub(in crate::project::binding) const REGULAR_ENUM: u32 = 1 << 6;
    pub(in crate::project::binding) const VALUE_MODULE: u32 = 1 << 7;
    pub(in crate::project::binding) const NAMESPACE_MODULE: u32 = 1 << 8;
    pub(in crate::project::binding) const TYPE_ALIAS: u32 = 1 << 9;
    pub(in crate::project::binding) const TYPE_PARAMETER: u32 = 1 << 10;
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
    pub(in crate::project::binding) const PROPERTY: u32 = 1 << 17;
    pub(in crate::project::binding) const METHOD: u32 = 1 << 18;
    pub(in crate::project::binding) const GETTER: u32 = 1 << 19;
    pub(in crate::project::binding) const SETTER: u32 = 1 << 20;
    pub(in crate::project::binding) const SIGNATURE: u32 = 1 << 21;
    pub(super) const CONSTRUCTOR: u32 = 1 << 22;
    pub(in crate::project::binding) const OPTIONAL: u32 = 1 << 23;
    pub(in crate::project::binding) const TYPE_LITERAL: u32 = 1 << 24;
    pub(in crate::project::binding) const OBJECT_LITERAL: u32 = 1 << 25;
    pub(in crate::project::binding) const PROTOTYPE: u32 = 1 << 26;
    pub(in crate::project::binding) const ENUM_MEMBER: u32 = 1 << 27;
    pub(in crate::project::binding) const EXPORT_VALUE: u32 = 1 << 28;
    pub(in crate::project::binding) const ALIAS: u32 = 1 << 29;
    pub(in crate::project::binding) const CONST_ENUM_ONLY_MODULE: u32 = 1 << 30;
    pub(in crate::project::binding) const MODULE_EXPORTS: u32 = 1 << 31;
    pub(in crate::project::binding) const ENUM: u32 = CONST_ENUM | REGULAR_ENUM;
    pub(in crate::project::binding) const NAMESPACE: u32 = VALUE_MODULE | NAMESPACE_MODULE;
    pub(in crate::project::binding) const VALUE: u32 = FUNCTION_SCOPED
        | BLOCK_SCOPED
        | FUNCTION
        | CLASS
        | ENUM
        | ENUM_MEMBER
        | VALUE_MODULE
        | EXPRESSION
        | PROPERTY
        | OBJECT_LITERAL
        | METHOD
        | GETTER
        | SETTER;
    /// Native SymbolFlagsModuleMember; excludes ordinary properties and enum members.
    pub(in crate::project::binding) const MODULE_MEMBER: u32 = FUNCTION_SCOPED
        | BLOCK_SCOPED
        | FUNCTION
        | CLASS
        | INTERFACE
        | ENUM
        | NAMESPACE
        | TYPE_ALIAS
        | ALIAS;
    pub(in crate::project::binding) const TYPE: u32 =
        CLASS | INTERFACE | ENUM | ENUM_MEMBER | TYPE_ALIAS | TYPE_PARAMETER;
}

pub(super) fn symbol_flags(kind: DeclKind) -> u32 {
    use flags::*;
    match kind {
        DeclKind::Var => FUNCTION_SCOPED,
        DeclKind::Parameter => FUNCTION_SCOPED | PARAMETER,
        DeclKind::Let => BLOCK_SCOPED,
        DeclKind::Function => FUNCTION,
        DeclKind::Class => CLASS,
        DeclKind::Interface => INTERFACE,
        DeclKind::TypeLiteral => TYPE_LITERAL,
        DeclKind::ObjectLiteral => OBJECT_LITERAL,
        DeclKind::TypeAlias => TYPE_ALIAS,
        DeclKind::Enum { constant: true } => CONST_ENUM,
        DeclKind::Enum { constant: false } => REGULAR_ENUM,
        DeclKind::EnumMember => ENUM_MEMBER,
        DeclKind::Namespace {
            instance: Instance::NonInstantiated,
        } => NAMESPACE_MODULE,
        DeclKind::Namespace { .. } => VALUE_MODULE,
        DeclKind::TypeParameter => TYPE_PARAMETER,
        DeclKind::Member(kind) => match kind {
            MemberKind::Property | MemberKind::ParameterProperty => PROPERTY,
            MemberKind::AutoAccessor => GETTER | SETTER,
            MemberKind::Method => METHOD,
            MemberKind::Getter => GETTER,
            MemberKind::Setter => SETTER,
            MemberKind::Call | MemberKind::Construct | MemberKind::Index => SIGNATURE,
            MemberKind::Constructor => CONSTRUCTOR,
        },
        DeclKind::Expression => EXPRESSION,
    }
}

/// Binder `bindWorker`'s actual SymbolFlags, without navigation's auxiliary
/// declaration classes. An export-assignment expression declares a Property;
/// Parameter/Module/implementation metadata are not independent native bits.
pub(super) fn native_symbol_flags(projected: u32) -> u32 {
    let expression = projected & flags::EXPRESSION != 0;
    let flags = projected
        & !(flags::PARAMETER
            | flags::EXPRESSION
            | flags::MODULE
            | flags::IMPLEMENTED
            | flags::AMBIENT_CLASS
            | flags::CONCRETE_CLASS);
    flags | if expression { flags::PROPERTY } else { 0 }
}

/// The complete native declaration exclusion question is separate from the
/// navigation projection's duplicate-parameter classification. Export
/// assignments use SymbolFlagsAll in `bindExportAssignment`.
pub(super) fn native_declaration_excludes(projected: u32) -> u32 {
    if projected & flags::EXPRESSION != 0 {
        return u32::MAX;
    }
    excluded(native_symbol_flags(projected))
}

/// The meanings a symbol with `flags` has of its own: the one rule for a
/// symbol's meanings, which its declarations' kinds and alias expansion use.
/// A module is a value and a namespace. Contextual meanings stay with their
/// rule: any export of a shorthand ambient module has every meaning, and a
/// type-only route restricts the meanings it reaches (see `targets`).
pub(super) fn meanings(flags: u32) -> u8 {
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
            DeclKind::Class | DeclKind::Enum { .. } | DeclKind::EnumMember => VALUE | TYPE,
            DeclKind::Interface | DeclKind::TypeAlias | DeclKind::TypeParameter => TYPE,
            DeclKind::TypeLiteral | DeclKind::ObjectLiteral => 0,
            DeclKind::Namespace {
                instance: Instance::NonInstantiated,
            } => NAMESPACE,
            DeclKind::Namespace { .. } => NAMESPACE | VALUE,
            DeclKind::Member(kind) => meanings(symbol_flags(DeclKind::Member(kind))),
        }
    }
}

/// TypeScript's `getExcludedSymbolFlags`: the flags a symbol with `flags`
/// cannot share a name with, each kind's `*Excludes` combined.
pub(super) fn excluded(flags: u32) -> u32 {
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
        (ENUM_MEMBER, VALUE | TYPE),
        (
            VALUE_MODULE,
            VALUE & !(FUNCTION | CLASS | REGULAR_ENUM | VALUE_MODULE),
        ),
        (TYPE_ALIAS, TYPE),
        (ALIAS, ALIAS),
        (TYPE_PARAMETER, TYPE & !TYPE_PARAMETER),
        (PROPERTY, VALUE & !(PROPERTY | GETTER | SETTER)),
        (METHOD, VALUE & !METHOD),
        (GETTER, VALUE & !(SETTER | PROPERTY)),
        (SETTER, VALUE & !(GETTER | PROPERTY)),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Resolution {
    Found(Meanings),
    /// A value that is not a reference target but whose identity and
    /// properties linking still needs (`ValueTarget`). It has only the value
    /// meaning; `type_only` marks a type-only route, after which only a type
    /// context may use it, as for a restricted symbol (`Meanings::select`).
    Value {
        target: ValueTarget,
        type_only: bool,
    },
    NotReference(&'static str),
    Unresolved(ResolutionFailure),
}

/// `resolveEntityName(ignoreErrors)` suppresses only its own missing-name
/// diagnostic. Errors acquired while resolving an Alias or module remain
/// consumed dependencies, even when the final name lookup ignores errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolutionFailure {
    pub(super) reason: String,
    pub(super) dependency: Option<(FileId, crate::Span)>,
}
impl ResolutionFailure {
    pub(super) fn at_dependency(mut self, file: FileId, span: crate::Span) -> Self {
        if self.dependency.is_none() {
            self.dependency = Some((file, span));
        }
        self
    }
}
impl From<String> for ResolutionFailure {
    fn from(reason: String) -> Self {
        Self {
            reason,
            dependency: None,
        }
    }
}
impl From<&str> for ResolutionFailure {
    fn from(reason: &str) -> Self {
        Self::from(reason.to_owned())
    }
}
impl std::ops::Deref for ResolutionFailure {
    type Target = str;
    fn deref(&self) -> &str {
        &self.reason
    }
}

/// A value that linking follows but that is not a reference target: a JSON
/// module's value or a value an object key reaches in it, or a member of the
/// library interface that is such a value's apparent type. TypeScript gives
/// each a property symbol (the file's `export=`, a transient property of the
/// value's type, an interface member), which star exports compare and whose
/// type's properties a later name selects (`Linker::value_property`). The
/// index reports it as not a reference target (`Linker::target`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ValueTarget {
    /// A value of a JSON file, by its index among the file's values
    /// (`super::JsonFacts`): `JsonFacts::ROOT` is the file's value.
    Json {
        file: FileId,
        value: super::JsonValueId,
    },
    /// A member of a global library interface, by the interface's canonical
    /// symbol when found. Boxed, so a resolution, which every reference's
    /// state holds, is no larger for it.
    Member { interface: SymbolId, name: Box<str> },
}

/// Whether a use with `mask` in a type context or not may select a value
/// reached through a type-only route or not: the rule `Meanings::select`
/// applies to a symbol's value meaning.
pub(super) fn selects_value(mask: u8, type_only: bool, type_context: bool) -> bool {
    mask & meaning::VALUE != 0 && !(type_only && mask == meaning::VALUE && !type_context)
}

/// `getResolvedSymbol` preserves a lexical alias symbol even when value
/// resolution follows it. Direct slots and alias slots ask different native
/// identifier setup and reference-identity questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ValueOrigin {
    Direct(SymbolId),
    Alias(AliasRef),
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
    parent: Option<SymbolId>,
}

/// Where one of a script's ambient module declarations binds, as
/// TypeScript's binder gives each file's declarations of one name one symbol
/// in the file's locals: the module of that name for the first file in
/// program order that declares it, and for any later file a fragment, its
/// own symbol and export table, which the checker merges into the module
/// (`merge_globals`).
#[derive(Debug, Clone, Copy)]
pub(super) enum AmbientHost {
    Module(ModuleId),
    /// An index into `Linker::fragments`.
    Fragment(usize),
}

/// A later file's declarations of an ambient module (`AmbientHost`).
#[derive(Debug)]
pub(super) struct Fragment {
    module: ModuleId,
    pub(super) symbol: SymbolId,
    /// The fragment's `export *` declarations, which the module gains when
    /// the fragment merges into it.
    stars: Vec<StarRequest>,
}

pub(in crate::project) struct LinkState {
    /// Tables, symbols, modules, and request targets, whose writes
    /// invalidate resolutions made while merges still change them.
    store: Store,
    scope_tables: Vec<Vec<Option<TableId>>>,
    /// The symbol each declaration created or merged into. It may since have
    /// merged into another, so readers canonicalize it with `find`.
    decl_symbols: Vec<Vec<Option<SymbolId>>>,
    decl_local_symbols: Vec<Vec<Option<SymbolId>>>,
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
    /// Parallel to reference states during linking, then moved into their
    /// retained attachments. Only the existing lexical lookup supplies this
    /// raw identity; qualified/module references require other native owners.
    ref_origins: Vec<Vec<Option<ValueOrigin>>>,
    ref_type_alias_origins: Vec<Vec<Option<AliasRef>>>,
    /// The same lookup's raw identity for local export aliases, required by
    /// native assignment marking without replaying lookup or following aliases.
    alias_origins: Vec<Vec<Option<ValueOrigin>>>,
    resolution: super::resolution::Runtime,
    /// The value each value symbol stands for (`value_symbol`), and the
    /// symbol of each value a checker merge met.
    values: HashMap<SymbolId, ValueTarget>,
    value_symbols: HashMap<ValueTarget, SymbolId>,
    conflicts: Vec<(TableId, String)>,
}

impl std::fmt::Debug for LinkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkState").finish_non_exhaustive()
    }
}

struct Linker<'p, 'l> {
    program: &'p Program,
    state: &'l mut LinkState,
    native_inputs: Option<super::NativeInputs<'l>>,
    native_facts: Option<&'l dyn super::NativeFacts>,
}

type Result<T> = std::result::Result<T, crate::analysis::AnalysisError>;

/// Why a JSON module's value is not a reference target.
const JSON_VALUE: &str = "JSON module value";

pub(super) fn link(program: &Program, parser: &crate::analysis::Parser) -> Result<Linked> {
    let mut state = LinkState::new(program);
    let raw = {
        let mut linker = Linker {
            program,
            state: &mut state,
            native_inputs: None,
            native_facts: None,
        };
        linker.check_mixed_exports()?;
        linker.check_module_declarations()?;
        linker.create_modules()?;
        linker.create_augmentations();
        linker.resolve_requests()?;
        linker.declare_all()?;
        linker.prepare_checker_inputs()?
    };
    let linked = super::super::semantic::initialize_loading(program, &raw, parser, &mut state)?;
    Ok(state.complete(raw, linked))
}

pub(in crate::project) struct NativeLinkOutput {
    index: Index,
    reference_sources: Vec<Vec<super::ids::OccurrenceId>>,
}

pub(in crate::project) struct ProjectionObserver<'p, 'a> {
    pub(in crate::project) program: &'p Program,
    pub(in crate::project) raw: &'a super::raw::RawBindingSnapshot,
    pub(in crate::project) state: &'a mut LinkState,
}
impl super::super::semantic::InitializationObserver for ProjectionObserver<'_, '_> {
    fn observe(
        &mut self,
        step: super::super::semantic::InitializationStep,
        facts: &dyn super::NativeFacts,
    ) -> Result<()> {
        Linker {
            program: self.program,
            state: self.state,
            native_inputs: Some(super::NativeInputs::new(
                super::inputs::BindingFiles::Loading(self.program),
                self.raw,
            )),
            native_facts: Some(facts),
        }
        .observe_initialization(step)
    }
}

impl LinkState {
    fn new(program: &Program) -> Self {
        let files = &program.files;
        let symbols = files
            .iter()
            .map(|file| file.facts.facts.decls.len() + file.facts.facts.aliases.len() + 1)
            .sum();
        Self {
            store: Store::new(symbols),
            scope_tables: files
                .iter()
                .map(|f| vec![None; f.facts.facts.scopes.len()])
                .collect(),
            decl_symbols: files
                .iter()
                .map(|f| vec![None; f.facts.facts.decls.len()])
                .collect(),
            decl_local_symbols: files
                .iter()
                .map(|file| vec![None; file.facts.facts.decls.len()])
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
            ref_origins: files
                .iter()
                .map(|f| vec![None; f.facts.facts.refs.len()])
                .collect(),
            ref_type_alias_origins: files
                .iter()
                .map(|f| vec![None; f.facts.facts.refs.len()])
                .collect(),
            alias_origins: files
                .iter()
                .map(|f| vec![None; f.facts.facts.aliases.len()])
                .collect(),
            resolution: super::resolution::Runtime::loading(),
            values: HashMap::new(),
            value_symbols: HashMap::new(),
            conflicts: Vec::new(),
        }
    }

    fn complete(mut self, raw: super::raw::RawBindingSnapshot, output: NativeLinkOutput) -> Linked {
        assert!(
            self.resolution.running.is_none(),
            "completed linking has no active demand"
        );
        // Final publication must not hide an incomplete lexical task even
        // though its working states no longer survive in the Project.
        assert!(
            self.alias_states
                .iter()
                .chain(&self.ref_states)
                .flat_map(|states| states.iter())
                .all(|state| !matches!(state, State::InProgress)),
            "completed linking has no demand in progress"
        );
        Linked {
            index: output.index,
            reference_sources: output.reference_sources,
            reference_origins: std::mem::take(&mut self.ref_origins),
            reference_type_alias_origins: std::mem::take(&mut self.ref_type_alias_origins),
            raw,
            link_state: self,
        }
    }
    pub(in crate::project) fn finish_with_native_facts(
        &mut self,
        program: &Program,
        raw: &super::raw::RawBindingSnapshot,
        facts: &dyn super::NativeFacts,
    ) -> Result<NativeLinkOutput> {
        let mut linker = Linker {
            program,
            state: self,
            native_inputs: Some(super::NativeInputs::new(
                super::inputs::BindingFiles::Loading(program),
                raw,
            )),
            native_facts: Some(facts),
        };
        linker.check_hoisting()?;
        linker.check_merges()?;
        let (index, reference_sources) = linker.resolve_and_index()?;
        linker.check_conflicts()?;
        Ok(NativeLinkOutput {
            index,
            reference_sources,
        })
    }
}

impl<'p> Linker<'p, '_> {
    fn inputs(&self) -> super::resolution::Inputs<'_> {
        super::resolution::Inputs {
            files: super::resolution::Files::Loading(self.program),
            hosts: super::resolution::ProjectedHosts::Loading {
                ambient_hosts: &self.state.ambient_hosts,
                fragments: &self.state.fragments,
            },
            store: &self.state.store,
            scope_tables: &self.state.scope_tables,
            decl_symbols: &self.state.decl_symbols,
            file_modules: &self.state.file_modules,
            augmentations: &self.state.augmentations,
            global_augmentations: &self.state.global_augmentations,
            values: &self.state.values,
            native_inputs: self.native_inputs,
            native_facts: self.native_facts,
        }
    }
    fn resolver(&mut self) -> super::resolution::Resolver<'_> {
        super::resolution::Resolver {
            inputs: super::resolution::Inputs {
                files: super::resolution::Files::Loading(self.program),
                hosts: super::resolution::ProjectedHosts::Loading {
                    ambient_hosts: &self.state.ambient_hosts,
                    fragments: &self.state.fragments,
                },
                store: &self.state.store,
                scope_tables: &self.state.scope_tables,
                decl_symbols: &self.state.decl_symbols,
                file_modules: &self.state.file_modules,
                augmentations: &self.state.augmentations,
                global_augmentations: &self.state.global_augmentations,
                values: &self.state.values,
                native_inputs: self.native_inputs,
                native_facts: self.native_facts,
            },
            states: super::resolution::States::Loading {
                aliases: &mut self.state.alias_states,
                refs: &mut self.state.ref_states,
                alias_origins: &mut self.state.alias_origins,
                ref_origins: &mut self.state.ref_origins,
                ref_type_alias_origins: &mut self.state.ref_type_alias_origins,
            },
            runtime: &mut self.state.resolution,
        }
    }
    fn json_value(file: FileId) -> Resolution {
        super::resolution::Resolver::json_value(file)
    }
    fn force(&mut self, task: Task) -> Result<Resolution> {
        if self.state.store.take_changed() {
            self.reset_resolutions();
        }
        self.resolver().force(task).map_err(|failure| {
            let site = match task {
                Task::Alias(alias) => (
                    alias.file,
                    self.facts(alias.file).aliases[alias.id.index()].span,
                ),
                Task::Ref(reference) => {
                    let facts = self.facts(reference.file);
                    let token = facts.refs[reference.id.index()].token;
                    (reference.file, facts.names[token.index()].span)
                }
            };
            self.lookup_failure(failure, Some(site))
        })
    }

    /// The Project::load boundary for an incomplete native name observation.
    /// Resolver failures stay typed until here; none become the navigation
    /// domain's ordinary missing-name result or an ignored dependency.
    fn lookup_failure(
        &self,
        failure: impl Into<super::LookupFailure>,
        site: Option<(FileId, Span)>,
    ) -> crate::analysis::AnalysisError {
        use super::semantic::TypeBindingFailure;
        let failure = match failure.into() {
            super::LookupFailure::Binding(failure) => failure,
            super::LookupFailure::Native(failure) => return failure,
        };
        let (kind, message, site) = match failure {
            TypeBindingFailure::NotRetained => (
                InputErrorKind::Binding,
                "native name lookup invariant failed: required symbol metadata was not retained"
                    .to_owned(),
                site,
            ),
            TypeBindingFailure::Pending(reason) => (
                InputErrorKind::Unsupported,
                format!("native name lookup could not complete: {reason}"),
                site,
            ),
            TypeBindingFailure::Unresolved(reason) => (
                InputErrorKind::Binding,
                format!("native name lookup failed: {reason}"),
                site,
            ),
            TypeBindingFailure::ErrorType(reason) => (
                InputErrorKind::Binding,
                format!("native name lookup selected errorType: {reason}"),
                site,
            ),
            TypeBindingFailure::Dependency { file, span, reason } => (
                InputErrorKind::Binding,
                format!("native name lookup dependency failed: {reason}"),
                Some((file, span)),
            ),
        };
        match site {
            Some((file, span)) => self.error(kind, file, span, message).into(),
            None => InputError {
                kind,
                message,
                path: None,
                position: None,
            }
            .into(),
        }
    }
    fn probe_export(&mut self, module: ModuleId, name: &str) -> Probe<ExportLookup> {
        self.resolver().probe_export(module, name)
    }
    fn star_export(&mut self, module: ModuleId, name: &str, type_only: bool) -> Step<ExportLookup> {
        self.resolver().star_export(module, name, type_only)
    }
    fn request_module(
        &self,
        file: FileId,
        request: RequestId,
    ) -> std::result::Result<ModuleId, &'static str> {
        self.inputs().request_module(file, request)
    }
    fn storage(&self, file: FileId, scope: ScopeId) -> Storage {
        self.inputs().files.storage(file, scope)
    }
    fn exports_have(&self, module: ModuleId, name: &str) -> bool {
        self.inputs().exports_have(module, name)
    }
    fn module_stars(&self, module: ModuleId) -> SmallVec<[(ModuleId, bool); 2]> {
        self.inputs().module_stars(module)
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
    fn native_merge_failure(
        &self,
        failure: super::TypeBindingFailure,
        _source: Option<super::ids::NativeHeaderId>,
    ) -> crate::analysis::AnalysisError {
        self.lookup_failure(failure, None)
    }

    /// The canonical symbol after merges both ways.
    fn find(&self, id: SymbolId) -> SymbolId {
        self.state.store.find(id)
    }
    fn symbol(&self, id: SymbolId) -> &Symbol {
        self.state.store.symbol(id)
    }

    fn new_module(&mut self, site: Option<Site>, shorthand: bool, declaration: bool) -> ModuleId {
        let flags = flags::MODULE | flags::VALUE_MODULE;
        self.state
            .store
            .new_module(flags, site, shorthand, declaration)
    }

    fn create_modules(&mut self) -> Result<()> {
        for file in self.file_ids() {
            if self.file(file).module {
                let site = Site {
                    file,
                    span: Span { start: 0, end: 0 },
                    name: self.program.sources.get(file).display.as_str().into(),
                };
                let declaration = self.file(file).declaration;
                let module = self.new_module(Some(site), false, declaration);
                // `bindSourceFileAsExternalModule` names the actual source
                // module, separately from its project-relative navigation site.
                let file_name =
                    project::program::native_file_name(&self.program.sources.get(file).path)?;
                let name = format!(
                    "\"{}\"",
                    project::program::remove_file_extension(&file_name)
                );
                self.state.store.set_native_name(
                    self.state.store.module(module).symbol,
                    super::ordering::NativeSymbolName::Bytes(name.into_bytes().into()),
                );
                self.state.store.add_source_file_declaration(
                    self.state.store.module(module).symbol,
                    file,
                    Span {
                        start: 0,
                        end: self.program.sources.get(file).text.as_str().len(),
                    },
                );
                if let Some(json) = &self.program.files[file.index()].facts.json {
                    self.state.store.set_json(module, file);
                    self.state.store.bind_json_symbols(
                        module,
                        file,
                        Span {
                            start: 0,
                            end: self.program.sources.get(file).text.as_str().len(),
                        },
                        json,
                    );
                }
                self.state.file_modules[file.index()] = Some(module);
            }
            let mut own: HashMap<&'p str, AmbientHost> = HashMap::new();
            let mut hosts = Vec::new();
            for ambient in &self.facts(file).ambient_modules {
                let site = Site {
                    file,
                    span: ambient.span,
                    name: ambient.name.as_str().into(),
                };
                let host = match own.get(ambient.name.as_str()) {
                    Some(&AmbientHost::Module(module)) => {
                        self.state.store.redeclare_module(module, site);
                        AmbientHost::Module(module)
                    }
                    Some(&AmbientHost::Fragment(index)) => {
                        let symbol = self.state.fragments[index].symbol;
                        self.state.store.declare_into(
                            symbol,
                            flags::MODULE | flags::VALUE_MODULE,
                            site,
                        );
                        AmbientHost::Fragment(index)
                    }
                    None => match self.state.ambient.get(&ambient.name) {
                        // The first declaration decides whether the module is
                        // shorthand.
                        None => {
                            let module = self.new_module(Some(site), ambient.shorthand, true);
                            self.state.ambient.insert(ambient.name.clone(), module);
                            if super::super::pattern::parse(&ambient.name).is_some() {
                                self.state.patterns.push((ambient.name.clone(), module));
                            }
                            AmbientHost::Module(module)
                        }
                        Some(&module) => {
                            let flags = flags::MODULE | flags::VALUE_MODULE;
                            let symbol = self.state.store.new_symbol(flags, Some(site));
                            self.state.store.export_table(symbol);
                            self.state.fragments.push(Fragment {
                                module,
                                symbol,
                                stars: Vec::new(),
                            });
                            AmbientHost::Fragment(self.state.fragments.len() - 1)
                        }
                    },
                };
                own.insert(&ambient.name, host);
                hosts.push(host);
            }
            self.state.ambient_hosts.push(hosts);
        }
        Ok(())
    }

    /// `resolveExternalModuleNameWorker` selects the raw module/script result
    /// before this tool's diagnostic admission. A tolerated script augmentation
    /// is ignored by lexical linking but remains Script in checker inputs.
    fn resolve_requests(&mut self) -> Result<()> {
        for file in self.file_ids() {
            let program_file = &self.program.files[file.index()];
            let mut modules = Vec::with_capacity(program_file.facts.requests.len());
            for (index, request) in program_file.facts.requests.iter().enumerate() {
                let resolution = &program_file.modules[index];
                let relative = project::is_relative_name(&request.specifier);
                let ambient = (!relative)
                    .then(|| self.state.ambient.get(&request.specifier).copied())
                    .flatten();
                let span = request.span;
                let augmentation = matches!(request.kind, RequestKind::Augmentation { .. });
                let target = match (ambient, resolution) {
                    (Some(module), _) => RequestTarget::Module(module),
                    (None, ModuleResolution::File(target)) => {
                        match self.state.file_modules[target.index()] {
                            Some(module) => RequestTarget::Module(module),
                            None => RequestTarget::Script,
                        }
                    }
                    (
                        None,
                        ModuleResolution::Missing
                        | ModuleResolution::NotAdmitted(_)
                        | ModuleResolution::Unchecked,
                    ) => {
                        // Pattern ambient modules also match relative names.
                        match self.pattern(&request.specifier) {
                            Some(module) => {
                                self.state
                                    .pattern_requests
                                    .push((file, RequestId::new(index)));
                                RequestTarget::Module(module)
                            }
                            None if matches!(resolution, ModuleResolution::Unchecked) => {
                                RequestTarget::Ignored
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
                                return Err(self
                                    .error(InputErrorKind::UnresolvedImport, file, span, message)
                                    .into());
                            }
                        }
                    }
                    (None, ModuleResolution::Unsupported(_))
                        if program_file.diagnostics.tolerates_unsupported_input() =>
                    {
                        RequestTarget::Ignored
                    }
                    (None, ModuleResolution::Unsupported(reason)) => {
                        return Err(self
                            .error(
                                InputErrorKind::Unsupported,
                                file,
                                span,
                                format!("module \"{}\": {reason}", request.specifier),
                            )
                            .into());
                    }
                };
                let raw_target = match target {
                    // Final binder associations are installed by declare_all.
                    // prepare_checker_inputs captures that header afterward.
                    RequestTarget::Module(_) => None,
                    RequestTarget::Script => Some(super::raw::RawModuleRequest::Script),
                    RequestTarget::Ignored => Some(super::raw::RawModuleRequest::Ignored),
                };
                if let Some(raw_target) = raw_target {
                    self.state.store.native_headers_mut().set_request(
                        file,
                        RequestId::new(index),
                        raw_target,
                    );
                }
                let target = if augmentation && matches!(target, RequestTarget::Script) {
                    self.augmentation_target_failed(file, index, true)?
                } else {
                    target
                };
                modules.push(target);
            }
            self.state.store.push_requests(modules);
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
        Err(self
            .error(
                InputErrorKind::UnresolvedImport,
                file,
                request.span,
                message,
            )
            .into())
    }

    /// The best-matching pattern ambient module, as TypeScript's
    /// `resolveExternalModule` chooses it.
    fn pattern(&self, specifier: &str) -> Option<ModuleId> {
        let patterns = self
            .state
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
        self.inputs().host_symbol(file, host)
    }

    /// Record each `export *` where its host's merges take it: on a file's
    /// or first-declared ambient module, which no merge combines, at once;
    /// on a later file's ambient module fragment or a module augmentation,
    /// when it merges (`merge_globals`, `merge_module_augmentation`). The
    /// module a star reaches is read through its request when used.
    fn collect_stars(&mut self) -> Result<()> {
        for file in self.file_ids() {
            for star in &self.facts(file).stars {
                let request = StarRequest {
                    file,
                    request: star.request,
                    type_only: star.type_only,
                };
                if let Some(owner) = self.host_symbol(file, star.host) {
                    let owner = self.state.store.header_for_symbol(owner);
                    self.state
                        .store
                        .native_headers_mut()
                        .add_export_star(owner, file, star.request, star.declaration_span)
                        .map_err(|failure| self.native_merge_failure(failure, Some(owner)))?;
                }
                match star.host {
                    Host::File => {
                        if let Some(module) = self.state.file_modules[file.index()] {
                            self.state.store.add_stars(module, &[request]);
                        }
                    }
                    Host::AmbientModule(index) => {
                        match self.state.ambient_hosts[file.index()][index.index()] {
                            AmbientHost::Module(module) => {
                                self.state.store.add_stars(module, &[request])
                            }
                            AmbientHost::Fragment(index) => {
                                self.state.fragments[index].stars.push(request)
                            }
                        }
                    }
                    Host::Augmentation(augmented) => {
                        let symbol = self.augmentation(file, augmented);
                        self.state
                            .augmentation_stars
                            .entry(symbol)
                            .or_default()
                            .push(request);
                    }
                    Host::Namespace(_) | Host::Global => {}
                }
            }
        }
        Ok(())
    }

    /// The table a scope declares into, created on first use.
    fn scope_table(&mut self, file: FileId, scope: ScopeId) -> TableId {
        match self.storage(file, scope) {
            Storage::Members(decl) => {
                let symbol = self.decl_symbol(file, decl);
                self.state.store.member_table(symbol)
            }
            Storage::Script | Storage::Own(_) => {
                if let Some(table) = self.state.scope_tables[file.index()][scope.index()] {
                    return table;
                }
                let table = self.state.store.new_table();
                if let Some((kind, span)) = self.facts(file).scopes[scope.index()].native_owner {
                    self.state.store.bind_locals_owner(table, file, kind, span);
                }
                self.state.scope_tables[file.index()][scope.index()] = Some(table);
                table
            }
        }
    }

    fn decl_symbol(&self, file: FileId, decl: DeclId) -> SymbolId {
        self.find(self.state.decl_symbols[file.index()][decl.index()].expect("declared before use"))
    }

    fn declared_symbol(&self, file: FileId, decl: DeclId) -> DeclaredSymbol {
        DeclaredSymbol {
            projection: self.decl_symbol(file, decl),
            header: self
                .state
                .store
                .declaration_header(file, decl)
                .expect("native declaration precedes its children"),
        }
    }

    fn scope_binding_table(&mut self, file: FileId, scope: ScopeId) -> BindingTable {
        if let Storage::Members(decl) = self.storage(file, scope) {
            return self
                .state
                .store
                .binding_symbol_table(self.declared_symbol(file, decl), false);
        }
        let table = self.scope_table(file, scope);
        self.state.store.binding_table(table)
    }

    fn host_binding_table(&mut self, file: FileId, host: Host) -> BindingTable {
        self.host_table(file, host);
        self.state.store.binding_symbol_table(
            self.host_declared_symbol(file, host)
                .expect("binder export host exists"),
            true,
        )
    }

    fn host_declared_symbol(&self, file: FileId, host: Host) -> Option<DeclaredSymbol> {
        match host {
            Host::Namespace(decl) => Some(self.declared_symbol(file, decl)),
            _ => self
                .host_symbol(file, host)
                .map(|symbol| self.state.store.declaration_result(symbol)),
        }
    }

    /// The export table of a host, created on first use. A module
    /// augmentation's is its own symbol's until it merges, then that of what
    /// it merged into, as TypeScript reads `getMergedSymbol`.
    fn host_table(&mut self, file: FileId, host: Host) -> TableId {
        match host {
            Host::Namespace(decl) => {
                let symbol = self.decl_symbol(file, decl);
                self.state.store.export_table(symbol)
            }
            Host::Augmentation(request) => {
                let symbol = self.augmentation(file, request);
                self.state.store.export_table(symbol)
            }
            Host::Global => {
                let symbol = match self.state.global_augmentations[file.index()] {
                    Some(symbol) => symbol,
                    None => {
                        let symbol = self.state.store.new_symbol(flags::NAMESPACE_MODULE, None);
                        self.state.global_augmentations[file.index()] = Some(symbol);
                        symbol
                    }
                };
                self.state.store.export_table(symbol)
            }
            Host::File | Host::AmbientModule(_) => {
                let symbol = self
                    .host_symbol(file, host)
                    .expect("exporting files and ambient modules have symbols");
                self.state.store.export_table(symbol)
            }
        }
    }

    /// Bind every declaration and alias, module augmentations into their own
    /// symbols and a script's top level into its own table. UMD globals join
    /// the global table as it is built (`merge_globals`).
    fn declare_all(&mut self) -> Result<()> {
        for file in self.file_ids() {
            let facts = self.facts(file);
            for &entry in &facts.binding_order {
                match entry {
                    BindingEntry::Declaration(index) => self.declare(file, index)?,
                    BindingEntry::Alias(index) => self.bind_alias(file, index)?,
                    BindingEntry::Module(index) => self.bind_module_declaration(file, index)?,
                    BindingEntry::CommonJsTypeExports(module) => {
                        let span = match module {
                            Some(module) => facts.module_declarations[module.index()].span,
                            None => Span {
                                start: 0,
                                end: self.program.sources.get(file).text.as_str().len(),
                            },
                        };
                        let declaration = match module {
                            Some(_) => super::store::SymbolDeclaration::Module { file, span },
                            None => super::store::SymbolDeclaration::SourceFile { file, span },
                        };
                        let owner = self
                            .state
                            .store
                            .native_headers()
                            .declaration_symbol(declaration)
                            .ok_or_else(|| {
                                self.lookup_failure(
                                    super::semantic::TypeBindingFailure::NotRetained,
                                    Some((file, span)),
                                )
                            })?;
                        self.state
                            .store
                            .native_headers_mut()
                            .bind_common_js_type_exports(owner)
                            .map_err(|failure| self.lookup_failure(failure, Some((file, span))))?;
                    }
                }
            }
        }
        self.state.store.finish_binding_headers();
        Ok(())
    }

    fn bind_module_declaration(
        &mut self,
        file: FileId,
        index: super::ids::ModuleDeclarationId,
    ) -> Result<()> {
        let declaration = &self.facts(file).module_declarations[index.index()];
        let table = self.scope_binding_table(file, declaration.scope);
        let preallocated = declaration.host.map(|host| {
            self.host_table(file, host);
            self.host_symbol(file, host)
                .expect("module host was allocated")
        });
        let name = super::ordering::NativeSymbolName::module_declaration(
            &declaration.name,
            declaration.global,
        );
        let flags = if declaration.value {
            flags::VALUE_MODULE
        } else {
            flags::NAMESPACE_MODULE
        };
        self.state
            .store
            .bind_module_declaration(table, preallocated, name, flags, file, declaration.span)
            .map_err(|failure| {
                self.error(
                    InputErrorKind::Unsupported,
                    file,
                    declaration.span,
                    format!("native module declaration: {failure:?}"),
                )
            })?;
        Ok(())
    }

    fn declare(&mut self, file: FileId, index: DeclId) -> Result<()> {
        let decl = &self.facts(file).decls[index.index()];
        // `bindFunctionOrConstructorType` declares this intermediate
        // Signature during its final TypeLiteral owner operation below.
        if matches!(
            decl.native_kind,
            SyntaxKind::FunctionType | SyntaxKind::ConstructorType
        ) && matches!(
            decl.kind,
            DeclKind::Member(MemberKind::Call | MemberKind::Construct)
        ) {
            assert!(self.state.decl_symbols[file.index()][index.index()].is_some());
            return Ok(());
        }
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
        if decl.member.as_ref().is_some_and(|member| member.optional) {
            flags |= flags::OPTIONAL;
        }
        let declaring = Declaring {
            flags,
            list: decl.list.map(|list| (file, list)),
            site,
        };
        // `declareModuleMember` creates the local before the export, even
        // when the navigation projection does not bind a namespace local.
        let local = if let Some(scope) = decl.native_export_local {
            Some(self.declare_export_local(file, index, scope, flags, declaring.site.clone())?)
        } else {
            None
        };
        // `bindAnonymousDeclaration` associates the node without declaring
        // a lexical/export name. Its children use this same Store symbol.
        let declared_symbol =
            if matches!(decl.kind, DeclKind::TypeLiteral | DeclKind::ObjectLiteral)
                || matches!(
                    decl.kind,
                    DeclKind::Function | DeclKind::Parameter | DeclKind::Class
                ) && decl.lexical.is_none()
                    && decl.export.is_none()
            {
                // `bindFunctionOrConstructorType` allocates the Signature first,
                // then the final node.Symbol. Both own the same AST declaration;
                // a table attachment does not invent a native Signature.Parent.
                let signature = self
                    .facts(file)
                    .anonymous_signatures
                    .get(&index)
                    .copied()
                    .map(|signature| {
                        let declaration = &self.facts(file).decls[signature.index()];
                        let symbol = self.state.store.new_symbol(
                            flags::SIGNATURE,
                            Some(Site {
                                file,
                                span: declaration.anchor,
                                name: declaration.name.clone(),
                            }),
                        );
                        self.state.store.add_declaration(
                            symbol,
                            file,
                            signature,
                            declaration.native_kind,
                            false,
                        );
                        self.state.store.set_native_name(
                            symbol,
                            super::ordering::NativeSymbolName::declaration(
                                declaration,
                                Default::default(),
                            ),
                        );
                        self.state.decl_symbols[file.index()][signature.index()] = Some(symbol);
                        (
                            symbol,
                            declaration
                                .member
                                .as_ref()
                                .expect("reserved signature has its native table role")
                                .key
                                .clone(),
                        )
                    });
                let symbol = self.state.store.new_symbol(flags, Some(declaring.site));
                if let Some((signature, key)) = signature {
                    let table = self
                        .state
                        .store
                        .binding_symbol_table(self.state.store.declaration_result(symbol), false);
                    self.state
                        .store
                        .hold_member(table.projection, &key, signature);
                    self.state
                        .store
                        .bind_table_symbol(table, self.state.store.header_for_symbol(signature));
                }
                self.state.store.declaration_result(symbol)
            } else if let Some(member) = &decl.member {
                let owner = self.declared_symbol(file, member.owner);
                if member.key == MemberKey::Dynamic {
                    let symbol = self.state.store.new_symbol(flags, Some(declaring.site));
                    self.state.store.add_late_member(owner, symbol);
                    self.state.store.declaration_result(symbol)
                } else if matches!(member.key, MemberKey::Private(_))
                    && member.private_class.is_none()
                {
                    // `declareSymbolEx` never inserts InternalSymbolNameMissing
                    // into a member table. Malformed private declarations outside
                    // a class still have their distinct node.Symbol attachment.
                    let symbol = self.state.store.new_symbol(flags, Some(declaring.site));
                    self.state.store.declaration_result(symbol)
                } else {
                    let table = self
                        .state
                        .store
                        .binding_symbol_table(owner, member.static_member);
                    let context = super::ordering::NativeDeclarationNameContext {
                        parent: Some(super::ordering::NativeSymbolIdentity::Bound(owner.header)),
                        private_class: member.private_class.map(|class| {
                            super::ordering::NativeSymbolIdentity::Bound(
                                self.declared_symbol(file, class).header,
                            )
                        }),
                    };
                    let name = super::ordering::NativeSymbolName::declaration(decl, context);

                    self.declare_member(table, &member.key, name, declaring)?
                }
            } else if let Some(scope) = decl
                .lexical
                .filter(|&scope| matches!(self.storage(file, scope), Storage::Members(_)))
            {
                let table = self.scope_binding_table(file, scope);
                let name = super::ordering::NativeSymbolName::declaration(decl, Default::default());

                self.declare_member(table, &MemberKey::Text(decl.name.clone()), name, declaring)?
            } else if local.is_some() {
                let (host, name) = decl
                    .export
                    .as_deref()
                    .expect("paired local has an export placement");
                let table = self.host_binding_table(file, *host);
                self.declare_in(table, None, name, declaring)?
            } else {
                match (decl.lexical, decl.export.as_deref()) {
                    (Some(scope), _) => {
                        let table = self.scope_binding_table(file, scope);
                        let signature = self.signature_table(file, scope);
                        self.declare_in(table, signature, &decl.name, declaring)?
                    }
                    (None, Some((host, name))) => {
                        let table = self.host_binding_table(file, *host);
                        self.declare_in(table, None, name, declaring)?
                    }
                    (None, None) => {
                        unreachable!("collect binds a declaration lexically or as an export")
                    }
                }
            };
        let symbol = declared_symbol.projection;
        self.state
            .store
            .add_declaration_flags(declared_symbol, flags);
        self.state.decl_symbols[file.index()][index.index()] = Some(symbol);
        if let Some(local) = local {
            self.state
                .store
                .set_declaration_export(local, declared_symbol);
            self.state.decl_local_symbols[file.index()][index.index()] = Some(local.projection);
            // Navigation retains its established target/group projection;
            // raw table readers see the independent local header at this key.
            if let Some(scope) = decl.lexical {
                let table = self.scope_table(file, scope);
                self.occupy(table, &decl.name, symbol);
            }
        }
        if let Some(member) = &decl.member {
            self.state
                .store
                .set_declaration_parent(declared_symbol, self.declared_symbol(file, member.owner));
        } else if let Some(Storage::Members(owner)) =
            decl.lexical.map(|scope| self.storage(file, scope))
        {
            self.state
                .store
                .set_declaration_parent(declared_symbol, self.declared_symbol(file, owner));
        } else if let Some((host, _)) = decl.export.as_deref() {
            if let Some(parent) = self.host_declared_symbol(file, *host) {
                self.state
                    .store
                    .set_declaration_parent(declared_symbol, parent);
            }
        }
        let name_context = super::ordering::NativeDeclarationNameContext {
            parent: self
                .state
                .store
                .native_header_parent(declared_symbol.header)
                .map(super::ordering::NativeSymbolIdentity::Bound),
            private_class: decl
                .member
                .as_ref()
                .and_then(|member| member.private_class)
                .map(|class| {
                    super::ordering::NativeSymbolIdentity::Bound(
                        self.declared_symbol(file, class).header,
                    )
                }),
        };
        self.state.store.set_declaration_name(
            declared_symbol,
            super::ordering::NativeSymbolName::declaration(decl, name_context),
        );
        self.state.store.add_declaration_to_header(
            declared_symbol,
            file,
            index,
            decl.native_kind,
            flags & flags::VALUE != 0,
        );
        if let DeclKind::Namespace { instance } = decl.kind {
            self.state
                .store
                .set_module_instance(declared_symbol, instance);
        }
        if let Some(members) = &decl.interface_members {
            self.state.store.add_interface_members(symbol, members);
        }
        if local.is_none()
            && let (Some(_), Some((host, name))) = (decl.lexical, decl.export.as_deref())
        {
            let table = self.host_table(file, *host);
            self.export_symbol(table, name, symbol, decl.kind.meanings(), file, decl.anchor)?;
        }
        if decl.kind == DeclKind::Class {
            self.bind_class_prototype(declared_symbol)?;
        }
        Ok(())
    }

    /// `declareModuleMember`'s local `declareSymbol` operation. Its Includes
    /// are ExportValue/None while Excludes remain the original declaration's.
    /// The table's raw header is independent from navigation's meaning slots.
    fn declare_export_local(
        &mut self,
        file: FileId,
        declaration: DeclId,
        scope: ScopeId,
        declaration_flags: u32,
        site: Site,
    ) -> Result<DeclaredSymbol> {
        let binding_table = self.scope_binding_table(file, scope);
        let table = binding_table.projection;
        let name = site.name.clone();
        let includes = if declaration_flags & flags::VALUE != 0 {
            flags::EXPORT_VALUE
        } else {
            0
        };
        let decl = &self.facts(file).decls[declaration.index()];
        let native_name = super::ordering::NativeSymbolName::declaration(decl, Default::default());
        let header = self
            .state
            .store
            .declare_native_symbol(
                binding_table,
                native_name,
                includes,
                native_declaration_excludes(declaration_flags),
            )
            .map_err(|failure| {
                self.error(
                    InputErrorKind::Unsupported,
                    file,
                    site.span,
                    format!("native export-local table: {failure:?}"),
                )
            })?;
        let entry = self.state.store.entry(table, &name);
        let has_alias = entry.is_some_and(|entry| entry.alias.is_some());
        let previous =
            entry.and_then(|entry| entry.member.or_else(|| entry.slots.symbols().next()));
        let symbol = if let Some(previous) = previous {
            let previous = self.find(previous);
            if self.symbol(previous).flags & excluded(declaration_flags) != 0 {
                if let Some((file, span)) = self.reported(previous, (site.file, site.span)) {
                    return Err(self
                        .error(
                            InputErrorKind::Binding,
                            file,
                            span,
                            format!("duplicate declaration of `{name}`"),
                        )
                        .into());
                }
                // Native duplicate recovery gives the declaration a fresh
                // header without replacing the table's original entry.
                self.state
                    .store
                    .new_symbol_projection(includes, Some(site), header)
            } else {
                self.state.store.declare_into(previous, includes, site);
                previous
            }
        } else {
            let symbol = self
                .state
                .store
                .new_symbol_projection(includes, Some(site), header);
            self.state
                .store
                .hold_member(table, &MemberKey::Text(name.clone()), symbol);
            symbol
        };
        if previous.is_some_and(|previous| self.find(previous) == symbol) {
            self.state
                .store
                .hold_member(table, &MemberKey::Text(name), symbol);
        }
        let decl = &self.facts(file).decls[declaration.index()];
        let declared = DeclaredSymbol {
            projection: symbol,
            header,
        };
        self.state.store.attach_projection(header, symbol);
        self.state.store.set_declaration_name(
            declared,
            super::ordering::NativeSymbolName::declaration(decl, Default::default()),
        );
        self.state.store.add_declaration_to_header(
            declared,
            file,
            declaration,
            decl.native_kind,
            false,
        );
        if has_alias {
            self.state.conflicts.push((
                table,
                self.facts(file).decls[declaration.index()].name.to_string(),
            ));
        }
        Ok(declared)
    }

    /// `bindClassLikeDeclaration` installs a fresh prototype after declaring
    /// the class symbol and before binding its children. Any existing export,
    /// including a type-only declaration or alias, receives the duplicate
    /// diagnostic before the entire table entry is replaced.
    fn bind_class_prototype(&mut self, parent: DeclaredSymbol) -> Result<()> {
        let exports = self
            .state
            .store
            .binding_symbol_table(parent, true)
            .projection;
        let held = self.held(exports, "prototype");
        let previous = if let Some(symbol) = distinct(held.slots).first() {
            self.symbol(*symbol)
                .sites
                .first()
                .map(|site| (site.file, site.span))
        } else {
            held.alias.map(|alias| self.alias_site(alias))
        };
        if let Some((file, span)) = previous.filter(|&(file, span)| self.reports(file, span.start))
        {
            return Err(self
                .error(
                    InputErrorKind::Binding,
                    file,
                    span,
                    "export `prototype` conflicts with the class prototype".into(),
                )
                .into());
        }
        self.state
            .store
            .class_prototype(parent, flags::PROPERTY | flags::PROTOTYPE);
        Ok(())
    }

    /// Native member `declareSymbol`: compatible roles at one key are one
    /// symbol even when their lexical meanings differ. A suppressed excluded
    /// declaration gets a separate symbol; the original table entry survives.
    fn declare_member(
        &mut self,
        table: BindingTable,
        key: &MemberKey,
        name: super::ordering::NativeSymbolName,
        declaring: Declaring,
    ) -> Result<DeclaredSymbol> {
        let Declaring {
            flags: declared,
            list,
            site,
        } = declaring;
        let header = self
            .state
            .store
            .declare_native_symbol(table, name, declared, native_declaration_excludes(declared))
            .map_err(|failure| {
                self.error(
                    InputErrorKind::Unsupported,
                    site.file,
                    site.span,
                    format!("native member table: {failure:?}"),
                )
            })?;
        let table = table.projection;
        let held = self.state.store.member(table, key);
        let symbol = if let Some(symbol) = held {
            let data = self.symbol(symbol);
            let repeated = list.is_some_and(|list| data.lists.as_slice().contains(&list));
            if conflicts(data.flags, declared, repeated) {
                if let Some((file, span)) = self.reported(symbol, (site.file, site.span)) {
                    return Err(self
                        .error(
                            InputErrorKind::Binding,
                            file,
                            span,
                            format!("duplicate member declaration of `{}`", site.name),
                        )
                        .into());
                }
                if data.flags & (flags::GETTER | flags::SETTER) != 0 {
                    self.state
                        .store
                        .add_projection_flags(symbol, flags::GETTER | flags::SETTER);
                }
                self.state
                    .store
                    .new_symbol_projection(declared, Some(site), header)
            } else {
                self.state.store.declare_into(symbol, declared, site);
                symbol
            }
        } else {
            let symbol = self
                .state
                .store
                .new_symbol_projection(declared, Some(site), header);
            self.state.store.hold_member(table, key, symbol);
            symbol
        };
        if let Some(list) = list {
            self.state.store.add_list(symbol, list);
        }
        if held.is_none_or(|old| self.find(old) == self.find(symbol)) {
            if let MemberKey::Text(name) = key {
                self.occupy(table, name, symbol);
            }
        }
        self.state.store.attach_projection(header, symbol);
        Ok(DeclaredSymbol {
            projection: symbol,
            header,
        })
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
                return Err(self
                    .error(
                        InputErrorKind::Binding,
                        file,
                        name.span,
                        format!(
                            "declarations of one name must be all exported or all local: `{}`",
                            facts.text(name.text)
                        ),
                    )
                    .into());
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
                return Err(self
                    .error(InputErrorKind::Binding, file, span, message.into())
                    .into());
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
                    name: request.specifier.as_str().into(),
                };
                let symbol = match by_name.get(request.specifier.as_str()) {
                    Some(&symbol) => {
                        self.state.store.declare_into(symbol, flags, site);
                        symbol
                    }
                    None => {
                        let symbol = self.state.store.new_symbol(flags, Some(site));
                        self.state.store.export_table(symbol);
                        by_name.insert(&request.specifier, symbol);
                        self.state.augmentation_order.push((file, id, symbol));
                        symbol
                    }
                };
                symbols[index] = Some(symbol);
            }
            self.state.augmentations.push(symbols);
        }
    }

    /// The symbol of a module augmentation request.
    fn augmentation(&self, file: FileId, request: RequestId) -> SymbolId {
        self.state.augmentations[file.index()][request.index()]
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
    fn prepare_checker_inputs(&mut self) -> Result<super::raw::RawBindingSnapshot> {
        self.collect_stars()?;
        // Modules use their final binder header before exact_module redirects
        // navigation requests. Script/Ignored were captured before lexical
        // augmentation tolerance and must not be recaptured from that view.
        for file in self.file_ids() {
            for index in 0..self.file(file).requests.len() {
                let request = RequestId::new(index);
                if let RequestTarget::Module(module) = self.state.store.request(file, request) {
                    let target = super::raw::RawModuleRequest::Module {
                        header: self
                            .state
                            .store
                            .header_for_symbol(self.state.store.module(module).symbol),
                        pattern: self.state.pattern_requests.contains(&(file, request)),
                    };
                    self.state
                        .store
                        .native_headers_mut()
                        .set_request(file, request, target);
                }
            }
        }
        Ok(self.capture_raw_bindings())
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
    fn observe_initialization(
        &mut self,
        step: super::super::semantic::InitializationStep,
    ) -> Result<()> {
        use super::super::semantic::InitializationStep as Step;
        match step {
            Step::GlobalThis => self.state.store.initialize_global_this(),
            Step::Script(file) => {
                if let Some(from) = self.state.scope_tables[file.index()]
                    .first()
                    .copied()
                    .flatten()
                {
                    self.run_projected_merges(vec![TableMerge {
                        into: GLOBAL,
                        from,
                        direction: Direction::Both,
                        stars: None,
                        parent: None,
                    }])?;
                }
            }
            Step::Umd(file) => {
                for (name, alias) in std::mem::take(&mut self.state.global_exports[file.index()]) {
                    let taken = self
                        .state
                        .store
                        .entry(GLOBAL, &name)
                        .is_some_and(|entry| !entry.slots.is_empty() || entry.alias.is_some());
                    if !taken {
                        self.state.store.set_alias(GLOBAL, &name, Some(alias));
                    }
                }
            }
            Step::GlobalAugmentation(file) => {
                if let Some(symbol) = self.state.global_augmentations[file.index()] {
                    let from = self.state.store.export_table(symbol);
                    self.run_projected_merges(vec![TableMerge {
                        into: GLOBAL,
                        from,
                        direction: Direction::Both,
                        stars: None,
                        parent: None,
                    }])?;
                }
            }
            Step::Undefined => self.state.store.add_undefined_to_globals(),
            Step::Ambient(source) => {
                if let Some(index) = self.state.fragments.iter().position(|fragment| {
                    self.state.store.raw_projection_header(fragment.symbol) == Some(source.0)
                }) {
                    let Fragment { module, symbol, .. } = self.state.fragments[index];
                    let stars = std::mem::take(&mut self.state.fragments[index].stars);
                    let target = self.state.store.module(module).symbol;
                    let target = Slots::only(self.meanings_of(target), target);
                    self.mark_merged_into(target);
                    let mut work = Vec::new();
                    self.merge_symbol(target, symbol, Direction::Both, None, &mut work)?;
                    self.run_projected_merges(work)?;
                    self.state.store.add_stars(module, &stars);
                }
            }
            Step::Module(site) => {
                let augmentation = self.augmentation(site.file, site.request);
                self.merge_module_augmentation(site.file, site.request, augmentation)?;
            }
            Step::Complete => {
                self.absorb_patterns()?;
                self.reset_resolutions();
                self.state.resolution.journal = None;
                self.state.store.freeze();
            }
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
        if let Some(stars) = self.state.augmentation_stars.remove(&augmentation) {
            self.state.store.add_stars(module, &stars);
        }
        let data = &self.file(file).requests[request.index()];
        // A JSON module's `export=` is its value, a property.
        let json = self.state.store.module(module).json.is_some();
        let target = if json || self.exports_have(module, "export=") {
            let resolution = if let Some(file) = self.state.store.module(module).json {
                Self::json_value(file)
            } else {
                self.settle(|linker| match linker.probe_export(module, "export=") {
                    Probe::Found(found) => Step::Done(found.resolution),
                    Probe::Need(need) => Step::Need(need),
                    Probe::Missing => Step::Done(Resolution::Unresolved("no export =".into())),
                })?
            };
            let target = match resolution {
                Resolution::Found(found) => found.targets().map(|symbol| self.find(symbol)),
                _ => Slots::default(),
            };
            if self.union_flags(target) & (flags::NAMESPACE | flags::ENUM | flags::MODULE) == 0 {
                if !self.reports(file, data.span.start) {
                    return Ok(());
                }
                return Err(self
                    .error(
                        InputErrorKind::Binding,
                        file,
                        data.span,
                        format!(
                            "cannot augment \"{}\" because it resolves to a non-module entity",
                            data.specifier
                        ),
                    )
                    .into());
            }
            target
        } else {
            let symbol = self.state.store.module(module).symbol;
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
            .and_then(|container| self.state.store.symbol(container).module);
        let mut work = Vec::new();
        self.merge_symbol(target, augmentation, Direction::Both, stars, &mut work)?;
        self.run_projected_merges(work)
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
        let matched = self.state.pattern_requests.contains(&(file, request))
            && self
                .state
                .patterns
                .iter()
                .any(|&(_, module)| module == pattern);
        if !matched
            || self.state.store.module(pattern).merged_into
            || self.exports_have(pattern, "export=")
        {
            return module;
        }
        let shorthand = self.state.store.module(pattern).shorthand
            && self.state.store.symbol(augmentation).flags & flags::VALUE_MODULE == 0;
        let exact = self.new_module(None, shorthand, true);
        self.state.store.set_pattern(exact, Some(pattern));
        self.state.pattern_augmentations.push((exact, pattern));
        let specifier = &self.file(file).requests[request.index()].specifier;
        for &(other, other_request) in &self.state.pattern_requests {
            if self.file(other).requests[other_request.index()].specifier == *specifier {
                self.state
                    .store
                    .set_request(other, other_request, RequestTarget::Module(exact));
            }
        }
        exact
    }

    /// Record that a checker merge merges into each module among `targets`
    /// (`Module::merged_into`).
    fn mark_merged_into(&mut self, targets: Slots) {
        for symbol in distinct(targets) {
            if let Some(module) = self.state.store.symbol(symbol).module {
                self.state.store.set_merged_into(module);
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
        for (module, pattern) in std::mem::take(&mut self.state.pattern_augmentations) {
            let stars = self.state.store.module(pattern).stars.clone();
            self.state.store.add_stars(module, &stars);
            let target = self.state.store.module(module).symbol;
            let source = self.state.store.module(pattern).symbol;
            let target = Slots::only(self.meanings_of(target), target);
            let mut work = Vec::new();
            self.merge_symbol(target, source, Direction::OneWay, None, &mut work)?;
            self.run_projected_merges(work)?;
            self.state.store.set_pattern(module, None);
        }
        Ok(())
    }

    /// Navigation's meaning-separated projection of completed native merge
    /// work. It cannot supply input identities to the native checker driver.
    /// Merge the tables a checker merge queued (`mergeSymbolTable`), each
    /// entry of `from` into `into`, depth first as TypeScript recurses: the
    /// tables an entry's merge queues merge before the next entry. An
    /// explicit stack keeps deep namespaces off the call stack. Merging both
    /// ways leaves `from` as it is: nothing reaches it through the merged
    /// identity, and conflicts recorded against it are still checked. A pair
    /// of tables merges once, so tables that reach each other, as a namespace
    /// exporting an alias of itself can, still finish.
    fn run_projected_merges(&mut self, work: Vec<TableMerge>) -> Result<()> {
        let mut merged = HashSet::new();
        let mut stack: Vec<(TableMerge, std::vec::IntoIter<(std::sync::Arc<str>, Held)>)> =
            Vec::new();
        let mut queued = work;
        loop {
            for merge in std::mem::take(&mut queued).into_iter().rev() {
                if merge.into == merge.from || !merged.insert((merge.into, merge.from)) {
                    continue;
                }
                self.merge_native_members(
                    merge.from,
                    merge.into,
                    merge.direction,
                    merge.parent,
                    &mut queued,
                )?;
                let mut entries: Vec<(std::sync::Arc<str>, Held)> = self
                    .state
                    .store
                    .table(merge.from)
                    .iter()
                    .filter(|(_, entry)| entry.member.is_none())
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
                Some((name, source)) => self.merge_entry(merge, &name, source, &mut queued)?,
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
        merge: TableMerge,
        name: &str,
        source: Held,
        work: &mut Vec<TableMerge>,
    ) -> Result<()> {
        let TableMerge {
            into,
            direction,
            stars,
            parent,
            ..
        } = merge;
        let held = self.held(into, name);
        let source_slots = source.slots.map(|symbol| self.find(symbol));
        let alias_clash = source.alias.is_some() && held.alias.is_some();
        let (targets, origin) = if !held.slots.is_empty() || alias_clash {
            (held.slots, Origin::Declared)
        } else if let Some(alias) = held.alias {
            match self.force(Task::Alias(alias))? {
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
            match self.settle(|linker| {
                linker
                    .star_export(module, name, false)
                    .map(|found| found.resolution)
            })? {
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
            // `mergeSymbol` keeps the initialized globalThis header on a
            // conflicting declaration. The separate 2397 diagnostic does
            // not invalidate native lookup or the installed global type.
            if targets
                .symbols()
                .any(|symbol| self.state.store.is_global_this(symbol))
            {
                return Ok(());
            }
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
                self.state.store.set_alias(into, name, source.alias);
                for meaning in each(meaning::ALL) {
                    if let Some(symbol) = source_slots.get(meaning) {
                        self.state.store.hold(into, name, meaning, symbol);
                    }
                }
            }
            return Ok(());
        }
        self.mark_merged_into(targets);
        // `mergeSymbol` combines one native name-table entry in actual
        // target/source order, even if the lexical contract separates its
        // compatible meanings. Source groups are already internally ordered.
        let reached = targets
            .symbols()
            .fold(0, |meanings, symbol| meanings | self.meanings_of(symbol));
        if !matches!(origin, Origin::Declared) {
            // TypeScript stores the merged symbol where the alias was.
            self.state.store.set_alias(into, name, None);
            for meaning in each(meaning::ALL) {
                if let Some(symbol) = targets.get(meaning) {
                    self.state.store.hold(into, name, meaning, symbol);
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
            self.state.store.set_alias(into, name, Some(alias));
            for symbol in distinct(self.held(into, name).slots) {
                self.state.store.add_alias(symbol, alias);
            }
            // The merged symbol is the alias's own, so TS2440 applies to it
            // as to a same-table alias; one way, the alias keeps its own.
            if direction == Direction::Both && !self.held(into, name).slots.is_empty() {
                self.state.conflicts.push((into, name.to_string()));
            }
        }
        let merged = self.held(into, name).slots;
        if let Some(parent) = parent.filter(|_| !held.slots.is_empty()) {
            for symbol in distinct(merged) {
                if self.symbol(symbol).checker_merged {
                    self.state.store.set_symbol_parent(symbol, parent);
                }
            }
        }
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
        let flags = self.state.store.symbol(source).flags;
        let with: SmallVec<[SymbolId; 3]> = distinct(targets.map(|symbol| self.find(symbol)))
            .into_iter()
            // The checker's initialized globalThis header is already mutable.
            // All compatible declarations merge into that actual header,
            // including roles the lexical projection otherwise keeps apart.
            .filter(|&target| {
                target != source
                    && (self.state.store.is_global_this(target)
                        || merges(self.state.store.symbol(target).flags, flags))
            })
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
        let (members, exports) = self.state.store.transfer(target, source, direction);
        self.state.store.mark_checker_merged(target);
        if let Some(from) = members {
            let into = self.state.store.member_table(target);
            work.push(TableMerge {
                into,
                from,
                direction,
                stars: None,
                parent: None,
            });
        }
        if let Some(from) = exports {
            let into = self.state.store.export_table(target);
            work.push(TableMerge {
                into,
                from,
                direction,
                stars,
                parent: Some(target),
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
            for table in self.state.store.symbol(symbol).homes.clone() {
                let entries: Vec<(std::sync::Arc<str>, Slots)> = self
                    .state
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
                            self.state.store.hold(table, &name, meaning, symbol);
                        }
                    }
                }
            }
            if let Some(module) = self.state.store.symbol(symbol).module {
                let own = self.meanings_of(symbol);
                for meaning in each(meaning::ALL).filter(|&meaning| own & meaning == 0) {
                    if let Some(other) = merged.get(meaning) {
                        self.state.store.set_merged(module, meaning, other);
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
            Some(&(file, span)) => Err(self
                .error(InputErrorKind::Binding, file, span, message)
                .into()),
            None => Ok(()),
        }
    }

    /// The declared symbols and alias of `table`'s entry for `name`, with
    /// symbols canonical.
    fn held(&self, table: TableId, name: &str) -> Held {
        self.state
            .store
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
                target != source && conflicts(self.state.store.symbol(target).flags, flags, false)
            })
    }

    /// The declaration sites of the symbols in `slots`.
    fn sites_of(&self, slots: Slots) -> Vec<(FileId, Span)> {
        distinct(slots.map(|symbol| self.find(symbol)))
            .into_iter()
            .flat_map(|symbol| self.state.store.symbol(symbol).sites.iter())
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
            let exports = self.state.store.module(star).exports;
            if let Some(entry) = self.state.store.entry(exports, name) {
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
        if let Some(&symbol) = self.state.value_symbols.get(&target) {
            return symbol;
        }
        let symbol = self.state.store.new_symbol(flags::PROPERTY, None);
        self.state.values.insert(symbol, target.clone());
        self.state.value_symbols.insert(target, symbol);
        symbol
    }

    /// The value a value symbol stands for (`value_symbol`).
    fn value_of(&self, symbol: SymbolId) -> Option<&ValueTarget> {
        self.state.values.get(&self.find(symbol))
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
    fn settle(
        &mut self,
        mut step: impl FnMut(&mut Self) -> Step<Resolution>,
    ) -> Result<Resolution> {
        if self.state.store.take_changed() {
            self.reset_resolutions();
        }
        loop {
            match step(self) {
                Step::Done(resolution) => return Ok(resolution),
                Step::Need(need) => {
                    self.force(need.task)?;
                }
                Step::Failed(failure) => return Err(self.lookup_failure(failure, None)),
            }
        }
    }

    /// Discard every resolution made while augmentations change tables, so
    /// that each is redone against the tables as they now are.
    fn reset_resolutions(&mut self) {
        self.state.store.take_changed();
        let tasks = self
            .state
            .resolution
            .journal
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        for task in tasks {
            match task {
                Task::Alias(a) => {
                    self.state.alias_states[a.file.index()][a.id.index()] = State::Unvisited;
                    self.state.alias_origins[a.file.index()][a.id.index()] = None;
                }
                Task::Ref(r) => {
                    self.state.ref_states[r.file.index()][r.id.index()] = State::Unvisited;
                    self.state.ref_origins[r.file.index()][r.id.index()] = None;
                    self.state.ref_type_alias_origins[r.file.index()][r.id.index()] = None;
                }
            }
        }
        self.state.resolution.export_memo.clear();
        self.state.resolution.cuts.clear();
        self.state.resolution.cut_tasks.clear();
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
                    let blocked = self.state.scope_tables[file.index()][scope.index()]
                        .and_then(|table| self.state.store.table(table).get(&*decl.name))
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
                        ).into());
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
                    && self.state.decl_symbols[file.index()][index].is_some()
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
                return Err(self
                    .error(
                        InputErrorKind::Binding,
                        file,
                        namespace.anchor,
                        format!("{message}: `{}`", namespace.name),
                    )
                    .into());
            }
            let generic: Vec<((FileId, DeclId), Vec<TypeParameter<'_>>)> = list
                .iter()
                .copied()
                .filter_map(|key| {
                    let parameters = decl(key).type_parameters.as_deref()?;
                    let parameters = parameters
                        .iter()
                        .map(|(name, default, _)| TypeParameter {
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
                    return Err(self
                        .error(
                            InputErrorKind::Binding,
                            file,
                            declaration.anchor,
                            format!(
                                "all declarations of `{}` must have identical type parameters",
                                declaration.name
                            ),
                        )
                        .into());
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
        self.state.store.type_parameter(owner, name)
    }

    /// Native function Locals combines its signature and body. Lookup keeps
    /// these partitions separate, but declarations share the native table's
    /// exclusion and identity rules in every meaning.
    fn signature_table(&self, file: FileId, scope: ScopeId) -> Option<TableId> {
        let scopes = &self.facts(file).scopes;
        let ScopeKind::Body = scopes[scope.index()].kind else {
            return None;
        };
        let parent = scopes[scope.index()].parent?;
        matches!(scopes[parent.index()].kind, ScopeKind::Signature { .. })
            .then(|| self.state.scope_tables[file.index()][parent.index()])
            .flatten()
    }

    /// Declare into `table`, merging with compatible same-named declarations.
    fn declare_in(
        &mut self,
        binding_table: BindingTable,
        parameters: Option<TableId>,
        name: &str,
        declaring: Declaring,
    ) -> Result<DeclaredSymbol> {
        let table = binding_table.projection;
        let entry = self.state.store.table(table).get(name);
        let has_alias = entry.is_some_and(|entry| entry.alias.is_some());
        let mut slots = entry.map(|entry| entry.slots).unwrap_or_default();
        if let Some(parameters) =
            parameters.and_then(|parameters| self.state.store.table(parameters).get(name))
        {
            for meaning in each(meaning::ALL) {
                if slots.get(meaning).is_none()
                    && let Some(parameter) = parameters.slots.get(meaning)
                {
                    slots.set(meaning, parameter);
                }
            }
        }
        let header = self
            .state
            .store
            .declare_native_symbol(
                binding_table,
                super::ordering::NativeSymbolName::Bytes(name.as_bytes().into()),
                declaring.flags,
                native_declaration_excludes(declaring.flags),
            )
            .map_err(|failure| {
                self.error(
                    InputErrorKind::Unsupported,
                    declaring.site.file,
                    declaring.site.span,
                    format!("native declaration table: {failure:?}"),
                )
            })?;
        let brought = meanings(declaring.flags);
        let symbol = self.combine(slots, name, declaring, header)?;
        self.state.store.attach_projection(header, symbol);
        self.occupy(table, name, symbol);
        self.debug_assert_placed(table, name, brought, symbol);
        if has_alias {
            self.state.conflicts.push((table, name.to_string()));
        }
        Ok(DeclaredSymbol {
            projection: symbol,
            header,
        })
    }

    /// The symbol a declaration of `name` forms with the same-named symbols
    /// it meets in one table, `existing`: one it merges with, or a new one,
    /// as TypeScript's binder decides (`declareSymbol`) by the one exclusion
    /// and merge test (`conflicts`, `merges`). Excluded pairs fail where
    /// bindings are checked (TS2300, TS2451, TS2567); declarations of other
    /// meanings stay separate symbols. Module augmentations merge by the
    /// checker's rule instead (`merge_entry`).
    fn combine(
        &mut self,
        existing: Slots,
        name: &str,
        declaring: Declaring,
        header: super::ids::NativeHeaderId,
    ) -> Result<SymbolId> {
        let Declaring {
            flags: declared,
            list,
            site,
        } = declaring;
        let mut merge_with: SmallVec<[SymbolId; 3]> = SmallVec::new();
        for symbol in distinct(existing.map(|symbol| self.find(symbol))) {
            let flags = self.state.store.symbol(symbol).flags;
            let repeated = list.is_some_and(|list| {
                self.state
                    .store
                    .symbol(symbol)
                    .lists
                    .as_slice()
                    .contains(&list)
            });
            if conflicts(flags, declared, repeated) {
                if let Some((file, span)) = self.reported(symbol, (site.file, site.span)) {
                    return Err(self
                        .error(
                            InputErrorKind::Binding,
                            file,
                            span,
                            format!("duplicate declaration of `{name}`"),
                        )
                        .into());
                }
                merge_with.push(symbol);
            } else if merges(flags, declared) {
                merge_with.push(symbol);
            }
        }
        let symbol = match merge_with.split_first() {
            None => self
                .state
                .store
                .new_symbol_projection(declared, Some(site), header),
            Some((&first, rest)) => {
                for &other in rest {
                    self.merge_symbols(first, other)?;
                }
                let first = self.find(first);
                self.state.store.declare_into(first, declared, site);
                first
            }
        };
        if let Some(list) = list {
            self.state.store.add_list(symbol, list);
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
                self.state.store.hold(table, name, meaning, symbol);
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
        self.state
            .store
            .unite(target, other)
            .map_err(|TwoModules(other)| {
                let site = &self.symbol(other).sites[0];
                self.error(
                    InputErrorKind::Unsupported,
                    site.file,
                    site.span,
                    "merging two modules is unsupported".into(),
                )
                .into()
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
            let (members, exports) = self.state.store.transfer(target, other, Direction::Both);
            if let Some(from) = exports {
                let into = self.state.store.export_table(target);
                self.merge_tables(from, into, &mut work)?;
            }
            if let Some(from) = members {
                let into = self.state.store.member_table(target);
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
        for (key, source) in self.state.store.member_entries(from) {
            match self.state.store.member(into, &key) {
                Some(target) => work.push((target, source)),
                None => {
                    self.state.store.hold_member(into, &key, source);
                    if let MemberKey::Text(name) = &key {
                        self.occupy(into, name, source);
                    }
                }
            }
        }
        let entries: Vec<(std::sync::Arc<str>, super::store::Entry)> = self.state.store.drain(from);
        for (name, entry) in entries {
            if entry.member.is_some() {
                continue;
            }
            for meaning in each(meaning::ALL) {
                let held = self
                    .state
                    .store
                    .entry(into, &name)
                    .and_then(|destination| destination.slots.get(meaning));
                match (held, entry.slots.get(meaning)) {
                    (Some(a), Some(b)) => work.push((a, b)),
                    (None, Some(b)) => self.state.store.hold(into, &name, meaning, b),
                    _ => {}
                }
            }
            let Some(alias) = entry.alias else {
                continue;
            };
            let Some(kept) = self.held(into, &name).alias else {
                self.state.store.set_alias(into, &name, Some(alias));
                continue;
            };
            // TypeScript reports both declarations; the first export stays
            // where neither is reported.
            if let Some(checked) = [alias, kept].into_iter().find(|alias| {
                let span = self.facts(alias.file).aliases[alias.id.index()].span;
                self.reports(alias.file, span.start)
            }) {
                let span = self.facts(checked.file).aliases[checked.id.index()].span;
                return Err(self
                    .error(
                        InputErrorKind::Binding,
                        checked.file,
                        span,
                        format!("merged declarations both export `{name}`"),
                    )
                    .into());
            }
        }
        Ok(())
    }

    /// `mergeSymbolTable` for the native member-key domain. Unlike lexical
    /// meaning separation, compatible Property/TypeParameter/signature roles
    /// at one key merge one way or both with the enclosing checker merge.
    fn merge_native_members(
        &mut self,
        from: TableId,
        into: TableId,
        direction: Direction,
        parent: Option<SymbolId>,
        work: &mut Vec<TableMerge>,
    ) -> Result<()> {
        for (key, source) in self.state.store.member_entries(from) {
            let source = self.find(source);
            let target = self.state.store.member(into, &key);
            let symbol = match target {
                Some(target) if target == source => target,
                Some(target) => {
                    if self.symbol(target).flags & excluded(self.symbol(source).flags) != 0 {
                        let source_sites: Vec<_> = self
                            .symbol(source)
                            .sites
                            .iter()
                            .map(|site| (site.file, site.span))
                            .collect();
                        let target_sites: Vec<_> = self
                            .symbol(target)
                            .sites
                            .iter()
                            .map(|site| (site.file, site.span))
                            .collect();
                        self.reject("member", &source_sites, &target_sites, false)?;
                        continue;
                    }
                    self.merge_into(target, source, direction, None, work)?;
                    self.find(target)
                }
                None => source,
            };
            if let Some(parent) = parent.filter(|_| target.is_some()) {
                if self.symbol(symbol).checker_merged {
                    self.state.store.set_symbol_parent(symbol, parent);
                }
            }
            self.state.store.hold_member(into, &key, symbol);
            if let MemberKey::Text(name) = &key {
                self.occupy(into, name, symbol);
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
                None => self.state.store.hold(table, name, meaning, symbol),
                Some(other) => merge.push(other),
            }
        }
        for other in merge {
            if other == symbol {
                continue;
            }
            if let Some((file, span)) = self.reported(other, (file, span)) {
                return Err(self
                    .error(
                        InputErrorKind::Binding,
                        file,
                        span,
                        format!("duplicate export `{name}`"),
                    )
                    .into());
            }
        }
        if held.alias.is_some() {
            self.state.conflicts.push((table, name.to_string()));
        }
        Ok(())
    }

    fn bind_alias(&mut self, file: FileId, index: AliasId) -> Result<()> {
        let alias = &self.facts(file).aliases[index.index()];
        let reference = AliasRef { file, id: index };
        let span = alias.span;
        // Native alias symbols are allocated once at their actual placement;
        // AliasBind::Both adds a navigation local, not another native header.
        let (native_table, native_parent, native_name) = match &alias.bind {
            AliasBind::Lexical { scope, name } => (
                self.scope_binding_table(file, *scope).native,
                None,
                name.as_str(),
            ),
            AliasBind::Export { host, name } | AliasBind::Both { host, name, .. } => {
                let table = self.host_binding_table(file, *host);
                (
                    table.native,
                    self.host_declared_symbol(file, *host),
                    name.as_str(),
                )
            }
            AliasBind::Global { name } => (
                self.state.store.native_global_exports(file),
                self.host_declared_symbol(file, Host::File),
                name.as_str(),
            ),
        };
        self.state
            .store
            .bind_native_alias(
                native_table,
                native_name,
                reference,
                native_parent,
                alias.native_kind,
            )
            .map_err(|failure| {
                self.error(
                    InputErrorKind::Unsupported,
                    file,
                    span,
                    format!("native alias table: {failure:?}"),
                )
            })?;
        let bind = |linker: &mut Self, table: TableId, name: &str| -> Result<()> {
            let held = linker.held(table, name);
            if let Some(kept) = held.alias {
                let kept_span = linker.facts(kept.file).aliases[kept.id.index()].span;
                // TypeScript's binder reports both declarations.
                if let Some((file, span)) = [(file, span), (kept.file, kept_span)]
                    .into_iter()
                    .find(|&(file, span)| linker.reports(file, span.start))
                {
                    return Err(linker
                        .error(
                            InputErrorKind::Binding,
                            file,
                            span,
                            format!("duplicate import or export `{name}`"),
                        )
                        .into());
                }
                return Ok(());
            }
            linker.state.store.set_alias(table, name, Some(reference));
            if !held.slots.is_empty() {
                linker.state.conflicts.push((table, name.to_string()));
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
                let exports = &mut self.state.global_exports[file.index()];
                if !exports.iter().any(|(other, _)| other == name) {
                    exports.push((name.clone(), reference));
                    return Ok(());
                }
                if self.reports(file, span.start) {
                    return Err(self
                        .error(
                            InputErrorKind::Binding,
                            file,
                            span,
                            format!("duplicate import or export `{name}`"),
                        )
                        .into());
                }
                Ok(())
            }
        }
    }

    /// Freeze one inventory for every captured file. Lexical resolution is
    /// demanded only for searched files; dependency names stay cold until a
    /// checker or import/export continuation actually asks for their symbols.
    fn resolve_and_index(&mut self) -> Result<(Index, Vec<Vec<super::ids::OccurrenceId>>)> {
        let mut index = index::Builder::default();
        let mut reference_sources = Vec::with_capacity(self.program.files.len());
        for file in self.file_ids() {
            let mut tokens = Vec::new();
            {
                let facts = self.facts(file);
                let texts = index.intern_all(&facts.texts);
                let names = &facts.names;
                tokens.reserve(names.len());
                for (position, name) in names.iter().enumerate() {
                    let target = if !self.search(file) {
                        Target::Unsearched
                    } else {
                        match name.target {
                            NameTarget::Decl(decl) => {
                                // The checker may resolve a parameter Name as an
                                // expression reference. Preserve that separate
                                // lexical answer without replacing its public
                                // declaring-symbol navigation association.
                                if let Some(reference) =
                                    self.facts(file).decls[decl.index()].name_reference
                                {
                                    self.force(Task::Ref(RefRef {
                                        file,
                                        id: reference,
                                    }))?;
                                }
                                Target::Symbols(SmallVec::from_elem(
                                    self.decl_symbol(file, decl),
                                    1,
                                ))
                            }
                            NameTarget::Ref(reference) => self.reference_target(
                                RefRef {
                                    file,
                                    id: reference,
                                },
                                &mut index,
                            )?,
                            NameTarget::Alias(alias) => {
                                let resolution =
                                    self.force(Task::Alias(AliasRef { file, id: alias }))?;
                                self.target(resolution, &mut index)
                            }
                            NameTarget::NotReference(why) => Target::NotReference(why),
                            NameTarget::Unresolved(why) => Target::Unresolved(index.intern(why)),
                        }
                    };
                    tokens.push(Token {
                        source: NameId::new(position),
                        span: name.span,
                        name: texts[name.text.index()],
                        roles: name.roles,
                        call: name.call,
                        candidate: name.candidate,
                        target,
                        lexical: name.lexical,
                        reference: name
                            .reference
                            .as_ref()
                            .map(|reference| index::TokenReference {
                                key: match &reference.key {
                                    Ok(key) => Ok(texts[key.index()]),
                                    Err(why) => Err(why.as_str()),
                                },
                                syntax: reference.syntax,
                            }),
                    });
                }
            }
            let occurrences = index.add_file(file, self.search(file), tokens);
            if let Some(json) = &self.program.files[file.index()].facts.json {
                index.add_json(file, json);
            }
            reference_sources.push(
                self.facts(file)
                    .refs
                    .iter()
                    .map(|reference| occurrences[reference.token.index()])
                    .collect(),
            );
        }
        let aliased: Vec<SymbolId> = self
            .state
            .store
            .canonical_symbols()
            .filter(|(_, symbol)| !symbol.aliases.is_empty())
            .map(|(id, _)| id)
            .collect();
        let projected: HashMap<SymbolId, Vec<Site>> = aliased
            .into_iter()
            .map(|symbol| self.projected_sites(symbol).map(|sites| (symbol, sites)))
            .collect::<Result<_>>()?;
        let index = index.finish(&self.program.sources, |symbol| {
            projected
                .get(&symbol)
                .cloned()
                .unwrap_or_else(|| self.symbol(symbol).sites.to_vec())
        });
        Ok((index, reference_sources))
    }

    /// The declarations a group lists for `symbol`: its own, and those of
    /// what its alias declarations name that it may merge with, as
    /// TypeScript's queries follow an alias among a symbol's declarations to
    /// its target. One way, as the target's group does not list `symbol`'s.
    fn projected_sites(&mut self, symbol: SymbolId) -> Result<Vec<Site>> {
        let symbol = self.find(symbol);
        let mut sites = self.symbol(symbol).sites.to_vec();
        let mut reached = vec![symbol];
        let mut stack = vec![symbol];
        while let Some(owner) = stack.pop() {
            let flags = self.symbol(owner).flags;
            for alias in self.symbol(owner).aliases.clone() {
                let Resolution::Found(found) = self.force(Task::Alias(alias))? else {
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
        Ok(sites)
    }

    /// Project `resolveEntityName` into the documented lexical query domain.
    /// A value property of an unresolved receiver is outside Q1 even when
    /// alias resolution retained a dependency failure for semantic consumers.
    /// Normalize only this index entry; the completed binding stays intact.
    fn reference_target(
        &mut self,
        reference: RefRef,
        index: &mut index::Builder,
    ) -> Result<Target> {
        let resolution = self.force(Task::Ref(reference))?;
        let facts = &self.facts(reference.file).refs[reference.id.index()];
        if facts.property && matches!(resolution, Resolution::Unresolved(_)) {
            if let super::collect::RefBase::Member(receiver) = facts.base {
                if matches!(
                    self.state.ref_states[reference.file.index()][receiver.index()],
                    State::Done(Resolution::Unresolved(_))
                ) {
                    return Ok(Target::NotReference("property"));
                }
            }
        }
        Ok(self.target(resolution, index))
    }

    fn target(&self, resolution: Resolution, index: &mut index::Builder) -> Target {
        match resolution {
            // A name that denotes a value, alone or beside declarations of
            // other meanings that a merge gave it, denotes no reference
            // target, as TypeScript's one symbol for them is a property.
            Resolution::Found(found) => {
                if found.targets().symbols().any(|symbol| {
                    self.symbol(symbol).intrinsic.is_some()
                        || self.state.store.is_global_this(symbol)
                }) {
                    return Target::NotReference(
                        if found
                            .targets()
                            .symbols()
                            .any(|symbol| self.state.store.is_global_this(symbol))
                        {
                            "built-in globalThis"
                        } else {
                            "built-in value"
                        },
                    );
                }
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
            Resolution::Value { target, .. } => Target::NotReference(value_reason(&target)),
            Resolution::NotReference(why) => Target::NotReference(why),
            Resolution::Unresolved(why) => Target::Unresolved(index.intern(&why)),
        }
    }

    /// `checkAliasSymbol`: an import alias may not share a meaning with a
    /// local declaration. The diagnostic and its suppression belong to the
    /// actual alias node, independently of which declaration completed the
    /// conflicting table entry first.
    fn check_conflicts(&mut self) -> Result<()> {
        let mut conflicts = std::mem::take(&mut self.state.conflicts);
        // `checkImportDeclaration`/`checkExportDeclaration` visit their alias
        // nodes in source order. A later local completing a table entry must
        // not reorder those checks.
        conflicts.sort_by_key(|(table, name)| {
            self.state
                .store
                .table(*table)
                .get(name.as_str())
                .and_then(|entry| entry.alias)
                .map(|alias| {
                    (
                        alias.file.index(),
                        self.facts(alias.file).aliases[alias.id.index()]
                            .native_span
                            .start,
                    )
                })
        });
        for (table, name) in conflicts {
            let Some(entry) = self.state.store.table(table).get(name.as_str()) else {
                continue;
            };
            let (Some(alias), slots) = (entry.alias, entry.slots) else {
                continue;
            };
            let file = alias.file;
            let span = self.facts(file).aliases[alias.id.index()].conflict_span;
            // An export specifier of a local names that local; any other
            // alias conflicts on a shared meaning even when it resolves back
            // to the local itself, as an import cycling through its own
            // module does (TypeScript's TS2440).
            let local = matches!(
                self.facts(alias.file).aliases[alias.id.index()].kind,
                AliasKind::Lexical { .. }
            );
            let overlap = match self.force(Task::Alias(alias))? {
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
                return Err(self
                    .error(
                        InputErrorKind::Binding,
                        file,
                        span,
                        format!("import or export `{name}` conflicts with a local declaration"),
                    )
                    .into());
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
pub(super) fn restrict(resolution: Resolution, type_only: bool) -> Resolution {
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
        ValueTarget::Json { value, .. } if *value == super::JsonFacts::ROOT => JSON_VALUE,
        ValueTarget::Json { .. } => "JSON property",
        ValueTarget::Member { .. } => "member of a library interface",
    }
}

/// A name that a value's type does not have as a property.
pub(super) fn no_property(name: &str) -> Resolution {
    Resolution::Unresolved(format!("the value has no property `{name}`").into())
}

/// Select the meanings a reference needs, retaining each target's own access.
pub(super) fn mask_resolution(
    resolution: Resolution,
    mask: u8,
    type_context: bool,
    name: &str,
) -> Resolution {
    match resolution {
        Resolution::Found(found) => {
            let selected = found.select(mask, type_context);
            if selected.is_empty() {
                Resolution::Unresolved(format!("`{name}` has no matching meaning").into())
            } else {
                Resolution::Found(selected)
            }
        }
        Resolution::Value { type_only, .. } if !selects_value(mask, type_only, type_context) => {
            Resolution::Unresolved(format!("`{name}` is a value with no matching meaning").into())
        }
        other => other,
    }
}
