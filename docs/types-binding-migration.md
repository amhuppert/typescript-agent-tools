# Bound-program migration and representation gate

This is a source audit and implementation proposal using the
`ousterhout-software-design` skill, not a production migration or a measured
representation decision. The operating envelope is one local immutable
Project generation and serialized service requests, with full replacement on
refresh. The 2026-10-10 sequencing review permits a bounded ownership move of
existing successful binding facts alongside checkpoint 1; completing unrelated
operator and Q4 graph examples is not a dependency of that move. It preserves
existing lexical behavior and does not complete BoundProgram, add semantic
roles, or authorize evaluator answers before their full baseline. No second
binder or checker-owned lexical lookup is proposed.

## What exists now

`Project::load_here` creates `Program`, calls `binding::bind(&program)`, takes
navigation indexes, and publishes Sources, FileLookup, the lexical Index,
Navigation, and effective SemanticOptions. The remaining ProgramFile facts,
diagnostics and module edges are then dropped. `link::link` drops its Store and
every Linker table after returning the Index.

The present Store freeze is an earlier boundary: `merge_augmentations` merges
globals and augmentations, absorbs patterns, resets provisional resolutions,
then calls `Store::freeze`. Linking still performs hoisting/merge checks,
forces searched token resolutions, computes directed declaration projections,
and checks alias conflicts afterwards. Publishing immediately at Store freeze
would bypass required hard failures. The retained BoundProgram boundary must
follow the complete successful `link::link` sequence, including
`check_conflicts`.

Collection already walks the final syntax tree and records file-local typed
IDs for declarations, scopes, references and aliases. Its transient NodeIds
never escape. Those facts provide the existing lexical decision owner; they
do not yet provide complete semantic source descriptions, flow nodes,
static/instance member recipes, or a complete name-bearing token inventory.

## Old facts to retained owners

The following is a transfer map, not a requirement for one type per row.
Retained binding operations should expose complete questions using typed
handles, without lending Store maps or parser arenas to a checker. Existing
compiler comments and distinctions stay at their current decision owner.

