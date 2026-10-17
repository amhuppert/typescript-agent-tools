# Native member value questions: next production increment

The operating envelope is the existing immutable accepted TS/TSX Project and
one request-local Evaluator. The retained Store owns canonical symbol identity,
flags, value declaration and native ordered own associations. Source owns final
AST attachment and syntax projection. Evaluation owns native Type/WriteType
properties and demand history; Model owns type identities/option-dependent
factories. The pinned implementation is TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

This plan extends the same canonical member operations already used for named
selection and inventory. It creates neither a spelling-based member binder nor
a parallel callable signature factory. Full member families remain required;
the first accepted annotation/ordinary-method increments do not close the full
semantic design.

## Current bounded proof

The nominal member suite `tests/type_member_values.rs` is GREEN at
`vrun-c90be8fb-4a6f-431f-b42a-106c90537c75` (one explicit file, zero skipped).
Its eight tests cover raw accessor read precedence at both own anchors,
strict/exact optional states, ordinary and overloaded methods, actual generic
read/write/signature substitution, class instance/accessibility metadata,
lazy nominal summary/depth-zero behavior, genuine Any/Unknown versus consumed
recovery, and all five setter parameter roles across BOM and ECMAScript line
break variants. The first compiled checkpoint `vrun-0941a19f-3442-4d03-8b9c-1da4c9837220`
passed five tests and exposed the optional anonymous function union's missing
parentheses; the presenter now owns the complete emitted-type precedence rule.
Two test premises were corrected separately: loose plus exact is a rejected
TS5052 configuration, and a whole depth-omitted field has no invented exact
count. Neither correction weakens the native parentheses assertion.

Own and instantiated property serialization now run the same complete decision
over the actual mapped raw/nonmissing read, actual write and actual own member
inventory. Mapping a previously chosen clause is removed. Property writes map
raw read before removing Missing; Accessor writes map the target's independent
write question. A full serializer's already consumed nonmissing read is Known,
so there is no parallel nonmissing-demand registry. Method own-object-property
acquisition precedes readonly, matching native question order.

This does not complete inline canonical members, class overloaded implementation
exclusion, full anonymous alias/context instantiation, receiver/context/body
dependencies, or actual instantiated member identity. These remain mandatory.
The independent inline suite `tests/type_member_display.rs` still has a genuine
four-test RED at `vrun-e6b78d99-9c62-4f7f-a39c-fa53b78df496`; native controls are
GREEN at `vrun-06824310-0f87-4728-aa41-94cfbc8ccf10`.

## Next inline canonical binding boundary

Native `bindAnonymousDeclaration` binds a TypeLiteral's own anonymous symbol
without inserting it in a lexical or export name table. The existing Collector
currently clears `member_owner` for TypeLiteral; Source therefore cannot obtain
canonical Method/Accessor identities there. The next increment must retain one
anonymous declaration association in the existing Store, then route TypeLiteral
members through its existing canonical member table and declaration/merge/value
owners. It must add no second Source name inventory and no synthetic Interface
declaration to borrow class/interface semantics.

The planned private boundary is an actual `BoundTypeLiteral` acquisition from
the exact retained native kind/first-token association, projected as a typed
TypeLiteral recipe. The evaluator establishes its real Anonymous identity on
that canonical symbol before inventory, then its existing `members(TypeId)`
operation projects the same canonical named/reserved members. Source's current
`canonical_members` operation will generalize the container input, preserving
one named-member/order/signature/index projection rule. Inline serialization
continues through the established PropertySerializationId operation, including
the setter's actual return demand even though its clause omits a return type.

Named class/interface and anonymous container identity remain distinct native
questions. Their shared member projection may reuse the same operation only
where they ask `getNamedMembers`/signature/index acquisition. Alias metadata,
TypeLiteral contextual instantiation and mutable factory/cache lifecycle stay
with their native owners; missing captured association remains a failure.

## Native questions and their owners

