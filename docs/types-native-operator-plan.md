# Native operator dependency closure

The current captured TS/TSX query worker already owns equality, logical,
null-coalescing and binary destructuring operations in `evaluation/operators`.
This next increment completes the remaining native binary/unary operator
questions; it keeps the selected T/Q3/Q4 scope and local query operating envelope.

`checkBinaryLikeExpression` (checker12298) has one dispatch owner. Destructuring
checks the right first; all ordinary families check left then right in the
supplied mode before their operator-specific work. Keep the existing Source
recipe and assignment syntax owners. Compound property assignment must acquire
the write-only setter type through the shared access owner. Plain assignment,
compound assignment and destructuring share `checkReferenceExpression` and the
reporting assignability operation; they are not the same type-acquisition
question. CommonJS export suppression is its actual native declaration-kind
branch, not a broad skip for property assignment.

Arithmetic checks silentNever before nonnull operands, then the Boolean bitwise
suggestion gate, both arithmetic relations (and native awaited suggestions),
number/bigint selection and compound assignment. Shift simplification finally
asks the shared source constant evaluator. Mixed bigint recovery remains an
InvalidSemantic consumed errorType; genuine Any/Number recovery stays distinct.
Plus retains its separate string gate, strict kind relations, symbol constraint
checks and native diagnostic path. Comparisons own their base-literal comparison
question, not ordinary literal widening. In/instanceof require their complete
well-known-symbol/reference and private-name dependencies before admission.

`reportOperatorError` and `getBaseTypesIfUnrelated` must have a single owner
covering both current equality and new arithmetic/relational families. Native
awaited operands are acquired only with a relatedness callback. If that check
does not suggest await, literal bases replace the effective display pair only
when the callback still rejects the bases. `getTypeNamesForErrorDisplay` then
consumes both effective operands (and may repeat with qualified flags). The
current equality helper omitted that final display demand; this is a correctness
repair required independently of broader arithmetic implementation.

`isGlobalNaN` is distinct from constant `evaluateEntity`: it compares the global
NaN symbol with actual getResolvedSymbol on the identifier. Both use the shared
getGlobalSymbol Value operation, with their distinct resolution/order rules.
Neither a spelling match nor ordinary value type establishes global identity.

Required proof: all native operator families and assignment forms, strict/loose/
exact states, const/global shadowing, number/bigint/Any/Unknown/Never/nullability,
setter read/write asymmetry, failed awaited/display dependencies, same-type vs
literal-base diagnostic pairs, and shift/constant numeric boundaries. Compare
fresh pinned before-summary diagnostics and dependency observations, then assert
final Project values or original failure locations. Source/expression and
resource limits remain independent of the compiler's own semantic recovery.

The first repair moves the complete `getTypeNamesForErrorDisplay` operation,
including its context selection, into `native_display`. Keeping it private to
relations would either duplicate the rule in operators or make operators depend
on relation-reporting details. Its three existing relation callers now use the
same display owner. `reportOperatorError` retains the optional compatibility
callback, so absent-callback diagnostics still serialize their effective pair.

Pinned operator-display proof passes 39 fresh queries plus exact diagnostic
text in `vrun-6457a8e9-c6b4-4693-94d5-f60889460c2b`. The final Project test
fails before the repair in `vrun-06642a1d-aefc-4dd3-af7f-2afd2db5cf7e`: a
required equality diagnostic silently skips `Missing` in an optional property.
Independent review confirms the repaired awaited/base/display schedule and
the existing qualification retry. Post-repair Project proof remains pending.

The assignment audit found that the existing
`checkTypeAssignableToAndOptionallyElaborate` adapter serves only validated
reference targets. Its limited expression traversal covers those current
callers, but cannot serve arbitrary binary right-hand sides. The existing
`satisfies` operation also skips this required elaboration/reporting path after
a failed silent relation. Completing these operations requires one owner for the full admitted
`elaborateError` decision: binary equals/comma, object literals, array literals
and arrows, including contextual tuple rechecking and `elaborateElement`.
The actual diagnostic node and elaborated expression are different arguments
for binary assignments. Preserve both rather than passing the left node for
both questions. The exact-optional head-message branch also acquires the
receiver's property type even when the eventual relation would succeed.
JSX-specific elaboration remains within the existing JSX exclusion.

The first operator-display repair passes both final Project tests in
`vrun-43874fb3-fe1f-4333-8a79-ce748a267c99`, following its recorded RED above.
The larger operator matrix is being captured separately; these two tests do
not establish assignment/arithmetic completion.


