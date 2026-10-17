# Native structured relations

This source-backed design uses the Ousterhout skill for one immutable accepted
Project, one retained canonical binder and a request-local native Evaluator.
It adds no backend abstraction, cross-request cache or concurrency mechanism.
The full selected type profile remains required. This document proposes the
next implementation boundary. Fresh compiler controls now establish the first
property/inventory schedule below; native production and final Project tests
for this boundary remain pending.

The source pin is TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. Current
`evaluation/relations.rs` already owns Assignable, Subtype, StrictSubtype and
Comparable scalar/primitive-union questions. Calls, constraints, narrowing,
assignment reduction and future iteration must continue to ask that owner.

Two designs were compared against a recursive interface pair, an earlier
incompatible property followed by an unresolved property, and a callable
interface with an index signature. A property loop in Calls and a second
callback/index loop in iteration would be short individually. Their callers
would have to coordinate normalization, inventory demand, variance, optionality,
cycle assumptions and native failure-cache installation. Adding recursion
would change each loop and its failure interpretation. The chosen design
extends the existing deep relation operation with one native Relater session.
Consumers still ask `type_related(source,target,kind,consumer)` and receive an
established boolean or an actual consumed failure. Internal Ternary values,
comparison caches and member schedules stay inside this operation.

The relation owner will retain request-local caches partitioned by the actual
native relation kind. Each root comparison has a session with source/target
stacks, recursion flags, intersection state, expanding flags, maybe keys,
allowance and overflow provenance. Nested comparisons inside a session return
native True/False/Maybe/Unknown, rather than calling the public boolean entry
and prematurely losing assumptions. A genuinely nested checker relation
question may create another session while sharing the appropriate native
cache. No per-property visited set can substitute for this protocol.

`isTypeRelatedTo` (relater.go:170) keeps fresh-literal regularization, identity
and simple relations before the object cache and checked relation. The
existing pre-normalization object-to-primitive failure must remain before
member/constraint demands. `isRelatedToEx` (2600) then owns directional
normalization, exact-constraint and nullable-union fast paths, excess-property
and weak-type checks, and small-union cache bypass. These are mandatory branches
as their native factories become established; a structural loop cannot replace
them. Ordinary unavailable factories stay located pending dependencies.

`getRelationKey` (checker.go:17526) must be ported as one complete decision.
Ordered type identities alone cover ordinary objects, but generic-reference
structure has a separate canonical key and constrained-parameter marker. The
marker permits a broadest-equivalent recursion key. Intersection state belongs
in the key. Identity comparison has
its own canonicalization; it must not be approximated by Assignable in both
directions. A future Identity relation is an explicit same-kernel operation,
needed by native generic/signature/algebra questions, rather than a second
object comparer.

The generic key is itself a checking operation. Its
`writeGenericTypeReferences` branch (checker.go:17364) calls the existing
`getConstraintOfTypeParameter` owner before deciding whether to write a shared
parameter index or its actual TypeId. A syntactic constraint-presence flag is
insufficient, and a visited constraint recovery must remain a product failure.
The broadest `ignoreConstraints=true` key deliberately skips that demand.
Nested generic references use this exact key question only to depth4. First
plain-interface/TypeLiteral controls have no generic argument key demand;
generic reference admission requires paired key-schedule controls.

`recursiveTypeRelatedTo` (3061) first preserves an active session overflow,
then checks installed cache entries before allowance,
then an existing maybe key or constrained broadest key, then actual depth100,
and finally structured comparison. Deeply expanding source and target chains
can return Maybe without being a depth error. A False under assumptions is
cached immediately. True, or Maybe at both root depths zero, commits the
participating maybe keys as succeeded; Unknown is not a succeeded cache entry.
Product Unsupported/Invalid/resource failures never become native ordinary
False or a succeeded assumption. Session cleanup must restore stacks/maybe
keys on every product failure and retain only properties actually installed
by a completed native operation.

The initial allowance in `checkTypeRelatedToEx` (359) is
`(16_000_000 - relation.size()) / 8`, not a new product threshold. Cached overflow
entries retain depth-versus-complexity metadata. A later cache hit still carries
that provenance into the exact consumer; it cannot become an established
ordinary False. Existing natural evidence in
`types-relation-limit-evidence.md` supplies depth99/100/101, allowance and Maybe
controls, but does not substitute for final native implementation tests.

For ordinary structural comparison the schedule in
`structuredTypeRelatedToWorker` (3261; ordinary branch near3840) is properties,
call signatures, construct signatures, then index signatures. Each established
False skips subsequent collections. Required inventory acquisition itself can
already demand index dependencies, so graph/display laziness must not be
mistaken for relation laziness. Generic reference variance, arrays/tuples,
mapped/conditional/indexed/substitution types and discriminant unions route
their full native branches before or around this ordinary comparison.

