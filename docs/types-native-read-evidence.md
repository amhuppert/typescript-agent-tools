# Native identifier-read increment

The operating envelope is one captured immutable Project and one isolated
request-local evaluator. This is a proved read increment within T, not complete
flow infrastructure or T/Q3/Q4 delivery. F remains excluded.

The implementation follows the Ousterhout alternatives in
[the flow plan](types-native-flow-plan.md): one retained binder graph and
complete checker operations over actual symbol/site identity. Source exposes
grammar and declaration flags. Evaluation owns reference matching, readonly
checking, mutable-local policy, assignment kinds, initial inputs, and native
condition order. Shared `GetAssignmentTarget`, `IsAssignmentExpression`, and
`IsLeftHandSideExpression` owners serve both binding and read setup.

The first six final Project regressions failed in
`vrun-0b40d0d4-3fbd-4045-a11e-2fd49933f121` and passed in
`vrun-64872dbf-6ec5-4240-8d9e-377b986e2251`. Shadowed-name controls compare actual
canonical symbols. The Never-antecedent regression failed in
`vrun-60a571a2-5e17-4544-b9ce-6910fb8f12b5` and passed in
`vrun-9a3be93f-caa5-4133-896e-5f3f5fffc3b0`: `getTypeAtFlowCondition` returns
Never before acquiring its guard. The unrelated unresolved condition failed
in `vrun-ac55a708-3586-44f6-b2c5-aa3ee19bc73f`; the retained raw-symbol operation
then passed that control among the eight passing tests in the next twelve-test
RED. Raw alias identity is separate from the selected runtime value target.

The twelve-test RED was
`vrun-d7748e6b-8a68-4e1c-bad9-da00bf97013d`: eight passed and four failed at
the missing assignment/read assertions, with no skips. Its strict uninitialized
read incorrectly succeeded as `number | undefined`; the native assignment
position lacked its early union return; readonly assignment lacked consumed
error-type recovery; and a regular literal compound target remained `1`
instead of `number`. After those corrections all twelve passed without skips
in `vrun-78012907-ce23-41d6-9e28-f39272245edd`.

Compiler controls in `tools/type-native-read-admission-oracle.test.mjs` passed
nine tests without skips in `vrun-291a4a20-8770-4abf-a50f-ad0ce1fbab75`. They
cover all assignment operators, increment/decrement, logical assignments,
for-in/of targets, literal and literal-union arithmetic conversion, and strict
versus loose inputs. `getBaseTypeOfLiteralType` is distinct from fresh-literal
widening. The whole native `containsUndefinedType` question consumes the first
canonical union constituent's Undefined flag.

The [independent read review](types-read-review.md) archives twelve additional
fresh same-checker controls. It confirms that required definite-assignment
analysis produces `flow_required`, rather than fabricating an error-type marker
from TS2454. Direct non-null and TypeQuery context setup remain separately
required. It also distinguishes a reachable Start, absent flow, and actual
unreachable sites.

Matching compound-assignment interpretation has
compiler ten-test GREEN `vrun-41067457-86b6-4f5d-9812-d07cf12a939e`.
`vrun-4d50b3f5-f943-4184-a48d-bd4d114c0404` failed during concurrent generic
scaffolding compilation and is not a behavior RED. Full graph construction,
capture/default/destructuring setup, effect/end reachability, bounded cycle
proof, semantic resources, service replay, and retained graph measurement remain
required before the full delivery gate.

That matching-assignment extension failed at its final wrong literal result in
`vrun-9b8b4d06-e281-407e-967c-204cf7bc64ac` (twelve passing, one failing), then
all thirteen passed without skips in `vrun-d2395895-5963-485c-9e76-f629703a027c`.
Compound assignments consume their antecedent and then native literal-base
conversion. Simple arithmetic assignments convert the declared literal family
before asking whether assignment-based union reduction is needed. The same
operation covers an ambient union compound read without inventing narrowing.

Direct non-null initialization context has separate final proof. Its fourteen-
test RED `vrun-08749902-a600-4b69-a83b-4ae5209426b5` had thirteen passing tests
and one assertion failure: a direct non-null operand incorrectly required
definite-assignment flow. All fourteen passed without skips in
`vrun-25d29996-2055-4359-9257-941bcdec76a5`. A direct `NonNullExpression` parent
contributes to native initialization setup; an intervening parenthesis does
not. Automatic-in-non-null initial input remains Undefined, and automatic
evolution remains a separate F blocker. Eleven compiler controls passed without
skips in `vrun-0b4ec65a-1c46-46de-ab66-82c3509ea2ef`.

Alias read setup and native Start container identity have separate proof. The
first fifteen-test RED `vrun-8b9c0bbf-8784-4ebb-8f7c-f599c29da477` failed at
ordinary import initialization. After acquiring the actual alias declaration's
flow attachment, the next RED `vrun-93cb0790-af3f-4f49-8887-8952029000ba`
failed at an arrow's own captured Start. All fifteen final Project tests then
passed without skips in `vrun-c6cb5e9c-0be5-4ab6-89b7-01e7731e3deb`.
Twelve compiler controls, including the expanded arrow variants, passed without
skips in `vrun-40b7c9e8-87f3-461f-9a04-eebab3546818`.

Raw aliases retain their own immutable declaration/read attachment. Read setup
never borrows the resolved value target's container or assumes two imports of
one target have one raw symbol. Imported union types do not consume the target
module's initializer refinement. A matching alias guard still requires F;
a guard for a different import of the same value is irrelevant. The native
Start operation compares actual container IDs: an arrow's own Start yields its
initial type, while crossing an outer Start remains a separate capture demand.
The same control proves a local non-null operand in the arrow, so the correction
is not an alias-only exception. Retained alias/flow cost still needs measurement.


