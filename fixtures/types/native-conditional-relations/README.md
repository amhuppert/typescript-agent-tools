# Conditional relation and inference controls

These final queries target the callers of native conditional factories:
normalization, conditional source/target comparison and conditional inference.
Every cold hook query is compared with the pinned 7.0.2 public API. The existing
opt-in expression observer records only already-returned native error types.

The five recovery controls all return non-error final types, but consume missing
types in native order. In particular, `targetTrue` still reads both branch types
during normalization; a later always-true relation gate does not undo that work.
The product preserves those consumed recoveries rather than presenting the final
type as healthy. Native acquisition totals do not define product cause counts.

The created alias controls combine a class's late static property with a
namespace's generic type alias. An independent cold API inspection verifies the
actual symbol has Transient, Property and TypeAlias flags and declarations in
native order. Overload comparison returns 41 and generic inference returns 31,
matching the ordinary bound alias controls. Symbol IDs are omitted from evidence.

All eleven roots matched in strict, loose and noCheck during bootstrap
`vrun-d1730e23-d60a-481f-b5ce-230e1ae3d0ab`; only the absent baseline files caused
that bootstrap run to fail. Expected baselines are captured native observations,
not current product output.

Maintained replay passed all 33 queries and actual-alias observations:
`vrun-96c94879-d65c-482e-9e40-65c0db48a59e`.
