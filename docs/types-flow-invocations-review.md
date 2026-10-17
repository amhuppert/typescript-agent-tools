# Native flow invocation and cache review

This read-only Ousterhout review covers the restricted Identifier read owner
after the explicit invocation-frame migration. The operating envelope is one
request-local evaluator over an immutable accepted final binder graph; native
flow history stays within that evaluator. It is not a claim that the excluded
full F analysis or evolving-array/changed-loop families are implemented.

The new boundary hides the important native distinction: a flow node, an
invocation and a scheduling continuation are different identities. Pinned
`getTypeAtFlowNode` in `internal/checker/flow.go:117–211` checks depth 2000
before Shared lookup and increments depth once per actual invocation. Native
assignment/call/array-mutation nil paths, singleton branch/loop labels and
eligible Start-container crossings tail-traverse at the same depth. Recursive
condition, compound-assignment, assertion and branch helpers create child
invocations. The explicit frames preserve this distinction; a work-stack size
guard or a node-count guard would reject healthy flat input and miss native
recursive depth.

`ReadInvocations` owns one Shared cache per reference question. It is not a
general FlowId result memo. Native Shared lookup scans only the current
`sharedFlowStart` suffix; nested reference questions create another suffix
and truncate it on return. A local map with first-write retention reproduces
that lookup for the admitted immutable FlowType result. Only normal frame
completion installs its last tail-encountered Shared node. A cache-hit frame
returns directly without installing an earlier saved Shared node. The
implementation retains both details and cannot reuse outer-invocation rows.
Future native incomplete FlowType state will need to remain part of this
cached value; the current admitted loop branch only returns a complete
declared-type result and otherwise keeps its full operation pending.

The retained binder `shared` bit follows `setFlowNodeReferenced` and is the
sole eligibility decision. The reader does not infer sharing from graph
shape. Ordered branch processing still returns immediately when a path is
the initialized declared type, preserving native checking history before
later alternatives. The graph and invocation modules therefore earn separate
boundaries: immutable graph construction owns relationships/sharing; the
reader owns actual recursive schedule, cache and consumed dependencies.

Native depth overflow installs errorType, disables subsequent flow analysis
and is a whole InvalidSemantic dependency here, separately from product
resource exhaustion. The disabled check precedes the absent-flow return in
`getFlowTypeOfReferenceEx`; it is not bypassed by annotations or a missing
flow attachment. No per-read reset is justified. Pinned `checkBlock` at
`checker.go:3772–3784` saves/restores that state only around actual
`IsFunctionOrModuleBlock` checking. That predicate includes SourceFile,
ModuleBlock and a Block whose parent is FunctionLike
(`ast/utilities.go:1128`); its SourceFile membership alone does not establish
that every source demand invokes checkBlock. Current root/body demand code
does not implement a blanket body check or restoration. Any future complete
body/source checker must retain this exact operation and inherited prior
state, rather than set disabled=false on entry.

No new incorrect successful answer was found in the inspected migrated
slice. Native natural limit controls passed in
`vrun-6ca5fb62-1362-4477-a9d2-939a0965857d`: depth 1999 is healthy, depth
2000 and over consume errorType (including native TS2563 observations), and
4000 unrelated flat assignments stay at native iterative depth zero. The
actual final four-test RED was
`vrun-1674bd2c-afca-4ab7-a9a5-ba268274c879`: healthy controls passed but
at/over depth incorrectly returned number. The final registered four-test
run passed with zero skips in
`vrun-2f421665-69f0-4b3f-b44c-5556039e7f08`; strict/loose native controls
also passed in `vrun-3cc00805-cd47-4eed-aab3-fee3a8ed61f5`.
Source attachment latency, whole temporary loop caches,
reduced labels, captured Start extension and all excluded F work remain open
required gates.
