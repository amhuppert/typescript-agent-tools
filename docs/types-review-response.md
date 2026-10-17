# T, Q3, Q4 response to Astra xhigh review

2026-10-09. All three findings in [the independent review](types-design-review.md)
are accepted. The revised [design](types-design.md) changes contracts and owners;
no native implementation or production fix is claimed. The reviewer used the
Ousterhout software design skill as requested, at model Astra and effort xhigh.
The original review and its artifact hashes remain intact.

## Finding 1: computed members require type-dependent binding

**Accepted and revised.** A frozen `BoundProgram` owns canonical *lexical*
identities, scope/name-use facts and member recipes. It no longer promises that
all member merging finishes during project loading. The member-table owner
completes ordinary and computed recipes using established key types, applies
the appropriate declaration-combination rules, and supplies both type lookup
and reference search with early/late/combined member views and their origins.

This operation follows `getResolvedMembersOrExportsOfSymbol`, `lateBindMember`
and `addDeclarationToLateBoundSymbol`. It does not mutate lexical identities,
merge by structural type equality, or introduce a second global binding pass.
Memo keys include owner, static/instance side and substitutions where relevant.
Computed-key demands pass through the same evaluation and failure policy as
other type demands.

The design now includes the review's merged `Late` interface and computed
accessor examples. Declaration-seed and member-use results must be compared
separately with the compiler; a combined lookup member must not erase the
early declaration's different reference behavior. This addresses the ownership
conflict inside T, not merely the top-level query contract.

## Finding 2: flow admission must be transitive

**Accepted and revised.** Semantic source captures syntax/effect facts; the
evaluation owner decides admission for each semantic demand and its dependencies.
An annotation is no general escape from flow analysis. Its body may be skipped,
but `typeof x` and other dependencies must still be established.

The design adds required refusals for:

* An immutable copy of an initialized annotated union whose initializer refines
  the use type. No declared-union fallback is allowed.
* A single-return boolean function whose signature could require inferred
  predicate analysis. No plain-boolean fallback is allowed.
* An explicit `typeof x` annotation in a narrowed region.

Initial admission is conservative. Reads of initialized annotated bindings need
proof that initial/declared types coincide without refinement; otherwise they
are refused. Possible assertion/never-return call effects require signature
knowledge, or a conservative refusal. Earlier sibling statements, enclosing
regions and captured writes are included. Unsupported demands propagate their
causal site to Q4 errors or Q3 candidate gaps.

This keeps F out of scope while acknowledging its effect on usefulness. The
fixed Command Center query set must report resulting refusal and partial-answer
rates before the semantic profile is considered delivered. The review's
counterexamples become behavior-level boundary fixtures during implementation.

## Finding 3: imported interfaces have no value type

**Accepted and revised.** Alias anchors follow the existing bound meanings.
Interfaces and type aliases expose declared types. For targets with both type
and value meanings, ordinary imports expose value types and type-only imports
expose declared types. A value-only target can still expose its value *type*
for inspection under a type-only import; that does not authorize runtime use.
Namespace aliases expose the export-object type. Name and declaration-anchor
selection use the same operation.

The design adds renamed interface/type-alias and imported-class examples. It
requires final query tests for both selectors and alias access restrictions;
an unresolved alias never becomes the compiler's displayed recovery `any`.

## Additional refinements

| Review/self-audit item | Revision |
| --- | --- |
| Legacy reference wrapper could lose partial coverage | Extend `ReferenceResult` itself with coverage and effective filters. Both APIs return that result through one owner; no compatibility adapter discards completeness. |
| Public responses were only described in prose | Added concrete JSON for a partial Q3 call search and a successful depth-elided Q4 result, plus graph-field and document-local-reference rules. |
| Option matrix contained a speculative removed option | Removed `keyofStringsOnly` from support; the pinned catalog has no such option. Named `strictBuiltinIteratorReturn` and `noPropertyAccessFromIndexSignature`, with diagnostic-only/excluded-flow distinctions. |
| Dual-purpose tokens needed selection precedence | Existing lexical interpretations win for shorthand and parameter-property tokens. A member-use seed can include the compiler's related local/property sites without changing the old lexical query question. |
| Diagnostic recovery may look like a normal type | Require operation outcomes, including failed overload applicability and `satisfies`, rather than relying solely on `isErrorType`. Unrelated diagnostics and unvalidated annotated bodies are not global blockers. |
| Ban on library name shortcuts was too broad | Explicit compiler intrinsic/global-type hooks remain valid owned ports; ad hoc utility-name shortcuts remain excluded. |

## Evidence and remaining work

The author's 27 type/symbol and 12 reference probes, and the reviewer's separate
flow/member/import probes, used the pinned 7.0.2 tools and produced zero semantic
diagnostics in their source fixtures. The original review retains its exact
cases and source-function references; these are compiler observations, not
conformance results for new Rust code.

This turn changes documents only. Native checker feasibility, broad semantic
conformance, complete candidate inventories, option-dependent behavior,
memory/latency, and query usefulness remain implementation proof obligations.
The design keeps request-local caches and the current session/service model;
the corrections add no production compiler dependency, incremental database,
concurrent checker protocol or F implementation.

The same Astra xhigh reviewer completed a focused follow-up: **ready to guide
implementation; no remaining design blocker found**. All three findings are
resolved in the design. The follow-up, its reviewed artifact hashes, and the
remaining implementation risks are recorded with the independent review.
This closing status paragraph was updated after that review; the reviewed
design and evidence artifacts are unchanged.
