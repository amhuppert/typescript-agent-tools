# Native Switch flow and exhaustiveness research

This Ousterhout review assumes one accepted immutable Project and request-local
checker, exact retained flow sites and canonical type identities. It is
source-backed research and a bounded implementation review; a complete
Switch implementation is not claimed by this document. The final Project
controls and their bounded implementation receipts are recorded below.

Two boundaries were considered. Treating Switch as an ordinary set of
condition branches simplifies the graph but loses grouped empty clauses,
fallthrough, separately bound case expressions, bypass scheduling and
exhaustiveness state. Retaining native SwitchClause data and one deep evaluator
operation hides those decisions once. The latter fits the existing native
binder/read/reachability owners without a second syntax or type inventory.

Pinned `bindSwitchStatement`, `bindCaseBlock` and `bindCaseOrDefaultClause`
(`internal/binder/binder.go:2073–2147`) preserve these distinct facts:

- Bind the switch expression before installing the switch break target and
  pre-case flow; restore both after the complete CaseBlock.
- An actual true expression or native narrowing expression creates SwitchClause
  nodes. Other switch expressions use the prior flow directly for case entry.
- Group empty-statement clauses until the next nonempty clause, retaining the
  original inclusive start/exclusive end indices. Case entry combines that
  group's actual pre-case flow and fallthrough in native order.
- Bind each case expression at pre-switch flow, then restore current case flow
  before binding its statements. Effects of a case expression cannot leak as
  ordinary fallthrough statement effects.
- Add the normal final path before an empty `(0,0)` bypass when no explicit
  default exists. An explicit default anywhere suppresses that bypass.

`getTypeAtSwitchClause` (`internal/checker/flow.go:1046–1076`) always consumes
the antecedent first. Matching discriminant, matching typeof operand, true
switch, optional-chain containment and discriminant-property operations have
different dependencies. Native discriminant narrowing checks the complete
case-type vector via `getTypeOfSwitchClause`, regularizes literal identity and
uses normal Never for default entries. It then applies comparable-type/literal
replacement and default unit exclusion. An unrelated selected reference does
not demand every case value solely because Switch syntax is present.

`getSwitchClauseTypeOfWitnesses` (`flow.go:1968–1991`) is instead a syntax-only
question. Every case must be StringLiteralLike; a single nonliteral causes the
whole witness result to be nil, without checking that expression's type.
Default and duplicate decoded strings retain empty entries. NoSubstitution-
TemplateLiteral is eligible, while parenthesized string syntax is not.
String equality and decoding must use the existing exact owner. The production
witness contract retains cooked UTF-16 units: ordinary text is encoded without
loss, and an existing unpaired-unit payload is retained directly. ASCII standard
names are classified without converting other units to replacement text. An
unpaired witness is consequently an ordinary host-type witness, not nil and not
an unsupported identity. The native unpaired typeof case narrows Number to Never,
while its default preserves Number. This differs from direct discriminant case
types: their complete case vector requires a semantic string type, and the
current Model cannot retain an unpaired string identity. That dependency remains
UnrepresentableIdentity for both case and default until the Model can represent
it exactly.

`narrowTypeBySwitchOnTypeOf` retains explicit-default position independently
of the witness vector. A default-containing group filters with the conjunction
of not-equal facts for witnesses outside that group; ordinary groups union
each native type-name narrowing, using Never for empty witnesses. The shared
`TypeFacts::typeof_test` already owns exact standard names and host-object
fallback bits, including the distinct Undefined bit. It can supply not-equal
facts, but facts alone cannot replace `narrowTypeByTypeName`: that operation
also consumes strict-subtype/subtype/intersection and native global Function
dependencies. Changed identity remains excluded flow admission until the
corresponding full operation is established.

`narrowTypeBySwitchOnTrue` first narrows preceding cases false in order. A
default-containing group then narrows following cases false; otherwise it
unions each current case narrowed true. A literal false case is not a binder
constant-if elimination: fresh inferred-return controls show it still
contributes its return type. Optional containment and discriminant-property
branches remain their native whole operations, not nominal or spelling tests.

