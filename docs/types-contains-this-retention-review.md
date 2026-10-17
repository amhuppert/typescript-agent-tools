# Bounded ContainsThis retention review

This independent read-only review uses the Ousterhout skill and the pinned
TypeScript 7.0.2 `GetContainerFlags`, `bindWorker` and `bindContainer` operations.
It covers the staged binding-fact retention boundary, not native ThisType
evaluation or complete class/interface semantics.

No concrete blocking defect was found in this slice. The existing transient
flow syntax owner now returns the whole container classification, and its
control-flow, immediate-function and capturing-method callers consume that
result. It retains distinct Interface and PropagatesThis questions; it does
not infer one from whether a container resets flow. Scope-only TypeLiteral,
TypeAlias, IndexSignature and class containers keep their native classifications.
Property declarations reset the seen-this state only when they actually have
an initializer. Ordinary function blocks use the existing whole IsFunctionLike
owner. JSTypeAliasDeclaration is explicitly outside the accepted TS/TSX grammar.

The iterative Builder records only real ThisKeyword or ThisType syntax before
container entry. Its completion frames save/reset/restore the state in native
order. Signature and arrow containers propagate their resulting state; ordinary
functions, methods, property initializers and module blocks restore the saved
state. An interface finalizes its own result and restores the surrounding state
without propagation. Container finalization precedes the seen-this completion,
matching the native restoration order. Root and unreachable traversal still
visit children, so binding facts do not disappear merely because flow becomes
unreachable. A property whose exact name is "this" contributes no token-kind
fact.

The interface-name span map exists only in transient Capture. Collector
transfers the finalized value into the already retained declaration attachment,
then drops that transient owner with the parsed tree. Canonical complete
BoundClassOrInterface own-declaration associations copy the same attachment.
Option distinguishes a finalized false interface result from a declaration
family with no such fact. There is no retained second lookup map or later source
scan to make the decision again.

The fresh native two-test gate is
`vrun-ab2602e8-f97e-479c-95d6-34688ec10af4`, covering 21 interface container variants and direct
ThisType legal/error questions. The new final-Project private retention test
in `binding/retained.rs` checks those 21 facts after Project load, crossed with
CR, LF, CRLF, U+2028 and U+2029 plus leading BOM and astral trivia. At this
review point its registered private terminal receipt is still pending; this
source review does not manufacture one. Public ThisType tests require their
own actual assertion RED and subsequent native evaluator implementation.

Remaining full-design work includes whole GetThisContainer/getThisType,
synthetic-this target identity, merged/interface/base propagation and mapper
questions, ordinary Reference shape/presentation, and the semantic/resource
gates. The mechanical retained fact does not establish those answers.

The later bounded semantic read compares the admitted no-base
`class_or_interface_type`, `is_thisless_interface` and `this_type` operations
with checker.go 17231–17294 and 22809–22835. No concrete false answer was
found. Canonical generic parameters and Class kind take the native earlier
synthetic-this branch. Ordinary interface declarations inspect their retained
ContainsThis fact before that declaration's bases; an unproved base remains a
located obligation. `getThisType` consumes the actual nearest parent symbol,
checks static/constructor-body context and retrieves that target's established
synthetic parameter. It does not fabricate a parameter from spelling.

Public This2 GREEN `vrun-869417eb-38c3-4336-823c-32ea285c8351` and extended
This3 GREEN `vrun-ea80dc69-79eb-4d94-8d6a-9569be7cdae2` are separate
semantic evidence after their assertion RED. They do not close base/merged
inventory or ordinary Reference presentation. The anonymous
`export default interface { ... }` candidate was independently rejected by
the pinned input gate with TS1003 in
`vrun-9515ff89-f04f-4f74-9aee-c801594937b1`; it is outside the accepted
semantic profile and supplies no valid-program retention defect. The private
retention checkpoint receipt remains separate and pending in this review.
