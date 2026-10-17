# Native inference and relation sessions

This extends [the generic-call plan](types-native-call-inference-plan.md) for
the same immutable Project and request-local Evaluator. It does not introduce
another inference implementation. The immediate compiled consumer is
`tests/type_structured_signatures.rs`, RED
`vrun-0cc7d2d2-a1af-47d6-b3cf-385720e961dc`; its pinned native controls pass in
`vrun-ce690e5f-5bec-4884-aba8-70aed278eaa7`. Feedback
`vrun-9ead948c-6595-4a95-b48b-afe9492a047b` passes six final signature groups;
the contextual generic group remains RED. The five existing call, effect,
signature, return and signature-mapper suites pass in
`vrun-02a7e0b1-9bd4-4274-929a-cfc1e272b822`.

The controlling native chain is `signatureRelatedTo` →
`compareSignaturesRelated` → `instantiateSignatureInContextOf` →
`newInferenceContext` → `getInferredTypes`/`getInferredType`. The context receives
the live relater's comparer. Constraint/default instantiation can reenter that
comparer through a non-fixing inference mapper. A RelationKind is insufficient:
the comparer must retain Ternary, the current maybe keys, stack depths,
intersection state, allowance and overflow provenance. Independent native
`isTypeAssignableTo`/Subtype questions inside candidate selection still start
their own sessions. Sharing every inference comparison with its caller would
also be wrong.

Two ownership designs were compared. Passing a borrowed comparer context
through `instantiate_type` and every generic mapper consumer makes the current
stack lifetime explicit, but forces a signature-specific dependency through
ordinary alias/member/default instantiation. Inference mappers retained by lazy
anonymous factories cannot store that short borrow. The preferred design gives
RelationsState a private active-session arena and typed RelationSessionId.
Inference contexts store an explicit comparer operation: ordinary Assignable,
or that exact session plus IntersectionState. Mapper representation retains an
opaque InferenceContextId; Model does not own candidate state or call back into
Evaluator. The existing mapper operation dispatches to the inference owner.

An active session is never temporarily removed to obtain a mutable borrow, and
no RefCell guard spans another semantic operation. Relation methods pass its
typed handle and borrow individual state fields only around local observations
or updates. Nested independent relations allocate a different handle and share
only the existing relation-kind cache. A monotonically unique request-local
handle prevents stale references from naming a reused slot. Only active
sessions occupy the arena; no completed session pool is needed for correctness.

The relation entry/exit operation owns allocation, native root overflow
installation and retirement on every Result path. Existing recursive cleanup
still restores stacks, expanding flags and uncommitted assumptions before an
error returns. Completed inner native cache installs, including Failed entries
with overflow provenance, survive. An error does not commit unfinished Maybe
keys. Moving storage must not change any current commit or allowance schedule.

Inference contexts have a longer request-local lifetime than a stack borrow.
The relation-specific constructor is nevertheless operation-scoped: native
`instantiateSignatureInContextOf` resolves every slot with `getInferredTypes`
before returning `getSignatureInstantiation` with an ordinary type vector.
Its own inference mapper used by constraints/defaults is non-fixing. Such a
mapper may remain attached to a lazy type, but later reads see installed slot
results and need no relater callback. The context's fixing mapper is not
published by that entry point. Native `putRelater` immediately resets the
relater after the root comparison, confirming that an escaped unresolved
callback cannot legitimately resume that invocation. Retained contexts must
therefore preserve all completed slot values or their original product
failure; they must not silently create a replacement relation. An attempted
uncached comparison against a retired handle is an invariant error. Call-site
contexts whose comparer is ordinary Assignable have their separate native
lifetime and remain usable through contextual checking and retries.

The invariant applies to uncached comparison, never mere mapper access. Late
non-fixing reads may observe an installed provisional result or original
failure. If constraint work fails after provisional installation, the owner
replaces that slot with the original failure and retains it for every unfinished
slot in the context before retiring the relation. A late mapper must not replace
that semantic failure with a retired-session error.

The inference owner retains native candidate order and depth, contra-candidates,
priority, top-level/fixed status, inferred/fallback values, implied arity,
intra-expression sites and traversal state. Fixing a slot consumes pending
intra-expression sites and clears only unfixed cached inferences. Resolving an
inferred type clears native active-mapper caches; immutable mapper interning is
not a substitute. The existing generic owner must gain that active-cache
operation and native instantiation limits before dynamic mapper callbacks can
be considered complete. No eager number/string substitution or signature
erasure can stand in for these decisions.
Active caches belong to each instantiation frame; clearing them preserves
committed ordinary instantiation caches. Mapper factories allocate fresh native
identities; equal mapping contents do not identify an active frame.

