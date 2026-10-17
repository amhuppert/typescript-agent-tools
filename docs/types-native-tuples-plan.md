# Native tuple owner plan

Status: initial `tests/type_tuples.rs` controls have genuine compiled RED
`vrun-1b466471-ed58-431e-a7a1-8ce965cc44bd` and tuple plus lazy-graph final GREEN
`vrun-d21b6bca-7e55-443e-8a0d-512458e59638`. Peer reviews checked native target
allocation, deferred resolution, syntax recipes and normalization scheduling.
The complete tuple consumer and boundary profile remains outstanding; these
checkpoint tests do not establish that full profile.

Native lazy graph controls in `tools/type-tuple-graph-demand-oracle.test.mjs`
passed `vrun-c4efb8f4-d1f7-4f23-82d9-f77d87c85160` (one test, nine fresh
strict/loose/exact questions, zero skips). Nongeneric and generic tuple aliases
have healthy alias summaries while requesting their actual argument collection
exposes the unresolved element. `tests/type_tuple_graph_demand.rs` pins the
product distinction between omitted depth-zero arguments and admitted depth-one
arguments. The final Project checkpoint above passed; counterfactual proof
remains pending. The graph
uses a nonforcing argument-count header and admits its byte lower bound before
the existing whole native argument operation; it does not evaluate tuple source
elements independently.

The native byte-boundary extension passed
`vrun-613cd3fb-9327-4807-a21c-4446380e0289` (two tests, 21 fresh questions,
zero skips). Large aliases retain their complete symbol name and healthy tuple
header; native `typeToString` truncates its display to 320 bytes and an admitted
argument collection returns the unresolved operand. The final Project test
now distinguishes 10/11 bytes available for an elided collection from 12/13
bytes admitting the member-inventory header and its argument dependency. These
byte assertions follow the closed public wire schema, not the native printer's
separate summary budget. Their first final verdict is pending.

The exact-optionality checkpoint exposed an omitted native display operation:
NodeBuilder `typeReferenceToTypeNode` removes Missing from optional arguments
before producing tuple element nodes, while retaining explicit Undefined and
the raw reference arguments. The complete shared tuple display operation is
staged with named, unnamed, readonly, required and explicit-Undefined controls.
Native `vrun-3382dfc7-80a4-4d43-a0e0-bf6367cb656e` passed all 18 fresh
strict/loose/exact cases with zero skips; final regression and added graph
assertions are pending. The graph continues to expose the actual unmodified
argument identities.

This uses the Ousterhout design skill. The envelope is one deterministic semantic
request over an immutable local Project. Compiler identities and lazy questions
remain request-local. The required outcome is the full optional/rest/variadic/
readonly tuple family in `types-design.md`, including dependencies acquired by
normalization, member resolution, signatures, relations and presentation.

## Chosen boundary

Extend the existing target/reference lifecycle. A separate tuple value with an
element list would be simpler to print but would duplicate reference identity,
mapper substitution, synthetic `this`, base resolution and property ownership.
Native `createTupleTargetType` instead builds a genuine nil-symbol target with
actual nil-symbol type parameters, synthetic properties, and an independent
synthetic `this` constrained to the target. Its instantiation table is the same
kind of target/argument cache as other references.

`evaluation/tuples.rs` owns `createTupleTypeEx`, `getTupleTargetType`,
`createTupleTargetType`, `createNormalizedTypeReference`, the complete
`TupleNormalizer` operation, and tuple element/slice observations. Model retains
the established target facts and reference identities. `source/tuples.rs`
supplies actual node recipes for `getTypeFromArrayOrTupleTypeNode`,
`getTupleElementFlags`, `getArrayElementTypeNode`, named/optional/rest nodes and
the readonly type operator. Array syntax goes through that same native node
operation; it does not get a second array factory.

Target data retains element flags, actual label declarations, readonly,
minimum/fixed lengths and combined flags. The target cache key uses exact flags,
readonly and actual label node identities; equal label spelling alone never
merges targets. Comparisons use native `compareTupleTypes`: readonly, arity,
flags, then label spelling. That is deliberately a different question from
cache identity. Labels retain native exact scalar text and separate display
spelling, never lossy identity conversion.

Model publishes `is_tuple_type` and a tuple-target observation. Native instances
have `REFERENCE` on the reference and `TUPLE` on its target; checking only a
reference's `object_flags(TUPLE)` is incorrect. All placeholder tuple gates must
be replaced according to the exact native predicate at their site. A Tuple view
contains its real reference header, ordered element demands, flags/labels,
readonly and the existing `MembersOfType` token. Its public graph is the already
defined `Tuple` record, not a new wire contract.

## Reference arguments are a required prerequisite

`getTypeFromArrayOrTupleTypeNode` (checker24024) does not always check children
immediately. Nonvariadic directly aliased nodes and appropriate circular alias
contexts use `createDeferredTypeReference`; an empty tuple returns its target.
Any variadic tuple element selects immediate checking and normalization.
The Source owner must retain exact alias ancestry and `mayResolveTypeAlias`
decisions; a syntax-wide eager child check would change healthy recursive roots
and consumed failures.

The current resolved-argument Reference representation must grow a real
deferred node/mapper/alias association and one `getTypeArguments` operation
(checker21800). That operation owns its native resolution stack entry,
provisional/circular recovery, child order, mapper application, cache and
original failure. Completion installs only if reentrant work has not already
installed arguments; circular recovery has the actual target arity. Reference
consumers ask it at their actual argument-demand
point. An immutable header observation must not force arguments. Model's
comparator remains pure over established facts: deferred references compare
actual node location then mapper even after arguments have been resolved;
ordinary references compare their established argument lists. Factories and
metadata must not replace deferred references with ordinary references merely
because their current printed arguments agree.

