# Template completion controls

These cold TypeScript 7.0.2 queries compare the instrumented compiler's returned
type with the public semantic API. They capture returned error-type acquisitions
before summary rendering; diagnostic codes alone do not establish acquisitions.

`equalSpansTwo` observes the source and target constraint for the first hole,
then the source and target constraint for the second hole. This interleaving
matches `inferTypesFromTemplateLiteralType`'s complete equal-span prepass.
`equalReject` still observes its later pair although validation rejects the
first string-to-number placeholder. The native return types remain 12 and 51.

`inferSame` observes source before target; `inferCross` observes target before
source. Each ordering belongs to its reached native operations. Generic mapping
observes only its source constraint and does not prove a later target read.

`validationReject` and `validationContinue` both acquire their target constraint.
The first therefore does not prove absence of that dependency, despite the
matcher's first-false validation short circuit. Its named constraint can be
acquired elsewhere in argument checking. Healthy simple/nested mappings prove
the final fixed-point versus changed-string decisions; they do not prove
recovery in a mapping factory. Existing matching fixtures cover the pure
empty/nonempty delimiter, primitive, intersection, and exact-byte branches.

Every row is captured under strict, loose, and noCheck profiles. Public recovery
assertions should preserve the observed dependency order, but should not equate
native hook event counts with product cached-cause counts.