Integration preserves `applyToParameterTypes` source/target counts and rest
questions, raw this types, positional pairs and target-rest construction.
`applyToReturnTypes` asks the target predicate first, then the target return;
the source return is visited only if that return could contain variables.
`getInferredType` installs its provisional result before constraint substitution,
uses the context comparer for constraint acceptance, and preserves return-only
candidate filtering and fallback selection. Common supertype/subtype,
literal widening, union/intersection, constraints and native type flags stay
with their existing owners. Mapped/conditional/tuple/variance inference remains
required shared-factory work, never a successful no-inference approximation.

Before the storage move, adversarial review must check reentry, late non-fixing
mapper reads, error unwinding and cache installation. Final controls then cover
parameter versus return priority; literal/primitive/const constraints;
co/contravariant conflicts; dependent defaults and recursive constraints;
missing constraints skipped by erasure but consumed by context inference;
callback fixation and contextual retry; a late lazy member read after relation
completion; and failure during a nested comparer followed by an independent
query. Existing active-Maybe/False and depth 99/100/101 consumers must remain
green after the session storage move. Wider native inference and resource
gates remain mandatory after those cases pass.

The property/member and algebra owners adversarially reviewed this design.
Both accepted the arena boundary with the late-slot failure and per-frame cache
rules above. Their required final controls remain in the implementation gate.

The first implementation checkpoint moves active Relater storage behind that
handle and adds persistent inference slots. Contextual signature instantiation
now collects direct parameter/return candidates, resolves defaults and
constraints in slot order, and publishes ordinary signature type arguments.
Candidate depth/priority, provisional installation, original-failure poisoning,
primitive/const widening, common-supertype/subtype choice, return-only filtering
and the captured constraint comparer are owned by `evaluation/inference.rs`.
The generic instantiation entry owns native depth/count checks and per-active-
mapper caches. Model mapper factories now allocate fresh native identities;
ordinary factory caches alone decide reuse. The expression owner resets the
per-expression count at native `checkExpressionEx`, without restoring it.

This checkpoint is not the full inference operation. Alias/reference variance,
matching composite constituents, object/member/signature/index traversal,
rest/tuple construction and contextual call fixation/retry still have explicit
pending dependencies. Their native factories and final controls remain required.
The existing contextual-signature RED is the first compiled feedback gate;
late lazy member reads, nested failure cleanup and the wider candidate matrix
must be proven before this increment can be considered complete.

The first compiled checkpoint passes all seven structured-signature groups in
`vrun-3b27955f-c543-4b0a-aa23-e08ad04a6665`, with zero skips. The ten-file
compatibility batch passes in `vrun-bd280555-4b6c-4e1e-9a0c-646946a2f151`,
covering structured relations, relation limits, calls, effects, signatures,
return bodies, signature mappers, member values, generics and this types.
These results establish the existing direct contextual-generic consumer and
preserved relation behavior. They do not prove the wider pending traversal or
replace the additional inference lifetime and candidate controls above.

The next native fixture, `fixtures/types/native-inference-sessions`, passes in
`vrun-18a1056b-dea5-44a2-9a53-f3c0594e834e`: two executed profile tests, zero
skips, seventeen selected-overload consumers per strict/loose-null profile.
Each has fresh public output and same-checker dependency observations. Cases
cover parameter-versus-return priority, return unions and constraint filtering,
ordinary constraint rejection, dependent/forward defaults, consumed and skipped
missing defaults, a consumed missing constraint, primitive/const parameters,
nested objects, reference arguments, callback inference, conflicting variance
candidates, matching unions and a recursive constraint. The literal cases are
admission controls; fresh-literal widening still needs the ordinary generic-call
consumer because contextual signature annotations already provide regular
literal types. `tests/type_inference_sessions.rs` promotes those measured
outcomes into seventeen final Project tests.
These cases do not yet establish the late lazy-mapper lifetime controls.

Compiled feedback `vrun-bb674fdd-0ff5-4fd7-b480-b7cf981a6b06` executes all
seventeen final tests: twelve pass, five fail. Nested objects, callbacks and
conflicting candidates reach the explicit object-traversal dependency;
reference arguments reach variance; matching unions reach composite matching.
The passing cases include original missing-default/constraint provenance and
the recursive constraint's captured comparer. This is the genuine RED for the
next traversal stage, not a compilation failure.

The next traversal stage keeps the existing inference pass alive through nested
members and signatures. The top-level contextual signature pairs each call
`inferTypes`, but nested `inferFromSignature` invokes the same parameter/return
application schedule with the current priority, variance, visited pairs and
recursion stacks. Reusing the top-level entry would silently reset that state.
The signature application owner will hide the shared demand schedule while its
two callers choose whether to begin a pass or continue the current one.

`getUnmatchedProperty` moves the whole current relation unmatched pass into one
shared operation. Its discriminant mode first reads the target property type,
then reads the source only for a Unit target; ordinary relation mode performs
no such reads. `isObjectTypeWithInferableIndex` uses actual Type.symbol flags,
then the native call/construct exclusion and actual object-factory flags. A
type's printed key or object kind cannot establish either decision. Index
inference and index relations share those owners plus the root-owned
`getLiteralTypeFromProperty` and applicable-index operations.

