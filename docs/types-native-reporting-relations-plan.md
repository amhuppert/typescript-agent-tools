# Native reporting relations

## Contract and operating envelope

Port pinned TypeScript 7.0.2 `checkTypeRelatedToEx` and the reporting branches of its existing Relater. This serves semantic checking operations whose diagnostics acquire additional types; it does not add a diagnostics product or admit changed F answers. The evaluator is request-local, owns one captured project and already supports reentrant relation sessions and lazy inference mappers. No process-wide cache, lock or second relation engine is needed.

The immediate caller is `checkTypeArguments`: checker.go 9198–9236 always uses `checkTypeAssignableToEx`. `chooseOverload` supplies no error node; an instantiation expression supplies the actual written argument node and the Constraint head message. The existing `isTypeRelatedTo` wrapper remains a different operation. Its fresh-literal/simple/outer object-cache shortcuts must not be inserted into the direct checking entry.

The current stage has a typed Silent/Report distinction and a located Unsupported at the unimplemented Report operation. That is temporary construction state. Native rejected-constraint tests retain their healthy expected signature, and complete delivery removes this boundary after final evidence.

## Alternatives and chosen owner

A second silent comparison followed by displaying both operands is smaller at the call site but asks the wrong question: reporting can query a first failed overload before a later successful overload, replay a failed relation cache, or read a weak source's return type to suggest a call. A separate diagnostic Relater would duplicate every structural decision and cache contract.

Extend the existing Relater and give each comparison invocation explicit reporting inputs. Keep session identity separate from invocation policy: the session owns its error node, chain, recursion history and caches; a recursive invocation owns reportErrors and its optional head message. No dynamically inherited reporting Boolean: native deliberately disables reporting at many nested comparisons. Inference comparers retain the live session handle and receive the caller's explicit report flag as native TypeComparer does. Actual error-node identity and chain snapshots survive callback reentry through that same session lifetime; a session-global report Boolean would be incorrect.

Use an internal `RelationInvocation` value containing the live session handle, report flag and optional typed head message. Keep recursion flags and intersection state explicit in the operation that changes them. Existing independent comparison APIs construct Silent invocations. `check_type_related_to` accepts a typed reporting context with the actual `ExpressionRoot`/captured node identity when reporting, plus the exact head category. Location remains error provenance, never a substitute for the error node. Source answers syntax and import-origin questions from that identity.

Implementation stays in `evaluation/relations.rs` for comparisons and `evaluation/relations/reporting.rs` for chain and reporting operations. The latter uses the same sessions; it does not rerun a separate comparator. Shared signature comparison and TypeComparer callback APIs carry the invocation's report policy through their existing owner. Model types and native relation keys do not acquire diagnostic fields.

## Diagnostic state and display interfaces

Use immutable chain nodes, referenced by `Arc`, with `{next, message, arguments}`. `None` is the native nil chain. Pointer identity implements the native `r.errorChain == saved.errorChain` question; depth and content equality are separate operations. A typed message enum represents actual pinned diagnostic constants by identity, including all messages tested by suppression/rewriting; it must not infer identity from rendered English or reuse numeric error codes as pointer identity.

Arguments retain native value equality: exact rendered text and signed native integer values. `chainArgsMatch` has a separate wildcard input; nil matching does not erase stored arguments. Rendered text uses the shared display owner's exact pinned Go-printer result and bytewise string equality. Printer.go 242–250 truncates its Go string by raw byte index, so the result can end with an incomplete UTF-8 sequence even when every input name was valid. `RenderedTypeName` and chain text arguments therefore retain bytes, with exact ASCII prefix/concatenation operations for `getPropertyNameArg` and `addToDottedName`. Neither lossy decoding nor UTF-16 normalization belongs here. Current exact-name refusals remain failures at their actual owners.

`ErrorState` snapshots the chain head. Related diagnostic notes have no decision consumer or product output, so only their required semantic acquisitions are retained. Restoring the chain removes only diagnostic state; it never restores semantic caches, reverses identity observations, or catches a `DemandError`. A failed semantic read aborts even if native would later replace an errorType recovery or discard a diagnostic chain. Successful inner relation cache installs and the existing original overflow provenance stay intact.

