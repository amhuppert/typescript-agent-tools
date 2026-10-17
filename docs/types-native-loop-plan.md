# Native loop results and invocation state

This Ousterhout design assumes an accepted immutable Project, the existing
canonical Identifier read identity, and one request-local evaluator. Binding
already owns actual ordered loop edges. It revises the bounded LoopEntry
specialization in types-native-loop-review.md; it is not a narrower delivery.

Two designs were considered. A generic fixed point over every graph node would
hide native cache keys, incomplete first-antecedent rules and expression-cache
restarts behind a different algorithm. Native getTypeAtFlowLoopLabel instead
owns its complete loop question, while the existing explicit read scheduling
stack preserves child invocation depth and tail traversal. That keeps one owner
for the compiler operation and avoids adding another graph or symbol inventory.

The result of a native invocation is a FlowType: an actual TypeId and its
incomplete bit. Shared caching, reductions, compounds, conditions, switches,
calls and branch joins must preserve that pair according to each native rule.
The native newFlowType operation replaces a Never result marked incomplete
with the cached silentNever identity, distinct from ordinary Never and the
unreachableNever sentinel. Complete assignment and Start results retain their
native complete status. Branch declared/initial early returns discard
incompleteness as the compiler does; other branch paths accumulate it in native
order, with the bypass contribution conditional on its actual inclusion.

The request-local loop cache is keyed by the actual immutable flow node and
getFlowReferenceKey. For currently admitted Identifier reads, the latter contains
the raw canonical symbol or alias identity, declared and initial TypeIds and the
actual flow container. Locations and printed names cannot substitute for those
identities. Other reference families require their native dotted-key operation
before they can use this cache.

The first actual antecedent is analyzed without adding a loop-stack entry.
Before each later antecedent, the checker adds the current ordered type list to
its request-local active loop stack and hides the flow-expression cache. A
matching nonempty active entry returns its native union as incomplete. An empty
active entry restarts analysis and reaches the non-looping first path; a generic
visited-node or active-key shortcut would change this behavior. Each later
antecedent restores both stacks before consuming its result, and picks up a
newly installed completed loop-cache result before continuing. Exact declared
identity stops the native ordered traversal. The final result is cached only
when the first antecedent is complete.

Native checkExpressionCached resets both the active loop stack and flow type
cache during its own computation, then restores them on every exit. Its
existing expression owner should hide this entire reset decision. A nested
read does not reset the loop stack merely because it starts another reference
invocation. Failures must restore the suspended state and remain failures;
failed work cannot install a completed loop-cache answer.

getUnionOrEvolvingArrayType remains the existing complete native junction
operation, including established union reduction, unknown recombination and
declared-union identity restoration. Evolving arrays, foreign structured
relations and excluded narrowing remain precise consumed dependencies until
their mandatory owners are implemented. Actual intermediate assignment types
can form a healthy final join; no declared-type fallback is permitted.

The new 27-marker compiler boundary matrix pins while, for, for-in and for-of
after-loop full unions, do-loop changed results, and every admitted statement
ending. Original two-test native GREEN
vrun-481443c4-e26a-4e10-84ab-b55525cf58c4 has 54 fresh strict/loose roots and
140 line-break/BOM roots. Frozen replay GREEN is
vrun-98a39d74-dfa2-4363-b123-1355e4d27fb8. Final boundary four-test compiled
RED vrun-1f539fb2-1d08-47ff-bd3a-ca8f733b4f71 first stops at the preceding
missing union-assignment prerequisite; that is not yet a reproduced loop
interpreter failure. A scoped retry after that prerequisite and focused nested
loop/cache controls must prove the actual missing loop branch before its
implementation. Fresh Boolean and regular-expression questions remain distinct
owners, not an excuse to collapse native freshness in a loop union.

The assignment prerequisite now passes the six capture tests in
`vrun-79eb51ae-5036-45b3-ab09-a6dc849a8809`. The four boundary tests were
retried in `vrun-664c53d0-5588-4c30-b94c-e98b28fd0563`: three passed and the
statement-boundary test failed at `while-after` on the actual missing native
incomplete-loop union operation. That compiled assertion is the loop RED,
distinct from the earlier assignment prerequisite failure.

The complete native loop protocol replaces that specialization in the
request-local reader. The same four-test scope passed with zero skips in
`vrun-d704e5fc-3cb6-4f20-b5fc-889846b2ca25`. Flow invocation and Shared results
now carry their incomplete bit, branch and bypass propagation follow the
native early-return rules, and loop/cache suspension is restored on failures.
Normal expression caching resets the active loop stack together with the
flow-expression cache; call resolution suppresses temporary active-loop cache
installation. The independent source reread found no further scalar/Identifier
algorithm mismatch. Focused nested-loop, natural restart/call-history, foreign
relation and evolving-array controls remain required; this boundary GREEN does
not claim their complete proof or the full T/Q3/Q4 gate.

TypeScript 7.0.2 rejects WithStatement in the strict parsing baseline, including
a script under strict:false; alwaysStrict:false is itself removed. A throwaway
compiler probe observed TS1101 and TS5108 rather than a healthy selected read.
The complete extendAssignmentPosition syntax owner retains the native With
case, but these rejected inputs are not invented healthy T controls.