Reference and alias inference require the actual variance protocol. Declared
in/out flags and the native array/tuple covariance shortcut remain distinct
from computed variance. Computation installs the native empty recursion result,
compares real marker instantiations through Assignable relations, retains
reliability observations, and restores the resolution-start window on every
exit. Structural guesses about how a parameter appears cannot replace that
owner. Composite matching likewise requires native Identity; testing both
assignability directions does not answer the same question.

The traversal stage is implemented. Compiled checkpoint
`vrun-fdfefa6a-0ba8-441f-8ea4-c217b7fd85a0` passes all seventeen final inference
tests with zero skips. Compatibility checkpoint
`vrun-dca01a1f-e6c5-4181-beb3-f6c5f14a2586` passes eleven related test files,
including structured relations/signatures, calls, generic calls, signatures,
mapper behavior, effects, this, heritage and declaration order. It includes
target-major composite matching through Identity,
native unmatched-property pruning, recursive `invokeOnce` priorities and stacks,
property/callback/index inference, base-signature constraint erasure, and measured
reference/alias variance. Relation cache entries retain reliability observations
alongside their existing overflow provenance; marker measurement restores both
the resolution window and reliability state on dependency failures. Ordinary
index relations now share the same canonical property-name and index owners.

The constructor integration preserves separate native return lifetimes: fresh
default constructors install their class return immediately, while explicit
constructors obtain class local parameters after parameter construction and
retain a lazy return annotation question. `Source.symbol_signatures` owns native
declaration order and adjacent implementation exclusion for function, method and
constructor inventories. Accessor serialization keeps its narrower first
matching declaration question. Algebra and member peers reviewed the factories,
signature list, traversal, Identity and variance schedules without a concrete
blocker in the admitted factories.

Index-name variants, measured variance polarity/reliability, late mapper lifetime and nested
failure recovery still need their expanded final controls. Tuple/mapped and
deferred-reference producers, service edited-node inference markers and ordinary
generic-call contextual retry remain required later owners; this stage does not
claim that they are complete.

Follow-up controls use final overload selection for index inference: numeric
declarations and numeric strings both apply to a number index; noncanonical
numeric-looking strings do not; computed literal names use their actual name
type; instantiated alias properties retain their original declaration name;
optional values distinguish exact-optional missing removal from ordinary
undefined removal. An excluded property's missing annotation must remain
unread. Public and private class instances both establish the native
non-inferable-index class exclusion; they do not falsely claim to exercise the
separate nonpublic-name filter, whose reachable producer requires later mapped
or rest work.

Variance controls distinguish measured covariance, contravariance, invariance,
method bivariance, independence and recursive references, alongside explicit
in/out overrides. Non-array rest reliability and mapped optional-removal
unmeasurability must retain final RED controls until their respective factories
and traversal are complete. A failure envelope expands a consumed variance
property whose value reaches a flow-sensitive initializer, followed by a second
independent healthy relation and a repeated failed edge in the same evaluator.
The product's declared FlowRequired partial-result contract makes cleanup
observable. An unresolved-name failure instead aborts the whole request; its
separate control proves original failure provenance, not later-session recovery.
The late successful mapper read needs the ordinary generic-call consumer, for
example a zero-argument generic call whose inferred constrained object retains
a non-fixing mapper for a later defaulted parameter; a selected scalar overload
alone cannot prove that lifetime behavior.

The maintained follow-up compiler controls are now GREEN:
`vrun-d3b923e7-e114-4aa2-9ded-dae741d37ebe` passes five tests with zero skips in
`tools/type-native-inference-boundaries-oracle.test.mjs` and
`tools/type-native-nominal-relations-oracle.test.mjs`. They measure eighty
boundary observations across strict, loose-null, exact-optional and bivariant
function profiles, plus seventeen nominal comparisons, using fresh public and
same-checker queries. `vrun-e58747c4-d120-40c8-ab44-386385c4fe1b` separately
passes the natural narrowed-initializer lifecycle control. Their final consumers
are `tests/type_inference_boundaries.rs`, `tests/type_inference_lifecycle.rs` and
`tests/type_nominal_relations.rs`; the coordinated Rust checkpoint is pending.

The nominal comparison increment consumes the existing class test's genuine
protected-override RED. `propertyRelatedTo` retains private value-declaration
identity and then delegates protected targets to native `isValidOverrideOf`.
The property arena owns actual declaring-class identity and underlying-property
traversal; relation policy iterates protected target origins before source
origins and uses the shared `hasBaseType` owner, including its nil-target
traversal. Peers reviewed both comparison order and raw-parent retention. No
generic-call inference behavior was added with these controls: the late-result
tests establish the next required dependency rather than bypassing it.