| Question | Native source | Production owner |
| --- | --- | --- |
| Raw symbol read | `getTypeOfSymbol`, checker.go16406 | Existing parent `value_type` dispatch; canonical symbol read property retained there. |
| Property read | `getTypeOfVariableOrParameterOrProperty`, `getTypeForVariableLikeDeclaration`, checker.go16457/16570 | Existing property/source/evaluator operation, extended with actual `isProperty` and optionality facts. |
| Accessor read | `getTypeOfAccessors`, checker.go18424 | `evaluation/members.rs`, on the parent canonical symbol Type property. |
| Native write | `getWriteTypeOfSymbol`, checker.go16347 | `evaluation/members.rs`, separate WriteType cache and native dispatch. |
| Accessor write | `getWriteTypeOfAccessors`, checker.go18479 | Same child, separate WriteType resolution property. |
| Annotated accessor syntax | `getAnnotatedAccessorTypeNode`, `getEffectiveSetAccessorTypeAnnotationNode`, `GetSetAccessorValueParameter`, checker.go20011/utilities.go1837 | `source/members.rs` plus one named syntax operation for setter parameter selection. |
| Optional type/missing removal | `addOptionalityEx`, `getOptionalType`, `removeMissingType`, checker.go18546/28849 | Model's whole optional factory, replacing the current narrower non-property helper; existing callers route through it. |
| Readonly/accessibility | `isReadonlySymbol`, `getDeclarationModifierFlagsFromSymbolEx`, checker.go13773/utilities.go694 | Complete canonical member metadata acquisition operation; actual canonical parent/symbol flags from binding, selected own syntax from Source. |
| Method anonymous type | `getTypeOfFuncClassEnumModuleWorker`, `resolveAnonymousTypeMembers`, checker.go16817/17609 | Same canonical value cache and actual structured inventory owner; canonical declaration signature factory reused. |
| Anonymous substitution | `getObjectTypeInstantiation`, `getOuterTypeParameters`, `isTypeParameterPossiblyReferenced`, `instantiateAnonymousType`, checker.go22220/23664/22348/22376 | One new deep anonymous-type operation within semantic evaluation, consumed by general `instantiate_type`, not a method-only signature copy. |

The related location read is a separate question. Native
`GetTypeOfSymbolAtLocation` removes missing and asks an annotated setter's write
type at its own declaration name. Product Q4 declaration selection deliberately
asks raw `getTypeOfSymbol`, including member read at either accessor anchor.
Tests must pin both operations and label that normalization.

## Source and binding boundary

Retain the existing `BoundMember` family/own roles. Add only canonical facts
required by native metadata that cannot be established from a declaration's
syntax: actual parent-is-class and relevant actual symbol/check flags. Prefer a
complete `Bindings::member_metadata` acquisition operation over forcing each
consumer to inspect Store state. Instantiated/synthetic symbol metadata stays
with its actual factory; unsupported late/assignment families remain located
failures until their complete native operations are implemented.

`Source::member_read_recipe` owns annotation-before-initializer precedence for
ordinary properties. It returns actual optional/isProperty facts and the same
variable-like annotation/initializer questions; optionality is applied by Model
after the selected dependency is consumed. Auto-accessors are not ordinary
`isProperty` declarations. Parameter-property/class constructor-flow and JS
branches retain explicit pending recipes until their complete operations exist.

`Source::accessor_recipe` projects the first actual getter, first setter and
first auto-accessor from the full native association list. It retains separate
annotations, actual bodies and locations without consuming them. Native setter
parameter choice is exactly: zero parameters => absent; exactly two with first
`this` => second; otherwise first. It is not a generic "skip this" heuristic.

Read dependency order is getter annotation, setter annotation, auto-accessor
annotation, actual getter body, auto-accessor widened variable-like type, then
genuine diagnostic-only Any. Write dependency order is setter annotation, then
auto-accessor annotation only if no setter; its own resolution pop precedes
fallback to the read type. Getter body inference routes the existing
`return_body_type` question directly, rather than adding a SignatureReturn frame
that TypeScript does not push. Contextual/body/global dependencies cannot be
guessed absent because getter syntax is simple.

`Source::canonical_property` returns property/method/accessor kind, optionality,
readonly/accessibility, exact key and complete own origins. Descriptor acquisition
does not force read or write values. It uses the newly shared final attachment
index; no AST walk or source-name map is added per member.

## Evaluation/cache boundary

Add a parent native `resolution::Property::WriteType` and canonical write cache;
the member increment wires its has-property callback coherently. Ordinary member write wrappers register
one demand per actual canonical symbol and delegate reentry to the same native
write owner, as read wrappers already delegate to `value_type`.

