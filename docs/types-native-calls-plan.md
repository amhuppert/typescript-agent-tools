# Native call resolution and return cycles

The operating envelope is the existing request-local native Evaluator over
captured accepted sources and the retained canonical binder. Calls use its
shared symbol, type, member, signature and flow history. TypeScript 7.0.2 at
commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b` owns expected behavior;
there is no production compiler process, independent lexical binder or
execution of application functions. Full call/new/tag, overload, inference,
contextual and relation obligations remain required for semantic delivery.

## Design twice and sequencing

Selecting the first or sole call signature and demanding its return is native
behavior for `getQuickTypeOfExpression` when its actual exclusions and
single-nongeneric inventory conditions apply. Variable initializer acquisition
asks that question first and may skip arguments, arity, this and parameter
annotations. The same shortcut would answer a different question inside
`checkCallExpression`, whose single nongeneric resolution path still checks
type-argument absence, semantic arity, explicit this and contextual argument
applicability. An omitted required void parameter can be accepted; a checked
call's required error annotation is a consumed invalid dependency; an unused
optional error annotation is not. The deep expression operation must preserve
this schedule rather than make callers coordinate exceptions.

A separate call checker with its own AST/symbol state would hide those details
but repeat source identity, signature inventories and resolution history.
The chosen boundary extends Evaluator with the complete native call operations
and one actual call-site cache. Source captures syntax recipes; the evaluator
owns choice, contextual checking, relation outcomes, return demands and
recovery provenance. Existing read/end-flow effects and public checking
preludes use the same getResolvedSignature operation when native consumes it.

Mutual inferred functions currently stop at the absent call expression owner
before SignatureReturn reentry. Fixing a generic demand cache at that point
would not prove native return cycles. Implement the real ordinary call
consumer first under its relevant compiler/final Project controls; then the
same consumer drives actual Signature/ResolvedReturnType stack tests. The
native direct bare-self-call exclusion is a separate control and remains
healthy Never. A recursive typeof return annotation can be healthy because
the function's value type is already established; it is not automatically a
return-cycle recovery.

## Complete native owners

Pinned checker.go defines `getQuickTypeOfExpression` (7338),
`getReturnTypeOfSingleNonGenericSignature` (7364),
`checkDeclarationInitializer` (16710), `checkCallExpression` (8298),
`getResolvedSignature` (8383), `resolveSignature` (8429),
`resolveCallExpression` (8447), `resolveNewExpression` (8551),
`resolveCall` (8819), `reorderCandidates` (8933), `chooseOverload` (9001),
`hasCorrectArity` (9083), `checkTypeArguments` (9198),
`isSignatureApplicable` (9232), `getCandidateForOverloadFailure` (9474),
`resolveUntypedCall` (9878), `resolveErrorCall` (9899),
`isUntypedFunctionCall` (9909), and `getApparentType` (21634).
Relater.go `getMinArgumentCountEx` (1708), `hasEffectiveRestParameter`
(1749) and `getTypeAtPosition` (1757) own semantic arity dependencies.

Source adds one exact call recipe operation using the existing captured
kind/span node_at attachment. It retains the actual callee, actual argument
and type-argument lists, optional-chain/checking-role facts, missing/list-end
facts where native asks them, and location. Acquisition does not type an
argument, choose a signature or fail because a captured argument recipe is
not implemented. Expression Call recipes retain the actual file/site;
Source's existing expression projection remains the only expression owner.
New/tag/decorator/JSX/instanceof dispatch preserve their distinct native
questions and explicit dependencies instead of pretending to be ordinary
calls.

The enclosing-call prelude retains actual file/kind/raw-span call attachments
from the same Source owner, so no evaluator reconstructs syntax identity from
printed positions. It preserves outermost-to-innermost order across every selected
role. Candidate collection remains metadata-only. Prelude resolution does not
independently demand a chosen signature's return: checkCallExpression does so
after resolution; resolveCall checks parameters/arguments in its native order.

### Executable checking prelude

The executable consumer replaces the temporary first-call refusal with
`check_site_prelude(&[CallRoot])`. Source's existing containment owner records
actual accepted call/new/tag attachments outermost first, and selection carries
those opaque identities through every supported role. Source does not acquire
a recipe or type while collecting candidates. The evaluator acquires each
recipe and runs its existing whole `getResolvedSignature` operation; it does
not call `checkCallExpression` or demand the selected signature's return just
because a subnode is selected. Failures stop the prelude before the root.

Designing this twice exposes two unsuitable alternatives. Keeping Locations
would make the evaluator reverse-map public positions into syntax, repeating
attachment/role decisions. Calling `checkCallExpression` would hide useful work
behind an easy API but add a return demand absent from the specified compiler
history. The chosen operation hides both actual syntax acquisition and native
signature-cache scheduling behind a complete site-prelude boundary. No new
checker, public testing API or unrelated whole-file pass is introduced.

Fresh controls in `fixtures/types/checking-prelude` cover exact nested roots,
renamed cross-file callees, expression/value/declared/type roles, and a selected
literal inside a function whose unused return annotation is MissingReturn.
That literal stays healthy after signature resolution; selecting the call
result separately consumes the return. The same-checker schedule distinguishes
an actually consumed parameter error and rejected arity/this/argument relations
from undemanded return annotations. Strict/loose null applicability is pinned
separately. Generic/context-sensitive/New/Tag branches continue through the same
complete dispatch and preserve their precise dependency failures until those
mandatory owners are implemented; they are not recast as healthy subnode
answers. The temporary refusal tests were revised after compiled four-test RED
`vrun-3cd019b7-c3a0-4d64-ad38-da13aca60a9b` and corresponding final GREEN
`vrun-42b95fec-d9f6-4dd5-b1d1-5fa08622e1a8` proved the new operation.
[types-checking-prelude-evidence.md](types-checking-prelude-evidence.md) records
the final three-test native baseline, compatibility gates and remaining owners.

checkDeclarationInitializer first calls the whole getQuickTypeOfExpression
owner. Ordinary calls exclude super, require, import and global Symbol calls;
new/await/assertion/literal branches keep their native questions. The call fast
path checks the non-null callee, obtains the actual sole nongeneric signature
with members allowed and demands its return without installing a resolved-call
cache or visiting supplied arguments. Optional chains retain their separate
propagation operation. An actual inferred return body uses
checkExpressionCachedEx and therefore reaches checkCallExpression instead.
These operations cannot share a blanket eager-call policy.

Evaluator's call cache distinguishes absent, native resolving marker,
established resolved signature and actual product failure. It is keyed by the
unique final call attachment in this request. getResolvedSignature saves the
current type-resolution window; only an initially uncached call resets its
start to the current stack length. It installs the resolving marker before
resolveSignature, restores the window afterward, respects a nested installed
result, and follows the native flow-loop cache suppression rule. Neither a
generic Resolving error nor a syntactic "inside a loop" boolean substitutes
for these protocols. The first ordinary initializer/body consumer has no
temporary flow-loop history; later flow consumers require the real native
state before that cache condition is generalized.

resolveCallExpression checks the callee first, then optional/non-null,
apparent-type and signature inventory dependencies. Error calls still visit
their actual type arguments/arguments through resolveUntypedCall. Genuine Any
uses the untyped path, which checks those expressions without contextual
parameter types. Unknown is not Any: loose getApparentType changes its type,
and the untyped branch can consume globalFunction subtype dependencies.
Primitive wrappers, constraints and composite inventories retain their real
global/member/reduction dependencies, using one shared apparent/signature
owner with effects rather than a duplicate special-type table.

resolveCall checks actual type arguments before candidates, reorders native
declaration/symbol groups and literal-specialized signatures, obtains effective
arguments, and performs native subtype then assignability passes. The single
nongeneric path still executes hasCorrectArity and isSignatureApplicable.
Arity owns initial signature minimum, native rest/tuple effective count,
trailing/incomplete-call facts and the backwards Void minimum reduction.
Parameter.optional is presentation data and cannot universally substitute for
the compiler's initial/effective counts. The constructor must retain the
actual signature facts, with minimum-question caching on native signature
identity. Ordinary own declarations can use their canonical SignatureRoot;
instantiated/composite/synthetic signatures require distinct actual identities
and target/mapper facts, not origin or return-type equality.

The existing signature factory is being extended by its owner with canonical
SignatureId, actual local parameter TypeIds and minimum/flag facts. Calls
consume that operation; their cache does not allocate competing signature
identities. ParameterCall/ThisCall demands provide getTypeOfParameter's distinct
call_type handle, retained beside graph value_type and display_type. Actual
signature instantiation maps all three through the same native combined mapper.

Applicability checks actual this first, then each parameter type and its
argument under checkExpressionWithContextualType, then non-array-rest spread
relations. Zero arguments do not force unused optional annotations, but
semantic minimum computation may force a required annotation. A false native
relation rejects a candidate as valid data. Unsupported/invalid comparison
cannot be turned into false and skipped. Generic type arguments/constraints,
inference, context-sensitive retry and signature mapper construction keep
their whole native owners and checking order.

getTypeAtPosition asks getTypeOfParameter (16955), which adds optionality for
an initializer or optional declaration to getTypeOfSymbol's answer. This is a
third actual parameter question alongside graph symbol reads and declaration
display serialization; initialized required graph parameters cannot simply be
reused as contextual call-parameter types. Omitted argument acceptance also
uses the native filterType/acceptsVoid path after minimum-count reduction.

When all candidates fail, getCandidateForOverloadFailure and its helpers mark
InvalidSemantic/overload_recovery at the actual call. A single failed candidate
may display its original healthy return; multiple failed candidates can produce
Never by intersecting incompatible return types. Neither is an exact successful
answer. Genuine Any/Unknown remain distinct. Call checking's super/new/JS
require/unique-symbol/assertion branches and return-cycle recovery retain their
specific dependencies; unrelated deprecation/grammar diagnostics alone are not
global invalidity. No argument is visited early merely for convenience: too
many arguments can fail arity without checking a missing-name argument.

## First real consumer and final tests

### Overload selection and declaration provenance increment

The next consumer extends the same resolveCall/chooseOverload operation, not a
second function-dispatch table. `reorderCandidates` asks for actual native
declaration symbol and Parent identities, then moves specialized literal
signatures before ordinary ones. Consecutive declarations under one parent
retain their order; a later parent group for the same symbol moves ahead of
the earlier group, while a different symbol establishes a cutoff. This is
observable for merged script functions when the files are reversed. Actual
canonical SignatureId remains independent from these declaration facts.

Designing twice rejects reverse-matching displayed origins or treating all
signatures as one source group. Either would lose generic instantiation's
original declaration and script/namespace/member parent distinctions. The
chosen `signature_declaration_root(SignatureId)` operation retains explicit
original factory attachments, follows instantiated targets, and distinguishes
synthetic None from a missing invariant. Source then acquires the actual
getSymbolOfDeclaration/Parent question at call resolution, not at function
type construction. No parameter or return type is demanded to reorder a
candidate. SignaturesState owns this provenance operation with the existing
factory/mapper owner; no scan of origins or return-demand locations identifies
a declaration.

The pinned binder's `bindFunctionOrConstructorType` creates both a Signature
and an anonymous TypeLiteral symbol on the actual function-type node. Such
an unretained anonymous symbol is not native nil and cannot become Synthetic
None. For a proven single nongeneric candidate, reordering has no observable
order change; Source must establish that its native symbol acquisition has
no late-bound dependency before this bookkeeping can be omitted. Named and
reserved Call/Construct roles reuse the retained binder's actual declaration
anchor and canonical symbol. Multi-candidate anonymous identities require
the canonical anonymous binder owner; no origin-derived identity is admitted.
Dynamic member associations keep the `getLateBoundSymbol` dependency at the
existing getSymbolOfDeclaration boundary, using native ClassMember symbol
flags rather than a class-parent syntax guess.

After checking source type arguments, the call owner reorders candidates and
uses native effective arguments/context-sensitive mode. A sole nongeneric
candidate uses Assignable directly; multiple candidates use Subtype first,
then Assignable only if no candidate succeeded. Each candidate applies the
whole type-argument arity, argument arity and applicability questions in that
order. Ordinary False continues the loop. A genuinely demanded unsupported,
invalid or resource dependency stops it; such a failure cannot make a later
candidate appear selected. Parameter calls preserve their separate read
handles, including Void minimum reduction and explicit this scheduling.

The generic inference/instantiation and context-sensitive retry branches stay
in this same operation with exact pending dependencies until their full owners
are wired. Their syntax/check-mode facts cannot be guessed from printed types.
Canonical parameter defaults and mapper factories must be reused where native
asks their existing question. Optional-chain candidate clones keep actual
distinct signatures and their original declaration provenance.

When no candidate is applicable, entering getCandidateForOverloadFailure is a
consumed language recovery and therefore an InvalidSemantic operation. The
product must not publish its healthy-looking union/intersection return as
Found. Recovery-related diagnostic/result construction is not a reason to
erase a failure already encountered while checking source type arguments,
minimum counts, this, parameters or arguments. Full longest-candidate,
combined-signature, deferred checking and inference recovery owners remain
mandatory before general call delivery.

`fixtures/types/native-overloads` pins literal specialization (including an
escaped exact value), Any's subtype-versus-assignability choice, ordinary and
Void arity, explicit this selection, same-parent ties and reversed script
parent groups in strict/loose profiles. Same-checker controls distinguish
actually visited Missing argument/type inputs from skipped extra arguments
and overload failure. Final Project tests will ask renamed cross-file result
values and exact graph literals, invalidity locations and source-file order,
under a compiled scoped RED before factory/call production changes.

That final five-test RED is `vrun-a526af36-00c9-4d09-88a2-fecc77bb384b`:
five executed failures on the missing actual reorder/selection operation,
with zero skips. The earlier `vrun-101ab4a0-922d-454a-8433-81203bace15f`
is setup-only: it stopped at concurrent member-stage compilation errors.
The expanded pinned native two-test control is
`vrun-07a446ae-9099-4b9a-af2a-d11a04f6fd44`, with 56 fresh healthy roots and
12 same-checker recovery/scheduling observations, zero skips.

The final public five-test GREEN is
`vrun-22e2b3e9-8cd3-477d-a287-c0404cc5301e`, with zero skips. See
`docs/types-native-overloads-evidence.md` for exact outcomes, the bounded
anonymous-symbol premise, independent review and recovery/cache obligations
that this increment does not close.

The first native production slice routes ordinary CallExpression recipes in
existing variable initializers through the native quick-type owner and actual
inferred return bodies through full call resolution. Established nongeneric
function-only targets with no supplied arguments provide the first real leaf
path. This is incremental implementation,
not a new supported-profile restriction; other required branches remain exact
located dependencies until their complete owners are admitted.

`fixtures/types/native-calls` / `tools/type-native-calls-oracle.test.mjs` pin
strict/loose fresh demands for annotated/inferred/literal returns, unused
optional Missing annotations, omitted Void parameters, explicit Void this,
wrong argument counts/this/types, genuine Any and invalid callee/argument paths,
overload selection/recovery, generic instantiation, mutual inferred recovery,
bare direct-self Never and established recursive typeof annotations. Same-checker
before/after-summary observations pin actual demand diagnostics separately
from display. Single-signature result initializers have no arity/missing-type
diagnostics. Checked inferred-function roots initially establish their signature
without its lazy return; summary demand then resolves the body's actual call
and records its arity diagnostic. Mutual result-variable inference consumes the
functions' returns before summary and records their actual cycle then.
A direct native post-root signature observation distinguishes a later
error-typed parameter from the healthy quick-path result; it does not convert
the subsequent signature inspection into an earlier dependency. Missing's
Any alias prints `Missing` for a required parameter and `any` for the optional
parameter's distinct read; both are isErrorType true. The quick root's
subsequent inspections do not retrospectively consume that annotation. Actual
checked-body demand records the missing annotation's 2304 plus arity recovery.

Before production, scaffold new modules/DTOs coherently and run a compiled
single-file Project RED in `tests/type_calls.rs`. The first assertions should
cover actual renamed cross-file result types and graph nodes for fixed,
inferred, literal, unused optional, omitted Void and explicit Void this calls;
genuine untyped zero-argument Any. Contrast quick initializer healthy answers
with inferred-body OverloadRecovery for omitted required and extra arguments,
and consumed required-annotation Missing/missing callee failures at their own
source. Keep expanded overload/generic/context arguments
as required subsequent behavior cases, not tests freezing temporary Pending.

Then a separate final cycle file requests mutual inferred calls and expects
consumed CircularType even where native displays non-error Any, contrasts
healthy direct-self Never and established recursive annotation cache barriers,
and exercises both strict/noImplicitAny and loose modes. Signature return
push/pop uses the actual source/instantiated signature question, with explicit
installed-recovery states. Cause location is annotation.location when present,
else the native SignatureRecipe.origin (actual name or anonymous declaration),
without restoring the removed duplicate Infer location. Repeat call/return,
read/effect, presentation and lexical compatibility checks at coherent source
checkpoints. Full semantic/order/resource/corpus gates remain open.

## Current receipts

The registered fresh compiler file passed seven tests with zero skips in
`vrun-dbbb2ca9-77e0-49b3-a62c-a030ac1a88b3`. Earlier runs corrected research
expectations about loose Unknown, overload-failure intersection, quick
initializer acquisition, function-summary phase timing and optional parameter
error display; they are not product REDs. The five final Project tests in
`tests/type_calls.rs` compiled and failed on missing call behavior in assertion
RED `vrun-75a126e7-e65c-4e8e-999f-518a9f58007f` (five executed, zero skips).
The first native source/evaluator slice passed all five final Project tests
with zero skips in `vrun-60d75191-1286-4070-9678-ed656c64997b`. The native
baseline expanded to eight passing tests in
`vrun-1550dd61-3cbf-4cf9-a5e3-75d633921ce7`, including fresh own-function
mutual-return demands and their before/after-summary phase distinction.

The separate three-test cycle file compiled and failed on actual missing
behavior in `vrun-b0643be8-86c4-4963-bb67-68e27829115d`. After the canonical
SignatureId return-property protocol was installed by its owner,
`vrun-267245da-48b7-4fec-95b2-4a1730761ccc` executed all three tests with zero
skips: two passed, establishing consumed mutual recovery and exact participant
provenance across both option profiles and summary/expansion depths. The third
test passed its healthy bare-self Never assertions, then failed on the
recursive annotated function's summary: the tool printed `() => …` while the
compiler prints `() => typeof recursiveAnnotation`. That remaining failure
belonged to native function-symbol serialization, distinct from return
resolution. After its separate owner was implemented, all three tests passed
with zero skips in `vrun-65060c24-24f5-4b84-a73f-78438505bdca`; see
`docs/types-function-symbol-evidence.md` for the native/public display controls
and source-written-name correction.

The passing production slice covers the native quick path and full checked
ordinary calls with one nongeneric signature and no supplied arguments. It
does not close contextual argument checking, native relations, overload
selection, generic inference, new/tag/super/require/import/optional-chain
branches, temporary flow-loop cache suppression, or the full checking-order
and resource gates. Those remain required subsequent implementation.

## Contextual arguments and shared relation increment

The next increment keeps the same request-local operating envelope. It adds
the actual supplied-argument consumer for established nongeneric signatures,
then extends those complete operations as further type families are admitted.
This is implementation sequencing, not a reduced required profile.

Two designs were compared against calls, switch comparability and typeof
narrowing. A local primitive compatibility table in Calls would be initially
small but would duplicate null rules, literal freshness, relation kinds and
union direction when the other consumers arrive. It would also make an
unsupported comparison indistinguishable from an ordinary rejected argument.
The chosen `evaluation/relations.rs` owns native `isTypeRelatedTo`,
`isSimpleTypeRelatedTo` and the scalar/primitive-union branches of the Relater.
`RelationKind::{Assignable,Subtype,StrictSubtype,Comparable}` plus
`Evaluator::type_related(source,target,kind,consumer)` return `Result<bool,
DemandError>`; `are_types_comparable` asks the native ordered forward-or-reverse
question. Equal canonical identities are true even for StrictSubtype, matching
the pinned early return. False is established relation data. Unsupported,
consumed invalidity and resource failure remain errors. Calls and root-owned
switch/narrowing consumers do not make independent primitive comparisons.

The alternative contextual design was to regularize every literal argument
and call the ordinary expression cache. It asks the wrong question:
`checkExpressionWithContextualType` uses Contextual checking, a contextual and
inference stack, and strips freshness only for
`isLiteralOfContextualType` after contextual instantiation. The chosen complete
Evaluator operation owns that schedule. Its first scalar leaf branches need
no contextual lookup inside checkExpression; they still apply the exact native
literal-context decision. Structured/function/context-sensitive branches
remain located obligations until the same operation installs their real
context-node/inference state. An already established noncontextual structured
answer cannot silently substitute for checking that expression in context.
The normal `checkExpressionCached` site cache belongs to the root's separate
question and is not reused here.

Pinned owners are checker.go `checkExpressionWithContextualType` (7461),
`isLiteralOfContextualType` (25430), `isSignatureApplicable` (9232),
`getThisArgumentOfCall`/`getThisArgumentType` (9321 onward), and relater.go
`isTypeRelatedTo` (170), `isSimpleTypeRelatedTo` (206),
`unionOrIntersectionRelatedTo` (2849), `someTypeRelatedToType`,
`eachTypeRelatedToType`, `typeRelatedToSomeType` and
`getUndefinedStrippedTargetIfNeeded`. The current intrinsic/literal factories
cannot produce enum, wildcard, intersection, indexed, conditional or
substitution types; those owners remain required when the factories arrive.
TypeParameter and structured comparisons that reach their native recursive
branches fail explicitly until constraints, properties/signatures/indexes and
their relation/cache/Maybe/limit protocols are implemented. A native early
simple result must not eagerly demand an otherwise unvisited child.

Native primitive-union applicability checks every source alternative;
Comparable asks whether some source alternative relates, and target unions
search alternatives in their actual order. Boolean's public primitive spelling
retains its real false/true union. Nullable target simplification,
correspondence fast paths and native literal/base-primitive membership paths
belong to this owner; they cannot be inferred from displayed unions or replaced
by the different `isTypeSubsetOf` question. Full relation depth/allowance/Maybe,
object/generic/enum/inference behavior and aggregate work calibration remain
mandatory open work rather than new product admission limits.

`fixtures/types/native-call-arguments` and the registered fresh compiler file
pin positive number/string/literal/boolean/primitive-union/Any/Unknown/Void and
optional arguments, source-union direction, rejected primitive/literal/Never
arguments, strict-versus-loose Null, this-before-missing and
first-rejection-before-missing. Same-checker phase controls distinguish native
healthy displayed return recovery from a consumed overload failure, and show
that unrelated missing-global diagnostics in noLib do not invalidate every
root. Final `tests/type_call_arguments.rs` must compile and fail on missing
argument behavior before production. Expected final outcomes assert Found
graphs for successful arguments, OverloadRecovery for a rejected relation,
and original UnresolvedSymbol for an actually visited missing argument.
