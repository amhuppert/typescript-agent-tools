# Intersection integration contract

This is the private API contract for the next native algebra stage. It applies
to one accepted immutable Project and one request-local Evaluator. Production
is frozen through the current full Rust checkpoint; these signatures are a
publication for coordinated implementation after that checkpoint releases.

The behavior gates are actual final Project assertion failures:

- `tests/type_intersections.rs`: seven tests failed on the existing pending
  type-node operation in `vrun-26a72be9-0e8f-45ec-a175-c036c525c3a7`.
- `tests/type_intersection_limits.rs`: two tests failed on that operation in
  `vrun-b4440b72-695c-4074-a8b7-528e1568b325`. The flat 100,001-operand syntax
  parsed successfully. This was a behavior RED, not a parser or import failure.
- The fresh compiler controls are seven algebra tests in
  `vrun-5cc69ced-4ae5-4175-9dce-871e3aaa9e1f` and two natural limit tests in
  `vrun-12485f96-91b0-4621-a9b6-9bcf82432e04`, all with zero skips.

The source pin is TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. The complete branch ledger and
design alternatives remain in the algebra implementation plan (archived in Git history).
These gates authorize the intersection increment; they do not prove it or
complete tuple, symbolic operator, relation, member, graph-budget or corpus work.

## Complete evaluator operation

```rust
#[derive(Clone, Copy)]
struct IntersectionFlags(u8);

impl IntersectionFlags {
    const NONE: Self = Self(0);
    const NO_SUPERTYPE_REDUCTION: Self = Self(1 << 0);
    const NO_CONSTRAINT_REDUCTION: Self = Self(1 << 1);
}

fn intersection_type(
    &mut self,
    types: Vec<TypeId>,
    consumer: &Location,
) -> Result<TypeId, DemandError>;

fn intersection_type_ex(
    &mut self,
    types: Vec<TypeId>,
    flags: IntersectionFlags,
    alias: Option<Alias>,
    consumer: &Location,
) -> Result<TypeId, DemandError>;
```

`evaluation/intersections.rs` owns `getIntersectionType`/`getIntersectionTypeEx`
(checker.go:25946–26118). The default operation supplies NONE and no alias.
Property/index consumers call that operation after their actual ordered value
questions. They never assemble a public intersection or repeat normalization.
The Ex operation preserves original input identities separately from its
flattened ordered set, because native propagation and divide-and-conquer use
the original list. Native relation/base-constraint dependencies remain inside
this operation. Pending, invalid, native complexity, product resource and
internal failures remain distinct `DemandError`s.

The evaluator requests the existing native StrictSubtype relation for the
two-member constrained-variable branch before looking up a factory result.
Ordinary False is a relation result; unavailable relation work is a failure.
Native Never/silentNever/Any/Unknown/Missing branches return their actual shared
identities. No error or wildcard identity is represented by a successful Any.

`getTypeFromIntersectionTypeNode` has a separate cache by captured file and
actual Site (full AST kind and token span). A hit skips alias, operands and
factory work. A miss acquires the alias first, operands in native source order,
then the exact emptyTypeLiteralType special flag. It calls the Ex operation
and caches the result or original failure. No synthetic resolving placeholder
TypeId is introduced; declared aliases retain their existing native resolution
protocol. Nested intersections that begin at the same token retain distinct
full Site identities.

## Model identity and owned views

```rust
enum TypeView {
    // Existing variants remain unchanged.
    Intersection(Arc<[TypeId]>),
}

struct MetadataView {
    // Existing fields remain unchanged.
    summary_origin: Option<TypeId>,
}
```

Intersection constituents are real established TypeIds in native insertion
order. They are not sorted, structurally deduplicated or keyed by summaries.
Model stores its actual alias separately from this owned Arc. An actual Union
can have a denormalized Intersection origin for native serialization. Its
graph kind and alternatives stay Union; `summary_origin` selects only the
native summary question after normal alias handling. The origin receives a
real semantic TypeId and no graph ID merely because a summary visits it.

Model owns the native `newIntersectionType` allocation and factory cache. Its
opaque key is ordered normalized constituent identities plus the actual alias
symbol/ordered argument identities when NoConstraintReduction is clear.
Otherwise the key carries the native `*` marker and omits the alias. It does
not serialize every IntersectionFlags bit. NoSupertypeReduction affects prior
normalization. A cache hit does not skip the evaluator's earlier two-member
constraint decision. Newly allocated Intersection propagation uses original
input flags with native Nullable exclusion, not normalized constituents.

The existing union normalizer gains alias/origin inputs inside the same owner;
it does not normalize a plain union and attach metadata afterward. That would
allocate an extra identity and use the wrong native cache partition. The
sorted union factory preserves the native key: constituent identities without
an origin; origin Union/Intersection constituent identity with its respective
marker; and actual alias symbol/argument identity. Named union origin
preservation, unchanged mapping/filter identity and changed-intersection-origin
discarding follow `getUnionTypeWorker`/`filterType`. Boolean remains a semantic
Union with its Boolean flag even when TypeView is Primitive(Boolean).

Literal reduction admits multiple ordinary object identities by native
CompareTypes ordering. Subtype reduction retains its actual relation
questions; it does not become Literal reduction to allow a result. The existing
union failure adapter preserves unavailable relation/recovery causes.

## Native flags and structural consumers