Accessor read owns native Type push/rejected-push/marked-pop/final-installed
property behavior inside the canonical parent cache. Rejected push consumes
ErrorType without installing. Marked pop installs diagnostic-independent Any
recovery as the product's original located InvalidSemantic; an installed
recovery is a native property barrier. The parent final cache commit must
preserve that installed Resolved/Recovered result rather than overwrite it with
ordinary Failed. Ordinary unsupported or resource failures are not barriers.
Write uses the separate WriteType frame/cache with the same distinctions and
its own annotation location. Every genuine circular or missing dependency needs
a final public test before claiming the recovery family complete.

`write_type(symbol, family)` ports the full actual dispatch: synthetic deferred,
synthetic established, ordinary Property missing removal, instantiated Accessor,
ordinary Accessor, then other-symbol raw read. Unimplemented synthetic/mapped
branches are distinct located Pending, not a wildcard raw-read fallback. The
instantiated write demand uses the original native property plus mapper, distinct
from its read demand; it reuses the generic type substitution operation.

Model introduces one private Missing identity with native Undefined flags. It
never aliases ordinary Undefined. `getOptionalType(t,isProperty)` selects missing
or undefined only through the captured effective options. `addOptionalityEx`
owns strict-null gating; `removeMissingType(t,isOptional)` owns exact-option
gating and actual identity removal, including union reconstruction. Every current
non-property optional caller routes through that complete owner. Loose+exact
TS5052 remains explicitly a diagnostic-policy configuration divergence; no
checker semantics are inferred from dismissing the diagnostic.

## Method/anonymous factory boundary

Ordinary methods create one canonical anonymous value type and lazy actual
overload inventory. Native optional methods apply property optionality to that
anonymous type. Signatures retain actual SignatureIds/canonical local parameters,
call types, predicates, returns, flags and native order from the existing factory.
Method symbols are not function declaration symbols for `typeof` serialization.

Generic member substitution cannot inspect the displayed callable structure or
instantiate all known signatures eagerly. Implement actual anonymous target,
declaration/outer parameter acquisition, conservative reference filtering,
effective mapper composition and native anonymous instantiation key before
admitting generic method objects. The factory installs a real deferred inventory
question on the resulting identity; inventory demand routes `instantiate_members`
and the existing signature mapper. Target+mapper alone is not the universal
cache key: native aliases and single-signature instantiation mode participate.
An unchanged effective outer argument vector returns the original target.
Unsupported contexts/inferred/mapped scopes remain failures until their native
outer-parameter acquisition is implemented. Synthetic-this participation cannot
be inferred from spelling or blindly copied from the incoming mapper sources.

## Presenter boundary and tests

Display is a separate native question from raw read. `getNonMissingTypeOfSymbol`
in `nodebuilderimpl.go::addPropertyToElementList` removes Missing before printing
an optional property. A method uses signature syntax and its nonmissing callable
type. The same anonymous-object printer consumes accessor write types to decide
whether to emit paired get/set clauses. Thus a healthy raw root and summary text
cannot prove display dependencies healthy: printing an inline accessor with an
unresolved setter adds TS2304 after the raw root is established. A nominal
interface summary does not invoke that anonymous member printer. Preserve a
distinct property display-read question and actual accessor write dependency;
do not eagerly consume them for nominal names or budget-excluded rows. Agent2's
fresh inline phase controls and final public tests will pin these decisions
before display admission. General anonymous method/body/context factories remain
part of this increment's full obligations.

Model property `write_type:Some(demand)` should represent an independent native
write question that may differ, not a promise that output contains `writeType`.
Readonly suppresses the writable operation. None retains the established
same-read question. After admission, compare actual TypeIds and emit writeType
only when different; strings/origins/serialized shapes are not identity. Shared
child reservations and rollback apply to a possible write field. The exact
lower-bound preview for an unknown comparison needs agent2's whole property
operation and boundary tests; do not mark possible field presence as known.

First final file `tests/type_member_values.rs` will cover direct getter/setter
anchors normalized to read; getter-only readonly, setter-only and paired distinct
types; annotated getter ignores invalid setter until an admitted write edge;
invalid getter read stays invalid; strict/loose × exact optional properties and
methods; ordinary/overloaded/generic method signatures and identities; generic
accessor read/write substitution; exact member kinds/origins/order/accessibility;
hidden invalid read/write children at depth/byte boundaries. Native method body,
auto-accessor initializer, parameter-property/class initialization, circular
read/write participants and aliases receive subsequent final tests through the
same owners, retaining full design obligations.

## Compiler controls and implementation proof

