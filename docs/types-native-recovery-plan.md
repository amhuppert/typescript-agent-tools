# Native recovery continuation

This design covers the request-local evaluator. Q3 candidates share native
caches within one request; Q4 presentation admits lazy questions separately.
There is no persistent cache or cross-request recovery state. TypeScript 7.0.2
defines native values and acquisition order; product admission still rejects
consumed invalid recovery. Ordinary diagnostics and genuine Any remain valid.

## Observed gaps

`TypeState::Recovered(DemandError)` records native installation but discards the
installed value. Unknown-signature returns contain errorType; false-pop
signature returns contain anyType. These are different native identities.
`DemandError` cannot reconstruct either from its reason or message.

The mixed arithmetic owner retains errorType during its own setter/constant
continuation, then returns only Err. `checkBinaryExpression` consequently stops
at its left operand and omits the right; native checker12304–12305 checks both.
`checkConditionalExpression` similarly omits its false arm after recovered
true-arm evaluation (checker10908–10915). A comma or typeof can have a healthy
native result while still having consumed an invalid dependency.

`synthetic_signature_return` stops its composite loop on the first recovered
return. Native checker19918 maps every child, reduces the resulting types, and
then instantiates. The current unknown-signature false-identity shortcut proves
only that exact installed singleton's comparison; it does not complete derived
returns or authorize ignoring their outstanding children.

`type_names_for_error_display` stops after failed left display. Native
getTypeNamesForErrorDisplay still acquires the right display and may compare
their actual bytes before fully qualified retries. `report_property_relation`
and `report_relation_overflow` have the same ordered argument issue. A missing
native value or rendered string must not be replaced by invented output.

## Whole-operation stop defects

These are source-level findings; new counterexample fixtures remain to be
measured. Existing native frames must be popped before returning, but that
cleanup does not authorize further semantic reads after product interruption.

* `symbol_aliases.rs::resolve_alias_target`: after false pop, diagnostic symbol
  display runs even when the local result is Resource, I/O or Internal. Only
  afterward does append_failure choose the earlier whole failure.
* `node_types.rs::alias_reference_value_type`: the corresponding false-pop
  report_symbol_circularity has the same eager continuation.
* `members.rs::accessor_read_value` and `accessor_write_value`: false-pop
  branches replace the local result with circular recovery, including an
  earlier whole failure or an earlier semantic cause.
* `declared.rs::declared_type`, alias branch: false pop discards the result of
  acquiring the alias body and installs a new circular failure.
* `generics.rs::base_constructor_type`: installed reentrant Resolved/Recovered
  state is returned before propagating the local result. Reentrant success can
  therefore hide a consumed failure.

A repeated resolution key marks every participant false. If a nested
participant's diagnostic or later child then interrupts, every outer false pop
must perform only required frame cleanup and return that whole failure. The
mapped-property owner already guards the original result and each subsequent
display argument in this order.

Relation overflow has an additional explicit installation requirement:
check_relation_session acquires a key and installs the native failed-overflow
entry before its diagnostic. Preserve this for semantic recovery. If the
earlier result is a whole product interruption, do not treat required native
work as unconditional cleanup: acquiring a new semantic key may itself perform
work. Resolve this boundary explicitly rather than allowing append_failure to
run after the forbidden demand.

## Alternatives

An ambient recovery journal could let internal operations keep returning
ordinary values while recording failures. Its initial signature diff is small,
but it introduces hidden rules at every nested cache boundary: replay evidence
exactly once, separate candidate scopes, retain source-free observations, and
keep semantic evidence outside diagnostic rollback. A global TypeId poison map
is invalid: Any/error singletons and transformed results do not own the source
operation that consumed recovery.

Prefer an explicit private `NativeCompletion<T>` containing the actual native
value and ordered, unprojected evidence. Existing operation caches store that
completion; they remain the only owners of their native slots. Outer Err means
the operation was interrupted without an established completion, or a whole
failure stopped execution. Only the actual native producer can construct a
recovery completion. Do not add Err-to-Any, reason matching, or caller-supplied
fallback values.

The installed-slot fact and execution outcome remain distinct. A native value
published before diagnostics does not disappear because later display failed;
a failed calculation that never published a value cannot become Recovered just
because the resolution pop was false. Sequencing must preserve local failures
even when reentry installed a healthy value.

Overload recovery has an early installation inside `resolveCall`, before its
diagnostic continuation. Callee/new/tag preparation therefore hands an owned
completion containing its actual candidates to that owner. `resolveCall`
consumes this prefix once, completes candidate selection and the recovery
factory, then copies that completion into the resolved-signature slot. Reporting
extends only the returned copy. New-expression call-signature fallback work
runs afterward and is likewise outside the early slot. Passing the completed
preparation avoids both an ambient recovery journal and losing callee evidence
when the nested call owner publishes before its parent returns.

The shared completion owner should encapsulate ordered sequencing, whole stops
and final admission. Native algorithms retain their own short circuits and
choose their own actual recovery values. This general boundary replaces local
failure accumulators; it is not a list of helpers each caller must remember.

## Provenance and observation

Keep the established unknownSignature contract: one singleton with no source
declaration/location; raw and derived cache entries retain source-free recovery;
actual observations bind a returned copy. Ordinary declaration/import failures
keep their original locations. The installed singleton-only identity question
must remain narrower than consuming an erroneous return value.

The original `DemandError::append_failure` projected away the secondary
unbound-recovery marker. Implementation review chose a smaller coherent repair
than a parallel raw-event vector: SemanticCause now retains private, wire-skipped
native kind and unbound marker, while SemanticError owns dependency append,
observation and a bounded classification aggregate for retained and omitted
causes. These fields survive internal AnalysisError round-trips, including Q4
summary's existing path. NativeCompletion can consequently carry the actual
value plus Option<DemandError> without another aggregation protocol.

Locate every retained unbound cause independently on an observation copy. A
located primary cannot suppress that step. The aggregate admission decision
also forbids an Unsupported leaf when any later cause is Invalid, including an
omitted cause. Deserialized causal rows lack private classification and are
conservatively ineligible. Replaying a cache must not mutate or append to its
stored evidence. Distinct child reads remain distinct events even if they read
the same unknown singleton. The wire schema and public cause cap remain unchanged.

