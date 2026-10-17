# Native mapped types

This is the implementation boundary for the remaining mapped-type producer and
its consumers. TypeScript Go 7.0.2, commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`, defines the behavior. The scope follows
[types-design.md](types-design.md); a symbolic mapped type is a valid answer.
Mapped declaration, lazy-object, factory, member, operator and rest integration
compiled at stage 14. The current source window stages mapped source checking,
native default serialization, live relations, generic/reverse inference and lazy
graph adapters. The integrated mapped baseline compiled and passed four of five
final test groups; numeric property-name rendering was the remaining failure
(`vrun-ed13508b`). The current repair is staged, not yet validated in Rust. Required
shared unique/late keys, generated display naming/scopes and final proofs remain
part of the same delivery scope.

## Boundary and alternatives

The operating envelope is one request-local Evaluator with lazy, reentrant
semantic operations. Native allocation identity, acquisition order and retained
failures are observable. This needs no locks, cross-request cache or new retry
mechanism.

Two plausible designs were considered:

1. Dedicated Model `Mapped` and `ReverseMapped` variants with immutable headers,
   sharing the existing object-instantiation and member operations. This makes
   the families explicit but duplicates the existing deferred-object header and
   adds family dispatch throughout mapper, graph and structural consumers.
2. Generalize the existing deferred-object representation, now named
   `Type::LazyObject` / `TypeView::LazyObject`, with an honest lazy-object name and
   exact native flags. A dedicated `evaluation/mapped` owner implements mapped
   semantics; existing instantiation, member and symbol owners retain their
   decisions.

Choose option 2. `Model::anonymous_type_ex` already stores an actual symbol,
mapper, metadata and arbitrary authoritative object flags without resolving
members. Mapped objects carry `MAPPED`, without `ANONYMOUS`; reverse-mapped objects
carry `REVERSE_MAPPED | ANONYMOUS`. Rename the representation mechanically and
audit its consumers rather than silently broadening an Anonymous assumption.
Native semantic predicates continue to inspect flags or actual identity.

The bounded peer review identified a concrete migration obligation: current
Anonymous consumers in `generics.rs` select `anonymous_members` and
`anonymous_consumer` directly, and instantiated anonymous resolution installs
its provisional inventory before acquiring the target. Dispatch mapped families
to their own native member and parameter-clone branches before entering those
anonymous schedules. Establish the final Mapped-only flags before publication;
renaming the view does not make `ObjectFlagsAnonymous` true. The existing
flag-based empty-anonymous predicate remains shared.

The lazy-object association/header establishes declaration, target and mapper
once. Mapped caches consume that header; they must not introduce an independently
mutable copy. The existing Model mapper observation remains nonforcing.
`getObjectTypeInstantiation` owns effective outer arguments, alias identity and
its target cache. StructureState owns installed inventories and the distinction
between absent, installed-nil and present-empty raw tables. The property arena
owns generated symbol identities and their read/write resolution slots.

## Complete operations

The module interface exposes complete native questions, such as
`mapped_type_node`, `instantiate_mapped_type`, `resolve_mapped_type_members`,
`name_type_from_mapped_type` and `substitute_indexed_mapped_type`. Callers do not
manually initialize caches, build mapped property records or repeat mapper rules.
Related but different native questions retain separate operations.

| Native operations | Implementation responsibility |
| --- | --- |
| `getTypeFromMappedTypeNode`, `checkMappedType` (checker 24163, 3371) | Source retains the actual declaration, parameter, optional name/template, modifier tokens, syntactic keyof constraint and alias recipe. Binder creates the actual anonymous TypeLiteral symbol and local parameter scope. The type-node cache installs the allocated mapped type before eager constraint acquisition. Checking visits parameter, name and template before checking name or constraint against string/number/symbol. |
| Parameter/constraint/name/template/modifiers queries (22576–22624, 27956, 28769–28820) | Mapped owner retains native lazy slots and exact nil distinctions; reuse guarded parameter constraints and evaluator optionality. Own homomorphic-variable, name-kind, generic, partial and combined-optionality decisions. Syntactic keyof classification must survive reduction. |
| `instantiateMappedType`, `instantiateAnonymousType`, mapped array/tuple/template operations (22370, 22441–22574) | Extend the shared object-instantiation owner. Clone the local parameter freshly, combine original-to-fresh before incoming mapping, and install that mapper on the fresh parameter. Preserve homomorphic distribution, primitive/Any/wildcard/error decisions, array/intersection branches, tuple labels and optional/rest/variadic flags. Instantiation must not enumerate members. |
| `resolveMappedTypeMembers`, `getTypeOfMappedSymbol`, `getLowerBoundOfKeyType` (20799–20956) | Install native nil table/empty inventory before parameter, constraint, target/name-kind, template and modifiers reads. Enumerate syntactic keyof properties/indexes through shared owners; otherwise lower constraint keys. Merge colliding nameType before keyType. Lazy property values use the existing symbol Type resolution key, Optional/StripOptional order, containsError and native circular recovery. Publish final members through StructureState. |
| Apparent/keyof/indexed/contextual operations (21620–21703, 26708–26799, 29048, 30364–30429) | Complete mapped apparent caching, generic indexed simplification/constraints, `getIndexTypeForMappedType`, optional-property substitution, contextual filtering and excluded conditional keys. Preserve alias/origin identity, generic deferral and mapped-property cycle observation. Route existing mapped guards to these owners. |
| Mapped relation branches (relater 3425–3985, 4583, 4624) | Use the existing live Relater/Ternary session, including reliability callbacks, optionality policy, actual name-type equality, indexed/keyof branches and recursion identity through `getMappedTargetWithSymbol`. Do not replace active comparisons with independent Boolean relations. |
| Generic and reverse inference (inference 653, 914–1150) | Extend existing candidate inference for generic mapped constraints/templates/names and `inferToMappedType`. Own homomorphic reverse creation, partially inferable sources, limited intersection-origin constraints and `replaceIndexedAccess`. Reverse members and lazy property values use the same arena/installer. |
| `createMappedTypeNodeFromType` and reverse-mapped serialization (nodebuilderimpl 1452–1565, 2530–2665) | Preserve bound parameter scope, exact modifiers/remapped name, homomorphic and modifier-preserving wrappers, generic/error mapped syntax versus resolved members, and reverse-mapped elision. Graph operands and summaries demand children only after admission; previews do not enumerate members. |

## Identity, installation and failures

- Root mapped identity uses the actual bound TypeLiteral symbol and captured
  declaration. Reverse-mapped object identity has an actual nil symbol. Neither
  uses graph keys, display text or newly invented declaration origins.
- The current Mapped binder branch captures a parameter scope but lacks its
  native anonymous TypeLiteral symbol. Add that symbol through the existing
  anonymous declaration factory, preserving the scope rather than replacing it.
- Native cache installations remain observable on reentry. Product failure
  provenance accompanies them; an installed value must never turn a consumed
  DemandError into success. Do not add a generic Resolving-cycle refusal where
  the native owner relies on the existing resolution stack.
- Reverse inference has two distinct triple-key caches. A nil result in
  `reverseHomomorphicMappedCache` is retried. Presence of a nil entry in
  `reverseMappedCache` yields Unknown on a subsequent lookup, although the first
  nil remains nil. Original failures are separate from both native nil states.
  Restore source/target stacks and expanding flags on every exit; reuse native
  recursion identity and its depth-two check.
- Deferred mapped templates and generated property values retain their mapper
  lifetime through the existing inference/context owner. Do not force unrelated
  slots merely to avoid a later callback; preserve original failure poisoning.

## Shared prerequisites and parallel work

Unique/late property names need one complete shared owner before full mapped
delivery. It must provide `isTypeUsableAsPropertyName` /
`getPropertyNameFromType`, actual unique-symbol escaped identity, late binding
and exact table keys. A public String key or printed unique-symbol name cannot
stand in for native identity. Existing `literal_type_from_property` consumes
those facts rather than reconstructing them.

Mapped generated symbols need actual nameType, mutable keyType and containing
type links; Mapped/Readonly/StripOptional/Late flags; optional syntheticOrigin and
copied Declarations. Native creation sets neither Parent nor ValueDeclaration.
Same-key collisions update nameType then keyType before lazy reading. Reverse
symbols copy source names/declarations/nameType and retain their source property
type, mapped type and constraint. Extend the existing arena with typed recipes
and link operations; do not create another property ID or value cache.

Root's IndexInfo prerequisite is compiled and tested: immutable Arc identity,
`Model::index_info(key, value, readonly, declaration, components)`,
`Index::same_identity`, a shared anyBase sentinel, and complete
`append_index_info(result, new, union, consumer)`. Unchanged instantiation shares
identity; changed values allocate fresh records. Mapped generation uses native
nil declaration/components and the union append policy. The bounded native
review and coordinated compilation passed. Mapped callers must still preserve
the native key/value acquisition and append order.

New `source/mapped.rs`, `evaluation/mapped.rs` and reverse-inference implementation
can be developed alongside unique/late-name and IndexInfo work. Shared edit
windows must cover the lazy-object Model rename, binding symbol factory,
anonymous instantiation, property arena, structure/keyof/indexed/contextual
dispatch, Evaluator state initialization and presentation. Relation/inference
mapped adapters remain with the mapped owner; existing template matching stays
with its owner. This division separates native decisions, not runtime phases.

## Proof and completion

Take expectations from the pinned compiler before final assertions. Cover strict,
exact-optional and loose profiles; unchanged/include/exclude readonly and
optionality combinations; property/index modifiers; string, number and unique
keys; filtering, remapping, collisions, Never keys and templates; homomorphic
primitives/unions/intersections/arrays/labeled optional/rest/variadic tuples;
generic relations, indexed/contextual reads and reverse inference; recursive
aliases/property cycles; failed lazy reads followed by healthy work; and symbolic
summaries, preserved modifier wrappers and bounded graph previews. Exercise
unique/late identity and cache nil/reentry rules through final consumers.

Coherent checkpoints may establish shared prerequisites first. Full delivery
requires the listed native operations and final-query matrix, including controls
that fail with the relevant implementation removed. Named unfinished dependencies
remain unfinished work; no empty inventory, Unknown/Any fallback or arbitrary
Pending subset counts as completion. Run only registered validation under the
shared source-freeze protocol.

## Next coherent implementation stage

The first activation covers the mapped declaration, instantiation and member
operations, including their existing generic/keyof/indexed/contextual consumers.
It is not an `Omit` implementation and introduces no library-name dispatch.
Reverse inference is a subsequent complete operation in the same delivery;
reaching its current missing producer still fails until that operation lands.
This checkpoint boundary follows native ownership, not a restriction on mapped
syntax or an assertion that the full mapped family is already delivered.

1. **Retain the actual declaration and header.** Add `TypeExpressionForm::Mapped`
   with an actual captured node; `source/mapped.rs` returns the parameter identity,
   optional name/template child handles, modifier tokens and syntactic keyof
   operand. It must not acquire semantic children while describing the recipe.
   Extend the existing binder anonymous-declaration factory for MappedType
   (binder 677), preserving its type-parameter scope. Allocate Mapped-only flags,
   acquire its alias, install the node result, then ask its constraint (24163).
   Keep the normal source-check operation separate: grammar, parameter, name,
   template, implicit-any diagnostic, mapped acquisition, then reporting
   assignability of name or constraint (3371). Store failures with the existing
   native installations; reentry cannot erase an acquired failure.
2. **Share object instantiation; give mapped slots one owner.** Perform the
   Anonymous-to-LazyObject representation migration with the new dispatch, not
   as an unrelated rename. The shared object association retains target/mapper
   and the actual root declaration; mapped links contain only its native lazy
   parameter/constraint/name/template/modifiers/apparent slots and containsError.
   Do not copy mutable target/mapper state into another cache. Native
   getObjectTypeInstantiation still owns outer parameters, effective arguments,
   alias keys, empty-parameter early return and cache publication (22209).
   Its mapped branch calls instantiateMappedType; the common anonymous factory
   performs the mapped parameter clone before alias instantiation (22370).
   Extract the existing signature clone setup into the same parameter-clone
   operation rather than reproduce mapper/constraint/default links. Add the
   native appendTypeMapping alongside prependTypeMapping; composition direction
   and fresh mapper identity remain owned by the existing mapper representation.
3. **Implement all mapped construction branches.** Homomorphic distribution
   preserves primitive, Any/Unknown, wildcard and recovery distinctions;
   array/tuple/intersection handling and nameType's syntactic absence follow
   22441–22574. Tuple labels, fixed-prefix string keys, optional/rest/variadic
   flags and readonly use the existing factories. Extend the post-instantiation
   couldContain flag operation for a mapped result that is a union/intersection:
   native 22294 sets Computed on such a result only when no effective argument
   contains variables. The current unconditional mark is valid for the existing
   ordinary object results but must not be reused for these new result families.
4. **Install members and lazy values through existing owners.** Resolve mapped
   members in native order (20799), including the initial nil table, target
   name-kind test, template and apparent modifiers. The property arena gets a
   Mapped value recipe and native links, using its existing read slot and Type
   resolution key. Same-name collisions union nameType first and keyType second
   while retaining the symbol identity. A property's value is instantiated only
   at getTypeOfMappedSymbol; optionality/StripOptional, containsError and circular
   diagnostic display retain their own order (20903). Index values are acquired
   while enumerating keys and use immutable IndexInfo plus appendIndexInfo.
5. **Replace every existing mapped guard at its native decision.** Implement
   generic/name-kind/modifiers queries, lower key bounds, apparent mapped types,
   getIndexTypeForMappedType and indexed substitution in the mapped owner. Reuse
   the existing no-reduction mapType owner. Route structure, keyof, constraints,
   indexed access, conditional inferred constraints, contextual property lookup,
   tuples and const-variable checks to these operations. Name-kind is the actual
   Assignable(nameType, iterationParameter) question (26708), not the presence of
   an `as` token. Preserve live Relater/Ternary state in the mapped relation
   branches; do not hide them in an independent Boolean relation. Existing
   generic-to-generic inference can use the shared slots immediately; reverse
   inference retains the two distinct native cache protocols described above.
6. **Expose lazy display children through the operator owner.** A mapped graph
   header first asks the exact native isGenericMappedType operation, even at
   depth zero. This can consume a constraint or remap dependency. A generic map
   then establishes its binder and child count without resolving its template
   or enumerating members. Concrete maps use the ordinary mandatory inventory
   question for their real object/function kind. Operand demands call the same slot operations after
   Q4 admission. A missing template is an actual deferred errorType dependency,
   not an absent healthy child. The native syntax builder owns
   createMappedTypeNodeFromType's template-first acquisition, scope and modifier
   preserving wrappers; product summary/budget policy remains in presentation.
   Test alias-hidden invalid templates at depth zero versus an admitted child,
   and preserve the existing failure identity/byte rollback rules.

### Property-key prerequisite and file lanes

Current `NativeSymbolName::Bytes/Unavailable` does not implement unique-symbol
keys. Created properties now retain their exact header bytes without a duplicate
String name. The complete shared key owner must land
with the mapped activation: exact text keys, reserved byte names and generated
unique/private forms remain distinct. Unique names retain their underlying
structured symbol name, even when that name is generated. Lookup equality uses
the actual symbol identity; generated-name ordering uses the adopted request-local
IdentityOrder policy. Formatting the key must not create a second identity.

The algebra lane owns `source/mapped.rs`, `evaluation/mapped.rs`, shared
object-instantiation integration and the operator consumers above. Coordinate
one mechanical Model lazy-object migration window. The member lane owns the
shared key/unique-symbol/late-name factories and a narrow
`property_symbols/mapped.rs` arena extension; no second symbol table or read
cache is introduced. The binder mapped anonymous-symbol change belongs in its
existing collector owner, coordinated with that lane. Relation/inference edits
need a reserved window from their owner; reverse inference should live beside
the existing inference state, not in a parallel inference subsystem. Root's
presentation/schema and resource owners receive only the necessary narrow
adapter windows. Each slot must preserve acquired DemandError precedence over
a subsequently observed successful reentrant cache entry.

### Evidence and first final controls

Object-rest native evidence is GREEN
`vrun-318f7c4e-0254-49ce-9ee4-c7f7bdac9918`: 54 ordinary rest observations across
strict/loose/exact profiles and 20 global/helper dependency observations. The
mapped alias in `native-object-rest/omit-mapped.d.ts` is an **inline mapped
control**, not the bundled standard-library Omit implementation. Keep that
control and add the exact pinned declarations from `internal/bundled/libs/lib.es5.d.ts`
1604–1628:

```ts
type Pick<T, K extends keyof T> = { [P in K]: T[P] };
type Exclude<T, U> = T extends U ? never : T;
type Omit<T, K extends keyof any> = Pick<T, Exclude<keyof T, K>>;
```

The distinction is observable: getModifiersTypeFromMappedType (27956) can
recover T through Pick's constrained K; substituting an inline Exclude
constraint is not that native question. Add renamed equivalent aliases to prove
ordinary alias semantics, readonly/optional retained properties, exclusion of a
property with an invalid value type, and a retained invalid property consumed
only by the admitted value edge. Include concrete and still-generic rest results.

The new mapped matrix must additionally cover the nine readonly/optional
modifier combinations in all three option profiles, numeric/text key collisions,
remapping versus filtering and Never names, template remapping, duplicate mapped
names producing a union value, pattern/string/number/symbol index keys, and two
distinct unique symbols with identical display descriptions. Pair homomorphic
primitive/union/array/tuple/intersection controls with an identity `as P` clause:
the latter's mere presence disables the native tuple/array shortcut. Preserve
native call/inference tests for generic mapped comparison and reverse inference
as required later checkpoints, plus failure-then-independent-healthy queries in
one expanded request. Native expectations and the matching final Rust queries
must precede success claims; no helper-only or alias-summary-only proof closes
member/key/mapper behavior.


## Current closure checkpoint

The mapped relation child runs inside the existing Ternary session. It now owns
mapped source/type-parameter proof, generic target alternatives, mapped-to-mapped
constraint/name/template comparison, both keyof directions and source indexed
constraints, plus index-signature and inferable-index branches. Recursion unwraps
instantiated homomorphic targets with symbols before comparing identities.

Reverse inference uses two separate native triple-key caches in InferenceContexts,
with nil retry versus cached-nil-to-Unknown kept distinct from failed questions.
The property arena and StructureState remain the sole symbol/read/inventory
owners. Reverse instantiation precedes ordinary object instantiation. The shared
getTypeFromInference operation selects covariant Subtype union, contravariant
intersection, or actual nil for both inference entry points.

Default mapped diagnostic serialization and bounded summaries preserve modifier
tokens, template-first acquisition and name/template scope. Graphs classify by
native genericity; generic operands and binder constraints remain deferred after
that mandatory header. Reverse display uses the native property recursion stack
and elides index values without reading them. The shared display scope currently
preserves mapper lifetime and nil-enclosing behavior. GenerateNames wrapper
factories and nonnil fake scopes requiring declaration-less parameter symbols
remain precise dependencies, not an implemented approximation. Existing public
diagnostic flags do not request GenerateNames. Those native branches remain
required delivery work; adding a flag consumer cannot precede their implementation.

Native capture `vrun-a662c7fa-03ec-4c23-b4bb-65d2b17a0ba1` recorded 222 roots across
strict, loose and exact options. The maintained family uses bundled definitions
of Pick/Exclude/Omit plus independently named equivalents, all nine modifier
combinations, key collisions/remapping, homomorphic arrays/tuples, reverse object/
array/tuple/index results, source-check failures and lazy graph headers. The next
asserted run extends inference/relations and must finish before these expected
files count as maintained proof. Absolute native unique-symbol counter spellings
are deliberately excluded; distinct key identity, order, values and origins stay
asserted under the documented identity policy.

The extended matrix has 88 selected roots across the same three profiles,
including three reverse-result reads and nine generic relation consumers. The
single-file rerun `vrun-3ae90db1-004f-49f1-919b-3781cab22b2c` reached the registered
900-second limit after completing core, homomorphic and generic captures. This
is a validation timeout, not a compiler counterexample. The maintained harness
now shares one acquisition/assertion helper across four bounded test-file scopes;
all cases and assertions remain. Fresh inference, lazy and relation captures must
be asserted and consumed by final Project tests before this increment is proven.

The read-only relation audit also identified a precise correction: the generic
mapped-key fallback for a target Index must accept exactly TernaryTrue
(relater 3523), whereas the source-side fallback accepts any non-False result.
Recursive Maybe must fall through on the target side. A separate native fixture
uses recursive mapped-key constraints and overloaded calls so the final selected
return type can witness that decision; it is a candidate until captured and
counterfactually verified, not a claimed final proof.


### Numeric-name repair and extended evidence

The integrated `numericName` failure expected `{ 1: "1" | 1; }` but Q4 quoted the
key. The root cause was a second property-name decision in presentation. The
native display owner now exposes both a nonforcing name preview for row admission
and the complete admitted spelling operation. Both diagnostic and Q4 display use
`getPropertyNameNodeForSymbol`: actual private ValueDeclaration, ordered
`Every(isStringNamed)` then `Every(isSingleQuotedStringNamed)`, Method flags and
nameType precede fallback classification. JSON declarations retain their admitted
double-quoted StringLiteral fact from `parser/json::property`; key text alone
never substitutes for that fact. The admitted order remains nonMissing property
read, name, then accessor/method serialization. Computed and unique names stop the
preview before a semantic name read. Existing Q4 byte-boundary tests remain
required compatibility coverage.

The independent property-name capture `vrun-c03058a3-309a-4fd1-9f6f-cf5e44f2f2bd`
proved eleven ordinary/quoted/numeric/computed/mapped/declaration-merge cases;
the maintained extension adds the quoted method name `new`. Capture4
`vrun-49ac22d2-9c9f-40b5-9ed0-4b52f5c1d5ca` recorded the full reverse-inference,
lazy, source-check and relation rows. Their expected files are installed, with
asserted reruns and final Project queries required before closure. Core graph
checks now assert property order, read summaries and exact origins in addition
to modifier flags, so alias-only healthy summaries cannot hide member failures.

Recursive mapped-key overload capture
`vrun-52c8bf68-568d-4a89-860c-2cdd37a5b7ab` selects the second overload at all seven
roots. The final queries are a candidate counterfactual for the exact-True
correction; a successful baseline alone does not prove the Maybe branch occurs.


The stronger direct self-remap probe (`[P in keyof T as keyof SelfKeys<T>]`)
crashed pinned Go7.0.2 during `getNameTypeFromMappedType` /
`shouldDeferIndexType`, before the relation worker. Receipt
`vrun-47f4469f-73c2-4952-a498-6a0d159e3b91` retains the stack overflow; the probe
source SHA-256 is `35bd145cb861d2b7c3860dae064309f0def44ea167919893065db21bc79764ef`. It is excluded from maintained harness paths and
retained only at `.cc/temp/native-mapped-index-maybe-probe/cases.ts`. Do not run
it through the Rust evaluator as a healthy parity case. Exact-True evidence
still needs a non-crashing discriminating final query.

The asserted mapped repair run `vrun-9f10cd19-1df2-4e3e-86b2-2b31e012b977`
passed all six maintained tests across inference/lazy/checking/relations/cycles
and the property-name capture. The twelve-row name expectation file and final
Project test are staged; independent assertion run
`vrun-5aa6b583-4681-4340-a6cf-ecefaed96d24` passed.
The final name tests also retain a Q4 lower-bound pair: a short computed string
key consumes its Missing value, while a 600-byte exact string key cannot fit a
summary row and must be elided before the same value question. The string-key
preflight refinement is staged in the shared Q4 row preview and awaits the
stage17 final Project checkpoint.

### Object-literal prerequisite for reverse inference

Stage16 exposed `recordProperties` at contextual object-literal acquisition.
The complete `checkObjectLiteral` operation is now in
`evaluation/object_literals.rs`; `source/object_literals.rs` retains actual
member/declaration roles. It owns grammar effective-name reads, the cached
contextual frame, computed-name prepass, property/shorthand/method acquisition,
fresh symbol optionality, inference sites, spread flushes and final type creation.
Contextual discrimination uses the shared key-map and lazy discriminator loop;
`getContextFreeTypeOfExpression` has its distinct cache and contextual override.
Name classification, raw-to-checked property creation, spread algebra and
structured member installation remain their existing shared operations.

The object owner and mapped/reverse/contextual discriminators now use
`NativePropertyKey`. Exact byte keys and private/unique identities remain distinct;
no caller invents generated symbol spelling. Missing generated factories and
canonical fake-scope parameter symbols remain required work in the same delivery.

Peer review of checker13094–13317 and13605–13799 corrected grammar collision
early-return behavior and the native final-spread distinction that preserves the
computed-symbol flag while clearing string/number flags. The bounded reread
passed; these source-level findings do not yet have a discriminating final
counterfactual. Initial combined grammar fixtures were rejected by the pinned
binder, so they were split according to actual compiler admission rather than
used as query parity evidence.

Native capture `vrun-1e5a4a1c-405c-472c-a046-f1f9d3652841` records 25 cold object
queries in strict/loose/exact profiles plus five separate grammar/load cases.
`tests/type_object_literals.rs` asserts the final public results and load
boundary in five groups; it is staged, not yet validated. Maintained assertion
run `vrun-c8c43f59-2be1-4fd8-806c-ddd7acdb64f2` passed all four native tests with
no skips. Existing mapped and elaboration final tests also exercise the
contextual owner at the coordinated stage17 checkpoint.