The shared native display owner must return its actual rendered result, with format flags and optional enclosing declaration. Its current `check_diagnostic_type_display` convenience may discard that result, but relations cannot: `getTypeNamesForErrorDisplay` compares initial strings, conditionally displays both again with FullyQualified, and later suppression compares chain arguments. Required surfaces are type, signature and symbol display with exact native options; native NoTypeReduction and FullyQualified are distinct. Public graph summaries, budgets and source-qualified graph labels are not substitutes.

`symbolValueDeclarationIsContextSensitive` must retain its counterintuitive native predicate: symbol and value declaration exist, declaration is an expression, and `!isContextSensitive(declaration)` (relater.go 1301). It selects the enclosing declaration for the first type display. This source/semantic question belongs to the existing symbol/context-sensitive owners.

## Required operation migration

| Native owner | Complete reporting work |
| --- | --- |
| `checkTypeRelatedToEx` 359–400 | Direct session entry; actual error node/head; existing root overflow installation; optional import-origin helpful retry through actual export-type links; native final chain elision. Retire session on success and failure. Product failure stops at its first consumed dependency; a native relation overflow remains InvalidSemantic with its existing relation-depth or complexity-limit reason after required cache installation, distinct from tool resource exhaustion. |
| `isRelatedToEx` 2600–2711 | Preserve object-to-primitive pre-normalization path, native normalization and simple checks; excess/common property reporting at their actual decision; alias/original operands for error display; reportErrorResults only at native exits. |
| `unionOrIntersectionRelatedTo` 2849–3060 | Primitive reporting suppression; last-source-only reporting for Some; silent correspondence fast path for Each; target-union candidates always silent then complete `getBestMatchingType` selection and one reporting replay; intersections propagate or disable reporting exactly as native. |
| `recursiveTypeRelatedTo` 3061–3189 | Replay cached Failed/nonoverflow comparisons only when reporting; successful entries and overflow entries keep native cache/reliability policy. No separate reporting cache key. Keep Maybe/depth/allowance and error snapshots independent. |
| `structuredTypeRelatedTo` 3192–3260 | Save diagnostic state before worker; silent effective-constraint fallback; extra property/index checks in native order; restore diagnostics only after ordinary success. |
| `structuredTypeRelatedToWorker` 3261–3899 | Preserve original variance error chain, invariant structural elaboration, chain-depth selection for competing indexed-access paths, branch-specific rollback and pointer-identity test for structural reporting; retain original chain on variance rejection despite structural Maybe. Every existing native operator branch gains the actual reporting flag, not an inferred family default. |
| `typeArgumentsRelatedTo` / mapped adapters | Exact variance direction and report suppression, including bivariant alternatives; current mapped producer dependency remains owned by Mapped and is not converted to healthy success. When that producer lands it consumes this same reporting owner. |
| `propertiesRelatedTo` / tuple loop 4100–4262 | Reporting changes unmatched collection and tuple/optional diagnostics. Preserve actual excluded-property progression, optional-only policy and property read order. Count errors create chain entries even if their arguments acquire no types, because chain identity/depth affects later decisions. |
| `propertyRelatedTo` 4263–4337 | Private/protected declaration identity and source/target display order; property-type chain wrapping; optional mismatch. Use existing property-symbol facade and declaration metadata, never graph keys. |
| `reportUnmatchedProperty` 4338–4371 | Actual private-name identity check, full unmatched list, one versus many property names, 5/6 count boundary, declaration-related info, and array-like elaboration guard. |
| `tryElaborateArrayLikeErrors` / primitive wrappers 4372–4408 | Exact tuple/readonly/mutable array predicates and short circuits. Wrapper checks ask global String, Number, Boolean and ESSymbol in native order. |
| `signaturesRelatedTo` 4434–4510 | First source candidate alone reports; later candidates are silent; successful later candidate restores saved diagnostics. A consumed first-candidate failure survives that restoration. Preserve no-source-signature display, constructor abstract/accessibility gates and same-target pairwise path. |
| `compareSignaturesRelated` / predicates 1458–1690 | Explicit reporting TypeComparer callback; arity, this, callback parameter and return/predicate errors at native points; return compatibility reporter wraps exact call/construct signature chain categories. Signature display uses shared expanded-parameter/mapper owner. |
| Index relations 4550–4681 | Missing index, property versus index, source/target key display ordering and chain messages share existing applicability/index factories. |
| `reportErrorResults` 4698–4741 | Nonaugmenting-base observations before restoring original aliases; array and primitive-wrapper elaboration; Object/JSX and never-intersection branches; relation report; unconstrained parameter clone/constraint/cycle query and optional related-info display. |
| `reportRelationError` 4744–4822 | Actual type-name display/equality; singleton generalization with Boolean exclusion; indexed-access target flags; base-constraint/Assignable alternatives; exactOptional unmatched-property policy; nonexistent string-literal suggestion; message-category and chain-argument suppression. |
| `reportError` / helpers 4824–4950 | Excess-property suppression; property-to-return and nested property chain rewrites; native two-node removal; dotted/new/bracket spelling; chain identity/depth, wildcard argument matching and conversion/implementation categories. |