Diagnostic-chain rollback must not roll back acquired semantic evidence.
Likewise relation false remains healthy native data; a missing implementation
does not become false. Display equality/qualification uses actual rendered
bytes; when no native string can be established, preserve that interruption.

## Migration and proof gates

First introduce the completion/evidence owner and fix whole-stop unwinding.
Then migrate signature recovery/composition and expression/operator recovery
through complete dependent call paths, followed by diagnostic display and
relations. Extend remaining explicit producers (unresolved type lookup,
circular member/alias results, global factory recovery) through their existing
owners. Source/binding acquisition failures stay failures unless an exact
native evaluator operation establishes a recovery value.

Required new pinned/final controls:

* `(1 - 1n, later.value)` with a deferred MissingLater property, both operand
  orders and a healthy right side. Genuine Any from `1 + 1n` distinguishes
  ordinary diagnosed output from consumed errorType.
* Conditional expressions with a recovered first arm and failed second arm,
  including literal conditions and reversed arm order.
* The existing Function-or-callback union with a later missing callback return;
  warm/cold/reordered calls must acquire all native children and locate replay
  at each actual observation.
* Two branded arithmetic operands with different missing display properties,
  followed by a failing compound setter: left display, right display, operator
  recovery and setter provenance retain native order.
* Final Q4 and Q3 requests with existing injected Limits and separate lazily
  captured dependency files. Measure just-under/at/over interruption points,
  assert no later acquisition after whole failure, and verify a subsequent
  independent request remains healthy. Bounded internal false-pop state tests
  can supplement, but cannot replace, final query evidence.

Current operator-recovery and unknown-signature final expectations remain
unchanged. New hypotheses above are not claimed as measured compiler evidence.

## Mapped false-pop installation follow-up

The stage16 read-only review found a second cache-state distinction in
`property_symbols/mapped.rs`. Its early whole-operation stop avoids both
diagnostic displays, but final failure publication still chooses `Recovered`
solely when the resolution frame pops false. That does not prove native
recovery was installed: interruption may have occurred before the recovery
slot assignment. The next cache repair must retain the actual preexisting
installed state separately and publish `Failed` when no native value was
installed. A false pop alone must never manufacture `has_type`. This is queued
for the coordinated next source window; no current proof claim covers it.

## Stage-16 unwind checkpoint

Whole-stop guards are staged in alias targets, alias values, variable/property
and accessor frames, declared aliases, base constructors and signature returns.
The accepted frame is popped before the guard. A bounded independent read found
no new issue in those control paths; final interruption proof is still required.

The mapped owner already stops later display reads after a whole failure, but
its final `!resolved` branch still infers Recovered from the false pop even when
no native property was installed. Keep native installation separate from the
current invocation's result in the completion migration; a successful earlier
installation also must not be erased by a later diagnostic interruption.

The new native-recovery-continuation fixture captures comma/conditional/typeof
continuation, both operand orders, genuine Any, both diagnostic displays and a
composite signature with a later missing return. Its maintained compiler expectations pass in all three profiles
(`vrun-1eed5cde-8a47-4f85-9f74-f464b8edddf0`). No expected result is inferred
from the current Rust implementation. Source-sorted compiler diagnostics do not
prove acquisition order; final assertions follow the native control flow.

## Stage-17 evidence owner

The typed causal metadata and aggregate admission repair is staged in the
existing error owner. Targeted tests cover secondary singleton markers under a
located primary, repeated observation after AnalysisError conversion, nested
omissions, 15/16/17 output-prefix boundaries, invalid causes beyond that prefix,
conservative deserialized evidence, and whole-failure precedence. The library
tests pass in `vrun-6e50f764-31b5-46d9-84a2-b221c40fe121`; NativeCompletion
and the first final continuation group are implemented and measured below.


## Stage-17 first completion path

`model/completion.rs` owns actual value plus optional typed evidence, ordered
sequencing, interruption precedence, observation copies and final admission.
Expression checking now preserves this value through its signature-instantiation
post-step. Binary and conditional operands sequence completed values; typeof
and unary consumption also retain their evidence. Mixed arithmetic establishes
its actual native errorType and exports that completion only when its native
steps completed. Existing diagnostic/write owners still return interruptions
when they cannot establish their results. Their retained independent dependency
collection does not authorize a completion or a fabricated display string.

This is a transitional migration: the existing TypeId-only caller boundaries
still admit early, and cache/producer migration remains required. The first
Project test group exercises forward arithmetic recovery and healthy controls;
a separate set of required groups exercises reversed member recovery, both
displays, and composite returns. The build compiles in
`vrun-bac55c0e-dc2b-4ff1-9a88-96af9efcb283` (19 preparatory warnings).
Fourteen forward/control cases pass across all profiles and both presentation
modes, as do the 15/16/17 final-query causal-prefix controls. Reversed member
and composite-return groups fail because later recovery evidence is absent.
The display group first exposes a missing BigInt global in the fixture; that
dependency is now supplied and its unchanged native baseline passes
`vrun-20b06791-03c8-45c5-bf72-6ed46e024761`. The final display rerun reaches
MissingLeftDisplay but omits MissingRightDisplay (`vrun-b6d6aaa2-7a4d-4d36-8b58-dff6f4f936ac`).
A bounded adversarial review of the completion owner and expression/operator
entry found no concrete issue; this does not substitute for final query proof.

The stage-17 mapped slot repair now publishes Failed only if no native Type
property exists. It preserves an already installed property across later
failure, and no longer infers installation from the false pop alone. This is
staged; the injected-interruption final query proof remains required.

## Next producer migration: native value inventory

The pinned `internal/checker/checker.go` establishes the values below. These
are source observations for the next migration, not a claim of Rust parity.
No consumer may infer the value by matching a product failure reason.

