# Native try/finally reductions and reachability review

This bounded Ousterhout review assumes one accepted immutable Project and a
request-local checker. Binder owns the actual flow graph; evaluation owns
native read and reachability questions. It covers the newly admitted try/catch/
finally families, not the complete excluded F analysis or switch/loop work.

No additional incorrect successful answer was found in the inspected slice.
The separate boundaries are justified: native read results carry a reference's
type and local Shared window, while end reachability has persistent boolean
Shared and last-node caches. Conflating those questions would hide checking
history and permit the wrong result cache to escape its scope.

Pinned `bindTryStatement` (`internal/binder/binder.go:1993–2072`) starts the
exception label with the prior flow, retains mutations as possible exceptional
antecedents, gives catch a new exception target, and restores the outer return/
exception targets before binding finally. The implementation follows this
ordering. Finally's label concatenates normal, exception and return lists
without ordinary deduplication or invented sharing. Reduced exits retain the
same finalizer antecedent and each exact ordered subset. An unreachable
finalizer ends the entire try flow; normal completion with no normal path is
unreachable even if a return path existed.

Pinned `getBranchLabelAntecedents` (`internal/checker/flow.go:208–220`) uses
the nearest active reduction for the same target label. The shared private
operation searches active scopes in reverse order and preserves the original
branch list otherwise. Loop entry is a separate native question. Read
ReduceLabel handling creates a real child invocation and pops exactly its
scope before normal completion, preserving the established depth/Shared cache
protocol. Labels remain immutable; no reduction-specific cache key is invented.

Pinned `isReachableFlowNodeWorker` (`flow.go:2492–2558`) checks the last-node
cache first, then persistent Shared state, and uses a no-cache-check recursive
call only for an uncached Shared node. Reduction clears only last-node history;
the compiler leaves its Shared map intact. The implementation preserves those
details, native first-reachable branch termination, loop-entry order and
separate call-effects demands. Asserts without an asserted type use the false-
expression rule; AssertsIs is a distinct DTO kind and cannot take that branch.
Never returns remain a separately demanded type. Invalid/unsupported effect
dependencies propagate; errors cannot become a reachable default. Temporary
reduction scope belongs to each operation and is discarded if it fails.

The actual public five-test missing-behavior RED was
`vrun-b4ce105b-c55f-4ca7-a28f-41768df00ef9`: one existing unreachable
control passed and four assertions exposed missing try-flow behavior. Fresh
native 21-marker and missing-predicate controls passed in
`vrun-c1268026-1ca0-4e8a-a289-c49b4b4b0cf7`. Final Project outcomes pass
with zero skips in `vrun-d530eddc-7cef-414a-86cf-fbb2ee3cf247`, including
exception-entry versus normal-exit refinement, failed effects, nested finally,
actual inferred returns and unreachable syntax.

This evidence is bounded. The whole switch-exhaustiveness operation, changed
loop/incomplete FlowType caches, evolving arrays, general narrowing and resource
limits remain full-design obligations. A source review plus these controls does
not prove every combination of nested reduction, persistent cache history or
callable dependency.
