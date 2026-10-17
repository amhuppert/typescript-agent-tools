# NoInfer index controls

Pinned TypeScript 7.0.2 `getIndexTypeEx` first reduces its operand, then treats
`NoInfer<T>` separately from ordinary substitution types. Its recursive NoInfer
branch preserves index flags and reapplies `getNoInferType`; ordinary conditional
flow substitutions continue to `shouldDeferIndexType`. The same source's
`getSubstitutionIntersection` returns a NoInfer base directly and otherwise
intersects constraint before base (`internal/checker/checker.go` 26555 and 26693).

These 14 roots distinguish object, union, intersection, Any, Unknown, Never,
empty-object, unique-symbol, and generic keys; accepted and rejected overload
arguments; and both ordinary conditional branches. `unusedMember` must remain
healthy even though its unacquired property value names a missing type. The
existing `native-mapped-deferred-index` fixture supplies the recursive ordinary
substitution control that exposed the former unconditional substitution guard.

The registered harness compares cold compiler-hook and public compiler API
queries for strict, loose, and noCheck profiles. All 42 captures matched, with
no queried error types, diagnostics, or observed erroneous type acquisitions.
Bootstrap `vrun-216aa068-1a65-4b2a-81b0-e6b70d0888f0` failed only because the
three maintained expected files did not yet exist. Acquisition observations
are native evidence; they are not asserted as product cause counts.

Maintained replay passed all 42 observations:
`vrun-8ccee0c6-fde7-4c17-ab31-0fc7ffb77d5c`.