| Current fact / operation | Retained owner and consumer contract | Migration / duplication action |
| --- | --- | --- |
| `Sources::{FileId,SourceFile}`; canonical path, display path, shared SourceText | Program sources remain the one source/file identity owner; all frozen handles refer to its FileId order. | Keep one Arc to each captured text allocation. A navigation Arc is sharing, not a second source allocation. Do not copy substrings into terms. |
| `Program::file_lookup`; searched ProgramFile paths | Program file policy; one public-operand lookup, distinct from whole-program semantic FileIds. | Keep FileLookup. Retain per-file search/provenance once for candidate accounting; dependencies can supply types/origins without becoming searched occurrences. |
| `ProgramFile.facts.module`, declaration status; `Loader::file_options` | Program owns effective per-file module/input facts already decided by config, format and source indicators. | Move facts into retained file records. `FileFormat` is currently consumed in `load_file` and lost; retain any format/module facts semantic consumers need at that point, not by reopening package manifests. |
| `ProgramFile.diagnostics`, Checking, directive coverage | Existing diagnostics owner keeps the reporting/tolerance policy. Semantic admission remains a separate question. | Move once if later checking/recovery needs it. Do not reconstruct this policy from noCheck/skip flags in the evaluator. A suppressed diagnostic does not certify an exact type. |
| `ModuleRequest` and `ProgramFile.modules` | Program owns exact specifier, request kind/mode/span, resolution outcome and dependency edge. | Preserve request order and remapped FileIds. Binding's RequestTarget is a different answer: Module/Script/Ignored after ambient resolution. Retain both needed answers without copying exact specifiers into a second resolver. |
| `Facts.scopes`, ScopeKind/parent and Linker.scope_tables | Frozen binding scope identity, parents and scope table association. | Transfer associations after linking. Keep signature/body/expression-name/infer-constraint/augmentation distinctions; root is not interchangeable with GLOBAL. No checker re-lookup by spelling. |
| `Facts.decls`, Decl.kind/name/anchor/lexical/export/list | Frozen declaration identity and semantic-source declaration context. | Associate a declaration site/role with its final symbol and owning scope/host once. Move semantic fields; remove check-only metadata only when its successful-load use is complete. |
| Decl.hoisted, mixed_exports, misplaced_modules, instantiated_augmentations and declaration container bookkeeping | Existing binding validation and namespace-state owners remain load-time. | Discard consumed check-only lists after all existing failure decisions run. Preserve definition/default/order facts needed for types rather than treating every checked field as disposable. |
| Linker.decl_symbols and Store.parents/find | Frozen canonical lexical symbol identity, declaration-to-symbol mapping. | Canonicalize all frozen links at one consuming freeze. Choose stable sparse IDs plus an owner canonical map or a tested dense remap; do not let each consumer implement find. Canonical symbol creation order and declaration order must survive compaction. |
| Store.Symbol flags/sites/lists/aliases | Frozen lexical symbols with their own declarations, flags, type-parameter declaration-list identity and attached aliases. | Move rather than cloning all raw Facts beside a parallel semantic symbol table. Own sites and alias-projected reference origins are distinct: never overwrite own sites with query-group projections. |
| Store.tables Entry.slots/alias, exports/members links; `targets::{Slots,Meanings,Reach}` | Binding owns declared meanings, merged identities and final lexical/module resolution. | Retain meaning slots and type-only provenance, including the deliberately different Qualified versus ExportAssignment rules. Existing Index::Target::Symbols loses this information and cannot serve as the semantic record. |
| `Facts.aliases`, AliasKind/AliasBind/type_only/span and final alias resolution | Frozen alias identity plus target/provenance, usable for alias-inspection and alias-bearing type construction. | Preserve the local alias identity separately from its followed target. An alias is often an AliasId with no standalone Store SymbolId; do not invent equivalence by flattening everything to target SymbolId. |
| `Facts.refs` RefBase/meaning/property/type_context and final resolution | Frozen name-use attachment: lexical result or typed pending semantic obligation at a source role. | Resolve the current lexical/namespace/import questions once. Preserve the richer final result before reducing it to the old lexical Index Target. Complete the required records for nonsearched dependencies; current indexing only forces searched names. |
| Store.Module, requests; Linker.file_modules/ambient host and augmentation associations | Frozen module/namespace/export identities and finalized request targets. | Preserve JSON/module/value identity, shorthand, stars, one-way merges and augmented module ordering. Builder fragments, pattern staging and augmentation work queues can drop after final associations are captured. Needed finalized export facts must survive; do not retain a mutable merge engine. |
| Linker.values/value_symbols and ValueTarget::{Json,Member} | Binding's narrow known-value identity, with typed handoff to member/type operations where needed. | Keep a canonical association if merges created a value symbol. Do not flatten a JSON value/library member into ordinary lexical symbol identity or lose it because old refs reported NotReference. |
| `Facts.names` Name.span/text/roles/call/candidate/target; Index.files | One frozen name-bearing token inventory, including semantic roles and legacy lexical projection. | Move selected token fields once; broaden existing collection for missing literal/element/member/binding-property sites. Index file-position lookup references the inventory rather than retaining another complete token array. Spelling/roles at shared physical spans must not erase distinct semantic questions. |
| Index.by_name and by_symbol; Builder.finish | Lexical/reference indexes are immutable derived accelerators over canonical inventory and symbols. | Keep necessary lookup lists and public result order. Store occurrence/site handles where practical instead of repeated name/role/call facts. A sorted location cache can be justified by measured old query costs; it is a projection, not a second identity owner. |
| Linker.projected_sites and Index.sites | Directed lexical-group origin projection, owned by reference search/index construction. | Preserve one-way alias projection, location ordering and dedup semantics. Retain canonical declarations once; projected origins may cache declaration handles. Do not substitute projected sites for type declaration origins. |
| `navigation::Index` keyed record ordinals/keys/container/kind/anchor | Navigation continues to own product selectors and catalog records. Semantic source receives an explicit association for relevant declarations/questions. | Preserve Q1/Q2/E selection behavior. A navigation record ordinal is neither a parser NodeId nor a complete semantic identity; unkeyed expressions/types are omitted from this index. Share source/site facts only where the question is identical. |
| `JsonText` recursive values, exact JsonString keys, every JsonProperty key_span | Extend the existing JSON reader product into retained JsonFacts and common member/token recipes. | Existing JsonModule::new drops every array element, repeated-key origins and value/key spans. Capture recursive shapes and every occurrence before this loss, with distinct last-value links for import typing. Preserve existing property lookup via the same fact owner; no TypeScript-AST JSON collector. |
| Decl.interface_members and Store.interface_members | Existing narrow JSON apparent-interface name lookup, later projected from retained member recipes if equivalent. | The current special collection records only String/Number/Boolean/Array member names and an inherits flag. It is not a full interface member table. Remove the duplicate special facts only once the replacement preserves its exact narrow known/unknown decisions. |
| Flow graph, read flow handle, function EndFlowNode | Binding must construct graph/attachment; semantic source exposes handles; evaluation interprets them. | None exist in current native binding. They cannot be claimed as retained by freezing Store. Extend the existing collector/owner with pinned binder operations and end-to-end read/return admission tests. |
| Class/interface/object member keys, static/instance side, accessors, parameter properties, late keys | Binding captures declaration/member recipes; member keys own exact identities; member tables complete typed keys. | Current member_name mostly emits NotReference; static/instance recipes and dual-role associations are absent. Extend that operation rather than adding a second AST scan with its own key/role rules. Do not infer computed names during load. |
| Linker State/journal/running/cuts/cut_tasks/export_memo/conflicts | Builder-only resolution and diagnostic work state, not mutable retained checker state. | Freeze completed answers required by consumers, then drop visitation states, cycles' task-stack scaffolding and merge invalidation journal. Export memo entries are optional derived caches, not the sole export owner. |

