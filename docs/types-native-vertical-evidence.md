# Native vertical implementation evidence

The native Rust `Project::type_of` operation now constructs a bounded first
family behind the complete designed TypeRequest/TypeAnswer contract. The
production path uses the retained canonical binder and captured source text;
the pinned compiler is used only for validation. Full native T/Q3/Q4 delivery
and all remaining semantic/resource/representation gates remain pending.

## Established behavior

The first source consumer follows ordinary named imports, renamed barrel
exports and imports, and type-only inspection of a value-only variable. A
variable initialized by an ordinary identifier-keyed object produces ordered
mutable properties with widened primitive types and exact own origins. Const
primitive variables retain fresh literals; mutable primitive variables widen.
An external dependency's member origins remain outside Searched. Changing the
selected file and dependency on disk after load leaves the captured answer
unchanged.

Variable annotations consume their declared types before initializer demand,
including absent initializers, incompatible initializers and unresolved names
in an initializer. Nongeneric type aliases consume their canonical retained
type-reference associations. Selected literal tokens ask an expression
question; literals in LiteralType nodes ask a type question and consume regular
literal identity rather than initializer widening.

Required ordinary string/number property signatures construct ordered type
literal members with readonly flags and exact own origins. Structural aliases
retain their own alias name/origin. A single nongeneric interface without
heritage produces its own nominal display name and no type alias metadata.
Renamed imports preserve these distinctions. Own required property-name tokens
ask a value question without forcing another member's type. Unrelated
interfaces with the same decoded key remain distinct candidates, and escaped
keys preserve decoded identity plus the exact raw source anchor for replay.

The evaluator now owns captured syntax and its shared parent table, canonical
value and declared caches, and distinct member/read-demand identities. Named
interfaces establish their roots without forcing required member annotations.
Summary-only and depth-zero roots can therefore remain Found; requested
unsupported valid members become located Partial leaves. Consumed unresolved
member recovery stays InvalidSemantic. Repeated unsupported alias demands keep
the original exact failed result instead of becoming a circular error.

Own property names with unpaired UTF-16 identities preserve their raw supported
value role until the enclosing-call prelude runs. Outside a call, consuming the
exact name reports UnrepresentableIdentity; inside a call the native pending
signature operation precedes that demand. Paired, cooked, numeric and escaped
identifier names follow the same prelude schedule without replacement decoding.

Expanded ordinary recursive interfaces point back to their established root.
Repeated references to one named interface share its type identity, while two
distinct named interfaces with equal property shapes retain distinct roots and
share their primitive child. These controls use same-checker native identities,
not summary equality, and passed the existing canonical caches without a fix.

The semantic worker owns request-local model/cache/parser authority and catches
ordinary panics inside that worker. This does not establish stack-overflow
recovery, execution headroom, work/deadline calibration, service responsiveness,
or the complete resource profile. The current captured-source reprojection is
provisional; eager-versus-reprojection measurement with this actual native
consumer remains due.

## Registered compiler and native tests

