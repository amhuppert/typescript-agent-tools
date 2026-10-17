# Native exception and finally flow

The operating envelope is one immutable captured Project and one request-local
checker. This implements required graph/reachability infrastructure for T and
exact supported reads; consumed refinement continues to report `flow_required`.
The complete T/Q3/Q4 delivery remains in progress.

Two designs were compared through Ousterhout's ownership principles. Mutating a
finally label while interpreting a reduction would expose graph lifecycle and
restore obligations to every read, nested reference and reachability caller.
One immutable graph plus native reduction history hides that temporary decision
inside each complete checker operation. Pinned 7.0.2 uses that same history in
`getBranchLabelAntecedents`; it is the chosen design. A new cache keyed by a
reduction history would change native Shared-cache semantics and is not added.

Binding's `bindTryStatement` saves return/exception targets, adds entry before
mutations, and gives catch its own subsequent exception label. Its
`createFlowMutation` path adds Assignment/ArrayMutation nodes to that target;
`createFlowCall` deliberately does not. Nested control-flow containers clear
exception targets through the existing container operation. An unreachable
syntax subtree stays under the existing `bindChildren` rule.

Finally sees concatenated normal, exception and return antecedents, preserving
order and duplicate identities from `combineFlowLists`. Its normal, outer
exception and IIFE/constructor return exits use immutable ReduceLabel nodes with
the exact subset. A returning finalizer or an absent normal exit remains
unreachable. Reduce construction does not invent additional Shared bits.
Scheduling frames contain temporary parser handles; the published graph contains
only file-local flow handles and exact sites.

Read typing recursively evaluates the finalizer flow while its nearest reduction
substitutes the target label's antecedents. That is a child native invocation and
consumes recursive depth. Its Shared cache still follows native last-shared
completion, without a reduction key. End reachability is a distinct complete
operation with native Shared and last-node caches, ordered branch early return,
loop entry and normal call-effect demands. Reduce clears last-node history before
its child; it does not erase all Shared reachability facts. Errors propagate
without becoming an absent effect or false reachability result.

Fresh strict/loose compiler controls cover 21 read/return sites and the actual
missing predicate child. They passed two tests without skips in
`vrun-c1268026-1ca0-4e8a-a289-c49b4b4b0cf7`. The first final four-test RED
`vrun-eb5554ed-9f6b-49bc-88d3-0f8c66f86960` exposed pending construction and
incorrectly consumed catch-call effects. The expanded five-test RED
`vrun-b4ce105b-c55f-4ca7-a28f-41768df00ef9` retains those four failures and
proves the existing unreachable-syntax control already passes. No bug fix to
that existing rule is claimed. Five final tests pass with zero skips in
`vrun-d530eddc-7cef-414a-86cf-fbb2ee3cf247`; switch, captured-start, complete flow cache/resource and full
semantic delivery gates remain open.
