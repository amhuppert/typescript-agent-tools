# Native union resource controls

The grouped and alternating declarations use the same aliases with the same
multiplicities in one captured file. Alias names and selected declaration names
have matching lengths. Small controls use two disjoint 16-member numeric unions
four times each; large controls use two 256-member unions 32 times each.
All four final types are `number`.

TypeScript 7.0.2 `getUnionTypeEx` (checker.go:25536) takes its general worker for
these lists, which all contain more than two entries. `addTypesToUnion`
(25669) suppresses only adjacent identical input types, before recursively
visiting union constituents. Thus grouped small/large lists flatten 32/512
numeric leaves, whereas alternating lists flatten 128/16,384; each also visits
the final `number`. `getUnionTypeWorker` first builds that ordered set and then
`removeRedundantLiteralTypes` reduces the literals under `number`. There is no
new final union allocation to stand in for this work.

Separate ascending, descending and adjacent-duplicate controls require the
complete native ordered result `0 | 1 | ... | 15`. A direct `number` declaration
is a healthy cheap control. Both strict and noCheck use noLib with complete
minimal globals, so all 16 cold hook/API queries must have no diagnostics,
erroneous acquisitions, or native limits.

Run `tools/type-native-union-resources-oracle.test.mjs` through the registered
harness. It writes captured results under `.cc/temp/native-union-resources-*`.
The source-derived leaf counts describe the native operation, not a new oracle
observer or an expected product work-unit count. The public resource tests must
separately show a grouped/alternating budget discriminator and fail when the
new inner-loop accounting is removed; existing generic-fanout accounting alone
does not prove this increment.

Maintained capture passed as `vrun-0e8a9414-7eac-407a-bf7d-806d5b765670`:
16 queries, public API parity, no skips. All native instantiation counters are
zero, separating this family from the existing generic-fanout control. The
captured `cases.ts` SHA-256 is
`c97eb838f5c109e7dd51b81d4ab58973d96a7d78d18914ebf47d4ec6c6c5f9bf`.
