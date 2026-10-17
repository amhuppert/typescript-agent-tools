# Native declared type-node increment

This increment continues the complete T/Q3/Q4 implementation for one captured,
immutable program generation and one fresh semantic request. Native public
delivery, complete candidate inventory, compiler limits, representation cost,
and service resource gates remain open. The first admitted structured descriptor family contains ordinary required
identifier-keyed property signatures. String and number read types are
implemented; other annotations retain lazy recipes and their richer semantic
families remain mandatory subsequent operations.

## Ownership and alternatives

The source owner acquires final-tree recipes from captured text and verifies
canonical binder declarations. Evaluation owns `getTypeFromTypeNodeWorker`,
`getTypeFromLiteralTypeNode`, `getDeclaredTypeOfTypeAlias`, and the corresponding
interface operation. Model owns type identity, literal regular/fresh state,
collection identities and views. Presentation owns bounded summaries and graph
admission. Import labels do not become type aliases.

Two boundaries were compared using the Ousterhout designing route. One would
have source turn member declarations directly into public graph rows, letting
each consumer infer names, alias metadata and property identity. The selected
boundary returns semantic recipes and established Model facts: evaluator and
Q3 consumers share semantic identities, while the presenter never inspects an
AST. This keeps source cooking, semantic construction and output policy in
their respective owners, at the cost of explicit request-local recipes.

The pinned compiler creates a lazy anonymous root in
`getTypeFromTypeLiteralOrFunctionOrConstructorTypeNode`; its
`resolveAnonymousTypeMembers` enumerates symbols before property type demands.
An ordinary object literal instead demands its initializers in
`checkObjectLiteral`. These are distinct operations. Required primitive
property signatures can be established without exercising failure differences,
but the next unsupported-child family must preserve a located, fallible member
demand. Eagerly demanding arbitrary invalid children at root acquisition would
silently change the checking history.

The first structural alias uses `getAliasForTypeNode`, and therefore exposes
the alias name and own origin as alias metadata. An interface's named display
comes from its symbol and is not alias metadata. A literal type uses
`getRegularTypeOfLiteralType(checkExpression(literal))`; it must not be widened
as a fresh mutable initializer. Shared Model APIs must retain these distinctions.

## Relevant compiler controls and native proof

`fixtures/types/native-type-nodes` and
`tools/type-native-type-nodes-oracle.test.mjs` freeze fresh declared/value/type
demands for string and number literals, inline required property signatures,
object aliases, interfaces, and renamed cross-file imports. Native behavior is
tested through `Project::load` and public `Project::type_of` in
`tests/type_vertical.rs`, first as an assertion failure on missing behavior.
No helper-only proof or second binder is introduced.

Required subsequent variants include optional property read types under every
null/optional option state, duplicate and merged declarations, call/construct
and index signatures, computed and exact decoded keys, empty objects,
inheritance, self references, type parameters and unsupported/invalid child
demands. Each family requires its relevant fresh native controls before
admission. Basic unions and arrays consume the shared Model algebra; default
library members must be established or explicitly blocked, never guessed empty.

## Member identity needed by Q3

Public display origins are insufficient identities. The proposed private
operation is `member_of(TypeId, exact_key) -> Result<Option<MemberId>,
DemandError>`. `MemberId` owns canonical source declaration roots and semantic
relationship provenance. A source declaration acquires its identity from the
captured file, raw name anchor and verified final syntax role; alias demands
reuse the same canonical declared-type cache. TypeIds and MemberIds remain
request-local and never become graph IDs.

The existing binder does not retain complete property symbols. Fabricating a
`BoundSymbol` from a lexical group or joining public origins would lose dual
roles, merged members and directional related-symbol behavior. Initial ordinary
property roots may be interned from exact source identity only after final
declaration/use controls prove the operation. Union/intersection/transient,
mapped, accessors, contextual and parameter-property roots require their own
native construction and cannot inherit that simple rule by default.

## Implemented ordinary property boundary

The first own-property value operation now covers one required identifier-keyed
property signature in a nonmerged interface or type literal. Its source root
is retained as captured file, raw token span and verified PropertySignature
role. Three unrelated `title` declarations, including an escaped raw spelling,
remain three candidates; each replays the same value question at its own anchor.
This is a canonical own source root, not a universal semantic MemberId. Merged
interface members and transient instances still require native identities and
directed root sets.

A supported direct PropertyAssignment name can also be represented as a pending
value seed. Collecting that metadata performs no type checking. Its pending
construction is consumed only after the actual enclosing-call prelude. A
computed-name expression retains its expression question, and unknown
identifier roles are not guessed to be values.

## Native evaluator and lazy member increment

The next mechanical ownership move places request-local Project, Source, Model,
canonical value cache and canonical declared cache inside one Evaluator. The
alternative of extending each expression helper with caller-managed source and
cache parameters would expose checking history to every caller. Complete
`getTypeOfSymbol`, `getDeclaredTypeOfSymbol`, `checkType` and `checkExpression`
operations instead own that state and keep the checking prelude before the root
demand. Expression construction and declared/type-node construction live in
separate implementation modules of the same evaluator, not separate checkers.

The following behavior change is lazy property demand. A TypeId continues to
mean an established semantic root. A member child is an opaque TypeDemand whose
known form contains a TypeId and whose deferred form addresses an evaluator-owned
DemandId. A separate MemberId identifies the member; different read, write,
constraint, default and return questions must not share one demand state merely
because they belong to a member. The ordinary property registry retains its
canonical own source roots independently of those question caches. Reserving a temporary TypeId and forwarding it later was
rejected: it would confuse semantic identity with unfinished source work and
leak those placeholders into unions, caches and graphs. Model views become
owned/shared values so presentation can demand children without borrowing the
model across a mutable operation. Presentation never reads recipes or raw syntax.

A named interface acquires its root and metadata without checking every member
annotation. Its complete ordinary property descriptors retain exact source
roots and deferred annotations. Presentation admits a child before consuming
its type demand. A currently unsupported valid member becomes an exact located
unsupported leaf and Partial; an unresolved member consumed as recovery stays a
whole-operation InvalidSemantic failure. Display omissions do not turn into
semantic failures or require child demands.

`fixtures/types/native-lazy-types` and its corresponding fresh oracle controls
separate a named interface's root request (`properties: false`) from explicit
member demands. A valid conditional member establishes number in TypeScript;
an unresolved MissingType member returns an error type when consumed. Final
Project tests will first require summary-only and depth-zero Found roots, then
expanded Partial for the pending valid conditional family and whole-operation
invalid failure for the consumed unresolved member. The full conditional and
relationship operations remain mandatory later increments.

Repeated deferred alias demands are also part of this increment. Two member
reads that consume one unsupported alias body must retain its original exact
reason/location. Canonical declared/value caches distinguish Resolving, Resolved
and Failed states; a failed demand is not evidence of a circular type. Demand
failures retain the full exact location in a TypeIssue before an AnalysisError
is converted for whole-operation delivery. Source acquisition of a pending
semantic form returns a located recipe, preserving that provenance for later
consumption.
