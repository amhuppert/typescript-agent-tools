# Canonical parameters, generic targets and references

This increment uses the Ousterhout designing route. Its operating envelope is
one immutable successfully loaded Project and one request-local Evaluator. It
continues the full T/Q3/Q4 implementation: generic relations, member roots,
standard-library facts, checking order, contextual array widening and resource
gates remain required. The initial final journey is imported JSON over a real
global `interface Array<T>`; the operations are shared generic operations, with
no Array spelling environment or guessed empty standard-library view.

## Existing evidence and first missing journey

The four-test native baseline `tools/type-json-generics-oracle.test.mjs` passed
in `vrun-b202cdec-24a1-4939-b6a9-0f322a21269c`, with zero skips. Its exact
archive is `fixtures/types/json-generic-targets/evidence`. Eight programs and
all eight profiles give 48 admitted semantic matrix requests and 16 input-only
TS5052 controls; 45 further archived requests have fresh checkers and explicit
named questions. Default-node observations do not claim the unavailable public
GetDefaultFromTypeParameter operation. Raw root flags do not prove consumed
recovery, especially for the missing-base control.

Native target/parameter identities are established before constraints, defaults
and unrelated member annotations are demanded. Merged T declarations have one
canonical identity, and a later declaration can supply the first constraint or
default. Real member/index/method-return substitution yields the element type.
`self: this` yields the exact instantiated reference identity. This rules out
both string-based parameter substitution and an Array-only empty shell.

Final `Project::type_of` test eight in `tests/type_json.rs` has genuine RED
`vrun-c2270db8-1b8d-4ac1-b152-6bbfdf48664e`: eight tests executed, seven
passed, one failed, no skips. It fails at the existing explicit pending Array
instantiation operation. Its expected graph contains a real Array root, the
actual number element/type argument identity, `length` at its native own
`globals.ts:1:22-28` origin, and the substituted number index. All six admitted
profiles are retained. The seven existing recovery/null/origin controls remain
GREEN; this RED is not a compile or fixture failure.

## Binder and source ownership

The retained Store owns canonical identity and ordered own declaration
associations. The already repaired bind/merge order is unchanged. New facade
operations read those associations, never Index projections or Store lexical
sites, and do not construct a second member/name inventory.

`BoundDeclaredType` gains `Parameter(BoundDeclaration)`. Its first own anchor
identifies the family and supplies the resolution/display location, preserving
the small Copy dispatch value. `Bindings::type_parameter_declarations(symbol)`
returns the complete ordered own TypeParameter associations for that canonical
symbol, validating the family. It permits multiple merged declarations; an
absent retained attachment remains an internal failure.

`Bindings::class_or_interface(symbol)` returns a complete typed target fact:
canonical symbol, native Class-or-Interface kind, ordered eligible own
declarations and canonical local parameter symbols. Class wins where native
class/interface declarations merge. It reuses the existing local-parameter
operation rather than reimplementing arity. `getGlobalType` continues owning
global lookup/kind/arity and exact recovery causes; genuine target acquisition
then uses this same declared-target operation as ordinary declared questions.

New `source/generics.rs` is a child of Source. It uses the existing captured
syntax cache, final declaration anchors, `type_expression`, exact-name owners
and syntax traversal. Callers supply canonical symbols or typed binding facts,
not AST field names, raw spans to search, or an environment of parameter names.

`Source::type_parameter(symbol, first_declaration)` returns an owned
`TypeParameterRecipe` with exact decoded name and separately selected
constraint/default TypeExpression
recipes. It ports `getConstraintDeclaration` and
`getResolvedTypeParameterDefault`: each selects its own first nonnil field over
the complete canonical declaration list. `getTypeParameterModifiers` combines
native In/Out/Const modifier facts across those declarations; declared variance is separate
from inferred relation variance. Unsupported inferred/mapped parameter
questions remain typed pending questions, never known absent constraints.

`Source::class_or_interface(target)` captures the declared target's header
facts and own declaration attachment. It does not check members or constraints.
`Source::declared_members(target)` owns the existing member projection operation,
extended whole and reused by the existing nongeneric Source interface path.
Properties, methods, signatures, indexes, private/exact keys, merge facts and
base attachments are distinct native questions. Unsupported collection
construction remains explicit; it cannot produce a known empty collection.
Full merged member identity requires the actual binder member owner, not
merging independently collected member spellings in this Source child.

