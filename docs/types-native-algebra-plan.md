# Native type algebra and remaining structured families

This is the next implementation plan within the full T/Q3/Q4 design. Its
operating envelope is one immutable accepted TS/TSX Project and one
request-local Evaluator, with captured syntax, canonical binder identities,
native demand caches, and the existing finite presenter. It does not declare
the existing scalar, callable, member, flow, or generic increments a release
subset. Intersections, tuples, and symbolic operators are required semantic
families, including their actual normalization and consumed recovery.

The pinned implementation is TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. Line references below identify that
source, rather than proposing a new algebra from public display strings.

## Design twice: syntax graph versus complete native operations

One option is to turn each IntersectionType, TupleType, or operator syntax node
directly into an immutable graph descriptor. It would make the first graph
easy to print, but asks every consumer to rediscover whether `string & number`
is Never, whether a tuple rest collapses to Array, whether a conditional is
resolved, and whether an indexed access consumed an invalid member. Flags,
constraints, mapper substitution, relations, and presentation would each need
their own corrective reductions. The apparent simplicity creates conflicting
successful answers and expensive changes across consumers.

The chosen option extends the existing deep Evaluator operations. Source
projects complete owned recipes through CapturedSyntax and canonical binding;
the Evaluator acquires native dependencies and runs normalization; Model owns
established identities, exact flags, native factory keys and immutable views.
The presenter reads the resulting semantic family and admits optional children
through TypeDemand before checking them. A deferred native operator is a real
established type, not a pending source recipe or an Unsupported leaf. A resolved
operator returns the actual reduced TypeId, so downstream consumers need no
syntax-specific correction. This preserves the existing separation between
required graph-kind inventory and optional displayed edges.

The public operations remain `Project::type_of` and the existing member/reference
journeys. Private helpers hide compiler algorithms; no caller receives a raw
AST, an environment of parameter spellings, or instructions to reproduce a
factory's lookup/cache sequence. All new views are owned/Arc values and all
deferred slots remain typed demands. Existing failure and resource distinctions
survive through those operations.

## Intersection ownership and first final journeys

`evaluation/intersections.rs` will own complete `getIntersectionTypeEx`
(checker.go:25950), including its evaluator-dependent questions. Model will
own the ordered identity set and `newIntersectionType`/intersection cache;
it will not call relations or inspect captured syntax. Native
`addTypesToIntersection` removes literal freshness, flattens intersections,
deduplicates exact TypeIds, and preserves insertion order. Empty anonymous
objects require the existing complete native shape question, not an empty
property slice or a display name.

That whole predicate is `IsEmptyAnonymousObjectType` (26375): a resolved
Anonymous uses its actual complete collections; an unresolved TypeLiteral
asks its canonical symbol member table instead. It does not resolve a
nonempty index/signature inventory merely to establish false. Native controls
show an unresolved index/return child can remain skipped before intersection
normalization returns Any or Never. The source-facing
`getTypeFromIntersectionTypeNode` (24126) also preserves a special two-operand
form when one operand is the exact shared emptyTypeLiteralType and the other
is String, Number, BigInt or a pattern template. It sets
NoSupertypeReduction for that identity-specific case; an alias to an empty
object is not that intrinsic identity.

The operation's complete branch ledger includes Never/silentNever; strict and
loose nullable behavior; disjoint primitive domains and distinct units;
Any/wildcard/error provenance; redundant supertypes; Missing versus Undefined;
zero/one constituent collapse; the two-constituent constrained-variable
reduction; primitive-union intersection; common nullable union extraction;
divide-and-conquer; and the union cross product with its native complexity
guard. Template/string-mapping reduction and constrained intersections use
the shared relation and base-constraint owners. An unavailable dependency is
located pending at that demand while its owner is being implemented; it is
never False or a healthy unreduced intersection. Those branches remain
mandatory even when an earlier final test only reaches simpler normalization.