Before adding any prerequisite, search its native name and route all matching-question copies through the complete shared operation. Known absent helpers include `getBestMatchingType`, `getExactOptionalUnassignableProperties`, `typeCouldHaveTopLevelSingletonTypes` and suggestion operations. These are dependencies of reporting, not reasons to select arbitrary union constituents, skip diagnostics, or use empty results. Keep smaller same-question owner additions in existing property/constraint/union modules where they belong. Enum, JSX, mapped and module-value factories retain their established producer boundaries until their complete producer stages; no reporting branch may invent their metadata.

## Failure and cache invariants

1. Reporting never changes the relation key. Cached successful answers skip reporting; cached ordinary failures replay; cached overflow preserves its attached product failure and native installation.
2. A diagnostic rollback does not roll back acquired types, symbol IDs, mappers, relation caches or failure provenance. Do not store `DemandError` in a disposable ErrorState.
3. Per-invocation false reporting does not clear the session error node or chain. Native later branches can restore, compare or wrap that same chain.
4. A reporting constraint that fails ordinarily returns nil from `checkTypeArguments`; instantiation retains the original generic signature. A consumed dependency failure aborts with its original cause. The same caller distinction is tested through final queries.
5. The instantiation cache remains installed before its separate no-applicable-signature display. Reporting constraints happen earlier during signature mapping. Do not move both displays to one final pass.

## Native and final control families

First pin each hypothesis with the compiler, then assert the final `Project::type_of` result. All families run strict, loose and exact-optional profiles where their rules differ. A compiler recovery displayed as Any is evidence of a consumed native error, not permission for a healthy product answer. Use existing zero-demand cache/return observation hooks when a healthy final native signature hides an internal recovery; do not add a semantic probe that itself triggers the demand.

- Existing instantiation `restricted<number>` remains the ordinary rejected-constraint healthy-signature control. Add assignable constraints and silent call controls, with defaults/arity unchanged.
- **Weak/common-property return hypothesis:** `Weak { foo?: string }`, callable source returning Missing, constrained generic overload plus unconstrained same-arity fallback. The call should reject the first constraint silently and select fallback; the instantiation should enter reporting and acquire the first call return. Cover healthy call return that matches (construct return skipped), nonmatching call then construct, construct-only source, and original alias display.
- **Overload diagnostic rollback hypothesis:** source overloads `(x:{a:number;hidden?:Missing})=>void`, then `(x:{a:string})=>void`, target `(x:{a:string})=>void`. Reporting elaborates the first mismatch before the second succeeds. The silent call control must stay healthy; a consumed reporting failure cannot disappear with restored diagnostics.
- **Failed-cache replay hypothesis:** `Bad { a:{p:0;hidden?:Missing} }` against `L { a:{p:1} } | R { other:boolean }`, with a fallback overload. Silent candidate checks install failures; reporting's best-match selection replays L and its nested pair. Verify actual selected branch and return/property demand without guessing from the final union spelling.
- Union source Some last-element policy and target best-match/no-best-match policy, including literal primitive reporting suppression; intersection effective-constraint success with diagnostic rollback.
- Private/protected/unmatched properties, one/2/5/6 missing-property boundaries, optional-versus-required differences, readonly tuple/array mismatches, rest/variadic arity diagnostics and actual labels.
- Signature missing/abstract/accessibility/this/predicate/return mismatch, first-failure-only matrix, same-target pairwise path, callback variance and generic constraint/default dependencies.
- String versus number index diagnostics, index-key order, exact-optional unmatched properties, literal base generalization and target type-parameter constraint alternatives.
- Same initial type name requiring FullyQualified second display, context-sensitive declaration selection, equal versus unequal rendered argument suppression, nested property/return chains, and shorter-versus-longer indexed-access error chains.
- Successful-cache skip, failed-cache replay, inner cache retention after an outer failed display, recursive Maybe commit, and under/at/over existing relation limits. Reuse current limit fixtures and add reporting controls without broadening their limits.
- Native display truncation boundaries with a lazy failed tail immediately before/at/after elision; graph/public summary budgets must not affect this semantic demand.

