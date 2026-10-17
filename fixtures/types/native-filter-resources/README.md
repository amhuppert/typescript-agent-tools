# Native cached filter resource controls

Each cheap/heavy pair first acquires the same wide numeric union and scalar,
then repeats eight eager scalar or union non-null assertions and returns a
declared number. The widths are 16 and 256. Paired names/operands have equal
lengths and every declaration shares one captured source.

Pinned `getFlowTypeOfReference` (flow.go111) performs a facts filter for a direct
non-null parent. `checkNonNullAssertion` then calls `GetNonNullableType`, which
filters again via getAdjustedTypeWithFacts/getTypeWithFacts/filterType. The
maintained hook asserts exactly sixteen unchanged wide-union filters per heavy
query, no wide filter per cheap query, and same-input results. Thus the witness
proves actual traversal despite unchanged final type identity. The subsequent
mapType pass remains a distinct accounting obligation.

The eager comma operands produce ten diagnostic-only TS2695 entries per paired
query and none for the standalone healthy control. The final type remains
number, errorType is false, and there are no erroneous acquisitions or native
instantiations. Registered native strict/noCheck validation
`vrun-e8372beb-7f53-4d68-8147-d25b8ea3af12` passes all ten cold hook/API
queries and traversal assertions (two tests, zero skips).

Earlier fixtures wrapped these operands in void. `checkVoidExpression` instead
queues operand checking for a deferred source-file pass; those cold queries did
not exercise filtering. Earlier number observations remain valid but their
resource-traversal/omission claims are superseded. Original inputs/observations
are preserved under `.cc/temp/stage67-deferred-void-*`. Production Void semantics
are unchanged. The corrected public resource GREEN and omission counterfactual
remain pending the connected checker checkpoint.

cases.ts SHA-256: `3d23a268bbd4a473fd0d524c3f3a66cdb3fcd34956f3881d562d79d99430eba1`.
