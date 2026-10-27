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

## Initializer widening

`getWidenedLiteralTypeForInitializer` keeps an initializer's fresh literal type
only for a constant declaration or one that `isDeclarationReadonly` accepts. That
predicate is a combined `readonly` modifier on anything but a parameter
property, so `constructor(private readonly key = KEY)` widens `KEY`'s `"value"` to
`string` while a class's `readonly key = KEY;` keeps `"value"`. Before this fix the
variable facts read the bare `readonly` modifier. A readonly parameter property
whose default was a string, number, boolean, enum-member or template constant kept
the literal type, so a call that passed another value consumed overload recovery,
and member searches through that call reported an `overload_recovery` gap. The
predicate now has one owner, `syntax::is_declaration_readonly`, which the
variable facts use. Other readonly checks answer different questions
(`isValidESSymbolDeclaration`, `isReadonlySymbol`, the pseudochecker's
template rule) and are unchanged.

`fixtures/types/native-initializer-literal-widening` pins 69 native 7.0.2 answers.
The sources are a same-file constant and one reached through a named import, a
renamed re-export, a star re-export and a namespace import. They also include
`let` and `var` sources, number, boolean, enum-member, enum-access, plain and
substituted template constants, `as const` values, and a fresh object's property
beside a readonly object-type property. Each source is used as a `private readonly`
parameter property's default and as a readonly class property's initializer. The
declaration forms, each with an imported constant, are parameter properties with
`private`, `protected` or `public` beside `readonly`, plain `readonly`, `override
readonly`, `private` and `public` without `readonly`, a plain constructor parameter,
function and arrow parameters, static readonly and mutable class properties, and
`let` and `const` variables. Both sides of each parameter-property pair are
asked: the parameter at its name, and the property through `this.name` inside a
method and, for the public ones, from outside the class. The calls that pass
other values must resolve.
`fixtures/types/native-initializer-literal-widening-call` is the smallest shape:
an imported constant as a readonly parameter property's default, and a call
passing another string constant.

`tools/type-initializer-literal-widening-oracle.test.mjs` takes every summary and
member search from the compiler hook, which answers a parameter-property question
the public API cannot. `tests/type_initializer_literal_widening.rs` asserts those
summaries through `Project::type_of`. It also checks that `Forms.describe`'s
member search is complete with the native occurrences, and that the CLI's
`--type` and `--references` answer the small shape. All three tests failed before
the fix: every readonly parameter property kept its literal, and the search
reported the `overload_recovery` gap at the call.
