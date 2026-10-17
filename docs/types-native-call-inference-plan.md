# Native generic and contextual calls

The operating envelope is one accepted immutable Project, the retained canonical
binder, captured source attachments and one request-local evaluator. This lane
extends the existing call/signature/contextual operations to the full selected
profile. Its first complete new consumer is explicit generic type arguments;
argument inference, contextual callbacks and retry then consume the same owners.
These are implementation increments, not delivery exclusions or a new reduced
profile. Public delivery still requires the complete baseline, relationship,
recovery and resource gates.

Two designs were compared using an explicit dependent default, a rejected first
constraint with a healthy later overload, and inferred literal pairs. A local
map from parameter spelling to supplied argument would be small but would lose
canonical parameter identities, merged defaults, cloned local parameters,
constraint substitution, ordered default placeholders, inference fixation and
actual signature-cache identity. The chosen design extends the existing native
mapper/signature owners and gives inference one request-local operation with
canonical slots. The Source layer supplies actual recipes/checking facts and
never performs substitution or reconstructs a symbol from a name.

The explicit argument path ports the complete sequence in `chooseOverload`:
type-argument count, effective argument count, `checkTypeArguments`, actual
signature instantiation, generic-rest arity recheck, and applicability using
the selected relation/check mode. `checkTypeArguments` returns an established
argument vector, ordinary rejection, or a consumed failure. A rejected
constraint may select a later overload; it does not itself create an invalid
product answer. An actually consumed missing constraint/default stays its
original failure. Source type-argument checking still precedes reordering and
candidate selection. The entire argument vector is filled before the
constraint loop, but that loop visits only actual supplied type-argument nodes.
Filled defaults do not cause omitted parameter constraints to be checked.
Conversely a needed default is acquired before an earlier supplied argument
can reject a candidate's constraint. `getMinTypeArgumentCount` observes syntax
presence without demanding defaults.

`fillMissingTypeArguments` belongs to the existing generic mapper owner and
must also replace the TypeReference caller's inline copy of that same question.
Missing slots start as native errorType placeholders, then defaults are acquired
and instantiated in parameter order against the evolving vector. A later
forward reference consumes that placeholder even without a diagnostic. The
TS default is Unknown; the distinct admitted JS implicit-any policy must remain
explicit when JS semantics are implemented. Existing constraint/default
question owners, actual merged declaration lists and their failure provenance
are reused. Passing every argument skips unused defaults.

The existing `instantiateSignatureEx` currently implements non-erasing member
instantiation. The same owner must accept the native erase flag. With erasure,
the supplied mapper replaces original type parameters and the resulting
signature has no local type parameters. Without erasure it allocates all fresh
local parameter identities first, composes their mapper, and retains separate
lazy constraints/defaults as it does today. Both paths preserve separate
parameter graph/read/display/call questions, predicates, return question,
flags, actual declaration provenance and actual SignatureReturn identity.
No arbitrary mapper-based signature interner is introduced:
`getSignatureInstantiationWithoutFillingInTypeArguments` alone owns native
`(SignatureId, ordered canonical type-argument list)` caching. Independently
instantiated member signatures remain distinct.

The subsequent inference owner retains actual `InferenceInfo` slots keyed by
canonical type parameters, candidate/contra-candidate order, priority, top-level
and fixed status, inferred/fallback results, implied arity and intra-expression
sites. It owns `newInferenceContext`, `inferTypeArguments`, `inferTypes`,
`getInferredType` and fixing/non-fixing mapper callbacks. An inference mapper
delegates to this operation through the existing mapper machinery; a frozen
spelling map or eagerly replaced type vector cannot model fixation. Native
union/intersection construction, comparison and constraint operations remain
shared. Ordinary False is data; unsupported, consumed invalidity and resource
limits are failures. Native relation Maybe/cache behavior is not guessed.