Model extends its sole native TypeFlags owner with Intersection. The native
`maybeTypeOfKind` operation traverses both Union and Intersection constituents
(checker.go:27472), rather than inferring kinds from display or constraints.
The factory exposes ordered intersection constituents to native semantic
consumers independently of TypeView.

The coordinated flag contract is a typed native `ObjectFlags` value and a
fallible Model observation:

```rust
fn object_flags(&self, ty: TypeId) -> Result<ObjectFlags, DemandError>;
fn mark_members_resolved(&mut self, ty: TypeId) -> Result<(), DemandError>;
```

Factory state retains actual flags, including Class/Interface/Reference,
Anonymous/Instantiated, ObjectLiteral/FreshLiteral/ArrayLiteral, PrimitiveUnion,
the three PropagatingFlags, FromTypeNode and the constrained-intersection flag.
Flags with reused bit positions retain their native TypeFlags qualification.
No type name, alias, graph kind or property count establishes a flag. The
existing target state already retains Class versus Interface. Reference and
Anonymous producers must supply their actual native construction facts; an
unretained deferred-reference node or unknown factory flag does not become a
false default. Known complete member factories set MembersResolved at creation;
the sole lazy inventory owner marks it only when its native inventory installs.
Allocating/poisoned targets preserve their existing fallible header protocol.

`createTypeReferenceEx` (25015) looks up target/ordered arguments before its
object flag input is used. A hit does not upgrade FromTypeNode. The reference
key therefore remains target plus arguments, and actual first-allocation facts
are preserved. Deferred references have their separate node/mapper allocation
operation. Anonymous instantiation retains its native original/mapper owner;
an opaque serialization handle is not object or recursion identity.

This contract gives the structured relater actual factory observations, rather
than a second flag map. Its generic, weak/excess, tuple and deferred branches
remain pending until their required factories and dependencies establish the
facts. The initial ordinary interface/type-literal relation tests cannot prove
those broader paths.

## Empty-object and recursion questions

`IsEmptyAnonymousObjectType` (26365) and `isEmptyObjectType` (26374) are distinct
native questions. The former tests Anonymous, then an already resolved empty
inventory or an actual TypeLiteral symbol's empty member table. It never
resolves a nonempty deferred index/signature inventory to prove false. The
existing TypeLiteral construction already acquires that table fact; its
association will retain it. Method associations are not TypeLiteral symbols.
An instantiated association follows its actual target symbol, while a resolved
inventory uses its installed collections.

`isEmptyObjectType` resolves its native structured inventory, tests generic
mapped status, and has separate NonPrimitive/Union/Intersection branches.
Facts use this complete question after the native Anonymous guard. The change
to the first predicate must not accidentally remove those facts dependencies.
Both use one native resolved-empty collection predicate and preserve failures.

The new shared `recursion_identity(ty, consumer)` operation proposed in
the property-symbol implementation plan (archived in Git history)
replaces `constraints.rs::constraint_recursion_identity`. It asks actual
symbol/node/type identity, including literal exclusion, deferred reference
node priority, FromTypeNode, static-class exclusion and cloned parameter
symbol identity. The constraints request cache and raw/guarded/base acquisition
order stay intact; the new fallible identity question remains at the same
native stack point. There is no second constraint-specific identity policy.

## Existing consumer migration

- Base constraints extend the existing Union branch with native Intersection
  all-unchanged identity and changed/nonempty constraint rebuilding through
  `intersection_type`. Missing/circular/safety-stop provenance survives.
- Facts first consume the native base constraint, then evaluate a surviving
  Intersection locally with native OR/AND masks. Returning the same constraint
  TypeId is not a recursive call to itself. Primitive tag handling uses the
  shared maybeTypeOfKind operation.
- `instantiateType` and `getTypeWithThisArgument` map ordered constituents and
  rebuild changed results through this factory. Actual aliases/arguments stay
  part of the native operation. Unsupported mapping is never identity fallback.
- Widening uses the same complete evaluator factory for a changed intersection;
  Model cannot call relations. Existing demanded failures remain intact.
- Shared apparent/structured inventory migrates to `evaluation/structure.rs`
  under the root's mechanical move. Its new exhaustive Intersection branch
  owns actual native composite dependencies, or reports their precise pending
  cause until that full operation is implemented. It cannot return empty.
- Root unary and relation exhaustive adapters receive the new view name before
  production publication. Their pending dependencies remain explicit until
  native family controls authorize their corresponding complete operations.

The presenter adds native Intersection summary precedence between Union and
TypeOperator, and uses its existing admitted-alternative BFS mechanism for
the real Intersection graph. The public graph contains constituent edges, not
invented composite member collections. Depth zero omits the alternatives field;
admitted list prefixes keep native order and exact omitted counts. Union
denormalized origin changes summary serialization only. New lower/at/over
display budgets, encountered failure rollback, and composite Q3/Q4 semantics
remain separate mandatory proof.

## Stage and evidence discipline

After the full checkpoint releases, the stage announces OPEN across the owned
Source, evaluator, Model and presenter regions. The required enum constructors,
producer inputs and exhaustive adapters must all be present before CLOSED and
the registered scoped GREEN runs. The native 99,999/100,000/100,001 cross-product
guard remains unchanged; a product work/depth budget never becomes a native
normal type. The flat 100,001 case must remain shallow and healthy. Broader
checkpoints and service identity checks use a frozen source snapshot.
