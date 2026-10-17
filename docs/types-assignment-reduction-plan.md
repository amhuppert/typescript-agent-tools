# Native assignment reduction

The operating envelope is one immutable accepted Project and one request-local
evaluator. This operation supplies intermediate flow types to the existing
completed-read admission owner. It neither admits changed reads as T nor
implements a second assignment collector or binding lookup.

Two designs were considered using scalar joins, Boolean freshness and an
erroneous assignment whose native reduction falls back to the declaration.
Patching the reader with primitive-name comparisons would hide assignability
direction, lose Boolean freshness and duplicate native relation rules. The
chosen deep operation is `getAssignmentReducedType(declared, assigned)` in
`evaluation/reads/assignments.rs`. It asks the existing Assignable relation
owner, preserves ordinary False, and caches the actual ordered TypeId pair.
Native identity and assigned-Never fast paths precede that cache. Consumed
unsupported, invalid or resource dependencies remain failures.

`getAssignmentReducedTypeWorker` filters the declared alternatives in native
order. `typeMaybeAssignableTo` uses ordinary assignability for a nonunion RHS
and short-circuit Some over a union RHS. A fresh Boolean literal maps the
filtered alternatives through the existing literal factory's fresh operation;
its semantic freshness cannot be inferred from the public literal graph.
The final assignability question decides whether to keep that reduction or
return the original declaration. Native erroneous assignment fallback is a
healthy type identity at this question, distinct from an actually consumed
errorType while obtaining the RHS.

A separate Source operation in `source/assignments.rs` acquires actual initial
and assigned recipes from the retained graph's target node and accepted
parents. It owns `getInitialType`/`getAssignedType` grammar dispatch, not types:
variable initializer, ForIn, ForOf, BindingElement, binary assignment,
destructuring default, Delete, array/spread/property/shorthand assignment and
native ErrorType fallback stay distinguishable. The evaluator consumes those
recipes through existing expression owners. `getTypeOfInitializer` belongs
beside `checkExpressionCached`/`getTypeOfExpression`: it checks that owner's
installed normal node result first, then asks the separate flow-only expression
question. No source span or variable type inverse lookup substitutes for the
cache question.

`getInitialOrAssignedType` then asks `getNarrowableTypeForReference` in Normal
mode. Established noninstantiable scalar families have the native unchanged
early result. Constrained/type-parameter/substitution and contextual position
dependencies, iteration and destructuring remain precise required operations
until their complete owners are wired. They are not frozen delivery exclusions.

The native control file passed two tests with zero skips in
`vrun-e35db9a1-93f9-4fec-9099-371db3e1b510`: twelve fresh final roots under
strict and loose options establish scalar join, Boolean join, union RHS,
erroneous-assignment fallback, and the healthy displayed result of a missing
RHS. Four same-checker phase controls show that a union assignment visits
Missing, while a nonunion assignment skips it. The latter distinction drives
the product's original UnresolvedSymbol failure and healthy Number result;
whole-program diagnostics cannot replace actual demand provenance.

Final `tests/type_assignment_reduction.rs` must compile and fail on these
missing behaviors before production changes. The existing capture six-test
RED proves the integration prerequisite too, but does not replace these new
final variant assertions. Named module declarations and existing flow/cache
fields are coordinated with their owners; no public helper testing API is
introduced. Full structured relations, generic narrowing, iteration,
destructuring and execution/resource calibration remain mandatory work.

The compiled final RED is `vrun-0dacf5b8-7e44-4ef3-b15a-8e220781987c`:
four executed tests, all four failing the actual missing assignment reduction.
The earlier `vrun-75d0113f-39ce-45d5-b367-b05c664862dc` stopped at unrelated
member-stage compilation errors and is setup evidence only. The first native
implementation run, `vrun-6699d8ea-1800-4aa3-b8dd-93960983f0f0`, executed four
tests with three passes. Scalar restored joins, union RHS, rejected-assignment
fallback, skipped nonunion RHS, and the original demanded missing-symbol
failure pass. The remaining Boolean failure is the completed expression
question: native `getTypeOfNode` uses `getRegularTypeOfExpression` after fresh
intermediate flow construction. The parent reader owner must preserve that
question and its final admission, rather than remove native freshness from
assignment reduction. No four-test GREEN or full assignment gate is claimed.