All listed runs matched one explicit file and reported zero skips. Expected
behavior comes from TypeScript 7.0.2, source commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`, with a fresh isolated checker and
baseline prelude. Whole-program diagnostics are separate observations.

| Proof | Durable registered run |
| --- | --- |
| Cross-file ordinary value and own member controls, four compiler tests | `vrun-77bcd0cc-1282-441f-9c27-7cf86930f982` |
| Initial native semantic stub fails the object assertion | `vrun-e5569f40-d37b-4167-9a4d-fa6d2e79dc27` |
| First three native value/origin/captured-text tests pass | `vrun-1486e16a-7025-432c-bfd0-ee9dce42c1a2` |
| Fresh declared annotation/type-reference/expression controls, three compiler tests | `vrun-bf64fbdd-d70d-45bc-a74f-1c702815b7bf` |
| New native annotations/expression assertions fail on missing behavior | `vrun-d158fd9d-8ee1-4d66-b6c0-a7efbcae57fe` |
| Six native tests pass after annotation/type selection | `vrun-1420b3aa-ace6-47a5-aa0b-6729cf8fe13d` |
| Fresh literal/plain-structure controls, three compiler tests | `vrun-8c828b95-a245-4176-ab2b-6a19020f75df` |
| Native LiteralType assertion fails with pending type-node construction | `vrun-93b1898c-6f92-48f8-a095-214d00f83f33` |
| Seven native tests pass after regular LiteralType construction | `vrun-a022e2ba-4a15-401e-bc33-29f0e0b2911d` |
| Native type literal and interface assertions fail with pending construction/selection | `vrun-0621eea2-05aa-48ea-9146-9ca8d02f9c91` |
| Nine native tests pass after required structured properties and interface construction | `vrun-d04743e4-12e8-46e6-8edf-3d70a3dddfb0` |
| Fresh own property value questions and distinct raw roots, four compiler tests | `vrun-47b34066-fbb1-43b8-96f7-6e5434330ad1` |
| Native property-name assertion fails at its missing value operation | `vrun-0f9f07c9-541e-4623-b43d-8bc97267295f` |
| Ten native tests pass, including escaped own keys and distinct candidate replay | `vrun-d91afce1-3876-4197-a187-9d884cf942d0` |
| Mechanical evaluator ownership and source parent cache preserve ten native tests | `vrun-2b855de2-f9a2-4b8c-8966-797826df6467` |
| Fresh lazy root/member/repeated alias controls, three compiler tests | `vrun-59036dfd-45a6-4e85-84b1-07c02025751e` |
| Three native lazy assertions fail on eager primitive-only root construction | `vrun-215b9d2a-45a2-44e6-994e-bad1d4ec1646` |
| Four native lazy tests pass, including original repeated failure provenance | `vrun-b0c257f8-96ce-475d-bb26-e9fbfe1b3946` |
| Fresh property-name identity and call-prelude controls, two compiler tests | `vrun-f7a1a00a-b192-4ac4-9030-b8c7360c8f04` |
| Native inside-call unpaired-name assertion fails on premature identity demand | `vrun-ba883aa3-4029-449c-911c-289453c466cf` |
| Two native property-name tests pass after deferred exact-identity demand | `vrun-22c43c2a-c9b7-4897-8670-36db73d33e1c` |
| Fresh recursive/shared/distinct-nominal identity controls, three compiler tests | `vrun-cbb95ec7-b5c5-4256-8e57-d640a7ce612a` |
| Two native recursive/shared identity coverage tests pass without a behavior change | `vrun-b27071d5-be1f-4d02-ad74-378c6bbbef61` |

The fixture/control hashes are recorded in
`.cc/temp/types-native-vertical/fixture-manifest.json`. The manifest's SHA-256 is
`91d5bdfd39345fa51ef65d6ea54710d2652370e7bebee5d3c805fd7528347f7a`.
Its entries cover fixed compiler-control inputs rather than mutable concurrent
production source; final implementation hashes belong to the delivery checkpoint.
The six fixture roots
are `native-vertical`, `native-annotations`, `native-type-nodes` and
`native-lazy-types`, `native-pending-values` and `native-recursive-types`; their
compiler controls are the corresponding `tools/type-native-*-oracle.test.mjs`
files. Native tests assert final `Project::type_of` results in
`tests/type_vertical.rs`, `tests/type_lazy.rs`, `tests/type_pending_values.rs`
and `tests/type_recursive.rs`. JSON, broader literal constants, presentation budgets
and CLI/service tests are separately owned evidence and are not implied by
this table.

## Limits and next work

The complete designed selection/token table, meaning/flow admission, circular
recovery, merged/generic/heritage declaration construction, exact literal and
computed keys, optional/read/write members, methods/signatures/indexes,
contextual and broader recursive types, full union/array algebra and native Q3
relationships remain required. Unvisited dependencies remain explicitly
NotDemanded and are never resolved by a second binder or guessed as missing.
Required identifier-keyed property annotations are now captured as lazy
recipes. Their richer construction families remain pending and can produce
located unsupported leaves when consumed; unresolved/invalid dependencies
remain whole-operation errors. Known-empty collections are emitted only when
every admitted member was checked as an ordinary property signature. Node/byte
rollback around encountered partial failures needs its independent boundary
proof; the narrow lazy tests do not establish every display-limit combination.

The binding facade now reads typed own declaration associations from the
canonical Store symbol in native bind/merge order; the JSON global-cause
regression required this shared owner, rather than a captured-file scan.
Canonical value and declared results remain cached by the native evaluator.
The retained association payload has a storage cost and does not close the
representation or large ambiguity-demand resource gates. Member identity must
remain separate from public origins: a request-local MemberId and canonical
root identity can diverge for merged, mapped and transient symbols. Remaining
gates cannot be inferred from successful summary strings or this small family.
