# Source-free signature recovery

The current unknown-signature singleton incorrectly captures the first
factory consumer in its recovered return error. `unknown_signature` writes a
located `TypeState::Recovered` and `resolve_signature_return` returns that
same failure to every later consumer. The compiler singleton is allocated at
startup with nil declaration and errorType return; it has no creation-site
source provenance (checker1043).

The product contract requires a consumed recovery to stay a failure, but does
not permit an unrelated earlier candidate to become its causal site. The
operating envelope is one request-local evaluator shared across Q3 candidates,
and the existing lazy Q4/display/relationship operations within that request.
There is no cross-request synchronization or persistent cache involved.

## Proof

`native-unknown-signature/warm.ts` has a healthy member inside
`new NotConstructor(healthy.probe)`. The healthy candidate's checking prelude
resolves that outer constructor to unknownSignature without asking for its
return. Two later `new NotConstructor().probe` candidates consume the singleton
return. The existing hook observes actual `result == c.unknownSignature` for
all three constructors in the same checker and preserves the healthy member's
origin; separate fresh type questions establish native errorType at the new
expression. Native source establishes the prelude's narrower operation; no
extra return getter is inserted into the observation.

Cold and reordered variants distinguish singleton creation history from the
actual later consumption. Capture receipt
`vrun-c3e247b6-e9ee-4c8e-b8d1-39f6cba537e8` passed five variants (warm strict,
loose, exact; cold strict; reordered strict). The exact asserted rerun is GREEN
`vrun-dd76a787-6a11-4434-bedc-d6bafdc723ef` (five tests, zero skips).
`tests/member_unknown_signature_provenance.rs` asserts final Q3
healthy groups, two invalid gaps, and each gap's cause at its own new expression
for all three variants and option profiles. Its compiled RED is established by `vrun-2e19c62a` (the warm prelude location incorrectly owns the first consuming call).

## Alternatives and choice

Allocating a signature per call or replacing the singleton's saved location
would break native signature identity or make previous results depend on later
reads. Fixing only `check_call_expression` would leave union Function fallback,
display, relation, inference, flow and derived-signature return reads wrong.
Dropping the location without retaining a typed source-free recovery would
remove evidence but leave callers unable to identify their actual consumption.

Choose a typed unbound recovery in the existing return/failure owner, plus an
explicit observed-return operation. Keep one actual SignatureId and its native
installed return state. Retain the recovery's kind independently from source
provenance; do not identify it by error text, ErrorType reason alone, or a nil
Location (ordinary global dependencies can legitimately have nil Location).
An unbound recovery is still a DemandError, never a successful Any or an
unsupported Q4 leaf.

The public evaluator operation becomes
`get_return_type_of_signature(signature, consumer)`. It runs the existing raw
return worker, then binds **only a typed unbound recovery** to the actual
consumer. Binding occurs on the returned copy after native cache publication.
Ordinary declared annotation, circularity and imported dependency failures
retain their original exact Location without alteration.

Raw return computation and observation must be separate operations: a union,
intersection or instantiated return worker propagates/cache-retains the
unbound recovery from its child. It must not invoke the site-binding wrapper
and then store that first observer's location in a derived return cache. The
external observation of that derived signature binds its current consumer.
This applies to failed replay as well as the initial computation. A later
source operation with its own real syntax, such as checkCallExpression, may
retain the already-bound failure in that actual expression's cache.

## Complete migration

Keep `unknown_signature` source-free; its consumer argument must not establish
return provenance. The current creators are the Function constituent branch
of `resolveUnionTypeMembers` and `resolve_error_call` (checked construction and
other callers). Collection remains healthy.

Route all actual return reads through the observed-return operation: calls and
quick returns, constructor fallback, signature top/noncircular helpers,
contextual/function/body inference, relations and reporting, base construction,
flow/reachability, variables, diagnostic signature display and public
presentation. The initial audit found 22 getter callsites, direct return-demand
reads in the latter families, and a reporting call directly to
`resolve_signature_return`. Recheck the complete source after implementation.
Only signature factories, immutable previews and private raw return computation
may inspect/pass the stored return demand without observing it.

Presentation must preserve lazy admission: carry an explicit signature-return
question in its private child descriptor/record traversal instead of eagerly
resolving a return while forming a header. A preview remains query-free; an
admitted edge asks the signature owner using the actual selected type/query
location, passed from the existing target. Do not reuse
`signature_return_location` for a declarationless singleton. Existing declared
locations still own declaration-specific predicate/cycle questions.

Use the current DemandError/SemanticError conversion owner for the small typed
unbound marker and its one binding operation. Keep it private and off the wire,
preserve causes and whole resource/internal failure precedence, and do not
replace missing source ranges with guessed points. No singleton metadata or
cached unbound failure is mutated when an observation is located.

## Required final checks

First establish the compiled Project RED above, then prove it GREEN in the
coordinated checkpoint. Add derived union/instantiated return and diagnostic
display consumers that replay the same cached unbound recovery at distinct
actual sites. Keep the existing declaration/import failure-location tests
GREEN to prove the new binding rule does not relabel real source failures.
Q4 signature-only previews must remain healthy until an admitted return edge;
the edge must fail at its actual observation. Resource failures remain whole
failures. This plan does not authorize production changes during the freeze.

## Staged ownership

`SemanticError.unbound_recovery` is a private, serde-skipped typed marker. It
survives the existing AnalysisError/DemandError conversions independently from
Q4 issue eligibility and optional global locations. `observe_recovery` binds a
returned copy and clears only this marker. No declared failure is relabeled.

`SignaturesState.returns` and the demand arena permit an absent operation
location only for the installed startup recovery. `unknown_signature()` takes
no consumer. `raw_return_type_of_signature` preserves Known return factories
and the existing lazy return owner; the observed getter attaches a site after
that owner publishes its cache. Derived instantiation retains its current
mapper operation context, while its raw child failure remains source-free.
Composite predicate and mixin inventory caches likewise retain unbound errors.
Actual expression, relation, diagnostic display and Q4 presentation boundaries
locate a propagated source-free inventory failure after their own cleanup.

Presentation retains a signature-return child role in both the summary and
record traversal; previews inspect its raw descriptor only. Admitted edges ask
the observed signature operation with the selected query location. The
`derived.ts` control uses a generic alias whose union combines global Function
with an ordinary callback; its unknown return travels through native union
signature composition and is replayed at two actual calls. A separate final
Q4 test distinguishes summary-only and depth-zero admission from an admitted
return edge. Pinned asserted receipt `vrun-c49c3acb-eead-4e33-ae73-3d2fb8a9498e`
passes strict/loose/exact profiles (three tests, zero skips): the composed
returned value is actual errorType while the alias itself remains healthy.

The derived control exposed a distinct native identity question in
`getUnionOrIntersectionTypePredicate`: a child's return is compared with
`falseType` and `regularFalseType` before effects eligibility is established.
The actual installed unknownSignature return is known to be neither. The shared
signature owner now makes that complete comparison without presenting the
sentinel as an erroneous return value; the subsequent actual invocation still
fails at its own site. This recognition requires the singleton's actual
signature identity and typed source-free marker, with no additional causes.
It must never infer completed native identity from a propagated marker on a
derived signature, whose return calculation may have stopped before later
children. All such failures continue to propagate. Stage14 compiled RED
`vrun-883be28a-d39b-4a24-9232-fda3ce58ba1e` demonstrated the prior predicate/effects
cache poisoning at the warm call; final expectations remain unchanged.
