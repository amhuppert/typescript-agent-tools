# Array presentation demand controls

`merged/` and `invalid-index/` retain four fresh TypeScript 7.0.2 public API
sessions, crossed with strict and loose null profiles. Both actual Array roots
are healthy `number[]` before member inventory. Native merged index enumeration
is healthy; the unresolved index annotation instead yields a child error type
while the Array root itself remains non-error. The probe does not run a
whole-file semantic diagnostic pass.

The executable compiler gate is `tools/type-presentation-array-oracle.test.mjs`:
`vrun-b7d9a6e0-ebfa-43b3-be26-c2ff4c23ab2f`, two passed, zero skipped.
Historical probe code, observations, exact fixture/test/tooling hashes, native
binary hash and the 416-file installed API inventory are under `evidence/`.
Reproduce the historical probe at its original
`.cc/temp/type-presentation-array-probe.mjs` path from the worktree root with
the pinned compiler environment variables; its output path is historical too.

The final public Project controls in `tests/type_presentation.rs` prove healthy
summary-only and depth-zero demand avoidance, partial blocked merged inventory
with no guessed counts, and whole failure for consumed unresolved index
recovery. Their 21-test GREEN is
`vrun-8456f5c2-8a3d-4571-8227-3af050225c0f`, with zero skips. An earlier
20-pass/one-fail run only corrected the expected fixture token column from 38
to its exact one-based column 39; it was not a production-behavior RED.

Fourteen later fresh native sessions pin the valid shallow union sizes and
large exact property keys used by final graph limits. Their historical
generator, observations and 56 exact generated-input hashes are under
`evidence/limit-*`. Final Project tests cover 1999/2000/2001 potential graph
records, depths one/two/three, and complete compact type fields of
262143/262144/262145 bytes. Encountered unsupported property issues remain
Partial after the property display rolls back; rejected oversized metadata
avoids its invalid child question. The 24-test GREEN is
`vrun-9fd766b4-5dc1-4d78-a35b-a2092c747297`.

Six additional fresh native controls establish healthy large literal-argument
Array roots with empty merged native inventories. `blocked-limit-*` freezes
their original generator, observations and 24 generated-input hashes. The
25-test final Project GREEN is
`vrun-1cf7b368-18f3-4ed7-9d4a-57e37ff5b4c4`, with zero skips. Its
262143/262144/262145-byte controls preserve the original inventory issue and
Partial result when the four blocked-collection display explanations exceed
the limit. The new valid boundary tests passed first; they introduce no
production RED claim. One earlier unrelated compile-staging failure is setup
evidence only.

These controls do not prove full native merged inventory, ordinary generic
reference callable shape, tuple/default-library members, outer result/issue
byte admission or representation/resource calibration. Those remain mandatory
work.
