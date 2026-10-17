# Native type and symbol ordering

The operating envelope remains one accepted immutable Project and one
request-local Evaluator. Model factories already sort unions, including the
ordinary objects admitted by the intersection increment. Their old
flags/literal/allocation comparator is incomplete: requesting `Zebra | Alpha`
creates Zebra first, whereas TypeScript orders the result `Alpha | Zebra`.
This is an observable regression in currently established families, so native
ordering precedes the next tuple factory increment.

The pinned owner is TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`,
`internal/checker/utilities.go:392` (`CompareTypes`). Its collaborators are
`compareTypeNames`, `compareSymbolsWorker`, `compareNodes`,
`compareTypeLists`, `compareTypeMappers`, and `compareTupleTypes` in that file.
Native symbol names come from binder `declareSymbolEx`, `getDeclarationName`,
and `bindAnonymousDeclaration`, not the NodeBuilder's written-name operation.

## Alternatives and selected boundary

Moving every union factory to Evaluator would permit comparator callbacks into
Source and mapper state. It would also spread a pure established-graph rule
across evaluator demand operations and change many otherwise pure Model
callers. Computing display-based sort keys would be smaller, but loses
canonical symbol names, actual first-declaration positions, mapper identity,
alias arguments, origin precedence, and the native length-first recursion.

The selected design keeps `CompareTypes` with Model's established graph. It
retains actual factory facts and moves mapper identity/data storage from
GenericsState into Model; Evaluator continues to execute substitution. The
comparator acquires no type, member, constraint, parse, or Source dependency.
Factory callers provide the actual symbol/mapper association when native
construction provides it. Unknown retained facts remain a typed failure,
distinct from native nil. A failed optional ordering fact must not fail the
factory eagerly when native comparison never asks it.

This is a single-owner move, not a second comparator state cache. The alias,
target, reference, anonymous, parameter and mapper factories establish their
own facts. The scalar factories prove absence. Reference and instantiated
anonymous factories preserve actual target symbol and mapper associations;
parameter clones preserve their symbol. Synthetic tuple parameters will
explicitly have a nil symbol when their factory is implemented.

## Established graph contract

`Model::compare_types(left, right)` returns native ordering or the original
required fact's failure. `sort_types` consumes this operation and leaves no
successful result if comparison fails. All Model factory and evaluator
failure adapters must preserve a consumed invalid/resource failure; a new
comparison error must not be collapsed to Unsupported merely because older
union failures only described unavailable algorithms.

Comparison follows native branch order:

1. Identical TypeIds, then native sort flags (including enum unit collapse).
2. Actual alias/name symbols, canonical name strings, then alias argument
   lists only when native symbol identity is equal. Named types precede nil.
3. Objects compare their actual symbols; reference kind precedes other kinds.
   Direct reference arguments use length-first recursive type-list comparison.
   Deferred references compare actual nodes then mappers. Nonreferences compare
   ObjectTypeKindMask then mappers. Tuple target details belong here once that
   family exists, including readonly, arity, element flags and decoded labels.
4. Unions compare native origin before alternatives; two present origins
   recurse directly. Intersections compare their actual ordered constituent
   lists, without reordering the intersection itself.
5. Enum/unique-symbol/type-parameter symbols; scalar literal values; actual
   operands, nodes, mapper data and flags for symbolic operators as their
   factories become established. An unestablished family cannot manufacture
   a healthy comparison through the allocation fallback.
6. Native final allocation TypeId fallback only after the preceding facts tie.

All type lists compare length before elements. Mapper nil sorts after a
present mapper; kind order is Unknown, Simple, Array, Merged. Composite, inference, and ArrayToSingle retain Unknown and therefore compare equal by mapper data. Simple compares source
then target; Array compares source lists then target lists; Merged recursively
compares first and second. The existing one-pair Rust Simple representation
maps to native Simple and multiple pairs to Array. The native default argument recovery target retains its private sentinel identity and Any flags when raw mapper comparison observes it. Identity-only comparison does not perform substitution: two identical sentinels compare equal, while a reached substituted value still reports native recovery failure. No healthy Any TypeId stands in for the sentinel.

## Native symbol and node facts

Binding owns canonical native symbol identity/name/first-declaration facts.
`Store::Symbol` currently has no native name field: lexical Site.name and
printer spelling are insufficient. The retention change must record the name
at the actual native bind/merge decision, including default export names,
anonymous internal type/object/function names, reserved signatures, and exact
computed/property names. Existing compatible native declaration groups remain
the first-declaration authority; merges preserve the native target's name.
Unretained contextual/replaced symbols keep their existing precise failure.
InternalSymbolNamePrefix is the invalid UTF-8 byte FE, so native names use
exact bytes or the existing typed reserved-name domain with an exact native
byte comparison. A String with a printable prefix is not the native name.
Default exports retain their exported versus local symbol role; declaring a
default export must not overwrite the local named symbol's name.

The seven existing Collector declaration construction sites can retain their
actual syntax span alongside the selection anchor. Parser::pos uses scanner
full_start and Node.span.start therefore represents native Pos, whereas
BoundDeclaration.anchor is a selection token and cannot be used. The
declare_local operation must receive its actual declaration node, rather
than infer that node from the name token. This retention lets binding own
comparison without Source capture on the sort path. FileId order already
equals accepted program file order after Sources::reorder. No public Location,
summary, alias display, projected origin, or name lookup reconstructs identity.

The shared `compareSymbols` owner compares identity, native nil-last,
declaration-presence (present first), first-node file/Pos, canonical name, then
native symbol-ID fallback. Native ast.GetSymbolId allocates lazily; our binder
index is not that allocation order. Existing retained declaration families
must prove that distinct identities cannot tie both actual first node and
canonical name before using that last branch; synthetic/no-declaration
families need the actual allocation question. Private names embed their
class's native lazy symbol ID and cannot be encoded using our binder index.
Existing typed private keys remain distinct and unavailable native name
facts remain failures when a comparison actually consumes them.
Binding::named_members currently duplicates this rule
using anchors. It must route through the same retained facts/comparison owner;
the heritage getNamedMembers owner additionally partitions own versus
inherited by actual ValueDeclaration range containment before sorting.
Native containment compares the spans without a file-identity guard; this
surprising cross-file behavior requires its own compiler control in the
heritage increment.

Object literal and JSON factories currently create eager Model objects without
actual native symbol metadata. Their source/binder association must be
established through the native object factory path. Merely using an object's
public location or assuming nil would silently change object union ordering.
An explicit unavailable fact is an implementation dependency while this
producer is migrated, not completion of the currently healthy union journey.

## Proof and migration sequence

Fresh native controls are in `tools/type-order-oracle.test.mjs` and
`fixtures/types/native-type-order`. Two compiler tests passed with zero skips:
`vrun-8352aaab-713d-4cab-be6b-11d238e8597b`. They assert native summaries and
actual ordered alternatives across cold, reversed, repeated acquisition, and
opposite program-file orders. Cases cover canonical names, same-target
arguments, intersection count/recursive lists, alias names/arguments and
denormalized origins, actual anonymous/function symbols, and scalar controls.

`tests/type_order.rs` exercises the same outcomes through final Project
queries with expansion and repeated histories. Its genuine RED checkpoint
is `vrun-87b3dcda-a536-4510-9508-1950a60b18b5`: five object/composite tests
failed and the scalar control passed before the comparator change. An earlier
attempt was a compilation-only failure in the queued intersection flag check;
it is not behavioral RED evidence. Further facts introduced by the producer migration
(equal-name symbols, merge declaration order, escaped canonical names,
default-export and eager-object symbols) require fresh compiler controls and
matching final assertions before declaring the whole owner complete.

The separate `tools/type-order-exports-oracle.test.mjs` compiler control passed
with zero skips (`vrun-6f9ea1ae-25a4-45e9-a81f-da930df4e227`). Its default
class is written Alpha, but both the checker-selected declaration symbol and
the declared target's symbol have native name `default`. Local and imported
type-reference unions therefore print `Zebra | Alpha`, while ordinary export
and escaped-name controls print Alpha first. The semantic declared-symbol
association retains this exported name separately from the lexical Site name;
it does not claim that the binder's local ExportValue symbol was renamed.
The same control proves two type parameters sort by canonical name.
`tests/type_order_exports.rs` carries these final outcomes; its RED is pending.

The first staged checkpoint contains binding retention and mapper-storage
migration only. `NativeSymbol` stores name and first-declaration failures
independently so an earlier comparator branch can decide without consuming
the later unavailable fact. `compare_symbols` is the shared binding owner,
and `named_members` uses it. A fallible merge sort preserves comparison
failures instead of inventing an Ordering inside an infallible sort callback.
ObjectLiteral receives an anonymous header association without claiming a
retained member table. Model owns MapperId/data/allocation/composition; evaluator
substitution interfaces remain unchanged. The type comparator and factory
symbol/mapper wiring are still pending at this checkpoint.

After review: retain binding/source symbol facts; move mapper storage; wire
factory associations and exact failures; replace comparison; route named
member sorting; run the registered scoped Project checks once every shared
source lane is closed. Reviewers must challenge lazy failure consumption,
comparison asymmetry, original input ordering in reductions, and alias/origin
precedence. Tuple and symbolic operator comparator branches arrive with their
actual graph families, never through syntax or display guesses.


## Implementation checkpoint (2026-10-11)

CompareTypes now owns established object/parameter/alias/list/origin/mapper
ordering. Actual Type.symbol facts live with Model factories and preserve
through references, clones, anonymous instantiation and widening. Source object
literal recipes carry the actual inner node Loc independently from the outer
parenthesized expression cache Site. JSON still lacks the native object-symbol
node producer and retains that explicit dependency if ordering asks for it.
Native unknownEmptyObjectType is distinct from emptyObjectType, and loose-mode
unknownUnionType is unknown. Established intrinsics are allocated before source
types in initializeTypeCaches order; stringNumberSymbolType is shared by both
existing consumers. All prior union adapters use one failure translation that
preserves consumed dependency failures.

The first exports consumer RED was
`vrun-0f46c91c-009a-4d92-b0e0-f79e98a7b3b1`: the default exported class failed
before ordering because written-name acquisition unnecessarily rejected it.
The fixed display context has InInitialEntityName and no enclosing declaration,
so native getNameOfSymbolAsWritten selects its actual declaration name while
CompareTypes retains canonical "default". Its native control is
`vrun-6f9ea1ae-25a4-45e9-a81f-da930df4e227` (one test, zero skipped).

This checkpoint is not a complete ordering proof. The six-case Project matrix
has five GREEN and one generic-alias RED at
`vrun-ef9aed32-2bbf-4f29-9208-83508a09cdcd`; its generic alias case requires
the shared alias/anonymous instantiation factories staged in the next phase.
Exports reaches its parameter selection role after passing all default-name
cases, `vrun-cd940a69-3d2e-4fcb-85a5-389bc573dcc6`. Those dependencies remain in the required scope, as do
fresh compiler/final recovery and allocation-history controls. Tuple/operator
comparison branches accompany their actual factories in the subsequent full
native T increment.
