# Native reference context: remaining alias-condition work

The local query checker does not execute application code. Native alias-condition
inlining is nevertheless a required dependency of its identity-based flow
admission: it can leave the final read unchanged. This note records an observed
ownership constraint, not a completed implementation or an expansion of F.

`narrowType` uses the checker-wide `inlineLevel` limit of five, then asks the raw
resolved symbol, `isConstantVariable`, the actual variable ValueDeclaration's
annotation/initializer, and `isConstantReference`. Only the admitted branch
increments the level around recursive initializer narrowing. Cleanup must also
restore that level after an interrupted product operation. The current
`admit_alias_condition` boundary predates the shared constant-reference owner;
its declaration-family approximation must be removed when the operation moves.

A single file argument is insufficient for that recursive question. A global
constant condition can be declared in another source file, while the original
flow reference and graph remain in the querying function's file. Reinterpreting
the initializer's Site in the original file would silently select unrelated
syntax. Replacing the state's file would instead corrupt the original
reference's symbol lookup. The same concern applies to matcher source/target
pairs; their already established asymmetry must be preserved.

Two possible migrations are explicit source/target file arguments at every
matching call, or file-owning captured reference handles with created nodes
retaining their actual parent/receiver handles. Prefer the latter if the trace
confirms the existing factories can supply it: it puts syntax identity in the
reference owner and avoids an additional pairing rule at every caller. Keep
flow-graph identity separate from syntax identity. Do not use text matching,
canonicalized paths, or a one-file approximation to avoid this migration.

Proof must distinguish same-file and cross-file constant aliases, raw aliases
that are not constant variables, annotated/uninitialized variables, mutable
and unassigned parameters, readonly and mutable properties, checker-created
references, and the four/five/six alias boundary. Identity cases must finish as
Found; genuinely changed reads retain FlowRequired. Error operands after alias
inlining must retain their original declaration locations and native acquisition
order. These expectations require pinned compiler captures before assertions.

Stage35 implements the first ownership move: `ReferenceNode` carries its source
file, including checker-created references; its captured children inherit that
file and its retained name/parent/receiver keep their own complete identities.
Matching resolves each operand in its own file. Contextual and narrowable-type
operations take the complete reference rather than a separate file plus node.
The flow graph and flow container remain in `ReadState`, independently of those
syntax identities. This is a prerequisite migration, not a claim that the
remaining alias-condition operation is implemented or verified.

The new native alias-condition capture has 27 roots in strict and loose modes.
It distinguishes alias eligibility, const/unassigned/assigned references,
readonly/mutable property references, same-file and cross-file initializer
contexts, imported aliases, recovery dependencies, and the four/five/six inline
boundary. Existing native flow-return observations prove final identity; their
map order is not used as an acquisition trace. File offsets of marked recovery
operands are kept distinct because that observer does not retain file names.

The full alias operation is now staged as well: `narrowType` retains its raw
symbol/declaration/annotation/initializer/constant-reference gates and
checker-wide five-level scope. It restores that scope before propagating either
a completed result or an interruption. Equality, truthiness, optional-chain,
call and discriminant operations now take complete syntax roots. Returned
candidate discriminant accesses also retain their own file; the old same-file
screens are removed. Correlated pseudo-references retain declaration syntax,
while their explicit borrowed flow is interpreted in the actual use-site file.

The baseline grew to 34 roots per profile after review requested same-query
scope restoration, depth-limit recovery, and cross-file discriminant/call
controls. Native captures prove both sibling errors after unwind, the
five-versus-six acquisition distinction, and the full cross-file operations.
The original 27 native rows remained identical. Source and final-test peer
reviews passed after tightening pure FlowRequired assertions to exclude hidden
causes. Rust compilation and behavioral verdicts are still pending.