Ordinary iterative statement reads have final sixteen-test RED
`vrun-d509c0e7-3fb9-4fe6-8b42-cb494d2cb7fa` (fifteen passing, one failing at
`do` construction) and GREEN `vrun-2600ebc1-6049-4f66-a52a-33e624a47d6a`,
with no skips. Thirteen fresh compiler controls passed without skips in
`vrun-cf670e98-cafe-4db8-80fc-c632bb082653`. The controls cover while/do/for/
for-in/for-of reads, unreachable bodies and code after an infinite loop, strict
and loose inputs. Construction preserves each loop's native continue target,
initializer/condition/incrementor order and actual entry/back-edge identities.

`getTypeAtFlowLoopLabel` consumes its first entry antecedent before later loop
paths. When that actual type is the declared type, the native rule stops before
any back edge and its one-type union preserves identity. The interpreter follows
that rule rather than treating a back edge as a missing proof. Incomplete-loop
union/subset processing, labels, effect/end reachability and the remaining full
construction/resource matrix still require their own implementation proof.


Labelled flow has final seventeen-test RED
`vrun-cacb4526-7082-4c05-8064-70c6227b46b8` (sixteen passed, one failed at
label construction) and GREEN `vrun-60a779a9-216d-48cb-b617-efa01818f9e2`,
with no skips. Fourteen compiler controls passed without skips in
`vrun-3d72c63f-b468-426d-897c-025c29940818`. The transient active label owner
ports `bindLabeledStatement`, `findActiveLabel`, `setContinueTarget` and
`bindBreakOrContinueFlow`, preserving decoded exact label names, nested wrapping
labels, loop-kind targets, and unreachable code. Container entry clears labels
and completion restores the enclosing list as `bindContainer` requires.
Unused-label diagnostics do not affect these query flow answers.

A broader Rust checkpoint `vrun-9dde03ac-a9f4-42b4-9749-921af4cfc39d`
stopped in the existing service protocol boundary test because concurrent edits
changed the live source identity after binary compilation. The scoped stable-
source service run passed all twelve controls without skips in
`vrun-d05e8764-61e1-439b-b86d-c0da6e9c1eaa`. No service identity assertion was
weakened, and the failed wider run is not recorded as a compatibility pass.

Native call effects have eighteen-test GREEN
`vrun-b0645bac-a837-48a4-b001-2cf4d79d90fe` after the previously recorded
missing-effect behavior RED. The ordinary predicate/never-return extension has
nineteen-test RED `vrun-eda782d6-4742-4121-b6c0-afb9c0cb5ec5` and GREEN
`vrun-81a2a426-4d2c-4d64-b866-d798b93b8ecc`, with no skips. An ordinary Is
predicate does not narrow a statement read. A native Never return supplies the
distinct unreachable-never sentinel before reading the call antecedent; final
reference checking normalizes that sentinel to its declared type.

Assertion reachability has twenty-test RED
`vrun-888274bb-838e-4392-98c3-2aa7a5f9a6b3` and GREEN
`vrun-6b7a2df2-7264-4562-859f-f51656222031`, with no skips. Compiler sixteen
controls passed in `vrun-d4af6694-7753-4614-96b0-d6580957fd36`. The complete
native syntactic `isFalseExpression` owner preserves parentheses and short-
circuit AND/OR order without checking unrelated argument values. Matching
assignments on these unreachable paths retain the sentinel. Native assertion
read narrowing itself remains a distinct operation that must consume its
incoming flow before interpreting the argument.

Logical and nullish graph construction has twenty-one-test RED
`vrun-c7c2954d-e591-496e-ab3f-88947a8f9e9d`, followed by the meaningful
assignment-condition RED `vrun-4849cd57-1e99-4a6a-9f90-5f8677b01ed6`, then
GREEN `vrun-d5e3a425-5e3b-4aa7-8cc7-a2de91752a1c`, with no skips. Compiler
seventeen controls passed in `vrun-08f63d35-e1aa-4010-8159-a9fbd520fa70`.
Native construction preserves branch targets, negation and pure-expression
restoration. Assignment narrowing consumes the right condition before left
truthiness. Matching truthiness uses the complete adjusted-facts operation and
admits only unchanged canonical type identity. Conditional-expression shape and
effects also pass both final review controls in
`vrun-a00a259b-ba76-4b95-9ab6-e945290fdd0f`. The independent
[bounded graph review](types-logical-flow-review.md) records the remaining
optional-chain, switch/try, narrowing and resource obligations.

Assertion read identity has twenty-two-test RED
`vrun-1af7128a-8e7a-41d0-bbd3-b8e6093562b5` (twenty-one passed, one failed
at the missing assertion operation) and GREEN
`vrun-3aa0ef8c-fb9e-45fe-896a-ad3ed2c247a5`, with no skips. Eighteen compiler
controls passed in `vrun-1d8769c6-3a0e-4d9e-b9bc-153349d082a5`. Read
assertions consume their actual antecedent first, then the separate
`narrowTypeByAssertion` question. A false literal produces unreachable-never;
AND feeds the left result into the right, while OR checks both from the original
input and uses the native union operation. The broader syntactic false
reachability query does not replace this operation. The controls distinguish
strict/loose literal falsiness, harmless number truthiness and literal 0/null/
negated true default branches. A union-refining incoming flow remains
`flow_required` even when the final false assertion would normalize to the
declared type. Typed predicate relations and remaining native narrowing cases
retain their required admission work.