`tools/type-member-values-oracle.test.mjs` first passed six registered tests with zero
skips (`vrun-d3a2a0e1-037b-4f0c-aa13-8d03c3ff0d6b`). Five make fresh public pinned
compiler demands; one validates the separately archived private observations and
their exact captured inputs/tooling. The archive at
`fixtures/types/native-member-values/evidence/` contains 24 fresh same-checker
baseline sessions, raw/read-nonmissing/write native TypeIds, per-question
accumulated diagnostic stamps, and SHA256 provenance. Its answers hash is
`192bb4de979ad6753e3fb26374c5fb2e178161d274f3f2b577fc9eb899a1bec2`.
The copied generated tools retain their original absolute dependency paths; this
is a frozen research witness rather than a standalone replay bundle. Loose plus
exact optional inputs deliberately retain TS5052 in both preflight and native
Program observations; permitting only that diagnostic in the research adapter
does not prove configuration or semantic validity for any other combination.

Fresh controls establish optional Property and optional Method differences:
strict/exact Property raw read retains a distinct Missing identity while its
write removes it; Method raw write retains that same raw read identity. Both
location reads remove Missing. Distinct accessor writes and generic original-own
setter location versus instantiated member writes are separately pinned. The
formal suite and archive do not prove native Rust behavior, circular resolution,
inline display, resource bounds, or general anonymous substitution.

The canonical-table increment exposed a legacy lexical regression in the full
Rust checkpoint `vrun-ee7bae63-a442-418c-ad93-7d3bf1855381`: a class static
property became a lexical member target through the newly retained export table.
The shared `Linker::member` operation now preserves the documented lexical
boundary before returning an export-table member while keeping genuine merged
namespace exports searchable. Final `tests/reference_fixtures.rs` passed
`vrun-397ed53d-5954-4837-a054-bf84b87a5421`, one matched file and zero skips. It
executes the original fixtures and a Property/Method/getter/setter/auto-accessor/
escaped-key class plus namespace/import-equals matrix across all five ECMAScript
line breaks with and without a leading BOM. The BOM tests query independent
analyzed-template coordinates; the existing decodeBytes owner and fresh pinned
compiler both strip the raw encoding prefix. Initial coordinate mismatches were
test setup defects and do not count as product regressions. Mechanical boxing of
the private anonymous property annotation also compiled in that final gate.

The extended native suite passed seven tests with zero skips
(`vrun-393211b4-bb28-438a-8c6e-ac58093ae2f4`). Its added fresh controls establish
the complete setter-value-parameter choice: absent for zero parameters, the
second only for exactly two with first `this`, otherwise the first. The new final
file `tests/type_member_values.rs` pins eight nominal/raw families. Its seven-test
assertion RED was `vrun-e042e8f3-46f9-4405-8ec5-b816bd28f253`; the extended
eight-test assertion RED was `vrun-82086309-baab-4bc4-916a-3fd527147727`, with
all eight failures at the expected pending member/optional factories and zero
skips. Coupled Source/DTO/Model/cache/presenter implementation follows those
registered behavioral failures. Run established
member/mapper/Array/presentation/cycle compatibility and private final-Project
retention tests at the next full coherent checkpoint. Canonical table, demand,
anonymous target and signature storage costs remain part of the open resource
gate.

The ordinary Parameter prerequisite shares the existing native variable family
and `getTypeForVariableLikeDeclaration` source recipe with signature parameters.
Switch exhaustiveness previously failed before it could consume an annotated
parameter; final Switch7 `vrun-a76f05d2-3297-433e-9151-8a629ff5f183` passed
after that prerequisite. A separate final identity test then exposed duplicate
parameter construction: signature raw read and the ordinary symbol read allocated
different anonymous object TypeIds (`vrun-7c294682-d138-4dd0-9676-ec91616bc4d5`).
Signature acquisition now retains the exact own declaration anchor, asks the
existing canonical binder value family, and registers the lazy canonical symbol
read question. It does not check parameter annotations during factory creation.
`tests/type_parameter_reads.rs` passed
`vrun-f691b62e-69e6-44a8-bb98-2582c5e477f3`, one matched file and zero skips,
pinning the shared object identity and optional/initialized raw-read versus
call/display behavior. Three fresh native controls passed in the single registered
oracle test `vrun-56727623-8eac-422a-a871-bb6421179de4`. Synthetic `this` keeps
its distinct existing question; contextual/setter/binding-pattern inference and
default-parameter flow setup remain required pending families.

### Nominal checkpoint and canonical inline continuation

