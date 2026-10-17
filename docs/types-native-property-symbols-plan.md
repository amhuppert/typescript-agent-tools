# Native property symbols and recursion identities

This Ousterhout design assumes one immutable accepted Project and a
request-local Evaluator. The new token represents an actual compiler symbol;
it is separate from graph rows, source origins and serialization questions.
The structured relation consumer has a genuine six-test assertion RED,
`vrun-4a7c8533-7e4a-483b-966d-9462b537552f`. This document proposes the shared
member-owner increment; production remains unchanged while the full Rust
checkpoint is frozen.

The source pin is TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. Required owners are
`instantiateSymbol` (checker.go:20658), `getNonMissingTypeOfSymbol` (16437),
`getDeclarationModifierFlagsFromSymbolEx` (utilities.go:694), `isReadonlySymbol`
(checker.go:13780), and `getRecursionIdentity` (relater.go:839). They ask
different questions. Reuse the canonical member inventory and existing
source/modifier/read owners without using one question's cache as another's
semantic identity.

Two designs were considered. Deriving property identity from the existing
PropertySerializationId avoids a field, but that ID caches a printer question
by original member and mapper; it cannot represent two fresh native transient
symbols. An independent source-name lookup would duplicate the binder table
and add another ordering/merge decision. The chosen design places a typed
PropertySymbolId arena in the existing MemberValuesState. Model Property holds
only its optional opaque handle. A producer with no established actual symbol
keeps None and the consuming semantic operation reports a located pending
dependency. None does not mean a synthetic property equivalent to another row.
This supports current canonical members without adding a searchable inventory.

The complete private interface is:

```rust
struct PropertySymbolId(usize);

fn register_property_symbol(symbol: BoundSymbol) -> PropertySymbolId;
fn instantiate_property_symbol(
    symbol: PropertySymbolId,
    mapper: MapperId,
    consumer: &Location,
) -> Result<PropertySymbolId, DemandError>;

fn property_symbol_flags(
    symbol: PropertySymbolId,
) -> Result<PropertySymbolFlags, DemandError>;
fn property_declaration_modifiers(
    symbol: PropertySymbolId,
    writing: bool,
    consumer: &Location,
) -> Result<u32, DemandError>;
fn property_value_declaration(
    symbol: PropertySymbolId,
    consumer: &Location,
) -> Result<Option<DeclarationIdentity>, DemandError>;
fn property_is_readonly(
    symbol: PropertySymbolId,
    consumer: &Location,
) -> Result<bool, DemandError>;
fn non_missing_property_type(
    symbol: PropertySymbolId,
    consumer: &Location,
) -> Result<TypeId, DemandError>;

fn recursion_identity(
    ty: TypeId,
    consumer: &Location,
) -> Result<RecursionIdentity, DemandError>;
```

PropertySymbolFlags carries established native questions: Optional,
ClassMember, Prototype, Partial, Static and private-name status. Declaration
modifiers and readonly are separate operations; they are not copied from
presentation accessibility. Canonical member flags come from the actual
retained binder symbol. Transient flags follow the native factory mask.
Synthetic partial/union, object-literal and JSON producers must establish their
own factory flags before entering these operations. They cannot inherit
canonical false defaults simply because their current display looks ordinary.

The original arena association is Own(BoundSymbol), installed when the
canonical inventory attaches that actual symbol. Original tokens are reused
by canonical BoundSymbol. The instantiated association retains target token
and mapper provenance and has a fresh actual token allocated at the native
instantiateSymbol point. The pair is not an interner key: two newly allocated
symbols can have the same target and mapper. Repeated member questions reuse
the existing members(TypeId) installed inventory and consequently its actual
tokens. PropertySerializationId remains a separate original/mapped printer
question; it cannot define token equality.