`propertiesRelatedTo` (4100) first checks unmatched properties, including the
relation-dependent requirement for optional properties, then any target object
literal restriction, then target properties in native order. It skips actual
prototype/filtered roles and identical symbols without demanding their types.
`propertyRelatedTo` (4263) owns private declaration identity, protected override,
StrictSubtype readonly direction, type comparison, and optional-to-required
rejection. Readonly alone does not reject Assignable. `isPropertySymbolTypeRelated`
acquires the target non-missing type first: target Any/Unknown can succeed
without acquiring an unresolved source member. This precise short circuit is
the first proposed reproduction below. Read and write accessor types must not
be collapsed; the native relation question uses its actual symbol read type.

The current `Property` presentation view contains key/read/write/display facts,
but display origins and PropertySerializationId are not semantic symbol
identity. Relations need an opaque actual property-symbol association supplied
by the existing member inventory owner, including native flags, value-declaration
identity and canonical/instantiated member provenance. Extend that owner and
its view once; do not recover identity from key text, origins, display IDs or
DemandId equality. Canonical named and TypeLiteral tables already provide the
actual source association. Object-literal/JSON/transient/merged union members
must supply their own real native association before the corresponding identity
fast paths are admitted. Missing association is a precise dependency, not a
synthetic equivalent member.

`signaturesRelatedTo` (4434) preserves constructor abstraction/accessibility,
same-target pairwise comparison, sole-signature comparison and target-each /
source-some overload search. `signatureRelatedTo` (4536) owns erasure and actual
SignatureCheckMode, using the existing SignatureId/factory/mapper/provenance.
`compareSignaturesRelated` (1458) checks arity before contextual generic
instantiation, then this, parameter positions and returns. StrictFunctionTypes
depends on the target declaration role; methods/constructors and nested callback
variance have distinct native rules. Instantiated generic parameters suppress
the callback shortcut. Parameter comparisons use the native positional call
type operation, independently of graph/display types.

Return comparison first gets the target non-circular return; target Void/Any
skips source return and predicates. That narrower native operation deliberately
uses Any while a return resolution is already in flight, rather than initiating
ordinary return-cycle recovery. Reuse the actual SignatureReturn owner to
implement this separate question. Predicates preserve kind/index compatibility
and child types; booleans cannot stand in for a required identifier/this guard.
Do not force both return bodies before this schedule decides their dependencies.

`indexSignaturesRelatedTo` (4571) acquires target indexes and distinguishes the
non-StrictSubtype target-string/Any shortcut. `typeRelatedToIndexInfo` chooses
an actual applicable source index before inferred member compatibility. Index
inference is restricted by native object-symbol kind, signatures, intersection
state and FreshLiteral under StrictSubtype; an interface and a type literal
with the same printed properties can differ. `membersRelatedToIndexInfo` (4627)
filters applicable native key types before reading properties, preserves exact
optional/undefined facts, then compares applicable source indexes. Index key
matching, property-name literal acquisition and non-missing reads remain shared
complete native questions, not local string tests in the relation module.

The first proposed fixture is a named interface pair behind a two-overload call,
so variable-initializer getQuickTypeOfExpression cannot bypass applicability:

```ts
export interface Source { gate: number; latent: MissingSource; }
export interface Target { gate: number; latent: unknown; }
export declare const source: Source;
export declare function select(value: Target): "chosen";
export declare function select(value: never): "fallback";
export const selected = select(source);
```

Fresh compiler controls establish the selected scalar result and actual
dependency trace before public expectations are asserted. Paired variants replace
target latent Unknown with Number (source failure should be consumed), or make
the earlier gate incompatible (later source failure should be skipped). A
target-order reversal makes the unresolved latent comparison precede gate
rejection. A same-canonical-interface control with the unresolved child checks
the identity fast path. Both source and target index dependencies are acquired
before property short-circuiting. The scratch observation archive is
`.cc/temp/types-structured-relations-answers.json`, with 39 fresh API and 39
same-checker observations over strict, loose-null and exact-optional profiles.
Every native selected scalar root is healthy; four cases per profile nevertheless
visit one unresolved source property or index dependency before summary. The
target Unknown/Any, earlier mismatch, unmatched property, exact identity and
optional rejection controls skip it. Readonly alone remains assignable.
The registered baseline file is
`tools/type-native-structured-relations-oracle.test.mjs`. Its first two tests
passed in `vrun-51fafb14-2420-41a6-a058-d6899c9cb7b3`, with zero skips.
These measurements are not native production or final Project RED/GREEN
evidence.

