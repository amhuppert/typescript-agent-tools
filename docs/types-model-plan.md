# Shared native type model

This owner constructs request-local semantic identities. Evaluation supplies
compiler-established source and binding facts; presentation receives complete
immutable views. Factories preserve literal freshness, compiler ordering and
typed failures, rather than asking the graph renderer to infer semantics.

Pinned 7.0.2 `getRegularTypeOfLiteralType` returns each fresh literal's regular
identity. Both forms share one exact payload and distinct caches; expression
widening only widens the fresh form. `newIntrinsicType` supplies genuine
successful special types. Recovery `any`, missing optional slots and widening
null/undefined identities are separate compiler concepts and must not be
collapsed into these successful intrinsic identities.

The `getUnionTypeEx` family flattens unions, ignores never, sorts with
`CompareTypes`, and applies the requested reduction. Literal reduction and
subtype reduction are separate operations. Sorting first uses compiler flags,
then alias/interface names, canonical symbols and type-specific data; exact
string/number/boolean values are not ordered by request allocation ID. Named
symbols use containing-program file index and source position. Object relation,
named-union origin and cache behavior remain implementation obligations.

Boolean is the compiler's regular false/true semantic union. Its public graph
classification is primitive boolean, while flattening a larger union consumes
its actual constituents. `formatUnionTypes` separately collapses adjacent
false/true display clauses and moves null/undefined to the end; graph order
retains the native semantic constituent order.

`createArrayTypeEx` consumes the established global Array/ReadonlyArray type.
Missing global targets yield the compiler's shared anonymous empty-generic
recovery type and a global diagnostic. This is consumed recovery, not an
admitted healthy empty object for a product array demand. A noLib option does
not prove a global target is missing: application
declarations can supply it. Element inference and its subtype/context
dependencies remain necessary before this fallback. Real arrays carry
established element, readonly and member facts; library members cannot be
invented as known empty.

Fresh controls in `tools/type-model-oracle.test.mjs` pass four tests in
`vrun-62a58a21-156f-437c-b9ed-ee45aaf1deb7` with zero skips. They pin intrinsic
special roots, regular false/true boolean constituents, union reduction and
ordering in strict/non-strict null profiles, and anonymous missing-global
array annotation fallback. These controls describe compiler behavior; they do
not close native checker, subtype, global-library or full algebra gates.

The final Project special-root regression was RED in
`vrun-6e77a2c4-2cbc-455c-9f84-6c5f4f1ca9c9` (nine prior tests pass). Source
projection, the shared special factory and the scalar graph view consume
the same six special variants.

Final Project GREEN `vrun-57063c13-a30e-4680-888c-8497e0daf669` passes all ten
presentation tests with zero skips, including successful genuine special
roots at depth 0/32 in both strict-null profiles. The compiler controls' raw
anonymous array fallback remains explicitly distinct from product admission.

The inline union Project regression is RED in
`vrun-da26d6a1-d225-4ac4-8f41-b2c9867478ce` and GREEN in
`vrun-f76a9493-125e-4bc9-a97e-ec32d533871b` (eleven tests, zero skips). It pins
boolean normalization, any/unknown dominance, never removal, exact literal
ordering, nullable profile differences and semantic graph alternatives versus
display clauses. Distinct structured comparison and subtype operations remain
explicit pending failures at their actual source site. The graph alternative
admission uses an identity-size lower bound before child metadata demand,
then exact local byte accounting and rollback as for ordinary properties.

The JSON null-family Project test is RED in
`vrun-988c1a31-87c0-4a22-8b0b-0377a5069536` and GREEN in
`vrun-dacf9a56-fe29-4785-8a3c-e7afd5aeca82` (five JSON tests, zero skips).
`null_initializer` preserves the distinct non-strict widening identity;
`widened_type` owns root-context null/object widening and preserves property
source facts. JSON constructs the original module, widens the root, and then
follows the retained exact export path through `own_string_property`.
Independently widening a cached selected initializer is not this operation.
Sibling-context unions and distinct property write types remain pending.

Established views now own immutable `Arc` payloads and collections, allowing an
evaluator to resolve a child without keeping a borrowed model view alive.
`TypeDemand` separates an established type from a deferred question; semantic
member and demand identities are distinct. Recipes and failure caches belong to
the evaluator, while the model owns established type construction and exact
property lookup. The current eager JSON construction reads only known children
through the model's established-type bridge; it cannot resolve deferred recipes.
The lazy named-member Project GREEN is
`vrun-b0c257f8-96ce-475d-bb26-e9fbfe1b3946`; presentation compatibility GREEN is
`vrun-9880c96e-dabe-4a66-a998-4eedf2886c61`, both with zero skips. These gates do
not prove broader merged/transient/instantiated identities, contextual widening
or complete lazy collection behavior.

The shared intrinsic factories now retain `autoType`, `undefinedWideningType`
and `implicitNeverType` as private identities distinct from genuine any,
undefined and never. Their immutable display views expose the established
intrinsic kind without erasing the identity used by native widening and union
operations. `automatic_variable` is selected by the evaluator's actual
variable-declaration question; it is not a fallback for an invalid initializer.
`undefined_initializer` and `null_initializer` supply the strict/non-strict
widening identities, and `empty_array_element` owns the compiler's separate
empty-literal element rule. JSON does not duplicate those option decisions.

