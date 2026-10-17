# Bounded identifier-read review

This Ousterhout review follows the reviewing route and the native decision
owners in `binding/flow.rs`, `binding/flow/syntax.rs`,
`binding/semantic_flow.rs`, `semantic/source/reads.rs`, and
`semantic/evaluation/reads.rs`. It reviews the admitted identifier-read
increment, not complete F or complete T. The final native baseline is
TypeScript 7.0.2 at commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

The current boundary is coherent: binding constructs ordered flow over the
final tree; source returns exact syntax context; evaluation interprets flow
against canonical raw reference identity. Graph absence is distinct from a
reachable Start. Known nonunion identity cases do not authorize arbitrary
annotations to bypass flow. Unsupported graph nodes and constrained families
remain located failures rather than empty/no-op graph answers.

## Confirmed missing final read branch

Fresh API and same-checker hook controls show that the ordinary strict read in
`let uninitialized: number; uninitialized;` returns `number`, with a non-error
root, only after the demand emits TS2454. The loose read returns `number`
without TS2454. These sessions do not run semantic diagnostics afterward.

The pinned `checkIdentifier` owner, `checker.go:11014`, computes an optional
initial type when initialization is not assumed. Its final branch at
`checker.go:11162` checks that the declared type lacks undefined while the
flow type contains it, emits used-before-assignment, and returns the declared
type to reduce follow-on errors. At the reviewed source point the native read
owner returned the optional flow type directly, so this reachable case could
produce an incorrect successful `number | undefined` answer.

The proposed product outcome is `flow_required` when that exact branch is
consumed. The design explicitly excludes required definite-assignment analysis
as F. TS2454 alone is neither an error-type marker nor a reason to invalidate
every demand in the file. The final Project reproduction and correction belong
to the read owner; their scoped receipts are recorded below.

The same source rule distinguishes context. Strict TypeQuery operand and
direct non-null operand controls both return `number` without TS2454:
`IsInTypeQuery(node)` and a direct NonNullExpression parent separately contribute
to `assumeInitialized`. Those conditions must remain separate setup facts,
not an annotation or wrapper-wide flow bypass. Assignment position and
compound-target checking have additional early-return and base-literal rules;
the root implementer already tracks their whole-owner extension.

## Raw identity and condition order controls

For a selected initialized `count: number`, both `typeof Missing === 'string'`
and `typeof OnlyType === 'string'` preserve the native `number` read. Their
same-demand diagnostic collections contain TS2304 and TS2693, respectively.
The native unrelated-reference comparison does not require the candidate's
value type to establish the selected read. A separate raw resolved-symbol
observation therefore belongs to matching; the selected value-read operation
continues to enforce runtime value meaning and actual read attachments.

The subsequent read-only linker audit found no new blocking defect in that
bounded raw observation. `probe_entry` extends its existing Probe result with
parallel first raw ValueOrigin metadata. Need/retry follows the existing task
owner, and augmentation or cycle Unvisited resets clear origin together with
the corresponding resolution state. An unresolved alias retains its own alias
origin; only a failed lexical lookup with no origin yields the genuine unknown
observation. Builtin and unretained roles remain pending. The selected
`value_reference` operation separately compares the raw alias's actual value
target with the lexical runtime target and refuses a divergence instead of
merging their identities. Complete alias/cycle/reset variant proof remains a
final-test obligation beyond this source audit.

For a declared `never` selected inside a `typeof Missing` guard, the native
read returns `never` without acquiring the missing condition symbol. The
native condition operation consumes its antecedent before narrowing and returns
a Never antecedent early. The reviewed source now follows that order and uses
the new `ReferenceSymbol` observation. These two issues were already being
corrected by the root owner during this review; they are not new open findings.

## Exact observation and validation limits

The executable probe and twelve full fresh observations are archived in
`fixtures/types/read-review/`. The matrix has strict and loose profiles crossed
with ordinary uninitialized, TypeQuery, non-null, unrelated unresolved,
unrelated type-only, and Never-antecedent sites. Hook `beforeSummary` captures
diagnostics already accumulated by the actual checker operation. The noLib
initialization TS2318 rows remain separate from TS2454/2304/2693 demand
observations; the probe does not claim their causal dependence on this read.

At the reviewed checkpoint the existing read suite had a narrower registered
GREEN, `vrun-64872dbf-6ec5-4240-8d9e-377b986e2251`, with zero skips.
The root owner later collected the exact twelve-test assertion RED
`vrun-d7748e6b-8a68-4e1c-bad9-da00bf97013d`: eight passed and four
failed, with zero skips. It confirms the strict uninitialized wrong-success
case and distinct compound, definite-assignment-target, and readonly checking
branches. The shared Model then gained the sole `containsUndefinedType` and
`getBaseTypeOfLiteralType` operations under that genuine RED; native base
conversion is distinct from fresh-only literal widening.
The twelve-test correction is GREEN in
`vrun-78012907-ce23-41d6-9e28-f39272245edd`, with zero skips. A
further exact compound-literal reproduction is assertion RED in
`vrun-9b8b4d06-e281-407e-967c-204cf7bc64ac` (twelve passed, one
failed, zero skips), then GREEN for all thirteen final tests in
`vrun-d2395895-5963-485c-9e76-f629703a027c`, with zero skips.
These receipts prove the admitted ordinary read branches and preserve the
explicit product `flow_required` policy for the consumed definite-assignment
branch. Later source changes require their own terminal receipts. This review
does not establish full optional-chain/destructuring,
closure, automatic evolution, call-effects, branch/loop-cycle, assignment-kind,
definite-assignment, generic constraint, resource, or checking-history parity.
