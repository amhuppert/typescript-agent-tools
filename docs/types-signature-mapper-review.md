# Bounded signature mapper review

This read-only Ousterhout review covers the first native signature
instantiation owner in `evaluation/generics.rs`, actual return-property
associations in `evaluation/signatures.rs` and the Model factory/clone
protocol. The operating envelope is one request-local evaluator over an
accepted immutable Project. It does not certify native relation selection,
generic call inference, erased signatures, anonymous structured mapping or
the full mapper algebra.

No concrete incorrect successful answer was found in the admitted slice.
All fresh local parameter identities are allocated before mapper composition;
old-to-fresh substitution precedes the outer mapper. A changed result from
the first mapper is recursively instantiated with the second. This retains
native composition rather than flattening two related lookups. Incomplete
parameter flags permit this native header-only substitution; immutable
descriptions remain unavailable until completion, and failed completion
poisons every actual early identity with the original cause.

Cloned raw constraints consume the original parameter's guarded constraint
question, then instantiate it with the combined mapper. They do not copy the
original raw annotation handle. Cloned guarded/base work still belongs to
the complete constraint owner. Defaults remain separate lazy questions;
absence is proved from the original parameter's complete association list,
and a missing demanded default retains its own error. Native cloneTypeParameter
creates an ordinary new parameter, so copying synthetic-this identity would
be incorrect; the Model explicitly retains false for the fresh clone.

`instantiateSignatureEx` allocates a new actual SignatureId on every
invocation, as pinned `checker.go:20524` does. Repeated inventory identity
comes from the resolved MembersOfType result, not a target/mapper/erase
signature interner. The new signature keeps original origins separately from
identity, preserves native propagating flags and minimum argument count,
and retains distinct read/display/call parameter demands. Known index-key
identity is retained while only its established value is substituted.

Return-property questions are associated with actual SignatureId, not source
SignatureRoot or the displayed origins. Original and instantiated signatures
therefore have separate lazy return questions and resolution frames. The
reverse demand association admits re-entry through the native property
operation rather than a generic resolving-demand fallback. The native final
property check after pop reuses a nested installed return or recovered cause;
it does not overwrite that property with the outer computation. Product
Failed state remains a failure without becoming an installed native property.

The registered four-test final mapper gate passed with zero skips in
`vrun-3c3407fa-413c-4fd6-b2d1-ef2aa0a4e4ee`, following the actual
`vrun-75d1f401-6c3c-4714-bad5-f48fc82581a0` assertion RED. The expanded
five-test gate passed in `vrun-450f464c-437e-4d5e-a6a8-faae10729054`,
also with zero skips; fresh native collection-order controls passed in
`vrun-e98f5506`. Actual recursive signature tests independently passed both
cycle/provenance controls in
`vrun-267245da-48b7-4fec-95b2-4a1730761ccc`, while that file still failed
its separately identified recursive function summary spelling control.

Future known child slots require particular care: instantiating a Known
property demand eagerly is harmless for admitted established primitive
families, but does not by itself prove native lazy symbolic/object child
behavior. Current canonical named property recipes are Deferred. Actual
Object/other unsupported mapper families must remain failures until their
whole native operation and public final outcomes are implemented. These
limits, native erasure/mapper families and full resource accounting remain
mandatory work rather than a reduced delivery scope.
