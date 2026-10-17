# Shared indexed access owner

This implements the existing T operator contract for a request-local native
checker. Type nodes, value element access, tuple normalization, constraints,
instantiation and relations ask the same native operation at different sites.
The immutable Project supplies actual syntax and binding facts; failures and
recovered compiler types retain their original provenance.

A tuple-only `T[number]` helper would duplicate generic classification, property
lookup, missing values and later substitution. A separate eager type-node
implementation would also erase the deliberate difference between expression
access and type access on generic objects. The chosen owner is
`evaluation/indexed_access.rs`, called by all those consumers. It owns
`getIndexedAccessTypeOrUndefined`, its required-result wrapper,
`shouldDeferIndexedAccessType`, `getPropertyTypeForIndexType`, simplification,
and the indexed-specific constraint question. Existing property-symbol,
relation, tuple and base-constraint owners continue to own their own decisions.

The input retains an explicit native context: no access node, an actual indexed
access type node, or the existing element-access recipe. Element access still
checks receiver/key and applies outer reference flow and final index validation
at its native sites. Deferral classifies actual node kind (nil, IndexedAccessType,
or another node), independently of ExpressionPosition/Writing flags. Native
PropertyName and PrivateIdentifier contexts retain their name-classification
behavior; a non-type node is not automatically an element-flow context. The shared
operation performs its inner property reference
flow only for element-expression contexts. A type node adds Undefined for a
Missing property; no-node accesses do neither. There is no invented source node
or expression context for tuple normalization.

A typed AccessFlags record preserves the native incoming flags, including
ExpressionPosition, Writing, NoIndexSignatures, IncludeUndefined, AllowMissing
and Contextual. Native cache identity uses the complete flags after the
unchecked-expression adjustment; the established operator retains only
Persistent flags. These are intentionally different questions. Alias identity
and arguments belong to the cache key and established metadata, not printed
names. Known concrete reads share the current property/index operations.

Model stores a genuine IndexedAccess type with actual object/index TypeIds,
persistent flags and alias. Its factory has IndexedAccess TypeFlags, no object
flags and no symbol. CompareTypes first uses its common alias/name comparison,
then compares object and index; persistent flags are not a comparator field.
Equal operands/names reach the native TypeId fallback through the existing
ordering owner. Instantiation first uses an explicit alias or instantiates the
existing alias, then maps object and index, then calls the shared factory.
It cannot reuse the union branch's later alias-acquisition schedule. Graph
presentation emits the existing Operator/IndexedAccess shape and ordered
operand edges under the same BFS and work/display limits.

The optional operation returns actual native absence. The required wrapper
turns absence into native ErrorType when a node was supplied and Unknown for
no-node compiler queries. It never converts consumed invalid dependencies or
resource failures into absence. A union key visits all children for diagnostic
contexts but stops on the first absent member without a node, matching native
order; reads combine with Literal union reduction, writes with intersection.
Contextual absent properties use native Any only at that explicit compiler
branch, never as a general recovery policy.

`getSimplifiedIndexedAccessType` retains independent read/write cache entries
and a reentrant original-type marker. It simplifies object/index in order,
distributes index unions before object unions/intersections, then asks actual
generic tuple or mapped-type owners. Changed results remove the original
operator before installation. A product failure replaces the provisional marker
with the original failure, so later access cannot see a healthy cached operator.
Distribution deliberately calls the no-flags,
no-node, no-alias wrapper. Mapped simplification uses the original operator's
index type after the simplified object's branch test. Constraints first consult
the noncircular base
owner, then simplified index and object constraints in native order. Shared
getConstraintOfType dispatch distinguishes guarded raw parameter constraints,
indexed and conditional constraints, and the ordinary base fallback; it does
not replace all those questions with base_constraint. The base
owner must call simplification at its existing scheduled point; new operator
facts cannot inherit an unchanged fallback.

Full dependency closure includes generic tuple fixed/variable slices, keyof
for final expression-index validation, generic reducible intersections and
mapped-index substitutions. Generic reducible intersections require the native
startup uniqueLiteralType Never singleton, uniqueLiteralMapper and cached
uniqueLiteralFilledInstantiation. The composite property owner excludes that
exact singleton from HasNeverType; replacing this with ordinary Never changes
reduction. These remain explicit staged obligations while
being implemented, not successful approximations or a reduced deliverable.
No blanket TypeParameter-to-Any result is permitted. Model edit windows are
coordinated with tuple/reference work so each exhaustive consumer is addressed.

Proof starts with pinned native and final Project controls for concrete names,
optional Missing, string/number/symbol indexes, union read/write order and
absence, Any/Never/Unknown, generic object versus generic key deferral, alias
and mapper identity, nested simplification, constraints, unused versus consumed
invalid children, and strict/loose/exact/unchecked options. Tuple and mapped
families add their actual factory controls. Existing access/flow/property tests
are compatibility gates for moving the entire shared decision.

Pinned TypeScript Go 7.0.2 source:
checker8140–8235,17133–17162,17493,22855–22865,24780–24832,
25108–25114,26789–27075,27236–27265,27731–27849;
utilities CompareTypes IndexedAccess branch; types AccessFlags.

Implementation review refinements: the tuple-miss branch acquires the numeric
index signature even when the access node is nil; string-index fallback asks
its compatibility relation before consuming the returned value. The index-only
classifier now calls the shared property inventory, index inventory and exact
string-index lookup in native short-circuit order. A structurally similar
single-inventory shortcut would omit those operations. The reviewers checked
the tuple/unique-literal deferral and distribution order against the pinned
native source. Final generic expression access requires actual `keyof`;
`evaluation/keyof.rs` owns that related but distinct question, with fresh origin
identities separate from cached generic Index types. Indexed operators keep
ordered object/index graph edges and field-level depth/byte omissions.

Deferred references now retain the exact shared file/parser-node identity for
recursion tracking, separately from native declaration-order facts. Indexed
recursion identity follows the leftmost object type. Tuple identity follows the
target after the native FromTypeNode exclusion. These extend the existing
recursion-identity operation rather than adding caller-specific approximations.