`getTypeAtFlowBranchLabel` (`flow.go:1232–1285`) postpones only the first empty
SwitchClause bypass. Other ordered paths run first and may return immediately
when their type is declared and declared equals initial. The bypass is then
demanded; exhaustiveness is checked only when its type is non-Never and not
already collected. Eager exhaustiveness would consume avoidable dependencies
and change native circularity/history. Read reachability differs: an empty
SwitchClause immediately asks exhaustiveness before following its antecedent.
These are separate questions that reuse one exhaustive-state owner.

`isExhaustiveSwitchStatement` (`flow.go:1912–1926`) has Unknown/Computing/True/
False state. Computing reentry deliberately installs False; an outer computation
must not overwrite that installed result. This is the native question's
ordinary conservative result, not a universal rule that circular semantic
failures become false. `computeExhaustiveSwitchStatement` asks typeof witnesses
before an operand's base constraint, and ordinary discriminants first ask the
actual cached expression/base constraint and require a literal type before
checking all cases. Typeof Any/Unknown is exhaustive only when all eight
standard not-equal facts are covered. Other typeof types use their full facts;
ordinary literal constituents use regularized canonical identity membership.
Checking every case eagerly in either question changes the native schedule.

The formal fixtures are `fixtures/types/native-switch-flow/{flow.ts,tsconfig.json,
loose.json,native-observations.json}` and the registered single-file suite is
`tools/type-switch-flow-oracle.test.mjs`. Forty-three markers in both strict-null
states produce 86 fresh requests. They pin unrelated erroneous case expressions,
direct literal narrowing, duplicate/grouped/default typeof cases, nil witnesses,
fallthrough, all eight typeof names, escaped/template/parenthesized/UTF-16
distinctions and exhaustive versus incomplete inferred returns. The first formal
GREEN is `vrun-ea10d016-bd65-4273-8280-1343a9ea626d`: three tests, zero skips.

An independently hash-frozen formal replay is
`vrun-75bc6c27-e290-48db-aaad-a833c35170f8`: three tests, zero skips, with identical
before/after tooling and fixture hashes. Raw exploratory answers, their original
inputs, the frozen replay tooling, installed compiler API inventory and binary
identity are archived under `fixtures/types/native-switch-flow/evidence/`.
The original exploratory run did not capture all compiler-read metadata hashes;
the later replay does not retroactively fill that historical gap. The API replay
records selected source hashes and a separate installed-package inventory,
rather than claiming compilerHookOracle's complete captured-filesystem input
manifest. No semantic diagnostics pass is added to the baseline demand.

Root's final Project seven-test RED is
`vrun-dd1611a1-b7f3-4081-a656-8dcce5806349`: six missing-behavior assertions and
one existing excluded-flow control passed. The first compiled implementation
check `vrun-95672a3f-9bff-4d96-b714-0968a84a0dfb` passed four and failed three
assertions. Rejecting a changed intermediate clause type incorrectly prevented
a healthy final join. Native `getTypeAtSwitchClause` returns that intermediate
identity; `getTypeAtFlowBranchLabel` may restore the declared identity before
`getFlowTypeOfReferenceEx` normalizes the final result. The production owner now
performs excluded-flow admission on the complete read, retaining changed
per-case reads as FlowRequired. The parameter value prerequisite was separately
implemented through its existing canonical value family.

Final seven-test GREEN is `vrun-a76f05d2-3297-433e-9151-8a629ff5f183`, with zero
skips. Compatibility exposed one further genuine regression in the read suite
(`vrun-2157bdf0-4078-443e-a501-21de42ad44da`): a matching native compound
operation may base-convert a literal even when its final identity differs from
the baseline. The correction records an exact request-local proof only when a
consumed Compound antecedent equals the admitted baseline, or the compound-like
operation base-converts that exact declared baseline. Final admission accepts
only the resulting TypeId; it does not compare arbitrary types by widened base.
Pinned `flow.go:230/244` defines these two conversion sites. Switch compatibility
then passed in `vrun-84d10b83` and the read suite in `vrun-32eadc63`, zero skips.

This bounded source audit found no further admitted-family defect in that gate.
Evolving arrays, non-null normalization, intersections, global Function facts
and unsupported narrowing dependencies remain full-design obligations. Public
native oracle questions prove their own summaries/error
predicates; they do not prove product behavior, arbitrary root-causal recovery
or whole checker/cache/resource limits.