instantiateSymbol must first implement both native identity-preserving paths.
MapsThisOnly asks the complete isThisless declaration/type/function rules.
An already resolved symbol type asks couldContainTypeVariables without forcing
an unresolved read. A setter additionally requires an already resolved write
type for the same fast path. Only then does an instantiated input flatten to
its real original target and compose mappers, followed by fresh allocation.
Neither eager resolution to prove equality nor unconditional fresh allocation
preserves this decision. Missing isThisless/mapper/flag facts remain located
pending dependencies until that owner is complete.

Original raw reads continue through value_type's canonical BoundSymbol cache.
Transient raw reads implement getTypeOfInstantiatedSymbol: ask the target raw
read, instantiate it, then install that transient symbol's resolvedType. There
is no new Type-resolution push in the native transient operation. An artificial
wrapper Resolving refusal must not prevent the original symbol's native cycle
protocol. Transient writes likewise retain their own actual property cache and
call the existing instantiated-write decision. non_missing_property_type reads
the actual symbol and routes Optional removal through Model.remove_missing_type;
it is not the graph display question or accessor write question. The relater
asks target non-missing type first, so a target Any/Unknown can skip the source
read entirely.

The existing Source.canonical_property already contains the ordinary
getDeclarationModifierFlagsFromSymbolEx and isReadonlySymbol rules. Move the
complete metadata decision into an operation used by both that producer and
the token's modifier/readonly demand. The actual write selector prefers a
setter; the read selector prefers a getter before ValueDeclaration. Class
parents retain accessibility flags, other parents strip them. Synthetic and
Prototype branches belong to the same owner when their actual metadata is
admitted. Do not make callers choose the declaration or repeat those checks.

DeclarationIdentity is the actual captured AST node identity, FileId plus
NodeId behind an opaque type. It is acquired through the existing exact
attachment index from the retained declaration kind/anchor. A DeclId alone is
insufficient because two retained roles can point to the same native syntax
node; a public Location or equal span is not identity. The comparison operation
asks this only when the native private/protected branch needs it. Unsupported
late symbols or unretained declaration roles remain failures.

RecursionIdentity is a separate typed identity with Symbol, actual captured
Node, or Type variants. The whole native rule owns object/array-literal
exclusion, deferred-reference node priority, eligible object-symbol identity,
tuple target identity, canonical TypeParameter symbol identity, leftmost
indexed-access object and conditional root node. Current Interface/Reference
target associations, Anonymous original/instantiated associations and the
parameter clone constraint-origin owner supply the admitted symbol facts.
FromTypeNode and deferred-node facts must be established at their native
factories; equal printer names or source origins cannot fill those branches.
Unsupported families stay pending until their complete factories exist.

Final proof must pin same actual canonical property skipping an unresolved
read, target-first Unknown/Any skipping source recovery, ordered mismatch and
optional rejection, exact optional Missing removal, readonly relation
direction, and private actual node identity. Fresh controls must also pin the
instantiateSymbol MapsThisOnly/resolved-read/setter-write paths versus fresh
transient allocation before generic properties gain token equality. Ordinary
structured six tests exercise the initial canonical interface consumer;
instantiated/private/synthetic/recursion branches keep their distinct final
controls. Existing graph and summary behavior remains a compatibility gate.
Arena/token bytes and merge/instantiation cost remain measured representation
obligations.

## Staged implementation and bounded peer review

The canonical-member increment is now staged. `MemberValuesState` owns a
separate PropertySymbols arena, keyed by BoundSymbol only for original symbols.
Actual transient entries are fresh allocations after `instantiateSymbol`'s
MapsThisOnly and already-resolved-type shortcuts. The latter observes the
setter's write cache only after the read shortcut succeeds. Installed native
recovery remains a consumed failure; a product question aborted before native
installation supplies no resolved-type shortcut. Own reads continue through
`value_type`; transient reads and writes cache on the actual token and add no
Type-resolution frame. Deferred wrappers delegate recursive reentry to that
operation. NonMissing remains a separate raw-read/Optional removal question.