| Native owner | Actual recovery and installation |
| --- | --- |
| `getReturnTypeOfSignature` (19906) | Rejected push returns errorType without installation. False pop selects anyType; final nil check preserves a reentrant installed return. Composite maps all child returns before union/intersection reduction and mapping. |
| `getTypeOfAccessors` (18424) | Rejected push returns errorType. Every false pop selects anyType, including annotated accessors. Symbol display precedes that choice at its diagnostic gate; final nil check preserves an existing slot. |
| `getWriteTypeOfAccessors` (18479) | Rejected push returns errorType; false pop chooses anyType; absent write annotation falls back to read only after pop and only if WriteType remains nil. |
| `getTypeOfAlias` (18511) | Target and immediate-export questions precede slot installation. A value target supplies its actual type; otherwise errorType is installed. False-pop circularity diagnostics do not replace an already installed value. |
| `getDeclaredTypeOfTypeAlias` (23745) | Rejected push returns errorType. False pop consumes symbol display then chooses errorType; a final nil check preserves reentrant installation. |
| `getTypeOfMappedSymbol` (20889) | Rejected push marks containing mapped type `containsError` and returns errorType without installing the property. False pop installs errorType only if absent, before both diagnostic displays. |
| `getUnresolvedSymbolForEntityName` (23010) | A nonempty entity creates/caches an actual synthetic TypeAlias with Unresolved check flag, exact name, recursive Parent and `unresolvedType` declared value. Its path-keyed identity and alias-bearing type must survive later display and relation work. Empty name uses unknownSymbol. |

Missing type references therefore need their native synthetic symbol owner and
alias factory. They cannot be completed by substituting the singleton errorType
at the binding failure boundary. Native declaration/type-node caches and the
value/read/access consumers must carry the completed type and its evidence
through the same operations they already use. The first step after the staged
integration is migrating installed TypeState values and signature composition;
unresolved type lookup then requires the synthetic alias path above.

The extended capture adds unary minus, logical not, delete, and deferred void
(`vrun-7cf61cd0-e02b-443d-b115-30b09d865bfd`, all three profiles). The first
three check their numeric-recovery operand before returning healthy native
number/boolean; void schedules deferred checking and immediately returns
undefined without acquiring that error. Maintained assertions for all 20 roots
pass in the receipt below. The separate 15/16/17 causal-prefix controls also observe ordinary
comma-unused-value diagnostics (2695), which do not themselves mark invalidity.
Their initial diagnostic-list assumption failed in
`vrun-22964b5b-6159-42dc-af1a-5a7100104f69`; the maintained assertion now includes
the exact observed 2365/2695 sequence. Rust prefix tests pass in
`vrun-bac55c0e-dc2b-4ff1-9a88-96af9efcb283`.

Maintained native proof is now GREEN:
`vrun-9b5ffa23-9bff-4318-baa6-f2671752e5ff` executes all four harness tests with
zero skips: 20 recovery roots across three option profiles, plus the three
causal-prefix boundary roots. The ordinary comma diagnostics are asserted
alongside each mixed-numeric diagnostic; no diagnostic alone is treated as
recovery. The remaining member/display/signature producers still require final
Rust verification; the arithmetic and causal-prefix paths are measured above.

## Diagnostic display migration review

This next slice assumes the existing request-local checker and immutable
Project. It requires completed type acquisitions from the shared TypeState
migration; it introduces no journal, second renderer, or diagnostic cache.

`printer.go::typeToStringEx` produces text after lazy checker recovery. The
node builder therefore needs `NativeResult<Node>`, with its existing ordered
children composed through `NativeCompletion`. The pure byte printer maps the
completed node and retains its evidence. A display-local recovery journal would
save return-type edits but hide the same replay and rollback rules already
rejected for the evaluator. Keeping completion explicit uses the existing
owner and makes a missing native result remain an interruption.

The outer display entry must restore serializationLevel, scopes, mapper,
visited/depth state and flags before returning either outcome. Level two still
returns the actual `?` without entering a new builder; native truncation still
skips its exact children. A completed recovered node is printed and truncated
with the same byte rules as a clean node. Unsupported work, unavailable native
values, and whole failures remain outer Err; they authorize neither invented
text nor subsequent checker calls. Source-free evidence is observed on the
returned copy, never installed back into raw type/signature caches.

`relater.go::getTypeNamesForErrorDisplay` must sequence source context, source
default display, target context, target default display, then compare the two
actual byte strings. Equal strings require fresh source and target displays
with FULLY_QUALIFIED alone and nil enclosing declarations. Recovery evidence
does not skip comparison or either retry. Every native acquisition contributes
ordered evidence, while a rollback of the diagnostic message chain cannot
undo that evidence. Migrate the corresponding property, return-type and
overflow reporting argument loops through the same completion owner. Overflow
keeps its existing primary failure and native cache-installation order; a later
whole interruption still wins.

### One property acquisition across name and mode selection

The current `diagnostic_property` calls `non_missing_property_type`, discards
the type, renders the name, then calls `serialize_property`, which acquires
the raw type again. Q4 has the same split: `summary_property_name` reads the
type before rendering the name, then `property_parts` calls
`serialize_property`. Native `nodebuilderimpl.go::addPropertyToElementList`
(2459 onward) retains one `propertyType` across those steps. Wrapping the
existing Rust results in completions would manufacture a second evidence
event at these implementation-only reads.

Consolidate that native property preparation into one operation producing the
established name and PropertyDisplay choice plus evidence. It owns the native
placeholder decision, the single non-missing read, temporary enclosing
declaration for name rendering, name-length contribution, then accessor or
method selection. Diagnostic traversal consumes this preparation and proceeds
to the selected child nodes. Q4 retains its nonforcing preview and row-budget
gate; after admission its wrapper consumes the same preparation once, then
uses its existing child admission and issue policy. Standalone property-mode
callers, if retained, enter that same owner rather than reconstructing the
prelude. Do not cache this preparation independently of native slots or put
evidence on the shared TypeId.

The name prefix includes native's late-name tracking prelude: save the
enclosing declaration, clear it, then run `hasLateBindableName` and
`trackComputedName` with the saved scope (or the declaration-less reporting
and SymbolToString path). Only afterward install ValueDeclaration, first
declaration, or the saved enclosing scope for the property name. Restore the
scope on interruption as well as completion. Generated-name producers remain
a required prerequisite; ordinary fixed-name completion does not authorize
skipping that tracking question for a late name.

