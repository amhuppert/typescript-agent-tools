# Native finite type presentation

This bounded implementation work serves one request over one captured Project
generation. Semantic type IDs and type construction stay private to that
request. The public graph has document-local IDs and fixed display limits; it
does not cache identities across projects or introduce another source reader.
The full T/Q3/Q4 deliverable and the remaining baseline/representation gates
remain required.

## Ownership and private interface

The complete public vocabulary is [types.rs](../src/types.rs), with its graph
contract in [types-design.md](types-design.md#q4-graph-and-limits). The presenter
owns bounded summaries, traversal, public graph IDs and compact-JSON display
budgets. The model owns semantic identity, alias facts, ordered collections,
read/write distinctions and located demand failures. The presenter reads no AST,
binder table, literal-freshness bit or compiler option.

Two boundaries were considered. Having semantic construction emit public nodes
would couple document IDs and display depth to type identity and make a display
change touch the checker. Owned immutable semantic views let one presentation operation
make those decisions while preserving model order. The latter is the chosen
boundary, with fallible child demand rather than a permanent eager-child promise.

The agreed operations in `src/project/semantic/model.rs` are:

- `Model::get(TypeId) -> Result<TypeView, DemandError>` exposes established facts;
  describing a collection does not recursively demand every child ID.
- `Model::metadata(TypeId)` supplies real aliases and type arguments, currently
  absent for the first ordinary object/literal family.
- `MembersView` distinguishes known-empty call/construct/index collections from
  an unimplemented or blocked enumeration. Properties carry separate read and
  distinct write IDs, kinds, modifiers, exact keys and origins.
- `DemandError` preserves the whole operation error and an optional located
  unsupported issue. Source provenance supplies the complete location; a start
  position alone cannot supply an invented end. Invalid recovery, resource and
  internal failures never become partial graph leaves.
- `present(&mut Evaluator, root, &TypeRequest) -> Result<Presented, AnalysisError>` returns
  the description and located issues together. The request orchestrator sets
  `found` or `partial`; CLI text rendering consumes that same document.

The initial views admit primitive, literal and ordinary object facts because
those are the first compiler-backed native vertical fixtures. Other type kinds
are added behind this interface as their evaluator operations are proved. Their
absence remains an explicit semantic failure; this is not a smaller release
contract or evidence that the full language profile is implemented.

Current established views also admit intrinsic specials, normalized unions,
complete nongeneric call/construct collections and canonical type parameters.
The evaluator's `describe`, `metadata` and `demand` operations keep deferred
questions outside immutable views. A type-parameter summary prints its name
without asking for its constraint/default. Its graph admits the exact optional
field before asking that child question; depth zero preserves both as omitted
fields when present, while proved absence creates no omission.

The nongeneric signature journey was RED in
`vrun-99dd5c3c-f242-4d84-a887-b03eae2ca66f` and four tests passed in
`vrun-09608720-3a22-42ee-9ee1-2fed984b3c4d`. A required initialized parameter
then exposed a distinct native summary question: its graph read type remains
number while strict serialization can print number | undefined. This actual
regression was RED in `vrun-b4e69451-4b26-438c-bc51-99448c90ed0b`; all five
signature tests pass in `vrun-fe1e209d-fd98-4e9b-9b11-0e587b908813`, with zero
skips. The shared descriptor retains `value_type` and `display_type` demands.
Signature, index and type-argument previews reserve established metadata before
child demand, but broad collection/byte/node boundary proof remains required.

The generic signature increment is final-Project assertion RED in
`vrun-18fa4965-5dd4-4fdc-a8ac-78bb4e70b12a` (nine passed, four
failed, zero skips), then all thirteen tests pass in
`vrun-d152990f-faa8-4e12-8836-8bb4c442ae23`, with zero skips. The
presenter consumes canonical parameter descriptors, prints native Const before
declared variance, retains the first raw declaration spelling in summaries,
and keeps decoded exact names in public parameter records. Constraint/default
questions remain lazy; this proof does not establish generic inference or
reference-member instantiation.

The staged Array boundary reads only an established canonical target's actual
element and display-readonly fact. Its deferred whole member inventory is a
separate `MembersOfType(TypeId)` question, absent from a depth-zero graph.
Unknown collection slots are absent with field omissions, rather than known
empty slices. Above depth zero, a true serialized header lower bound admits
the inventory question; successful enumeration then admits exact counts before
any property/signature/index child record. Required element admission replaces
the already reserved omitted edge instead of treating the required field as
absent. An encountered unsupported inventory keeps its original issue and
`blockedCollections` explanation; display rollback can omit that explanation
without turning the result into found. Invalid/resource failures remain whole
errors. The positive Array Project gate passes all eight JSON tests in
`vrun-71fca5a4-34f8-4d73-9650-f13e2e2fec0d`, with zero skips,
against actual Array-instantiation RED
`vrun-c2270db8-1b8d-4ac1-b152-6bbfdf48664e`. Fresh strict/loose
Array roots and index child controls are retained in
`fixtures/types/presentation-array/`; their two-test compiler gate is GREEN
`vrun-b7d9a6e0-ebfa-43b3-be26-c2ff4c23ab2f`. Final Project depth-zero
avoidance, blocked whole inventory and consumed invalid index tests pass in
the 21-test presentation GREEN
`vrun-8456f5c2-8a3d-4571-8227-3af050225c0f`, with zero skips. An
initial expected-column correction supplies no production RED. The mechanical
removal of unconsumed Known-inventory and duplicate target-kind scaffolding
keeps the sole real members question; all 21 tests pass afterward in
`vrun-f3ead5ce-beb8-411f-8d25-0485550252ee`, with zero skips.
The next final Project Array boundary matrix passes all 24 tests in
`vrun-9fd766b4-5dc1-4d78-a35b-a2092c747297`, with zero skips.
Actual Array/union graphs cover 1999, 2000 and 2001 potential records at depths
one, two and three, preserving shared type-argument/element identity. Independent
compact-schema controls cover 262143, 262144 and 262145 bytes: the encountered
unsupported property retains its located issue and Partial result when its
display row and failed child record roll back. Oversized property metadata
avoids an unshown invalid child question. Fourteen fresh pinned compiler
sessions, exact generated sources and annotation spans are retained under
`fixtures/types/presentation-array/evidence/limit-observations.json` and
`limit-provenance.json`. These new tests passed on their first valid execution;
they are boundary evidence, not invented production REDs. Whole blocked-inventory
explanations have separate 262143/262144/262145-byte controls in the final
25-test GREEN `vrun-1cf7b368-18f3-4ed7-9d4a-57e37ff5b4c4`, with zero
skips. All four explanatory records fit under/at the limit; over it their
display fields are omitted while the original encountered issue, Partial
outcome and previously admitted shared literal child remain. Six additional
fresh compiler sessions and 24 exact generated inputs are retained in
`blocked-limit-observations.json` and `blocked-limit-provenance.json`. An initial
unrelated effects compile-staging failure was a setup failure, not a boundary
RED. Ordinary reference graph kind is
explicitly pending the native callable-shape query, rather than being guessed
from class/interface flags.

Canonical parameter public tests were RED in
`vrun-97bd635e-b9dc-4458-9fa0-6a23bf85bfa7` and all three pass in
`vrun-136a2036-da03-47fa-ad85-a3a4f91a684b`, with zero skips. These prove the
admitted ordinary/merged first-nonnil children, lazy invalid-child refusal and
genuine-any constraint normalization. General constrained parameter chains,
mapped/inferred parameters, generic references and Array member collections
remain required semantic work.

## Traversal, limits and errors

One queue discovers graph nodes breadth first. A map keyed by private `TypeId`
assigns `t0`, `t1`, and so on in discovery order and retains the minimum root
distance. Repeated identities reuse IDs; declaration equality does not merge
different instantiations. Edges retain the design order: type arguments,
array/tuple/alternative/operator edges, parameter edges, property read/write,
call/construct signatures, then indexes. Alias origins are metadata, not edges.

The operation validates depth 0–32 before selection or semantic work through
the single `TypeRequest::validate` owner. CLI syntax parsing calls that same
policy. At depth zero a primitive remains complete, while a structural node
retains its real kind and reports only fields actually elided. An empty known
collection is not an omission.

Display defaults are 2,000 nodes, 262,144 UTF-8 bytes for compact serialization of
the complete `type` field, and 512 UTF-8 bytes per summary including markers.
Admission reserves envelope and omission space before adding fields. Stable
prefixes of established lists carry exact omitted counts; unknown enumeration
cannot invent a count. Identity strings are retained whole. A root whose exact
metadata cannot fit fails with `OutputLimit`.

Summaries elide whole tokens or clauses and include any marker in their byte
count. Graph summaries use aliases or bounded child descriptions rather than
recursively inlining every reachable type. Display omission can avoid a demand
outside the view, but any already encountered semantic failure keeps its
classification. Summary truncation and graph truncation are display facts;
unsupported child/collection issues independently make the answer partial.

## Verification sequence

Behavior tests use final `Project::type_of` results in
`tests/type_presentation.rs`, with a registered single-file red/green run for
each increment. Initial compiler-backed facts come from
`fixtures/types/native-vertical` and its independent four-test oracle gate
`vrun-77bcd0cc-1282-441f-9c27-7cf86930f982`. The first native vertical evaluator
test also uses final Project results, owned separately in `tests/type_vertical.rs`.

The sequence covers direct depth-policy rejection before selection; admitted
primitive/literal/object descriptions; compiler property order and origins;
shared IDs at unequal depths; depth/node/byte/summary boundaries in their exact
units; long exact names and shallow wide lists; then recursive/growing types,
read/write/signatures/predicates and supported versus blocked child collections
as the corresponding model operations become available. Full-schema, cache,
causality and corpus gates remain open until that evidence exists.

Pure view/failure-record scaffolding was wired before those future semantic
families exist. It is not a behavior or completeness claim. The first depth
test's two initial runs hit adjacent in-flight compiler errors and are not valid
behavior RED. Its desired missing-validation RED is
`vrun-bf5c4fb8-5fe0-4d6e-8b05-0a220a6775a3`; the sole depth-policy owner then passed
the final public test in `vrun-4e70aacd-5236-43c3-b772-1c95e361a3f2` (one executed,
zero skipped). Valid semantic roots at depths 0/31/32 are the next presentation
controls, rather than evidence inferred from the validator alone.

The first scalar/object gate is
`vrun-3b9c9051-5a8d-4ba0-ac24-26682857740c` (three final Project tests), after
desired presenter RED `vrun-4922f871-aeae-4242-b44c-661e6a0e33d3`. The earlier
direct-name attempt reached an unimplemented selector rather than the presenter;
the corrected tests use the proved imported-alias position route.

Two summary regressions reproduced graph text elision being absent from the
expansion flag and active-path leakage into a later shared scalar in
`vrun-037e5b66-3858-458a-b096-bf86d1d73f7e`. The five-test GREEN is
`vrun-6cf93e70-c48d-409f-91c1-cb34fb93ebe6`. The summary nesting policy is separate
from expansion depth, and a bounded path exits before mutating the active set.
Fresh pinned 7.0.2 API controls confirmed exact string values without recovery
for ASCII and multibyte payloads whose quoted forms are 511, 512 and 513 UTF-8
bytes. A 31-object nested initializer's deep leaf and later sibling are both
the same native `number` identity; summary elision does not change that fact.

Byte-prefix behavior is independently RED in
`vrun-8be87833-a0f6-4e01-96da-acb1c47ac072`: five passed and two failed with the
old final `OutputLimit` safeguard. The tests cover 2,000 ordered properties all
sharing `number`, and an exact-key family whose complete compact type document
is 262,143/262,144/262,145 bytes. The incremental implementation reserves the
root envelope, every queued node's exact metadata and all collection omission
records. Each member's local serialized size adjusts those reservations, so a
wide list does not repeatedly serialize its existing prefix. The final GREEN
is `vrun-bf321797-552d-41cd-822b-1adfe06797ed` (seven executed, zero skipped).
The exact under/at documents retain the whole key; over-budget and wide objects
emit a stable prefix with an exact omitted count. Semantic controls and raw
source hashes are retained in `fixtures/types/presentation`. The large native
key control is separate from the product document-byte boundary.

The root's adversarial read found no blocking issue for the ordinary-property
path: queued-child rollback restores IDs, queue and byte accounting together,
and first breadth-first discovery preserves minimum distance. Future
type-argument/signature/index families still require metadata previews before
avoidable child demand, checked size conversions and final behavior tests.
The exact final serializer check stays as a safeguard. Full graph kinds,
node-boundary, lazy-failure and resource/corpus gates remain required.

The exact final-clause summary regression is RED in
`vrun-1b5f5582-a6bf-4dc7-91d4-bedbb8fa1d62`: a complete 511-byte object clause
was omitted prematurely. The final clause now reserves its closing brace;
earlier clauses reserve the marker for the actual remaining count. Final GREEN
`vrun-525a3e6a-880d-44a1-b2e7-aa0be19a38ca` passes nine Project tests with zero
skips, including whole clauses at 511/512 bytes, elision at 513 and shared-node
minimum distance in breadth-first order. The checked signed size conversion is
pure safety scaffolding rather than a separately reachable behavior claim.

## Deferred child boundary

An established `TypeId` identifies a healthy semantic root. A child slot is a
`TypeDemand`, either a known type or a separate `DemandId` for one exact semantic
question. A semantic `MemberId` and a demand identity are distinct: a member's
read and write questions need not have the same answer, and not every deferred
child is a member. The evaluator owns recipes, canonical member/question
associations and resolved/failed caches. Source owns captured syntax recipes;
presentation receives no AST, binding or cache representation.

Immutable owned views share literal payloads and established collections through
`Arc`. `describe`, `metadata` and `demand` remain evaluator operations. Enumerating
an object's descriptors does not check each annotation. Presentation first tests
the member's exact identity metadata against its remaining record budget, then
demands an admitted child. An unresolved handle uses the actual minimum encoded
reference size for that preview, since it might resolve to an existing shared
type. Success deduplicates by established type identity; a failed child uses its
exact demand identity and never receives a fabricated type identity.

An admitted unsupported child retains its original located issue and emits an
unsupported graph node when that node fits. Encountered issues survive graph
rollback, keeping partial semantics distinct from display elision. Invalid,
resource, parser, I/O and internal failures stay whole-operation errors. Cached
failures preserve their original cause; a later sibling does not manufacture a
different circular failure from an abandoned resolving state. Named-root summary
and depth-zero expansion avoid unshown member questions. Eager inferred object
and JSON module construction still check their required initializer dependencies
before establishing the root.

The desired public lazy RED is
`vrun-215b9d2a-45a2-44e6-994e-bad1d4ec1646`: all three final Project tests failed
on the existing eager primitive-only structured-root admission, with no setup
failure. The owned-view and known-handle conversion is mechanical wiring; the
deferred member and partial-leaf behavior requires the final registered lazy
gate. Final lazy GREEN `vrun-b0c257f8-96ce-475d-bb26-e9fbfe1b3946` passes four
Project tests with zero skips: named summaries/depth-zero roots, admitted
unsupported child details, whole invalid unresolved recovery and original-cause
retention across repeated alias demands. Compatibility GREEN
`vrun-9880c96e-dabe-4a66-a998-4eedf2886c61` passes the 13-test presentation file
with zero skips, including valid shallow unions at 1,999/2,000/2,001 potential
graph nodes and a node omission only on the over-budget edge. The frozen native
controls confirm every semantic alternative remains a valid regular literal.
Unsupported records at node/byte boundaries, recursive demand state,
signature/index/type-argument metadata admission and complete collection
failures remain explicit obligations beyond the first named-member family.

Public issue ordering is independently RED in
`vrun-651d758c-e337-4aa9-92b3-eef33ca3251c`: 13 tests pass and a reverse-ordered
alias demand reports line two before line one. `Issues::finish` now orders the
existing exact `Location` values and then the vocabulary's serialized reason,
without changing semantic or graph enumeration order. GREEN
`vrun-7a84d7cb-955d-4284-b4c2-ceeeb094fc6b` passes all 14 tests with zero skips,
including same-file and cross-file reverse-order variants. A mechanical green
refactor stores the complete immutable member view once rather than allocating
three empty Arc collections on each description; its compatibility GREEN is
`vrun-1617601a-d385-4469-89fa-2354bdf3705e`.

Eight fresh pinned controls, retained in
`fixtures/types/presentation/lazy-budget-observations.json`, confirm healthy
named roots and exact identifier keys in strict and non-strict profiles. The
three conditional members reduce natively to number; the metadata-excluded
member's explicit native demand instead produces an unresolved error type.
Closed-schema complete documents are 262,143/262,144/262,145 bytes. Final
Project coverage GREEN `vrun-5406cf97-5faf-4203-8197-145c50543270` passes all 16
tests with zero skips: under/at display the unsupported leaf and remain partial;
over rolls back the staged node and property while retaining the exact original
issue and partial outcome. An oversized member identity is rejected before its
unresolved annotation is demanded, so that named root remains found with an
exact display omission count. These coverage tests passed their first run;
there is no invented missing-behavior RED. Encountered unsupported failures at
the node cap have their separate under/at/over controls below.

Three fresh pinned node-boundary controls are retained in
`fixtures/types/presentation/lazy-node-budget-observations.json`. Their named
Budget root contains an inline union followed by a named Late root; breadth-
first expansion can reserve the union alternatives before demanding Late's
conditional member. The compiler confirms 1,995/1,996/1,997 valid alternatives,
healthy Budget/union/Late/number types and the exact member annotation span
`cases.ts:2:34–73`, with all four pre-demand input-diagnostic families empty.
The corresponding product graph has 1,999/2,000/2,001 potential records when
the unsupported child is included. The unchanged historical probe is archived
under `captured-tooling`; reproduce it at its original
`.cc/temp/types-lazy-node-budget/probe.mjs` path with the two pinned compiler
environment variables. Final public Project coverage passes all 17 tests with
zero skips in `vrun-ebeb92d0-e553-4991-bdd8-84f6163ed542`. Under and at the
boundary, the unsupported child has an exact graph record; over the boundary,
the graph stays at 2,000 allocated records and the edge reports a node display
omission with unsupported kind. In all three cases the original exact issue
and partial outcome survive, and every union alternative remains displayed.
The earlier attempt `vrun-4bb96409-c8a7-48ec-9c9d-9ac42c2a0f84` hit a
concurrent signature-stage compile error and supplied no behavioral verdict.
The final coverage passes on first valid execution; no missing-behavior RED is
invented. Signature/index/type-argument limits, recursive state and the full
semantic/resource matrix remain required.

The current 29-test presentation checkpoint passes with zero skips in
`vrun-88afc44f-66b0-4993-90f6-10de14f17017`. Ordinary reference graph kind
comes from the actual complete native inventory, including its consumed index
dependencies at depth zero; array kind instead comes from established global
target identity and can defer inventory. Nested unsupported inventory failures
remain exact separate question identities. The blocked-array fixture uses a
site-sensitive index query to consume excluded flow admission after the Array
root is established; native controls prove the actual array/element/argument
types independently. Exact explanation byte controls at 262,143/262,144/262,145
retain Partial and original issue on rollback. Two older test assumptions about
merged inventory refusal were replaced after that inventory became supported;
fixture correction is not a new product RED. Callable reference assertions now
consume the independently proved native signature mapper. Broad graph/resource
and repeated failed-reference preview boundary proof remain required.
