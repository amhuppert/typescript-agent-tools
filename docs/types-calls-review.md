# Bounded native call review

This read-only Ousterhout review assumes one immutable accepted Project and
one request-local evaluator. It covers `source/calls.rs`,
`evaluation/calls.rs`, signature call-slot construction and the shared
type-resolution window at the first five-test call checkpoint. It does not
claim the generic, contextual argument, overload, optional, new, tagged or
decorator families are complete.

No additional incorrect successful answer was found in this admitted slice.
The distinction between quick initializer typing and checked expression
typing is necessary: pinned `getQuickTypeOfExpression` at
`internal/checker/checker.go:7338` obtains a sole nongeneric return without
checking the call's arguments or arity. The implementation retains that
question separately and does not install a resolved call cache. Actual
checked calls acquire the callee, non-null/apparent type and complete
signature inventory before candidate selection; absent primitive/global or
composite structure remains a consumed pending dependency.

The complete captured call recipe hides final-node attachment and preserves
type/value argument order without reproducing binding. Its parser-established
closing parenthesis justifies excluding native recovery-only incomplete
argument-list behavior in this first accepted syntax family. Global Symbol
and require/import/super exceptions retain their distinct semantic questions;
a spelling candidate is not treated as an established global identity.

`hasCorrectArity` preserves native parameter-count/minimum-count order,
spread handling and omitted Void acceptance. The minimum-count cache is keyed
by the actual immutable SignatureId. Its backwards scan consumes call-slot
types, independently of displayed parameter types; Undefined and Any do not
become Void. The zero-argument applicability path still consumes explicit
this before arguments, but does not demand an unused optional parameter.
Missing parameter or callee work stays its original failure. An arity failure
does not visit a missing extra argument merely to make the eventual failure
look more complete.

`getResolvedSignature` preserves the native resolutionStart window: only a
previously uncached call resets the lower bound, a re-entrant resolving call
uses the existing window, and an inner installed signature wins over the
outer local result. The shared owner restores the previous window after both
success and failure. Product Failed state deliberately retains the original
cause; it is not a healthy native resolving/any sentinel.

The canonical empty-object apparent type is a justified intrinsic identity,
separate from a fresh empty object literal and missing-global recovery.
Type-parameter apparent typing consumes the complete base-constraint
operation, retaining CircularConstraint and other failures. An actual absent
native constraint supplies Unknown; no failed constraint becomes absence.
Calls and effects share signature inventory acquisition without copying
apparent-type or structured-member rules.

The final public five-test gate passed with zero skips in
`vrun-60d75191-1286-4070-9678-ed656c64997b`, following the genuine
missing-behavior RED `vrun-75a126e7` (full original receipt retained by the
call owner). This source review does not replace those final outcomes.
Actual SignatureReturn identity cycles are being independently tested and
remain an open coupled owner. Native temporary flow-loop cache suppression,
generic inference, candidate reordering, checked argument relations, wrapper
globals, composite signatures and all other pending call dispatch families
remain mandatory work; their explicit failures are not delivery exemptions.

## Explicit generic candidate follow-up

A later bounded read covers the current `check_type_arguments`,
`get_signature_instantiation` and generic `choose_overload` increment against
checker.go:9198 and 19198–19243. It found no additional incorrect successful
answer in the admitted explicit-argument family. The full source argument
vector is acquired, all omitted defaults are filled, and only actual supplied
type-argument nodes consume their parameter constraints, in that order. The
constraint mapper is created lazily after a nonnil constraint; actual this
argument adjustment follows instantiation. Ordinary relation False continues
candidate selection, while consumed failures keep their location and cause.

The completed instantiation cache key is actual SignatureId plus ordered
canonical argument TypeIds. It calls the erased signature factory; arbitrary
member-signature instantiation remains the separate native operation that
allocates each invocation. The native inferred-return clone branch is not
reached by this explicit family. At review time its genuine final generic5 RED
has run; the final five-test file then passed with zero skips in
`vrun-02c482de-8ebc-4434-b45e-b378dd2cdcab`.
Default schedule controls alone do not prove full inference, contextual modes,
diagnostic recovery candidates or every relation family. Newly reachable
intersections must extend the existing whole getTypeWithThisArgument
same-map/rebuild branch, rather than inherit the scalar/reference proof.