Natural caller controls in the fixture's `limits` directory demand an overloaded
call with structurally equal Source/Target types and a later exact-Source
overload. Depth99 selects `"chosen"`, with guard maxima99/99 and no failure.
Depth100/101 select the healthy literal `"same"` through the later overload,
with guard maxima100/100, a consumed `relation_depth_limit`, TS2321 and the first
comparison's overflow False. This is actual call consumption, independently
of the earlier conditional research. The final product must preserve that
visited overflow even though the compiler's selected scalar is healthy.
Recursive `{next:Self}` controls finish with Maybe at depth1/1 and select
`"chosen"`; adding String/Number leaves produces established False and selects
`"same"` without overflow. Ten fresh API and ten same-checker controls over
strict/loose null profiles are archived in
`.cc/temp/types-structured-call-limits-answers.json`. The expanded baseline's
three tests passed in `vrun-5bffa6f4-994b-4b7c-860d-fb69984d83ab`, with one
matched file and zero skips. The complete matrix is 49 fresh API and 49
same-checker controls. `.cc/temp/native-structured-relations-inputs.json`
records separate post-run source/tooling hashes and explicitly states that
the capture is not atomic with compiler execution.

After those fresh controls, one final Project test file should assert exact
Found scalar outcomes, later-overload selection, OverloadRecovery for ordinary
complete rejection, and original InvalidSemantic causes for visited missing
members/indexes. It must compile and fail on the missing native relation owner
before production. Subsequent complete-owner increments cover optional/exact
options, readonly/accessibility, call/construct/overload/callback variance,
generic instantiation, indexes, recursive positive/negative pairs and natural
native limits. The full relation/profile/resource gate remains open throughout;
iteration and inference consume these operations incrementally without cloning
their comparison rules.


The first final Project file is now `tests/type_structured_relations.rs`. Its six
behavior tests compiled and failed on the missing native structured relation
operation in `vrun-4a7c8533-7e4a-483b-966d-9462b537552f` (six executed, six failed,
one matched file, zero skips). The public assertions cover the three option
profiles, exact skipped/visited declaration dependencies, recursive Maybe versus
ordinary False, and actual depth99/100/101 call consumers. The unmatched test
supplies a genuine global `interface Object {}` through the augmented profiles:
`getUnmatchedPropertiesWorker` asks ordinary `getPropertyOfType`, whose absent
own entry must still consult the actual global Function/Object augmentation
owner. Original noLib observations retain the missing-global recovery history
as separate evidence; they do not justify a product own-table absence shortcut.
The additional formal native control is the fourth test in the same oracle
file. The complete four-test baseline passed in
`vrun-ae5500b0-3e83-46dd-8aa4-31e9b3581125` (four executed, one matched file, zero
skips). The expanded matrix has 52 fresh API and 52 same-checker controls;
three of each supply actual global Object inventory and preserve the ordinary
False result without a consumed missing-property dependency.

The same native worker terminal is required by destructuring assignment
reduction. A real `Array<any>` assigned to declared `string | number` is first
unrelated to both primitive constituents. The reduced candidate becomes Never,
then the native final heuristic validation compares `Array<any>` to Never.
Never is not in the native Primitive mask, so the earlier object-to-primitive
fast path is insufficient. `structuredTypeRelatedToWorker`'s ordinary default
has no object/intersection-to-object or object/intersection-to-union comparison
for this target and returns False. The current Model already classifies actual
Reference/Target/Anonymous types as Object correctly. This missing terminal
must be implemented in the same structured worker, preserving its preceding
normalization/constraint/generic branches, rather than changing Model flags or
adding a spread-specific relation shortcut. Root's final basic-flow consumer
`vrun-97b935c0-ae8f-4353-8dad-bc1bfed5c6ac` is the existing compiled reproduction
(five passed, one failed at matching-spread-assignment).


A further scratch-only natural key control is archived in
`.cc/temp/types-structured-key-constraints-answers.json`. Both A<T> and B<T>
contain only `{gate:number}`; T appears in neither member. The first overloaded
A<T>→B<T> comparison with `T extends Missing` selects healthy `"chosen"` while
consuming one TS2304 before summary. Replacing the first candidate with the exact
A<T> identity preserves healthy `"identity"` and skips that missing constraint;
Number and unconstrained counterparts also visit no TS2304. These are four fresh
API plus four same-checker scratch observations, not a registered baseline or
final Project proof. Together with checker.go `getRelationKey` /
`writeGenericTypeReferences`, they show that generic key construction cannot be
a pure serialization of printer shape or TypeIds: constrained generic arguments
ask the actual parameter constraint before cache lookup. The broadest key's
ignoreConstraints question remains distinct. Generic keys must preserve their
located obligation until that complete acquisition is wired; ordinary nongeneric
reference keys do not justify a generic-key shortcut.

