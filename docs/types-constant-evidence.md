# Native constant-expression increment

The native evaluator now admits boolean literals, no-substitution templates,
bigint literals and the proved prefix-unary scalar family. This continues the
complete T/Q3/Q4 delivery; it does not establish general constant evaluation,
enums, binary operators, interpolated templates, contextual signatures, flow
admission, compiler recovery or semantic resource gates.

## Ownership

Source recipes retain parser-owned cooked template/string text and normalized
scanner payloads. `checkExpression` owns their fresh literal types. Bigint
construction ports `jsnum.ParsePseudoBigInt`, `NewPseudoBigInt` and
`PseudoBigInt.String` in `semantic/constants.rs`, delegating arbitrary-precision
integer conversion to pinned `num-bigint` 0.4.6, as the Go compiler delegates to
`math/big`. This avoids a second integer implementation and never rounds through
a floating-point number. Its graph value is canonical signed decimal text
without `n`; its summary adds `n`. Number graph values use the scanner's canonical
numeric text. Both numeric and bigint signed zero share the value `0`.

`checkPrefixUnaryExpression` tests the direct operand's syntax kind before its
literal shortcut. Consequently `-1` yields literal `-1`, `-(1)` yields number,
`-1n` yields literal `-1n`, and `-(1n)` yields bigint. General minus/tilde result
typing and the Truthy/Falsy mask of `getTypeFactsWorker` consume established
types. They do not execute application code. Empty anonymous objects permit both
truthiness facts, so `!{}` yields boolean even though a runtime object is truthy.
The strictNullChecks-dependent base facts retain that option input: non-strict
true/nonzero/nonempty literals include nullable Falsy facts, so their negation
is boolean. Additional reachable type families must extend these operations with their
compiler dispatch rather than inherit an approximation.

## Compiler and native proof

`fixtures/types/constants` contains 55 expression variants. Controls include
both booleans and mutable widening; cooked escapes and raw LF/CR/CRLF/U+2028/
U+2029 templates; decimal/hex/binary/octal bigint forms with both prefix cases,
separators, leading radix zeros and magnitudes beyond fixed-width integers;
negative bigint zero; direct versus parenthesized/nested unary forms; numeric
radix/exponent/underflow cases; and literal and empty-object truthiness.

Two fresh TypeScript 7.0.2 compiler tests passed without skips in
`vrun-344c8799-ee43-4199-90be-c1e1bb5253d8`, pinning selected value-question
summaries, non-error roots and the absence of an enclosing-call prelude. The
initial control's runtime-based expectation for `!{}` was corrected to the
compiler's boolean answer; that expectation correction is not a production RED.

Final Project tests ask captured imported aliases and assert selected outcome,
exact summary, complete depth-zero kind/value graph and no issues. The initial
two-test RED `vrun-52bdd5c4-1569-4f5d-8517-18aed8894980` reproduced pending
booleans. A refined five-family RED
`vrun-c2a56df5-0ae2-470b-af25-6e7e8bdac6a6` failed independently for bigint,
booleans, templates, negation and numeric unary forms. After the native changes,
all five tests passed without skips in
`vrun-32ee2dea-047f-445e-a115-adea7494deaa`.

Ten fresh non-strict controls passed in the expanded three-test compiler run
`vrun-ae2e8470-0d64-418c-ae56-ed98347d6ce3`. The sixth native test reproduced the
option-dependent defect in `vrun-ed268d2b-fe91-4510-9c1d-abec8751c9fb`:
`!true` returned false where the non-strict compiler returns boolean. After
moving the whole base-facts decision into the existing truthiness owner, all
six tests passed without skips in
`vrun-2129eda1-420e-4ded-b54b-fd33b9ea3ef5`.

The independent Ousterhout review in `.cc/temp/types-constants-review.md`
found no blocker within the proved initial family. It warned that future
callable/indexed/class/Array views must not infer empty-anonymous-object facts
from property count alone; that remains a separate implementation obligation.
Its optional cleanup was applied: bigint normalization no longer accepts an
unused sign argument. The existing unary sign/zero owner makes that decision.

Position selection is a subsequent proof gate.
This increment does not promote a punctuation token to a new product anchor,
silently bypass an enclosing-call checking dependency or authorize unsupported
member/module-name tokens as literal expressions.
