# Bounded native return-body design review

This review uses the Ousterhout skill to assess the actual operating envelope:
one request-local Evaluator, canonical captured-source/binder identities,
deferred signature questions and final Project results. It reads
`types-native-return-body-plan.md` against pinned TypeScript 7.0.2 checker
`getReturnTypeOfSignature`, `getReturnTypeFromBody`,
`checkAndAggregateReturnExpressionTypes` and their complete adjacent branches
(checker.go 19906–20335). No production edits or helper-only proof are claimed.

No blocking ownership defect was found in the proposed boundary. Extending the
existing signature Return question keeps expression checking, native flow,
contextual widening and resolution history behind one operation. A separate
return collector/checker would force callers to coordinate those policies and
would answer a different question. The plan correctly keeps syntactically
unreachable return contributions, skips nested functions, uses actual body
EndFlowNode for implicit returns and distinguishes a literal union from Unit.

Three source distinctions need to survive implementation and final tests:

- The direct self-call shortcut checks the actual callee expression type's
  symbol against the function's merged symbol. A FunctionExpression/Arrow
  value declaration additionally requires `isConstantReference`; simple
  resolved-symbol equality or matching text does not implement that rule.
  Native argument checking and full call resolution are bypassed only on the
  successful shortcut. Parenthesized returns and async `return await` use
  their native unwrapping order.
- Async empty/never returns use `createPromiseReturnType`, while the ordinary
  final contribution path uses `createPromiseType`. Missing global Promise
  can yield root errorType on the former and Unknown on the latter. A
  non-error root does not erase its consumed recovery dependency. Generator
  fallback has its own global Generator/IterableIterator chain.
- Return resolution is keyed by actual Signature/ResolvedReturnType in the
  shared native stack. Annotation, inferred-cycle and direct-self-call
  branches have different schedules and causes. A generic deferred-demand
  reentry check cannot replace those questions. The resolved return cache is
  installed only under the original native conditions.

Useful final-boundary extensions are mutable versus constant function-expression
self calls, a same-spelled shadow callee, parenthesized and awaited self calls,
an implicit-return effect call before a later expression return, explicit
annotation with an invalid unconsumed body, and async/generator missing-global
controls that retain consumed recovery despite non-error root display.
Expected answers must come from fresh pinned demand sequences; diagnostics alone
are not an invalidity oracle. Existing compiler four-test GREEN
`vrun-27d443a4-6a4b-4632-9dc0-9c13c5fa9956` and final Project five-test
RED `vrun-4781025e-04f0-4934-9454-04be3e9af37c` authorize the current
increment; this review does not turn them into proof of the remaining branches.

Contextual signatures, async/generator dependencies, relation reduction,
return cache/cycle causality and full resource/corpus gates remain mandatory.

The follow-up read covers the staged `evaluation/returns.rs`,
`source/returns.rs` and `syntax/returns.rs`. No concrete false answer was found
in the admitted ordinary declaration family. The complete native own-return
traversal and BodyData function flags have one syntax owner; acquisition uses
the final SignatureRoot and actual body-container attachment. Aggregation asks
implicit end reachability before any source-ordered return contribution,
preserves bare/unreachable returns and actual identity deduplication, then
uses the shared Subtype union factory. Fresh scalar Unit returns receive the
ordinary declaration's proved noncontextual widening; literal unions skip
that branch. Existing expression, union and whole widening failures retain
their causes or precise located capability obligation.

The self-call witness is safe in this family because each canonical ordinary
function symbol owns a unique anonymous value TypeId; callee checking obtains
that actual type before comparison. Shape/origin equality or a spelling
interner would invalidate that proof. Function expressions, contextual returns,
awaited operands and generator construction remain explicit dependencies;
their future admission needs the native rules listed above. The parent
Signature/ResolvedReturnType stack integration must be tested separately before
claiming return-cycle/cache semantics. This source read predates the registered
first return-body GREEN and does not manufacture that receipt.
