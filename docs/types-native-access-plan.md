# Native property and element access

This is an implementation stage within the full T/Q3/Q4 design, using the
immutable accepted Project and a request-local native Evaluator. TypeScript
Go 7.0.2 commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b` defines the operations.
No property lookup may reconstruct identity from graph origins or names alone.

The expression owner currently retains accesses as explicit pending work.
A complete access operation belongs in `evaluation/access.rs`, fed by exact
Source access recipes. Moving every access branch into expression dispatch
would hide receiver/key/flow scheduling among unrelated expression families.
The access operation instead hides those decisions and reuses canonical
property, apparent, index and flow owners. Dot and element dispatch remain
separate where native asks different questions, sharing flow completion.

`checkPropertyAccessExpression` checks the receiver for null before
`checkPropertyAccessExpressionOrQualifiedName`. The latter widens for writes
and method calls, obtains its apparent type, handles native Any/silentNever,
looks up the actual property symbol, and only then considers an applicable
index. Generic writes restrict index fallback. The selected property's
read/write value is a canonical property-symbol question; readonly writes
consume errorType, while readonly index diagnostics alone preserve the type.
Accessibility and before-declaration checks can themselves have dependencies
and cannot be dismissed because diagnostics are outside the response format.
Optional chains use native optional-marker removal and propagation, distinct
from ordinary null removal and from an optional property's missing type.
Exact private and unique names retain their semantic identity.

`checkElementAccessExpression` checks the index expression after obtaining and,
when appropriate, widening the receiver, even when that receiver will return
silentNever/errorType. `getIndexedAccessTypeOrUndefined` owns union index
ordering, read unions versus write intersections, numeric for-in handling,
tuple bounds and deferred generic index factories. An element literal which
names an actual property preserves that property's symbol. Index results use
the same applicable-index operation as dot fallback; exact index lookup remains
a different question. `checkIndexedAccessIndexType` follows access flow.

`getFlowTypeOfAccessExpression` handles definite writes and missing removal,
non-variable/member early returns, automatic constructor properties,
narrowable-reference constraints, strict-property-initialization setup and
compound literal-base conversion in native order. Changed final flow retains
the existing F boundary; an identity result must still execute and establish
its consumed dependencies. A scalar display or initializer position does not
justify bypassing this operation.

Binding's existing `isNarrowableReference` syntax owner decides which accesses
receive native `bindWorker` flow before child binding; unreachable bindChildren
clears it. Source consumes this attachment, not the receiver's attachment.
`isMatchingReference` is extended as one complete owner for identifier/private,
this/super/meta, access, qualified name, assignment/comma and wrapper cases.
Stable identifier element keys require actual symbol identity and native
const/unassigned-local facts. The native `getFlowReferenceKey` owner similarly
adds access and wrapper keys. Its non-dotted sentinel makes a loop return the
declared type immediately; it does not mean continue traversing without cache.

Final controls cover dot versus literal/computed element reads, decoded names,
optional properties and chains, primitive apparent wrappers, numeric/string
index precedence and unchecked-index options, read/write accessor asymmetry,
readonly property versus index recovery, missing and unused bad members,
receiver/index demand order, stable and mutable computed keys, loops and
assignments. Native controls precede each final Rust assertion. The initial
access suite is expected RED at the existing expression boundary; it does not
narrow the complete access or Q3 scope.

The initial pinned controls pass `vrun-0e9f91fb-10e4-4df4-a2c6-543a025be28e`
(five tests, zero skips): 23 final declaration queries per strict, loose,
unchecked-index, exact-optional and dot-index-restriction profile. Both written
and decoded keys, no-substitution templates, singleton/union computed keys,
optional chains and numeric-name index choice are included. The matching final
Rust suite is authored and awaits its initial assertion-level RED. Error and
write/flow variants above remain separate required controls.

The first implementation snapshot compiles (`vrun-6d11b355-5039-42de-8c78-85e0dfefd43c`).
Its final test advances through ordinary, literal/computed, readonly and optional
member reads, then reaches the still-pending native global `NonNullable`
dependency at `chainAny`. This is an assertion failure, not access-suite GREEN.
The shared nullable owner is now being completed; pinned controls for missing,
wrong-kind, zero/two-argument, custom, identity, intersection and broken aliases
pass all 16 strict/loose cases (`vrun-0c620015-4801-409d-9655-244d7bc152cf`).
They demonstrate that a custom global alias changes optional-access semantics;
a hard-coded non-null filter would be incorrect.

Access capture now retains the actual native flow attachment. Named loop keys
preserve unescaped property suffixes (including the native dotted-name collision)
and distinguish identifier/this roots. Stable dynamic identifier suffixes remain
pending actual numeric symbol-ID allocation; assigning binder indices would
invent a different key. Qualified/meta reference matching and several indexed
operator/class factories remain required work, not silently admitted cases.
Adversarial review corrected BindingElement constant-key acquisition, assignment
LHS grammar in reference matching, and the native Property-bit gate on the
readonly constructor exemption. The new shared generic-flag and mapType owners
also serve reference narrowing, avoiding duplicate semantic decisions.


Qualified TypeQuery names now share actual property access evaluation:
`checkQualifiedName`, `getRegularTypeOfExpression`'s immediate right-side
promotion, `bindWorker`'s TypeQuery-only flow attachment, and the asymmetric
qualified/access cases of `isMatchingReference`. The same raw lexical-symbol
identity owner serves runtime and TypeQuery contexts; actual value reads retain
their distinct context-sensitive policy. Qualified loop keys retain native
name concatenation. Module `includeTypeOnlyMembers`/export-star provenance is
still an obligation of the unimplemented raw module factory and option-bearing
lookup, not something inferred from an ordinary property vector.

Ousterhout review removed value construction from call-effect annotation
eligibility. Source's `getExplicitTypeOfSymbol` declaration recipe now observes
actual ValueDeclaration syntax, annotation presence and the subsequent for-of
expression dependency. It does not acquire member/variable initializer recipes
merely to find out whether a declaration has an annotation. Prototype and
independent merged property flags remain with their canonical member owner.