The three named hypotheses above are now measured by the standalone controls
recorded below. Member review independently audited their native schedules.
No expected product result is weakened to make the temporary Report boundary
pass.

## Files and execution order

1. During the frozen checkpoint, finish this plan, review with native_members, and settle algebra's native rendered-result/flags/enclosing-declaration API. No production edits.
2. After root opens a source window, author new standalone native controls and final tests; obtain native observations and a genuine RED or a root-coordinated isolated counterfactual for new behavior.
3. Add immutable chain/context state and complete the direct checking entry; mechanically carry explicit invocation policy through existing comparisons and live TypeComparer callbacks, preserving all Silent callers.
4. Port reporting branches in native order with shared display and semantic helper owners. Add the full chain transformations before relying on chain equality/depth decisions. Error-node syntax/import helpers belong to Source/binding, not Relater AST parsing.
5. Peer review the actual implementation and close all lanes. Root runs one compile checkpoint, focused reporting/native-control finals, then existing relation/inference/call/instantiation/property suites. Run registered formatting/lint and full profile before delivery.

Relations, signatures reporting callbacks and call constraint routing remain this lane. Native display implementation remains algebra's lane; exact syntax/symbol/alias metadata is coordinated with native_members; public graph schema/rendering remains root-owned. Mapped work follows reporting closure and reuses these owners. No temporary Report refusal counts as completion of the authorized full task.

Read-only reviews: native_members passed the session/invocation split, cache replay, chain identity/depth/equality, rollback/failure separation and display boundary. Root passed ownership and corrected the native overflow classification and Go-printer text equality wording. Production remains frozen pending the shared checkpoint.

The first implementation checkpoint carries the explicit invocation through
the existing comparator/inference interfaces and keeps true session IDs in the
active arena. Immutable chain rewriting, snapshots, byte arguments and cached
Failed replay are staged; the Report entry still refuses until the complete
reporting owner is wired. Member's implementation review passed native
reportError rewrites and replay/reliability placement. Before enabling Report,
every recursive edge must clear the head message exactly as native
isRelatedTo/Worker do, and cached-overflow reporting must acquire source display
then target display before returning the retained product overflow cause.

The initial standalone native capture `vrun-cb9f89d1-5251-4e7a-bb5c-c461ff92aa5f`
observed 72 final roots across strict, loose and exact-optional profiles. It
confirmed the weak-call/construct return, first-overload diagnostic rollback,
and failed-union-cache replay hypotheses in every profile: silent controls
stayed healthy without the missing dependency, reporting consumed it despite
a healthy native final signature. The zero-demand getReturnTypeOfSignature
hook records actual declaration kind/span and the returned errorType before
summary rendering. Exact asserted refinements and final Rust controls remain
required; a capture-only pass is not their substitute.

The final asserted run `vrun-9ee057e8-d0ec-43a2-aa62-ca14d3f257d3` passed all
72 queries (three profile tests, zero skips). Every final summary, errorType
observation and diagnostic-code list is asserted. Return witnesses distinguish
silent call/construct controls from reporting reads, prove that a compatible
call return skips the construct return, and prove that a nonmatching call
return advances to the construct return. The overload rollback and failed
union-cache replay controls retain healthy native final signatures while
their reporting paths consume the missing type. Final Rust controls and the
complete reporting implementation remain required.

The separate native rendered-byte run `vrun-09dc73d9` also confirmed five
default truncations ending in invalid UTF-8 and complete bytes under
noErrorTruncation. The display owner accepts exact caller flags:
`FULLY_QUALIFIED` alone does not implicitly add `DEFAULT`, matching
`getTypeNameForErrorDisplay` rather than the initial default display.


