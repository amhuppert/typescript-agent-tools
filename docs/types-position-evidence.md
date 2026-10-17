# Literal selection and checking-dependency checkpoint

`src/syntax/expression.rs` owns the pinned compiler's complete
`IsExpressionNode` / `IsInExpressionContext` dispatch, including their narrower
heritage, name and import.defer questions. The mutual ancestor recursion is an
iterative walk. Source selection applies it after the literal-type role check.
Literal tokens in nested unary, array, conditional, computed-name, return and
for-condition positions retain their own expression question and exact span.
The closing template delimiter belongs to the same complete literal token.
Punctuation, comments and whitespace do not acquire a nearby expression.

Direct PropertyAssignment names retain a Value question and exact own root;
their pending type operation is distinct from an inner computed-name expression.
Module specifiers have no supported Q4 type anchor. Selection records actual
call/new/tag ancestors for every admitted role, including callback declarations
and pending property values. Evaluation must consume getResolvedSignature
outermost first. It currently returns a located unsupported failure at that
call dependency; native signature checking remains required.

Four fresh compiler tests passed without skips in
`vrun-54cfe1c4-55ea-492a-8dc5-08aa4025baa7`. The producer's PropertyAssignment
question bug was separately reproduced and corrected by its owner; it was not
treated as evidence of a native evaluator failure. Native behavior RED
`vrun-6d366625-6a5f-45d7-a044-fc52ae139c70` executed four tests and failed for
pending nested literals, the module-specifier failure kind, and the call
dependency's wrong location. All four final Project tests passed without skips
in `vrun-7bdf904a-1431-4f4a-ad65-ee1f6cc5d718` after the corrections.
Intervening lifetime/Cow setup failures are compilation failures, not RED proof.

The independent Ousterhout source review in
`.cc/temp/types-expression-owner-review.md` found no blocking discrepancy in
the classifier or outermost-first metadata traversal. That read-only review
does not admit future expression families or replace their end-to-end tests.
