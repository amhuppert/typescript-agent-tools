# Bounded loop construction and identity-read review

This read-only Ousterhout review compares the current immutable binder graph
and restricted read interpreter with TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. It covers ordinary loop
construction and the exact-declared-type loop-entry admission. It is not a
verdict on complete flow analysis, native checker delivery or resource gates.

No blocking defect was found in this bounded lane. One binder owner constructs
the ordered graph; one checker owner interprets the selected reference. The
Source layer does not repeat loop interpretation or use a syntax screen to
declare an effect irrelevant. The explicit scheduling stack is proportionate
to the captured-tree operating envelope and avoids recursive construction.

The LIFO schedules in `flow.rs` agree with the named native binder operations:

- `bindWhileStatement` attaches the non-looping entry before condition/body
  edges and routes continue to the loop header.
- `bindDoStatement` routes continue to the condition, restores the surrounding
  targets after the body, and attaches the condition's true edge to the header.
- `bindForStatement` visits initializer, condition, body and incrementor in that
  order. Continue goes through the incrementor; break goes to the exit.
- `bindForInOrForOfStatement` evaluates the iterable before the loop, attaches
  the possible exit before initializing the iteration target, and then binds
  the body/back edge. `bindVariableDeclarationFlow` supplies the declaration
  assignment even without an ordinary initializer.

The shared `addAntecedent` and `finishFlowLabel` operations preserve native
insertion order, identity deduplication and unreachable-edge exclusion. The
first loop antecedent remains the actual entry path; it is not reconstructed
from source position or sorted after binding. Nested break/continue targets are
restored by the iterative operation. Labelled targets and unimplemented
destructuring/control forms retain explicit Pending obligations rather than
becoming absent flow or a successful no-op.

The checker `LoopEntry` task is a bounded specialization of
`getTypeAtFlowLoopLabel`: it evaluates the first actual antecedent and accepts
only its exact established declared TypeId. Native iteration breaks at that
identity before consuming later antecedents. In the admitted initialized and
ambient ordinary families, the initial type is that identity or its established
optional superset, so the native `isTypeSubsetOf` question adds no structural
relation dependency. The one-member union preserves the same identity.
Nonmatching entry types are refused as `flow_required`; the implementation does
not fabricate an incomplete union or treat a graph cycle as proof of identity.

This does not port the complete native loop protocol. Flow reference-key and
loop caches, incomplete loop-stack restarts, evolving arrays, subtype union
reduction, foreign injected types, call effects, narrowing, captured origin
extension and labelled construction remain required where consumed. The
current interpreter's exact identity result must not become a general shortcut
for those families. Captured graph allocation/load cost and demand resource
limits remain part of the full representation/resource proof.

The compiler and final Project matrices separately cover all iterative
forms, literal-false reachability, cyclic identity reads and a read after an
infinite loop in strict and loose profiles. This review did not rerun tests or
mutate production. Their implementation lane supplied final compiler13 GREEN
`vrun-cf670e98-cafe-4db8-80fc-c632bb082653` and final Project read16 GREEN
`vrun-2600ebc1-6049-4f66-a52a-33e624a47d6a`, with zero skips. The latter
followed actual read16 RED `vrun-d509c0e7-3fb9-4fe6-8b42-cb494d2cb7fa`
(15 passed, one do-loop assertion failed). These receipts close the bounded
loop matrix; they do not close the broader protocols listed above.

## Whole-loop operation plan reread

The later `types-native-loop-plan.md` was compared with pinned
`getTypeAtFlowLoopLabel` (flow.go:1304) and `getFlowReferenceKey`/
`writeFlowCacheKey` (1636/1644). No additional algorithm mismatch was found in
that proposed operation. It replaces the earlier LoopEntry specialization with
actual FlowType incompleteness, native loop and expression-cache scopes, and
ordered antecedent questions. This source read is not implementation proof.