Call resolution must retain mutable native candidate/check-mode state across
Subtype and Assignable passes. A context-sensitive first pass skips susceptible
arguments, then retries inference/applicability with Normal mode at the actual
native transition. Context-node/type and inference stacks are installed by
the complete contextual-check operation and restored on all exits. The existing
reference-narrowing owner uses that same active context. Return-context
inference clones the outer context, respects binding-pattern exclusions and
uses its distinct return mapper. Generic-function deferral, intra-expression
sites and omitted/spread/rest arguments remain actual scheduled questions,
not syntax screens or blanket exceptions.

The complete native CheckMode decision needs one shared owner as those consumers
arrive: Contextual is not cacheable Normal, Inferential preserves generic
reference types, SkipContextSensitive and SkipGenericFunctions have separate
transitions, and TypeOnly/RestBindingElement/ForceTuple are distinct questions.
Current boolean mode DTOs cannot silently define a second interpretation of
those native flags. Calls, contextual checking, expression caches and reference
narrowing must consume the same established mode facts.

Recovery is part of this same operation. `getCandidateForOverloadFailure`
checks deferred syntax, chooses longest/generic candidates or combines
parameter types and return intersections, and installs its native recovery
candidate before diagnostic reentry. The recovery signature, resolving
signature, any signature and silentNever signature are distinct identities.
An arbitrary product failure cannot establish the native resolved property.
Actual temporary loop resolution restores prior cache state; an independently
installed inner signature wins before restoration. Error-candidate reentry and
default/constraint child scheduling require final native controls rather than
assuming that all diagnostics make every root invalid.

Pinned owners are checker.go `chooseOverload` (9001),
`checkTypeArguments` (9198), `inferTypeArguments` (9365),
`getSignatureInstantiation` (19198),
`getSignatureInstantiationWithoutFillingInTypeArguments` (19223),
`createSignatureInstantiation` (19233), `fillMissingTypeArguments` (21859),
and inference.go `newInferenceContext` (1217),
`newInferenceContextWorker` (1239), `getInferredType` (1283) and
`getInferredTypes` (1372), at TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

The candidate compiler fixture is `fixtures/types/native-generic-calls`.
Fresh research in `.cc/temp/types-generic-calls-{answers,phases}.json` and
`types-generic-calls-added-answers.json` establishes explicit intrinsic/literal/
union arguments, ordered dependent defaults, constraint rejection followed by
a healthy candidate, broken-but-unused defaults/constraints, original missing
inputs, forward-default errorType and initial inference outputs. The registered
compiler file `tools/type-native-generic-calls-oracle.test.mjs` must pass before
public expectations are promoted. Earlier raw captures precede the extra
schedule variants; their input hashes identify their exact history, and they
are not a final shipping gate.

Additional scratch-only controls in
`.cc/temp/types-generic-calls-schedule-answers.json` establish the two scheduling
dimensions before final fixture promotion: an omitted defaulted parameter with
a missing constraint remains healthy number without 2304; a needed missing
default is consumed before rejecting an earlier supplied constraint and yields
the actual MissingDefault errorType with both 2304 and 2344. These four fresh
strict/loose observations do not substitute for the registered compiler and
final Project controls.

The extended registered compiler file passed in
`vrun-8828dea6-0eb5-421a-a229-14d24e0a1553`: two executed tests, zero skips,
52 fresh healthy controls and 36 same-checker phase observations across the two
profiles. The previous 50/32 matrix passed in
`vrun-885f4613-972d-4820-a7e2-73e747adfc14`. These are compiler baseline receipts;
`tests/type_generic_calls.rs` is the final native behavior test. Its compiled
assertion RED is `vrun-1c39add4-ae93-429d-9d14-6258ee09c1fb`: five executed tests,
five failures at the existing explicit pending generic-call operation. The
missing type-argument assertion already reaches its original invalidity before
the next missing-argument assertion stops at that pending operation. This is a
behavior RED rather than a missing-import/setup result; native implementation
and its final GREEN remain required.