## Evaluator operations and caches

New `evaluation/generics.rs` owns complete parameter, declared-target,
reference-member and mapper operations. Parent dispatch wiring is mechanical;
root retains ownership of its declared-resolution protocol, while ordinary
signature projection/checking remains with its current owner.

`Evaluator::parameter_type(symbol, first_declaration)` gets the complete Source
recipe and calls the one Model parameter factory. The existing canonical
declared-symbol cache owns its TypeId; names never intern parameters. The
factory establishes that identity without demanding its constraint/default.
It registers separate lazy ParameterConstraint and ParameterDefault questions
in the request's demand owner, preserving full failure/cause caching.

Those questions are not plain annotation checks.
`getConstraintOfTypeParameter` first guards `getResolvedBaseConstraint`;
`getConstraintFromTypeParameter` normalizes genuine any to unknown, with the
native mapped-key exception. Missing/error types, circular constraints,
constraints through other parameters and the native recursion guards keep their
distinct states. Default resolution uses its own resolving/no-default/circular
states and first-nonnil rule. Unsupported type families stay pending at the
actual demand. A source absence becomes known absent only where the complete
native ordinary-parameter rule proves it; circular/recovery results are not
converted into absent successful children.

`Evaluator::class_or_interface_type(symbol)` uses the same parent canonical
declared-symbol cache as ordinary declared questions. The native factory
establishes its declared target and actual parameter identities before child
member demands. Generic interfaces and every class get a distinct synthetic
this parameter whose constraint is the exact target. Nongeneric interfaces use
the complete `isThislessInterface` question; a pending base/body fact cannot be
turned into a false this-less answer. The native target-then-member lifecycle
is separate from alias circular recovery.

`Evaluator::reference_type(target_id, actual_argument_ids, consumer)` is the
complete createTypeReference operation. The Model interner owns the single
identity key of actual target TypeId plus ordered argument TypeIds. Actual
arguments exclude the synthetic this argument. The complete reference is
established before its lazy member questions; a later mapper pads the native
synthetic this mapping with that exact reference TypeId. No independently
cached second reference identity is introduced in Evaluator.

The Evaluator owns typed MapperIds and complete mapper records, keyed by actual
type identities. Its instantiated demand cache uses the actual own semantic
member/signature question plus mapper identity. Source origins and graph IDs
do not identify these questions. `instantiateType` handles parameter
replacement, unions, references, anonymous members and signatures through their
existing factory owners. Unimplemented families remain explicit demand
failures. A new member is not unified with its source merely because origins
match, and failed instantiated reads retain their original causes.

## Immutable Model views and deferred collections

Agent two owns the Model/presenter changes. `Model::type_parameter` allocates a
distinct established identity with owned name and lazy constraint/default
TypeDemands; `TypeView::TypeParameter` exposes that immutable view. The presenter
demands a child only after its existing display reservation/admission. A summary
of T does not force its constraint/default.

The generic target view supplies target/actual/local/synthetic-this identities
and native kind; the reference factory interns target plus arguments once.
Array classification requires exact established global target identity, not
nominal name or the noLib option. Array views expose the actual element,
readonly fact and common type arguments. Ordinary generic references use the
same machinery.

Reference member inventories remain deferred native operations. A root Array
summary needs its element, not its members or bases. `Evaluator::members(type)`
therefore performs the complete resolveStructuredTypeMembers operation only
when the caller requests that inventory. It returns an owned established view
or an explicit failure; known empty is a proved result, never a placeholder.
Model root views carry a typed collection demand for such references, not raw
AST or a guessed MembersView. The presenter consumes this operation after the
existing display gate admits the corresponding collection.

This distinction is required before admitting valid standard-library targets:
summary-only arrays cannot force every method/constraint, and expanded arrays
must expose their true property/signature/index facts. Native
resolveDeclaredMembers can consume index type questions while resolving member
inventory; that order is preserved, rather than making every field an
independent syntax-only collection. Unsupported collection recipes become
blocked collections at their actual source; invalid/resource/I/O failures fail
the operation.

## Sequence and validation obligations

