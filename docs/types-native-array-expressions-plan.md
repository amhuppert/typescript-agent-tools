# Native array expression integration

The existing immutable Project and request-local checker remain the operating
envelope. Array literals require actual contextual, tuple, union, property and
iteration decisions; they cannot be implemented as a list-to-union shortcut.
The new owner ports `checkArrayLiteral` from the pinned 7.0.2 checker.

The main ownership change is a complete mutable-location operation. Object
properties previously widened literals inline and could not preserve const or
contextual identities. `checkExpressionForMutableLocation` now checks first,
asks the shared const-context owner, preserves assertions, and applies shared
contextual instantiation before widening. Source supplies exact ancestry and
element roles; evaluation owns the semantic const-variable and enum questions.
The existing context-sensitive syntax traversal now accepts expressions as well
as call argument lists, instead of introducing a separate array predicate.

A local array-only context stack would duplicate inference lookup and could
mask an active inference with a cached nil contextual entry. The contextual
owner instead distinguishes the native contextual and inference stacks,
including cached nil entries and their different lookup directions. Array
acquisition is enclosed by one operation that restores the stack on failure;
tuple normalization and subtype reduction run after that frame is popped.

Tuple and array spreads use the existing type arguments, normalization, indexed
access and iteration owners. Exact-optional omitted elements use Missing and
make following elements optional; ordinary undefined and loose widening stay
distinct. Const array readonly selection consumes actual mutable-array-like
context after all elements have been checked. The same questions are shared
with the checked-call rest owner.

Const assertions and satisfies expressions supply public query controls for
array contexts: an opening bracket is deliberately not a public type anchor.
An ordinary false satisfies relation preserves the expression type, while
consumed target or relation failures remain failures. Adversarial review required a separate type-node source-check owner: lazy
getTypeFromTypeNode alone does not reproduce checkSourceElement dependencies.
The new owner preserves array, tuple, union, wrapper and operator child order,
including the tuple variadic prepass and each child's native instantiation-count
window. Named tuple members acquire their type after checking the child, before
the next sibling. Type queries acquire their existing query type; indexed access
uses the existing index-validation owner. Type references check their argument
children before acquiring the reference, then check the actual alias or target
local constraints. A failed assignability relation is diagnostic data: later
constraints are still acquired, but their assignability checks short-circuit.
Effective arguments and their mapper are acquired only at the first constraint.
Other demanded source-check workers remain
explicit dependencies until implemented. The array tests alone cannot establish
that entire checker family.

The assertion/satisfies controls in `native-type-checks` distinguish ordinary
constraint rejection from a consumed missing constraint, including a later
missing constraint after an earlier false relation. The first compiler run
(`vrun-27a7d0a7-3aa0-48ce-ad18-2e5248060074`) disproved an initial expectation:
`getQuickTypeOfExpression` returns an ordinary assertion's annotation without
checking its operand or the annotation's source elements. This applies to
declaration initializers as well as expression queries. The revised controls
retain those healthy shortcuts and separately force `checkAssertion` inside
`satisfies`. Extending the shared quick-type operation preserves this distinction;
adding a bypass only to assertion tests or variable recipes would duplicate it.
The controls also cover skipped relation dependencies, ignored alias arguments,
type queries, labeled tuples, indexed access, function/constructor annotations,
and assertion-versus-satisfies failure ordering. All 69 compiler questions pass
in `vrun-5785148e-ec57-4c71-ab6f-8ac2b9aeb88c` (one test, zero skips). The
instrumented diagnostic codes are sorted observations, not an execution trace;
the native worker source establishes the checking order. Final-query proof is pending.

The maintained 14-case compiler matrix passes in strict, loose and exact
profiles (`vrun-c8006daa-9ddb-4150-9e79-f1be2198e934`, 42 queries, zero skips).
The added parenthesized-empty case passes all 45 queries separately in
`vrun-af0d87fc-17fe-4793-87d7-4abfb582ac31` (three tests, zero skips).
Compiler observations corrected the expected literal order, ordinary string
widening in contextual holes, and loose undefined-to-any tuple widening.
`tests/type_array_expressions.rs` asserts final summaries with and without
expansion. Final Rust proof is pending the coordinated checkpoint, including
the pre-existing missing automatic-array variable marker. An added direct `[]`
versus parenthesized `([])` pair distinguishes the native automatic-array gate:
its `isEmptyArrayLiteral` question does not strip parentheses.

`tests/type_union_limits.rs` uses the existing natural subtype fixture family:
1250 types and 43/42/41 early removals produce estimates 991250, 1000000 and
1007500. It checks the surviving union counts at the public graph and preserves
the consumed over-limit cause. A 10001-element flat array proves that large
shallow input does not itself consume that pair limit. The maintained compiler
test repeats the exact family under strict and loose options. It passes with
the flat controls in the same seven-test receipt above; initial missing-global
diagnostics were resolved by completing the fixture globals.

Adversarial review matched array acquisition, spread, omitted, context and
factory scheduling. It also retained unresolved work explicitly: qualified
enum const-argument resolution, mapped contextual producers, complete iterable
protocols, full assertion diagnostic dependencies, and canonical mutable
signature observations for contextual callbacks. These remain implementation
obligations, not exclusions added to the delivery profile.