## Ordinary relation implementation checkpoint

The ordinary relation increment now lives in `evaluation/relations.rs`.
`type_related` remains the operation used by applicability, narrowing,
assignment reduction and constraints. Its request-local `RelationsState`
partitions caches by the native relation kind. A separate invocation-local
`Relater` retains Ternary answers, maybe keys, independent source/target stacks,
expanding-recursion flags and the native allowance. Recursive child comparisons
stay inside that session; a separate checker question can start another session
against the same cache. No call-site comparison loop or visited set was added.

Native overflow unwinding installs the completed inner Failed cache entries.
Each installed entry also retains product overflow provenance; the top-level
native overflow installation and subsequent cache-hit consumption preserve
the depth-versus-complexity distinction. A product dependency failure restores
the invocation's stacks and assumptions without installing a native result for
the unfinished operation. Already completed native child cache entries survive.

Ordinary comparisons now acquire target weak-type inventory, unmatched property
lookups, target-ordered property comparisons, call signatures, construct
signatures and index signatures at their native points. Property comparisons
use the member owner's actual opaque symbol tokens, declaration modifiers and
ValueDeclaration identities. Target non-missing reads precede source reads;
Any/Unknown skipping still reaches optional-to-required rejection. Source
readonly rejects only StrictSubtype in the native direction. Exact canonical
property identity skips the member type demand. Object-to-Never reaches the
ordinary worker's final False after its earlier normalization branches.

`getRelationKey`/`writeGenericTypeReferences` now retain ordinary versus generic
keys, actual constrained-parameter acquisition, broadest-equivalent parameter
positions and the native nested-reference key depth four. This implementation
does not yet establish generic variance or all generic member factories. The
scratch key-constraint observations above still need registered final proof.

The six-test final Project checkpoint passed in
`vrun-043a1ce6-5ab4-43d6-a5f3-00566f2d3c7c`, following the six-test assertion
RED and four-test compiler baseline above. Nonempty signature/index
comparison, all-optional weak targets, excess checks, composite normalization,
protected overrides, variance and the other factory-dependent branches retain
specific pending failures. This checkpoint does not close the full semantic
profile or natural allowance/expanded-recursion evidence gates.

An adversarial peer review checked the property and session operation against
native `relater.go` 768–839, 3061–3181 and 4205–4335. It found no concrete mismatch
for the admitted canonical property family: unmatched lookup precedes reads,
actual symbol equality precedes modifier/type demand, target Any/Unknown still
allows optional rejection, and generative-recursion counting advances the last
TypeId for every matching identity. A reciprocal review checked the new member
owner against `instantiateSymbol`, modifier selection and the thisless rules;
no concrete admitted-family blocker was identified. These source reviews are
not a substitute for full-profile final Project runs.

## Signature and index continuation

The next increment preserves the same immutable Project and request-local
Evaluator envelope. `signaturesRelatedTo` and `compareSignaturesRelated` belong
to the relation session: recursive parameter/return comparisons must retain its
native Ternary, cache and overflow provenance. A standalone boolean signature
helper would lose that context and duplicate call applicability decisions.
Signature acquisition, erased/canonical instantiation and actual target/mapper
association remain with the existing signature/generic factory owners. The
association is an actual signature target, not a flattened declaration root.

The existing resolution owner gains the complete read-only
`findResolutionCycleStartIndex` operation used by
`isResolvingReturnTypeOfSignature`. Both it and `pushTypeResolution` share the
native reverse traversal, installed-property barriers and call-resolution
window. Checking a lazy wrapper's Resolving state cannot answer that question.
The whole evaluator property dispatch moves into that owner; heritage adds
BaseTypes and BaseConstructorType only when its real installed-cache facts
exist. `getNonCircularReturnTypeOfSignature` retains native transient recovery
while attaching consumed product failure provenance.

`GetNonNullableType` and `getSingleSignature` become complete shared structure
operations used by callback comparison and the current call quick-return
consumer. Explicit/inferred predicate acquisition has one signature owner;
effect-signature checks route through it. Constructor accessibility is captured
from the actual signature declaration's modifiers, independently of instance
property visibility. Generic erasure, contextual signature instantiation,
abstract constructors and class static/default-constructor factories remain
required; an ordinary callable result does not close those obligations.

