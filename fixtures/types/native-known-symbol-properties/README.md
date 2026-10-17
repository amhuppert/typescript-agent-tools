# Known symbol property compatibility

Pinned TypeScript 7.0.2 `internal/checker/relater.go` `isKnownProperty`
(lines 719–742) checks ordinary object members, an applicable index, the
`isLateBoundName` plus string-index compatibility exception, then JSX names.
Eligible composite targets recurse through the same operation. The symbol-name
exception is specific to this question; it is not an element-access index rule.

The overload calls turn excess-property acceptance into a final inferred literal
type. They distinguish string, symbol, number, absent, and exact-member targets,
plus union/intersection callers, ordinary names, numeric names, a different
symbol-property value type, and a method. Element-access controls record both
the returned type and diagnostics: a native diagnostic alone does not establish
that the returned type is the compiler's error type.

`tools/type-native-known-symbol-properties-oracle.test.mjs` queries each marker
with a fresh cold compiler hook and the pinned public compiler API, asserting
summary/error-type parity before checking the maintained baselines. The strict,
loose, unchecked-index-access, and noCheck profiles retain the option states
separately. No generated numeric symbol spelling is an expected product value.