The implementation now places `checkTypeRelatedToAndOptionallyElaborate` and
its complete admitted elaboration dispatch in `relations/elaboration.rs`.
Keeping the reference-only helper and adding separate satisfies/binary helpers
would duplicate callable suggestions, contextual frame lifetime, optional
property rules, and reporting reads. The shared owner takes the actual reporting
node separately from the expression being elaborated; its callers only supply
those native inputs. The obsolete reference-only traversal is removed.

`with_contextual_type_override` owns balanced non-cache frames on success and
failure; it does not create an inference scope or perform the distinct
`checkExpressionWithContextualType` literal regularization. Related-information
checks read captured default-library provenance and raw declaration identities,
including JSON, rather than classifying paths or fabricating syntax nodes.
The existing optional-mismatch reporting filter now uses the same complete
`isExactOptionalPropertyMismatch` operation as assignment and element heads.
These changes serve one request-local evaluator over captured files and add no
cross-request synchronization or durable state.

The maintained operator oracle passes all 321 exact observations in
`vrun-76f53c95-83ea-425f-9414-8bd3d04ce934`. Arithmetic, unary, binary assignment,
elaboration, conditional/delete/in changes are staged, with final compile and
Project validation outstanding. `instanceof`, private-name checking and the
remaining tested-symbol usage branch still require their full native owners;
this checkpoint does not establish complete operator support.


Adversarial review found that mixed numeric arithmetic previously stopped at
its error-type result before native's compound-assignment and shift evaluation
continuation. The implementation now retains the actual native error singleton
at its startup position, distinct from genuine Any, and carries the original
product failure separately through that bounded continuation. Union and
intersection error inclusion use the same identity predicate, with wildcard
precedence preserved. This is a real native recovery value only for its explicit
producer, not a replacement for arbitrary failed demands. The primary failure
always wins over a successful continuation; later semantic failures append in
order and whole-operation failures stop immediately.

The existing failure-composition operation was already used by relation,
alias, node-type and mapped-property consumers. It now lives on DemandError as
`append_failure`, so operator continuation reuses the complete policy rather
than depending on relation-reporting internals. Its callers retain the same
ordering and bounded provenance behavior.

Native elaboration capture and maintained assertion both pass all 114 rows
(`vrun-f5f3823d-d2c8-4845-92f4-37020aa52cb0`,
`vrun-dc99bda2-8fee-4374-94cf-7bb91e198f30`). Conditional/delete/in capture
passes 87 rows (`vrun-21e73a9b-9fd9-4ae5-8181-1ff46271a0cb`). Mixed-recovery
capture passes 18 rows (`vrun-58deab4a-e62a-4a68-add5-2fe31256d089`), including
Missing setter annotations consumed after the mixed arithmetic error, and a
genuine Any plus result without error recovery. Final Project tests are staged;
these compiler observations are not yet a Rust implementation pass.

The maintained conditional/delete/in oracle also passes all 87 exact rows in
`vrun-6c50a9e6-6024-4814-a326-9ef26d45f393`. Recovery capture now covers 24
rows in `vrun-bf82b0ef-7d2e-49ea-bb68-2f580c40973e`: a branded numeric operand
first consumes its hidden display annotation, then produces arithmetic recovery,
then acquires the setter annotation. The final Project test asserts that exact
primary/cause ordering. The extended maintained rerun and shared Rust checkpoint
are running; the expected results remain unchanged.

The next truthy-check increment will separate the external selection prelude
from the actual-node `getSymbolAtLocation` owner. Reusing the current combined
operation inside checker diagnostics would execute enclosing calls again and
make symbol lookup depend on selection machinery. Duplicating the dispatch
would instead give the same symbol question two owners. The actual-node owner
will retain the existing native recipe and links; the selection adapter will
perform its prelude before delegation. Binary-chain usage must start at each
right operand's children (not that operand itself), preserve early success,
and use actual symbol identity. Promise diagnostics consume the shared native
display operation only after neither chain nor body reports a use.

The extended recovery oracle's maintained assertion passes all 24 rows in
`vrun-e949dbbf-abc4-4d16-83d0-4ec14f2584c0` (three tests, zero skips). The first
shared Rust compile (`vrun-65909d70-74a3-4c6c-98cb-e8876a421958`) found five
integration errors before executing tests: Source expression/text owner calls,
query-binding default-library access, and the new reverse-mapped association
match. The fixes reuse those existing owners; the second compile is pending.
