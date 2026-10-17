# Union and intersection property ownership

The current local immutable Project and request-owned Evaluator remain the
operating envelope. This implementation uses the Ousterhout skill: a complete
property-symbol factory hides synthesis, caches, flags, declarations and deferred
read/write normalization from flow, relation and presentation callers.

The observed access failure is not evidence that a nullable union's property is
unrelated. `getDiscriminantPropertyAccess` asks the actual composite property,
including properties hidden from normal lookup by ReadPartial. The factory must
therefore precede that flow consumer. No scalar or nullable-only bypass is used.

`PropertySymbols` owns created symbol identity and links alongside its existing
canonical and instantiated symbols. Composite links retain containingType,
ordered declarations, actual ValueDeclaration and its original symbol's raw
Parent, nameType, CheckFlags, resolved types, and deferred constituents. Binding
and Source retain these facts; the factory does not infer a parent from a display
origin or choose an arbitrary constituent's canonical parent. Its no-propSet
merged-instantiation clone is a distinct native factory branch, with the original
readonly flags and copied mapper/write cache, not a synthetic-property stand-in.

`createUnionOrIntersectionProperty` walks actual constituents in native order,
asks apparent type and property lookup with the operation's augmentation policy,
and handles independent accessor, optional, readonly and accessibility flags.
Identical target instantiations use the existing compareProperties owner with
exact type identity. Missing union properties separately consume applicable
indexes, tuple rest values, non-spread object-literal undefined, or ReadPartial.
Existing missing tuple/late-key factories remain explicit dependencies where
reached. Private/protected combinations use common actual declarations.

Only actual native comparisons set HasNonUniformType, HasLiteralType and
HasNeverType. More than two property types defer union/intersection normalization
until the corresponding read or write question; writes do not invent index-value
constituents. The two augmentation caches propagate only complete entries from
unaugmented to augmented lookup. Normal lookup filters ReadPartial; discriminant
inspection intentionally does not. Inventory resolution preserves constituent
order and stops after a union constituent without indexes.

All property facade operations must support created symbols before they enter a
consumer: flags, names, original declarations, readonly, declaring class,
forEachProperty, target identity, raw and non-missing reads, writes, fresh
instantiation, ordering and serialization. `forEachProperty` redoes the native
constituent lookup lazily and short-circuits callback success. It does not merely
iterate a precomputed propSet, which would change demand order and missing-entry
behavior. Shared compareProperties gates remain one operation for both relation
and exact-identity comparators.

The flow owner then acquires candidate access, exact property name, subset choice
and isDiscriminantProperty in native order. Optionality/truthiness reductions use
the same native discriminant filter and comparable relation. Their intermediate
types can rejoin; completed read admission still rejects a changed final type as
flow_required. A missing demanded semantic operation remains a failure.

Compiler controls cover common/partial union properties, intersections, native
read/write/readonly/optional distinctions, index contributions, deferred three-way
normalization, consumed unhealthy members and discriminant joins. Final Project
queries cover both summary and expansion, with original failure provenance. The
existing access and optional-join failures are genuine end-to-end RED evidence.
No factory, control or full deliverable is claimed complete by this plan.

Implementation checkpoint: the property-symbol facade also projects binder
prototypes; graph metadata no longer comes from a separate class-static literal.
Instantiated and merged-clone symbols reset Partial check flags, required method
serialization preserves the native empty-signature result, and auto-accessor
serialization uses the two actual synthetic-signature factories. Peer review
covered these changes and the independent Prototype/Accessor dispatch gates.

The shared `getReducedType` operation now consumes this inventory. Intersections
publish their computed marker before property acquisition; ordered discriminant
and private-conflict questions may reduce them to Never. Unions containing
intersections map semantic constituents, preserve unchanged identity, and cache
the rebuilt union as its own reduced result. Product failures replace provisional
markers rather than turning a later query into success. The maintained native
control passed 63 queries across three option profiles in
`vrun-4ba27960-c3a0-4a32-9e97-4c0270d14f37` (three tests, zero skips). Final Rust
controls are staged, not yet validated for this checkpoint.

Review is still investigating the merged-instantiation clone with an unresolved
ordinary Method type. Native `createSymbolWithType` intentionally leaves that
read cache empty even when comparison warmed the original non-transient symbol.
The new anonymous type must carry the clone's actual symbol if this branch is
reached. Delegating that read to the original symbol is not an identity proof.
