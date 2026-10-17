# Captured final-tree attachment index

The operating envelope is one accepted immutable Project with request-local
syntax capture. Source already reparses each demanded file once and retains
its complete final AST and parent table. Semantic and binder identities must
not become parser NodeIds; the proposed index only attaches an existing exact
source site to that captured final tree.

The current `CapturedSyntax::node_at` walks every final AST node for each
flow/call/effects/return request. Ordinary declaration recipes repeat similar
whole walks. Thus a flat list of N unrelated assignments can require O(N²)
attachment work despite native flow being iterative. The registered final
four-test flow-limit RED
`vrun-1674bd2c-afca-4ab7-a9a5-ba268274c879` took 103.68 seconds and passed
the valid flat-4000 and depth-1999 controls; depth-2000 controls failed only
because the native guard had not yet been implemented. The flat test emitted
the runner's over-60-second notice. This is bounded end-to-end latency
evidence, not a profiler's attribution of all time to Source.

Two designs were considered. Per-operation memoization avoids some repeated
identical lookups, but still scans once for each new site and makes each
caller own attachment semantics. One capture-time index over the final walk
hides exact role/range matching and ambiguity once. The latter is chosen:
capture records exact syntax kind and token span, plus declaration-name and
first-token attachment facts from the same actual nodes. No token catalog,
name resolver, binder inventory or second AST is introduced.

The private operations are `node_at(site)` and
`declaration_at(allowed_kinds, raw_name_anchor)`, with a distinct first-token operation
for reserved signatures. Each returns the unique actual NodeId or preserves
missing/ambiguous failure. Index keys use raw byte spans and exact syntax
roles; no decoded spelling or nearest-token fallback participates. A small
Unique/Ambiguous payload retains a collision as a failure rather than a last
writer choice. Traversal sees only final reachable nodes, not unused parser
arena entries. Parent capture and all attachments use that same walk.

This is a behavior-preserving refactor, so no artificial missing-behavior RED
is required. Existing final read, member, call, return, generic and presentation
outcomes must pass at a coherent source checkpoint. Record before/after
latency and actual index rows/payload accounting. The flow guard changes in
the parent lane may confound a complete-four-test time comparison; retain
that limit and report the valid flat control separately when the runner
exposes a useful measurement. No complete representation/resource gate is
claimed from this bounded index improvement.

The index is now implemented in the one `CapturedSyntax` owner. Declaration,
member, type-parameter, target, function, variable-read and retained flow sites
use its exact attachment operations. Query selection and enclosing-call
selection retain their once-per-query offset walks; those are different
questions from replaying an exact retained site. Recursive function display
metadata was added in a concurrent slice, and therefore compatibility is
reported against the combined source checkpoint.

All 29 final presentation tests pass with zero skips in
`vrun-88afc44f-66b0-4993-90f6-10de14f17017`. The registered four-test
flow-limit run passes with zero skips in
`vrun-94f7d921-6a08-4a66-8864-4003e3c053e6`; its explicitly measured whole
invocation took 3.258760 seconds, including queue/compilation. The prior
103.68-second failing run used the old attachment walks and lacked the new
native guard. These are different timing scopes/source states, so they support
a bounded latency improvement without attributing the whole difference to the
index or claiming a general speedup ratio.

An isolated exact-source-copy allocation spike records the actual storage
cost in [historical evidence](../fixtures/types/source-attachments/evidence/README.md).
The AST is parsed before accounting starts. On this 64-bit target, attachment
keys are 24 bytes, values 16 bytes and parent slots 16 bytes. The healthy
flat-4000 source is 44,096 bytes and has 20,020 reachable nodes. Its table holds
20,023 rows at capacity 28,672; capture retains 1,663,824 requested heap bytes,
including a 320,320-byte parent vector. The index's incremental requested heap
is therefore 1,343,504 bytes. The depth-1999 control has 3,020 reachable nodes
and 3,023 rows, retaining 216,272 bytes including parents. All four observations
have zero ambiguous rows; none exercises the reserved-signature first-token
rows. Capture allocation counts/totals and instrumented times are preserved in
the raw evidence, separately from registered latency. Allocator overhead, RSS,
whole-corpus query capture and service budgets remain unmeasured obligations;
this cost must enter the full representation gate.