The parent fix retains the actual Source expression site before parenthesis
removal. Public expression selection now consumes the whole
`getRegularTypeOfExpression` question, including `getTypeOfExpression`'s quick
path and flow-only cache. Admission may use exact regular identity only for
that actual identifier root; nested inference dependencies and
`GetTypeOfSymbolAtLocation` retain their raw question. The discarded alternative
was a global regularized comparison at every identifier read, which would
allow a public normalization to change the answer to a different native
question. The final four-test scope passed with zero skips in
`vrun-d1b0c42a-5ee8-47b9-af96-82534bc13cb1`, retaining the original Boolean
assertion and distinct skipped/demanded RHS outcomes. The intervening
`vrun-9c71a3c0-cb37-4e9e-8aac-d1b2a4bbdf3d` compile failure came from the
new inline syntax attachment's missing Hash trait and executed no assertions.

A further producer audit found that native `checkDeclarationInitializer`
uses `checkExpressionCachedEx` after its quick-type path, while the earlier
Rust producer called uncached `check_expression`. The retained actual
`Expression.root` now routes that producer through the existing normal cache
after its quick path; a root-less failure recipe preserves its original
failure. This is same-owner wiring rather than a new semantic test claim.
Scalar controls cannot prove fresh object/contextual cache identity. Those
variant controls remain required before complete cached initializer
integration is claimed; no origin-to-site inverse lookup was introduced.

The stage33 completion migration keeps these same operations and cache keys.
`initial_or_assigned_type_completion` owns one local `NativeSequence` across
the existing explicit continuation stack. A native missing projection installs
the actual errorType with evidence; a following default still obtains its
expression and performs the native union. Outer interruptions remain errors.
Property lookup uses the shared structured key before own-property and index
questions. Tuple projection uses the existing whole tuple-tail operation,
including its distinct Undefined out-of-bounds argument, before iteration's
Missing policy. Fresh Boolean reduction now uses the shared mapType owner
instead of a second union walker.

The assignment-reduction slot stores the completed comparison worker only.
Identity and assigned-Never exits precede lookup; only a completed worker
publishes a slot. The initial/assigned producer and narrowable-reference
questions remain invocation work outside that comparison cache. Cache hits
return their actual stored evidence. The implementation neither deduplicates
same-site acquisitions nor adds evidence to other entries retroactively.
Three bounded native-source reviews passed this schedule against `flow.go`
2209–2420 and `checker.go` 23452–23482; these reviews are not a final test pass.

`native-assignment-continuation` supplies 29 selected roots under strict,
loose, and unchecked-index options. Its bootstrap capture is complete. Property,
index, tuple and iteration recoveries are followed by an independent Missing
default; the nested-array control takes the distinct ordinary-RHS branch. Scalar and union declarations distinguish skipped assignment
acquisition from required work. Three comma-expression controls acquire two
distinct references in the same checker, with the same declared/assigned pair,
to test a real assignment-reduction cache hit. The test-only native observer
reads the existing map lookup and write, already-allocated type IDs, and default
entry/return fields; it invokes no additional semantic operation. Exact final
Project expectations will follow the pinned outputs and native acquisition
schedule. Neither this new final proof nor the stage33 compiled gate is claimed
complete here.

The completed stage34 capture is
`vrun-94a1aaab-7051-41a1-bc38-7e6a121fdf97`: all 87 hook results matched the
ordinary API; the bootstrap failed only because maintained expectation files
had not yet been installed. The three capture files were then copied verbatim
into the fixture. Each warm comma query records a lookup miss, installation,
then hit for the same actual pair. The cold control records only miss/install.
Identity, assigned-Never, and scalar skip controls never enter that cache.
These observations establish within-query reuse; repeated Project requests
remain separate evaluators.

The native `isDestructuringAssignmentTarget` question checks the immediate
container, so `({item: [value = later.value]} = nestedBroken)` takes the ordinary
binary RHS path when computing this assigned value. Its observer has no default
entry and only the later unresolved annotation is consumed. This deliberate
native distinction is retained rather than broadening a recursive syntactic
destructuring predicate. The unchecked-index profile also distinguishes actual
errorType input from an alias-bearing Any at the default boundary.

`tests/type_assignment_continuation.rs` asserts the final healthy results and
the documented FlowRequired boundary for changed reads, actual recovery sites
before later defaults, and exact cause-sequence replay across the observed warm
cache hit. The warm failure is two copies of the cold worker's ordered evidence,
not a set of sites: equal locations remain separate acquisitions. No final Rust
pass is claimed until the parent runs this newly staged scope.
