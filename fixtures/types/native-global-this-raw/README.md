# Native globalThis raw identity proof

Pinned TypeScript 7.0.2, commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

The duplicate declaration-file aliases were rejected with TS2300 by the pinned
ordinary API before queries, even with skipLibCheck. They remain an explicit
rejection control; they do not prove admitted missing-metadata behavior or the
proposed visible-var/block-scoped lookup distinction.

The overload rows trigger addImplementationSuccessElaboration, which consumes
failed.declaration.Symbol().Declarations without getMergedSymbol. A namespace in
a separate source file adds declarations only to the merged initialized header.
The reverse file order and external-module local control distinguish a real
merged source from lexical shadowing. Collision rows retain the initialized header.

The additive observer reads pointer identity and existing declaration lists only
at getSymbolOfDeclaration, getTypeOfSymbol and the existing raw implementation
elaboration branch. It performs no extra semantic getter or symbol-ID allocation.
Native diagnostic codes establish membership, not acquisition order.

Capture `vrun-172be940-2cb8-4493-bd3c-5a2762bea1eb` completed all 11 API/hook
comparisons; only the expected file was absent. The duplicate-alias rejection
control passed. Both overload file orders reach the actual raw implementation
branch: raw symbol differs from intrinsic, raw Declarations length is 2, merged
intrinsic length is 3, and the existing merged map points at the intrinsic.
The good call skips that branch. The local overload has no intrinsic mapping.
Collision rows retain initialized Transient/Readonly with no declarations.
Maintained comparison `vrun-e275d1b3-6af2-4b50-b3e1-3d5fe0b57d1a` passed
all 11 admitted roots and the separate rejection control, with zero skips.
Final Project assertions are staged in `tests/type_global_this_raw.rs`; their
coordinated Rust run remains pending.
