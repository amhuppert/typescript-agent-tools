# Native base-constraint operation

This implementation increment uses the Ousterhout skill. Its envelope is one
fresh request's canonical Evaluator, captured syntax, immutable established
type identities and existing type-resolution stack. The complete T/Q3/Q4
scope remains required; this is not a primitive-only delivery boundary.

Pinned owners are `getBaseConstraintOfType`, `getResolvedBaseConstraint`,
`computeBaseConstraint`, `getNextBaseConstraint` and
`getConstraintFromTypeParameter` in `internal/checker/checker.go:16969–17018,
27302–27472`, with `getRecursionIdentity` in `internal/checker/relater.go`.
The guarded parameter constraint is a different question from its recursively
flattened base: `T extends U`, `U extends "ready"` exposes `U` as T's declared
constraint and `"ready"` as T's base.

Two designs were considered. A Model callback that acquires annotations and
constraints would make each caller carry evaluator state, resolution
cleanup and error policy. Reusing the public guarded constraint recursively
would also re-enter the same guard instead of asking the native raw worker.
The chosen boundary is one deep Evaluator operation:
`base_constraint(TypeId, consumer) -> Result<Option<TypeId>, DemandError>`.
It owns acquisition, native cache/sentinel protocol, shared resolution
history, recursion identity and the entire branch schedule. Model retains
established flags, native type facts and canonical construction identities;
Source remains the sole syntax/own-declaration recipe owner. Presentation
does not inspect raw recipes or perform constraint evaluation.

The raw `getConstraintFromTypeParameter` and guarded
`getConstraintOfTypeParameter` questions receive distinct canonical demand
identities. The raw worker checks the actual first nonnil annotation,
preserves recovery, normalizes genuine Any to Unknown (or the native mapped
key union at that separate branch), and caches native absence. The public
guard first resolves the parameter's base, then returns the original raw
constraint. Parameter graph edges therefore retain symbolic U; type facts
consume the actual base. Synthetic `this` is a retained semantic fact, never
inferred from its printed name, and its base is its actual target without
recursively acquiring that target's members.

Native `Type`, `NoConstraint` and `CircularConstraint` cache results remain
distinct. Product failures retain their original `DemandError`; absence is
not a catch-all for failed annotation, I/O, work, memory or depth questions.
`ResolvedBaseConstraint(TypeId)` participates in the existing property-aware
type-resolution stack. A successful or native recovered installed property
ends native cycle search; a work-in-progress marker or a product failure
before installation does not. Consumed CircularConstraint is reported as
InvalidSemantic/CircularType with the actual constraint provenance even
though the public compiler API returns no constraint. Summary-only and
depth-zero parameter roots do not consume omitted constraints.

The recursion guard must retain the exact source order: push the base
resolution property, compute native recursion identity, always explore below
10 stack entries, then explore below 50 only without a repeated identity,
simplify for reading, compute, pop, and install the native result if still
unset. The native safety approximation is separate from product resource
budgets. Its `NoConstraint` result may be admitted only with source and
causal evidence for that exact branch; exhausted product budgets always
remain failures. Conditional constraint depth 100 is a separate native
operation and is not a substitute for this guard.

`computeBaseConstraint` must account for every native branch: parameter and
synthetic this; union/intersection ordered constituent substitution;
keyof/mapped-key remapping; template literal and string mapping; indexed
access (including the indexed mapped substitution case); conditional;
substitution intersection; generic tuple variadic elements; and unchanged
ordinary types. The established parameter/union/object/reference families
provide the first public consumers. Symbolic deferred operators must retain
their native types and operands when those factories become reachable;
they are not Unknown or unsupported display substitutes. The underlying
simplification, mapper, relation, intersection, indexed access, conditional
and tuple owners are mandatory dependencies, not reasons to declare the
whole operation complete early.

Initial fresh API controls in
`tools/type-base-constraint-oracle.test.mjs` passed three families with zero
skips in `vrun-87cfda2b-db22-40bb-b95f-70c2e769a673`: raw versus flattened
parameter/union bases, free-parameter absence, structural/reference bases,
circular absence, actual MissingType recovery and synthetic this. An initial
token control used a synthetic/incorrect token location and was corrected
to the complete TypeReference/ThisType node; one union print-order assumption
was corrected from the native result. Those are evidence setup/expectation
corrections, not product REDs.

Natural exact native evidence is archived at
`fixtures/types/base-constraints/evidence/provenance.json`: 51 historical
artifacts plus captured loader dependencies and compiler pins. Distinct-symbol
parameter counts 50/51/52 reach stack depths 49/50/50: the first returns a
literal base, the others install native NoConstraint without added circular
diagnostics. A bounded repeating-identity family reaches depth 9 for N=5 and
returns `number | "done"`; N=6/7 stop at depth 10 with the repeated identity and
NoConstraint. No counters, thresholds, caches or native inputs were seeded.
Registered native four-test GREEN is
`vrun-f0aa41a3-3178-49ff-a3d6-2d14b0c6fbab`.

The safety-stop policy was challenged in independent Ousterhout review because
the native source calls depth 50 a safety stop and the repository requires
product depth failures to remain failures. The chosen implementation preserves
typed native-stop provenance in the installed NoConstraint cache, separately
from ordinary absence and every product failure. Admission follows the measured
compiler approximation for that exact source branch; it does not reinterpret
an exhausted product resource budget as successful absence. This distinction
must remain visible when symbolic operators or resource owners expand.

Final `tests/type_base_constraints.rs` assertion RED
`vrun-4b8fb865-f2f9-445a-b63b-03021b14ef49` became three-test GREEN
`vrun-3ef4c7a9-3e64-41c4-b13e-b6c2d376f2ff`, one file matched and zero skips.
The public operation proves raw symbolic constraints after flattened literal,
free and object chains, reference/recursive structural bases without unrelated
child demands, and summary/depth-zero valid roots versus consumed direct/mutual
CircularType failures. It does not prove every symbolic operator branch, fresh
mapper constraint/default protocol, native safety-branch public consumer or
resource gate.
