# Bounded basic-flow review

This read-only Ousterhout review assumes one accepted immutable Project and a
request-local evaluator. It covers the staged whole binder decisions for
variable/default/destructuring/delete/call/array mutations and their admitted
read consumers. Pinned TypeScript 7.0.2 binder functions are
`bindInitializedVariableFlow`, `bindInitializer`, `bindBindingElementFlow`,
`bindParameterFlow`, `bindDestructuringAssignmentFlow`,
`bindAssignmentTargetFlow`, `bindDestructuringTargetFlow`,
`bindCallExpressionFlow`, and `bindDeleteExpressionFlow`.

No additional false successful answer was found in this source slice.
The explicit work stack preserves native child order: parameter modifiers,
rest/question/type/default/name; binding rest/property/default/name; every
initialized binding leaf; nested-pattern RHS before its pattern and ordinary
destructuring pattern before RHS. Native omitted binding nodes are also retained
as assignment nodes rather than being dropped by a narrower source rule.

`EndInitializer` retains the unevaluated entry alternative first, and creates
a join only when reachable entry and exit differ. Pattern-state transitions
match the two native destructuring branches. Immediate property deletion is
distinct from element or parenthesized deletion. IIFE type arguments and value
arguments precede callee traversal. Nonoptional super calls and actual
identifier push/unshift property calls retain their separate mutation decisions;
element assignment adds its array mutation after ordinary target flow.

Array mutation read admission asks actual automatic identity before acquiring
the receiver. An ordinary annotated type therefore does not demand unrelated
mutation arguments. Matching evolving-array work remains an explicit excluded
flow dependency. Raw reference identity and type acquisition remain separate
questions; a same-named unrelated target cannot become a consumed initializer.

The completed native union reducer changes the earlier strict mutation-join
expectation. A zero-demand named-return observation on
`getFlowTypeOfReferenceEx` proves that push, unshift, escaped push and element
assignment joins return the exact declared type identity in both strict and
loose profiles; the declared-first branch does too. The maintained compiler
control is `vrun-45af64df-d2a8-4216-83e1-2e1cf42197d9` (two tests, no skips),
with observations in `.cc/temp/native-basic-flow-identity.json`. The final
Project test now expects healthy `number | "ready"` for those joins. This
corrects a stale refusal expectation; the flow admission owner was not relaxed.

The parent independently identified ordered branch-label early termination:
an antecedent equal to declared type when initial also equals declared type
must stop before later alternatives. The reviewed current reader has a
sequential Branch work item with that exact early return, ordered deduplication
and native subtype-versus-literal union choice. Its final compiler and Project
controls belong to the parent lane; this source read does not claim their
terminal verdict. It must not be replaced by eager traversal of all antecedents.

The original final basic-flow missing-behavior RED is `vrun-dd70ac4a` and the
fresh compiler marker matrix is `vrun-175d6507` (full receipts held by the
parent). The final expanded six-test Project run passed with zero skips in
`vrun-3f8977fc-29f2-46ef-a435-862d9e1fc514`, including the ordered branch
controls and compiler-derived expectation corrections. Complete call
resolution, evolving arrays and excluded F analysis are separate mandatory
owners; this review does not authorize guessed outcomes for them.