1. Collect the public Array RED above against compile-stable shared APIs.
2. Establish the canonical Parameter factory with separate constraint/default
   owners. Add final type-parameter selected/graph controls, including merged
   first-nonnil declarations, genuine any normalization and lazy invalid-child
   refusal; verify omitted children do not force them. Reuse this operation in
   signatures instead of adding a generic function spelling map.
3. Establish true generic class/interface targets, synthetic this and the one
   reference interner. Keep member inventories deferred and implement canonical
   mapper/instantiated question identities. Turn the first final Array journey
   GREEN with actual own property/index facts, not empty members.
4. Expand the same owners through methods/signatures, recursive reference
   members, merged binder member facts, bases, class/accessor/private facts and
   real standard-library collections. Preserve native query order and failures.
5. Route all array factories and annotations through those owners. Extend the
   shared union/contextual widening operations for recursive/unlike-object
   JSON arrays, optional slots, nullable/empty arrays and readonly fallback.
6. Validate refresh/service equivalence, display reservations, shared/cyclic
   identity, semantic guards and work/memory limits using final operations.
   Measure storage/load/query effects; the SmallVec association payload or a
   prototype timing is not a retained-heap or complete performance proof.

Each new behavior family uses registered single-file public TDD. Mechanical
type/dispatch scaffolding has no independent behavior RED; it is identified as
such and followed by final journey proof. Independent Ousterhout/adversarial
review checks early identity versus recovery, exact associations, lazy child
boundaries and no second owner. This plan does not claim completed generic,
Array, JSON, representation, resource or full semantic gates.

## Parameter checkpoint and target lifecycle refinement

The first three final product tests passed in
`vrun-136a2036-da03-47fa-ad85-a3a4f91a684b`, with one explicitly matched
test file and zero skips. They follow the assertion RED
`vrun-97bd635e-b9dc-4458-9fa0-6a23bf85bfa7`. They cover canonical T
references, independently selected later merged constraint/default fields in
all six admitted strict-null/exact-optional/unchecked profiles, lazy invalid
children at summary/depth-zero boundaries, unresolved annotations when admitted,
and ordinary genuine any-to-unknown constraint normalization. The exact native
controls are archived separately in `fixtures/types/parameters/evidence`.

The specialized constraint operation currently establishes non-constrained
primitive/literal/special/object families and unions whose base constituents stay
unchanged. Parameter chains, mapped/inferred constraints, constrained union
reduction and the native circular/recursion protocols remain explicit pending
questions at the child. The default operation uses complete canonical first
nonnil source selection and the existing type-node owner; full instantiated
default/circular-sentinel behavior remains required. This checkpoint proves the
listed final journeys, not complete generic parameter semantics.

An independent selection control found that an escaped parameter has a decoded
semantic name but a raw native printed spelling. The declaration and reference
selection lane is adding genuine final tests for that distinction. The Model
must retain these separate facts rather than using the semantic name as a
universal summary token.

Design-twice review settled a native lifecycle refinement for declared targets.
A complete factory accepting already-created parameter TypeIds would reverse
native allocation order. The sole Evaluator target operation will call narrow
private Model begin/complete operations: allocate the real declared target and
publish its canonical cache identity first, acquire canonical outer/local
parameters second, and allocate the synthetic this parameter last. The Model
distinguishes allocating, complete and poisoned states. A later failure retains
its original cause in the parent cache and poisons an escaped target; incomplete
parameters, this facts or members never become guessed empty views. Only this
deep operation manages the lifecycle; parent dispatch and ordinary callers do
not manage builder steps.

Deferred inventory uses a typed `MembersOfType` token wrapping the established
TypeId. `resolveStructuredTypeMembers` is one fixed question per type identity,
so a second collection-ID registry adds no distinction. The Evaluator owns its
result/failure cache; the Model factory installs the self token and the presenter
requests it only after collection admission.

For the first generic/Class target family, canonical Class-or-Interface flags
and the existing local parameter owner supply the header. Nonempty parameter
lists or Class kind prove native synthetic-this construction without inspecting
members or bases. Outer parameter acquisition, nongeneric interface native
ContainsThis/isThisless lifecycle, complete merged member tables, and reference
classification outside a proved canonical global Array target remain required.
The intermediate increment will use a single deep target entry point and retain
the existing admitted nongeneric interface path inside it until those whole
native questions are ported. This is an implementation sequence, not a reduced
supported contract or a permanent split in ownership.