## Resolution classes and compatibility

The new record must answer two different operations: existing lexical reference
projection and the richer semantic attachment. Old NotReference is not a
universal semantic absence. Preserve its lexical result while adding a typed
pending obligation where the design calls for one.

| Existing result / case | Lexical projection retained | Semantic attachment / refusal boundary |
| --- | --- | --- |
| Decl / alias / Ref resolves Found(Meanings) | Canonical symbol targets in existing slot/dedup order. | Retain own declaration or alias identity, per-meaning target and type-only access; namespace/module paths are already bound. |
| GlobalThis or built-in undefined/implicit arguments | Existing NotReference reason. | A built-in semantic operation with source role; not an ordinary unresolved lexical spelling. |
| Property access through variable/class/function/unknown receiver; computed member | Existing NotReference property/member result. | A located receiver/key demand. Freeze a pending member obligation rather than a guessed symbol or no member. Computed-key expression references stay lexical uses independently. |
| Enum member, this parameter/type, private member, parameter-property name/modifier, shorthand | Existing legacy result. | Source role distinguishes enum/private/member/parameter/read questions. Required new identities/dual roles must be captured; existing NameId alone cannot encode all roles. |
| JSON ValueTarget or known apparent-interface member | Existing NotReference value reason. | Retain JSON/value/member identity; type operations consume its facts. A known direct property and an inherited/library-member type demand are distinct. |
| `value_property` direct missing key on known object/scalar | Existing absent/no_property or legacy property reduction, according to call site. | Preserve known absence for this narrow question. Do not broaden it to full receiver-type property absence. |
| `value_property` inherited interface member or property of a library member | Existing NotReference explanation. | Explicit deferred semantic obligation. Change its return from ambiguous Option/reason to known-present/known-absent/deferred only with fixtures pinning every existing caller. |
| Missing lexical name, no matching meaning, missing namespace export, circular alias/export, ambiguous star or shorthand export | Existing Unresolved text and UnsupportedTarget/UnresolvedTarget query behavior. | Retain the reason category/provenance; do not turn a load/query failure into any or a successful empty type. Demand evaluation may report compiler recovery only under its distinct documented contract. |
| RequestTarget Script/Ignored; ModuleResolution Missing/Unchecked/Unsupported/NotAdmitted | Existing diagnostics/provenance settlement. | Do not collapse the loader outcome into a tolerated semantic Unsupported. Store settled request outcome separately from raw input resolution where needed. |
| Collector Unsupported, I/O, syntax/resource limits, merge/alias conflicts, required unresolved import | Existing hard InputError and no published Project. | Freeze never starts a fallback parse or publishes a partial snapshot. Keep all original error kinds and reporting policy. |

