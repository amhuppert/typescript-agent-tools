# Null, void and variable widening

This native increment extends the existing evaluator; it does not close the
T/Q3/Q4 delivery. The fresh pinned controls are
`tools/type-special-expression-oracle.test.mjs` and the immutable fixture
`fixtures/types/special-expressions`. Twenty-two declaration questions cover
all four strictNullChecks/noImplicitAny combinations, plus the direct null
expression token in both strict-null states.

`checkExpression` returns nullWideningType. `checkVoidExpression` schedules its
operand through checkNodeDeferred and returns undefinedWideningType without
demanding that operand for the immediate type answer. Thus `void MissingValue`
establishes undefined. Exported const, let and var declarations use complete
getWidenedType after literal initializer handling: null/undefined stay themselves
under strictNullChecks and widen to genuine any otherwise. A const initializer
does not bypass nullable widening.

`getTypeForVariableLikeDeclaration` selects autoType for nonambient, nonexported
mutable declarations when noImplicitAny is enabled and the initializer is absent
or null, including parenthesized null. Automatic any has a private identity
distinct from genuine any. Its variable declaration type is valid; a future
automatic/evolving use-site read must pass the design's flow admission boundary.
The resolved intrinsic undefined-symbol branch remains pending alongside native
identifier acquisition; no name-spelling approximation has been added.

`checkPrefixUnaryExpression` consumes its operand type but discards the returned
checkNonNullType value for plus/minus/tilde. The nullable diagnostics therefore
do not turn these number results into consumed error types. Plus bigint also
returns number despite its diagnostic. Null, undefined and void have Falsy-only
facts in getTypeFactsWorker under both strict-null states, so their negation is
the true literal. These observations do not admit unchecked unresolved operands.

The whole pinned ModifierToFlag/ModifiersToFlags rule now owns syntactic bits.
Existing has_modifier callers use it. The existing combined node-flag traversal
moved whole into syntax, shared with the distinct GetCombinedModifierFlags
question; export is obtained from the variable statement, not symbol metadata.

Registered compiler controls first passed two tests without skips in
`vrun-61fbdad7-4353-4450-a6f5-99fc72a9b0d7`; the expanded four-option-state
matrix passed in `vrun-4d084334-ffb4-4621-b88a-7ffeabe1d0c8`. These are fresh
type requests, not a diagnostics-first checking history or recovery inventory.
Native final-operation reproduction first failed all four tests in
`vrun-8f7208b3-6598-4bd8-b1af-ed4cd5986baa`: null/void construction was
explicitly pending and the null token was unselected. The expanded reproduction
retained those assertion failures in
`vrun-1151e69c-12bd-4e9f-8107-d28f9e0c01b2`. After the owner changes,
`vrun-4a377cee-1ad7-4d92-8276-3a52ccd5c8a7` passed the four public tests
with zero skips. No setup or compile error is reported as a behavior RED.

## Complete structural truthiness distinction

The native `IsEmptyAnonymousObjectType` / `isEmptyResolvedType` question is
owned by Model and uses actual Anonymous versus Interface/Class/Reference
kind independently from nominal display text. All four known collections
matter: properties, call signatures, construct signatures and index signatures.
Source supplies actual factory kind, and evaluation consumes this operation.
An empty interface is truthy under strict null checking even though an empty
anonymous object retains both truthy and falsy facts. Ordinary named references
need no member inventory merely to establish that they are not Anonymous.

The final five-test RED
`vrun-1f6359d7-4711-4394-a596-5c4c1edd516e` had four passing tests and one
assertion failure: negating a function incorrectly returned boolean rather than
false. All five passed without skips in
`vrun-9dbb35a4-539d-498f-a9ca-0ce0633a2be3`, covering callable, constructor-only,
index-only, empty anonymous and empty interface objects under strict/loose null
states. Fresh compiler controls passed all three tests without skips in
`vrun-ae5dcfb0-5574-45c9-9cd2-b0b08a21a9a9`. The independent model-core baseline
also passed six controls without skips in
`vrun-22812617-5925-40f5-870e-6add37ce3c7e`.

General constrained/composite truthiness and the special anyFunctionType factory
remain required native questions. This increment does not infer an ordinary
Object from unresolved generic member shape.
