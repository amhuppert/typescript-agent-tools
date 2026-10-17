# Conditional completion controls

Pinned 7.0.2 `getConditionalType`, `getConditionalTypeInstantiation`, and
`getTypeFromConditionalTypeNode` own conditional evaluation and root caching
(`internal/checker/checker.go` 24208, 22391, and 24177). The controls distinguish
both branches for Any, union distribution, lazy unselected branches, exact native
errorType versus unresolved-alias Any, inference, immediate and aliased tail
transitions, repeated root instantiation, and mapped-key lower bounds.

The mapped cases use the non-keyof mapped-syntax path in
`resolveMappedTypeMembers` (20884), which calls `getLowerBoundOfKeyType` on the
conditional constraint. The erroneous key is acquired before the erroneous
mapped value; the equivalent healthy query returns 44.

The registered harness compares every cold compiler-hook query with the pinned
public compiler API, then preserves summary, errorType, accumulated diagnostics,
and zero-demand observations of already-returned erroneous types. A healthy
tuple can still depend on erroneous types, so final errorType alone does not
classify a product query. Acquisition event totals are not product cause counts.

All 15 roots in strict and noCheck matched the maintained baseline:
`vrun-27f8683d-8090-4893-8243-abd2806ce36f`. The separate bootstrap capture
`vrun-7954b855-2de7-4286-bfc5-58df1164c931` completed all observations and failed
only because the two expected baseline files had not yet been installed.
