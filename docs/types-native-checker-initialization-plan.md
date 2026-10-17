# Shared checker initialization for load-time merges

Status: accepted A2 boundary, connected implementation in progress. Loading and
each fresh controlled request now use the shared initializer over immutable raw
binder inputs. QueryBindings and the alternate load-native resolver/cache import
are removed. The connected library root passes, including the formerly failing
eighteen JSON augmentation cases. Actual JSON ValueDeclaration carriers pass
four public initialization tests and eight pinned compiler checks, including
direct/service recovery causes, native cache replay and failed-refresh generation
behavior. The final connected library and selected JSON/augmentation/member/
relation compatibility matrix passes. Full-project compatibility, representation
and resource gates remain open; see
[implementation status](types-implementation-status.md#json-declaration-carriers-and-shared-initialization-proof-2026-10-12).

The staged interfaces and earlier checkpoint descriptions below preserve the
migration rationale. Their references to disabled initialization or retained
QueryBindings are historical, superseded by the connected execution cutover and
the current status above.

## Operating contract and observed dependency

One immutable Project generation supports independent concurrent requests.
Loading must retain the current lexical admission checks and exact InputError
locations. Queries perform no filesystem IO and own their mutable semantic
caches. TypeScript 7.0.2 is the behavioral source; this proposal does not add
whole-program diagnostic checking to Project::load.

The current cycle is concrete:

1. `Linker::run_native_merges` resolves a non-transient merge target before
   cloning it (`binding/link/native.rs`, `Work::Symbol`).
2. An imported JSON property requires `getExternalModuleMember`: obtain the
   export-assignment value type, then get its actual property, before querying
   the original module's exports (checker.go 14593).
3. That property is created by the existing evaluator's `construct_json`,
   object-property factory, and widening owners. `getWidenedProperty` preserves
   identity only when the widened type is unchanged (checker.go 18345).
4. The staged evaluator borrows CheckerInputs independently of Project, but
   still explicitly requires QueryBindings over completed Bindings. Removing
   that initialized native dependency is the remaining lifecycle cycle.

Raw JSON binder properties cannot break this cycle. `checkObjectLiteral`
(checker.go 13180) creates a transient property for each initializer, and the
last property enters the final table. Its installed type can come from the last
duplicate while its copied ValueDeclaration still points at the first. Array
global validity, nullable widening, and sibling widening contexts can determine
whether another property identity is created. Neither projected Slots nor a
JSON-shape classifier supplies the completed native identity.

## Alternatives and choice

**A2. Shared initialization, fresh state per invocation (chosen).** Separate
captured checker inputs from the completed Project. Linker uses a temporary
checker and asks the same operations that requests use. After lexical admission
and indexing, discard that checker. Project retains raw binder inputs and the
initialization recipe; each fresh request replays the same initialization owner
before its selected operation. This moves actual acquisition to where it is
needed and keeps JSON/type/alias/merge decisions in their existing owners.

**A1. Shared initialization, fork a completed state seed.** This would preserve
the work from loading, but requires a graph-aware fork of all ID-bearing arenas,
counters, and caches. Signature currently contains Arc-shared mutable OnceLock
slots, so ordinary Clone would leak contextual installation between requests.
No measured replay cost or public cross-request checker-identity contract
justifies that extra mechanism. Start with A2; consider a seed only after a
measured performance need, without changing operation ownership.

**B. Defer native initialization until the first request.** This would avoid a
load-time semantic state seed, but it does not currently preserve loading.
Projected `lookup_name` calls `scope_export_lookup`, then consumes actual
named-default flags and projection; raw-local lookup consumes native flags
(`binding/resolution.rs`, currently 1080–1110). Mixed Alias|Interface,
declaration order, and transient-clone state affect those decisions. Keeping
the old Slots walk or binder-only headers would change Q1 and load outcomes.
A faithful deferred version therefore needs a second exact load-time facts
execution or a deferred-symbol protocol through every projected consumer. It
does not remove the hard acquisition and has a larger policy surface.

Do not implement an intermediate raw-property fallback, preallocate a guessed
final symbol, or replay property factories under a site-to-identity cache. A
factory's source site is not its native identity.

## Immutable input view

Introduce `semantic::CheckerInputs<'a>`, constructed from either the loader's
captured Program plus completed binder facts, or the final Project. It borrows:

- Sources and FileLookup, configuration path, SemanticOptions, and the frozen
  SpecifierEnvironment;
- immutable per-file syntax/binding facts: declaration kind/full span and raw
  declaration-symbol associations, scope plans/tables, aliases, stars, JSON
  facts, flow facts, diagnostics eligibility, module requests and resolved raw
  paths, module formats, ambient declarations, and augmentation order;
- immutable binder native headers/tables and raw ambient host associations.
  These are the raw inputs, not merged checker tables and not union-find
  canonical projections. Checker-created globalThis is an initialization recipe,
  not a binder allocation retained under that name.

It must not require Index, occurrence attachments, a fake completed Bindings,
or navigation's post-merge representative. Split the existing binding input
adapter at this boundary; reuse its Loading/Retained file-facts mechanism.
Move semantic fact accessors off completed Bindings where they only need these
captured inputs. Keep lexical index operations on the final Bindings.

Source receives this view, Parser, and Control. Its captured Ast cache remains
local to the current invocation. Existing `location(Project, ...)` helpers
take the source/input view instead. This is a concrete view with two constructors,
not a pluggable host trait and not a filesystem abstraction.

The staged interfaces are concrete; neither loading constructor enables a
checker nor supplies missing retained state:

```rust
#[derive(Clone, Copy)]
enum BindingFiles<'a> { Loading(&'a Program), Retained(&'a Bindings) }

#[derive(Clone, Copy)]
struct CheckerInputs<'a> {
    sources: &'a Sources,
    files: &'a FileLookup,
    config: &'a Path,
    bindings: BindingFiles<'a>,
    raw_bindings: &'a RawBindingSnapshot,
    options: &'a SemanticOptions,
    specifiers: &'a SpecifierEnvironment,
}
impl CheckerInputs<'_> {
    fn from_project(project: &Project) -> CheckerInputs<'_>;
    fn during_loading(program: &Program, files: &FileLookup,
                      raw: &RawBindingSnapshot) -> CheckerInputs<'_>;
}
impl Evaluator<'_, '_> {
    // Current checkpoint: same request-owned QueryBindings cache as before.
    fn new(inputs: CheckerInputs<'_>, bindings: QueryBindings<'_>,
           parser: &Parser, control: Control) -> Self;
}
```

Source receives the same explicit inputs, QueryBindings, parser and control.
BindingInputs still serves lexical resolution and keeps `&Store` plus
ProjectedHosts separately; it is not embedded in CheckerInputs. Its Loading
host association retains AmbientHost/Fragment storage, while its Retained
association reads completed lexical representatives. BindingFiles returns the
original names, occurrence spans, flow graphs, module facts and JSON facts with
the captured owner lifetime. No view extends a borrow of a temporary value.

The later enabling constructor removes QueryBindings. It accepts the same
Parser and an optional Control: requests supply their existing ledger; loading
currently has no product ledger or deadline. Initialization and native read
operations use the evaluator's state plus raw_bindings. This is a planned
interface change, not a claim that the current constructor supports loading.

`InitializationRecipe` is the ordered existing global/UMD/ambient/augmentation
inputs, not a recorded list of resolved targets. Resolution is performed by the
shared owner at its original phase during each run. Loading drives those phases
alongside its projected checks; the request constructor runs the same complete
recipe without lexical publication.

## Mutable state and the borrow seam

The current evaluator-owned state contains Model and all evaluator caches,
property and signature arenas, demands, actual symbol overlays, raw created
tables, merged links, alias completions, and export completions. Keep these
owned by one evaluator. Replace its Project borrow with CheckerInputs; its
Source uses the same view. Initialization and ordinary queries use this same
facade and the same semantic owners. A new state wrapper is unnecessary for
replay: do not move every field merely to name this conceptual boundary.

The binder facts become immutable at the new post-star/request capture boundary
described below; navigation's mutable projection remains separately owned by
Linker. Checker-created and
checker-mutated native headers/tables move to evaluator state. Extend the existing
`reference_current_header`, `getMergedSymbol`, and raw-table owners to supply
initialization tables as well as late merges. Do not copy checker mutations
back into raw binder headers.

The freeze is a lifecycle change, not a rename of the current method. Today
`finish_binding_headers` precedes request-header capture and globalThis creation.
Capture each request's raw module/pattern association before projected request
retargeting; preserve the globalThis creation/association recipe at the native
initializeTypeChecker phase (checker.go 962). Retain raw ambient host identities
before `Completed.ambient_symbols` currently applies store.find. The initializer
creates globalThis and its global-table export association inside its own state;
the loader projection receives its corresponding lexical adapter separately.

Between completed checker operations, projected resolution receives a read-only
native-facts view. It asks the shared `resolveNameHelper` precheck for actual
flags, transient state, declarations/default-name facts, and actual raw table
entries. The precheck policy remains one operation. Its input provider reads
the checker overlay plus binder facts; it does not reconstruct flags from Slots.
The projection association returned alongside an actual header is an adapter
for the lexical result only, never a native symbol identity.

`NativeFacts` exposes complete read-only questions needed by the projection:
the native scope-export precheck (Skip/Table/NamedDefault with its actual flags
and projection association), actual local-symbol flags, and the current raw
header/table observations used by those questions. It takes raw binder scope
and symbol associations and reads the evaluator's current overlay. It cannot
resolve an alias, materialize a type, advance initialization, or manufacture an
absent result for a missing fact. The existing `scope_export_lookup` policy is
parameterized by these facts as one whole operation; both current native name
selection and projected lookup call it. Its default-first ordering, pure-alias
test, and ModuleMember narrowing stay together. Only the facts provider changes.

There is no simultaneous mutable borrow of Linker's Store through the evaluator:
raw binder inputs, projection writes, and evaluator state are separate fields.
The orchestration borrows the evaluator mutably to complete one operation, then
borrows its facts immutably for the existing projected operation. No RefCell or
unsafe lifetime extension is needed.

## Initialization and publication

Move the native merge scheduler into the shared checker operation owner. Reuse
`merge_reference_symbol_completion`, extending its direction argument to cover
the existing Both/OneWay initialization calls. Reuse the existing alias,
external-module-member, module-export, property, array, widening, and global
owners. Retire the load-only native alias/merge implementations once all their
initialization callers use that owner; retain lexical alias resolution because
it asks a different projection/admission question.

Preserve native `initializeChecker` order (checker.go 1290–1398): global script
symbols and first-wins UMD entries; global augmentations; undefined and fixed
global installation; deferred ambient module symbols; nonglobal augmentations.
Intrinsic Model creation alone does not install fixed globals. Fixed-global
identity recipes must bind to that phase's symbols, before ambient/nonglobal
merge work can acquire their types. Keep the documented startup-identity versus
later array-construction completion distinction.

For the failing JSON import merge, the exact sequence is:

1. Resolve the actual import alias through the existing checker owner.
2. Acquire the JSON SourceFile value, check every original initializer, build
   actual transient properties, and run the shared whole-root widening owner.
3. Select the actual value property before the original module's export table;
   complete native default/import-specific branches at their existing owners.
4. Publish the alias target according to resolveAlias's barrier/pop rules.
5. The original merge target's Transient bit decides cloning before alias
   resolution. A nontransient alias therefore causes cloneSymbol even when its
   resolved JSON property is transient. Copy only Flags/Name/Declarations,
   Parent, ValueDeclaration, Members, and Exports (checker.go 14269); do not copy
   resolvedType or SymbolLinks.target. The existing clone_reference_symbol owner
   already installs the declaration-backed value recipe with no cached type.
6. Merge Members before Exports, then publish the one-step source merge link at
   the existing point. OneWay suppresses only its corresponding source link.
7. Run the existing projected merge/admission work using the completed native
   facts at that phase. Finish lexical validation/index publication afterward.

This distinction is observable with duplicate JSON keys. The ordinary checked
object table carries the last initializer's type, but an imported property
cloned by augmentation loses that installed type and later checks its copied
first ValueDeclaration. Native root's duplicate-object and duplicate-scalar
controls return the first object's type after the merge. Preserve both
behaviors at their respective owners; do not extend the ordinary last-property
rule across cloneSymbol.

Keep recovery and interruption distinct. JSON and the array/widening operations
on this route must return their stored NativeCompletion through the bootstrap;
do not call `.admit()` just because the caller is loading. A clean or recovered
native value can establish identity. Interrupted operations cannot supply an
identity and cannot publish completed initialization. Native eager diagnostic
acquisitions also remain shared: mergeSymbol calls symbolToString(target) for a
NamespaceModule conflict except globalThis, and reportMergeSymbolError calls
symbolToString(source), including after a resolved-target conflict (checker.go
14072–14163). Loading cannot bypass these alias/export/display acquisitions or
their cache effects. Only subsequent emitted-diagnostic/admission policy differs:
the existing projected Binding rejection remains the load admission owner.
Do not implement a cheaper merge execution for the loader.

Do not attach one bootstrap recovery to all queries. Preserve evidence in the
specific installed type/alias/export/global cache property. An unrelated
annotated query stays healthy; a query that actually consumes recovered array
construction fails at its own consumer. For a newly exposed interruption in a
currently healthy native load control, complete the reached owner: neither
publish partial native tables as complete nor weaken the fixture to Unsupported.

## Fresh requests and the retained-consumer ledger

After successful initialization and existing load checks, retain only the raw
captured binder inputs, the ordered initialization recipe, and the completed
lexical projection/index. Discard the load evaluator. No TypeId, PropertySymbolId,
DemandId, request-created signature, or mutable semantic cache crosses into a
later request. That request runs the same initializer in its own fresh evaluator,
then its selected Q3/Q4 operation. Charge initialization work through the
existing request Control; no filesystem or parser-policy change is involved.

The current dependencies on load-created native state are explicit migration
sites, not a reason to copy a checker graph:

| Current retained reader | Replacement |
| --- | --- |
| Completed.native_aliases/native_exports/native_resolved_exports | The request initializer installs these slots through the existing semantic owners; remove the retained load-native caches. |
| Evaluator module_exports and symbol_tables importing those snapshots | Read their own initialized cache; retain nonforcing distinctions within that checker. |
| Bindings::merged_symbol, module_symbol, parent_symbol and declaration adapters applying load merge links | Return the raw binder association, then use the evaluator's actual getMergedSymbol operation where the native caller asks it. |
| Global/name/raw-table lookup reading load-mutated NativeHeaders | Read the request's current table/header overlay, falling back only to immutable raw binder inputs. |
| Projected named-default/default-export precheck during loading | Borrow actual facts from the temporary load evaluator; retain only its completed lexical result. |

The handoff includes less obvious wrappers: globals.rs `global_type_symbol`,
`global_type_worker`, and `GlobalTypeLookup::Target` must accept actual
ReferenceSymbol and select from the current overlay (including wrong-kind Alias
behavior); source/references.rs `reference_declaration_origin` starts from the
raw declaration before the request's late-symbol and one-step merge owners;
property_symbols.rs `canonical_parent` and the raw-table/parent/globalThis
readers use the current checker; helper_module, reference_module_symbol, and
file_module_symbol must not retain load-merged handles. Q3's separate lexical
SymbolId grouping remains a projection question and is not compared with native
checker identity.

Audit projection-to-semantic adapters while migrating: they may retain raw
source associations, never a load-created clone handle masquerading as a raw
binder declaration. Native identity within each checker remains exact, including
all property reuse-versus-creation decisions. Public symbolic identities and
origins do not expose the compiler's process-wide numeric GetSymbolId values.

This avoids new state cloning, signature-slot migration, parser-cache copying,
and arena remapping. Existing request-local signature sharing remains inside
one evaluator. The only performance tradeoff is repeated native initialization;
measure that after correctness instead of adding a seed preemptively.

## First coherent migration and affected callers

The completed preparation chunks preserve behavior before loading is enabled:

1. Extract Loading/Retained facts, then separate BindingFiles from lexical
   BindingInputs and its projected host/store borrows.
2. Capture RawBindingSnapshot after stars and raw module requests, before
   globalThis creation and checker merges. Retain both copies temporarily;
   existing readers still consume the initialized Store.
3. Pass CheckerInputs through Source, Evaluator and located failure adapters;
   keep QueryBindings explicit. Q3's four inventory operations accept facts or
   an explicit retained inventory. No initialization call is added.

The first two chunks have compiled and passed their coordinated regression
checkpoint. The third is under its coordinated regression checkpoint now.

The following complete behavior slice adds the shared initialization scheduler,
phase-correct native-facts adapter for projected lookup, JSON completion route,
and fresh-request initialization in one coordinated change. The eighteen JSON regressions must
turn green before claiming this slice complete. Do not leave a permanent second
JSON or merge algorithm while migrating.

Primary files: project/mod.rs; binding/{link,retained,resolution,semantic}.rs,
binding/link/native.rs and the raw native store; semantic/{source,evaluation,
model}.rs; evaluator symbol_tables, symbol_aliases, module_exports, json,
property_symbols, arrays, globals, widening. Other evaluator callsites need
mechanical context access changes only. No signature-contextual ownership edit
is needed for replay, so root's current contextual work remains separate.

## Completed preparation: raw capture without changed readers

This binding-only increment passed source review and its coordinated library
checkpoint. It makes the loader's pre-initialization facts available after the
current loader finishes; it does not fix the eighteen JSON regressions by itself
and is not completion of shared initialization.

Capture once in `Linker::merge_augmentations`, after `collect_stars` and the
existing raw request-header loop, immediately before `merge_globals`. Earlier
capture at `finish_binding_headers` misses native ExportStar declarations and
request associations. Later capture includes checker-created globalThis,
undefined, merge clones, merged tables, and pattern retargeting. Capture must
precede the `mem::take` operations on global exports, ambient fragment stars,
and nonglobal augmentation order.

Add one immutable `RawBindingSnapshot` in the binding owner, retained by
`Completed` and borrowed by CheckerInputs.raw_bindings. A narrow binder-snapshot
operation on the native store copies the original header/table arenas and
their exact association maps without using table enumeration as a conversion.
This preserves allocated-empty versus absent tables, pending-name counts,
unavailable-key failures, and declaration order. It excludes checker merge
links, pattern-augmentation replacements, alias targets, and export caches.
The capture owner checks that checker merge/pattern state is still empty; a
violated lifecycle is an invariant failure, not permission to erase that state.
The already allocated empty table associated with projected GLOBAL is an arena
slot, not an initialized checker global table: requests must obtain globals
from the initializer's own state.

The exact retained fields are:

| Field | Source and invariant |
| --- | --- |
| Native headers and tables | Raw NativeSymbols arenas, preserving IDs, header declaration order, raw Parent/ExportSymbol links, actual Members/Exports presence, and table failures. No Store::find. |
| Declaration, alias, node-local and SourceFile.GlobalExports maps | Existing NativeSymbols maps; declaration recovery identities remain distinct from navigation groups. |
| Scope table associations | Convert each current scope_tables TableId once through Store::native_table. Preserve None. |
| Declaration LocalSymbol associations | Convert decl_local_symbols directly through header_for_symbol. Declaration.Symbol uses the actual NativeSymbols declaration map, not decl_symbols' projected representative. |
| File-module, ambient-host, augmentation and global-augmentation associations | Convert original hosts/modules to raw NativeHeaderId before any projection retargeting. Ambient hosts retain their original per-file declaration order, including fragments. |
| JSON node and export-assignment associations | Capture the existing direct JSON maps as native header IDs before Store::json_symbol/json_export_symbol can canonicalize through find. Preserve duplicate raw ValueDeclaration behavior. |
| Raw module requests | Every request retains Module { header, pattern }, Script, or Ignored. Keep the existing per-file ModuleResolution alongside it so Missing, Unchecked, Unsupported and NotAdmitted remain distinct. A Module capture missing its header is an invariant failure. |
| Exact ambient names and ordered patterns | Existing ambient/pattern associations mapped to raw headers. Pattern replacement is mutable checker state and starts empty. |
| InitializationRecipe | Program-order script-local tables and UMD entries; global augmentation headers; ordered nonglobal (file, request, source) triples. The initializer classifies every raw script-local entry and queues every ambient module, including first declarations, for the deferred pass. These are input identities, never pre-resolved alias targets. |

Native initializeChecker defers every ambient Module symbol in each script's
Locals (checker.go 1305–1316); mergeGlobalSymbol chooses its target when the
deferred pass executes. Existing Fragment target/source pairs describe only
later files and must not replace that pass. The captured raw script tables,
per-file ambient declaration facts, and original ambient hosts are authoritative;
no resolved target/source ambient schedule is retained as an initializer recipe.

GlobalThis creation, undefined, arguments/unknown types, fixed-global lookup and
automatic array construction stay operations in the initializer, not fabricated
binder entries in the snapshot. Their constant names and native schedule need
one operation owner, not a copied configurable list in the recipe.

For this transitional increment, the existing mutable Store and its current
native initializer remain untouched, and no existing query switches to the new
snapshot. The temporary additional raw native copy is intentional and bounded
by captured binder data; it is not a second token inventory or a CheckerSeed.
After shared initialization is enabled, remove the obsolete load-native copy
and its caches rather than retaining both indefinitely. Do not clone all of
Store, lexical resolutions, parser arenas, or semantic state to achieve this.

The staged production owners are binding/raw.rs and link/raw.rs, store/native.rs
and store/json.rs for owned capture, link.rs for the lifecycle call, and
retained.rs for transfer into Completed. No lookup policy, load admission or
cache consumer changed in that increment. Its regression checkpoint retained
the identical known JSON failures. The later enabling slice supplies the native
end-to-end proof below; a helper assertion alone is not behavior proof.

## Context and enabling access ledger

The context-only rewrite is staged after the conditional checkpoint passed.
Source and Evaluator now retain Copy CheckerInputs; Project remains at public
Q3/Q4 selection and result assembly. Source::new and Evaluator::new accept the
same view plus explicit QueryBindings, and located failure helpers use captured
Sources rather than a completed Project. Source's Ast lifetime remains tied to
captured Sources, not to a temporary view. Parser/Control and invocation-local
syntax caches remain unchanged.

BindingFiles' staged File view contains module/declaration/module_semantics,
diagnostics, flow, JSON, scopes, declarations, aliases, stars and requests. Its
Reference view preserves the original name/span/base/meaning/property/type_context.
The enabling slice still needs conditional_scopes, lookup_scopes, JSX/search
facts where reached, ModuleResolution values, ambient/module declarations and
instantiated augmentations. Loading obtains these from ProgramFile/FileFacts/
collect::Facts; Retained obtains them from its existing File. Expose a reference
count and reference(RefRef), not a fabricated common reference slice: Loading
names collection tokens while Retained names occurrence IDs. Add the original
flow attachment to Reference for existing value_reference_flow callers. Do not
supply load-derived value_origin/type_alias_origin during loading; those are
lexical results and semantic consumers reacquire their actual identities.

The method migration is by question, not by moving every Bindings method to
another impl:

| Current access | New owner / required action |
| --- | --- |
| declaration_name/kind/full-span, alias_declaration_at and alias_syntax_is_type_only | BindingFiles over borrowed declaration/alias facts; preserve ambiguity and NotRetained errors. |
| json_facts and JSON declaration location spans | BindingFiles over the existing captured JsonFacts; no JSON reconstruction. |
| flow_graph/flow_attachment/file_is_module | BindingFiles over original graph/module facts; explicit nil flow remains distinct from missing attachment. |
| reference_module_sites/site/specifier/resolved_path, file module format/default-library/diagnostic facts | BindingFiles over captured requests and ProgramFile facts; keep raw paths and all resolution modes. |
| entity_name_recipe, lexical_reference_index, type_reference_index | Existing recipe/selection policy reads the borrowed reference view; names remain borrowed until an owned recipe actually needs them. |
| raw declaration/LocalSymbol/JSON/SourceFile associations | RawBindingSnapshot accessors returning the original bound header, without merged-symbol normalization. |
| flags, declaration lists, Parent/ExportSymbol, raw locals/members/exports, parameter-property selection, global selection | Current evaluator header/table owners over raw inputs plus its mutable overlay. An unchanged-looking BoundSymbol does not license a load-mutated header read. |
| QueryBindings::select_* and resolution name_scope/scope_export_lookup | Preserve the shared scope/precheck policy; use current NativeFacts for header/table eligibility. Loading lexical projection borrows the same completed-operation facts. |
| QueryBindings methods invoking Resolver::request for alias/global/member/export results | Migrate callers to existing evaluator resolveAlias/name/member/export owners; remove the redundant native cache/import path once shared initialization is enabled. Keep the different lexical resolution operation. |
| Completed cached alias/export/type-only tables | Remove semantic fallback to these completed-load caches; the current evaluator installs and observes its own slots, including nonforcing absence. |

QueryBindings' present Deref<Target=Bindings> hides these distinct dependencies.
Replace that implicit dependency during the coordinated rewrite; do not add a
Loading branch that pretends a completed Bindings exists. The first mechanical
checkpoint may still use existing request-native behavior, but enabling loading
requires all current-state rows above to use the evaluator overlay.

The completed reference inventory is a separate, finite Q3 seam. The only
Source methods that consume ReferenceToken/Index facts are reference_site,
reference_selector_anchor, reference_node_site, and reference_declaration_name.
Their callers are Q3 selection, reference_search/import tracking, and its
reference_shorthand_fallback. None is an initializer prerequisite. Pass the
retained inventory's borrowed facts into the syntax projection operations, and
pass the retained inventory explicitly through the Q3-only node-to-site and
shorthand continuation calls. Keep reference_tokens/name candidates at the Q3
facade. Do not add an optional/empty Index to CheckerInputs, rebuild the token
inventory, or let a loading call quietly return no reference role.

FileLookup can be built once from Program before binding: file_lookup reads
captured aliases and performs no IO. The same lookup is borrowed by loading's
CheckerInputs and moved into Project on successful publication. The next
semantic constructor change therefore has no reason to create an incomplete
Project or duplicate resolution/file membership policy.

### Context-only checkpoint and ownership partition

The context-only source migration is staged for coordinated validation. It
separates BindingFiles from lexical BindingInputs, passes CheckerInputs through
Source/Evaluator and located failure helpers, and passes Q3 retained inventory
explicitly at its four syntax seams. Existing QueryBindings remains an explicit
constructor dependency, with the same shared cache. No loading evaluator or
shared initialization is enabled by this checkpoint. Completion and conditional
tail-limit behavior remain owned by their existing operations.

There is a concrete borrow obligation beneath the staged CheckerInputs type:
the extracted BindingInputs still contains `&Store`, because it is the lexical
resolver's input. A checker that keeps this borrow alive would prevent the
loader from mutating that Store's projection. Do not simply retain it inside
the final loading evaluator and declare the borrow cycle solved.

Separate the common file-fact adapter from those lexical inputs. The shared
file view has Loading(&Program) and Retained(&Bindings) storage, and owns
file/reference/name/flow borrowing. The lexical BindingInputs keeps &Store and
its projected associations; move its loading ambient-host/fragment references
out of the file view into its own projected-host association. The checker view
borrows the common file facts plus &RawBindingSnapshot, so its loading lifetime
is independent of projected Store writes. This separation moves existing
borrowing operations, not name-resolution policy, tokens, or another AST cache.

For the behavior-preserving context checkpoint, retain the existing
QueryBindings request-native facade as an explicit constructor dependency:
the public request entry creates it from completed Bindings and passes it with
CheckerInputs to Evaluator/Source. Replace evaluator/source Project fields and
location helpers with CheckerInputs. Existing initialized native reads go
through that same QueryBindings; pure source facts use the common file view.
This is an honest retained dependency during migration, not a loading fallback.
No loading evaluator is constructed at this checkpoint. Enabling loading must
remove that request-only constructor dependency after all legacy alias/global/
export/current-header questions use the evaluator's actual owners.

The concrete file partition is:

- Binding/source: binding/inputs.rs, raw accessors and syntax-only methods in
  binding/semantic.rs, semantic_flow.rs and reference_modules.rs; semantic/
  inputs.rs, source.rs and source/*.rs. Keep raw and initialized-header questions
  visibly separate; do not switch current native reads to the raw snapshot yet.
- Shared context and facade: semantic/mod.rs location/error helpers and the two
  public evaluator constructors; Q3 selection/reference search/import helpers
  pass the existing inventory explicitly at the four identified seams.
- Evaluator plumbing: evaluation.rs constructor/field plus evaluation/**/*.rs
  context and source-binding accesses. Preserve every NativeCompletion owner,
  acquisition order, cache key and completion conversion from the conditional
  migration; this phase changes dependencies, not checker behavior.

The shared interface is the same Copy CheckerInputs value. AST and returned
fact borrows keep the underlying captured-source lifetime. No Source method
may keep a reference to a temporary view, and no constructor invents a missing
index, a completed load cache, or an initialized raw global table. Once this
checkpoint passes, the shared initializer/current-overlay slice removes the
temporary request-only dependency and the obsolete load-native snapshot cache.

The later loading constructor must receive the loader’s existing Parser/Control
lifecycle. The request’s semantic deadline is not a load admission policy.
Enabling shared initialization must preserve load resource failures and retain
request accounting separately from these immutable inputs.

## Next enabling migration by operation owner

The enabling work is one behavior slice with reviewable internal steps. A
partially routed raw reader cannot be enabled: a raw header observed beside an
old load-merged parent or installed alias target is a different checker.

| Owner and present dependency | Concrete replacement and removal gate |
| --- | --- |
| binding/semantic.rs and ordering.rs raw declaration, alias, name, flags, parent, valueDeclaration and JSON attachments | Move the complete raw read operations to a binding native-input view over BindingFiles plus RawBindingSnapshot. A BoundSymbol denotes only a raw binder header. Use raw declaration kind/full span, raw alias association, declaration-local association and JSON node maps. Do not call Store.find or merged_symbol. Retain NotRetained/Pending and table failure states exactly. |
| resolution.rs Storage/name_scope/lookup_scope_chain and resolution/native.rs native_name_selected | Keep one source-scope schedule, with a facts provider for raw scope/host tables. The evaluator supplies current headers and its global table, then its existing actual-symbol meaning/alias callback. The lexical resolver consumes the same scope schedule and precheck but keeps its own projection result and alias task scheduler. |
| QueryBindings select_reference_name/select_entity_head/select_name_at_node/select_global_name | Preserve each selection's exact captured-reference or actual-node recipe, meaning mask and missing-name result. Move alias eligibility to evaluator completion operations; raw source attachment remains binding input work. Do not import RequestState native alias answers. |
| reference_symbols.rs current header, parent, raw-table and getMergedSymbol | Raw fallback reads only the captured native arenas. Existing SymbolTables.headers applies same-identity mutable headers; SymbolTables.merged applies exactly one redirect. Delete the second fallback through Bindings::merged_symbol. SymbolParent and module APIs carry actual ReferenceSymbol when created headers are possible. |
| source/references.rs reference_declaration_origin and source/symbols.rs symbol attachment | Return the raw binder attachment. Existing evaluator late-symbol then one-step merge operations remain responsible for completed getSymbolOfDeclaration. Q3 lexical grouping keeps its separate retained SymbolId inventory. |
| symbol_aliases.rs and reference_symbols.rs bound alias/UMD/module wrappers | Resolve aliases only through checker_alias_target_completion/resolve_reference_symbol_completion. Preserve absent versus unknown, first actual alias declaration, resolution stack/barrier/publication, type-only links and failures. Select raw requests before pattern and merge maps. Helper-module and file-module readers follow the same actual module owner. |
| module_exports.rs ModuleExports.links | Remove reference_cached_module_exports import and all snapshot type-only fallbacks. The shared getExportsOfModuleWorker installs the evaluator's own NativeResult links. Missing type-only map remains a nonforcing absent observation; it does not invoke the export worker. |
| symbol_tables.rs resolved Members/Exports | Remove reference_cached_resolved_exports import. Keep the distinct resolved-table cache and late-container priority; the module worker's late_worker path must not publish module links. Raw table nil, allocated empty and unavailable remain distinct. |
| globals.rs global_type_symbol/global_value_symbol/global_type_worker, GlobalTypeLookup and alias cache | Select from the evaluator's current global table using the existing getSymbol completion owner. Carry ReferenceSymbol instead of assuming BoundSymbol. Preserve direct flags before alias eligibility and wrong-kind alias behavior. Freeze fixed getter targets at the native initialization phase, before ambient/nonglobal merges. Lazy Promise/Iterable and other getters remain lazy. |
| json.rs construct_json/json_declaration_type; globals.rs array factory; members.rs created-property value worker | Migrate the whole JSON child/union/array/widening operation to NativeResult. Implement the actual emptyGenericType→emptyObjectType generic-global factory branch, retaining recovery. Dispatch a cloned JSON ValueDeclaration through the same JSON worker; do not require a TypeScript AST declaration or wrap the result clean. |
| symbol_tables.rs merge and property_symbols.rs creation | Extend the existing complete merge owner with Both/OneWay, preserving clone publication, original target Transient test, declarations/valueDeclaration, Members before Exports and diagnostic acquisitions. All JSON/object/array/widening properties remain products of their current semantic factories. |
| binding Runtime/Completed native_aliases, native_exports, native_resolved_exports, LoaderExports and link/native.rs | Delete after every initialization/native query caller uses the evaluator owner. Runtime's lexical journal/export_memo/cuts/tasks remain because they answer projected load questions. Remove QueryBindings and the initialized native Store copy at the same final gate; retain raw binder facts and lexical state. |

The binding native-input view earns its boundary by owning complete raw
attachment questions that combine per-file facts and native arenas. It owns no
resolution cache and exposes no lexical Index. Do not replace QueryBindings
with another Deref wrapper around Bindings. Existing pure BindingFiles owners
stay shared; the native-input operations borrow them when declaration/source
facts are needed. The evaluator's public dependency remains CheckerInputs.

### Projected NativeFacts contract

`scope_export_lookup` presently reads current exports, tests the raw named
default before narrowing the lookup mask, tests its actual first declaration
and local name, excludes only a nontransient pure export-specifier alias, then
rejects reserved `default`. Move that whole operation behind a narrow native
facts provider. Do not copy the policy into a lexical adapter or use projected
slots to reconstruct flags.

The caller audit found no post-load lexical resolver that needs a retained
native-facts snapshot. Every `Resolver::request` caller belongs to QueryBindings'
legacy native adapters; projected `force(Task)` runs only in Linker during load.
Legacy Q1 reads the completed index and occurrence groups. Keep NativeFacts as a
load-only borrow and remove the obsolete request resolver alongside QueryBindings.
Do not retain another initialized header/table copy for hypothetical lexical use.

Raw keyed lookup must preserve its current error schedule: a known key succeeds
before an unavailable-table failure, whereas whole-table enumeration fails.
The bridge must not enumerate a table merely to serve a keyed precheck or flatten
its failure into a host-wide unavailable result.

The provider must answer current scope exports, raw lookup with typed failure,
actual header flags/transience/declarations, and the existing first-declaration
local-name association. The semantic selector retains the actual symbol/table
handle. The projected adapter consumes only the resulting Skip/Continue or
NamedDefault plus actual flags and a lexical association. `lookup_name`'s raw
local EXPORT_VALUE gate also reads flags through this provider. These are the
two present projection-native reads, not a license to route lexical lookup
through semantic getTypeOfSymbol.

A clone's lexical association is auxiliary provenance. Copy the current
binder `NativeHeader.projection` association where today's native clone owner
copies it, and retain the existing association on same-identity mutable headers.
Do not substitute that SymbolId for a native identity. Newly created properties
without such a lexical association retain None; a required projection remains
a typed missing-association failure rather than choosing an arbitrary own
declaration. Audit every existing projection reader before removing Store's
native half. NativeFacts itself cannot resolve an alias, create a property,
parse another AST or advance initialization. Captured declaration facts needed
for a created header are retained by its creating operation, not synthesized
by the adapter.

### Shared scheduler and load borrow lifecycle

Build FileLookup once before binding and reuse it on publication. Split linker
setup from initialization: bind declarations/CommonJS post-child promotion,
collect stars, capture raw request selections, then take RawBindingSnapshot
out of the mutable linker. The snapshot and Program live beside Linker while a
load evaluator borrows them. After the evaluator is dropped, move that same raw
snapshot into Completed and move FileLookup into Project. No fake Project or
self-referential linker field is needed.

One evaluator initialization driver consumes the captured recipe. It creates
globalThis and the native global table, visits scripts in program order while
deferring every ambient module, adds first-wins UMD globals, applies global
augmentations, installs intrinsic/fixed global slots, merges deferred ambient
symbols, then processes nonglobal augmentations. Pattern matches apply their
OneWay copy and publish the exact-specifier pattern result immediately, before
the next augmentation. Export-star augmentation collision acquisitions stay
inside the shared native augmentation owner.

The driver exposes completed-step observations only where projected linking
must run at that native phase. Loading supplies the existing projected merge
work; requests supply no projection work. The observation borrows NativeFacts
immutably and may mutate only the separate lexical Store. It cannot call back
into mutable semantic checking. Preserve typed native interruption separately
from a projected InputError at this seam. This is a two-caller sequencing seam,
not a second merge scheduler or a general host callback framework.

After initialization, the loader keeps the evaluator alive through its existing
hoisting/merge checks, resolution/indexing and conflict checks, since their
projected scope prechecks need actual native facts. Then discard every semantic
arena and cache. A request repeats the shared driver in fresh state before
selection; it never uses load-created symbol/type IDs. Phase observation must
not cause an extra initialization or global getter acquisition.

### Startup publication and later recovery consumption

The initialization entry point is not a selected-query acquisition. Do not make
callers consume a `NativeResult<()>` with `NativeSequence::read` or `.admit()`
before every query. The exact state boundary is:

```rust
enum InitializationState {
    NotStarted,
    Running,
    Complete(NativeCompletion<()>),
    Failed(DemandError),
}
// Private worker executes and sequences all native steps, including diagnostics.
fn initialize_worker(&mut self, /* phase observer */) -> NativeResult<()>;
// Publishes Complete only after the final step; returns only an interruption.
// Complete's evidence remains stored; this method never calls admit or clean.
fn ensure_initialized(&mut self, /* phase observer */) -> Result<(), DemandError>;
```

`ensure_initialized` is used once at load/request startup. An already complete
state is a nonforcing installation observation, like inspecting a native cache
slot; it returns without replaying the startup invocation as a dependency of an
unrelated selected expression. A running state is not completed initialization.
A failed worker preserves its outer failure and cannot publish a Project or let
a selected query continue. Keep the phase observer's projected InputError in
the separate typed load failure channel, not inside a clean native result.
This state is evaluator-local and does not survive into another request.

`Complete` retains the complete startup invocation rather than erasing it, and
the producing operations retain their own NativeCompletion slots. Subsequent
native acquisitions sequence the relevant slot's completion normally. Neither
getMergedSymbol nor raw header/flag/table identity observation is a type/alias
reacquisition; do not attach the startup trace to those reads. A diagnostic-only
startup acquisition remains in the startup trace and any native caches it
actually populated, not an invented dependency on every symbol it displayed.

The array recovery route is concrete and currently incomplete:

1. `global_type_lookup_completion("Array", 1)` installs its getter result,
   including Missing/WrongKind/WrongArity, at the fixed-global phase. Existing
   `Model::empty_generic_type` supplies native emptyGenericType identity. A
   startup identity observation does not consume global factory recovery.
2. Native `createTypeFromGenericGlobalType` (checker.go 24581) returns
   emptyObjectType when that target is emptyGenericType; otherwise it calls
   createTypeReference. `array_target_completion` currently returns an outer
   error in this case. Replace the complete generic-global creation decision,
   not merely that error branch: JSON array construction must obtain the real
   `{}` value with its located consumed recovery. Preserve the missing versus
   wrong-arity cause and original declaration provenance.
3. `construct_json` sequences every original child initializer, subtype union,
   array factory and widening acquisition. It currently returns Result, calls
   array_type's admitting wrapper, and the BoundValueType::Json caller wraps a
   successful result clean. Those routes become completion operations. The
   SourceFile value Type slot retains the full root/widening completion. Each
   created property's installed read Type preserves the completion producing
   that initializer, via the shared object-property factory. Duplicate final
   table selection does not erase earlier initializer acquisition from the
   object completion or attach it to an unrelated sibling property's own slot.
4. `alias_property_completion` sequences the source-file value before selecting
   `a`; `resolve_alias_target_completion` installs its complete target result
   before pop. Initialization reads its recovered value to continue native
   cloning/merging while preserving that alias cache evidence. No load-only
   alias resolver or clean cache import remains.
5. `cloneSymbol` deliberately copies neither resolvedType nor target. The
   merged clone has a declaration-backed Type slot. Its later value acquisition
   must accept `ReferenceDeclaration::Json` and call the same
   json_declaration_type_completion for the copied first ValueDeclaration.
   The current `property_value_declaration` TypeScript-only adapter rejects this
   variant; change the complete actual-value-declaration dispatch in the shared
   value-symbol worker. Do not copy an earlier property's type/recovery into a
   clone that native leaves unresolved. `finish_value_symbol` then installs the
   completion produced by this clone's own question.
6. A later query of X may consume the installed alias completion and/or the
   clone's own value completion, according to its actual path. It therefore
   reports InvalidSemantic/ErrorType at the JSON array start with the missing
   or wrong-arity Array cause. Query `healthy: 44` reads none of those slots and
   returns Found/44, even though its fresh checker also ran initialization.

This is the required behavior in tests/type_json_initialization.rs for
array-missing and array-wrong-arity in both strict modes, query orders and
compact/expanded repetitions. Project::load succeeds for these native-recovered
programs. Ordinary duplicate-key clones still recheck the first ValueDeclaration;
that source-based distinction does not change with completion transport.

Eager merge diagnostic displays remain native acquisitions before emitted
admission policy. Recovered startup completion alone is neither a load Binding
error nor a reason to reject a healthy query. Existing projected Binding
checks remain the load admission owner. Unsupported dependencies and actual
interruptions remain outer failures; no partial tables can be published as a
completed initializer.

### Load interruption boundary

Loading already owns a Parser on its analysis thread and enforces the existing
syntax/grammar-depth policy. It has no product Control or 30-second deadline.
Use the existing optional-Control parser convention for loading; requests keep
their current Control, including request-initialization work. Do not introduce
a dummy unlimited deadline or convert a semantic Stop into parse ResourceLimit.
Parser diagnostics still use InputError::unparsable with their original source.

Use the existing AnalysisError envelope at the composing load boundary.
Project::load, on_loading_thread, load_here, binding::bind/link entry and private
ProjectSession::refresh return AnalysisError. Program/config/parser and lexical
link helpers continue returning InputError; the existing From conversion keeps
their category, source and message. A native DemandError converts by unwrapping
its existing AnalysisError, retaining reason, causes and interruption kind.
Recovered startup completion is still not admitted here.

The alternative was extending InputError with semantic kinds and payloads.
That either loses semantic evidence or duplicates the sum and serialization
already owned by AnalysisError. Returning a new LoadError would create the same
unnecessary third envelope. This checkout-built, unpublished library has local
callers, so refining its load signature is the smaller coherent change.

This is an explicit Rust API change: Project::load's error is now AnalysisError,
and callers serializing that value directly see its enum wrapper. Existing CLI
and service Input error documents retain their fields and spelling; their
existing AnalysisError delivery owner already handles Semantic errors, causes
and Internal exit status. The reference_batch example preserves its old input
projectError record and emits the full semantic record for the new family.
Tests for legacy input behavior match the Input variant explicitly and reject
unexpected Semantic/Query failures instead of coercing them.

ProjectSession::new remains Result<Self,InputError>: it only normalizes a path
and performs no load. Refresh clears the previous generation before loading and
publishes/increments only on full success. Preserve the existing thread helper's
panic behavior; changing the Result envelope neither installs a catch nor adds
a Control/deadline. No parse ResourceLimit or tolerated Unsupported conversion
is introduced. The existing Unsupported-only native_merge_failure formatter is
retired with the obsolete native linker path, not reused for checker errors.

The transport is staged ahead of enabling. Its final proof must retain all
926 existing load probes, including the eighteen currently broken JSON cases,
and compare direct/service semantic load failures with full causes, failed
refresh generation/no-stale-answer behavior, and a subsequent healthy reload.

## End-to-end proof

- All eighteen existing JSON augmentation cases retain exact Project::load
  Binding/healthy outcomes and locations, including dependency tolerance,
  direct/star exports, own properties, whole JSON values, and primitive apparent
  properties. The full 926-case load corpus is the regression guard.
- Add native-derived final-query controls after healthy interface augmentation:
  actual flags/declaration origins, duplicate first ValueDeclaration versus last
  resolved type, nested arrays/objects, nullable widening in both strict modes,
  and valid/missing/wrong-arity Array. Observe final queries, not header IDs alone.
- Retain mixed Alias|Interface, pure export aliases, named default, pattern
  OneWay augmentation, module-star collision, and global augmentation controls.
- Query the same snapshot in both orders and through two independent requests;
  contextual signature installation in one request must not alter the other.
  Include an unrelated annotated query beside consumed recovery.
- Existing no-IO snapshot tests continue to apply. Resource controls cover
  request-initialization work at the existing boundaries before replay is used.

Native matrix status: manifest32 config-cwd and workspace-cwd maintained replay
passed together in vrun-ae24e404-7d02-4657-b6e2-57eeac9ba437. This receipt proves
the separate manifest evidence, not the proposed JSON initialization change.

### Per-entry publication evidence boundary

Pinned `mergeSymbolTable` (checker.go14035) publishes each completed key into
the already-installed target table. `mergeSymbol` installs a nonnil table
before recursion and processes Members before Exports; `cloneSymbol` records
its own merge redirect immediately. The shared mutable-table implementation
preserves these source rules, including separate storage for shallow copies.

The isolated final-query reentry witness remains unproved. A self-alias
namespace fixture returned healthy direct and cross-nested marker types, but
clone/alias behavior masked delayed table publication. A mixed Alias|Namespace
candidate did not reach the hypothesized branch: `resolveSymbolEx` follows
only `IsNonLocalAlias`, so its local namespace meaning prevents that alias
resolution during merging. Its conjunction returned 42 with circular-alias
diagnostics, not the proposed healthy witness.

Authorized diagnostic counterfactual `vrun-17891dbf-e1f5-446d-99e7-fa730fe90188`
compared eight cold native/mutant query pairs across strict and noCheck. The
isolated oracle copied the target map inside `mergeSymbolTable` and published
its writes on return, preserving the native merge/alias/clone operations and
final original table identity. Every pair matched: self-alias results 41/81/91,
mixed conjunction 42, and unchanged diagnostics. The pinned compiler cache and
shared oracle/hooks were untouched; mutant outputs are not compiler goldens.

These probes are rejected as publication proof. The broad JSON/augmentation
load and final-query matrices remain the primary enabling regressions; do not
claim they independently distinguish this reentry decision. Investigation
details and exact paired outputs are retained in
`.cc/temp/stage66-merge-publication-witness-ledger.md` and
`.cc/temp/stage66-merge-publication-counterfactual.json`.

### Stage 68 shared execution cutover (assembled, validation pending)

The production path now uses `NativeInputs` in Source and initializes one fresh
Evaluator before type/reference selection. Loading calls the same native phase
driver with no request Control, then retains that evaluator through lexical
admission/index/conflict checks. The raw binder arena moves into the immutable
snapshot before the driver; lexical Store keeps no initialized semantic arena.

The loading worker now borrows owned `LinkState` for each completed phase and
for final admission. Its `NativeFacts` view borrows the current evaluator
read-only. It cannot acquire aliases/types, attach syntax, or advance the
initializer. Raw keyed-table and declaration-local observations keep
`LookupFailure::{Binding,Native(AnalysisError)}` until the composing boundary;
only Binding uses the existing InputError formatter. The private lexical work
chain carries the existing AnalysisError envelope so these distinct failures
need no third load-error type. Pure program/config/parser collection and the
InputError diagnostic constructors retain their existing policy and payloads.

Current headers carry `DeclarationAssociation::Raw/Attached`; cloning,
SetValueDeclaration and merge append operations copy latent associations.
Syntax consumers materialize them through the existing Source attachment owner.
Clones copy auxiliary lexical projection provenance separately from their new
actual semantic identity. Header key/default/meaning prechecks share binding's
complete scope policy; the facts provider observes current tables without
flattening a failed inventory into a successful keyed lookup.

The initializer retains Complete(NativeCompletion<()>) without admitting the
startup invocation as every query's dependency. It installs intrinsic value
links and fixed global slots in pinned phase order, preserves IArguments'
source-free installed-global recovery, defers ambient modules, and immediately
publishes OneWay pattern augmentations. Later factory/alias/type reads consume
the relevant installed producer completion. Fixed identity predicates observe
nil before their slots are installed. Startup array identities use stored slot
values; their recovery cannot reject an unrelated selected expression.

QueryBindings/Resolver::request, retained native alias/export/resolved-export
cache imports, the alternate load-native merge/alias/export workers and their
checker merge/cache producers in binder storage have been removed. Existing
private native retention tests now read immutable raw inputs; Q1 continues to
read its final lexical index and projected groups.

This records assembled code, not a passing acceptance claim. The registered
compile and JSON initialization checkpoint is next, followed by all 926 load
probes, strict/noCheck final-query matrices, load/service failure transport,
refresh/no-stale-generation behavior, scoped compatibility and resource checks.
The isolated per-entry publication witness and before-phase global factory
behavior remain evidence obligations; the earlier rejected counterfactuals
are not promoted to proof by this cutover.