## Target allocation and shared consumers

`createTupleTargetType` allocates element parameters and numeric properties in
order, then the `length` property and its literal union, then the tuple target,
then synthetic `this`. Synthetic parameters reuse the nil-symbol parameter
factory; their native `?` printing fallback is not an identity or binding name.
The declared member table is installed as complete factory data; there are no
invented BoundSymbols or Source declarations. Root's property-symbol arena
needs the narrow actual synthetic property factory: exact name, optional,
readonly and known read type, with no declaration/parent and a fresh symbol
identity. Instantiation retains the existing native resolved-read fast path.
The raw-constraint owner must recognize these actual nil-symbol synthetic
parameters and their installed absent constraint; it cannot require the
BoundSymbol association used by source-declared parameters. Synthetic clones
retain their original native target and mapped constraint question.

The target association distinguishes Bound Class/Interface from Tuple so
`getBaseTypes` and member completion use their existing resolution/cache owner.
Tuple base construction is `Array<union of element parameters>` or its actual
ReadonlyArray counterpart. A variadic parameter contributes its real numeric
indexed-access type. `getNamedMembers` accepts the actual absent container
symbol for tuples and sorts the actual synthetic property headers; target
completion must not insist on a BoundSymbol.

Root's property/element/index owners consume tuple minimum/fixed/variable
facts for lookup, writes, undefined tails and optionality. Relations and
signatures consume the same target metadata for tuple variance, arity, rests,
parameter positions, spread calls and inference. They must not derive those
facts from serialized elements or duplicate normalization. Presentation emits
type-argument edges, tuple-element edges, then ordinary members under the
existing minimum-depth BFS and display budgets.

## Normalization and dependency closure

`TupleNormalizer` (checker23262) is one operation: optionality is added while
copying elements; variadic Any becomes Rest; generic instantiable and mapped
variadics remain generic; actual tuples expand; other inputs ask the real
array-like and numeric-index questions and retain consumed recovery. Optional
elements before the last required position become required. The region between
the first Rest and last optional/Rest becomes one Rest union, using actual
numeric indexed access for any intervening generic variadic.

Variadic Never/Union uses the native cross-product guard and map schedule.
The shared algebra owners retain failure and origin behavior. Tuple expansion
rejects `expanded count >= 10_000` at the native diagnostic site; this is
distinct from product graph/display budgets. The extra synthetic `this`
argument is excluded from normalization and appended afterwards.

One Rest target collapses to the actual Array/ReadonlyArray target before
ordinary tuple construction. Missing/wrong global recovery remains a consumed
dependency even when native returns a non-error empty object. Fixed tuples may
exist before their base/member operation demands that global; phase controls
must pin the exact difference.

Complete generic tuples therefore require the existing pending indexed-access
factory to become a shared owner with root's value-access operation. There will
be no tuple-only approximation of `T[number]`. The deferred IndexedAccess
representation, native flags/keys/mapper/constraints and concrete numeric
reduction must be implemented through that owner. If its dependency closure
requires another operator family, that remains explicit unfinished profile
work; a fixed-tuple-only checkpoint is not completion of the tuple task.

## Proof sequence and ownership

Existing `tools/type-native-algebra-oracle.test.mjs` already controls empty,
fixed, optional, labelled, readonly, Rest collapse and generic Variadic forms.
Its healthy root with an error child demonstrates why final-root error checks
are insufficient. Extend maintained controls before production changes for:

- strict/loose/exact optionality; escaped and same-spelling distinct labels;
  readonly target distinctions and tuple/array target collisions;
- nonvariadic deferred aliases, immediate variadics, recursive arguments,
  mapper/alias caching, unused versus consumed invalid children;
- optional-before-required normalization, tuple spreads, Any/Never/Union
  variadics, multiple rests and generic variadics between rests;
- actual numeric/length property read and write identities, base/index/global
  phases, missing/wrong-kind/wrong-arity globals and readonly fallback;
- 9,999/10,000/10,001 expansion, cross-product boundaries, and long shallow
  lists, with compiler-limit provenance distinct from product display limits;
- summaries, depth zero, bounded element expansion, member admission, alias
  metadata, repeated/cyclic graph identities and lower/at/over display budgets.

`tests/type_tuples.rs` must compile and fail at the current missing operation
before source behavior changes. Later normalization/reference/index additions
receive their own discriminating controls and final assertion REDs where the
initial tuple refusal otherwise masks them. The algebra lane owns Source tuple
recipes, Model/reference changes, tuple evaluator and presenter. Root owns
synthetic property arena and indexed-access integration; relations owns actual
tuple relation/signature/inference consumers. Publish interfaces before edits
and perform adversarial review before each coordinated CLOSED checkpoint.

Pinned source: TypeScript Go 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`, checker17456,19080–19126,
21800–21846,23144–23414,24024–24113,24617–24769,25029–25067, and utilities
`CompareTypes`, `compareTupleTypes`, `compareElementLabels`.

The exact optional tuple-summary and lazy graph continuation is now GREEN in
`vrun-da7c0838-867d-4061-bffa-9b9c3e7a60bb` (tuple/keyof controls) and
`vrun-2759d89a-4eb7-476d-bcc8-be8556665493` (presentation controls). The next
cohesive continuation owns tuple object inference, const-variable traversal,
mutable array/tuple conversion, and effective rest-call arguments. Its prior
whole operation bodies are retained under `.cc/temp/types-call-rest-before/`
for bounded counterfactual RED after the native-derived final tests pass; no
wholefile restoration may overwrite concurrent owners.
