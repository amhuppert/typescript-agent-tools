# Type algebra and inference compiler controls

2026-10-10. These are bounded TypeScript 7.0.2 public API observations, one fresh
checker per request, using the selected Q4 question and enclosing-call prelude.
They supply expected final types for later native implementation. They do not
implement T, close the semantic baseline gate, or establish graph expansion,
dependency admission, recovery causality or native resource behavior.

The nine registered tests in
[type-algebra-oracle.test.mjs](../tools/type-algebra-oracle.test.mjs) passed in
`vrun-ff06f5ec-3ab5-43c8-a2ec-96a3d054ca91`: one matched file, nine executed,
nine passed, zero skipped. Fixture/expected-result additions have no new
production behavior to implement; no native red-green cycle is claimed.

## Observed distinctions

| Family | Pinned final answer |
| --- | --- |
| `keyof` and indexed access | Exact string-literal key union; selected numeric property type. |
| Generic conditional and distribution | Unsubstituted `Distributed<T>` stays symbolic; `'s' \| 1` distributes into two objects; `unknown` selects the false branch. |
| Conditional inference | A function return is inferred as `number`. |
| Mapped modifiers and key remapping | Removing optional/readonly modifiers yields `first: string`, `second: number`; remapping yields `getFirst`/`getSecond` callable members. |
| Template and intrinsics | Four ordered template-product literals; Uppercase, Lowercase, Capitalize, Uncapitalize and nested Awaited reductions. |
| `BuiltinIteratorReturn` | `undefined` under effective strict options; genuine `any` with strict false. |
| Tuples and explicit receiver | Readonly, named optional/rest and variadic ordering survive; `this: Date` stays in the member signature. |
| Inference and freshness | Generic factory returns callable/readonly tuple members; contextual callback produces an object array; fresh object values widen while `as const` retains readonly literal members. |
| Recursion and growth | Optional recursive child is `Recursive<string> \| undefined`; growing child is `Growing<string[]>`, a different instantiated type. |
| Unique symbol and enum | A computed unique-symbol property retains its exact declaration origin; enum member presentation retains `Stage.Next`. |
| Validity | Genuine any and unknown are non-error roots. Unresolved alias `Unresolved` is an error root without a semantic diagnostics pass. |

Unique-symbol property display names contain an internal compiler ID, observed
as `__@symbol@2` in this capture. The test asserts the exact computed declaration
origin and property type, never that ID as canonical identity. The current
oracle exposes summaries and selected member types, not a complete typed graph
or compiler construction order. Symbolic summaries alone do not prove deferred
type edges or complete alias identity. The enum summary does not prove the
auto-incremented numeric constant; that requires a constant-value assertion.

The positive NoInfer call uses `hinted(1, 1)` and returns literal `1`. A separate
`hinted(1, 2)` request also returns `1` with a non-error root; requesting semantic
diagnostics after that demand reports TS2345 on the second argument. That is an
ordinary error control, not proof that the selected root consumed recovery,
nor a rule for invalidating unrelated demands. Product admission and precise
call/recovery provenance remain required. The unresolved declaration elsewhere
in the same source likewise does not invalidate independent fresh requests.

## Provenance and remaining proof

[Raw observations](../fixtures/types/type-algebra/observations.json) preserve
35 independent requests: the 33 primary markers, the strict-false hook control,
and the negative NoInfer diagnostic control. Diagnostic collection is absent
unless explicitly requested, and is then performed after the selected demand.
[Provenance](../fixtures/types/type-algebra/provenance.json) hashes the exact local
fixtures, historical shared tooling, executable assertions, pinned API package
metadata and native binary. The historical probe keeps its scratch paths; it is
an archive, not a second shipping oracle owner.

This capture does not hash every resolver metadata file the public API might
read and is not a complete frozen program-input proof. It makes no claims about
properties of an entire union from the oracle's object-only property projection,
write types, modifier flags, expanded signatures, type argument/tuple edges,
cycles at unequal depths or graph/display limits. Those require separate final
result assertions and the native Project/Session integration.
