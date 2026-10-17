# Native return-body inference

This increment implements the existing public type operation over captured
sources and the retained canonical binder. The envelope remains one native,
request-local Evaluator with shared value/declared/member/signature caches,
fixed fresh-demand checking order and no production compiler subprocess. The
full selected profile remains required; contextual, async/generator, relation
and resolution dependencies cannot be replaced by syntax approximations.

TypeScript 7.0.2 commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`
defines the expected operations. The relevant owners are checker.go
`getReturnTypeOfSignature` (19906), `getReturnTypeFromAnnotation` (19963),
`getReturnTypeFromBody` (20031), `checkAndAggregateReturnExpressionTypes`
(20164), `functionHasImplicitReturn` (20212), `mayReturnNever` (20217),
`getWidenedLiteralLikeTypeForContextualReturnTypeIfNeeded` (20312),
ast/utilities.go `ForEachReturnStatement` (1138), and ast/functionflags.go
`GetFunctionFlags` (13). End reachability reuses the existing native flow owner.

## Designing twice

One alternative would collect `return` syntax and widen all its expression
types. This is small but answers the wrong question: it misses actual end-flow
reachability, visits nested functions, wrongly ignores syntactically unreachable
returns, conflates fresh and regular literal unions, and cannot distinguish an
empty normal declaration from a never-returning function expression. A caller
would have to repair those exceptions outside the operation.

A second alternative would add a separate body checker with its own binder and
expression cache. It could reconstruct the rules, but breaks source identity
and checking history and duplicates decisions already owned by Evaluator.

The chosen design extends the actual deferred Signature Return question.
Source captures a body recipe from its exact SignatureRoot and final AST role;
Evaluator owns complete native return aggregation, calling its existing
expression/type/union/widening/contextual/effect operations. The existing
retained flow graph supplies the actual body's EndFlowNode. No caller walks
statements, guesses fallthrough, rebinds a local name, or selects a union mode.

## Owned operations and transfer boundaries

The parent `DemandRecipe::Return { root, recipe }` retains the actual
SignatureRoot once; `SignatureReturnRecipe::Infer` is a unit variant rather
than duplicating the root's location. `Source::return_body(root)` lazily captures that body's
inputs; annotated returns continue through their existing annotation operation
and never acquire the body. Acquisition uses CapturedSyntax's existing exact
kind/span `node_at` owner. The root's raw name anchor supplies the canonical
function symbol through `Bindings::declaration_symbol`, rather than comparing
function spellings.

The body recipe distinguishes a block from an expression body, records native
function flags and mayReturnNever syntax, and captures own return statements in
source order. The complete ForEachReturnStatement owner uses an explicit stack:
only ReturnStatement and the native block/control kinds participate, and
nested functions/classes are skipped. Every own return expression is checked,
including a return after an unconditional return or throw. A bare return sets
the native no-expression fact even when syntactically unreachable.

The retained `Graph::container_at(site)` operation identifies the actual native container by
final role and raw span. A function name declaration's existing flow attachment
belongs to its outer container and cannot substitute. The getter reuses the
stored Container and its end; it creates no second retained site/name inventory.
Evaluator's existing `reachable_flow` becomes visible to this consumer and
keeps call/assertion/never effects, ordered paths and failures intact.

`check_return_body(recipe)` first establishes implicit return through actual
end reachability. It then performs source-ordered return aggregation, native
self-call exclusion, never contribution tracking, const-context regularization,
identity deduplication, strict implicit/bare Undefined inclusion and the
Subtype union question. Normal declaration throw/infinite bodies can have void;
function expressions/arrows and object methods have the distinct mayReturnNever
question. Unit returns then ask actual contextual return widening, followed by
whole widened_type. A literal union is not a Unit flag and must not be widened
merely because each alternative is a literal. Empty returns still ask contextual
Undefined versus Void. Async and generators consume their actual awaited,
promise and yield/next/global dependency operations rather than a fabricated
local wrapper. Missing native globals can produce non-error recovered public
types; that does not authorize healthy product semantics.

Return resolution must use the same native TypeResolution stack with a
distinct Signature/ResolvedReturnType key, rather than treating every deferred
Demand reentry as an identical cycle. Marked pop-false installs the native
return recovery; actual Return-type annotation cycles and inferred recursive
cycles keep their distinct cause locations. The current parent stack has
symbol Type/DeclaredType keys and a separate SignatureReturn key. Parent
metadata is wired; the actual SignatureReturn push/pop and recovery consumer
is coordinated with root and needs its own circular-return final proof.
The direct bare self-call shortcut is an independent source-backed question:
its actual callee type/symbol must be acquired before comparing the canonical
function symbol; arguments and full call resolution are intentionally not
consumed on that native branch. Other calls retain the real checkExpression
obligation.

## Compiler and public behavior obligations

The fixture `fixtures/types/native-return-body` covers scalar/object widening,
boolean and mixed literal alternatives, all-return versus implicit/bare states,
unreachable return contributions, nested function exclusion, throw/infinite
declarations, explicit annotation precedence, unresolved return recovery,
direct self calls, never operands, expression/arrow bodies, const/contextual
literal state, and async/generator global recovery. Strict and loose options
use separate fresh demands. The compiler harness is
`tools/type-return-body-oracle.test.mjs`; final behavior will be asserted through
`tests/type_return_body.rs`, own and renamed cross-file Value selection, with
actual Function graph return refs as well as summaries and failure provenance.

The first compiler attempt `vrun-6ebf9aa8-ebee-4150-823b-e01f33c6db78`
had ambiguous substring fixture anchors, so it is not a baseline pass. The next
`vrun-1c060cec-b692-4b6a-b5a4-d270a47a0d83` passed three of four controls
and corrected an expectation from number to the native `1 | 2` union. The next
`vrun-9f55eecb-c205-4f62-be92-6fe632c3e391` corrected native string-first
display order for mixed literal unions. Final compiler four-test GREEN is
`vrun-27d443a4-6a4b-4632-9dc0-9c13c5fa9956`, with zero skips. These are
research corrections, not production behavior REDs. Production acquisition
changes wait for a compiled final Project assertion RED against the frozen
compiler controls. Mechanical parent Return metadata wiring can precede that
RED; no artificial failure test is needed for DTO scaffolding.

Implementation proceeds through complete operation ownership with relevant
baselines per increment. The ordinary captured declaration family is the first
reachable consumer, while every other dependency above remains in the full
delivery plan. A pending dependency is exact and located; it is not a permanent
scope exception, known empty collection, successful fallback, or final gate
closure.

The initial final Project attempt
`vrun-3578b743-6c1a-4ec1-966a-8fededf4da8c` had wrong test DTO field/path
types and failed compilation; it is not a behavior RED. After correcting that
test setup, `vrun-4781025e-04f0-4934-9454-04be3e9af37c` compiled and
executed all five final-boundary tests, failing their actual Found/consumed
invalid assertions against the existing unsupported Return demand. It is the
behavior RED authorizing the return-body operation.

The ordinary declaration consumer is implemented in
`evaluation/returns.rs`, with captured recipes in `source/returns.rs` and
the sole `GetFunctionFlags` / `ForEachReturnStatement` syntax owners in
`syntax/returns.rs`. Final five-test GREEN is
`vrun-e4252356-990c-4027-9c4b-d3ec24dc044b`, one matched file and zero
skips. It proves own and renamed cross-file function results, actual return
graph refs, strict/loose bare and implicit states, unreachable and nested
traversal distinctions, direct bare-self-call/never contributions, annotation
precedence, and consumed missing-operand failure at its own source.

The canonical ordinary function value cache supplies a unique anonymous
TypeId per actual function symbol; callee expression checking obtains that
identity before the admitted bare-self-call comparison. No structural,
display-name or origin interning is involved. Later expression/arrow self
calls still require the native constant-reference owner. Ordered identity
deduplication uses a request-local set plus vector instead of a quadratic
scan; this is a behavior-preserving refactor after GREEN, not new semantics.
The duplicate Infer location was likewise removed mechanically. The parent
actual SignatureReturn resolution consumer and real call operation remain
separate pending proof, along with contextual/const-expression, async,
generator, relation and complete resource/corpus obligations above.


## Stage 24 async and widening continuation

The existing return aggregation, contextual return, Promise adoption and whole
widening owners now carry NativeCompletion through their actual continuations.
Async body returns use checked no-alias await, unwrap, and the checked Promise
factory in native order. Empty-body contextual detection asks union-only
someType(Undefined); it does not descend intersections. Contextual async return
expressions preserve a nil mapped await result and otherwise construct
AwaitedResult | PromiseLike<AwaitedResult>. Function-return widening reports
run before unit-context widening. Root and sibling widening caches retain
complete acquisition prefixes and native nil/empty distinctions.

The corrected capture `vrun-562fed16-28a8-4709-a8ea-9e7059f4296d` establishes
22 roots in strict, loose-null and exact-optional profiles. All 66 checker-hook
and public-API summaries/error identities match; the initial three comparison
failures are expected-file ENOENT after complete capture. Baselines are copied
verbatim. The earlier `vrun-a20695f6-6b88-46e8-884a-0a2800e7c2f6` stopped at
an AsyncKeyword query anchor and supplies no complete capture. The replacement
contextual roots select actual inferred declaration values.

`tests/type_async_returns.rs` asserts final Project results for ordinary,
nested/structural Promise adoption, legitimate any, empty/bare/self/await-self
returns, arrows, annotation precedence, contextual inference and sibling
widening. Invalid thenables and two missing-return branches must retain ordered
recovery even though the native outer function/Promise has errorType=false.
The lazy getter control distinguishes strict/loose widening diagnostics from
the actual missing type dependency. Query order and graph options check request
isolation; they do not substitute for native warm-cache evidence.

These new production paths and Project controls await coordinated compilation
and validation. Generator inference, the function-expression bare-self constant
reference case and ordinary variable reportImplicitAny acquisition remain
explicit work, not alternative successful behavior or delivery closure.
