# Equality flow acquisition

The operating envelope remains an immutable accepted Project and one local
request-owned evaluator. This repair uses the Ousterhout skill and keeps the
native decision in the shared reference-flow interpreter. Predicate inference
does not have a second condition evaluator, and F remains excluded.

The existing final function control `constantComparison` was genuine RED in
`vrun-b935921d-68bd-4179-9c61-611ef5a72391`:
`(x => 1 === 1) satisfies (x: number) => boolean` returned Partial. The actual
parameter selector had already succeeded. The shared equality arm rejected all
non-typeof comparisons before native reference matching could establish an
unchanged result.

`reads/equality.rs` now owns native `narrowTypeByBinaryExpression` equality
dispatch (flow.go469–532), `narrowTypeByEquality`, optional-chain containment,
typeof/literal dispatch, discriminant callbacks and constructor comparison.
The operation preserves candidate, typeof, matching-left, matching-right,
optional-left-or-right, discriminant-left, discriminant-right, constructor-left,
constructor-right and Boolean-recursion order. Only completed reference flow
compares its actual result against the admitted identity. Intermediate narrowing
can still cancel at a native join.

The existing switch owners for typeof facts and primitive-to-literal replacement
are shared and extended to the current intersection/template/string-mapping
factories. Native unit-like checking acquires the base constraint before scanning
an intersection, while unit extraction separately reads the original type.
Both switch and equality consumers use those same operations. Cooked UTF-16
typeof witnesses remain syntax facts, independent of semantic string literals.

Any equality returns before acquiring the opposite expression. A normal number
comparison consumes that expression even when native error recovery leaves the
final type unchanged. Product admission preserves the acquired invalid failure.
Similarly, constructor inequality returns before acquiring its opposite value;
constructor equality checks Object before call signatures, then constructor
signatures, prototype raw type, global identity and class-symbol equality or
Subtype. A typeof Function comparison consumes the global factory, so its
recovery does not become healthy merely because identity-only global questions
can observe the native sentinel.

The native `getKeyPropertyName` optimization is still an explicit dependency
after its exact acquisition-free gates: union length at least 10, nonprimitive
union flag, and at least 10 Object/InstantiableNonPrimitive constituents. Its
complete future owner must retain the actual union key-name/map cache,
source-order candidate property/raw-type acquisition, regular-literal keys,
duplicate Unknown sentinel, count-at-least-10 and at-least-half thresholds. It
must serve relation and flow consumers together. The present gate establishes
only native negative cases; it does not claim the large-union factory complete.

Adversarial source review covered native flow469–793,1019,1886. The maintained
native fixture `native-equality-flow` and final `tests/type_equality_flow.rs`
distinguish all four equality operators, operand order, Boolean wrappers,
matching and unrelated typeof, skipped and consumed missing types, nullable
profiles, optional containment, ordinary discriminants and constructor gates.
The existing zero-demand `referenceFlow` hook witnesses sameDeclared and
sameInitial; summaries alone do not establish identity. Expanded and unexpanded
final requests must preserve unchanged success, exact FlowRequired admission,
and acquired InvalidSemantic failures. Native baseline
`vrun-adb90d6d-5036-4fda-8adf-d995875bded2` passed all 81 observations across
strict, loose and exact profiles, with zero skips. Final Rust receipts are
recorded after the coordinated checkpoint; production compilation is not yet
claimed.
