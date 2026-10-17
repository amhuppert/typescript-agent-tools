# Native destructured assignment dependencies

This increment runs inside one immutable captured Project and one request-local
Evaluator. It extends the existing `getInitialOrAssignedType` owner; it does not
admit changed use-site flow types or create a second binding-pattern collector.

The full compatibility run `vrun-1ae7a98d-1f41-45eb-8ab1-1ed2901aa95e`
reached an earlier blanket `flow_required` expectation after assignment
reduction began demanding its real producer. The actual missing array-element
operation is earlier than final read admission. The old assertion does not
describe the pinned compiler's dependency schedule.

Two approaches were considered. Detecting patterns in the flow reader would
couple reference matching, source parents, defaults and native iteration rules
at every consumer. Instead, Source supplies the actual recursive assignment
question and the Evaluator executes the complete initial/assigned operation.
Source owns accepted syntax and exact child identity; type/member/iteration
owners retain their own decisions. Recursive grammar edges never become types
or guessed property identities in Source.

`getInitialTypeOfBindingElement`, `getAssignedTypeOfBinaryExpression`, array
elements, spread elements, property assignments and shorthand defaults retain
their different parent questions. Native defaults acquire the source type
before the default expression and union the non-undefined source with the
actual default type. A present default is not a branch that skips an erroneous
source type. ForIn supplies String; ForOf asks its actual iteration operation.

`getLiteralTypeFromPropertyName` is a checker operation distinct from the
binder's `getDeclarationName`. Numeric and computed names check their actual
expression; ordinary property names construct exact string literals; private
or unnameable syntax follows the native Never result. A demanded invalid or
unsupported computed name keeps its own cause. The existing canonical member
factory supplies property identity, and index applicability belongs to the
shared relation owner. Names never pass through lossy decoding.

`getTypeOfDestructuredArrayElement` asks tuple-like projection first, then the
ordinary destructuring iteration question and its no-unchecked-index policy.
Spread asks iteration before creating the real global Array reference. Property
projection asks the regular property question before applicable index lookup.
A missing property/index consumes ErrorType rather than returning Any or
Unknown. Every native recovery dependency remains an invalid product result,
even if native assignment reduction later displays the original declaration.

A source-pinned fresh compiler hook confirmed both strict/loose controls:
an `any` array assignment retains the declared `string | number` identity;
an `any` spread assignment constructs Array and retains the declaration through
ordinary rejected assignment; object assignment from `any` consumes one actual
assigned errorType before displaying that same declaration. The six raw
observations are in `.cc/temp/types-assignment-destructure-probe.json` and are
research evidence, not a final production pass. A minimal real global Array
declaration is in the fixture's separate script `globals.d.ts`; the remaining unrelated
global diagnostics do not determine query success.

The initial compiled assertion RED is
`vrun-5b2674c2-d010-48d7-b3af-1c06b017a152` (five tests pass, one missing
array projection). The first grouped attempt was a compilation failure, not
behavior evidence. A subsequent compiled attempt
`vrun-c942202e-9932-4e19-8998-ce6654ebce3f` reached actual missing-global Array
recovery: the first fixture declaration was mistakenly module-local. Moving
that declaration to its own accepted script fixes the input premise, without
changing the consumed-global recovery policy. Six new same-checker observations
after that move retain the actual assigned ErrorType only for the object
control and use a real Array for spread. The final product retry is pending.

Typed property,
array/tuple/iterator, defaults, rest, nested patterns and option-dependent
projection controls remain required. This staged operation does not close the
full type/iteration, structured relation or resource gates.

The corrected basic-flow compiler baseline passed in
`vrun-a45be6ed-3f4f-48d7-951c-655547e81f2d`: 56 selected controls over
strict and loose null settings, with zero skips. A separate typed-destructuring
fixture now supplies real global Object, Array and ReadonlyArray declarations.
Its three compiler tests passed in
`vrun-60488dde-5013-4761-bd53-ac46d7820158`, with zero skips. The matrix
contains 33 fresh public summaries, 33 separate cold selected/declared TypeId
identity checks, and 33 same-checker dependency observations. Own, shorthand,
numeric, cooked and computed property names; defaults; array elements and
rest; omitted positions; and ForOf all preserve the declared union while an
unresolved sibling stays unvisited. This is compiler evidence; the six final
Project tests have not yet supplied their native RED/GREEN receipts.

The next shared property operation ports `getPropertyOfTypeEx`'s own-table,
Function/CallableFunction/NewableFunction, then Object schedule. Global lookup
does not recursively augment those global inventories. StrictBindCallApply
chooses the named callable/newable global; disabling it chooses Function.
`getIndexInfoOfType`'s exact key question remains separate from
`findApplicableIndexInfo`'s ordered non-string matches and string fallback.
Multiple applicable values use the shared native intersection factory.
Iteration preserves its native use flags: ForOf permits strings while
destructuring does not, and possible-out-of-bounds policy belongs to the
particular iteration or projection question rather than all consumers.
