# Ordinary reference graph presentation

The operating envelope is one captured Project and request-local Evaluator,
with actual target/reference TypeIds, canonical mapper identities and deferred
whole member inventories. Presentation owns bounded graph vocabulary and BFS;
it receives complete semantic views and never classifies callability from
nominal Class/Interface names or source flags.

Designing twice exposes a necessary semantic boundary. A syntactic callable
header is inexpensive but loses inherited, merged and instantiated signatures
and moves native policy outside its owner. A second signature-only member
resolver appears lazy but answers a different question: TypeScript 7.0.2
`getSignaturesOfType` calls `resolveStructuredTypeMembers`, which consumes the
whole declared/instantiated/base inventory, including index annotations.
The chosen operation reuses the existing deep Evaluator `members(MembersOfType)`
and its canonical resolved/failed cache. The presenter classifies Function from
actual call or construct collections and Object otherwise.

Required graph-kind metadata runs at depth zero. A nominal summary-only
reference can avoid inventory; an expanded real root kind cannot. Ordinary
property/signature child types remain lazy, but an actual index recovery
consumed by required inventory fails the operation even if the nominal root is
non-error. Arrays retain their separate actual-global-target classifier and
can defer their inventory because their real kind is already established.
The design records this distinction explicitly.

Fresh native two-test GREEN
`vrun-144ec43b-dbda-47a5-b74c-627b35bd167c` covers ordinary object,
callable and constructable generic references, canonical substitution,
property-lazy versus index-consumed dependencies and strict/loose profiles.
Six same-checker captures preserve the exact demand sequence, accumulated
diagnostic snapshots and actual index-value error predicate independently of
summary-only roots. Their archived generator/input/tool hashes live under
`fixtures/types/reference-shape/evidence`; diagnostics alone do not supply
the recovery proof.

The first final Project attempt `vrun-2d4e9476-163c-4eb9-b585-2531baa85775`
had two wrong test DTO field names and is setup-only. Corrected actual
`vrun-78892af5-a982-4b8c-ad33-d4e05cdcd931` compiled 28 tests and failed
the three new final assertions at the old unlocated reference-kind placeholder.
The presenter now routes real shape through the native inventory, uses known
skeletal collections and reuses ordinary object/signature/index BFS admission.
Generic callable/constructable mapper completion remains a required sibling
semantic dependency; graph code cannot replace it with a guessed descriptor.

A subsequent final test establishes a Host alias and admits a nested reference
whose required inventory encounters an unsupported conditional annotation.
Actual `vrun-20ff021a-9d8c-4f98-83ed-1afae2856644` exposed that failure
erasing the established parent. Its fix retains a separate graph failure
identity for the exact `MembersOfType` question, alongside deferred child
question failures. It never caches a failed description as a semantic TypeId.
Unsupported encounters preserve the located issue and Partial through display
rollback; invalid/resource/internal dependencies remain whole errors. Required
root-kind failure stays an atomic root failure. The same run corrected a test
assumption about depth-zero omission counts: absent collections use the
existing fields/Depth vocabulary, while counts describe displayed prefixes.
That expectation correction is not a product defect.

The coupled failure-identity change initially missed two preview constructors;
`vrun-eaa62af0-69c1-44fc-8cb4-60518373379e` is compile/setup evidence only.
They now retain the explicit Child question variant. After actual generic
signature mapping was implemented and separately proved through its own Project
tests, all 29 presentation tests pass with zero skips in
`vrun-88afc44f-66b0-4993-90f6-10de14f17017`. The run proves the current
ordinary object/callable/constructable reference assertions, nested inventory
failure provenance and the exact blocked-array inventory byte controls. It
does not promote every generic or broader graph kind to supported semantics.

Full generic signature mapping, ordinary/class/merged/inherited inventories,
all graph kinds and origins/write/predicate metadata, every broader byte/node
boundary, semantic failure causality and representation/resource/corpus gates
remain mandatory. A current precise capability failure is not a permanent
scope waiver.