Freeze cannot simply keep whichever State happens to be Done after
`resolve_and_index`: dependency references and otherwise-unused aliases may
still be Unvisited. The migration must explicitly enumerate which whole-program
lexical attachments are completed and which are typed pending obligations.
Forcing more current lexical references must not introduce new load diagnostics
or execute a type evaluator during load. An incomplete attachment is a recorded
obligation, not an implicitly absent symbol.

## Source-site identity and round-trip proof

Use one owner for generation-local site/role identity. Final-tree preorder plus
an explicit semantic role is a plausible representation, but the semantic
collector must define the eligible nodes and roles and own both capture and
reprojection. Do not use Ast.nodes allocation ordinals: module-await reparsing
leaves superseded nodes in that arena. Do not identify a site by span alone:
shared spans can carry distinct parameter/member or shorthand/read questions.
All spans come from `Ast::token_span` and logical child traversal from
`Ast::{walk,logical_fields,children}`. Exact names remain with existing text/key
owners. Public replay anchors are the selection owner's derived product, not
an exported internal ID.

Required behavior tests select/replay through final Project/Session operations
after arbitrary disk mutations to prove captured-source use. Cover declaration,
type/value alias, expression/call punctuation, distinct accessors, parameter
property, shorthand, computed key vs key-variable reference, literal element
access, quoted/escaped/numeric/empty/private names, merged interface/class,
overloads, qualified namespace/type-only imports, UMD/augmentations, JSON
duplicates, JSX member tag vs unsupported attribute, BOM/shebang/all line breaks,
and final-tree module-await reparse. Run each operation's supplied replay anchor
through that same operation's At question. Cross fresh/repeated/reversed request
order and successful/failed refresh; old Q1/Q2/E behavior must stay equal.

## Representation comparison that can close the gate

No semantic latency or memory measurements are supplied by this audit. Existing
`benches/navigation.rs`, `benches/references.rs`, and
`docs/navigation-performance.md` supply reusable measurement mechanics and an
example of a real completed comparison, not evidence about the new semantic
representation. Their corpus revisions differ from the types design baseline.

Implement the two private candidates behind the same semantic-source operations:

- **Eager:** compact owned declaration/type/expression/body descriptions captured
  during the initial final-tree pass. References contain existing site/scope/
  symbol/flow handles. No raw syntax arena, source substring or checker result
  is retained in each term.
- **Reprojection:** parse only the already captured SourceText using the shared
  analysis Parser and original file-name mode. Project complete descriptions
  with the same identity/role owner, attaching the frozen scope/symbol/flow
  handles. Cache file descriptions request-locally; a request touching 100 sites
  in one file must not parse that file 100 times. Transient syntax stays inside
  semantic source and is dropped before descriptions reach evaluators.

First instrument descriptor acquisition separately: capture/projection time,
visited sites, parsed files/bytes, peak requested heap, retained snapshot bytes
and request-local bytes. These are **semantic-source costs**, not end-to-end
type/member query latency. A token-count walk or public parser benchmark cannot
close the semantic latency condition. A compiler API trace can seed representative
source demands but is not proof of the native evaluator's actual demand graph.

For an end-to-end comparison, both candidates must feed the same bounded vertical
native evaluator and selection/results on the pinned manifest. Before a full
checker is built, implement enough exact native requests to exercise repeated
cross-file demands (annotation/declaration, factory/signature/contextual callback,
member root and controlled refusal). Pin those outcomes to checkpoint 1's oracle,
then measure their actual native semantic-source demand traces. If that consumer
does not exist yet, leave reprojection as the provisional starting choice and
the semantic-latency portion of checkpoint 2 explicitly open; do not choose
eager from description timings alone or declare the gate complete.

Use the unchanged Command Center revision
`92fe6ab06fbea2c4b6a305689f8cf336173eef81` and its recorded tsconfig/dependency/
library hashes, plus a small controlled fixture and a large-file/large-shallow-list
fixture. Regenerate a baseline only with an explicit manifest revision. Every
sample verifies expected outcome and touched-source identity before timing.
Alternate strategies on otherwise idle release builds and record machine,
compiler/tool revision, corpus/option hashes, sample count and dispersion.
Measure at least the following independently:

