# Native rest calls and predicate inference continuation

This continuation uses the same request-local evaluator and immutable captured
Project as the composite-signature owner. TypeScript Go 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` remains the behavior authority.
The preceding stage supplies complete signature position/count/rest/label
operations and tuple factories; the caller must now use those operations in
the native order. A passing ordinary-array subset will not complete this work.

`getEffectiveCallArguments` (checker29799) is the first missing semantic owner.
It cannot be implemented as a syntax recipe: a tuple spread creates actual
synthetic argument expressions whose established TypeId, spread bit, original
parent and tuple label participate in subsequent inference and applicability.
Conversely, replacing source expressions with invented parser identities would
pollute contextual and expression caches. The chosen representation is a local
call-argument enum with captured and synthetic variants, carried by one resolved
call input. Source continues to capture actual syntax; the evaluator creates
the semantic arguments. A competing approach that threads separate type, label,
spread and source arrays through each caller duplicates native argument identity
and permits those arrays to disagree.

The call input owns original call metadata plus its ordered effective argument
list. Synthetic arguments retain the original spread expression as their parent
and established type/label metadata. They do not enter the captured-expression
cache. Complete argument operations expose native spread status, effective check
node, contextual checking and ordinary checking; those operations dispatch to
the native `checkSyntheticExpression` worker for synthetic arguments: a spread
is a numeric indexed-access value, while a nonspread retains its type. The
shared `checkExpressionEx` generic-signature post-step still follows this worker.
Arity, argument-mode selection,
inference and applicability all consume the same list. Reentry creates the
native call-local list again, while the existing resolved-signature and source
expression caches retain their separate ownership.

Effective arguments are acquired in `resolveCall` after explicit type-argument
source checks, candidate reordering, and the empty-candidate exit. Before the
first spread, arguments are shared unchanged.
Each later actual spread checks its expression using the cached question unless
an active flow loop requires ordinary checking. Tuple element types are acquired
once through `getElementTypes`; Rest elements become arrays, Variable elements
mark synthetic spreads, and actual label declarations are retained. Nontuple
spreads stay original expressions. Tagged templates, JSX, decorators and
instanceof have separate native acquisition branches and remain explicit
dependencies until their actual syntax/semantic producers are owned.

`hasCorrectArity` (checker9083) asks parameter count before minimum count.
For ordinary parsed complete calls, a spread position uses minimum count and
then effective-rest/count short-circuiting. Excess arguments ask effective rest
before rejecting. Missing arguments use `getTypeAtPosition`, not a slice of raw
parameter descriptors, and require an actual Void constituent. New expressions
without an argument list retain their distinct minimum-zero branch. Source must
retain that list-presence fact; an empty list is a different native question.

`isSignatureApplicable` (checker9232) asks raw this first, skipping its comparison
for new/super access exactly as native. Next it asks `getNonArrayRestType` and
uses the effective parameter count to split ordinary and rest arguments. Each
ordinary argument asks its position type before contextual checking, applies
the native regular-object operation in SkipContextSensitive mode, then relates
the actual argument. The rest suffix uses `getSpreadArgumentType` followed by
one relation. Inference (checker9435) uses the same split, including implied
arity for a bare type-parameter rest, raw this inference before positional
inference, and final rest inference only when it can contain variables.

`getSpreadArgumentType` (checker29257) owns both the last-spread fast path and
tuple construction. The fast path checks a real spread contextually, preserves
synthetic established types, returns mutable array/tuple conversion for
array-like types, and otherwise asks iteration before creating an array.
Other spreads use normal checking and become Variadic or Rest tuple elements.
Ordinary suffix arguments get tuple contextual element types or a Contextual
indexed access, then contextual checking and literal regularization/widening in
the native order. Const context and mutable-array-like checks determine readonly
only after all elements. Shared owners are required for mutable array/tuple
conversion, contextual tuple elements and iteration; the call owner must not
reconstruct these from printable kinds.

The existing final composite `rest` control proves ordinary rest calls; new
compiler-derived controls must distinguish fixed/optional/rest/variadic tuple
spreads, zero and multiple spread arguments, labeled synthetic arguments,
readonly arrays and tuples, const inference, contextual literals, rejected
arity before child reads, and failure followed by an independent healthy call.
Boundary expectations come from native final query results before production.

Predicate inference (checker20440–20514) is a separate continuation exposed by
correct lazy signature rendering. Source owns actual function-kind/flags and
single-return syntax eligibility; the evaluator owns implicit-return flow,
cached expression checking and ordered raw parameter reads. Inferred predicates
remain outside the F-output contract: this continuation establishes exact
eligibility and unchanged answers, and preserves a changed flow result as
`FlowRequired` rather than publishing a newly inferred predicate.
Only a Boolean return enters the parameter scan. Assigned, rest, Boolean or
nonidentifier parameters are skipped in native order. True/False synthetic
conditions use the expression flow, parent-return flow, or a fresh Start node
as antecedent. Both reads must use the shared `getFlowTypeOfReferenceEx` owner:
the True read has declared and initial type equal to the parameter's raw type;
the False read would retain the same declared type but start from the True
result in the compiler. Here a changed True read stops with `FlowRequired`
before allocating that second condition. The first read uses exact admission,
without the selected-expression regular-literal allowance.
The shared Captured/Created reference and flow-node boundary also serves the
before-use owner; no predicate-local narrowing implementation is permitted.

Predicate controls must cover rejected syntax/async/generator/multiple/bare/
implicit returns before expression reads, nonboolean returns, assigned/rest/
Boolean/nonidentifier parameters, useful and one-sided refinements, original
child failures, and lazy preview/elision. A missing inferred predicate is not
represented as healthy None when an admitted native dependency failed.

The eligibility implementation now uses a separate Source predicate-body recipe
and the existing implicit-return reachability owner. The signature cache installs
the native no-predicate sentinel before body checking and replaces it with the
original failure on abort. Captured parameter names and freshly allocated
conditions go through the shared flow owner; no separate narrowing interpreter
or inferred-predicate factory was added.

Maintained native final-call controls passed in
`vrun-00c571e5-b6d6-4de0-ba8b-f6fd8ef7cfbb`: twenty questions across strict and
loose profiles, with no skips. They distinguish nonboolean, multiple/bare/implicit
returns, unchanged flow, Boolean/assigned/rest parameters, useful inference and
one-sided inference. `tests/type_predicate_eligibility.rs` expects the measured
healthy answers for the eight unchanged/eligibility groups and `FlowRequired`
for both groups whose first True flow read changes the type. Source and evaluator
ordering/cache integration passed bounded adversarial peer review. The new final
Rust controls remain unvalidated until the next coordinated checkpoint.

The implementation continuation now uses `CallInput` / `CallArgument` in the
call owner, with contextual operations supplied by the common ContextNode
owner. It also ports the complete tuple branch of `inferFromObjectTypes`,
`tupleTypesDefinitelyUnrelated`, rest `applyToParameterTypes`, and the existing
`isConstTypeVariable` branches for indexed access and variadic tuples. Mapped,
conditional, and substitution producers retain their actual unresolved owner
dependencies; no healthy answer is inferred from their printable form.

The maintained native matrix is `fixtures/types/native-call-rest/`, with
`tools/type-native-call-rest-oracle.test.mjs` and final
`tests/type_call_rest.rs`. It distinguishes fixed/optional/rest spread shapes,
readonly arrays and tuples, synthetic labels, multiple and empty spreads,
const generic rest, tuple prefix/suffix inference, two variadics, constrained
variadic slices, callback parameter slices, the minimum-arity probe
that consumes an invalid annotation despite a later healthy overload, and a
consumed invalid spread dependency followed by independent healthy queries.
Expected summaries are measured by the pinned native compiler.

Pre-change owner bodies are retained under `.cc/temp/types-call-rest-before/`.
The coordinated Signature header migration changes their mechanical interfaces;
controlled RED must reintroduce only the prior semantic pending gate in the
current owner, preserving the new CallInput and Signature getter plumbing.
Restoring an old whole method with stale interfaces would prove only a compiler
failure. Tuple object inference, spread argument construction, and const tuple
recognition need separate bounded gate reversions when their final cases pass.

The maintained call-rest matrix is GREEN in
`vrun-2e56c72d-8064-4d99-9d23-dca9516a107a`: 96 fresh checker questions
across strict, loose, and exact-optional profiles, with no skips. Its globals
include the mutable Array.push member, so readonly constraints are observably
distinct from Array<any>. Native measurements establish that optional tuple
spreads create Required synthetic slots (retaining their raw nullable type),
and a two-variadic target without implied arity can leave the second inference
as unknown[]. The apparently successful zero-argument overload still consumes
the first candidate's missing annotation during minimum-argument probing; the
product test therefore preserves that exact failure.

`fixtures/types/native-const-tuples/` and its native/final controls separately
exercise const variables inside generic variadic tuples: fixed head, fixed
tail, optional tail, mutable constraints, and callback parameter slices. The
15 strict/loose/exact questions are GREEN in
`vrun-cd4f5281-7a5a-49cc-b76f-0d1d2110f1aa`, with no skips. The call-rest and
const-tuple final Rust suites are staged and await coordinated validation.

The IIFE parameter branch is now one operation in the same call owner. It
acquires effective arguments before deciding rest versus ordinary parameter;
ordinary argument checking temporarily installs the canonical anySignature
and restores the previous cache entry on every exit, including failures.
Source owns actual IIFE ancestry; functions do not manipulate call state or
reconstruct effective argument lists. This newly connected branch will receive
its final controls through the function-type producer's checkpoint.

The contextual integration now separates oldest-first contextual-type frames
(including cached nil entries) from newest-first inference frames (where nil
shadows an outer context). Captured and synthetic expression identities use the
same wrapper; synthetic nodes retain fresh identity and a captured parent only
for ancestry. Array element, contextual property, object-element and return
expression operations use the shared tuple/index/signature owners. The original
contextual owner is retained at `.cc/temp/contextual-before-native-arrays.rs` for
an isolated semantic counterfactual after mechanical compilation.

Native Signature identity now owns one-time contextual type-parameter and this
fields. Cloning a Rust handle shares those fields; a native clone factory copies
the currently established values into fresh fields. All field consumers use the
same getters, so contextual assignment does not require cached member inventories
to be refreshed. Parent and member adversarial reviews found no sharing violation;
review corrected absent-this installation to precede the context's raw type
read, using a fresh unresolved parameter question.

The predicate control failures also exposed missing `typeof`, equality and
logical expression producers. These now preserve checked operand order and
TypeOnly's equality suppression. The logical diagnostic path consumes actual
call signatures and promised/awaited types. Their shared promise owner preserves
native non-nil caches, union traversal, recursive nil recovery and original
semantic failures. Tested callable/enum-symbol diagnostic usage, NaN symbol
identity and other binary families remain explicit dependencies; they have not
been presented as completed behavior. This integration is staged and requires the
coordinated Rust checkpoint and additional compiler-derived boundary controls.

The call cache now preserves an acquired failure before considering a signature
installed by reentry. Native successful-result replacement still has its original
ordering. An exploratory eight-case final-request probe with a zero-demand hook
did not reach that replacement branch (`vrun-8ae4312e-2e09-4585-96d0-b21f4ce53c56`);
the probe and its failed assertion are archived under
`.cc/temp/types-call-reentry-probe/`. This is a remaining coverage obligation,
not evidence that the failure-priority correction is proven end to end.

`getQuickTypeOfExpression` now owns assertion, literal, and Await shortcuts as
well as ordinary calls. Assertion types are acquired without checking the
operand or type-source diagnostics. Await recursively asks the operand's quick
type before unwrapping; native nil remains absence here, whereas checked Await
consumes errorType. Literal eligibility deliberately uses the original node,
while the other shortcuts use the parenthesis-stripped expression. The new
quick-expression native/final matrix distinguishes skipped missing operands,
skipped call arguments, checked promised/scalar operands, invalid thenables,
and const assertions that must consume their operand. All 39 native questions
are GREEN in `vrun-0a216e68-f92f-41e6-a712-224cb7863f33` (strict, loose, exact-optional; zero
skips); final Rust validation remains pending. The first native measurement
also pinned diagnostic 1355 in addition to the consumed unresolved-name 2304
for the invalid const assertion; that syntax-only diagnostic does not replace
the original semantic failure in the product.

The compiled call-rest suite reached five genuine failures at SpreadElement
expression acquisition. The repair routes actual spreads through the shared
`checkSpreadExpression` operand/iteration operation, preserving cached checking
and active-loop bypass. The IIFE failures reached the separate native
`getContextualTypeForArgument` question; its shared owner acquires effective
arguments before actual-node matching, handles the resolving signature sentinel,
and uses Contextual indexed access for rest positions.

Explicit call type arguments now carry their actual source-check node beside
their type recipe. `resolveCall` and `resolveUntypedCall` source-check these nodes
before subsequent native work; the quick nongeneric call path deliberately skips
that question. The new native matrix is GREEN in
`vrun-1cdf3369-6669-4916-82a7-d1366c19c95a`: 27 fresh checker questions across
strict, loose, and exact-optional profiles with no skips. It distinguishes
ordinary diagnostic-only rejection, hidden missing constraints, later constraint
acquisition after an earlier relation rejects, untyped calls, spreads, and the
quick bypass. `tests/type_call_type_arguments.rs` retains both healthy answers
and original consumed failures followed by independent healthy queries. These
repairs and their final tests await the coordinated Rust checkpoint.

The integrated call-rest checkpoint passed four of five groups and exposed a
synthetic spread applicability regression at `restLabels` (`vrun-f8bd3f94-d32e-41b7-8d1e-b0b984f9339d`). The old synthetic contextual worker always
returned its raw array type, while native `checkSyntheticExpression` returns
`getIndexedAccessType(t, numberType)` for a spread. The staged repair shares one
synthetic expression operation between contextual argument and ordinary IIFE
checking, including count reset and the existing generic-signature post-step.
`getSpreadArgumentType` deliberately retains its separate direct raw type read.
The common generic-signature operation now accepts actual ContextNode identity;
it never invents a captured expression for a synthetic node. Native contextual
frame lookup remains oldest-first with cached-nil and flagged-cache distinctions.

Additional final controls in `tests/type_synthetic_expressions.rs` cover the
ordinary IIFE path, positional synthetic spread, raw-rest tuple result, a generic
function slot and a lazy invalid index demanded by the generic post-step. Native final queries and before-summary diagnostic assertions are GREEN in
`vrun-3f9e7014-556b-4dc7-84c4-30b4e9838c02` (18 questions across
strict/loose/exact, zero skips). The invalid-index control keeps a healthy native
`Broken` summary but consumes diagnostic 2304 before summary; the Project test
requires the original InvalidSemantic dependency. Bounded Ousterhout peer review
found no blocker in synthetic checking or contextual-frame ordering. Final Rust
verification remains pending for these new controls. Required post-GREEN counterfactuals are separate: return the retained
raw type instead of numeric access for synthetic spreads, then restore and bypass
only the synthetic generic post-step. Each must fail its corresponding final
control, with no stale whole-file restore.

The next compiled checkpoint (`vrun-fdae7bd0-d132-4372-a5f0-92fb025281fb`)
passes all five call-rest groups and the synthetic generic-postcheck failure
control. The raw-rest IIFE reaches the remaining Contextual indexed-property
dispatch. Native `getPropertyTypeForIndexType` delegates this question to
`getTypeOfPropertyOfContextualType` and converts only its nil result to Any.
The repair calls the existing complete contextual-property operation; it does
not introduce a tuple exception or a second property/index policy. In particular,
the IIFE's rest inference uses Any as its context, so native absence here is an
ordinary Any result. Consumed lookup failures still propagate. Additional
compiler/final controls cover fixed and optional tuple callbacks, array index
callbacks, and an unresolved contextual parameter followed by a healthy query.
All 30 native questions pass in
`vrun-e36de53c-9ff3-4cbe-9e76-8c47d6117531` (zero skips). Native retains the
unresolved parameter's alias spelling in its errorType summary and consumes
diagnostic 2304; the final product assertion requires the original unresolved
dependency rather than that recovery value. The repair and expanded controls
await the coordinated final checkpoint.
