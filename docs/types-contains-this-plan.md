# Native interface ContainsThis and ThisType

This increment operates inside one captured Project generation and its existing
native binder/checker owners. It adds no production compiler process, second
binder, public fact-query API or retained source-site lookup map. TypeScript
7.0.2 commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b` owns expectations.

`bindWorker` records ThisKeyword and ThisType, independently of reachability.
`GetContainerFlags` controls `bindContainer`'s save/reset/finalize/restore state:
control-flow containers propagate only when PropagatesThisKeyword is set;
interfaces restore their saved outer state unconditionally. This applies to
SourceFile, arrow/signature/function roles and initializer containers, including
computed names whose functions isolate a ThisKeyword. A property name spelled
`this` is not a ThisKeyword fact.

A source-wide syntax search is cheaper to write but cannot distinguish those
native lifecycle rules. A retained AST flag/site map would keep construction
handles alive and introduce another searchable inventory. The chosen boundary
extends existing `flow::syntax::Facts::container_flags` to the whole accepted
TS/TSX GetContainerFlags rule and uses the existing Builder scheduling stack.
A transient finalized interface-name attachment transfers once into the existing
Collector Decl.contains_this fact. Canonical own declaration associations expose
that same fact through BoundDeclaration; it is not reconstructed from projected
lexical groups or spelling. None means a different declaration role, not an
unproved false interface flag. The transient map is released after collection.
This adds an optional bool to declaration facts/borrowed descriptors and a
construction map bounded by interface count; retained-layout and whole-program
resource gates must still account for the actual representation.

`isThislessInterface` remains one evaluator question over complete ordered own
interface declarations. Each finalized flag precedes that declaration's bases;
no flag OR skips an earlier native base dependency. The current no-base family
proves absence/presence. Reached base-symbol/base-this acquisition remains a
located pending semantic obligation. Generic parameters and Class kind provide
the native earlier synthetic-this branch without demanding thisless bases.
The existing no-this ordinary-interface graph representation stays temporarily
inside the same deep target operation until ordinary Reference shape display
is admitted; it is not a permanent second target decision or known-empty shell.

`GetThisContainer` is one shared syntax operation, including computed-name and
decorator escape rules and explicit arrow inclusion. Source projects pure
nearest-container/parent/modifier/body facts. The evaluator's `getThisType`
operation decides eligibility, acquires the actual canonical named parent's
declared type, and consumes its real synthetic parameter. Illegal contexts and
native absent-this fallback become InvalidSemantic/ErrorType at the actual
selected location. An anonymous class lacking the retained own symbol remains
explicit pending; no name or fresh parameter substitutes for that identity.
Constructor/value type questions, outer generics, full class and merged member
inventories, inherited this acquisition and general mapper families remain
required subsequent work.

The fresh registered native baseline
`vrun-ab2602e8-f97e-479c-95d6-34688ec10af4` passed two tests with zero
skips: 21 final interface flags, 12 legal/error ThisType questions, and six
invalid/two class/body contexts. A nested object-type ThisType and a generic
constraint set ContainsThis but still return native errorType at their own
sites. No semantic-diagnostic prelude is used. A first baseline setup assertion
incorrectly expected getTokenAtPosition to return ThisType rather than its
ThisKeyword child; the corrected control explicitly asks the parent type node.
This was a test setup correction, not a compiler behavior defect.

Public This2 assertion RED
`vrun-6c984f17-b4ab-4cf4-92b4-c7ecd9b3fad3` executed both tests and failed
both missing type-node assertions. Earlier `ca508596` and `6785f49e` runs were
compile/setup failures during owner staging and do not count as behavior RED.
Public This2 GREEN `vrun-869417eb-38c3-4336-823c-32ea285c8351` and
extended This3 GREEN `vrun-ea80dc69-79eb-4d94-8d6a-9569be7cdae2`
passed with zero skips. The third test adds constructor-body/class-field arrow
contexts and both merged-interface declaration orders. The extended native
baseline `vrun-895e180f-a7d2-4e83-9d3e-1b822f2b6b89` passed both tests
with zero skips, including static arrows, nested functions and class-computed
name escape contexts. All public position controls exercise final Project loads
and type answers, including exact invalid consumer locations.

An exploratory anonymous `export default interface { value: this }` candidate
produced native TS1003 (Identifier expected). It is outside the accepted semantic
input profile; the tentative extra attachment wiring and fixture were removed.
The rejected source is retained only in `.cc/temp/types-contains-this/` and is
not a claimed valid-profile defect or production remedy.

The final-Project private binding test checks every native flag variant across
CR/LF/CRLF/U+2028/U+2029, BOM and astral text. Its terminal execution remains
checkpoint-pending; source inspection and public This3 GREEN do not substitute
for that private retained-fact proof. Native/source observations and selected
This behavior also do not close class/member/base/generic-resource gates.