The completion path also activates a previously unreachable native accessor
branch: acquire writeType even if the read type recovered, then expand getter
and setter signatures only when both types pass `!isErrorType`. If either is
an error type, continue as an ordinary property using the already acquired
read type. The current `serialize_property` lacks this guard because an Err
previously stopped before it. The existing Model `is_error_type` owner covers
the singleton and Any with an actual alias; genuine Any must retain its
different accessor behavior.

### Producer prerequisite and final proof

Missing type names are not the singleton errorType. Native
`getUnresolvedSymbolForEntityName` (checker23010) constructs a path-cached
synthetic TypeAlias with Unresolved check flag, exact Parent/name and
unresolvedType declaration. `getTypeFromTypeAliasReference` (23488) then
acquires every type argument in order and caches a distinct alias-bearing Any
by the actual alias key. This producer must land through the shared symbol,
alias and type-node owners before the display migration can complete the
Missing-property proofs. A generic binding-error conversion cannot supply it.

The maintained `bothDisplays` and `bothDisplaysAndSetter` controls are the
first final-query gates: retain left display failure, right display failure,
arithmetic errorType, then setter failure in native acquisition order, with
the existing strict/loose/exact expectations and compact/expanded requests.
The fixture must establish BigInt as required by the arithmetic relation; a
missing global must not substitute for the intended display dependency.

Add measured controls for multiple failing properties within one display and
for `{ get p(): MissingRead; set p(value: MissingWrite); q: MissingTail }` in a
normal checked equality against a primitive. The latter must establish the
write acquisition before the error-type gate and subsequent plain-property
and next-property continuation; healthy and genuine-Any accessor variants
distinguish it from unconditional suppression. These new cases are proposed,
not yet compiler-measured. A separate equal-byte/truncated-name control must
prove both qualified retries with a zero-demand observation of the existing
display returns/flags; source-sorted diagnostics alone cannot prove retries.
Keep raw-byte cutoff, serialization-level recursion, cold/warm replay and a
following independent healthy request in that matrix. Final resource tests
must also prove no right display or retry after a whole interruption.

## TypeState alternatives and installation audit

Read-only stage-17 review compared two request-local designs against the pinned
checker. No production change or new validation was performed for this review.

* Retain `Resolved(TypeId)` and change `Recovered` to contain an actual
  `NativeCompletion<TypeId>`. This minimizes healthy constructor edits, but
  stores the same native installed-property state in two variants. Every
  writer must decide which variant to use; a clean completion in Recovered
  becomes redundant or invalid. Later diagnostics must preserve the old
  arbitrary type while switching variants. The branching remains in callers
  rather than the existing completion owner.
* Replace both with `Installed(NativeCompletion<TypeId>)`, retaining Resolving
  and Failed. This has a larger mechanical diff but one installed-slot
  representation. Clean versus recovered belongs to NativeCompletion;
  `typeResolutionHasProperty` only asks whether Installed exists. Prefer this
  alternative. It hides the distinction at the owner already responsible for
  evidence, without adding a cache framework, journal, or TypeId poison map.

Failed means no native value was installed. A returned interruption is a
different fact: an invocation can fail after publishing Installed. Do not
centralize an unconditional `Err => Failed` conversion. Preserve an earlier
installation, return the invocation's failure, and never put a whole resource,
I/O, or internal error inside NativeCompletion. Semantic evidence retained with
an installed value must be copied on reads, not appended to the cached entry
again. A native final nil check may select an existing value without erasing
dependencies already consumed by the current invocation.

The existing value inventory above covers accessor, return, mapped and alias
slots. The remaining current producers and related caches are:

| Native producer | Exact value and publication boundary |
| --- | --- |
| `unknownSignature` factory (1043) | Installs singleton errorType as its return at construction. Keep the single actual signature and source-free UnknownSignatureReturn evidence; signature collection itself succeeds. |
| `getBaseConstructorTypeOfClass` (16870) | No extends installs undefinedType. Rejected push returns errorType without installation. False pop and invalid constructor consume their diagnostic dependencies before installing errorType if absent. The invalid TypeParameter branch additionally reads its constraint, constructor signatures, first constructor return, declarations and related-info display. Existing reentrant values win the final nil check. |
| `getTypeOfVariableOrParameterOrPropertyWorker` / `reportCircularityError` (16490 / 18727) | Rejected push and false pop call the same reporter: an actual annotated ValueDeclaration returns errorType, otherwise anyType. The outer getTypeOfVariableOrParameterOrProperty (16457) installs only if its slot is nil and the declaration is not a context-sensitive parameter. A rejected worker push therefore differs from a rejected accessor/return push: its caller can install the returned value. The current variable cycle operation remains explicitly pending. |
| `getTypeArguments` (21804) | Rejected push returns an errorType array of the target's full type-parameter count without installation. False pop installs that array only if absent, before symbol display. Current TypeArgumentsState::Recovered and Failed{resolved:true} retain presence but lose the actual array. They need the same completion contract in their own owner. |
| `getResolvedBaseConstraint` (27313) | Uses circularConstraintType, not Any/errorType. A rejected push returns the sentinel without installation; false-pop diagnostic work precedes its final nil-check installation. Preserve the distinct noConstraintType and circular sentinel operations. |
| `getResolvedTypeParameterDefault` (21912) | Installs resolvingDefaultType before the declaration read; recursive observation replaces it with circularConstraintType. This is its own sentinel protocol, not a false-pop TypeState recovery. |

The audit found these concrete current hazards:

* `members::accessor_read_value` installs Recovered, but both its bound wrapper
  `value_type_worker` and created wrapper `finish_value_symbol` overwrite that
  slot with Failed on the returned error. This removes an actual native
  `typeResolutionHasProperty` barrier. A blanket preservation of cached success
  would cause the opposite bug by hiding the local failure; keep both facts.
