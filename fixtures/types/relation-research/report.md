# Natural subtype and relation limit evidence

This bounded research uses the Ousterhout reviewing route to test the compiler
premise before choosing a native representation or error interface. It changes
no production Rust files or pinned compiler source. It establishes natural
compiler input families and a concrete missing invalid-reason contract; it does
not close checkpoint 1 or classify arbitrary roots from global failure events.

## Pinned execution and ownership

The compiler is TypeScript 7.0.2, source commit and tag target
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` (`typescript/v7.0.2`). The research
oracle strictly verifies the source/tag/version and clean build inputs, then
uses `go test -overlay` with one fresh checker per selected final query. The
ordinary native compiler is used separately for whole-program diagnostic
controls. Every input uses `strict`, ESNext target/module, no emit and one
TypeScript file; no counters are seeded, reset or reduced.

The frozen research oracle, setup, generators, copied hooks, exact sources and
raw JSON answers are in `.cc/temp/types-relation-limits/`. `provenance.json`
records hashes for these files, the native binary/package and the pinned source
owners. These are disposable research artifacts, not a production backend.
`setup.mjs` recreates the overlay from the then-current harness; the frozen
`research-oracle.mjs` and captured source hashes identify the reviewed run.

Instrumentation observes the existing operations without changing their
branches. `beforeSummary` clones counter/event state after the selected type
demand and before `TypeToString`. A private test-only operation reads diagnostics
already in that checker; it does not run `GetDiagnostics`, which would check the
whole file and establish more facts. Array element identity/cardinality is
inspected after that snapshot through the already returned array type's type
argument. The final root outcome and this child identity are reported separately.

This is throwaway compiler research, so there is no behavior-level red/green
shipping-test claim. Registered tests are required when these fixtures and
instrumentation enter the checked-in harness. The oracle agent owns that step.

## Exact subtype estimate

Pinned `checker.go::removeSubtypes` (25828) increments its comparison count even
when a discriminant immediately excludes a target. At count 100,000 it evaluates
`(count / (length-i)) * length` using integer division and rejects estimates
strictly greater than 1,000,000. Original `length` remains fixed while redundant
constituents are removed. `getUnionTypeWorker` converts the operation's nil result
to `errorType`; `checkArrayLiteral` can then return an array whose element is that
error type. The resulting array is not itself `errorType`.

`subtypeRemovalSource(1250, removals)` in `probe.mjs` declares `1250-removals`
variables with distinct annotated `{tag: i}` types, followed by `removals`
variables with annotated `{tag: 0; extra_i: 0}` types. The final demand infers
`query = [all variables]`. The redundant types are removed first, extending the
processed-source denominator without doing a full target scan.

| Removals | Count at estimate | Original length | Processed | Estimate | Final array element | Final root | Already demanded diagnostic |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 43 | 100,000 | 1,250 | 126 | 991,250 | 1,207 constituent union; not error | Union array; not error | None |
| 42 | 100,000 | 1,250 | 125 | **1,000,000** | 1,208 constituent union; not error | Union array; not error | None |
| 41 | 100,000 | 1,250 | 124 | 1,007,500 | Exactly `c.GetErrorType()` | `any[]`; not error | TS2590 |

The over case records `subtype_complexity_limit` before summary construction;
under and at record no demanded failure. The under/over estimates are adjacent
members of this natural family, not the integers 999,999 and 1,000,001. The exact
at case distinguishes `>` from `>=` without replacing the compiler's threshold.
Raw artifacts are `removal-1250-{43,42,41}/{cases.ts,answer.json}`.

Two failed fixture ideas informed the final family. `CompareTypes` sorts by type
flags, name and compiler-owned structure, not simply type creation ID. Primitive
literal terms therefore did not extend the processed denominator as predicted.
Fresh object literals also trigger excess-property rejection before structural
subtype removal; annotated variable types are essential. The failed mixed array
answers are retained under `subtype-1250-{45,44,43}`; they all reached estimate
1,542,500 and TS2590. These are unsuccessful fixture attempts, not boundary proof.

## Relation depth: an overflow False is consumed by the conditional

`relationDepthSource(depth)` constructs two distinct but structurally equal
literal object types: `depth` explicit `{next: ...}` wrappers around
`{value: string}`. It then demands `Source extends Target ? true : false` through
an annotated variable. There are no recursive aliases or generic instantiations
in this family, so the active-pair and deeply-nested assumptions do not shortcut
the comparison.

| Literal wrapper count | Maximum observed guard-entry source/target stack | Selected final type | Root is error type | Failure before summary | Already demanded diagnostic |
| --- | --- | --- | --- | --- | --- |
| 99 | 99 / 99 | `true` | No | None | None |
| 100 | 100 / 100 | `false` | No | `relation_depth_limit` | TS2321 |
| 101 | 100 / 100 | `false` | No | `relation_depth_limit` | TS2321 |

The control `relationDepthSource(99, 'different')` uses `number` at the target
leaf. It also returns `false`, with no overflow event or diagnostic. That is an
established incompatibility, not an invalid result. Raw artifacts are
`depth-{99,100,101}-same` and `depth-99-different`.

The causal path is visible in pinned source:

1. `getConditionalType` (checker.go:24285) negates the result of
   `isTypeAssignableTo` on the permissive check/extends types. For these
   non-generic, non-any objects, the failed check selects and instantiates the
   false branch (24301–24316).
2. `isTypeRelatedTo` delegates to `checkTypeRelatedToEx`. In
   `recursiveTypeRelatedTo` (relater.go:3103), stack depth 100 sets `r.overflow`
   and returns `TernaryFalse`.
3. `checkTypeRelatedToEx` records a failed relation with stack-overflow metadata,
   emits TS2321, and returns `result != TernaryFalse`. The conditional therefore
   consumes an overflow False and returns the ordinary boolean literal `false`.

The compiler returns this observational recovery value; it also diagnoses the
comparison failure. The design's promise to reject consumed invalid recovery
therefore requires the overflow cause to follow that relation result into the
conditional result. Root error-type identity alone cannot prove validity.

## Natural relation allowance

`allowanceSource(n)` writes a source union of `{common:string; p_i:string}`
objects in ascending logical-property order and a distinct target union in
reverse order. Reversal defeats the same-index correspondence fast path. A
matching target still exists for every source; natural pair checks populate the
relation cache until the unchanged allowance is exhausted. The selected demand
is again `Source extends Target ? true : false`.

| Union terms per side | Initial allowance | Minimum observed uncached guard allowance | First comparison's final allowance | First final relation cache size | Final selected type | Overflow/diagnostic |
| --- | --- | --- | --- | --- | --- | --- |
| 1,998 | 2,000,000 | **4** | **1** | 1,999,999 | `true`, non-error | None |
| 1,999 | 2,000,000 | **0** | **-2** | 2,000,002 | `false`, non-error | Allowance overflow; TS2859 |
| 2,000 | 2,000,000 | **0** | **-2** | 2,000,002 | `false`, non-error | Allowance overflow; TS2859 |

These values distinguish guard entry from operation completion. The under case
does **not** reach allowance 1 at a guard: its last three successful cache commits
happen while unwinding. At/over encounter zero at an uncached guard, then still
commit two failed entries while unwinding. Do not cap final cache size at two
million or describe the observed -2 as the allowance seen by the guard.

The under case's second permissive/restrictive relation operation starts with
allowance 1,750,000 from the cache-size formula and succeeds through its cached
root. This is legitimate reuse, not a reset to two million for each recursive
pair. Raw artifacts are `allowance-{1998,1999,2000}-all`.

An independent fresh public native API request agrees with all three hooked
final outcomes. The exact request selects the `query` Identifier at
`cases.ts:4:25` (span ending column 30), asks the inferred `value` question through
`GetTypeOfSymbol`, and has an empty enclosing-call prelude. The annotated `Query`
alias contains the non-generic conditional. Configuration/program/syntax/bind
diagnostics run before demand, while semantic diagnostics do not. The complete
requests, source hashes, selected origins, effective options and results are
captured in `api-controls.json`; its empty public diagnostics arrays mean
diagnostic collection was not requested, not that an overflow had no diagnostic.

Whole-program native CLI checking is a distinct observation. It emits TS2859
for **all three** allowance inputs, including 1,998. The baseline result of
`true` below the guard is therefore not a CLI whole-program acceptance claim.
Earlier semantic checking/cache state can affect allowances; the specific
whole-program cause here has not been established. The design intentionally
chooses the fresh-demand/prelude baseline, and the equal-demand native API
control confirms the replay/overlay result. Diagnostics-first checker
equivalence remains open.

This natural family proves a valid operation finishing one allowance unit above
exhaustion and failed operations taking the zero-allowance guard. It does not
claim guard-entry observations of 1 / 0 / -1, nor full cache/reset equivalence.

## Maybe and cache distinctions

The order in `recursiveTypeRelatedTo` matters: prior overflow, global-cache
lookup, allowance exhaustion, active-key assumptions, depth exhaustion, then
deeply-nested expansion detection. Instrumentation must observe those existing
sites rather than moving a guard to function entry.

Two self-recursive interfaces `{next: Self}` reach `TernaryMaybe` through an
active key at stack 1 / 1. The root relation result is Maybe (numeric 3), the
selected conditional is `true`, and the operation commits one succeeded cache
entry at depth zero. Adding a `string` versus `number` leaf yields a final
ordinary False despite the earlier active-key Maybe; it has no overflow or
diagnostic. The active-key observation itself therefore identifies neither
invalidity nor a final True.

Expanding generic aliases `Source<T> = {next: Source<Source<T>>}` and the analogous
Target alias reach the deeply-nested Maybe branch at pushed stack 3 / 3. The
root relation remains Maybe and commits three succeeded cache entries, returning
the conditional `true` with no failure/diagnostic. The pre-guard maxima are 2 / 2;
the branch observation is 3 / 3 because pushes precede detection. Raw artifacts
are `maybe-active`, `maybe-active-false`, and `maybe-deep`.

`resetMaybeStack` commits Maybe as succeeded only when its owner admits it at
depth zero (or a definite True proves the assumptions). It does not cache Unknown
as succeeded. These samples support that distinction; constrained-key Maybe,
reliability/variance propagation and all cache histories remain unproved.

## Contract recommendation and limits

Add deliberate invalid reasons `relation_depth_limit` and
`relation_complexity_limit`. The latter names allowance exhaustion, matching
TS2859; the research's internal observation label `relation_allowance_limit`
should not become a separate public synonym. Both belong to consumed overflow
comparisons, not to ordinary False or the compiler's admitted Maybe protocol.

The operation-owned cache stores
`RelationComparisonResultFailed | StackDepthOverflow` or
`RelationComparisonResultFailed | ComplexityOverflow`. `isTypeRelatedTo` can
return False directly from such an object-pair cache entry without producing a
new event or diagnostic. Native outcomes must retain the cause on the entry and
propagate it only to consumers of that failed relation. A global error/event bit
would incorrectly invalidate unrelated successful roots; a bare cached bool
would lose the failure on subsequent consumption. TS2590 is shared with other
union complexity operations, so diagnostic code alone cannot distinguish subtype
reduction from cross-product construction.

This keeps one owner for the full relation decision and its cache lifecycle.
No additional wrapper that callers must remember is warranted: the operations
that already consume relation outcomes must receive established True/False,
admitted assumptions, and overflow provenance distinctly.

Remaining proof includes Q3 candidate/relationship completeness, arbitrary
root/candidate/rejected-overload recovery causality, complete cache/reset/Maybe
and variance behavior, all configured option histories, representation gates,
native semantic owners and Q4 graph/materialization. These natural compiler
families do not supply a native implementation or close those gates.

`native-controls.json` records 13 independently executed pinned native CLI
controls: depth under/ordinary-False/Maybe controls and subtype under/at have no
diagnostic; depth at/over emit TS2321, subtype over emits TS2590, and all three
allowance inputs emit TS2859 in that distinct whole-program history. No test is
skipped or represented as a registered shipping check.