| Workload | Required measurements / safeguards |
| --- | --- |
| Cold full load with no semantic request | Wall time, peak and retained requested heap, optional process RSS; token/symbol/declaration counts are normalization metadata only. Include standard libraries/dependencies, not just searched files. |
| Warm unchanged service request | Freshness validation separately from query evaluation; first and repeated type/member answers on the same immutable generation with request-local caches reset each request. |
| Changed successful refresh | Full replacement load/peak/retained costs; snapshot generation advances and subsequent answers use the replacement. Restore controlled inputs between samples. |
| Failed refresh | Typed final failure, no earlier-generation answer; previous snapshot release and failure peak separately. Neither strategy publishes partial facts. |
| Native first/repeated/reversed semantic queries | End-to-end evaluation and descriptor acquisition separately; exact and refused controls; actual description-cache hit/miss and file parse counts. Include one large file and cross-file signatures/library hooks. |
| Old lexical/Q1/Q2/E operations | Load then lexical position/name groups, missing/ambiguous/unique show, outline and edit preview; expected documents/anchors unchanged. Include call/test filters and JSON/augmentation fixtures. |
| Request cleanup and limit exit | Peak during evaluation, request-local bytes after cleanup, no growth across repeated requests, typed resource/refusal exit followed by successful lexical request. |

Requested heap and RSS are different metrics. Allocation instrumentation must
not subtract deallocations from pre-existing snapshots into a counter reset to
zero during refresh. Use a lifetime-aware measured allocation epoch or keep one
counter active from creation through replacement; verify balanced teardown for
each candidate. Retained bytes must be sampled after the AST, Linker, temporary
maps and description request cache have dropped. Record vector capacity and
intern-table overhead, not only `size_of` or element counts. Wall time samples
should not run under another validation's competing load.

Prefer reprojection unless measured semantic latency makes eager terms necessary
and their additional load/refresh/retained cost is justified on the same corpus.
The design supplies no numeric crossover or acceptable service target yet;
freeze the comparison criteria with actual baseline distributions before
interpreting results. Avoid permanent candidate-selection flags or backend traits
in the shipped interface. After choosing, remove the other implementation and
retain the measurements and a regression benchmark for the observed risk.

## Implementation order and review points

1. Finish checkpoint 1's pinned manifest, result/rejection branches and option
   classification. A bounded move of existing successful binding facts and its
   unchanged lexical projection may proceed concurrently under the safeguards
   above. Keep sparse IDs and one canonical lookup owner initially; do not force
   unvisited dependency references or unused aliases. New semantic attachment
   roles, dense remapping, JSON structure and flow wait for their relevant
   compiler baselines. Full semantic answers and the complete migration gate
   remain dependent on checkpoint 1.
2. Write red final-boundary migration tests for all old projection categories
   above, including load errors and direct/service paths. Add the minimum internal
   scaffolding needed for tests to fail on behavior, then use the registered
   single integration-file TDD command.
3. Make the existing Linker return its completed binding product after all checks.
   Transfer collected facts after its Program borrow ends; do not clone all
   ProgramFile facts to satisfy a borrow. Keep sources/config/resolution decisions
   with Program and freeze only the finalized binding knowledge.
4. Build the old lexical Index as a projection of that retained product and remove
   duplicate token/site facts. Keep derived order/location caches only when useful;
   compare old-operation allocation/latency before removing existing caches.
5. Extend the existing collector for source roles, member recipes and flow handles;
   extend JSON's existing fact owner. Review whole compiler operations, not isolated
   branches. Prove round trips before relying on a frozen handle in evaluation.
6. Implement both private description candidates, run source-cost measurements,
   then native semantic workloads as available. Keep the representation decision
   provisional until both cost and exact-result comparisons exist. Repeat the old
   compatibility suite at each freeze/canonicalization/inventory boundary.

This artifact changes no production behavior, so no new test-first loop applies
to the audit itself. The configuration integration/private snapshot tests are
separate implementation evidence; passing them does not prove this migration,
representation costs, semantic source, flow graph or type results.