* `node_types::alias_reference_value_type` installs the selected actual target
  type before pop, then replaces it with value-less Recovered when circular
  symbol display fails. Native getTypeOfAlias preserves that particular type;
  it does not choose errorType or anyType merely because reporting failed.
* Accessor and declared-alias false-pop branches currently discard acquired
  semantic child evidence when selecting circular recovery. Value replacement
  does not authorize evidence loss; retain the established primary-attribution
  policy independently from the actual native value and secondary evidence.
* Return/base-constructor branches infer Recovered from false pop even when an
  earlier semantic interruption produced no native child completion. Conversion
  must occur at the actual recovery assignment after required diagnostics,
  not from a boolean plus arbitrary Err.
* `constraints::resolved_base_constraint` replaced a whole computed failure
  with Circular after a false pop. Root applied the accepted separate guard
  before the next validation: pop first, return the whole failure, then consider
  native circular work only for a completed native path. `tuples::get_type_arguments`
  still forwards a whole error with `recovery = !complete`, allowing an invented
  installed array-presence flag; its whole-stop/cache repair remains required.

Smallest coherent migration: replace the installed TypeState representation
mechanically at all readers/writers, then provide raw completion getters in the
existing demand/return/property owners. Compatibility admission wrappers can
remain temporarily; native recursive algorithms must use the raw getter and
sequence actual values through NativeCompletion. Start with the source-free
unknown return and complete composite-return acquisition/reduction/mapping,
then each circular producer above with its own exact frame and publication
schedule. No caller may turn an arbitrary binding or display failure into a
native type. TypeArguments and sentinel caches remain explicitly tracked
follow-ups; a TypeState rename alone does not close recovery continuation.

Required proof combines final pinned queries with bounded internal protocol
tests. Final cases must distinguish unknown errorType from false-pop anyType;
rejected push from accepted false pop; annotated from inferred value/accessor
cycles; and genuine Any from recovery. Retain signature-only healthy preludes,
same-checker cold/warm/reordered queries, all composite children in source
order, optional-chain transformations, and later Missing dependencies after
an earlier recovery. Pin base-constructor TypeParameter diagnostic reads and
mapped symbol-display-before-type-display order. Exercise whole interruption
before and after installation, plus healthy independent queries afterward.
Internal tests should additionally prove the exact Installed TypeId, native
barrier visibility, preservation of a reentrant arbitrary value, no extra pop
after a rejected push, and observation-copy/source-free cache behavior. These
protocol tests supplement final query outcomes rather than replacing them.


## Stage-18 installed-value migration checkpoint

The implemented slot representation is now Resolving, Installed(completion),
or Failed. NativeCompletion keeps the actual native value and private evidence;
only a native producer selects anyType, errorType or an error array. Raw
signature, property, type-node, variable-widening and expression-cache operations
return completions. Product admission wrappers remain explicit at native owners
not yet migrated. Their errors still interrupt, rather than supplying a value.

The signature owner now maps all composite children, then reduces and maps their
actual returns. TypeArguments retains an actual array and publishes false-pop
recovery before symbol display. Base-constructor reporting preserves its full
TypeParameter diagnostic argument order. Native final nil checks project the
local completion onto an existing installed value; they neither reacquire its
evidence nor replace the installed entry. An interrupted local calculation
preserves an earlier native slot while returning the interruption.

Display traversal carries completed nodes through its existing builder and
prints their actual bytes. Property preparation owns a single raw read across
name and mode selection; accessor write acquisition precedes the error-type
mode guard. Parameter and tuple display similarly reuse an already acquired
native value instead of repeating a read. Context/flags/depth/serialization
state restore on either completion or interruption. Operator reporting now
sequences this display completion, the arithmetic recovery assignment and the
compound setter question through the shared completion owner.

Read-only adversarial review caught and corrected two alias/accessor defects:
a false pop must not convert an arbitrary outer error into a completed native
value, and a final nil selection must not append an installed completion a
second time. A second review found that PropertySymbolValue demand wrappers
could cache Failed above an already installed native property. Both canonical
MemberValue and PropertySymbolValue recipes now dispatch before wrapper state;
only their actual symbol cache owns the answer.

The initial stage-18 compile (`vrun-2bc5090c-f006-4142-9df1-268cc0bf6cda`)
reported six visibility/parent-adapter errors and one unused mut. These seams
are repaired and compilation is being rerun. This checkpoint is not a runtime
parity claim. The new display fixture covers eight multi-property/accessor
roots, including genuine Any and healthy controls; maintained native and final
Rust receipts are pending.

Review also identified the next required continuation path: Identifier still
enters the legacy check_identifier/value_reference_type admission boundary.
`const bad = 1 - 1n; const q = bad + later.x` therefore needs the complete
identifier/alias read owner migrated, even though its inline and ordinary dot
counterparts now carry recovery. Read flow must consume an established type
and preserve its evidence, without turning excluded narrowing into identity.
Imported aliases and repeated actual reads belong in the final proof matrix.


## Stage 19 invocation transport

The next trace exposed call resolution and identifier reads that still admitted
a recovered value before the native caller had completed its later questions.
The implementation keeps native slot owners and returned invocation evidence
separate. In particular, `getResolvedSignature` retains a signature installed
by nested contextual resolution, projects a completed local result onto that
value without re-consuming its cached evidence, and does not replace an
installed slot when an outer invocation is interrupted. Native active-loop
restoration and the resolving-signature deferred result remain distinct cases.

The core `NativeCompletion` sequencing rule now also supports `NativeSequence`
for imperative native loops. One local sequencer belongs to one actual worker;
each `read` consumes a completed child and an error returns immediately through
`?`. `finish` runs after cleanup, composing direct failures and returned values
through the existing completion policy. It is not evaluator state, a relation
session journal, or a second error-merging policy. This keeps source-native
branch and loop structure visible without deeply nesting continuation closures.

