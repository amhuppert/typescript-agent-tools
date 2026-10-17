# Native imported-JSON evaluation lane

This is the next bounded native increment, using Ousterhout's designing route.
The envelope is a request over one immutable successful Project generation.
It consumes retained JSON facts and options, performs no disk reads/rebinding,
and keeps the complete T/Q3/Q4, semantic-admission and resource gates open.

## Coordinated boundary

Extend the existing binding `getTypeOfAlias` operation, rather than adding a
JSON-specific alias resolver. The agreed typed result is
`BoundValue::{Symbol(BoundSymbol), Json(BoundJson)}`. `BoundJson` is an opaque
file/value handle over the retained `JsonFacts`. Existing NotDemanded,
Unresolved, GlobalThis, library-member and unsupported outcomes stay distinct.
Type-only inspection still follows the target's value type without authorizing
runtime use.

Selection should use `SelectedSeed::{Variable { symbol, declaration },
Json { value }}` instead of a declaration option whose invariant every caller
must remember. Canonical value identity, not display origins, collapses aliases.
The binding/selection owner will integrate this after the first ordinary-native
GREEN window. After the shared request-owned Evaluator refactor, JSON checking
now lives in `semantic/evaluation/json.rs` and uses evaluator/model operations
through that owner. Shared model, selection and binder changes stay coordinated.

The retained JSON owner exposes complete borrowed value/property operations.
Object views preserve first distinct-key order, last value, and every exact
origin. Origin aggregation uses already-established duplicate relationships;
it must not decode keys or infer identity a second time. No JSON AST collector,
second binder, flattened reference-result join or source reparse is needed.

## First vertical and rule expansion

The first behavior-level `tests/type_json.rs` RED selects ordinary named aliases
and renamed/type-only aliases of imported JSON scalar/object properties through
the complete public `Project::type_of` API. The existing pending JSON binding
result ensures a real unsupported-semantic failure, not a missing-import/setup
failure. Expectations come from the frozen JSON native controls; additional
fresh native alias controls will pin the exact new fixture before the RED.

Start with widened string/number/boolean aliases, top-level/nested last-value
duplicates, decoded/empty/numeric/astral exact names, nested object fields and
all duplicate key origins. A changed JSON disk file after Project loading must
leave the old generation's type and origins unchanged. No scalar literal
payload is needed for this imported widened-value question. Existing Model
primitive/object operations are sufficient for that first family.

Subsequent required branches use the same operation and public test file:

| Branch | Native fact/rule obligation |
| --- | --- |
| Whole root, synthetic default and namespace routes | Existing JSON `export=`/synthetic-default binding decisions; a literal `default` key remains distinct. Selection must use its own compiler alias table. |
| Null and empty roots | `getWidenedType(getWidenedLiteralType(...))`, strict-null-dependent genuine `any` versus `null`; empty/trivia root is the compiler's empty object. |
| Empty/numeric/mixed/recursive arrays | Ordered child facts, general array/union type constructors and `checkArrayLiteral`/widening rules; strict-null empty `never[]` versus genuine `any[]`. |
| Arrays of unlike objects | General `getWidenedTypeOfObjectLiteral` context/missing-property construction, with exact-optional missing slots distinct from explicit undefined. JSON must not own a second union/object algorithm. |
| Element/property reads | Existing semantic expression/member owner reads the constructed type; unchecked-index behavior must not be implemented in JSON import construction. |
| Missing/unknown/genuine any | Preserve absence and native recovery/admission provenance; never manufacture `any` as a fallback or equate a false root error flag with a healthy journey. |
| Unrepresentable exact keys/optional JSON grammar | Retain exact facts; pin the native demanded branch and supported-profile failure/partial contract before admitting it. No lossy key conversion or silent member omission. |

The frozen eight option combinations include six admitted native profiles and
two native Program TS5052 combinations (`exactOptionalPropertyTypes:true` with
`strictNullChecks:false`). That program-diagnostic boundary is separate from
Rust Project configuration loading. The lane must preserve every option state;
it must not add a JSON-only configuration rejection, claim the two raw checker
controls are healthy oracle profiles, or resolve global admission policy inside
JSON widening. Current construction/read distinctions remain explicit.

