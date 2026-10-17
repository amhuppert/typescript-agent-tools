# Native narrowable reference operation

This Ousterhout design assumes an accepted immutable Project, canonical native
TypeIds and exact captured reference sites. It extends the existing deep
evaluator and constraint owner. The presenter does not choose checking mode,
discover contexts or substitute a displayed parameter name for type identity.

Two designs were considered. A reader-local exception for unconstrained T would
leave assignment questions, union constituents, context acquisition and failure
ordering duplicated. A single evaluator `getNarrowableTypeForReference`
operation owns the whole native decision and calls the existing base-constraint,
expression, contextual and union operations. Source projects exact syntax and
context recipes through its existing attachment owner; it does not decide from
spelling whether a constraint or context is irrelevant. The latter design
hides the checking-order and identity decisions once without creating another
symbol inventory.

TypeScript 7.0.2 checker.go:31248 first unwraps actual NoInfer substitution
types. Normal checking then asks ordered `someType` of
`isGenericTypeWithUnionConstraint`; Inferential checking skips this entire
predicate and every position/context question. The predicate recurses through
actual intersections and otherwise consumes the native base constraint only
for Instantiable flags. A resulting Nullable or Union constraint enables the
later position/context question. Missing or circular recovery consumed here
remains an invalid dependency even if native subsequently returns a healthy T.
An unconstrained T or primitive constraint keeps its original identity and
does not inspect a sibling Missing context.

`isConstraintPosition` uses the actual immediate parent. PropertyAccess and
QualifiedName qualify; a Call/New qualifies only for its invoked expression.
ElementAccess qualifies only for the receiver and has the native exception:
when some receiver constituent is generic without a nullable constraint and
the actual checked index is generic, preserve T[K]. That exception asks the
index expression only after the receiver predicate succeeds. It is not an
identifier-text or constant-value heuristic.

If position is false, `hasContextualTypeWithNoGenericTypes` admits only the
native Identifier/PropertyAccess/ElementAccess shapes and excludes JSX tag
names. It asks `getContextualType` with the actual RestBindingElement flag,
then the complete `getGenericObjectFlags` question. Generic here means native
top-level instantiable/mapped/tuple/index/string-like state, not recursively
searching every Reference argument or object property. Captured initializer
annotations, parenthesized/non-null forwarding and annotation contexts route
the existing type-node owner. Unimplemented contextual/inference families
remain precise consumed dependencies; they must not silently become absent
context. Future NoInfer, intersection, mapped, tuple and template factories
must extend this whole owner when established, without substituting guessed
scalar types.

When substitution is selected, ordered native `mapType` replaces every actual
constituent by `getBaseConstraintOrType`, rebuilds through the existing Literal
union owner only when an identity changed, and preserves an unchanged union's
actual identity. Native Boolean union constituents remain semantic types even
when their display view is Primitive(Boolean). No origin/name cache is added.

The ordinary reader calls this operation after assignment/non-variable early
returns and declaration eligibility, before control-flow container/capture
marking. Assignment's `getInitialOrAssignedType` calls the same operation in
Normal mode using the selected reference site, after the actual initializer or
assigned-expression question. The two operations therefore share the native
question without merging their expression caches or role-specific scheduling.

Formal native suite `tools/type-narrowable-reference-oracle.test.mjs` passed two
tests and forty fresh selected roots, zero skips, in
`vrun-6b3eb7ac-505a-4675-8dbe-57de8aad730b`. It distinguishes unchanged T,
primitive/nullable/union initializer contexts, generic context, exact element
receiver exception, actual Call/New preludes, skipped Missing contexts and
strict/loose union constituent substitution. A copied same-checker scratch
overlay calls the exact Normal/Inferential operation without a preceding
location demand; its mode/phase witnesses are research evidence pending the
complete frozen archive. The four final Project tests in `tests/type_narrowable_reference.rs` produced
compiled assertion RED `vrun-5246a92f-11b6-436f-a16a-d0f4871f2dae`: four executed,
four expected missing-behavior failures, zero skips. Production then moved the
complete decision into Source/evaluation narrowing owners. The first GREEN
attempt `vrun-87d28885-6739-4972-91a6-6d3435366c94` stopped on four mechanical
compile gaps (collector module paths and exact Site equality), so it provides
no behavior verdict. No implementation completion or reduced
native delivery is claimed by the compiler controls.

The compiled first behavior checkpoint `vrun-07819d70-525c-40d6-8ea8-0420b0986083`
passed three families and failed only the test's product-reason assumption.
The dependency already failed as InvalidSemantic/UnresolvedSymbol at the exact
Missing type reference. Native `isErrorType` describes the recovery type, not
the product's original dependency cause. The existing binding failure owner
(and prior generic/signature tests) requires preserving UnresolvedSymbol here.
The test was corrected to that reason plus the original path/position; this
is an expectation correction, with no production fix or softened failure.

Final scoped Project GREEN is `vrun-fdd13747-eed1-45a4-b1a1-756150e0c963`,
one matched file, four tests, zero skips. Native compiler controls remain
`vrun-6b3eb7ac-505a-4675-8dbe-57de8aad730b`. Bounded independent source review
found no extra admitted Parameter/primitive-union mismatch in the native
predicate, element exception, context precedence and unchanged union identity.
This proves the reachable Normal reader/assignment increment. Full shared
CheckMode, NoInfer/intersection/mapped/variadic/string-like factories and
contextual/inference families remain required extensions of these same owners;
this receipt is not full T/Q3/Q4 completion.