Call acquisition now transports completed callee, argument, display and return
values through the same call-site cache. UnknownSignature is created by
`resolveErrorCall` after the actual argument checks; only consuming its return
adds that return's error marker. The apparent-error early return precedes
signature inventories in ordinary and constructor resolution, as in the
pinned compiler. Source-element checking, relation workers, signature parameter
questions and identifier producers are being migrated in their own owners.
Legacy downstream APIs remain genuine interruptions until those owners expose
a completion; no caller infers a replacement native value from an error reason.

The new call matrix deliberately includes native Any outcomes after invalid
arguments, repeated quick/checked callee acquisition, constructor and tagged
calls, diagnostic display dependencies and healthy Any controls. Public query
assertions must retain ordered, located causes rather than using the displayed
Any or the compiler's final errorType bit as the whole validity test. New
unresolved-alias cases also distinguish exact qualified names, source-site
provenance, argument/default schedules and healthy twins. Both matrices require
pinned compiler capture plus the final Rust queries before this checkpoint can
be treated as validated.

### Relation completion checkpoint

Relation workers now carry native comparison values and ordered recovery through
property, signature, index, union, mapped and diagnostic-argument loops. Their
raw display entries defer observation until the outward relation operation;
existing display entry points still observe returned copies for call/operator
consumers. Diagnostic ErrorState rollback never restores acquisition evidence.
Signature rest/position/count helpers share the completion owner, including
minimum-count cache entries.

`recursiveTypeRelatedTo` retains comparison evidence separately from its
pre-lookup `getRelationKey` acquisition. A returned cache hit replays only the
installed comparison; ordinary Failed plus reporting re-executes without adding
old evidence. Each pending Maybe key records its own completed comparison before
`resetMaybeStack` commits it. Active assumptions add no future ancestor events.
Warm and cold acquisition can differ under this native cache schedule; the
contract does not invent transitive proof edges between Maybe assumptions.

The earlier limit-first aggregation convention is superseded by producer-owned
completion. The two recursive depth/allowance guards establish False with a
typed source-free limit event at that exact point. Earlier recovered properties
therefore remain earlier, while root/cache diagnostic display remains later.
Only native `checkTypeRelatedToEx`'s root write adds Overflow flags; recursive
Failed and Maybe commits retain their actual native flags plus separate product
evidence. Root key, installation and display do not run after any interruption.
An already installed root entry survives a later display interruption. Whole
resource/I/O/internal errors retain their distinct stop/precedence policy.

This is an unvalidated source checkpoint, with internal sequence and ordered
limit-replay tests staged. Required final proof still includes mixed earlier
property recovery followed by the natural native depth boundary, warm/cold
failed-cache reporting, overload diagnostic rollback and equal-byte qualified
display retries. The latter needs a coordinated zero-demand hook observing only
already-produced display bytes, never an extra TypeToString call.

The next complete downstream slice is inference's comparer-to-slot transport:
`instantiateSignatureInContextOf`, `applyToParameterTypes`, `applyToReturnTypes`,
`inferred_type`/`inferred_type_worker`, `inference_compare`, and fixing/nonfixing
mapper cache publication must retain their actual completions. They currently
remain explicit admission boundaries. Signature predicate resolution likewise
still uses its existing Result slot; neither owner is replaced by a relation
session journal or a guessed recovery value in this checkpoint.

## Promise adoption and inference continuation

Promise adoption retains its existing request-local owner in `promises.rs`.
`getPromisedTypeOfPromiseEx` (checker28683) and
`getAwaitedTypeNoAliasEx` (checker31027) cache only actual nonnil native values.
An awaited union may drop a nil child and still acquire later constituents;
nil remains distinct from errorType and from an interrupted operation. The
Any/Awaited-instantiation prefix runs before cache lookup on every invocation.
The recursive worker unwinds its stack before returning either completion or
interruption. No promise-specific filtering rule replaces shared `mapType`,
type-fact, constraint, signature or relation operations.

The alternative of admitting each child's evidence in the current Result
wrappers would preserve refusal but stop later native work. The chosen design
stores native completions at the existing slots and sequences their actual
consumers. A private promised result also retains the last rejected `this`
type when all then-signature candidates fail. This is invocation data, not a
second cache: checked awaited diagnostics acquire the original type's display
and that rejected type's display only in the compiler's invalid-thenable
branch. Quiet awaited suggestions omit those displays. Only
`checkAwaitedType` converts a completed nil into consumed errorType;
`createPromiseType` has its separate completed-nil-to-Unknown rule.

The inferred-constraint final test demonstrates why the same migration must
include inference slots and dynamic mappers. A call-only accumulator leaves
`getInferredType` interrupted at a recovered constraint and prevents the later
ordinary argument check. The complete owner preserves the provisional inferred
value installed before constraint checking, the fixing/nonfixing distinction,
the active-mapper cache window, and the live relation comparer. Recovery is
carried by each invocation and installed property; candidate TypeId writes and
the native visited-priority table do not become an evidence journal.

`isInferencePartiallyBlocked` belongs to the language-service
`runWithInferenceBlockedFromSourceNode` scope, entered through
`GetContextualType(IgnoreNodeInferences)`. Current Q3/Q4 do not enter that
service operation. Ordinary call reporting therefore stays enabled; a
recovered constraint or skipped generic function must not impersonate that
scope. A future entry must port its ancestor skip set and cache save/restore
together with the reporting predicate.

### Stage22 template and mapped continuation checkpoint

Template source nodes now carry all placeholder completions through the shared
factory, including union distribution, cross-product recovery, nested span
flattening and string-mapping reconstruction. Mapped nodes and their six native
links retain completions at their existing installation points. The mapped node
itself is published before its eager constraint read: that read contributes to
the current invocation, while a warm node read returns the preinstalled clean
object. The constraint link independently owns its acquired completion.

Mapped instantiation carries these links through scalar, array, tuple and union
callbacks. The shared generic-object flag cache and generic mapped/index queries
also transport their consumed evidence. These changes use the same native
completion owner as generic mapper and anonymous-object instantiation; they do
not add a second callback journal. Remaining legacy child operations still
interrupt when no completed native value is available. This source checkpoint
has peer review but has not yet passed a Rust build or final query suite.