Two exact boundaries need explicit final controls. Native scans active loop
entries in insertion order, so the first outermost matching nonempty entry
supplies the incomplete result. An unknown-symbol or nondotted reference key
returns the declared type before any loop cache or active entry is consulted.
Identifier keys retain the raw resolved symbol, including alias identity,
declared/initial type identities and actual flow container; runtime alias
targets and locations are not replacements. After a later child invocation,
the hidden flow-type cache and active loop stack are restored before checking
whether restarted expression checking installed a completed loop answer.
Failures must restore both states without installing a completed result or
turning resource exhaustion into native incompleteness.

The first loop antecedent deliberately runs without an active entry. Empty
active lists restart analysis; they are not a circular failure or a cached
Never. Only completeness of that first antecedent decides whether the final
union enters the loop cache. Exact declared identity stops later antecedents
after native subset classification. Focused nested-loop/cache and final result
controls remain required after the assignment-reduction prerequisite; the
existing boundary RED that stops in that prerequisite does not prove a missing
loop branch.

## Whole-loop implementation reread

The later `evaluation/reads/loops.rs` implementation was compared directly
with pinned flow.go:1304–1389 while its coupled source stage was open. The
completed cache precedes the insertion-order active scan. Only nonempty active
entries supply native incomplete unions; the first antecedent runs without an
active entry, and later antecedents suspend the transient expression cache.
Restoration precedes the restarted completed-cache lookup. Unique TypeIds,
native subset classification and exact declared-identity early stopping occur
in source order. Only the first antecedent controls final cache installation.
The shared `newFlowType` keeps actual incomplete Never as silentNever.

The cache key uses the raw resolved identifier origin, declared and initial
TypeIds, actual flow node and actual flow container within its file. This
matches the admitted Identifier question. Other key families remain required;
locations, canonical alias targets or source spelling cannot replace raw
symbol identity. The read boundary unwinds suspended/active scopes on original
failures, without installing a completed loop result. A successful invocation
with unbalanced scopes becomes Internal rather than a false completed answer.

One concrete coupled protocol gap was found: `getResolvedSignature` still
installed every newly resolved call result, although native checker.go:8417–8424
restores the prior call cache whenever the actual active loop stack is
nonempty. That premise changed when the whole-loop interpreter became real.
The same Calls owner now first chooses an independently installed inner
Resolved signature, then either installs the result with an empty active stack
or restores the previous cache state/absence. Both successful and failed
temporary computations restore the scope. This same-owner completeness wiring
was authorized under the coupled loop RED; no separate artificial RED was
claimed. A product Failed state is not substituted for an installed native
resolved-signature property.

No additional algorithm mismatch was found in the admitted immutable
Identifier/scalar family. Final whole-loop outcomes are now GREEN in
`vrun-fcb92525-32e3-4d8b-8b6a-6e8f1a122317` (`tests/type_loop_cache.rs`):
four tests cover all 18 strict/loose native markers, including nested backedges,
conditional assignments, multiple continues, RHS reference reads, early
declared-identity stopping, reachability and shadow identity. The earlier
statement/text/capture boundary is GREEN in
`vrun-d704e5fc-3cb6-4f20-b5fc-889846b2ca25`.

The focused compiler suite is GREEN in
`vrun-55ae3d8e-7c4d-4976-b9b2-de3c7c8d99d0`: two tests, zero skips, with 36
fresh public requests and 36 separate raw/location identity controls. Exact
copied tooling, inputs, native/API identity, and 34 fresh same-checker phase
requests are archived in
`fixtures/types/native-loop-cache/evidence`. Before/after tool hashes match,
and every hook input hash matches its corresponding captured byte snapshot.

Natural counters demonstrate first-antecedent incompleteness, active reuse,
completed installation, Normal expression-cache resets, restarted cache pickup,
and temporary-call cache suppression. These callbacks are observations, not
seeded expected counters. The actual assigned ErrorType/TS2304 control and
inferred-copy TS7022 controls retain recovery causality even where the final
root is non-error. No product eligibility claim follows solely from that final
identity. The empty-active-list branch was not naturally observed. Exhaustive
cache/reset/failure-unwind proof, evolving arrays, arbitrary reference-key
families and the full semantic/resource/corpus gates remain open.