The parent null/void/automatic-variable public regression was RED in
`vrun-8f7208b3-6598-4bd8-b1af-ed4cd5986baa` and GREEN in
`vrun-4a377cee-1ad7-4d92-8276-3a52ccd5c8a7` (four tests, zero skips).
The JSON array recovery regression was RED in
`vrun-b7a60967-d2c6-45e1-9834-979e39d46ece`; its later complete seven-test
GREEN is `vrun-51c73f61-f666-45e5-86ec-a8fbc1c1692b`, with zero skips.
These results prove the admitted scalar questions and missing-global recovery
ordering. They do not prove evolving arrays, general automatic-variable flow,
array instantiation, sibling-context widening or complete intrinsic hooks.

The signature increment uses one complete `structured(MembersView,
Metadata, ObjectKind)` constructor. It receives established property/call/construct/index
collections, including genuinely known empty collections, rather than
inventing those facts in a callable-specific wrapper. The existing object view
is sufficient for nongeneric callable types; presentation derives function
classification from actual call/construct structure and owns the finite native
summary syntax. Canonical generic type-parameter identities and array targets
remain separate upcoming semantic operations, never labels or graph IDs.

The unused `object_with_metadata` pass-through was removed when its production
consumers began supplying complete structured collections. Inferred object
literals retain their distinct literal-widening factory. Signature parameter
read and display types are separate demands: `getTypeOfSymbol` supplies graph
reads, while `serializeTypeForDeclaration` can add undefined for an initialized
required parameter's summary. This does not alter its graph read type.

A mechanical lint cleanup boxes `DemandError.error` while preserving the
original typed error and optional located issue. The conversion back to
`AnalysisError` unwraps that same value; no failure category or display policy
changes. Post-cleanup presentation compatibility passes all 16 tests in
`vrun-0bc48512-ef2d-4924-859d-44ae997a3685`, with zero skips, and the final lint
gate is `vrun-9cc5c355-c9d2-423d-881e-032f24bfd337`. This is a green mechanical
refactor, not a new missing-behavior RED.

The established-type flag question now has one typed owner shared by native
union ordering and evaluator reads. `has_type_flags` asks the compiler's any-bit
mask question; `is_automatic` asks the distinct autoType identity question.
Presentation kind does not substitute for either: boolean retains native union
flags, and automatic any remains distinct from genuine any. A reachable
`object | { value: string }` regression exposed an old primitive-object mask
that belonged to Conditional in TypeScript 7.0.2. The native NonPrimitive flag
is 131072. Both strict profiles are pinned by five compiler tests in
`vrun-6a4f741a-2c9b-41b7-8257-8e31a04ca1df`; the final Project regression was
RED in `vrun-91b2ae4d-014c-4468-9fc0-f1a21484b3be` and all 18 presentation
tests pass in `vrun-0654fd50-7a85-4137-840f-e9f481beb617`, with zero skips.
Future established families extend this same flag owner. These results do not
prove the complete native read admission or flow operations.

Native object-kind facts remain independent from alias and nominal display
metadata. `isEmptyAnonymousObjectType` checks the actual Anonymous kind and all
four established collections; an index-only, callable or constructable shape
is not empty, and an interface is not anonymous merely because it has no
properties. The actual unary-truthiness assertion is RED in
`vrun-1f6359d7-4711-4394-a596-5c4c1edd516e`, then all five final
special-expression tests pass in
`vrun-9dbb35a4-539d-498f-a9ca-0ce0633a2be3`, with zero skips.
Fresh pinned empty-interface controls remain distinct from anonymous `{}`
controls in the six-test model oracle GREEN
`vrun-22812617-5925-40f5-870e-6add37ce3c7e`. This proof covers the
admitted type-facts family; it does not replace arbitrary native object facts
or flow truthiness.

Canonical ordinary parameters now have established request-local identities;
the evaluator's canonical-symbol cache owns reuse across their declarations and
reads. The factory retains exact name, separate lazy constraint/default demands,
declared variance and Const modifier facts. Absence is supplied only after the
complete ordinary declaration list proves no field. It is distinct from a
pending, circular or recovery child. Public parameter tests were RED in
`vrun-97bd635e-b9dc-4458-9fa0-6a23bf85bfa7` and all three pass in
`vrun-136a2036-da03-47fa-ad85-a3a4f91a684b`, with zero skips. The fresh
same-checker GetDefaultFromTypeParameter controls additionally distinguish nil,
a later merged literal default and a consumed unresolved default; an answer
from the default TypeNode alone would not establish that operation.

The admitted parameter summary is its name and does not force children.
Constraint/default graph demand begins only after exact field admission.
Generic targets/references, synthetic this, parameter-chain/circular guards,
mapped/inferred modifiers and generic signature inference remain mandatory
next work. Const retention is a private fact, not a newly invented public field
or a claim of implemented const-parameter inference.