## First target, mapper and reference checkpoint

The admitted target path now publishes a real Model target before canonical
local Parameter factories, completes its synthetic this last, and poisons both
escaped Model state and canonical cache with an original failure. Classification
currently uses a pure retained global-header inspection before allocation;
future global/base/context operations must preserve the full native publication
order instead of generalizing that bounded acyclic inspection.

Source explicit property, call/construct and index recipes use one structured
member operation. Declared and instantiated member inventories have separate
native question caches. Actual mapper IDs key canonical source/target TypeIds;
lazy instantiated reads retain original child DemandId plus MapperId rather
than source-origin identity. Number indexes demand their value before key,
preserve first-key identity after checking later duplicates, and substitute
only the established value as native instantiateIndexInfo does.

The first imported JSON Array journey is GREEN
`vrun-71fca5a4-34f8-4d73-9650-f13e2e2fec0d`, with eight tests and zero
skips, after genuine eighth RED
`vrun-c2270db8-1b8d-4ac1-b152-6bbfdf48664e`. It proves all six option
profiles' actual Number element/argument identity, own length origin/read,
substituted number-index value and admitted exact inventory. Native temporary
array-literal identity and whole-root widening route through the shared Model
and Evaluator operations.

Implementation exposed a separate native question: a healthy declared target
does not establish a source type reference's argument count or defaults. The
complete reference operation now uses default presence before child demand,
checks count before actual arguments, and fills defaults in order through
canonical mapper identities. Unsupplied forward/self default references retain
a typed native errorType mapper target, never a healthy any/TypeId. Overridden
invalid defaults and unused constraints stay undemanded. A merged-interface
header can establish these parameter/default facts while its merged inventory
remains explicitly pending.

The seven final reference tests followed desired assertion RED
`vrun-a8c8e56c-15af-4e33-b6ea-1ee250291ee0` with GREEN
`vrun-c993a08c-1378-4e24-8ac0-44c5aa1dca23`, zero skips. The earlier
member-operation visibility attempt failed compilation and supplies no behavior
RED. Eighteen independent native TypeReference controls and earlier direct-alias
observations are archived under `fixtures/types/reference-arity/evidence` with
their capture limits. Directly aliased generic references remain an explicit
pending deferred-node/alias-context protocol: target+arguments is not a universal
instantiateTypeWithAlias interner.

Class, merged and inherited inventories, native nongeneric ContainsThis,
outer/context parameters, recursive canonical member roots, method/accessor/
signature instantiation, complete constraint/default sentinels, generic aliases
and structured/union mapping remain required. No representation, resource,
standard-library or complete generic gate is closed by this checkpoint.

## Canonical class header checkpoint

The private declared-family enum is now `ClassOrInterface`. Canonical symbol
Class flags take precedence over Interface, independently of a namespace's
first own display-name attachment. The same early target/cache/poison owner
constructs class instance references, local parameters and synthetic this;
there is no class-specific reference shell or constructor/value approximation.
Final `tests/type_generics.rs` eighth behavior RED
`vrun-3359cece-1a9f-4cd7-8d76-02e9ab05666e` failed the selected class reference.
GREEN `vrun-82bf64d6-dabb-4ef9-ba84-e3e9fa0b5aba` passed all eight tests,
zero skipped, including generic/nongeneric classes, later merged defaults,
interface-first Class precedence, noninstantiated namespace attachments,
written escaped names, unused constraints and overridden invalid defaults.

Fresh compiler baseline `vrun-d30191e2-e9e0-44f0-bb3b-da0629470964`
passed two controls with zero skips. The raw instantiated namespace-before-class
case has a healthy type but separate post-demand TS2434; existing Project load
rejection was preserved, and the positive case uses a valid type-only namespace
attachment. Archived exact source rows and post-observation provenance are in
`fixtures/types/native-class-targets/evidence/README.md`. They do not prove an
atomic compiler input capture. Class declaration constructor/value questions,
ContainsThis/IsThisless bases, canonical class/merged members and complete
instantiation remain mandatory subsequent work.
