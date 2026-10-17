# Bounded logical and conditional binding review

This Ousterhout review covers the immutable graph construction in `binding/flow.rs`
and the logical/optional parent-role questions in `binding/flow/syntax.rs`. The
operating envelope is one captured accepted TS/TSX program generation, with
request-local semantic consumption of that retained graph. It does not assess
whole-program flow narrowing or prove pending optional-chain construction.

No blocking defect was substantiated in this increment. Native TypeScript 7.0.2
`bindConditionalExpressionFlow` and `bindBinaryExpressionFlow` preserve the incoming
current flow when their child traversal has no real mutation/call/jump effect.
The new `EndExpression` work item retains both the saved flow and the saved effects
state, reduces the post-expression label only when needed, and restores the outer
effects with OR. LIFO scheduling visits condition, question token, true branch,
colon token and false branch in native source order.

`bindLogicalLikeExpression` consumes the left condition before the operator and
right operand. AND sends the true edge to the right; OR/coalescing send the false
edge there. Logical assignment binds the right under the actual branch pair,
then attaches the left assignment mutation and final expression conditions.
`schedule_in_branches` captures/restores the caller's pair around child traversal,
matching `doWithConditionalBranches`; prefix negation swaps that pair rather than
inventing a separate condition owner. The final `bindCondition` condition step is
skipped only for the native logical, logical-assignment and outermost optional
forms which route their own edges.

The parent-role owner climbs only parentheses and prefix `!` for
`isTopLevelLogicalExpression`, then distinguishes actual statement conditions,
logical parents and optional expression receivers. Optional root and outermost
questions remain separate named operations. The current optional-chain dispatch
attaches an explicit pending mutation instead of claiming complete branch facts;
this preserves the remaining construction obligation. An absent enclosing target
pair likewise remains pending rather than fabricating labels.

The review read native binder lines 1788–1811, 2175–2185, 2219–2278,
2289–2317 and 2782–2788 at pinned commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. It inspected the existing final
`Project::load`/type-read controls rather than adding helper-output tests. Root
reported final read21 GREEN `vrun-d5e3a425-5e3b-4aa7-8cc7-a2de91752a1c`
(21 tests, zero skipped) and native compiler17 GREEN
`vrun-08f63d35-e1aa-4010-8159-a9fbd520fa70`. Those receipts are the owner's
behavior evidence, not an independent rerun or a claim about all future graph
forms. Unknown callee effects, nullish/optional narrowing, complete loop
interpretation and whole Q3 scheduling retain their separate required gates.
