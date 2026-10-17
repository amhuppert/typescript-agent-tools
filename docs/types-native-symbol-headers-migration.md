# Actual native symbol headers

This migration serves the existing loaded-project/query-checker envelope.
TypeScript 7.0.2 binder and checker operations define symbol identity and table
mutation. Lexical navigation keeps its documented meaning/group projection.

A raw/merged view bit on BoundSymbol cannot represent the third symbol allocated
by cloneSymbol, nor independent one-way clones of one source. Retaining only
old fields also fails when source tables are drained or target tables mutate.
The chosen boundary is a Store-owned arena of actual native headers, with an
explicit checker merged-symbol map. The existing lexical SymbolId union and
Slots remain a separate projection. BoundSymbol denotes a native header.

The header owns flags, exact name/key, actual declarations and ValueDeclaration,
Parent, ExportSymbol, and optional Members/Exports table identities. Raw getters
never follow either lexical union links or checker merge links. getMergedSymbol
is the sole explicit checker-map operation. Native table rows retain headers;
raw keyed lookup does not enumerate or acquire unrelated metadata. AliasRef
remains a resolution recipe, but the binder registers its actual Alias header.

Factories mutate these records at their native acquisition/publication point:

- Binder declareSymbol reuses one compatible raw table entry or creates the
  actual replacement header. Meaning-separated lexical roles attach to that
  same header without creating extra native symbols. Named exported declarations
  retain separate local/export headers and actual links.
- Checker cloneSymbol allocates a fresh transient header, copies target Parent
  including nil, shallow-clones each nonnil table, and records target-to-clone.
  mergeSymbol appends source data, merges native tables and, unless one-way,
  records source-to-target. An already transient target mutates in place.
- A source declaration keeps its original header. Initialized globalThis keeps
  its own mutable header and self-referential global Exports table. Conflicting
  declarations do not replace it.

Implementation checkpoints are (1) arena/table contracts and binder factories,
(2) checker clone/merge work scheduling and explicit links, (3) native facade
migration and removal of old bound_* snapshots/native-group reconstruction,
then (4) final raw-globalThis, paired-symbol, augmentation and Q1 compatibility
proof. Each checkpoint must compile coherently before further expansion. No
post-merge synthesis from projected sites is admitted. Existing unavailable
factory branches remain failures until their complete native factory is wired.

The first Stage29 compilation checkpoint stages the arena and the initial
binder event attachment only. `BoundSymbol` still denotes the old projection,
and existing raw metadata guards remain in place. The new keyed lookup can
admit stable binder headers; its temporary bridge refuses any header the old
handle would canonicalize or replace. ConstEnumOnlyModule likewise reads the
actual binder field only when the old handle preserves it. Neither adapter
claims checker clones or created module flag copies are complete.

Before changing those admissions, the next checkpoint must finish actual
container table placement (including replacement declarations and shared
function Locals), route checker merges to the new tables/map, then migrate
BoundSymbol and remove the bridge and snapshots. Current native enum/private
flags and key constructor APIs are shared prerequisites, not completion claims.

The arena's clone operation refuses an unfinished binder table: pending name
registrations belong to the original table and cannot settle a shallow copy.
A keyed hit reads only its own header; full inventory separately observes any
unrepresentable key retained in that table. Native key construction never
allocates an identity-order rank.

The Stage32 binder checkpoint routes declaration factories through an explicit
pair of lexical and native destination tables. Replacement declarations own
their own Members/Exports tables; signature and body scopes share the actual
Locals owner. Alias placement, export-assignment ValueDeclaration, prototype
insertion, and ambient/global module declaration attachment use the same raw
table owner. Native symbol keys now retain NativeHeaderId rather than a lexical
union identity. These changes leave the guarded semantic bridge in place.

Checker merge must next run at the existing per-entry work schedule, including
alias resolution and post-child recordMergedSymbol. Replaying native merges
after lexical merges would observe later table mutations too early. Exact
Interface-only resolution and generated-name literal production remain required
follow-on owners; this binder checkpoint does not admit either through a broader
meaning mask or a formatted identity substitute.

The next core checkpoint wires native checker merges before each corresponding
navigation projection change. Its explicit frames retain the table entry while
`resolveSymbol`, clone publication, Members, Exports, and the final source merge
link run in native order. Raw resolver answers carry header identities directly;
they never reconstruct a target from a completed lexical `Resolution` or `find`.
The existing scope traversal and storage decisions serve both result domains.
Public handles stay guarded until the following facade checkpoint. Compiling an
unused second merge implementation would not establish this checkpoint.

Stage34 core checkpoint: the active checker merge path now runs through
`link/native.rs` before navigation projection. `NameScope` is the shared
per-scope recipe for both result domains; raw alias targets and export tables
have native-header keys and survive projection cache resets. Members, Exports,
and source-map publication are separate stack phases. Pattern requests retain
their original header and receive the native augmentation result at that merge's
turn; navigation's later `absorb_patterns` changes only its projection.

The export worker now visits complete tables, including an actual binder
ExportStar header and ExportDeclaration associations, before caching. Namespace
wrapper/default-import fallback, export-value member lookup, late-container
lookup, and the implicit arguments factory remain explicit prerequisite failures
when the raw startup resolver reaches those operations. They are not completed
by substituting a projected target. Public BoundSymbol/raw-table/parent facades
remain guarded pending the next actual-header migration, so this checkpoint does
not claim globalThis raw metadata or aliasSelf final-query completion. No local
Rust validation ran; the parent coordinates the combined compiler gate.


Stage35 facade checkpoint migrates BoundSymbol and BoundAliasSymbol to actual
NativeHeaderId. Alias is an operation tag only; its declaration recipe is chosen
by FindLast on the current header at the native question. GetDeclaredType and
GetTypeOfSymbol inspect all actual flags before Alias dispatch. Raw declarations,
ValueDeclaration, Parent, members/exports, node.Symbol and source module requests
now read the arena. The old bound-parent/native-declaration group snapshots and
projected semantic export/name resolver have been removed. Navigation still uses
its existing lexical result domain through the shared NameScope policy.

The module exports cache and late-container resolvedExports cache are distinct
native slots. Only the module getter installs typeOnlyExportStarMap; checking
that map observes it without resolving a table. Checker-created shallow tables
use the same worker with their own actual symbol identity. The entity-name walk
retains which head/member was absent, follows aliases by the requested meaning,
and explicitly observes getMergedSymbol even after special scope shortcuts.

The bounded peer review certified actual enum header/declaration/parent order,
and the earlier core review certified merge publication and export traversal.
Final compilation and Project queries remain required. Late-bound semantic
member creation, implicit arguments, and raw lookup paths requiring a namespace
wrapper or export-value property computation remain explicit prerequisites of
the native resolver. This checkpoint does not replace those missing operations
with projected lookup results or claim their final-query proof. The evaluator's
existing complete alias/module operations remain the owner when directly asked
for the alias target. Raw globalThis inventory and aliasSelf are priority final
queries for the combined gate, alongside reference compatibility and qualified
missing-name provenance.
