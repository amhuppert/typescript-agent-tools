# Native literal increment

The first string/number family now uses the parser's complete cooked string and
scanner-normalized numeric payloads. Source acquisition performs no second raw
escape decode, radix classification or decimal conversion. Evaluation and the
type model retain their own literal-type and freshness operations. This bounded
increment does not implement all constant evaluation, bigint/unary/arithmetic,
template interpolation, type operators or the complete T/Q3/Q4 profile.

Using Ousterhout's ownership test avoided a separate constant module for two
already parsed payloads. The existing `printer.EscapeString` operation moved
intact from navigation names to `src/printer.rs`, including its private tests.
Navigation and semantic literal/property summaries now use that one policy;
generic CLI JSON quoting is a different display operation. This extraction is
mechanical wiring; no new behavior RED is claimed for relocating the function.

## Compiler controls

`fixtures/types/literals` contains 17 string variants and 22 number variants,
plus distinct unpaired high/low UTF-16 controls. Strings cover hex/Unicode/braced
escapes, an astral surrogate pair, NUL and NUL-before-digit, control characters,
quotes/backslash, identity escapes, U+2028/U+2029/U+0085, and line continuations
after LF, CR, CRLF, U+2028 and U+2029. Numbers cover decimal fractions, leading
and trailing dots, exponent case/sign/separators, both cases of hexadecimal,
binary and octal prefixes, separators, rounding beyond 2^53, large/small
scientific notation, overflow to Infinity and underflow to zero.

Forty fresh TypeScript 7.0.2 API requests pin value-question summaries and
absence of selected error types. The Infinity case uses the existing pinned
same-checker Go hook: the public API cannot transport a float64 +Inf and panics
while writing that type's response. The hook returns the actual compiler's
`Infinity` summary, non-error root and no demand failures. Its ten TS2318 global
diagnostics are the fixture's intentional `noLib` missing globals, outside this
literal demand. These are not silently removed or treated as demanded recovery.
All four registered compiler tests passed without skips in
`vrun-91fe7eb6-9367-40ee-903f-86b1b0607874`.

Initial compiler control runs corrected test setup (an unavailable fixture
generator command and a prefix-based anchor), expected NUL/vertical-tab/UTF-16
escape spelling, the public API's nonfinite transport limitation and the
hook's separate global diagnostics. None proves missing native behavior.

## Native proof

`tests/type_literals.rs` asks final `Project::type_of` questions at imported
alias tokens. It asserts exact root kind/value/summary, selected location,
complete depth-zero graph and located unrepresentable-unit failures. The first
run used relative file operands from the wrong working directory and failed
setup; it is not behavior RED. Corrected run
`vrun-23d18bcc-ea38-41c3-8dad-2a70f6fb09ce` passed the two distinct unpaired-unit
controls and failed three missing behaviors: escaped literal cooking, numeric
normalization variants, and the already admitted raw U+0085 summary's incorrect
generic JSON escape policy. All four tests passed without skips in
`vrun-4786237a-98d4-4590-a9de-457fd298d375` after the literal and shared printer
changes. An intervening in-flight signature mismatch was only a compilation
failure and is not native literal evidence.
