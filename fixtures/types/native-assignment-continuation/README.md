This matrix exercises the pinned compiler's complete `getInitialType`,
`getAssignedType`, destructured projection/default, and `getAssignmentReducedType`
operations. The selected reads have union declarations where the native flow
worker must acquire assignment types; `scalarSkipsAssignment` pins the scalar
skip separately. `assignedIdentity` and `assignedNever` pin the pre-cache exits.

The three reduction pairs select a variable whose initializer evaluates distinct
references in one comma expression and one checker. The assignment observer records the existing cache lookup and
publication with actual type identities. Reversing the two operands is a second
native query, not proof of cache sharing between Project requests. The observer
also records entry/exit of the existing default operation; it adds no semantic
queries. Diagnostics are source-sorted and are never treated as an acquisition
trace.

Strict, loose-null, and unchecked-index profiles distinguish the native
undefined and missing sentinels. Expected outputs are captured from TypeScript
7.0.2 before final Project assertions are installed.

Capture `vrun-94a1aaab-7051-41a1-bc38-7e6a121fdf97` matched all 87 ordinary
API/hook rows. Every warm pair records exactly miss, install, hit for the same
native key. The nested-array binary deliberately takes ordinary RHS typing:
its immediate array container is not `isDestructuringAssignmentTarget`, so no
native default operation is entered. The final tests preserve this distinction
and retain all acquired evidence on cache replay.