The pinned combined matrix `vrun-490e3d0b-4ab3-4e99-b03f-cd081ad09075`
passed all three profiles for the 25 source-check controls and 29 promise
controls. Its zero-demand tuple observation records the actual variadic element
nil-to-errorType assignment at element 1 for the numeric input. The final
source-check expectation therefore includes the tuple-root recovery between
the placeholder failures and the later runtime argument. Diagnostic code lists
remain sorted summaries, not causal-order evidence.

The original promise invocation control remains unchanged: the empty global
Function permits the untyped-call fallback, so its clean Any result does not
prove a quick-call bypass. The added union invocation excludes that fallback
and records actual errorType plus two missing-name diagnostics and the
noncallable diagnostic in every profile. All 29 captured rows are now compared
unconditionally; the maintained rerun and final Project tests remain required.

### Stage22 inference and mapper checkpoint

`inferTypeArguments`, the shared inference traversal, comparer callbacks, and
fixing/nonfixing mappers now transport operation-local completion. Candidate
lists remain actual TypeIds, and visited pairs retain only native priority.
The TypeScript-only input envelope excludes native AnyDefault; an absent
inference therefore publishes Unknown at the native provisional assignment.

`getInferredType` publishes the completed candidate prefix before acquiring the
constraint. A nonnil constraint replaces that slot with this invocation's full
completed result. A nil constraint leaves the provisional slot installed even
when the constraint question returned recovery evidence; the current invocation
still returns that evidence. An interrupted suffix likewise cannot erase an
already-installed provisional value. Unfinished slots retain interruption, and
fixing mappers still run their actual intra-expression work before fixation.

`instantiateTypeWithAlias` keeps containment and alias-argument questions outside
the active mapper cache. That cache stores only completed worker results; depth
cleanup always runs and interruption never fabricates a result. The native
instantiation limit produces actual errorType with its consumed limit evidence.
The containment computed-bit owner retains its own completed acquisition;
factory-supplied computed bits remain intrinsic clean facts. Raw flag and
installed-symbol-value observations do not replay acquisition evidence.

Object instantiation similarly separates effective argument/alias prefixes from
the target cache. The completed object factory installs before the computed-flag
suffix. Later failure or recovery in that suffix is returned by the invocation,
without rewriting the already-installed native factory result. Homomorphic
reverse inference retries installed nil; the separate reverse-value cache maps
an installed nil to Unknown on the later native hit.

This is a coherent transport checkpoint, not completion of every downstream
producer. Conditional/substitution and keyof producers, structure inventories,
object outer-parameter collection, widening, contextual object methods, and
higher-order generic function inference still have separately owned admission
or explicit unsupported boundaries. They must migrate as whole native
operations; no clean wrapper or guessed value has been added to bypass them.
The first final gate is the maintained circularInferred clone control, whose
later argument must still be checked after constraint recovery. Further proof
must cover defaults/backreferences, fixing and nonfixing callbacks, captured
silent comparers, native cache hits, and reverse-cache nil distinctions.

### Stage23 literal-context and body continuation checkpoint

The first inferred-call argument may consume a circular constraint before
`inferTypeArguments` collects any candidates: `checkExpressionWithContextualType`
(checker7461) regularizes a literal through `isLiteralOfContextualType`
(checker25430), which asks its contextual parameter's base constraint. That
recursive predicate now returns completion. Its union/intersection short
circuits, nil-constraint Unknown, and primitive/literal gates are unchanged.
Contextual frame cleanup still precedes returning recovery or interruption.

The actual mutable-location and return-body consumers carry this completion
through literal/unique-symbol widening, const-context checks, every admitted
return contribution, and the subtype union. Context-free function types cache
only their completed body/factory result, with contextual prelude outside the
cache. Contextual signature return installation retains the produced body
completion only when the native return slot is still absent. Fresh synthesized
Known return fields remain clean copies. General widening, contextual-return
inventories, reachability, and async/generator producers remain separate
explicit completion/dependency work.

The bare-self branch now checks the cached Normal callee and compares its actual
Type.symbol with the merged raw function symbol. It no longer acquires the
function value or compares TypeIds. The function-expression constant-reference
question remains an explicit dependency only after matching symbols. The
preexisting missing `reportErrorsFromWidening(fn, returnType, FunctionReturn)`
operation (checker20099), before contextual unit widening, is also required
follow-up; this transport checkpoint does not certify its absent behavior.

The mapped `limited` control already records native TS2353 in all profiles.
Its non-error `{ a: string; b: number; }` result is overload-failure recovery,
so the final product expectation is located `overload_recovery`, consistent
with the design's compiler-recovery policy. Required diagnostic serialization
must still finish: an Unsupported cause remains a failing test. A separate
no-extra-property twin distinguishes healthy limited-key inference from that
recovery path; its native harness is `type-native-mapped-limited-oracle.test.mjs`.
Its three profiles passed in `vrun-e491e864-f605-47da-a67e-c5a311701c3f`;
the final Project control asserts the healthy summary and both inferred members.

### Stage23 structured-inventory completion checkpoint

The inventory owner now returns `NativeCompletion<MembersView>` through
`getSignaturesOfType`, `resolveStructuredTypeMembers`, reference/declared,
anonymous, mapped, reverse-mapped and composite inventories. The reference
branch acquires arguments before declared members (checker19000–19023).
`resolveDeclaredMembers` publishes its provisional declared fields before
call, construct and index acquisition (19517–19560); index values precede
keys even when a key is later rejected or duplicated.

`setStructuredTypeMembers` remains the single publication owner (25053):
raw table and MembersResolved precede named projection, while previous
signature/index fields remain visible until their actual assignments. The
installed completion is separate from the current invocation. A stopped
suffix does not erase a published table or replace its established fields
with failure. Flag-cleared recomputation also preserves previous fields.
Inherited raw-table mutation retains its property acquisition evidence
before the later signature/index suffix; final named projection remains at
its native final setter. Nil, absent and present-empty tables remain distinct.

