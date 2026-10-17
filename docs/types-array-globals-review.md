# Array dependency owner review

This bounded read-only review applies the Ousterhout reviewing route to
`binding/semantic.rs`, `evaluation/globals.rs` and `evaluation/json.rs`. It
reviews ownership and consumed recovery, not complete native arrays, generics,
library members or the full T/Q3/Q4 contract.

The global operation owns type-meaning eligibility, class/interface flags and
actual canonical parameter arity. Evaluation consumes that typed result and
retains the original missing/kind/arity cause while reporting invalid recovery
at the array consumer. A noLib setting does not decide symbol presence. A valid
target remains explicitly pending until generic instantiation and actual member
recipes are available. JSON checks each original array element, obtains the
shared subtype-reduced or empty-element input, and only then consumes the Array
target. None of those operations advertises the native fallback `{}` as a
healthy array.

The bounded six-test Project GREEN is
`vrun-0c2a7adf-35a0-4341-a367-5ede7a3549a2`, with zero skips. This source read
found no additional blocking defect in its single-target/own-list scope.
Distinct structured subtype comparison and contextual widening remain pending
operations; the broader missing-slot/array/member/resource obligations stay
open.

The first review exposed one concrete declaration-order defect.
`own_declarations` traversed captured source IDs/list order, whereas native
global symbol declarations follow binding and merge phases. A fresh control
places a declare-global augmentation earlier in program file order and a script
declaration later; native wrong-arity recovery identifies the script declaration.
The seventh Project test was RED in
`vrun-e3e3dfb7-3673-4433-8134-549709d415de`: six tests pass and the new assertion
reports `globals-first.ts:1:39` instead of the native
`globals-second.ts:1:11`. This is a failed exact-cause result, not a setup error.

The repair retains typed `(FileId, DeclId)` associations in Store through the
existing `addDeclarationToSymbol` operation. Both-way merges move those
associations and one-way merges copy them in the existing target-then-source
order. When canonical module identity swaps storage IDs, the declaration
vectors preserve the original merge order before transfer. The semantic facade
reads that retained order; it does not sort files, replay merge phases or use
lexical display sites as semantic declarations.

The final seven-test Project GREEN is
`vrun-51c73f61-f666-45e5-86ec-a8fbc1c1692b`, with zero skips. The follow-up
read of Store, Linker's declaration association and the semantic facade found
no blocking defect in this bounded repair. The earlier six-test GREEN alone
did not prove multi-file cause equivalence. Broader canonical parameter/merge
histories and the complete array, generic, member and resource obligations
still need their own final controls.
