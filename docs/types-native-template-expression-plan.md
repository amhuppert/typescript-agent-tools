# Template expression checking and constant evaluation

This implementation serves the captured TS/TSX query worker and the native
7.0.2 checker. It introduces no compiler process, execution of user code, or
persistent cache. The complete T/Q3/Q4 contract remains in force.

`checkTemplateExpression` owns ordered Normal-mode substitution checking,
`maybeTypeOfKindConsideringBaseConstraint`, assignment to templateConstraintType,
source constant evaluation, and the short-circuit const/element/contextual type
checks. Existing template-type construction owns distribution, normalization,
exact UTF-16 joining, and literal identities. Source owns actual parent identity
and cooked token payloads; no display string is used as an inferred type.

A literal-type shortcut would lose the native distinction between an annotated
literal constant and an unannotated const initializer, assertion boundaries,
source ordering and imported constant declarations. Instead `NewEvaluator` and
`evaluateEntity` form a shared source constant operation. Its four result facts
(value, syntactic stringness, other-file resolution and external reference)
remain distinct for its consumers. It uses existing retained Value-meaning
resolution and declaration-before-use checking. `getGlobalSymbol` has one shared
selection owner for existing Type/TypeAlias queries and actual Value identity.
Numeric conversion and bit operations belong to the common jsnum owner.
There is no second expression cache: native constant evaluation does not have
one. Enum member acquisition is still an explicit dependency requiring its
own native owner; it is not approximated by the member's displayed type.

The fixture includes literal versus checked constant distinctions, annotation
and assertion boundaries, const/ordinary contexts, unions, nullable and invalid
substitutions, nested templates, freshness/widening, exact cooked text and
original unresolved-symbol failures across strict, loose and exact profiles.
Pinned compiler captures establish expectations before final Project equality.
A TemplateHead is not an admitted query token; contextual and indexed controls
must use a supported parent expression anchor or the enclosing value query.

Remaining verification includes source-order and imported const controls,
special global identity and shadowing, numeric bitwise/constant arithmetic,
Unicode code units and line classes, generic contextual constraints, full enum
constant dependency closure, and controlled recursion/allocation. A scoped
fixture pass does not claim these remaining obligations complete.

Native capture `vrun-2ffdc9eb-26da-47ef-bddd-d690518cdce2` completed all
31 queries in each of three profiles with zero skips. Fresh API and private
native hook agreed for all 93 results, including healthy root String with a
consumed MissingPlaceholder diagnostic, and the symbol-conversion diagnostic.
`expected.json` records the captured summaries, kinds, root recovery bits and
ordered diagnostic codes; the maintained harness asserts each field. Earlier
capture attempts stopped on unsupported TemplateHead/OpenParen anchors; the
final controls use the actual call-close and element-access-close tokens.
The bounded independent Ousterhout review verified the staged native demand
order and exact-text/metadata rules and found a missing ++/-- constant projection
case. Both native prefix operators now reach the ordinary absent-value result
after operand evaluation. Their checked expression owner remains pending.

Maintained exact oracle gate is GREEN
`vrun-896f291b-b72d-4a95-9550-c6f60b4932f1`: 93 cases across three tests,
zero skips. A second bounded independent review confirmed Value-only terminal
lookup, qualified Namespace receivers, global raw-alias identity comparison,
and existing Type/TypeAlias eligibility/failure behavior. It found that two new
lookups incorrectly called project-wide Bindings; both now use request-local
QueryBindings through Source. Final Rust tests remain the next gate.

The shared contextual Source owner now implements the native substitution
parent gate (`getContextualTypeForSubstitutionExpression`, checker29787): an
ordinary template substitution has no contextual type. Only an actual tagged
parent reaches its still-pending effective-argument context. The earlier blanket
Pending for every TemplateSpan would incorrectly refuse ordinary generic
substitutions even though the checker has an explicit nil branch.
