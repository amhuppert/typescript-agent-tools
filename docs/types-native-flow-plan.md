# Native read and reachability infrastructure

The operating envelope is one immutable captured Project and one isolated
request-local checker. Ordinary use-site reads are required by T/Q4; F remains
excluded. The full selected design is still in progress.

## Ownership and alternatives

The Ousterhout design route compared a syntax walk at every read against one
immutable binder graph and a restricted native interpreter. A syntax walk
would duplicate conditions, assignments, execution boundaries and relevance
rules across reads and function returns, and could mistake an unknown effect
for an irrelevant one. The graph provides one owner for binder decisions;
read setup, node interpretation and end reachability remain distinct complete
checker operations over that graph. This implements the existing design's
ownership rather than broadening the profile into F.

Binding captures flow at final-tree sites inside its existing collection
operation, before the parse arena is released. Stable file-local graph handles
refer to exact source sites and canonical binding attachments; raw parser IDs,
display origins and property spelling are not semantic identities. Final
publication moves the graph with the other retained file facts. Source exposes
immutable read/container/end handles from its captured syntax without another
control-flow interpretation or disk read.

The construction owner ports `bindContainer`, `bindChildren`,
`createFlowCondition`, `isNarrowingExpression`, `bindAssignmentTargetFlow`,
`maybeBindExpressionFlowIfCall`, branch/loop/try/switch/optional-chain binding,
and `EndFlowNode` from pinned TypeScript 7.0.2. An explicit scheduling stack
preserves compiler visitation and execution-boundary order. Labels preserve
ordered deduplicated antecedents and actual cycles; referenced/shared state is
owned by graph construction. Unsupported semantic construction remains an
explicit deferred obligation during intermediate increments; it cannot make a
later read look like a flow Start or absent-flow case.

Evaluation's complete identifier/read operation owns symbol/alias resolution,
assignment position, declaration/flow containers, automatic types, initial-type
and assume-initialized setup, default parameter optionality and the actual
`getFlowTypeOfReference` question. It interprets graph nodes with native reference
matching and a bounded memoized proof. Only a native operation that establishes
irrelevance permits skipping a node. Union assignment refinement, automatic
evolution, narrowing and unsupported call effects report `flow_required` when
consumed. Product resources and invalid dependencies remain distinct failures.
No annotation or nonunion shortcut skips this setup.

`getTypeAtFlowNode` uses explicit invocation frames. A child helper increases
native recursive depth; tail assignment/call/array skips and singleton labels
keep the same frame. The depth-2000 guard runs before a Shared cache lookup.
Each reference question owns its native `sharedFlowStart` window; only the
last Shared node in a normally completed tail frame is installed, and a cache
hit returns without that installation. Completed helper results are distinct
from cached flow questions. First-write retention matches native cache lookup.
Native overflow disables further flow analysis and consumes errorType, reported
as InvalidSemantic/ErrorType. Product work/depth/memory/deadline exhaustion keeps
its resource failure instead. Actual `checkBlock` scope restoration belongs to
that future complete operation; there is no per-read reset.

Natural strict and loose controls pass in
`vrun-3cc00805-cd47-4eed-aab3-fee3a8ed61f5`: recursive depth 1999 is healthy;
at/over 2000 consumes native errorType; 4000 flat assignments stay at depth zero.
The final four-test Project RED
`vrun-1674bd2c-afca-4ab7-a9a5-ba268274c879` wrongly returned healthy number at
the boundary. The invocation-owner fix passes those controls with zero skips
in `vrun-2f421665-69f0-4b3f-b44c-5556039e7f08`. The valid flat input exposed
attachment lookup latency; this does not close the product resource or full
flow gates. [Invocation review](types-flow-invocations-review.md) preserves the
independent source audit.

Function end reachability has its own native operation and caches; it is not a
read-narrowing question. It consumes the same actual graph and normal admission
for assertion/never call effects and switch exhaustiveness. This is a prerequisite
for exact body return inference and predicate-inference admission.

## Proof sequence

1. Freeze fresh compiler read controls and source/hash inventories. Establish
   ordinary nonunion reads, ambient union identity, union initializer reduction,
   automatic reads, unrelated and relevant assignments/guards, unreachable nodes,
   and local versus captured execution boundaries.
2. Add final `Project::type_of` tests and collect assertion-level RED. During a
   construction increment, explicit pending graph forms must remain failures;
   no supported read is inferred solely from a textual absence of guards.
3. Construct and retain the binder graph, expose site handles, and implement
   supported setup/identity reads. Validate final outcomes through Project and
   repeated Session/service requests. Extend positive and refusal variants from
   the complete pinned dispatch rules, including joins, cycles and boundaries.
4. Add function-end and effect controls, then final inference RED/GREEN. Complete
   the native construction/admission matrix and resource accounting before
   delivery. Measure retained graph cost and include it in the representation
   and actual-corpus gates.

Compiler controls and public native assertions are staged separately so
compilation/setup failures cannot be mistaken for behavior RED. This document
does not claim implementation or final proof of flow infrastructure.
