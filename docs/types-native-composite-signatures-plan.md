# Native composite signature and inventory owner

This stage extends the shared signature and structured-member operations for
the complete union/intersection inventory contract. The envelope is one
request-local evaluator over an immutable Project and retained source/binding
associations. Compiler-derived final call/index controls precede production
implementation; passing a fixed or ordinary subset does not finish the profile.

The Ousterhout design comparison considered eagerly constructing signatures
whose returns are union/intersection types, and retaining the native signature
recipe. Eager results are simpler descriptors but demand returns while merely
collecting members, fix inference prematurely, lose predicate-specific rules,
and cannot preserve clone/mapper/cycle behavior. The chosen design retains
actual SignatureId associations and extends the existing lazy owners. Ordinary
call, property, index, relation and presentation consumers keep using their
current operations.

`evaluation/structure/composite.rs` owns `resolveUnionTypeMembers`,
`resolveIntersectionTypeMembers`, `findMixins`, `includeMixinType`,
`getUnionIndexInfos`, `appendIndexInfo` and array-member fallback. Its completed
inventory cache is keyed by the actual composite TypeId. The existing complete
MembersState/member-table cache and `setStructuredTypeMembers` installer move
from generics to StructureState as one owner shared with class, reference and
anonymous resolution. Actual nil tables remain distinct from present-empty
tables: only nil skips `getNamedMembers`. Native composite property/member
tables are nil; property synthesis remains the separate existing owner.
Calls, constructs and indexes are acquired in native order, including all
constituent inventories before union-signature matching. Failed acquisition
retains its original failure instead of installing empty lists. The global
Function identity guard uses the existing global identity/recovery owner.

`evaluation/signatures/composite.rs` owns `getUnionSignatures`,
`findMatchingSignatures`, `combineUnionOrIntersectionMemberSignatures`,
parameter/this combination, `appendSignatures`, `cloneSignature` and
`createUnionSignature`. It uses the shared comparison operations in relations;
partial matching selects independent subtype comparisons, while exact matching
selects independent identity comparisons. Existing Relater clients continue
using their own active Ternary session. Parameter names and tuple labels come
from their native source/tuple owners, never from rendered signatures.

SignaturesState retains each composite's ordered actual signatures, union versus
intersection kind and optional mapper, alongside the existing target/mapper
association. The latter takes precedence exactly as native. These are factory
facts, not a second signature representation. A fresh unresolved return marker
enters the existing SignatureReturn resolution frame and chooses target,
composite or actual declaration recipe. Composite returns collect every child
return in order, combine with Subtype union reduction, then apply the mapper.
Composite predicates have their separate Literal-reduction operation and do
not apply that composite mapper; target predicates still instantiate normally.
The return-cycle observer recursively checks composite signatures before the
signature's own frame.

Cloning allocates a new identity, retains declaration/target/mapper/composite,
and resets return and predicate results. It does not copy a previously resolved
return value. `createUnionSignature` then replaces the composite and clears
target/mapper. Default-constructor and mixin overrides reuse this clone owner
before installing their eager return. UnknownSignature has one actual nil
declaration identity and an already installed native error return, represented
by retained recovery provenance; collecting it remains healthy, while demanding
its return reports ErrorType.

Native combination has two separate left-composite policies: its constituent
list flattens whenever the left composite is a union, regardless of the new
kind; its mapper carries or combines only when the old and new kinds match.
Combining this parameters returns the right parameter unchanged when the left
is absent, without applying the right-side parameter mapper. These asymmetries
were confirmed during adversarial review and remain explicit in the owner.

New synthetic parameter symbols have established raw types and native optional/
rest facts. `createSymbolWithType` retains the original parameter's declaration
semantics, so separate call/display optionality wrapping facts must survive
replacement and instantiation. These facts belong to the parameter factory;
the combiner must not infer them from printed optional markers or whether two
already-computed types happen to be equal.

Array-member fallback uses actual instantiated type symbols, their actual
parent, native same-reference comparison with the global Array/ReadonlyArray
symbols, exact native names and the retained anonymous mapper. It maps the
actual global element parameter through each mapper, creates the combined
array, and reads that property's signatures through the shared lookup owner.
Missing/wrong globals and unavailable aliases remain their original scheduled
dependencies. Tuple rests, generic indexed access and full source parameter
labels use the shared owners being implemented in the tuple lane; no tuple-only
index approximation or healthy empty inventory substitutes for those questions.

Proof must distinguish exact versus partial matching; ignored this/return
demands; matching generic defaults versus constraint-only compatibility;
parameter count/name/optionality/rest combination; generic mapper direction;
lazy returns and predicates; union false-return predicate constituents;
construct mixins including all-mixin normalization; index key presence,
readonly policies and ordered values; global Function and array-member fallback;
clone versus instantiation; cycles and consumed failures before and after
independent healthy siblings. Existing signature, inference, call, heritage and
composite-property suites remain compatibility gates.

Pinned source is TypeScript Go 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`: checker10271,13442,16939,
19215,19906,20524,20957–21302 and relater2016–2270.

The implementation checkpoint includes the shared installed-member move,
composite signature recipes and clone/predicate/return integration, array-member
fallback, and lazy signature rendering. Preview and elided signatures do not
query predicates. An admitted failed predicate blocks its signature collection
with the original issue; rolling back graph display allocation does not remove
that issue. Parameter descriptors now retain actual declaration identity and
the independent call/display optionality transforms, including Known raw types
created by combination.

The final fourteen composite tests were compiled RED in
`vrun-54fe9324-6d64-4c4c-827a-4bb0ff2f6376` before production implementation.
The maintained native boundary matrix additionally passed
`vrun-2e6323ad-59af-451a-8ea6-4ebce6fa5a50`: two nullability profiles, eleven
questions covering ordinary/false/boolean/assertion/mismatched predicates,
mixed and all-mixin constructors including readonly array rest, and mutable,
readonly and mixed array-member fallback. `tests/type_composite_boundaries.rs`
asserts those final answers with and without expansion. The compiled checkpoint
`vrun-faaa88da-d3d4-4a3e-aa51-19417a2bc9b6` passed twelve of fourteen composite
groups; defaults required the shared subtype reducer, and explicit this required
intersection relation decomposition. The boundary checkpoint passed eight of
eleven groups, with all three mixin groups stopped at that same relation owner.
The next coherent stage implements native constituent decomposition, effective
constraints, nested/optional property rechecks and tuple structural comparison.
These changes still require the coordinated Rust checkpoint. Tuple spread and
remaining tuple inference remain required continuations.

The overload predicate groups are compatibility evidence, not sufficient
isolation of the composite-predicate owner: a source union can be compared one
constituent at a time before that signature is acquired. A follow-up natural
resolved-signature consumer, native acquisition observation, and counterfactual
final check must establish that the composite predicate itself was consumed.