## Reporting/discriminator checkpoint evidence

The complete 24-case discriminator matrix is compiler-asserted in strict,
loose and exact profiles (`vrun-f84713bc-2fdd-40f8-97ae-c744a54008cc`, 72 final
queries, zero skips). It includes 9/10/11 constituent thresholds, the exact
half threshold, nonliteral and empty keys, object-count gates, duplicate and
third-duplicate counts, and ambiguous lookups. Payload witnesses establish
semantic consequences: accepted key maps skip `MissingPayload`, whereas a
rejected map or selected ambiguous key consumes it. Final Rust controls are
in `tests/type_discriminant_maps.rs`; their integrated verdict and controlled
counterfactual remain required.

The next reporting implementation stage adds property/unmatched/index/tuple
elaboration, native silent child comparisons, structured-frame error snapshots,
invariant variance fallback and indexed-access shorter-chain selection. Report
entry remains gated until the remaining whole traversal is coherent; this is
an implementation checkpoint, not delivery of a narrower contract.

Overflow diagnostic coexistence follows the existing failure contract. Root
cache installation precedes native diagnostic display, including the silent
root-overflow entry. Cached overflow only renders when reporting is requested.
Overflow is already a consumed invalid dependency, so its relation-depth or
complexity-limit reason remains primary. A later semantic display failure is
appended as a cause, followed by its original causes, in stable order with the
shared sixteen-cause cap and omitted count. A full location is retained only
when already available; no end position is invented. Later resource, I/O and
internal failures remain whole failures. Once a display acquisition fails, no
later display operand is read. Native display-invocation and final aggregation
controls are still required before this policy is considered proven.

The current integrated stage now dispatches Report through the shared Relater.
It includes fresh-object excess checks, regular object-literal transformation,
property/index/tuple diagnostics, native structural variance fallback, and the
25-combination discriminated structural comparison. `checkTypeAssignableTo`
uses an actual node and nil head; `checkTypeArguments` preserves its Constraint
head. No admitted factory currently creates `cloneTypeAsModuleType` export-type
links, so the optional helpful-import retry has no present entry; introducing
that namespace factory must publish and consume those actual links. Generated
private keys, JSX intrinsic elaboration, mapped producers, and unfinished native
display qualification remain precise dependencies in their owning operations.
They are required remaining work, not a narrowed delivery contract.

The integrated reporting final suite is GREEN (`vrun-b998c54c`), and the four
discriminator groups are GREEN. These prove the existing natural cases, not
all remaining boundary families below.
Additional final/compiler controls remain required for spelling gates and
native tie ordering, the discriminated product's 24/25/26 boundary (including
zero constituents and tuple exclusions), and overflow plus diagnostic failure
aggregation. Existing native reporting and key-map controls do not prove those
new boundary families by themselves.

## Failure provenance checkpoint

`SemanticError` now retains an internal complete source range independently of
`DemandError.issue`. The existing Source operations that return `AnalysisError`
can therefore preserve that range through their normal conversions. All four
record-construction sites were audited: located semantic failures retain their
actual supplied range; unlocated internal/resource failures retain absence.
Serialization skips this field. The public Rust record gains private internal
state; external callers observe the existing serialized error contract rather
than constructing a complete `SemanticError` literal.

The shared `as_cause` projection supplies reporting overflow aggregation and
Q3 coverage. A candidate gap prepends its primary dependency only when that
dependency has a different range or no range, then retains the existing causal
order and sixteen-entry cap. Invalid errors still have no `TypeIssue` and cannot
be rendered as Q4 unsupported leaves. Final controls in
`tests/member_reference_failure_causes.rs` exercise distinct dependency ranges,
missing-global absence, and the unchanged error wire shape. Their first
integrated verdict and a controlled counterfactual remain pending.

The unused related-diagnostic record vector was removed from reporting state.
Native related-note creation cannot influence chain identity, depth, arguments,
or relation decisions, and this product does not publish TypeScript diagnostic
notes. Its required declaration observations and constraint/type-display
acquisitions remain at the original native call sites and still propagate
failures. The active chain retains its native snapshot and rollback behavior.
