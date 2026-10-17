# Bounded optional-chain read review

This review applies the Ousterhout skill to the optional-chain syntax, binder
and restricted Identifier-read admission patch. The operating envelope is a
fresh request over the captured final TypeScript tree and one canonical
Evaluator; it is not a review of full optional-chain expression checking or
all flow narrowing.

No concrete incorrect successful answer was found in the inspected slice.
The shared `src/syntax/optional.rs` owner retains four different compiler
questions: `IsOptionalChain`, `IsOptionalChainRoot`,
`IsOutermostOptionalChain`, and `IsExpressionOfOptionalChainRoot`. Their kind,
flag, question-dot and immediate-parent tests match pinned 7.0.2
`internal/ast/utilities.go:344–385`. In particular, a root receiver is not
itself necessarily an optional-chain root, and NonNull can continue a chain
without becoming its root.

`binding/flow.rs` schedules receiver traversal, receiver conditions, the
pre-chain join, rest traversal and final outermost conditions in the native
`bindOptionalChain` order (`internal/binder/binder.go:2346–2414`). The native
top-level optional operation preserves incoming `hasFlowEffects`, unlike the
logical-expression operation's reset. The implementation keeps this
distinction rather than reusing a related operation with different state
semantics. Rest traversal also retains question-dot, type-argument and
argument order. `createFlowCondition` asks the receiver-role question before
treating true/false literals as impossible paths.

`evaluation/reads.rs` checks optionality before syntax-kind dispatch, using
the native nullable facts masks. Its optional-containment loop follows only
actual flagged chain receivers. Its call-condition admission asks actual
argument matching and containment before the effects signature, and skips
that signature on the false branch of an optional call, matching
`internal/checker/flow.go:379–468,1830–1882`. The admitted source reference is
an Identifier, so `hasMatchingArgument` has no access-expression ancestors
of that reference to inspect. This boundary is documented rather than
silently widening the reference family.

The owner split is justified: Source supplies exact syntax roles; the
Evaluator owns canonical-symbol matching, consumed semantic dependencies
and the identity admission policy. A failed matching/constraint/effects
question is not replaced by an unrelated-condition success.

The admitted fixture forms passed the registered single-file
`tests/type_optional_reads.rs` run
`vrun-85969240-6fa1-4b89-9871-68cdabf90a8a` with zero skips. The expanded
`tests/type_reads.rs` compatibility run
`vrun-a160a7f2-2da1-428a-b05c-2e720ff8e488` also passed with zero skips.
These receipts support this bounded source assessment. Matching optional
narrowing, optional expression checking, complete
call effects, array mutations, access-reference discriminants and full
flow analysis remain separate mandatory implementation work. No result
here establishes those families or the complete design gate.
