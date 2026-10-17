# Nongeneric assertion-predicate tail

This two-root control isolates checker.go 8330 from generic signature mapping.
Both calls resolve the first of two nongeneric overloads. Multiple signatures
defeat getQuickTypeOfExpression, but the chosen signature needs no generic
instantiation that could acquire the predicate before the reporting tail.
Only the statement call has the actual ExpressionStatement parent.

The native hook and ordinary API are compared before baselines are accepted.
The existing getter entry/exit trace identifies actual asserted-type reads;
source-sorted diagnostic codes are membership evidence only. Capture
`vrun-b980dcb0-07c5-44f3-96fe-71eb49bfc35a` confirms the statement alone acquires
the `MissingTailPredicate` type node and reports TS2304 in both profiles. Both
calls still return genuine Void. The returned-call control does not acquire
that type node. This matrix leaves the already captured generic controls unchanged.
