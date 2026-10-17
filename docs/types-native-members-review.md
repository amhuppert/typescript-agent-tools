# Canonical member ownership review

This bounded read-only review applies the Ousterhout software-design skill to
the first retained native member stage. It reads `binding/member.rs`, the
collector's declaring-container context, linker declaration/table merge paths,
Store ordered associations/value declaration precedence, the binding semantic
facade and Source/evaluator member children. TypeScript 7.0.2 at commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` is the source authority.

No new incorrect answer was established in the admitted ordinary property
slice. The important ownership decisions are cohesive: the existing binder
produces actual canonical identities; the Source child acquires only their
demanded syntax; one evaluator symbol-Type cache supplies member reads.
Public origins do not become semantic identity. A TypeParameter/Property
symbol filters its actual declaration roles separately for declared and value
questions while retaining the complete association list for origins.

## Actionable quality finding

`Source::canonical_property` scans the whole captured AST for every named
member. `member_type` repeats that scan for each demanded read, and reserved
signature acquisition performs further complete scans. Consequently acquiring
a wide named inventory costs O(member count × AST size) before presentation
can omit member records. There is no measured resource violation in this
review, but this cost belongs to the unresolved runtime/representation gate.

Keep actual retained declaration role/site attachments and resolve them through
the existing `CapturedSyntax::node_at` boundary, with one request-local exact
site index if necessary. That operation already distinguishes absence and
ambiguity. Do not add a second member-name map or a second AST selection rule.
The index can be a mechanical acquisition improvement followed by the existing
final member/query controls; new behavior requires its own final reproduction.

## Checked boundaries and proof limits

The early key owner keeps literal exact scalar/UTF-16 payloads, signed numeric
computed declaration keys, private keys, reserved signature roles and dynamic
keys in separate domains. Typed private/reserved partitions cannot collide with
legal user string keys. Static members use the declaring symbol's export table;
instance members use its member table. A late computed declaration remains an
explicit inventory obligation. Anonymous object/type literals clear the named
declaring-container context instead of attaching their properties to an outer
class/interface.

`declare_member` combines compatible flags and ordered own declarations at the
actual key. A suppressed exclusion conflict allocates a separate declaration
symbol without replacing the table's earlier entry. Store transfer retains
target-before-source associations, and the shared SetValueDeclaration owner
selects the actual recipe declaration. Named member ordering uses first actual
declaration file/position, exact name and canonical identity; source table
iteration order is not public member order. Inherited/class inventory remains
explicitly pending, so this stage does not prove the native own/inherited
partition or complete class prototype/constructor behavior.

The implementer reports the first public member two-test GREEN
`vrun-f564dcd9-dd38-47d5-b546-33592eeb8046` and the extended four-test GREEN
`vrun-4a549888-b1e5-4b09-84bd-98d6f7a6d15f` after a genuine complete-origins
assertion failure. This review
read the final four public test definitions: merged first-value precedence,
exact literal/escaped keys, canonical merged generic inventory and the shared
TypeParameter/Property question distinction. It did not independently rerun
those tests or close the broader retention, lexical compatibility or resource
gates.

Computed-name wrapper interception was independently reproduced in the final
five-test member run (assertion RED reported as `d86c289c`). The implementer
pinned fresh selection controls and restricted declaration-name interception
to actual supported name tokens. The final five-test GREEN is
`vrun-637db28d-2302-4674-a414-1952fa213b27`, with zero skips; the compiler
control run was reported as `9061262a`. This disposition closes that wrapper
interception finding, rather than the wider computed-member construction gate.
Methods/accessors, optional read/write,
parameter properties, private/static public inventory, late computed members,
inheritance and full mapper identities remain required producers/consumers;
their current explicit Pending paths are intermediate implementation states.

## Inline TypeLiteral follow-up

The generalized TypeLiteral factory and member projection were read against
checker.go `getTypeFromTypeLiteralOrFunctionOrConstructorTypeNode` (22839),
`resolveAnonymousTypeMembers` (20555) and `getNamedMembers` (21954). No incorrect
successful answer was found in the admitted direct inline family. Its exact
anonymous node association and retained member table decide the factory;
unaliased empty tables select the shared emptyTypeLiteral identity, while an
alias keeps its own identity. Nonempty anonymous identity is installed before
property values or signature children are demanded. The shared canonical member
projection supplies existing separate read/write/display questions, exact
names/order and actual reserved signature declarations. Removed raw-span member
identity does not become a second identity rule for this family.

The implementer's five final serialization/identity controls passed in
`vrun-3488e91f-6cd6-438b-8789-007de8ffb86d`, with zero skips. This review read
the test definitions but did not rerun them. Native anonymous resolution also
publishes temporarily empty collections before resolving signatures/indexes;
the current MembersState::Resolving path instead refuses recursive inventory.
Recursive qualified/type-query inventory therefore remains an explicit native
protocol obligation, rather than proof of complete anonymous resolution.
Mapped TypeLiteral scope acquisition and Function/ConstructorType's two actual
binder roles/final node.Symbol attachment remain required later operations.
Those limits do not invalidate the successful direct controls or close the
full semantic, lexical compatibility or resource gates.