The new compiler control file
`tools/type-native-structured-signatures-oracle.test.mjs` and final consumer
file `tests/type_structured_signatures.rs` use 34 actual overloaded scalar
consumers. They distinguish target Void/Any skipping from Unknown/Number
source-return demand; target-first return failure; minimum-arity trailing reads
and earlier unread annotations; extra target positional reads; this-parameter
top-signature observation; property/call/construct short circuits and eager
inventory dependencies; target-method variance and nested callbacks; signature
matrix order; explicit predicate kind/index; generic context versus erasure;
and abstract/concrete direction. Strict-null, loose-null and strict-function
option variants are explicit. The six compiler-control tests passed in
`vrun-ce690e5f-5bec-4884-aba8-70aed278eaa7`, with zero skips. Missing-name
observations include TS2304 and TS2552, which differs only by the native name
suggestion. The genuine seven-test compiled final RED is
`vrun-0cc7d2d2-a1af-47d6-b3cf-385720e961dc`. Ordinary signature comparison is
now implemented and awaiting coordinated final feedback. Peer review corrected
raw this-symbol reads versus top-signature parameter reads. The shared
resolution, arity, predicate, annotation-return and signature-instantiation
owners now serve existing callers. Contextual generic inference and tuple-rest
factories remain pending; the
[inference/session design](types-native-inference-session-plan.md) records
their shared-owner continuation.

Index comparison retains a separate continuation. Root owns
`getApplicableIndexInfo`/`getApplicableIndexType` and canonical numeric-name
questions. The relation owner will consume those operations in native order,
including actual index before inferred member checks, target Any skipping,
Interface versus TypeLiteral inference eligibility, and exact-optional
Undefined removal. Source inventory acquisition still precedes comparison
shortcuts. No property-name spelling heuristic or empty-inventory fallback can
replace those operations. Their native/final controls follow the shared index
API and remain an open delivery gate.

The composite-signature continuation now shares the complete
`compareSignaturesIdentical` worker. Its options independently select partial
arity, ignored this types and ignored return/predicate types. Existing Relater
clients retain their live Ternary/session callback. Standalone identity and
`findMatchingSignature` comparisons enter native identity or subtype sessions
for each type pair; a single artificial signature-wide session would change
that boundary. `compareTypeParametersIdentical` maps target constraints into
the source domain and deliberately skips defaults. Both constraint comparisons
use raw `getConstraintFromTypeParameter`, as native does, rather than adding
the guarded constraint operation's base walk. `getTypeAtPosition` now owns the
canonical Any answer for a proven absent position; failures remain failures.
The extension passed bounded adversarial source review. Final composite
inventory controls and coordinated compilation remain required for this stage.


Conditional/Substitution integration checkpoint (2026-10-11): native conditional
controls measure 20 queries in strict, exact-optional, and loose profiles
(`vrun-25acb34e-54fa-4d4d-943f-9c94b079873d`, three tests, zero skips).
Conditional roots own distribution, tail evaluation, instantiation, and lazy
branch caches. Recursion identity uses the actual retained source node, while
root allocation and instantiation caches remain separate. The native checked
infer flag is provisional; a consumed constraint/default failure is retained
in that entry and cannot become a successful repeated check.

Conditional inference reuses the active relation comparer. Instantiated
anonymous/reference/tuple factories acquire the outer arguments they retain,
so referenced infer slots are fixed before an instantiated type escapes.
Unused phantom infer slots are not forced. Any failed slot poisons unfinished
slots with the original error; an actual late unresolved callback still fails
the existing retired-session invariant. Member peer review found no escaped
unfinished-slot path in these producers. Summary extends clauses replace and
restore the node-builder infer list, and graph branch operands stay lazy until
admitted. Mapped producers remain the next coordinated implementation window.

The first integrated Conditional final run (`vrun-0595700e`) was genuinely red:
all six groups reached the retained infer scope through a token span while the
binder keys that scope by the full native node span. The repair projects the
captured node through the existing `type_node_declaration` and
`declaration_ordering` owners; both root construction and outer-parameter
frames consume that single `infer_type_parameters` operation.

Inline summary controls now cover array infer precedence, explicit constraints,
constrained inferred function returns, rest constraints, inferred template
constraints, nested conditionals, a resolving function check, and tuple
constraints. The pinned compiler asserted all eight queries in three profiles
(`vrun-fbc8432a-eeb4-4d8c-a66b-cf5f14363d5f`, three tests, zero skips). The
summary owner separates node-builder infer scope from printer extends context;
parenthesization consumes the rendered child's acquired constraint-presence
fact instead of repeating the semantic constraint/identity questions.
