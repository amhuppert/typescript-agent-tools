# Native cached mapping resource controls

Each pair warms the same 16- or 256-member numeric union and scalar, then checks
eight eager object literals whose property value is either the scalar or cached
union. Both end with a declared number. Names/operands have equal lengths and
all roots share one captured source.

Native checkPropertyAssignment/checkExpressionForMutableLocation calls
getWidenedLiteralLikeTypeForContextualType. With no literal context,
getWidenedLiteralType and getWidenedUniqueESSymbolType each map every union
member. Regular literals survive unchanged, but mapTypeEx still traverses and
allocates its mapped scratch list before returning the existing type. Pinned
checker.go25395/25413/25423/25473 own these decisions.

The maintained hook now asserts each of the eight actual object-literal
checkExpressionEx exits, so a deferred void wrapper cannot silently bypass the
intended work. It pins the final number, no erroneous acquisitions/instantiations,
and ten diagnostic-only TS2695 entries for each pair (none for the standalone
healthy control). Native strict/noCheck traversal validation passes in
`vrun-defc4c98-8d98-45b7-97d2-44b503dcf6bb` (both map tests; the same run's
filter assertion was corrected separately to native's sixteen passes).

The earlier void-wrapped observations did not execute mutable-location widening;
their resource-omission claim is withdrawn. Original inputs/observations remain
under `.cc/temp/stage67-deferred-void-*`. Production mapping accounting and the
corrected final-query omission discriminator await the shared checker assembly.
Other object-literal and regular-literal operations remain separate owners.

cases.ts SHA-256: `e3f046864af03e09b86abad6003c9618e9aea512c8a9184c050eb21c8cd92858`.