The final Project test will assert exact Found graph roots for explicit
substitution/defaults and later-candidate selection; OverloadRecovery for all
rejected constraints/applicability; original UnresolvedSymbol at actually
visited type/argument/constraint/default sources; and ErrorType at the consumed
forward-default placeholder. Strict and loose options remain paired. It must
compile and fail on the missing operation before production mapper/call changes.
The existing scalar overload gate does not substitute for this RED. Inference
and contextual callback variants then extend that same final contract with
their own complete-operation controls and compiled behavior REDs.

The current inference-boundary checkpoint (`vrun-4281b23e-25e1-44d4-9cd9-0c7682851e1f`)
passes all ten variance groups. Its index failures separate native global Object
inventory consumption from the weak-target prerequisite, and its two late-slot
cases reach the missing ordinary `inferTypeArguments` owner. The natural
same-query failure/recovery test reaches the duplicate outer-parameter ancestry
screen (`vrun-a5caf171-c9b3-4344-af3d-95073a40bb38`). The outer-parameter recipe
and evaluator operation are now shared by declared targets and anonymous
instantiation, with the native includeThisTypes argument; contextual frames
remain actual semantic questions in that operation.

The expression integration uses explicit CheckMode arguments, not a mutable
ambient mode. Native recursive expression calls sometimes deliberately restart
Normal checking, so inheriting one dynamic mode would change which generic
references narrow. `check_expression` remains the Normal convenience operation;
`check_expression_ex` owns actual mode dispatch. Identifier/access read setup
receives that mode, while flow assignment questions retain Normal. Contextual
checking owns the dynamic context-node/type and inference stacks separately:
those are native dynamic state, and their push/pop spans literal-context
regularization as well as expression checking. Source supplies actual ancestry
for getInferenceContext; source span containment cannot replace node ancestry.

Call candidate state owns the mutable argument mode across both relation passes
and retries. Inference contexts own fixing and non-fixing mappers, NoDefault
clones, the separately inferred return mapper, cached outer return mapper and
intra-expression sites. Mapper fixation consumes sites before clearing unfixed
inferences; every failure poisons unfinished slots before cleanup. Calls do not
reimplement candidate collection or constraint/default substitution. A retained
lazy mapper continues to read installed slots after call selection returns.

The current ordinary-call compiler baseline is
`vrun-e6374ae2-aad3-4b68-be92-55600bf8e219`: eight fresh cases in
`tools/type-native-call-inference-oracle.test.mjs`, zero skips. It measures
literal and pair inference, a nested call's return context, rejected generic
overload selection, eager versus short-circuited default acquisition, receiver
inference and unused optional parameters. `tests/type_call_inference.rs` retains
the final Project assertions. All eight final tests pass in
`vrun-30e39073-a9a9-4bec-b8a1-989d9ffbd5d8`. The natural same-query lifecycle
test also passes in `vrun-56648a1e-ced4-495b-8406-8f37ff5e8b89`: failed,
healthy and repeated property reads retain their actual
order, one original failure, and a healthy intervening relation.
The boundary fixture now provides explicit standard globals because the native
reverse relation actually consumes Object's inventory. Its revised compiler
baseline is `vrun-42ff2af8-c929-450c-a767-d15428ed4d05`: all 80 observations
across four profiles pass without changing the expected boundary answers.

Adversarial review verified ordinary call demand order and found one dynamic
lookup distinction: `findContextualNode` selects the oldest matching type
frame, while `getInferenceContext` selects the newest matching ancestor frame,
including nil shadowing. The implementation keeps those operations separate.
The native deferred generic-call result also remains separate from an ordinary
signature and a failure: it retains the resolving cache property and produces
silentNever for this check. Context-sensitive function/object producers,
higher-order signature promotion, rest/spread traversal and full recovery
construction remain required next work; this checkpoint does not declare the
complete call profile delivered.