`getIntersectionKey` (17445) writes ordered normalized constituent TypeIds and,
when NoConstraintReduction is clear, the actual alias identity/arguments.
Otherwise it writes `*` and omits the alias. NoSupertypeReduction affects prior
normalization; it is not an independent key dimension. No structural
deduplication, sorted union key, or printed alias may replace this native key.
A reduced scalar result does not gain a fabricated alias. The current alias
written-name/provenance split is a separate coordinated owner change; this
factory will consume actual alias identity in addition to those display facts.

The two-constituent base-constraint/StrictSubtype question occurs before cache
lookup even on a hit. Union distribution runs only on a cache miss. Its
divide-and-conquer branch checks both normalized count at least three and
original input count greater than two. New intersection propagating flags
come from the original input types, separately from flattened constituents.

The source-facing type-node operation has its own native TypeNodeLinks cache.
A hit there skips alias/child/factory acquisition altogether; it must not be
confused with an intersection factory cache hit, which still performs the
two-member constraint decision. The captured IntersectionType attachment
supplies exact file/kind/span identity for this request-local node question.
Failure caching preserves the original cause, and recursive aliases continue
through their existing declared-type resolution owner rather than a fake
forwarding TypeId.

The same stage extends the existing Literal union factory to admit multiple
ordinary object identities without inventing structural comparisons. Native
CompareTypes orders those identities, while Subtype reduction has its distinct
relation dependencies. Cross-product results retain the native denormalized
intersection origin separately from the actual Union alternatives and use the
native origin/alias cache partition. Filtering a union with intersection origin
uses the native no-origin result when constituents change; unchanged mapping
retains the original identity. Widening, base constraints and this-argument
mapping rebuild intersections only through this one complete factory.

The initial fresh compiler matrix will pin inline intersections and named
aliases, nested/duplicate ordering, scalar collapse, both null modes, object
members, constrained parameters, common nullable unions, and distributive
cross products. Final `tests/type_intersections.rs` will assert actual kinds,
ordered alternative identities, summaries, child/member dependencies, alias
provenance, and whole invalid causes through `Project::type_of`. The first
test is run against the current explicit missing type-node operation before
Source, evaluator, Model, or presenter behavior is added. Natural cross-product
and relation-overflow controls must prove just-under/at/over native thresholds;
a product budget failure never becomes native Never or a valid intersection.

## Tuple target, reference and normalization ownership

Tuples use the existing generic-reference lifecycle, rather than an independent
tuple identity table keyed by printed element types. Native
`createTupleTypeEx`, `getTupleTargetType`, and `createTupleTargetType`
(checker.go:24648–24741) distinguish target identity from ordered actual
arguments. The target key includes complete required/optional/rest/variadic
element flags, readonly, and native label declaration identity. Synthesized
parameters and synthetic this retain real TypeIds; they are not spelling-based
substitutions. Tuple target/member construction must retain min/fixed lengths,
native length literals, optional properties, and actual numeric indexes.

`createNormalizedTypeReference` (23204) owns spread/variadic expansion,
optional/rest normalization, and collapse. A single rest can select actual
Array/ReadonlyArray, and a missing global target is consumed recovery even
when the final native root is the non-error empty object. Complete target
lookup/classification remains with the existing global/generic owners. A
noLib flag or the text `Array` never establishes a target. A fixed tuple can
exist independently of a missing array target until its native members/base
question consumes that dependency; fresh phase controls must pin that exact
schedule before a healthy final answer is claimed.

Model's immutable Tuple view supplies ordered element TypeDemands with exact
labels and flags, readonly, actual type arguments, and MembersOfType for the
true inventory. The presenter follows type arguments then tuple elements then
normal members within the shared BFS/minimum-depth and byte/node budgets.
Label spelling is display metadata; label/source identity is retained by the
native target operation. Invalid unpaired UTF-16 identity is not replaced.

Fresh compiler and final `tests/type_tuples.rs` controls will cover empty,
required, optional, labelled, readonly, rest-only collapse, fixed+rest,
variadic generic instantiation, union variadics, and consumed missing-global
recovery. Tuple element identity, readonly, exact order, numeric property/index
values and length are compared as named native questions. Summary-only,
depth-zero, admitted element expansion, hidden invalid children, and lower/at/
over display budgets are separate final journeys, with no helper-only proof.

