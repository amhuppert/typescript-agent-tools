# Native algebra ownership review

This bounded read-only Ousterhout review assumes one immutable accepted Project,
canonical request-local TypeIds/SignatureIds, captured source attachments and
the existing evaluator/presenter. It reviews the intersection section of
`types-native-algebra-plan.md` against pinned TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. No algebra production is reviewed
or claimed complete.

The deep evaluator factory and shared Model identity boundary are appropriate:
callers should not reproduce normalization, relation or cache sequencing from
syntax descriptors. Ordinary False, relation overflow and consumed invalidity
remain distinct questions. Reusing insertion order from native orderedSet,
rather than sorted union identity, preserves the native intersection family.

One key-contract correction is required before implementation. checker.go
`getIntersectionKey` (17445) writes the normalized ordered type list. If
NoConstraintReduction is clear it then writes the actual alias; otherwise it
writes a literal `*` marker and omits alias. It does not serialize every
IntersectionFlags bit. NoSupertypeReduction affects the preceding normalization
but is not an independent key dimension. A larger invented key could create
new identities/cache histories where the pinned compiler deliberately shares
a result. The plan's general “flags and alias” wording must name this actual
partition rather than imply all flags plus every alias.

The branch schedule also belongs inside the complete operation. The
2-constituent variable/base-constraint and ordered StrictSubtype questions run
before the intersection cache lookup, including a would-be cache hit. Union
intersection/distribution runs only on a miss. Divide-and-conquer requires both
at least three normalized constituents and more than two original input types;
the original count prevents repeated expansion. The final propagation flags
use original inputs while excluding nullable kinds. A normalized identity list
alone cannot reconstruct these scheduled questions.

Final proof should pin both constructor-order identity and cache histories,
constraint failure before a cached result can escape, NoConstraintReduction's
alias partition where callable, and ordinary False versus consumed relation
limit recovery. Existing primitive relation controls do not prove an
intersection relation result by applying a universal Some/Every shortcut.
Tuple and symbolic operator details were read at plan level but not audited
against their full native owners here. Their fresh controls, compiled public
RED/GREEN, required-kind inventory and resource-boundary journeys remain open.

## Intersection constraints follow-up

The composite-property final tests exposed a separate reference-narrowing
refusal before reduced intersections could run. Native
`isGenericTypeWithUnionConstraint` and `isGenericTypeWithoutNullableConstraint`
(checker31276/31283) recursively search intersection constituents in order and
stop at the first true leaf. Both now use that operation instead of an
unconditional missing-operation result. The union-level `someType` remains a
distinct outer operation.

The same review found that `getResolvedBaseConstraint`'s retained
`AsConstrainedType` mask omitted intersections even though
`computeBaseConstraint` already contained their full child/rebuild branch.
The gate now admits intersections. Changed unions rebuild through the shared
evaluator union owner, including its acquired constrained-variable questions;
changed intersections retain every available child constraint, while changed
unions require a constraint for every child. Every child is acquired in order
before that decision, and consumed failures retain their original provenance.

Relations' bounded adversarial review found no mismatch in these staged native
branches. `tools/type-intersection-constraints-oracle.test.mjs` passed
**vrun-b095b05c-e0f6-4f60-815c-2bc5239b5995**: 14 fresh strict/loose observations
(15 Node tests including the parent), zero skips. The controls distinguish
ordinary, changed, absent, nested, nullable, unchanged and consumed-invalid
constraints. Changed intersections retain the native denormalized origin in
their displayed union, rather than printing the distributed constituents.
`tests/type_intersection_constraints.rs` carries the corresponding final
query assertions and original failure location. The original composite tests
have a compiled assertion RED; the next coordinated Rust checkpoint must
establish their progress and these new final variants. This is not yet a Rust
GREEN claim.
