# Native union reduction owner

This stage retains the immutable Project and request-local evaluator envelope.
TypeScript Go 7.0.2 commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`
defines `getUnionTypeEx`, `getUnionTypeWorker`, `removeSubtypes`, and
`isTypeDerivedFrom`. Inferred composite returns exposed the missing operation:
the maintained `defaults` query compiled and failed at structured subtype
reduction in `vrun-faaa88da-d3d4-4a3e-aa51-19417a2bc9b6`.

The old model-only factory could reduce primitives, while evaluator callers
handled constrained variables through a separate retry. Return inference, JSON,
iteration and flow still called the model directly and bypassed that protocol.
The evaluator now owns the complete union operation and every semantic caller
uses it. Model retains ordered normalization, metadata and final type identity.
Its typed dependency requests suspend at each constraint or subtype question;
the evaluator supplies actual answers before resuming. A dependency request
escaping that boundary is an internal failure, never an unsupported answer or
an unreduced successful union.

Moving the entire factory into evaluation would expose its canonical identity,
alias and origin storage or add several field-level adapters. Keeping semantic
callbacks inside Model would couple its basic factories to checker state and
make recursive borrowing indirect. The chosen continuation remains private to
one evaluator operation; source, flow and signature callers see one ordinary
type constructor. Only established metadata is replayed on resumption. Semantic
requests and their failures are retained by their actual owners.

`addTypesToUnion` now walks each source list forward, skips adjacent identity
duplicates, and inserts using the shared comparator before reductions. Numeric
allocation-ID sorting at that earlier stage changed the subtype/key-property
demand order. `addNamedUnions` separately walks forward and descends only union
origins; named origins are inserted in native order. Reference aliases now
participate in `compareTypeNames`, including deferred tuple aliases.

The two-input union-of-union cache uses the unordered actual ID pair, reduction,
and actual alias key only when no explicit origin is supplied. This cache is
distinct from final union identity and subtype reduction: final identity alone
cannot prevent repeated origin allocation before its lookup. Acquired failures
remain request-local failures in each cache.

`removeSubtypes` preserves the original ordered list/cache key and original
length. It resolves the empty-object question first; scans sources backwards;
handles union-constrained parameters against the union of other constituents;
then acquires the first unit-valued property and compares targets in order.
Native work estimation, distinct empty sentinels, strict subtype comparison and
the additional class-inheritance gate remain separate questions. Symbol-less
legacy object/JSON properties reach the shared explicit property-identity
dependency, not an internal invariant or fabricated symbol. Their complete
property producers remain required work.

`isTypeDerivedFrom` owns the hierarchy/global exceptions. `isFunctionObjectType`
now shares its whole operation with facts: evolving arrays, resolved signatures,
the raw bind table, then actual global Function subtype demand. An installed nil
raw table remains empty, distinct from a synthetic object's model-owned table.
Literal-base union conversion uses the existing mapType owner so native union
origins keep their traversal and identity behavior.

Ousterhout/adversarial review changed the pair cache, raw bind lookup, reference
alias comparison, source-order insertion, named origins, literal-base mapping
and missing property-symbol failure. Fresh strict/loose compiler controls for
nine ordinary reduction cases, including deferred tuple aliases, pass in
`vrun-f284e446-47ef-44fa-af69-8980c2491524` (two profiles, no skips). The initial
probe corrected primitive display order to the compiler's `"x" | 1`. Final controls are in `tests/type_union_subtypes.rs`; this source stage
has not yet passed its coordinated Rust checkpoint.

Remaining proof includes the native subtype work-estimate boundary, pair-cache
allocation/demand counterfactuals, invalid later unit-property dependencies,
and final recovery controls for global array identity versus consumed factory
or inventory recovery. Broader mapped/string/enum producers remain required;
their absent operations are not treated as successful subtype facts.