## Shared type model and presentation

Null/array/union branches require coordinated general Model constructors and
immutable TypeViews for special types, arrays and unions. Missing optional slots
need their native semantic distinction, not a summary string or an undefined
substitution. The model owner owns identity, interning, union ordering/reduction
and widening operations; the presentation owner renders established views and
graph omissions without acquiring JSON syntax or evaluating child types.

Primitive/object JSON first can proceed behind the current interface while
those additions are pinned. This sequencing does not reduce the selected scope:
the later branches, contextual/flow/member relationships and full presentation,
worker/resource/refresh obligations remain mandatory. Native JSON successes
will be described only for actually executed final-result tests, with compiler
provenance and meaningful failure variants. Representation and semantic gates
cannot be closed by this one leaf family.

## Module-wide demand and first fixture evidence

The pinned Go source settles the named-import dependency. In
`checker.go:getTargetOfImportSpecifier` (around 14628), an `export=` module
resolves a named import by `getPropertyOfTypeEx(getTypeOfSymbol(targetSymbol),
name, ...)`. The JSON SourceFile branch in
`getTypeOfVariableOrParameterOrPropertyWorker` (around 16502) checks and widens
the root expression. `checkObjectLiteral` (around 13172) checks each source
property assignment before the final property table is established, including
initializers overwritten by later duplicate keys. A child-only recipe is
therefore not an admitted implementation of this alias question.

The coordinated first operation constructs the root dependencies in source
order, retaining established child identities in the request model, and then
returns the canonical imported value's established identity. A pending sibling
or discarded duplicate remains a failure at its JSON source. No absent child
is manufactured into an ordinary type.

The rich fixture `fixtures/types/json-native` remains unchanged. A separate
`fixtures/types/json-native-basic` preserves the selected aliases and exact raw
key positions while replacing sibling null/array values with scalar/object
values. Each fixture has 88 original fresh pinned raw API controls: eleven aliases
times eight option states, 66 input-diagnostic-clean rows and 22 rows with native
Program TS5052. Those original scripts collect diagnostics after demand, despite
the earlier incorrect pre-demand label. Additional independently fresh baseline
controls collect config/program/syntactic/bind diagnostics before demand and
agree on values/origins and the 66/22 split. Separate manifests retain their
exact input/tooling hashes and snapshots; the original artifacts are unchanged.
Post-probe hashes are inspection provenance, not transactional compiler reads.
Neither diagnostic ordering inventories all checker recovery dependencies.

`tests/type_json.rs` pins first-family native outcomes against the basic
fixture and now requires consumed InvalidSemantic for a raw non-error scalar
result in the rich noLib module. Its empty-array sibling requires a missing
global Array target. Genuine targets still require full generic/member and
structured contextual construction; the raw fallback cannot become a positive
admission expectation. The first attempted scoped test run,
`vrun-3efe7dae-7141-4a2a-87f7-8a616bd92006`, stopped at an in-flight shared
presenter signature compile error and is setup evidence only, not behavior RED.


## Array recovery boundary discovered during implementation

The rich noLib fixture is not a positive admission proof. Every imported named
property requires its root module type, and its array siblings consume global
Array recovery. Pinned checker `initializeTypeCaches` eagerly asks
`getGlobalType("Array", 1, true)` (around 1350). `getGlobalType` (around 1210)
requires a Type-meaning class/interface of arity 1; absence, wrong kind or wrong
arity reports a checker diagnostic and returns `emptyGenericType`.
`createArrayTypeEx` (around 24613) passes that identity to
`createTypeFromGenericGlobalType` (around 24576), which returns `emptyObjectType`.
A raw `{}` root with `IsErrorType:false` is therefore not a healthy ordinary
object in the product's consumed-invalid-dependency policy. Four input-only
diagnostic families cannot prove otherwise.

The additional twelve noLib controls enumerate empty array-result members,
while ten default-library controls from `json-types` enumerate the real Array
member/signature/index view. No library-based shortcut or empty guessed Array
member view is authorized. Canonical global lookup must retain actual target
kind/arity/admission facts because user global declarations remain possible
with noLib. Full array/contextual/native member construction remains open.