Source now owns the complete ordinary member modifier selector shared by
presentation and relations: write setter, read getter, then ValueDeclaration,
with the native class-parent accessibility rule. Readonly is a separate shared
operation. Private comparison acquires FileId plus actual captured NodeId;
retained declaration roles and equal spans do not establish identity. Static
and private-name flags come from the binder's actual value declaration facts.
Native early members establish ClassMember and absence of synthetic Partial
and Prototype; the arena does not accept synthetic producers through these
false defaults.

The printer question carries its actual property token in addition to original
printer/mapper provenance. Its mapped-question cache includes that token, so
serializing one fresh transient cannot warm a different transient's read cache.
Property-symbol equality never reads a printer ID. The old printer-keyed
instantiated-write demand was removed. `instantiate_members` now follows native
`resolveObjectTypeMembers` order: symbols, call signatures, construct signatures,
then index infos.

`getRecursionIdentity` has one evaluator owner shared by constraints and
relations. Its admitted factory observations preserve literal exclusion,
FromTypeNode, canonical reference/anonymous symbols, and cloned/this parameter
symbols. Constraint identity failures still unwind the existing native
resolution frame. `couldContainTypeVariablesWorker` uses Model's actual
computed/result bits and Source's top-level alias question. An unavailable
object-literal symbol association remains pending. The shared
`IsEmptyAnonymousObjectType` checks already resolved collections or actual
TypeLiteral symbol table emptiness without starting lazy inventory resolution.
Function, method and TypeLiteral factories retain their canonical symbols
separately from written-name metadata.

The structured-relations peer compared the ordinary modifier, thisless,
instantiation, readonly, NonMissing and recursion branches with the pinned
source and found no current blocking mismatch. This is a bounded read, not
execution proof. The six final Project tests in `tests/type_structured_relations.rs`
retain their genuine pre-change RED; the coordinated post-change Rust checkpoint
has not yet run at this document update. Existing member-value, display,
generic, anonymous and base-constraint tests are compatibility gates. Direct
final-consumer proof for every transient fastpath/fresh-allocation distinction,
private node-role identity, broader synthetic/union property families,
deferred/tuple/indexed/conditional recursion factories, representation costs
and the complete T/Q3/Q4 profile remain mandatory work.

The protected-comparison increment exposes `property_declaring_class`,
callback-based `for_each_property`, and nullable-target `has_base_type`.
Binding records actual Symbol.Parent at the bind point and preserves that
parent's flags before checker merges; `getDeclaringClass` tests those raw flags
before `getParentOfSymbol` canonicalizes the parent. Checker export collisions
replace the parent only on actually merged symbols, while member merges retain
the original target parent, including nil. A merged parent's flags remain
mutable as in native `mergeSymbol`. This retention was chosen over recovering
ownership from ValueDeclaration, which loses the distinction between same-file
binder merges and later checker merges. The later composite factory additionally
retains the original declaration-node symbol's Parent and the raw parent's own
declarations, shared once per parent. Native local-parameter acquisition reads
that raw list, while getDeclaringClass canonicalizes only after its raw Class
gate. These are distinct native observations; presentation identity never
participates in either decision.

`for_each_property` directly invokes the callback for the current Own and
Instantiated factories. Neither factory sets Synthetic. A future synthetic
property producer must retain its actual containingType and add the native
ordered, short-circuit constituent lookup inside this operation. A nil target
class still traverses actual source bases; it is not an early false that could
hide a consumed base failure. The existing class-constructor final RED and the
new nominal-overload controls are the consumers of this increment.