The eight nominal tests passed in
`vrun-c90be8fb-4a6f-431f-b42a-106c90537c75` (one matched file, zero skips).
An intermediate compiled checkpoint `vrun-0941a19f-3442-4d03-8b9c-1da4c9837220`
exposed the optional-method arrow/union precedence defect. Presentation now uses
the single native emitted-type-precedence owner; final nominal proof includes
`((value: number) => string) | undefined`. Two other mismatches in that run were
incorrect test premises: loose plus exact optional is rejected by the pinned
TS5052 configuration owner, and whole-field depth omission retains no exact
remaining count. Their corrected assertions preserve those existing contracts.

The next inline increment removes Source's property-name/count reconstruction,
its source-root member identity registry, and its separate annotation-only
selection route. A retained `BoundTypeLiteral` associates the actual node with
one native anonymous symbol and that symbol's existing Store member table.
`TypeExpressionForm::TypeLiteral` captures only the final own token and alias
metadata; evaluation owns native allocation and node-link caching. Mandatory
`getMembersOfSymbol` table acquisition precedes the factory's empty branch,
including precise pending late-binding dependencies. Source projects the same
canonical member symbols as class/interface members, preserving independent
raw reads, writes and native serialization questions.

Fresh compiler controls in `tools/type-member-display-oracle.test.mjs` passed
five tests with zero skips in `vrun-86fc4edf-52ca-4807-89d3-66ebfa6b7821`.
The added same-checker identity control proves that two unaliased empty literals
share `emptyTypeLiteralType`, while an empty alias has a distinct real anonymous
identity reused by its own return annotation. The model therefore owns a distinct
empty-type-literal cache; it does not reuse `emptyObjectType`. The original inline
four-test assertion RED remains `vrun-e6b78d99-9c62-4f7f-a39c-fa53b78df496`.
The fifth final identity test is a new compiler-backed control under that
increment, not a separately collected assertion RED.

Private retention proof is tested after final `Project::load` for empty versus
reserved-signature tables, distinct anonymous owners, exact decoded/raw escaped
property names, accessor-pair shared symbols and the unchanged lexical alias
question. The single integration-file gate does not execute that library test;
the full checkpoint `vrun-1ae7a98d-1f41-45eb-8ab1-1ed2901aa95e` executed
`retained_type_literal_symbols_own_exact_tables_after_final_project_load`
successfully. Its library phase passed 111 tests with one existing ignored
test; the overall run failed later in a separate flow integration test.

This increment covers the TypeLiteral branch of
`getTypeFromTypeLiteralOrFunctionOrConstructorTypeNode`. FunctionType and
ConstructorType still use the earlier captured structured-signature path. Their
retained internal anonymous node-symbol and general node-link factory migration
remain required through this same native owner. Generic/contextual TypeLiteral
outer parameters, complete alias/single-signature instantiation keys, late-bound
members, circular member participants and representation/resource gates remain
open; none is replaced by a guessed empty table or inferred binding name.

The first compiled inline checkpoint
`vrun-c3df8ea8-8689-40c7-98ee-d3785122be4d` passed four tests, including the
empty-identity control, and rejected the bad setter with the correct
`InvalidSemantic`/`UnresolvedSymbol` cause. The test had expected generic
`ErrorType`, confusing the compiler's recovery identity with the product's more
specific original unresolved-reference cause. Its corrected assertion requires
that existing reason and the exact `Missing` path/position; production is
unchanged. That result is not a product-fix RED. An earlier retry
`vrun-9fb471d9-3c44-4e11-b19c-b249ab47a652` failed compilation in the concurrent
narrowing scaffold and has no behavioral verdict.

The corrected final inline file passed in
`vrun-3488e91f-6cd6-438b-8789-007de8ffb86d` (one explicit file matched, zero
skips). Five tests cover strict/plain, strict/exact and loose states: optional
property nonmissing summary, optional method syntax, paired accessor clauses,
mandatory invalid setter serialization dependencies at summary-only/depth-zero/
expanded queries, and native empty/alias type identity. The subsequent broad
checkpoint executed the private final-load association test successfully, as
recorded above. It did not produce an overall compatibility GREEN. In that retry pair,
`vrun-9fb471d9-3c44-4e11-b19c-b249ab47a652` was setup-only;
`vrun-c3df8ea8-8689-40c7-98ee-d3785122be4d` compiled and exposed only the
incorrect reason assertion described above.
