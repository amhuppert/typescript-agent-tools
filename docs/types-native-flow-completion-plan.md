# Flow completion ownership

The operating envelope is the existing request-local native query checker.
This change preserves the documented rejection of final flow-sensitive results
with `flow_required`; it does not expand the product's flow-analysis profile.
The implementation is being revised against TypeScript 7.0.2 `flow.go`.

`getFlowTypeOfReferenceEx` currently enters a Result-only interpreter. Completed
invocations and Shared entries contain bare FlowType, and completed loop slots
contain bare TypeId. A recovered child therefore interrupts before native
continuation, while a clean wrapper around this interpreter would hide the
missing transport. Both identifier and member access need the same solution.

Use the existing NativeCompletion/NativeSequence policy on actual native
operation frames, rather than a second failure format or a session journal.
A small invocation owner will centralize child consumption, Shared publication,
completed results and ancestor failure composition. Native invocation depth and
loop/expression scope restoration remain the existing interpreter's questions.

Each flow-node invocation has an ordered prefix. At a Shared miss, split that
prefix from the suffix that computes the selected Shared value. A later Shared
miss supersedes the earlier cache candidate. Publish only the selected suffix
in the Shared slot; return the whole invocation completion to its parent. A
Shared hit consumes its cached completion after the current prefix and returns
without publishing the earlier Shared candidate. This prevents earlier tail
work from becoming a dependency of an unrelated cached suffix.

A cold getTypeAtFlowLoopLabel computation needs an operation frame of its own.
It shares the surrounding flow node's native depth: the helper itself is not a
recursive getTypeAtFlowNode invocation. Its actual antecedent calls still add
one depth. Key acquisition precedes the completed-loop lookup. The helper's
completed value owns only its body dependencies, excluding the enclosing
flow-node prefix and key acquisition. Completed loop slots retain that
completion; active-loop assumed candidates remain copied native type IDs.
Reentry consumes the actual union operation on those candidates, without
replaying an unfinished computation. Expression-cache restoration still
precedes the restarted completed-loop lookup.

This is preferable to putting the enclosing invocation's entire failure list
in every cache slot, which would replay acquisitions that a cache hit skips.
It is also preferable to making every worker manage recovery vectors and cache
slices: workers consume named child operations; the invocation owner owns the
lifetime and publication rules (Ousterhout P04/P10/P14).

Migration order:

1. Invocation/Shared/loop completion transport and the existing final identity
   admission, with the real native depth error remaining a recovery producer.
2. Existing branch/assignment/condition/switch/reference/key workers use the
   completion forms of the semantic operations they already perform. Admission
   adapters remain only for still-Result callers and are explicit migration
   boundaries, not evidence of completed continuation.
3. Identifier and access flow callers consume the complete operation. The
   access lane owns narrowing/setup/readonly rules; binding owns retained graph
   facts. No second flow interpreter or class-provenance reconstruction is added.
4. Pinned native evidence and final queries for before/after-Shared dependency
   boundaries, loop key/cache/restart boundaries, recovered children followed by
   later acquisitions, native depth limits and whole-failure cleanup. Existing
   loop, flow-invocation, admission and resource tests remain required.

Status: invocation, Shared-slot and completed-loop transport is staged for the
coordinated compiler checkpoint and adversarial review. Branch-union, literal-base
and subset children consume their completion forms. Other flow semantic workers
still have explicit admission boundaries and remain migration work; this checkpoint
does not establish complete native continuation. The graph-label metadata fix and
missing-property reporter are also awaiting final Rust-query evidence.


The first transport checkpoint compiled and passed the loop-cache final queries
(`vrun-7ee1a6cb-23cd-4555-9d36-6bc44c126eb6`). Native strict/loose linear
false-condition chains were measured in
`vrun-88e916f6-73f0-4fd2-a743-c555b6edb9a1`: 1,999 returns number; 2,000 and
2,001 return errorType with TS2563 and measured depth 2,000. The earlier empty-if
diamond fixtures still return native errorType, but replaying each Shared
completion produces exponentially many causal entries. The existing checked
omittedCauses counter therefore returns whole OutputLimit. Adversarial review
confirmed there is no native errorType/Any shortcut in the branch worker; the
solution preserves that output limit and isolates native depth with linear
controls rather than suppressing dependencies. Final Rust linear controls are
staged, not yet proven at the next checkpoint.

Read reference keys now follow FlowState.refKey: the first loop's actual key
acquisition contributes to its invocation, then stores only the key. Later loop
labels observe it without reacquiring names or replaying that key prefix. The
matching operation's declaration case normalizes the source's value export,
while identifier pairs retain raw resolved identities.

Reachability is a different native query with different cache rules. Its Shared
miss recursively computes a fresh worker; that completion alone is published,
then consumed by the outer worker after any tail prefix. Branches consume
children in order and stop at first true. The top-level last-result slot is
published only on completion; ReduceLabel clears it before the reduced walk.
Per-worker NativeSequence frames preserve interruption ancestry. This lifecycle
received bounded adversarial review; effect/predicate/switch dependencies and
final end-reachability witnesses remain separately required.

Before-use checking now transports static-property type, optional type, flow,
raw-parent declared type, base list and property lookup completions. The
isPropertyDeclaredInAncestorClass question uses the raw Parent directly; the
related getDeclaringClass helper would add an incorrect late-parent acquisition.
Generated property names retain their structured identity through base lookup.
Unused Result adapters are removed as their last callers migrate.


## Stage 34 condition and discriminant continuation

Condition, assertion, optional-chain matching, candidate discriminant and
truthiness operations now sequence their completed native values. The flow
interpreter and reachability worker consume the new equality/switch completions
at their actual acquisition sites. The call condition obtains the actual
signature predicate before deciding its existing flow boundary; hasOwnProperty
uses its separate native missing-type branch. Matching an identifier against an
unrelated syntax kind does not resolve that identifier. Constant discriminant
aliases use the shared `isConstantVariable` rule.

Union key-name/map publication now stores only completed pairs, including the
native negative sentinel. An interrupted computation installs no pair. Reading
an already acquired constituent map observes its value without replaying key
acquisition. `isDiscriminantProperty` installs its computed false bit before
raw property type/genericity work. Only the native true-bit write publishes the
completed suffix; false computations retain recovery in the current invocation
while keeping the provisional cache. Composite lookup remains outside that
slot. Two bounded native-source reviews confirmed these schedules.

The new native-flow-continuation fixture observes exact erroneous opposite
references inside logical conditions, assertion conditions and switch cases.
Its hook uses existing return observations and makes no acquisition-order claim
from map iteration. Capturing the pinned results and proving final queries remain
required. These staged changes do not claim a completed native checker or extend
F's output profile.

The 32-root capture additionally distinguishes optional-chain match direction.
In strict mode, `(x satisfies T)?.p !== undefined` narrows the original reference,
while `(x = other)?.p !== undefined` preserves its declared union. Loose mode
retains identity for both. The matching owner now accepts actual reference nodes
on both sides, so checker-created references follow the same asymmetric rule.
Native bootstrap `vrun-f28f3ad7-6872-4427-aedf-f8e72b11b409` agreed with all
ordinary API results; its baseline differences were precisely the two new rows
per profile. The expanded baseline is installed; maintained and final-query
validation are tracked separately.
