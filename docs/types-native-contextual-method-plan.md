# Contextual methods and inference completion

The operating envelope remains a local immutable Project and request-local
checker. This is a required inference operation, with no new runtime service or
public option. The implementation is staged; validation below is scoped evidence.

TypeScript 7.0.2 `inferFromIntraExpressionSites` calls
`getContextualTypeForObjectLiteralMethod` for method declarations and
`getContextualType` for other sites. The former deliberately bypasses general
context-frame lookup. Both use NoConstraints; each inferred source is consumed
before the next site, and the site list is cleared after a completed pass.

The existing object-element owner already retained the method InWith gate and
excluded method return annotations from property annotation overrides. Routing
methods through the general expression-context operation would lose those
rules. Copying element lookup into inference would duplicate computed-name,
property, index, and mapped-type decisions. The chosen change extends the shared
owner and calls it directly from inference.

Its previous Result-only interface admitted a recovered type before subsequent
native work could run. The contextual element/property/index owners now return
NativeCompletion and preserve each dependency in native order. Nullable union
maps retain a separate sequence per constituent. Their mapped substitution and
conditional exclusion dependencies use the same completion contract. Conditional
true/false/inferred/default branch caches now retain completed values and their
evidence together. Existing admission adapters serve callers that have not yet
migrated; they never turn an interrupted computation into a value.

The initial native matrix contains eleven queries in strict, loose, and noCheck
profiles: methods, arrows, function expressions, computed/numeric/unique-symbol
names, an explicit method return annotation, producer/consumer ordering, mixed
syntax, missing return dependencies, and a later healthy query. It passes in
`vrun-714649f3-48a4-4a32-955b-99db594141f8`. The consumer-first case returns
unknown; the missing value returns native errorType. These are compiler results,
not expectations inferred from the new implementation. The final Rust test
repeats compact/expanded requests and both query orders. It passes in Stage58
(`vrun-8fa5ac6e-69b1-452b-ad98-be003e059f21`) after the remaining generated
property lookups were migrated to structured keys.

The separate contextual-recovery test also passes
(`vrun-c28e7fd8-5bc0-4d05-935f-8b5d830c3ce7`): generic call results whose
native final values are 41/42/43 retain both missing property, index, and mapped
dependencies at the product failure boundary. A later healthy query stays
healthy in both query orders. The computed-name row remains excluded from that
Rust proof until its expression acquisitions are independently observed.

Contextual union discrimination exposed a further admitting Result boundary.
The final-query test fails after `missingBrokenTag`, omitting `MissingFirst`
and `MissingSecond` that native acquires later
(`vrun-23cd58ae-2246-4fde-8713-af79b302f952`). Its staged migration includes
the discriminator cache, context-free expression cache, shared lazy comparison
loop, and apparent-context caller together. The original TypeDiscriminator
caller retains an explicit admission adapter around its older callbacks; the
comparison algorithm itself has one owner. Completed values and recovery share
the existing cache slots; no provisional native result or new context frame is
introduced. The final-query regression now passes
(`vrun-571e1686-6e3a-4995-b79a-363304db3715`).

The union fixture has nineteen queries in strict, loose, exact-optional, and
noCheck profiles. It covers selected and missing tags, a missing shorthand type,
absent/present optional tags, multiple methods, nine/ten/eleven-constituent key
maps, duplicate keys, and a later healthy result. Duplicate controls add another
constituent and are separate from the threshold counts. Native expression
acquisition is an opt-in extension of the existing diagnostic observer: it
reads only arguments and already-returned types. It records missing tag recovery
and context-free cache hits without making an additional checker demand.
All 76 native observations pass in maintained replay
(`vrun-b1dab906-77e7-42e4-9d7a-636b8f1aed29`). Product assertions cover final
types and exact first/later recovery locations;
they do not infer product cause multiplicities from nested hook event counts.

Older mapped-instantiation and conditional-instantiation admission boundaries
also remain separate required work; the branch-cache migration does not claim
those complete operations.
