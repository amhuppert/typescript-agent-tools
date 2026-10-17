# Native unary and binary operator continuation

Pinned TypeScript 7.0.2, cold checker hook and cold compiler API, under strict,
loose and noCheck. Each recovery root retains two generic constraints until an
operator asks for them; the hook observes already-returned error types without
creating extra checker demands. Generic identifier reads would acquire these
constraints earlier and would not isolate operator checking.

The six unary roots use comma expressions to require the first recovered value
to permit checking the second operation. Subtraction, addition and relational
comparison acquire both constraints. Equality is checked both as a generic-call
argument and as a variable initializer; both are Normal-mode observations. The
initializer row does not establish the separate TypeOnly suppression rule.
Compound subtraction acquires the getter constraint before its missing setter
type. All twelve roots return non-error native types despite their consumed
errors. Addition returns Number for these recovered generic operands.

The four controls preserve direct numeric/BigInt literals, Boolean bitwise
Number, and relational Symbol exclusion. Symbol exclusion emits diagnostic
2469 but does not acquire the right operand's lazy missing constraint. Diagnostic
codes such as 2357, 2447 and 2873 are not themselves native errorType results.

Corrected capture: vrun-733b3e5d-78f3-4a6f-92ca-3b46b6c1bb70, all 48 queries
and compiler API parity complete; bootstrap failures were absent expected files.
The captured baselines are maintained by:

    node --test tools/type-native-operator-completion-oracle.test.mjs

Public continuation tests use exact missing-token locations and ordered later
causes, not an assumed equality between native exit counts and product recovery
counts. Production operator changes require a public RED first.

Maintained replay: vrun-19de2c4d-cb33-441d-bbc3-676938c0662b PASS, three
executed tests, zero skips; all 48 cold native/API observations match the baselines.

Public operator continuation GREEN:
vrun-053f31e4-5658-4e35-940f-deb86bacb500 executes both
`tests/type_operator_completion.rs` tests successfully. The recovery test checks
all twelve roots across strict/loose/noCheck, repeated requests, reversed query
order and compact/expanded results; the four literal/bitwise/Symbol controls
also pass. The earlier behavior RED
vrun-8ef04bcd-d60e-4e1c-99ae-8725cf4ba4b5 failed because unaryPlus retained
MissingPlusFirst but lost the later MissingPlusSecond acquisition. The bounded
repair sequences existing completed unary/operator dependencies and retains all
native short circuits. The same combined run exposes separate stale graph-label
expectations in type_order; that does not negate its completed operator tests.
