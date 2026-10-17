# Shared apparent types

The request-local checker already has one apparent-type operation, consumed by
property, call, iteration and relation operations. Its remaining primitive and
constraint branches currently stop before acquiring real native dependencies.
The next change extends that owner; callers keep their existing interface.

The pinned owner is `getApparentType`, TypeScript 7.0.2 checker.go:21634.
Native flags determine its ordered cases: an instantiable type first acquires
its base constraint (or genuine Unknown), mapped types use their own apparent
factory, changed references substitute the original type as this argument,
intersections use the native apparent cache, and primitive-like flags acquire
String, Number, BigInt, Boolean or Symbol. NonPrimitive and loose Unknown use
the actual emptyObjectType. Index types use the canonical string/number/symbol
union when that factory is established. An arbitrary union is not mapped into
a union of apparent constituents; Boolean is a union with Boolean flags and
must take its native wrapper branch.

The actual globals already have a shared `getGlobalType` owner. String, Number
and Boolean are native eager global dependencies; BigInt and Symbol are lazy
resolvers. Our demand records consume the same canonical target or recovery
without treating a missing wrapper as a successful ordinary empty object.
Global recovery remains distinct from the healthy emptyObjectType used for
NonPrimitive. The existing generic `getTypeWithThisArgument` operation owns
reference substitution and intersection rebuilding; apparent conversion must
call it rather than reproduce parameter-count or this-parameter decisions.

`getApparentTypeOfIntersectionType` has two native caches. When the original
type is the intersection, the intersection identity owns its resolved value;
otherwise CachedTypeKindApparentType is keyed by the original type identity.
The operation installs after construction, without inventing an in-progress
sentinel or circular recovery. Failures keep their original dependency and do
not install successful values. A small request-owned state can retain these
cache facts without putting evaluator demand work into the pure Model.

Native controls `tools/type-apparent-oracle.test.mjs` pass all three profiles
with zero skips (`vrun-55b65a28-5433-49e0-a8cc-7b01145a7090`). Ten final
assignment reads per profile cover five primitive variables, four literals and
a constrained generic reference. Real wrapper/base properties retain their
declared union while unrelated missing property annotations remain unvisited.
`tests/type_apparent.rs` observes the same final Project results, expanded and
unexpanded. Its genuine RED check precedes implementation. Intersection and
operator integration require their own final controls as their native graph
factories and inventories become established; this increment does not close
the full T profile.

Recovery tracing now has a pinned native witness beyond the final summary:
`vrun-76e98aa7-f5e3-4c11-8b67-f40ae786f939` passes all seven maintained
controls with zero skips. A guarded native overlay observes the return of
`getTypeOfDestructuredProperty` using native `isErrorType`, without another
checker query or identity allocation. Nine primitive cases across four global
configurations establish false for healthy wrappers and true for absent,
wrong-arity and wrong-kind wrappers, although every final selected read still
prints its declared union. This justifies preserving the consumed recovery
failure in the product. The staged apparent implementation awaits its Rust
GREEN after the recorded initial RED.

The intersection apparent cache remains owned by the same operation and keyed
by the original actual TypeId. Native's direct-intersection and cached generic
apparent cases occupy disjoint source domains, so one private map stores their
successful results without adding a cross-owner protocol. It never installs a
resolving sentinel or turns a failed conversion into a cached success.
