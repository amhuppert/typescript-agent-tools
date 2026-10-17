# Native keyof owner

The request-local evaluator owns `getIndexTypeEx`, its shared
`shouldDeferIndexType` decision, and `getLiteralTypeFromProperties` in
`evaluation/keyof.rs`. Model owns actual Index allocation and the generic
Index cache; Source retains the actual keyof node and operand. This fits the
existing immutable Project/request-local checker envelope.

A concrete-only key collector would force indexed-access validation,
instantiation and syntax callers to reproduce deferral and origin policy.
The shared native operation instead reduces the operand, asks deferral, maps
union/intersection constituents in order, and acquires ordinary property and
index inventories through their existing complete operations. No caller
reconstructs property names, accessibility or synthetic symbol identity.

The property-types key includes actual TypeId, include flags, includeOrigin,
and UnresolvedMembers captured before member resolution. A fresh Index origin
is allocated before either inventory. The native alias branch deliberately
applies even when includeOrigin is false. Property keys precede index keys;
string index keys include the canonical stringOrNumberType when requested.
Both type-node and property collection failures remain request-local failures.

Generic Index caching distinguishes only StringsOnly, exactly as native
26719; other IndexFlags affect the question, not the resulting cache identity.
NoReducibleCheck suppresses only generic reducible-union deferral. The same
predicate serves indexed-access simplification. NoInfer, mapped, wildcard and
enum-index sentinel factories are broader producer obligations: mapped and
substitution acquisition currently fails explicitly; no current enum factory
can manufacture enumNumberIndexInfo. Its future factory must attach actual
sentinel identity, never infer it from a number key. These dependencies remain
part of the required profile and this stage is not a claim of full delivery.

Maintained controls in `tools/type-keyof-oracle.test.mjs` cover 25 final
questions in strict, loose-null and exact-optional profiles, plus public actual
union constituents and consumed operand/index-info errorType observations.
`vrun-95c49daa-97c1-4119-8a91-12dcf133c7d6` passed all four harness tests with
zero skips. Class/private and aliased/interface cases preserve the displayed
keyof origin while exposing actual ordered literal alternatives. A broken
property annotation stays unconsumed; a broken index value is consumed by
getIndexInfosOfIndexSymbol before its key is checked, even though the final
native keyof summary is healthy. The product final test preserves that failure.

`tests/type_keyof.rs` asserts public summaries, both expansion modes, actual
expanded alternatives, and consumed failures. Its coordinated Rust run and
counterfactual restoring the old unsupported keyof branch remain pending.

## Adversarial checkpoint finding: global array recovery

`getKnownKeysOfTupleType` (relater.go1901) reads the retained global Array or
ReadonlyArray type directly, then asks getIndexType. The current shared
array_target operation instead consumes global recovery as a failure. These
are different native questions. Native getGlobalType(arity != 0) returns the
distinct emptyGenericType singleton on missing/wrong-kind/wrong-arity globals;
its native keyof can be Never and its native fixed-key union can remain healthy.
That observation alone does not establish product admission.

The approved contract in `types-design.md` (invalidity follows data
dependencies) and `types-array-globals-review.md` rejects consumed factory
recovery even when native returns a non-error emptyObjectType. Existing array
and tuple constructor failures must retain the original missing/kind/arity
cause. Identity-only comparisons can observe a real sentinel without consuming
it as an exact type; structural inventory/key acquisition can consume recovery.
Do not infer admission merely from a healthy final native summary.

The next stage must establish the shared emptyGenericType identity and exact
operation-specific provenance boundary, preserving ReadonlyArray's fallback to
Array. Pin native and public final controls for valid, missing, wrong-kind and
wrong-arity Array, plus independent ReadonlyArray fallback. Distinguish skipped
or identity-only global questions from known-keys inventory acquisition and
from createTypeFromGenericGlobalType/createTupleTypeEx factory recovery.
Neither an emptyObjectType substitute nor broadly allowing healthy Array
fallback is authorized. The known-keys recovery-consumption proof remains
open; the first compile checkpoint preserves its explicit failure boundary.