Native property-flags controls passed 5/5 with zero skips in
`vrun-75492246-067a-4f19-b237-ae3587703ec9`. They independently establish writable
Prototype symbols, Public|Static declaration modifiers, independent Property
and Accessor readonly tests, declaration-order/getter precedence, direct class
name selection, and class/namespace prototype binding order. The four final
groups in `tests/type_property_flags.rs` all failed in the genuine pre-fix Rust
run `vrun-2a5819f8-9f39-4a29-91ef-a17fc5334ce7`. Prototype, readonly and direct
class-anchor corrections are now staged. The temporary Collector binding-event
sequence interleaves actual declaration and alias capture, so class prototype
installation sees earlier aliases and precedes class children. Replacing a
prototype replaces the entire prior export entry after any native diagnostic.

The extended native run `vrun-cf3f1e32-eb75-49dd-bb9a-68df7b52b151` passed 5/5,
zero skips. It adds both declaration orders for import/type aliases, and shows
that explicit static prototype properties/accessors supply real ValueDeclaration
metadata: readonly property and getter-only writes consume an error type, while
getter/setter writes retain the declared class instance type. Prototype flags
alone therefore do not bypass the shared declaration-modifier/readonly owner.
The new Source modifier and auto-type operations support both bound and captured
declaration lists without eagerly acquiring ordinary unselected declarations.
Final expanded/control results remain pending the coordinated Rust checkpoint.

The discriminating prototype run `vrun-f1a94145-2dcc-40c3-82a4-e494682db304`
passed 6/6 with zero skips. An ordinary explicit `static prototype: string`
still reads as the class instance because the variable/property worker handles
Prototype first. A getter returning string (with or without a boolean setter)
instead reads string: `getTypeOfSymbol` selects Accessor before entering that
worker. The setter-bearing prototype also writes string because its independent
Property flag takes precedence in `getWriteTypeOfSymbol`. Binding now preserves
that dispatch distinction. Expanded final graphs cover the actual prototype
flags, declaration origins and class-versus-accessor value types across all
three option profiles; those new final assertions await coordinated validation.

## Property failure provenance checkpoint

The full compatibility checkpoint exposed two issues for one unsupported
property read: graph read and dependent write wrappers had different demand
IDs. `DemandError` now retains the actual failed native PropertyValueQuestion
(Read or Write plus PropertySymbolId). The canonical value owner attaches Read
even when called outside the property arena. A property write that delegates
to that read preserves its identity; an independent accessor write retains
Write. Instantiated and printer operations propagate the original question.
Healthy raw reads, non-missing reads and accessor write caches remain distinct.
Presentation uses the retained question for issue and unsupported-node identity,
with its existing deferred-question identity for other failures. Equal source
sites and messages never merge distinct questions.

The existing final CLI partial-delivery test supplied the genuine regression
RED. Its suite passed with zero skips in
`vrun-6d17eeaf-1ef0-421e-b5aa-0d519c179433`. The member-value suite passed with
zero skips in `vrun-57b0cafd-d9c3-4555-ad9e-02d803b94456`, including final
queries that share a failed annotation across distinct properties/accessor
writes and queries that share one read through optional write and inline
display paths, across loose, strict and exact optional modes. Bounded
adversarial peer review found no blocking mismatch in this provenance change.

The next inherited-symbol controls live in `fixtures/types/native-heritage`,
`tools/type-native-heritage-oracle.test.mjs` and `tests/type_heritage.rs`.
They force an actual A/B structural comparison through own marker members;
empty A/B interfaces are a separate nonaugmenting-base shortcut control.
The maintained compiler and final RED checkpoints remain to be run before
heritage implementation. Scratch fresh native controls show Missing is
unvisited for inherited thisless symbols but consumed for parenthesized/union
property annotations, with identical behavior across the three profiles.

The inherited-symbol control baseline is now GREEN
`vrun-26bcdc79-25b9-4cd0-a4dd-f8eb3be93435` (two tests, zero skips; 27
case/profile observations), with actual native nonmissing-property error-type
return observations. The final-query RED is
`vrun-04ef7e88-e5d8-4b18-b41e-60b23bbc25f8` (three tests). The staged interface
implementation and remaining full heritage obligations are recorded in
[the heritage plan](types-native-heritage-plan.md).