## Deferred operators are established semantic families

The public closed operator vocabulary is already defined in `src/types.rs`:
keyof, indexed_access, conditional, mapped, template_literal, string_mapping,
and substitution. Each factory has a separate complete native owner, while
immutable Model views share only the graph mechanism: ordered operands and
bound parameter descriptors. Operators never share a universal cache merely
because they share a public record shape.

| Family | Complete native operation and important distinction |
| --- | --- |
| keyof | `getIndexTypeEx` (26555) first reduces, handles NoInfer/defer/union/intersection/mapped branches, then actual property/index keys. `getIndexTypeForGenericType` caches the native Index type/flags. A generic `keyof T` is a real operator; a concrete key union is the actual reduced union. |
| Indexed access | `getIndexedAccessTypeOrUndefined` (26801) distinguishes generic deferred identity, reading/writing flags, index/property acquisition, optional slots and invalid lookup. Object/index operands do not imply their member result was checked. |
| Conditional | `getConditionalType` (24208) and `getConditionalTypeInstantiation` (22391) own check/extends/inference, distributivity, actual mapper, branch demands, tail count and their native keys. Deferred branch recipes remain lazy; ordinary False/Maybe are distinct from overflow and recovery. |
| Mapped | `getTypeFromMappedTypeNode` (24163), `resolveMappedTypeMembers` (20799), and anonymous instantiation own canonical bound parameter, modifiers/name remapping, key iteration, actual member identities and lazy values. A syntax dictionary or reused source member ID is not a mapped member. |
| Template literal | `getTemplateLiteralType` (28902) owns exact text/type segments, union cross products, literal concatenation and generic/pattern reduction. Text boundaries stay exact; native/product resource exhaustion remains a failure. |
| String mapping | `getStringMappingType` owns actual intrinsic symbol, concrete transformation and deferred target identity. Arbitrary names resembling Uppercase do not select the intrinsic. |
| Substitution | `getSubstitutionType` and NoInfer factories own base/constraint identity and simplification. Normal versus Inferential narrowable-reference behavior consumes the established flag; it does not infer a substitution from a displayed operator. |

For each family, fresh native controls first pin resolved and deferred results
and required/lazy dependency order. A real final Project assertion RED then
authorizes factory and view changes. The initial operator journey is generic
`keyof T` plus concrete object-key reduction; indexed access then consumes the
same canonical member/index operations. This order supplies actual prerequisites
for conditional and mapped dependencies without hiding them behind blanket
Unsupported. All operator families remain required after that first journey.

## Integration and checkpoint discipline

Every new semantic family extends the sole Model flag owner and the existing
facts, base-constraint, narrowable-reference, instantiateType, relation and
summary-precedence owners where native asks that question. Exhaustive adapters
may stage typed pending dependencies before a factory is reachable; they cannot
convert a new family to primitive/object defaults. Bound source recipes, actual
TypeIds, source member roots, signature IDs, mapper IDs, and output IDs remain
distinct. Native guard/cache/reset order is ported before memoized answers can
escape; product budget ledgers do not change native constants.

Each coordinated source stage announces OPEN/CLOSED and includes every enum
constructor/match, required producer, and parent dispatch before scoped Rust
execution. Registered final tests use one explicit file for RED/GREEN. Wider
compatibility checkpoints follow stable stages; service tests require a frozen
source/build identity. Mechanical DTO cleanup is separate from these behavioral
increments and earns no fabricated RED.

The complete graph remains subject to 2,000 nodes, 262,144 compact type-field
UTF-8 bytes, 512 summary bytes, BFS minimum distance, stable admitted list
prefixes, and preserved encountered issues after rollback. Each added graph
kind needs final under/at/over tests and actual unsupported/invalid dependencies;
the ordinary property proof cannot be inherited without evidence. Full algebra,
whole Q3/Q4 member relationships, corpus agreement, representation comparison,
and service/resource limits remain mandatory delivery gates.