Union composition retains all call inventories before matching, array-method
fallback before construct inventories, and indexes last (20957). Intersection
mixin classification and signature/index composition preserve native child
order (21207). Signature matching, parameter/this composition, mapped key
callbacks and index value reductions now use their real completion siblings.
The same native getBaseTypes list/cache, interface acquisition, base recursion
and class/static inventory phases transport completed values. Native global
lookups supply their retained declared target instead of reacquiring it.

This is a staged transport checkpoint, pending the coordinated Rust and final
native inventory matrices. General class/interface type construction,
conditional key lowering, composite-property synthesis, generated property
keys and alias eligibility still contain explicit independent producer or
admission boundaries. A missing completion from those owners remains an
interruption; no value is invented to reach later inventory work. The
single-signature/contextual-return and remaining noninventory callers are
not certified by this slice. Bounded peer review covered native publication,
reference order, signature matching/composition and signature descriptor
construction; the instantiateSymbol and derived return/array completion
seams found during that review were corrected before closure.

### Stage23 facts and global factories

The operating envelope remains one request-local native checker model over a
loaded Project. No shared daemon cache or compiler subprocess is introduced.
The facts owner now performs the complete recursive `getTypeFacts`,
`getTypeWithFacts`, and nullable-intersection adjustment questions with
completion values. Callers no longer decide whether a facts dependency may
continue. Function-object classification retains its actual member inventory,
raw bind-symbol question, Function lookup and subtype relation in native order.

`getGlobalType` now retains both the bound symbol and the acquired declared
TypeId. Reacquiring the type at each consumer would replay an unrelated cache
history. A quiet identity read retains failures acquired while resolving that
type but does not consume a missing/wrong global fallback. A factory consumes
that fallback at its own caller location. The alternative of putting the
factory marker in the resolver cache would contaminate later quiet lookups and
give repeated factories the first caller's location.

Native `getGlobalPromiseType` and `getGlobalPromiseTypeChecked` have separate
memo cells even though they share `getGlobalType`. The lookup cache therefore
includes that getter identity. `createPromiseType` returns native Unknown when
the checked target is emptyGenericType while retaining the consumed recovery;
it does not then unwrap or await its argument. Generic array construction still
has no model emptyGenericType representation and keeps that case an explicit
interruption, rather than inventing a normal reference target.

`getBaseTypeOfLiteralType` now transports recursive union completion and also
implements the native template/string-mapping-to-string branch. Its cache is
used only for unions, matching the native worker. Strict and loose logical
operators select the same native facts, filtering, base and union operations;
logical-assignment checking remains after the computed result. The 48-query
inventory matrix covers callback constraints, then/callback indexes, deferred
arguments, union/intersection inventories, Awaited alias absence/recovery, and
strict/loose template and string-mapping behavior. Reversed Project requests
prove request isolation only; they are not native warm-cache evidence.

### Stage24 composite lookup continuation

The stage23 `unionInventory` final witness stopped after both `LeftThen` and
`RightThen` index failures. Its first remaining admission was not union
signature matching: `getUnionIndexInfos`'s first exact-key presence query called
`index_info_of_type_completion`, which still consumed the legacy inventory
wrapper. The observed left/right, left/right, left, left acquisitions match call
inventories, construct inventories, the first index list, and the first presence
query (native checker13442,18879–18906).

The shared exact/applicable index queries now sequence actual inventory
completions. Ordinary/object property lookup carries the same protocol through
reduced/apparent conversion, raw table lookup and native augmentation. Composite
property creation and inventory own their completed caches: absent properties
remain misses; completed empty inventories are hits; augmented propagation copies
only an already installed nonpartial property. Their cache entries contain the
factory's acquisition evidence, excluding any calling inventory's prefix.
Intersection reduction consumes that complete composite inventory and its native
never-property predicate. This preserves the existing 48 native inventory
expectations, including the later comma-expression dependency; validation remains
required at the coordinated checkpoint.

The now reachable composite factory also retains the native actual-errorType
constituent skip, late-bound-name index exclusion, tuple-rest/Undefined value
selection, and literal-or-pattern HasLiteralType decision (checker21374–21520).
Readonly-assignment and merged-parent child questions consume their existing
completion producers; alias eligibility and unsupported symbol metadata remain
explicit interruptions. No recovery value is fabricated at those boundaries.

Intersection reduction also preserves the pre-walk clean computed marker when
the completed walk finds no never-property. Only the later native
IsNeverIntersection write publishes that invocation's traversal evidence mapped
to Never; union reductions retain their separate postcomputation slot. This is
the same installed-field versus returned-invocation distinction used by mapped
node and inherited-table publication.

### Stage24 unique-symbol factory checkpoint

The unique type now retains the actual `Type.symbol` and a typed
`NativePropertyKey::Unique` whose underlying key remains exact. The evaluator
owns `getESSymbolLikeTypeForNode`'s symbol cache and the UniqueKeyword node cache;
Model only allocates `newUniqueESSymbolType`. The source owner preserves the
immediate SymbolKeyword gate, parenthesized-parent walk, and existing
`isValidESSymbolDeclaration`. The shared `getSymbolOfNode` now performs late then
merged normalization after its optional real attachment. Checked and quick call
paths share the full `isSymbolOrSymbolForCall` raw-global/raw-Value identity rule.

The graph projection exposes UniqueSymbol origins and typed symbol property keys
without encoding numeric compiler IDs. Exact generated-name comparison and
approximate length belong to the existing binding key/IdentityOrder owners, per
design B. Length reads do not observe ordering. Ordinary/fixed names retain their
byte lengths. Existing facts, constraints, instantiation, containment, and graph
view dispatch explicitly handle the new established type.

This is an integration checkpoint, not completion of unique-symbol support.
Remaining required native owners are nonnil declaration visibility
(`IsAnySymbolAccessible`/`hasVisibleDeclarations`), actual late declaration
`trackComputedName`, qualified expression-chain type arguments and shared fake
scopes, and any generated-name-as-string consumer still requesting bytes.
Declaration-less late mapped properties perform the actual diagnostic-symbol
acquisition and report state before their name; no accessible-chain result is
substituted for symbol accessibility. Native and final factory/identity matrices
and B cutoff boundary controls remain required. No validator ran in this lane;
root owns the combined frozen checkpoint.
