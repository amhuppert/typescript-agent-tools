# Native completion correctness checkpoint — 2026-10-13

**T/Q3/Q4 is complete as of 2026-10-16 under the user-approved single-user
resource policy and the documented TypeScript compatibility profile.**

The per-operation work/allocation accounting and quota-only probes have been
removed. Native cycle semantics, compiler limits and explicit recursion
protection remain. Read-only query processes now use a sampled 6144 MiB memory
threshold and a 60-second deadline. A terminated service restarts and rebuilds
its cache on the next request; edit application is outside this watchdog.

Final validation passed: all 284 Rust library/CLI/integration roots in 37
registered batches; all 352 retained harness files (1,084 tests, zero skips);
formatting; all-target lint; the pinned-compiler semantic gate; and the release
build. The final release binary is identical to the one that passed 16 direct
and 16 service corpus comparisons plus four lexical baselines. The eight
process lifecycle tests passed both their focused run and the final Rust replay.

Two filesystem-dependent Rust cases skipped because this host rejects
non-Unicode names, and one existing external-input library helper was ignored.
The replay does not claim Cargo doctest execution. Memory sampling can overshoot
and is not a hard OS cap; allocator abort can precede a structured report.
macOS x86_64 was exercised; Linux monitoring is implemented but was not exercised
here. These are recorded limits of this delivery, not pending accounting work.

The final reconciliation is indexed in
`docs/types-completion-validation-2026-10-13.json` and preserved in
`.cc/temp/process-limits/final-validation-complete.json`. User-facing behavior
and configuration are described in `docs/types.md`.

Dated sections below retain chronological evidence, intermediate failures and
their limitations. The 2026-10-16 user decision supersedes exhaustive fee-coverage,
omission-experiment, cumulative-allocation calibration and same-process-survival
requirements. Earlier statements that delivery or gates remain pending describe
the state at those dates; the final closeout below governs the current status.

## Fresh type allocation

TypeScript 7.0.2 `newType` clears `CouldContainTypeVariablesComputed`,
`CouldContainTypeVariables`, and `MembersResolved` on every new identity.
`instantiateAnonymousType` copies other flags from its target, so its fresh
object must not inherit a completed member inventory.

`Model::push_with_flags` now owns this rule. Reference, union, intersection,
anonymous and eager-object factories route their supplied flags through it.
Eager objects establish their own completed member inventory; lazy objects do
not. Native reference-clone operations retain their separate explicit
post-allocation flag-copy rule.

The public query `keyof NoInfer<typeof createBox("text")>` reproduced the
corpus Internal failure, `resolved anonymous object lacks installed member
fields`. The pinned compiler returned `"value"`. The regression matrix includes
nested calls, direct property and method returns, constrained arguments,
nonempty and empty anonymous results, mapped types, strict/null-loose profiles,
and both summary and graph requests.

- Public-query RED: `vrun-9a4df499-c7fc-4639-8720-44e39a1379e5`.
- Initial native control: `vrun-a57028d5-2b20-41b0-9c27-9acde338e7c3`.
- Expanded native control: `vrun-b94bb779-655d-402e-8b6d-2b8e77405289`.
- Expanded Rust GREEN: `vrun-f6d55691-da02-4d3c-b7d3-d05b26b2293a`.

## Applicable index symbols

Q3 formerly reported valid indexed candidates as unsupported. The shared
`getApplicableIndexSymbol` owner now preserves native nil and any-base sentinel
results, chooses the applicable index, collects its actual declarations,
creates its Property/IndexSymbol header and installed value, and caches it by
IndexInfo identity. Combined indexes collect applicable declarations in native
order. Access lookup installs only a non-nil result on its node.

The final-query fixture includes a declared string index, two overlapping
pattern indexes, unrestricted any, and an any-base class. All remain unrelated
to the healthy named member, so its reference search is Complete with exactly
its two occurrences. The new owner checks request depth and accounts for its
explicit declaration scratch and symbol-cache growth; this is not a claim of
complete evaluator accounting.

- Public-query RED: `vrun-1c882b24-b618-4da8-a58f-945b2c194692`.
- Expanded native control: `vrun-2a93213e-ca22-4035-90ef-8734f47c55c6`.
- Rust GREEN is included in the eight-target replay below.

## Inherited assertions

`getBindingElementTypeFromParentType` produces errorType for invalid object
rest; its failure now uses ErrorType rather than UnresolvedSymbol, at the actual
binding declaration. The strict, loose and exact-option fixture passes.

The pinned controls confirmed that the lazy conditional members are now
supported and resolve to number, the quiet Alias-only display lookup does not
invalidate a healthy local shadow type, and retained raw globalThis metadata
removes the former artificial unsupported cause while preserving overload
recovery. `resolveEntityName` reports an absent namespace export on the right
identifier; the new native location control verifies that exact range.

The completion design preserves every acquired recovery in native order,
including cache hits. Q3 property lookup checks a receiver and, when its symbol
is absent, reads the cached receiver again for applicable-index lookup. The
cause assertions now preserve both reads and the ordered Object/Array fallback
chain; no production deduplication or failure suppression was introduced.

- Pinned five-file controls: `vrun-586aae28-dc4a-4178-8d23-c0d6fcf32ca1`,
  10 executed tests, zero skips.
- Exact missing-export range: `vrun-e32d862e-3061-4216-9883-69982dbf8dba`,
  four executed tests, zero skips.
- All six inherited targets plus both new regressions:
  `vrun-5c58fb9a-9617-482e-9092-87e0cdcf7e39`, passed, zero platform skips.
- Library and expanded index fixture:
  `vrun-b28b84d0-59e0-4f82-97bc-d9318e24da86`, passed, zero platform skips.
- Formatting: `vrun-81647c12-381d-46c6-9b9d-0960f1e3edbe`, passed.
- Registered semantic comparison:
  `vrun-426043cf-b6b0-4113-9111-2a5ae8acce2d`, passed.

## Broader replay and corpus work

The sequential integration replay executed all 273 unique targets, continuing
after Cargo stopped at each failed target. Its first completed replay passed
253 targets and reported failures in 20. The constructor target passes after
its production fix; subsequent cwd and exact-byte literal checks are recorded
below. Other failures require causal review rather than blanket expectation
updates. They include unknown-signature provenance, global-wrapper and ordered
display recoveries, explicit flow admission, module/type-query behavior, and
semantic deadlines on native union/intersection boundary cases.

The harness replay executed all 321 files in non-overlapping passed old scopes
and the narrowed scopes. It executed 1,037 tests: 1,033 passed, four failed, and
none skipped. The four failing assertions were in three scaffolding files:
`type-native-generic-aliases-oracle` omitted its newer nested imported fixture;
`type-oracle-review` omitted two imported modules when relocating tooling; and
the isolated template getter compared current files to a historical hash list.
The latter also named files since removed from the worktree. The replay now
checks the current shared-tool inventory and hashes before/after its operation.
The original failed observations remain preserved; later scoped receipts record
the repairs.

Exact targets, execution counts, run IDs and receipt paths are in the
[validation scope record](types-completion-validation-2026-10-13.json).
Timed-out or unexecuted scopes are excluded from these execution counts. The
Rust replay has one filesystem-dependent skip: this macOS filesystem rejects
non-UTF-8 names. Scope success does not establish that physical input variant.

The new `native-corpus-journeys` wrapper builds only the release CLI with two
jobs and runs the three Q3/five Q4 frozen corpus requests sequentially. It binds
binary, source, tooling and native-capture hashes and compares exact native
ranges, admitting only the explicit JSX gap contract. Its registration and
wrapper remain local to this worktree; the first run uses the project-setup
wrapper-development exception, not a registered CC gate.

An initial debug run was stopped after its first 180-second timeout. It did not
establish a semantic verdict. The release run is separate. Corpus success,
exact failures, timing, resource calibration and service guarantees remain open
until their completed captures and subsequent checks are recorded.

## Constructor argument projection and oracle execution

The broader replay reproduced an Internal failure in `deprecatedNew`:
`matching arguments have a different call syntax role`. TypeScript's
`hasMatchingArgument`, through `isUncalledFunctionReference`, accepts Call and
New expressions. The existing argument-site owner now accepts both; the
predicate-only argument operation keeps its narrower native question.

- Existing final-query RED:
  `vrun-e4edd55a-8323-4487-bca4-73aae0704712`.
- Three-test Rust GREEN: `vrun-2fdc4b2b-7283-4201-b119-f01ffef5ae34`.
- Formatting: `vrun-22a499e6-6a87-43f3-b765-cab9bceecf05`.
- Native matrix plus cache test:
  `vrun-280dade2-f63d-4e5b-8687-80e8f3baaff1`, four executed tests, zero skips.
  The native portion checks 23 roots in each of strict, loose and exact profiles.

The oracle now caches its compiled test executable, not semantic state. The key
includes pinned clean compiler source, exact overlay bytes/destinations, and Go
build environment. Each filesystem replay, retry and query executes a fresh
process. Publication is atomic; cache corruption remains a failure. Go's derived
GOGCCFLAGS embeds a random build directory, so the key excludes that field while
retaining its compiler, CGO flags, platform and toolchain inputs. A cache test
proves stable reuse, source/toolchain separation and corruption handling. Its
isolated omission RED is `vrun-58c71d04-9685-40a9-b6d3-32ce0fff74c8`; its GREEN
with fresh-checker native controls is `vrun-356a593b-500a-41c7-b099-7c6d6496bb83`.

The deprecation matrix timed out after 900 seconds without this stable key.
The successful cached replay completed in 91.99 seconds from submission to
receipt. Host activity is uncontrolled, so this is an observed feedback-time
improvement, not a controlled speedup ratio. Compile-worker count stays two.

The broad harness batches repeatedly timed out without a usable output receipt.
They were stopped and replaced by sequential two-file scopes. Do not count
requested files in timed-out batches as executed. The narrowed replay's receipts
and scopes are in `.cc/temp/full-implementation-harness/narrow-report.json`.

## Completed release corpus diagnostic

The first release capture is preserved at
`.cc/temp/native-corpus-release-initial.json`; its exact initial driver is
`.cc/temp/native-corpus-release-driver-initial.mjs`. The failed-journey replay at
`.cc/temp/native-corpus-current.json` also preserves raw stdout, including Partial
documents. These are local wrapper-development diagnostics, not CC gates.

All five Q4 roots are Found, with no issues. Four summaries match the frozen
native strings exactly. The filter-property summary is a complete 431-byte
spelling, while the default native formatter truncates its capture earlier.
The product design allows 512 bytes; this difference needs a native
NoTruncation control rather than weakening the comparison to a prefix.

All three Q3 queries now produce useful Partial documents instead of the
previous Internal failure. The two cleanup seeds each establish five of the
native seven occurrences. One also retains an extra empty seed group that needs
review against the result contract. Each has 27 flow_required gaps (F is an
explicit exclusion), 11 unsupported_semantic_form gaps and seven
instantiation_limit gaps. The JSX query establishes two occurrences and reports
three expression-operation, three JSX-typing and two flow gaps. Pending
expression/await operations and the unexpected instantiation-limit acquisitions
remain implementation/conformance work; these Partial results do not close Q3.

The release diagnostic binary hash is
`e76f9c44dcde3e1523e912935d4509d50f1be7b95fc71a1d830aa644e1012505`.
It precedes the constructor projection fix; captures bind their actual source
and tooling hashes rather than claiming to represent all later changes.

## Bounded executable verification

Cache validation now hashes the executable in 64 KiB chunks instead of reading
an executable-sized Buffer for every query. It still validates the entire file
and propagates read failures. The cache test corrupts a byte beyond two chunks;
validation rejects it without rebuilding or treating the entry as absent.
The cache test and fresh native index controls pass at
`vrun-32f3f715-c99c-4ad6-99ed-efcfd44d674c`, two executed tests, zero skips.
This bounds hash scratch allocation; it is not a new measured production RSS
claim.

## Exact native literal projection

The older literal test expected lone high/low surrogates to be rejected. The
accepted native-byte projection now represents both exactly, as `eda080` and
`edb080`, with native escaped summaries `"\\uD800"` and `"\\uDC00"`.
Its assertion now requires Found, lossless distinct byte parts, a complete graph
and no issues. The existing pinned literal and full native-string controls
establish the compiler values; no production rejection was removed.
`vrun-674cebfa-780b-455f-81c9-8bad90c63336` passes both final-query targets with
zero skips.

## Await operand contexts

The shared source/evaluation contextual-type operation now implements
TypeScript 7.0.2 `getContextualTypeForAwaitOperand`. It obtains the parent's
context, reads its awaited type without an alias, then creates the actual
PromiseLike constituent and native union. Absent context and actual nil stop
before the factory. Yield inference remains a separate excluded question.
The new operation checks request depth and accounts for its two-element union
scratch; whole-evaluator accounting remains open.

The public query reproduced the corpus pending-operand failure at
`vrun-7a4791c7-2f87-4d72-a1f5-6a5175c7bc52`. Native controls cover absent,
variable annotation, function return and recursive-thenable/nil contexts, with
strict, loose and missing-PromiseLike profiles. The new non-forcing observer
records only results of native calls already made. It verifies early stops and
actual emptyGenericType consumption, with one checker per request.
`vrun-aee024de-c3d6-41ab-a6dc-e2b7ef011783` passes all 12 native controls,
including exact diagnostic-code assertions, with zero skips. Its previous compiler-build timeout has no semantic verdict.

A missing checked PromiseLike getter can leave the compiler's final property
type healthy after its factory falls back to unknown. Product admission still
preserves that consumed recovery. The variable case attributes it to await;
the return case preserves the earlier async-function context consumer. The absent/nil cases
avoid that factory and remain healthy. Existing comments calling this getter
quiet were corrected: the pinned resolver uses reportErrors=true.
This fixture proves the owner through final queries; the release corpus binary
above predates this fix and has not been recaptured.

The new public-query matrix and existing async-return, contextual-recovery and
promised-recovery targets pass at
`vrun-572254da-06ff-4558-b730-d64dd6dd0b64`, zero skips. An earlier failed
assertion expected the return-context recovery at await instead of its earlier
async-function consumer; the production attribution was preserved.

## Captured working-directory failure

Module-name generation needs `Program.GetCurrentDirectory`; absolute
configuration loading and lexical operations do not. Loading now retains the
failed cwd observation in the frozen module-spelling input rather than failing
all operations. The existing generation operation consumes that Result and
preserves Input/Io when it reaches the host dependency. No replacement cwd is
invented, and queries perform no new filesystem observation.

The original full replay's final service/CLI regression was RED at
`vrun-46d9e536-476b-4e39-8aa4-19d5126fbf0b`. The expanded tests cover relative,
absolute and absent operands, text/JSON, direct/service, and healthy/failing Q4
queries with and without expansion. A service started from a deleted cwd keeps
that captured failure when a later caller has a valid cwd; its generation stays
one. Both fresh spelling and existing-import reuse consume the retained failure.
All three targets pass at `vrun-5b38a0f3-f196-43bf-a127-182e03635028`, zero skips.

## Latest verification and remaining gates

Full Clippy passes at `vrun-52890664-bf0e-4b47-9053-0037976e4094`.
Final formatting passes at `vrun-2a403bfb-6741-4e2e-a593-1c76838ee456`.
All checks above identify their exact scope; the original full replay remains a
historical result and is not relabeled as a full pass on the latest source.

Three of its 20 failing Rust targets are now repaired and have subsequent
passes: constructor deprecation, removed-cwd service behavior, and the stale
scalar-only literal rejection. Seventeen originally failing targets remain
open: member_unknown_signature_provenance, type_apparent, type_elaboration,
type_intersection_limits, type_member_values, type_missing_properties,
type_mixed_alias_variables, type_module_wrappers, type_operator_recovery,
type_query, type_query_parameters, type_reads, type_recovery_calls,
type_recovery_continuation, type_recovery_display, type_special_expressions,
and type_union_limits. Classify each against the pinned acquisition and product
admission contract before changing its expectation or production behavior.

The next delivery work also includes remaining Q3 corpus expression/JSX gaps,
unexpected instantiation-limit acquisitions and empty grouping, the explicit
summary-format control, whole-evaluator resource accounting and calibration,
and final service/compatibility proof. Repeat the release corpus on the final
binary and preserve its source/binary/capture hashes. These gates still prevent
claiming completed T/Q3/Q4 delivery.

The final library target plus retained-storage, fresh-type and index-symbol
regressions pass at `vrun-abf8ffd6-ffcb-496d-9e78-ae2e3c056703`, zero platform
skips. The library's external program-listing diagnostic remains explicitly
ignored. The final Project still satisfies its 8 MiB retained-heap budget; this
check does not repeat the earlier large-corpus performance measurement.
The scope record includes subsequent receipts and a separately labeled latest
source inventory, rather than attributing its bytes to historical runs.


## Completion follow-up: recovered acquisitions and expression owners

The follow-up continues under the same completion contract: a healthy final
compiler root does not erase recoveries acquired while producing it. Cold
TypeScript 7.0.2 controls now explicitly observe unknown-signature returns,
non-null checking returns, and raw symbol-resolution diagnostics. The revised
assertions preserve repeated acquisitions in order rather than deduplicating
causes or tolerating invalid dependencies.

The synthetic module alias fix follows the native split: `getTargetOfAliasDeclaration`
accepts a missing declaration and returns nil; unresolved `resolveAlias` and
`getImmediateAliasedSymbol` still require one, while installed target links are
usable without a fabricated declaration. The original module-wrapper regression
is RED at `vrun-c1acd1d2-568c-4b3d-9084-8bb7c693bffe`; the native matrix is
`vrun-ddeb0339-87b0-4870-b87b-898239fb46b4`, and final queries pass at
`vrun-adea1193-fa1d-4789-a879-a5db455c5622`.

Regular-expression expressions now dispatch to `checkRegularExpressionLiteral`'s
actual RegExp global acquisition. The fixture distinguishes a healthy interface,
a missing global, wrong arity and wrong kind, plus escaped literals and dependent
property reads. RED is `vrun-d6f379b6-a313-457d-9942-9a8c944c0bfe`; native control
is `vrun-0c66daf9-9cd0-4069-b645-af4e5f818b8c`; GREEN is
`vrun-5068c8ef-7f2a-46dd-ae7c-9b62ea87eff4`. Invalid global fallback remains a
consumed recovery even when the native literal root is an empty object.

`typeof this` now uses the existing `checkThisExpression` owner. Instantiation
expression display follows `createAnonymousTypeNodeEx`: reuse a written TypeQuery
only when its acquired type has the exact identity being printed; retain the
visited guard and structural fallback for other syntax and composite constituents.
The existing type-query repros failed at `vrun-fa5730bf-3863-49db-b9c3-77db49775b19`
and `vrun-9bea185e-5e0d-47fb-8803-24cf4ca89381`. Type-query, parameter, runtime
instantiation-expression and this-expression targets all pass at
`vrun-027128d1-830a-4803-b2ce-b29a008d5542`.

All three JSX result expression kinds now report the design's explicit
`jsx_typing_required` exclusion. The behavior RED is
`vrun-51b2bc45-2237-43ef-af7d-ff292733b4c5`. Existing attribute gaps and new result queries
pass at `vrun-480aef0a-830f-4d6b-be5e-5a499217a4f4`.

The numeric-unary controls establish that non-null checking consumes errorType
for both null and void operands under strict and loose null options, despite a
healthy number root. Logical negation and the diagnostic-only unary bigint case
remain valid. The combined native unary/read-acquisition control passes 23 tests
without skips at `vrun-95dd25a5-4c55-4e83-a530-5fda41eba95f`. Final unary queries
pass at `vrun-3ad28e93-4c95-4e12-bf30-80a61374413a`; final read admission passes
at `vrun-0677f2f1-2f09-49c3-bd1c-6c6d69d3e595`. Unknown-signature provenance
passes at `vrun-89ae497a-bc0b-41c8-89b4-2e9de148b5b3`.

## Explicit full native summary control

The optional oracle `noTruncation` request adds only native TypeFormatFlagsNoTruncation
to the default TypeToString flags; it leaves compiler options and query history
unchanged and prints once. Its exact-byte behavior RED is
`vrun-3b38415e-c0f5-41a3-ab4e-db05ca568001`, and its GREEN is
`vrun-6728fd7e-b721-4e89-ad05-999cb2fa0bfa`.

The frozen single-thread corpus's predicate-callback result is exactly 431 bytes
under that flag. The cold native capture passes at
`vrun-52df91ef-e543-4bd3-b9a7-1a12fbd2ff26`, with exact original program/input
hash verification, no source modifications and no skipped controls. The capture
is `fixtures/types/command-center/aa484f4-2026-10-12/single-threaded-native/no-truncation-observations.json`;
its driver and input tooling are archived under the adjacent
`captured-tooling/no-truncation` directory. The temporary external-corpus test was
removed from the default harness suite after capture. This closes the required
native spelling observation; release comparison and remaining delivery gates
are still in progress.


## Contextual grouping and remaining instantiation mismatch

The unfiltered member search now creates groups from actual found references.
Its native contextual seed can belong to an interface group, without adding a
second empty group for the implementation seed. Filtered and entirely empty
results still retain the selected seed metadata required by the product contract.
The portable contextual seed fixture is RED at
`vrun-9bc4ed4d-e563-434b-92d2-50b04b1eeb73`, its pinned compiler observation is
`vrun-e592f327-a478-4816-8b07-dfc55f157c56`, and its final-query GREEN is
`vrun-5aa068da-ea95-47c3-949c-6e2f55c64fda`.

The corpus comparison now rejects arbitrary omitted occurrences, unexplained
empty groups, resource/instantiation gaps, omitted causes and gaps without a
native candidate range. Only the explicit flow and JSX exclusions can remove a
candidate occurrence from comparison. The comparison's behavior REDs are
`vrun-b057e167-93bf-4e30-a86b-70bf017acd9b` and
`vrun-c87b5d2d-5cde-424b-a9cf-cdb041ce378d`; its GREEN is
`vrun-98613646-89f6-432d-83ec-f7d6bb7bf68b`. These test the comparator, not corpus
conformance on the final executable.

A constrained call to a schema factory reproduces the remaining corpus
instantiation-limit acquisition. It has now been reduced from external Zod
inputs to the portable `native-recursive-format` fixture: a concrete interface
extends a schema interface whose result includes a recursively formatted error.
Ordinary recursive formatting and a factory returning the base schema work;
the inherited result under a generic constraint triggers the false limit.
The final-query RED is `vrun-b4256c11-9cd4-4049-9d31-f4383c77304b`.
The strict/loose native control is
`vrun-3a5b7b18-82d0-4e9d-b733-803a2ad89096`: final string, no acquired failure and
no TS2589 before summary. The production fix remains under investigation.

The release-only union boundary diagnostic passes all eight strict/loose
requests at `vrun-73d94e9a-7ea6-4d6e-90fe-72dfb9e4a8bf`. Its executable hash,
results and observed times are in
`.cc/temp/completion-next/release-limit-observations.json`. This does not replace
the registered Rust regression, which still hits its unchanged 30-second
request deadline in the unoptimized test profile. Removing full-inventory
copies from a property lookup does not by itself close that failure:
`vrun-a89fb0fa-a5ae-4d88-b965-ba5b8ab70b8f` passes the long flat-list control but
still times out on the healthy subtype boundary.


## Recursive mapped substitution correction

The remaining portable false instantiation limit came from the mapper factories.
TypeScript 7.0.2 `prependTypeMapping` and `appendTypeMapping` use
`newMergedTypeMapper`: apply the first Map, then the second Map. Both Rust
helpers incorrectly selected `combineTypeMappers`, whose composite mapper
recursively instantiates the first Map result through the second mapper. A
recursive mapped argument consequently acquired additional indexed-access
layers that the compiler does not construct.

Both helper owners now use `merge_mappers`. `substituteIndexedMappedType` retains
its explicitly different native composite operation instead of borrowing the
append helper. All helper call sites were checked against the pinned native
operations; signature and anonymous-key cloning keep their explicit composites.
No native instantiation count or depth limit changed.

The original portable final-query RED is
`vrun-b4256c11-9cd4-4049-9d31-f4383c77304b`; its strict/loose compiler control is
`vrun-3a5b7b18-82d0-4e9d-b733-803a2ad89096`. The corrected public queries pass at
`vrun-48a0a610-eb43-48d5-af02-ce6ba818eb85`. A zero-demand native relation trace
(`vrun-2c60036d-0786-4a95-bc1c-c3ad8dede6fd`) helped locate the divergence;
its driver and overlay generator are archived in
`.cc/temp/completion-next/diagnostic-tooling/recursive-variance`. Temporary Rust
and compiler tracing has been removed from production tooling. The expanded
strict/loose/exact, reduced-control and graph matrix and neighboring semantic
checks are in progress. This closes neither final corpus nor resource gates.


## Current replay reconciliation

Sixteen of the seventeen targets listed in the earlier “Latest verification”
section now have subsequent scoped passes. The only still-open original target
is `type_union_limits`. The historical replay above remains unchanged; the
current outstanding list and all follow-up receipts are in the validation scope
record.

The expanded recursive fixture's native matrix passes 42 roots across strict,
loose and exact-optional profiles at
`vrun-128beb68-a028-444e-b296-6f7d31ab3d7c`. The Rust replay
`vrun-13a3de20-c3d0-4e17-beba-691159532726` passes all seven portable targets,
including 84 summary/graph requests in the new fixture and existing mapped and
conditional behavior. That replay as a whole is FAILED: its final temporary
external-Zod diagnostic reached SemanticDeadline in the unoptimized test build.
It must not be reported as a passed eight-target scope.

The strengthened recovery controls pass all 26 tests without skips at
`vrun-cce81938-f174-490f-b8e3-d92dbd7ad95c`. Corpus capture/comparison tests pass
all 12 without skips at `vrun-33621573-7a61-4d5c-9e15-23b9789246d8`. The replay
now uses the capture owner's exact input-byte verifier before and after each
native capture's requests and checks the full-summary control's exact consumed
input hashes. This brackets changes; it is not a transactional filesystem claim.

The test profile now enables optimization while retaining debug assertions.
This configuration-only change is intended to run native boundary fixtures
within the unchanged 30-second production deadline. Its union/Zod replay is
pending; no test-profile timing is claimed as a release/service measurement.

The optimized boundary replay subsequently passed both union-limit and external
Zod targets at `vrun-c8186690-875d-4647-b05a-e8dd0be65914`, with zero skips.
Every originally failing integration target now has a subsequent scoped pass;
this is not a full-suite pass on one final source snapshot. The temporary Zod
test has been archived outside default discovery; the portable recursive-format
matrix remains in the suite. Production limits are unchanged.


## Mapper traversal accounting

`TypeMapper.Map` now checks the shared request controller at invocation and
charges every visited source comparison in ordinary, deferred and erased
mappings. It borrows the stored source/target table instead of cloning it for
each lookup; Mapper no longer implements Clone. `InferenceTypeMapper.Map` owns
its separate context-slot scan and now charges every visited slot there too.
No native instantiation limit, mapper identity or substitution order changed.

The first public-query work RED is `vrun-dc629e65-442f-453b-b3fd-2e0be6ec464b`:
early/late parameter selections had identical work at widths 8 and 128. Its
initial GREEN is `vrun-bbf5d8ae-cbcb-45c0-82ac-429ad7b44e62`. A separate omission
of only comparison charges, retaining invocation guards, fails all three
ordinary/deferred/erased witnesses at
`vrun-a3cf8164-9de9-48f8-ab71-24dad3a5a01f`; the other 153 tests pass. Those
charges were restored. The inference witness subtracts the ordinary-substitution
difference measured by equivalent explicit calls. It is RED at
`vrun-9fa3323b-5998-4013-842f-7406bea7a0c5` with zero residual growth at either
width; the other 156 tests pass. The final restored/expanded library replay passes at
`vrun-933dcb1f-e9b1-4034-872a-6be357344e4c`, with zero platform skips.
Formatting passes at `vrun-3546177a-d0b5-462d-8faa-30af614adf61`.

The cold compiler control `vrun-5bd24cd3-60c9-493d-8c3c-19dd4e680f9d` passes 40
queries under strict and noCheck profiles. The opt-in `inspectMappers` observer
counts actual Map invocations and nesting without adding a type demand or
repeating a comparison. It verifies each fixture reaches its intended mapper;
explicit callback arguments omit the inference-mapper invocation present with
inferred arguments. Exact consumed input hashes and counters are recorded in
`.cc/temp/native-mapper-resources.json`. The tests assert final number answers,
exact work boundaries, a cheap/expensive discriminator, flat checked depth and
healthy semantic successors.

An attempted inheritance depth witness was rejected: it fails with the depth
guard both omitted (`vrun-6e1f77c4-1368-4782-9a53-2cfba5eacf6b`) and restored
(`vrun-c47230e2-8bd3-486a-9bf1-d7569fb2446c`). Native observations show flat mapper
calls in that fixture. Its source and observations are archived under
`.cc/temp/completion-next/mapper-inheritance-witness`; it is not a depth proof
and is absent from the shipped test matrix. Mapper-specific depth dominance,
mapper construction/cache payloads, other evaluator families, calibration and
teardown remain open.

The corpus wrapper now snapshots source, vendored dependencies, Cargo inputs and
build configuration before its release build. Both direct and service replays
verify that snapshot and the captured compiler inputs, and require a completed
replay marker. The service pass additionally compares exact direct-response
bytes, requires one project generation and stops its private service. This is
validation wiring, awaiting its actual paired replay; it is not yet corpus or
service acceptance evidence.


## String-resource controls and deadline diagnostic

The mapper checkpoint's full Clippy gate also passes at
`vrun-a3ece6a3-deb9-4aa1-b6e9-c38de8dbbb78`. The three-file tooling checkpoint
passes 19 tests without skips at `vrun-e5f20cec-5817-4d8b-9b16-d4e3fcb32c4c`.

The new `native-string-resources` compiler baseline passes 72 cold queries
under strict and noCheck at `vrun-699e9146-2ad5-40c5-aa75-e0f76661ff5b`.
The opt-in observer counts actual `applyStringMapping` calls without adding a
semantic demand. Each chosen root maps once, its cheap twin never maps, and all
finish as number without errorType or demand failures. Cases include all four
intrinsics, expansion, both Final_Sigma contexts, a lone surrogate and an empty
string. Exact inputs and observations are in `.cc/temp/native-string-resources.json`.
The observer's missing-feature RED is `vrun-ccdaab0b-2001-44e7-9384-fb6cecf080c1`;
the first restored run timed out compiling Go at 180 seconds
(`vrun-2cbafa67-0d81-4c7b-b521-480c69c01ff2`) and supplies no native verdict.
No production string-resource change or Rust resource GREEN is claimed yet.

The completed paired release replay is archived under
`.cc/temp/completion-next/corpus-mapper/`, with binary SHA-256
`574ee9a291f6f4ebc80bcfaa8683364f2a2738736d9d5ff082b1099d2c0d3420`.
Both direct and service passes complete all eight requests and pass six:
all five Q4 roots and the JSX Q3 root. The two other direct Q3 roots return
SemanticDeadline at the unchanged 30-second limit. The service roots complete
as Partial with one unexpected unsupported-semantic-form gap each, from a
bodyless dependency constructor incorrectly admitted for property inference.
All six successful service responses equal their direct responses exactly.
The service retains one PID/generation and its process exits after stop;
the observed stop interval is 2,220 ms. This is ordinary teardown evidence,
not worst-case teardown, queue-latency or full service acceptance.

One-second process samples separate load/link work from controlled semantic
reparsing and show substantial clock-polling cost in the latter. These samples
are diagnostic, not a calibrated performance comparison. They motivated the
bounded polling increment below; the 30-second deadline remains unchanged.

The registered full Rust replay `vrun-f8199d3c-4075-41c7-8808-8947d189052f`
times out at 900 seconds while building integration targets, with no captured
execution output; no targets from that scope are counted as executed. The
registered semantic compatibility gate passes at
`vrun-e6d7cea7-b491-427d-9ffe-eca0f190bde3` on the pre-constructor-fix source.

## Bodyless constructor selection

The reduced final-query RED is `vrun-bcc1e418-7f5c-441b-b5b6-90d119c025af`:
an unannotated private property in a declare class fails with the same pending
constructor-flow producer seen in the corpus. The compiler control passes at
`vrun-c70e663f-44ba-42ce-bd9b-bb94c2f883a0`, covering declaration files, declare
classes, overload-only constructors, absent constructors, static properties,
and an implemented annotated control under strict, implicit-any and noCheck.
`FindConstructorDeclaration` selects the first constructor with a present body.
The new shared syntax owner uses `NodeIsPresent`; both class checking and
property inference route through it. The restored-source Rust replay passes 90 summary/graph queries at
`vrun-fd24a434-100a-4c89-9dea-ad6caa8dac1c`, with zero skips.


## Bounded deadline polling

The final-query RED `vrun-d7be7ab6-61e6-4d35-ae75-55f43d2658ca` measures
3,201,018 clock observations for 3,200,952 work units while inspecting a number
declaration after a long comment. The new clock-cost assertion fails; the
other 157 library tests pass. Its observation counter exists only in test builds.

Control now amortizes wall-clock observations across 1,024 scalar work or
scanner/grammar checkpoints. Every debit still checks exact work, requested
allocation and depth limits; every checkpoint immediately propagates the
sticky first interruption. Large work debits poll before the operation and
exhaust the interval when necessary, forcing the next checkpoint too. Forced
clock checks remain at worker entry/completion, before allocations and around
indivisible native numeric parsing. No semantic, grammar or native compiler
limit changed. The whole-library GREEN is `vrun-4e0ec0c8-9a24-4068-9e5a-487248d71720`.
The subsequent paired corpus replay passes as recorded below.

The final-query clock-frequency gate proves at least an eightfold reduction
in clock observations relative to its measured work; it does not prove an
unchecked bulk operation's latency, complete evaluator accounting, calibrated
production work/bytes/depth maxima, worst-case stack or service queue bounds.
The existing remaining resource gates still apply.


The expanded library replay passes at
`vrun-57acf1e0-bc87-4021-a812-1d9960d082d6`, including actual expiry with
1,023/1,024/1,025-unit debits and expiry after the last real type demand. The
former verifies the periodic checkpoint itself aborts, rather than relying on
the worker's later check; the latter separately verifies that final check.
Both finish as whole SemanticDeadline errors with healthy subsequent queries.
Formatting passes at `vrun-2d5dc2f1-3727-4a34-be15-33d821b64c42`.

After the clock-cost fix, Cargo's default unoptimized test profile passes all
five native boundary/recursive targets at
`vrun-7b3671a4-4448-425b-8819-f16fe4be77a5`, with zero skips. These include union,
intersection, template and conditional-tail boundaries and recursive formatting.
The temporary test-profile optimization workaround is therefore removed; debug
assertions remain enabled. This is test-profile validation, not a release
latency claim. The paired corpus replay below is complete. The intermediate
full Rust/harness replays were subsequently stopped as recorded below.

The corpus comparator now reports a whole-query error's actual product kind and
reason instead of failing while accessing an absent coverage field. Its RED is
`vrun-9edab4f3-57f8-4305-93ed-1a232b177a13`; all four comparator tests pass at
`vrun-49e8d82c-911a-47e6-97ec-33c141fae34c`. This improves failure reporting and
does not admit a failed query or weaken native range comparison.


## Paired corpus after constructor and polling fixes

The fresh release replay passes all eight direct requests and all eight service
requests. Its input-bound captures and build snapshot are preserved in
`.cc/temp/completion-next/corpus-constructor-poll/`; both reports have
`replayComplete: true` and `passed: true`. The binary SHA-256 is
`d5278896b10b7f3176c26f7dbece8952d21c311be2b93637c4d948505339dbe7`.
The wrapper verifies source, tooling, binary and native-capture hashes after
execution. This remains local wrapper-development evidence, not a registered
Command Center gate.

The annotated and contextual Q3 results each establish five native occurrences
and retain exactly 30 flow-required and two JSX-typing gaps. The JSX Q3 result
retains six JSX-typing and two flow-required gaps. These are the declared scope
exclusions; no pending-expression, constructor-flow or instantiation-limit gap
remains in these corpus queries. All five Q4 roots are Found without issues,
including exact agreement with the native NoTruncation predicate summary.

All eight service responses match their direct bytes. The service keeps PID
76895 and generation one throughout. Direct elapsed times, including separate
project loading for each request, range from 33.7 to 51.8 seconds. The first
service request takes 55.5 seconds including loading; subsequent Q3 requests
take 14.3 and 5.4 seconds, and Q4 requests take 2.2–2.9 seconds. No request hits
the unchanged 30-second semantic deadline. Host load is uncontrolled; these are
observations, not calibrated latency maxima. Stop succeeds and process exit is
confirmed after 2,527 ms. Worst-case teardown, queued lexical latency and stack
headroom still require their separate proofs.


## Preliminary queued-lexical deadline measurement

The local diagnostic scripts `.cc/temp/completion-next/queue-probe.py` and
`queue-probe-384.py` use the same frozen release binary above. They load a
private project with one declared number and a long comment, establish the
lexical baseline, start Q4, observe the named semantic worker with a one-second
process sample, and submit a lexical request while Q4 is still running. Each
owns a private service directory, collects both clients and confirms service
exit before cleanup. These diagnostics fill a concurrent-timing scope the
registered commands cannot express; they are not registered validation gates.

The 64 MiB control is Found as number in 7.08 seconds. The 384 MiB source
(402,653,219 exact bytes) produces a whole pathless SemanticDeadline, with no
causes or partial result, in 32.16 seconds including pre-semantic refresh. Its
queued lexical request finishes in 31.85 seconds, versus a 1.88-second standalone
lexical baseline. It is byte-identical to that baseline. A subsequent outline
succeeds, and status preserves PID 67202/generation one. Stop and confirmed exit
take 0.14 seconds. Reports bind the binary and input hashes and preserve raw
responses under `.cc/temp/completion-next/queue-probe{,-384}/`.

The 384 MiB queued observation fits 30 seconds plus its measured baseline
without an added slack allowance. This is one deadline workload under actual
host contention, not a calibrated maximum or a complete service acceptance
claim. It does not prove deterministic-limit exits, maximum evaluator teardown,
stack headroom, all unchecked units or queue fairness. Production limits remain
unset while accounting and calibration are incomplete.


## Intermediate compatibility replay disposition

The constructor/polling snapshot passes the registered semantic gate at
`vrun-639d7532-d84b-4d72-9f94-2d429a6e3ae8`. Its batched replays pass 61 Rust
roots (the library and 60 integrations) and 82 harness files before the remaining
production accounting work resumes. Exact scopes and initial input hashes are
in `.cc/temp/completion-next/full-{rust,harness}/`; all hashed inputs remained
unchanged through stopping. Their `stopped.json` records explicitly retain
`replayComplete: false`.

The active Rust scope `vrun-a3843941-72ed-4832-87a0-79ba1a3d38af`, harness scope
`vrun-b9064b38-143f-41b0-a0d8-cc4c9fa2c2f2` and Clippy scope
`vrun-dc7813ec-db3d-4856-9f89-8b93ba443399` were deliberately cancelled. They
supply no passing scope or additional executed-target claim. Final full
compatibility and Clippy checks remain required on the resulting source.


## Unannotated property inference selection

The final-query RED `vrun-effda93d-99eb-4233-9cd7-09252bb92ce2` reproduces an
explicit declare property's missing base-class fallback. Source now distinguishes
no inference, automatic constructor/static-block inference, and the own Ambient
modifier's base-class fallback, in native order. Constructor/static-block presence
is specific to instance/static properties. The base case uses raw declaration.Symbol
and the existing `getTypeOfPropertyInBaseClass` owner, including the first-base
rule and ordinary optionality. Automatic/evolving inference retains the design's
explicit FlowRequired exclusion rather than a generic pending-operation reason.

The initial 40-root compiler control passes at
`vrun-edfbf18a-e7bd-4eb2-a60a-2a7f3301cf32`. The first repaired Rust run
`vrun-eb3405a5-4e02-432a-8cc0-98dae1b7e7e8` exposes a separate fixture dependency:
a missing base property consumes missing global Object recovery. That failure is
preserved. Healthy fixtures now declare globals, and a separate missing-globals
control asserts ErrorType with its UnresolvedSymbol cause and a healthy successor.

The expanded Rust matrix passes 114 summary/graph/recovery queries at
`vrun-71c97c30-3618-4f0d-8c12-3541c79e9221`, zero skips. Native controls cover
70 cold requests across strict, implicit-any, null-loose, noCheck and missing-global
profiles, including opposite-kind constructor/block presence and Ambient precedence.
They pass together with the 96 string-resource controls at `vrun-b0a85ef7-7506-4f8e-8d81-b757a67c82d0`.
Static declared properties deliberately use the same native declared-class base
lookup; the fixture distinguishes its number result from the base constructor's
same-named string property. No constructor or static-block flow types are invented.


## String casing resource increment (scoped proof)

The expanded native string baseline covers all four intrinsics, UTF-8 payload
expansion through lowercase U+0130, supplementary first characters, lone
surrogates, both Final_Sigma contexts, a same-width uncased-prefix control, and
empty strings. Each chosen root maps once before returning number; each cheap
twin avoids mapping. The 96-root 16/4096 baseline passes at
`vrun-5d08644d-e7f1-49d6-8af3-b7bae5beaf85` and is archived with fixtures and
oracle under `.cc/temp/completion-next/string-4096-baseline/`.

The final-query RED `vrun-dc64afb1-b68c-4c9c-a07f-e6bc0bac188b` fails all 22
independent work/allocation growth assertions, with the other 160 library tests
passing. To reduce inner-loop cost, the large fixture is narrowed to 512 runes.
Its fresh 96 native controls pass at `vrun-b0a85ef7-7506-4f8e-8d81-b757a67c82d0`,
and its RED `vrun-b4567e73-9983-403a-a2fd-5c0cbb9da0b2` retains exactly those 22
failures. These are behavior failures, not setup or compilation errors.

Casing now receives the existing optional request Control at every apply caller.
The upper/lower owners debit decoded runes and bounded exact-byte copies; lower
also debits actual sigma lookahead. Growth uses the existing reservation policy,
and capitalize/uncapitalize account for their unchanged tails. Vec-to-Arc payload
copies are charged before conversion, with forced checks around the indivisible
standard-library copy. Rune encoding remains one native owner and can write to
a four-byte stack buffer without adding a second decoding rule.

The full library passes at `vrun-27482ab5-788c-4a32-a221-95a896831bc0`, including
all 22 work/payload controls under strict/noCheck, measured just-under/at/over
limits, flat-width depth and healthy successors. Exact string-value and
constructor/property compatibility pass at
`vrun-ea16964a-0ad2-46eb-8e83-e02099366e6d`, zero skips. The lookahead-only omission fails exactly its new discriminator at
`vrun-8504a523-e193-43e7-a8b0-15f3db77ed9a`: small and wide residual work both
remain zero, while the other 182 library tests pass. The preceding run
`vrun-7ef49e12-5527-4429-906b-e8c3bfc7a1d0` failed to compile after a test-helper
refactor and is not behavior RED.

The independent work-only omission preserves all casing allocation charges and
fails the eleven work tests plus that lookahead test at
`vrun-ad899448-53e2-4423-84f3-f9cb04e5fed9`; all eleven payload tests pass.
The allocation-only omission preserves work, forced checks and the exact capacity
growth policy, and fails exactly the eleven payload tests at
`vrun-b42a580a-2ea9-4174-a00f-f97246ff8a6f`; work and lookahead tests pass.
The exact mutated source hashes and terminal receipts are retained under
`.cc/temp/completion-next/string-*-omission*`. Both source files were restored
byte-for-byte afterward. Final restored-source verification follows below.

This is a casing-buffer/copy increment, not whole-string or whole-evaluator
accounting. Canonical coalescing and surrogate normalization are addressed by the next
increment. General literal materialization and interning, comparison/search,
other evaluator families and calibrated
production maxima remain open. The successful paired release corpus above
predates the property-inference and string-accounting changes and must be
repeated on the final delivery binary.


## Template concatenation and surrogate normalization (scoped proof)

The pinned compiler control covers ASCII, paired and lone surrogates and
supplementary characters at two widths under strict and noCheck. Each cheap
request obtains its literal alias and returns number; the chosen request also
concatenates prefix, alias and suffix and normalizes surrogate pairs before the
same final result. A zero-demand observer records existing
`getTemplateLiteralType` invocations. Every cold checker makes one startup call
for `${number}`; chosen requests make one additional call. The initial oracle
assertion omitted that startup call and failed at
`vrun-0e42dae9-2750-4e1a-a9c6-7e1dbd94bd1f`; corrected source-based expectations
pass all 32 controls at `vrun-4f5752b5-b406-412d-b105-c813f96efe8c`.
The first Rust reproducer fails all eight new work/payload tests at
`vrun-c75dc871-cf5f-4f12-88b9-1cdac047be0f`, with narrow and wide extra work and
payload both zero. The other 183 library tests pass, including the restored
casing and lookahead controls.

The paired-surrogate input is strengthened with a source line continuation
between halves so the scanner retains two native sentinels. The observer now
records the inputs and outputs of the existing normalization call without
adding semantic demands. All 32 strengthened native controls pass at
`vrun-36212ee6-0abc-46bd-a6ad-7214980fceb8`: pairs contract from six bytes to four,
while ASCII, lone sentinels and supplementary characters retain exact lengths.
The earlier fixture/capture/oracle is archived under
`.cc/temp/completion-next/template-initial-baseline/`. Strengthened Rust RED
`vrun-d4caf437-98a5-4fee-8433-d95d73202d86` retains exactly the eight failures,
with the other 183 library tests passing.

The canonical `NativeStringValue::from_parts` constructor now explicitly takes
the optional request Control, charges each part and bounded byte copy, and
reserves both byte and part storage before growth. Vec-to-Arc copies use the
same resource owner. `CombineSurrogatePairs` charges decoded runes, pair
lookahead and output copies, using the existing exact native decoder/encoder.
Casing, templates, template-match normalization and constant concatenation all
pass their existing request handle through the literal operations. Constants
retain their distinct native behavior: concatenation alone never normalizes.
Binding-only constructors still pass no request Control; loading policy is
unchanged. String slicing, comparison/search, materialization/interner work,
template traversal scratch and general evaluator accounting remain separate gaps.

The first build `vrun-b97f7544-093e-46d0-8eff-44fd77a42a4f` catches two omitted
constant-expression caller arguments; it is a compile failure, not a behavior
verdict. After wiring both callers, the full library passes at
`vrun-a1256bee-fba0-4e6a-b23b-5056633a229e`, including all eight template controls,
strict/noCheck exact under/at/over work and payload boundaries, flat checked depth,
and healthy semantic/lexical successors. Seven exact-value/template/generated-ID
compatibility targets pass at `vrun-d1ecf5e7-e039-466b-a027-bc9d05f58e6c`, zero
skips. The source before this increment and its hashes are archived under
`.cc/temp/completion-next/template-before-accounting/`.

Earlier casing omission receipts bind the earlier source. Canonical-copy charges
now compose with casing; those earlier experiments are not relabeled as isolated
omission proof on the expanded implementation. Whole-evaluator accounting,
calibration and final compatibility/service proof remain open.


## Cached string comparison resource follow-up (in progress)

The new prefix/suffix fixtures warm both an input literal and template pattern in
identical outer conditionals. Only the chosen request compares the cached input
with the pattern; both roots remain number. Mismatches occur at the final byte
of a 17- or 4,097-byte span, so the selected comparison traverses the whole span.
A zero-demand `inferFromLiteralPartsToTemplateLiteral` observer records the
actual call and existing prefix/suffix lengths.

The first 16 native controls pass at `vrun-93ffe081-8b53-4a93-b1ed-a20fa4db7b46`.
The first Rust run `vrun-30c49f0d-1389-46f3-bbaf-dcc9cb918861` fails before its
accounting assertions because the chosen relation consumes a missing global
String wrapper. That is a fixture dependency, not accounting RED. Its fixtures
and compiler capture are archived under
`.cc/temp/completion-next/string-comparison-missing-globals/`.
The fixtures now declare empty global String and Object interfaces, preserving
the production failure contract; the corrected 16 native controls pass at
`vrun-56922560-ae71-4501-b906-a09a0844b03d`. Corrected Rust RED
`vrun-c115027b-86d6-4304-9deb-b5da5f38168a` fails exactly the two work tests:
chosen-minus-cheap work is 46 at both widths. The other 191 library tests pass.

The staged implementation carries the existing optional Control through native
string positions, canonical equality, semantic comparisons, delimiter searches
and slice construction. Exact byte comparison is split into the existing
1,024-unit polling chunks; it neither decodes bytes nor gives opaque identities
a decimal spelling. Part walks and scratch growth use the shared work/reserve
owners. The first full library GREEN is
`vrun-3d639a49-c0da-459e-91c5-63482d39af3f`; the nine exact-value, template,
generated-identity and ordering targets pass at
`vrun-948607d4-8212-4ee7-ac5b-ea09164ae5f9`, zero skips. A follow-up refactor
routes ordinary canonical equality and controlled equality through one owner,
with exhaustive byte/identity alternatives. Clippy caught derived Hash beside
manual PartialEq (`vrun-36ca0d1b-a6e8-42f3-b089-7dda4ecf14a2`); explicit
variant/payload hashing preserves the canonical representation contract. The
refactored full library passes at
`vrun-21a4e42a-a8bc-47a7-8eeb-288f8835a015`. The interrupted earlier library
client left an empty receipt and supplies no verdict. Formatting before the
Hash change passes at `vrun-388336d3-8e89-4f63-b115-374c76675a37`; all-target
Clippy passes at `vrun-03602d52-d85c-4988-9dae-67f68a48fe57`. All nine exact-value/template/generated-identity
compatibility roots pass at `vrun-32fd04e2-cf2d-41c4-9a60-f5794400d6fa`,
zero skips.
Hash/interner work, general literal materialization, binding-only string calls,
template traversal scratch and the remaining evaluator families stay open.


## Cached relation property traversal (under implementation)

The first declared-property witness passes sixteen native controls at
`vrun-a0ead140-00fa-43be-8a2a-9fadbbfc4a0d`, but captured declaration lookups
already charge work through Source. Mapped properties remove that incidental
cost. Empty interfaces over the same mapped base are not a valid replacement:
native `getSingleBaseForNonAugmentingSubtype` normalizes both to that base before
property comparison. The mapped/empty-interface RED
`vrun-9fd9ad1e-f8b1-47c3-903c-2f05b1d7b5a4` therefore establishes only a fixture
failure, not missing traversal accounting (extra work is 50 at both widths).
Intermediate native failures and fixtures are retained in the validation index
and `.cc/temp/completion-next/relation-properties-*-baseline/`.

The corrected fixture gives each interface one own property, preventing that
normalization, and retains 16/256 identical inherited mapped symbols. Both
requests warm both member inventories; only the chosen conditional relates the
two types. Required and optional mapped properties are covered under strict
and noCheck. The native observer records existing property iterations and
pointer-identity skips without adding semantic demands. The final type is
number; the chosen traversal visits width + 1 properties and skips exactly
width identical symbols. All sixteen controls pass at
`vrun-3955b0be-3f12-4aba-8e3e-7461ce54bad3`. Genuine final-query RED is
`vrun-9059b9be-0760-4539-9091-b6bd588663ad`: both property tests fail with
chosen-minus-cheap work fixed at 78 across widths; the other 193 library tests
pass. The two scan owners now guard their invocations and charge each visited
property before optional/private/excluded/identity skips. Unmatched collection
retention reserves before growth through the existing resource owner. Native
lookup, comparison, reporting and short-circuit order are unchanged. Work
GREEN is `vrun-5a1f664e-f21a-4f1c-aaf2-454c98ab2331`, including the strict/noCheck
work boundaries, flat depth and healthy semantic/lexical successors. Formatting
passes at `vrun-9f6f164b-e5e0-4229-81ee-4b31a6117bd8`.

The first unmatched-list witness passes all 24 native controls at
`vrun-49e5a419-9f8e-4bb5-a2e8-67b1dc35442c`, but payload-only omission
`vrun-fa551502-82ec-4912-8dfb-219a73492ebd` also passes its new final-query test.
It is therefore combined reporting coverage, not isolated retained-list proof.
The corrected pair warms both inventories and invokes reporting on both sides:
its cheap source lacks one property and its chosen source lacks every property.
The native observer checks actual retained counts. The corrected 24 native controls pass at
`vrun-1bdfa9b4-8365-4e92-9d0c-474a6e0de228`, but the corresponding omission
`vrun-c541b077-c98e-436e-997b-b21176ac0704` exposes unequal diagnostic display
work: cheap-side allocation grows faster because its mapped source is expanded.
That failure alone is not an accepted resource discriminator.

Both sources now have named interface identities, preventing expanded source
display while retaining one-versus-all missing-property lists. All 24 native
controls pass at `vrun-66cc01da-3f7d-4ef1-807f-b687d3260ea1`. Independent
payload-only omission `vrun-6f0f122e-afb9-44af-9912-9d5d4e79e82c` fails only the
new retained-list test: narrow and wide extra payload are both zero. All 195
other library tests pass. The experiment preserves traversal charges, deadline
checks and capacity growth. Its signal-aware runner restores the exact source
hash on exit; no omission is installed. Restored-source full-library GREEN is
`vrun-234fafd4-f677-4fb7-839e-6fc18c35fbb2`, covering strict/noCheck exact
payload boundaries, flat depth and healthy successors. Formatting passes at
`vrun-15432c54-0910-4ce6-8597-a904dfcf5fff`. All seven targeted structured,
conditional, nominal, reporting, property-flag, recovery and enum relation
compatibility roots pass at `vrun-6b73cfc8-3c61-4577-bf2a-4ac96d1a980b`,
zero skips. All-target Clippy passes at
`vrun-03602d52-d85c-4988-9dae-67f68a48fe57`.

This is not complete relation accounting. Member-inventory copies, relation
keys/caches/stacks, other pairwise algorithms and diagnostic presentation retain
separate outstanding accounting obligations. A textual direct-recursion scan
finds 96 evaluator-method candidates without an explicit local depth guard;
that inventory is a guide for the next audit, not proof of 96 distinct defects
or proof that all mutual-recursion cycles have been found.


## Generic relation-key resource follow-up (native proof)

The next witness holds arity and all warm-up reads constant, but compares
references with two repeated free parameters versus references with 2w distinct
free parameters. A preceding concrete comparison warms the common variance
inventory. The native key builder's actual `slices.Index` result determines the
observed comparison count; the observer never performs an extra semantic read
or parameter search. Root results remain number in strict and noCheck.

The initial observer expectation omitted keys built while warming variance;
`vrun-8f71ff3c-1106-4ae5-a548-1d8cc1d32c2f` fails its expected key-build count
(130 rather than one at width 16). This is an oracle expectation failure.
Corrected source-based expectations count 8w + 2 builds and 2w arguments per
build. The chosen-minus-cheap comparison count is exactly 4(w - 1)^2: 900 at
width 16 and 15,876 at width 64. The common instantiation counts are identical
within each pair (767 and 12,287). All eight cold controls pass at
`vrun-535ca0a3-34c9-472a-8944-16003ba0a64d`.

Production key numbering, key scratch, generic-reference traversal and
independent variance-argument traversal accounting are not yet changed.
Final-query RED/GREEN remains required. Long key hashing/equality, cache growth
and other relation operations remain separate obligations.


## Generic relation-key accounting (scoped proof)

The final-query RED is `vrun-cb3ffe13-becf-4d92-84b2-f37b9ed39e80`:
only the two new tests fail, with 196 other tests passing and one configured corpus test ignored.
The distinct-versus-repeated parameter comparison adds only 49 charged work
units at both widths; its extra requested payload is zero at both widths.
These observations establish missing quadratic search work and retained scratch,
independently of declaration and variance warmup. The work discriminator allows
nine times the small-width overhead, so linear constraint reads cannot mask the
native search's quadratic growth.

`GenericKeyWriter` now owns first-seen parameter lookup and checked growth of
its parameter and key-part vectors. Each actual comparison and emitted part
spends work. `getRelationKey`, `isTypeReferenceWithGenericArguments` and the
recursive reference writer guard their invocations; argument loops charge each
visited argument. The completed exact key uses a checked immutable Arc payload,
so relation-cache and Maybe-key clones share it instead of silently duplicating
the vector. Canonical numbering, constraint demand order and the native four-level
key expansion rule are unchanged. A map-based numbering optimization was
considered, but this increment retains the pinned algorithm and accounts for
its actual comparisons.

The full library passes at `vrun-b8021bae-4174-4799-b795-c1308c659713`.
Both new public-query tests cover strict/noCheck, just-under/at/over work or
payload budgets, equal peak depth for flat widths, and healthy semantic and
lexical successors. Formatting passes at
`vrun-fe441180-8097-4f3a-ba4e-ef98853bca75`.
A compatibility submission named nonexistent test files and was rejected before
execution (`vrun-9e4d1c65-8d6e-4d4d-bd9c-3709bbe0387f`); its corrected scope
is recorded separately. This increment does not cover long-key hashing/equality,
relation cache/stack growth, other evaluator families or production calibration.

The corrected four-root compatibility scope passes at `vrun-db4657a0-508c-4eeb-9710-0ba4ee1d4010`
(relation recovery, structured, nominal and conditional relations), zero skips.

The preceding string/property all-target Clippy run passes at
`vrun-03602d52-d85c-4988-9dae-67f68a48fe57`; it predates the generic-key change.


## Cached tuple element traversal (under validation)

The first tuple pair compared literal-to-number elements and returned number on
both branches. Native traversal control passes at
`vrun-67939bab-f11b-4b25-8f14-2605204c595b`; the unchanged Rust implementation
also passes the proposed budget discriminator at
`vrun-e5724a29-e2d7-4d71-9772-901cf5a3e8cd`. Other per-element work masks the
missing traversal charge. Distinct labels with equal element identities avoid
that work, but the successful conditional still performs an extra relation-key
lookup; that second candidate likewise passes without a production fix
(`vrun-03dbe2a8-2bbb-492f-bec0-be1a6988a147`, with native control
`vrun-08e28411-0c08-4807-a2c8-dee3f143a8c5`). Neither is isolated proof.
An intermediate malformed named-optional fixture was caught and its runs
cancelled; those submissions establish no behavior verdict.

The final pair differs only in rejection position: the first versus last
element. Both conditionals choose the false branch. Tuple labels retain
separate target identities, while all earlier compared elements share their
actual type identities. The pinned control observes exactly one versus 16/256
element visits for required, optional and readonly tuples, strict/noCheck:
`vrun-a57f998a-1f65-44d2-8085-5db576119ca0`, 24 cold queries. The actual Rust
RED is `vrun-a58d8a19-7794-484c-8a4b-62c3472071aa`: all three new tests fail
with zero extra work at both widths; the other 198 tests pass.

The tuple branch of `propertiesRelatedTo` now guards its invocation and charges
each source element before flags, exclusion, identity or failure shortcuts.
The following validation combines its GREEN checks with a new cache-payload
RED. Tuple edge-count scans, tuple-target ordering and other tuple operations
remain separate accounting questions.


## Relation cache capacity (under validation)

The preceding tuple checks pass in the combined library run
`vrun-3b0aeab8-3bdc-49fe-aaf7-00354f2f1bfa`; its only failure is the new
cache-payload test, with zero extra charged payload at both widths. All other
201 library tests pass. Native control `vrun-67bad7b1-2f39-4b84-ad1b-5fbad6377852`
observes one committed entry for repeated comparisons and 16/256 for distinct
comparisons, with every root number and no acquired recovery.

`RelationsState` now receives the existing request Control and owns
`Relation.set` capacity reservation. Failed comparisons, successful Maybe
commits and root overflow writes all use that owner. The outer relation-kind
table also reserves before growth; existing entries do not request replacement
capacity. These changes preserve native commit order, cache sizes and allowance
decrements. Active sessions, provisional stacks, retained failure copies and
long generic-key hashing/equality remain separate accounting obligations.

The first post-change run (`vrun-b6251e37-5d2f-4c35-98ac-ea3ee80c5568`)
passes the payload discriminator and exact limits, but exposes an invalid flat-depth
assumption in the fixture: its comma chain creates a deep AST (52 versus 772
checked levels). This is not a passing gate. The revised fixture uses flat object
property lists for warmup and comparisons. Its eight native controls pass at
`vrun-2cbdd31d-6d1c-4468-9a0f-b93a1ae4436b`, retaining exactly the same one-versus-
width cache schedule. It still requires the revised final-query and omission
checks; the depth assertion has not been weakened.

Seven relation/tuple compatibility roots pass at
`vrun-862ce08f-fe3d-4489-b10d-401cec560a76`, zero skips. Formatting passes at
`vrun-eb37c3d5-5130-4c5b-84c4-6e7ea57d6826`.


The flat-cache full-library run passes at
`vrun-feec05c9-a88e-40aa-8ee5-cee7dd07e478`, including exact payload boundaries,
equal peak depth at both widths and healthy semantic/lexical successors.
A capacity-debit-only omission check is running before this cache increment is
closed. The earlier all-target Clippy run was cancelled to release the shared
Cargo build lock for this source-mutating check; it supplies no pass. Clippy
will be repeated after the exact source is restored.


The independent cache-capacity omission fails only its new payload test at
`vrun-da086f5e-eb7d-4f56-ae5d-781fb524f386`: both widths again have zero extra
charged payload, while all 201 other tests pass. Work/deadline checks and the
capacity policy remain intact. The runner restores the exact source SHA-256
`813bb2ad114918b58200dadcf5e12374610e09789980b53f41e55d367860c8dd` and removes its
active marker. The prior flat-fixture GREEN uses this same production source.

## Relation recursion stacks (native proof)

The next control compares shallow and nested trees with equal numbers of
unique relation pairs: 9 at width 8, and 65 at width 64. Both queries warm the
same member inventories and return number. The native observer reads existing
stack lengths immediately after their actual pushes; it adds no semantic reads.
All eight controls pass at `vrun-1c09c1c8-ab37-4975-93ab-f17ca2404d42`.
Shallow peak source/target depth is two; nested peaks are 9 and 65. The summed
stack lengths at pushes are 17 versus 45 and 129 versus 2,145. Those sums describe
the snapshots copied by this implementation, not an allocation claim about Go.

Final-query work and payload regressions are now running. They permit twice the
linear width ratio in their discriminator and retain exact budget boundaries;
actual semantic nesting gets its own under/at/over depth checks, while the earlier
flat-width controls keep their equal-depth assertion. Production stack accounting
has not yet changed. A prior submission was cancelled after the test-edit script
rejected an ambiguous replacement, before these tests had been installed; it
supplies no behavior verdict.


The installed stack RED is `vrun-6266d3e5-7406-4fb7-a25d-e459050a44cf`:
only the two new stack tests fail, each with zero extra work/payload at both
widths; the other 202 tests, including restored cache accounting, pass.
`recursiveTypeRelatedTo` now charges active/provisional stack storage and copies
source/target snapshots through a checked scalar-copy owner. `resetMaybeStack`
reserves before draining and charges every moved/committed entry. The
`isDeeplyNestedType` and `hasMatchingRecursionIdentity` owners guard their own
invocations, including recursive intersection checks; active relation-session
capacity also uses the existing request ledger.

The first post-change run (`vrun-bc54ae35-931f-4444-b872-aa58463caeb3`)
passes the work/depth checks and all 202 prior tests. Its payload test records
4,608 versus 71,832 extra bytes, below its initially overgenerous 16-fold cutoff.
The revised payload allowance is 12: it exceeds the growth of all newly covered
linear capacity components. For grow-only Vec stacks the extra requested
capacities are 28 - 4 and 252 - 4, a ratio of 248/24; the set's ratio is smaller.
A snapshot-payload-only omission is being checked under this allowance, retaining
all stack/set capacities, copy work, depth guards, deadline checks and native
comparison order. The work discriminator retains its stronger 16-fold allowance.


The snapshot-payload-only omission fails just the new payload regression at
`vrun-14452dea-adc0-4caf-8812-9516666b34c9`: extra bytes are 4,368 versus 41,576,
while all 203 other tests pass. The runner restores the exact resources source;
the full restored library passes at `vrun-e0b792f6-c0b3-4772-a368-5e915c961496`.
This proves the snapshot debit independently of stack capacity and copy work.

## Exact relation-key identity and checked lookup

Four final-query controls warm a compatible generic relation before overload
selection tests a different parameter position, target identity, nested argument,
or constrained parameter. Each must select the fallback returning number; a
false cache hit selects `"wrong"`. Strict/noCheck native controls pass at
`vrun-9e1339d6-6561-4e9d-a8c5-08cf1aad1143`, and 16 Rust summary/expanded cases
pass at `vrun-db1f7a6e-73f1-45d3-bc01-6b392ff1b95b`.

The first fixture omitted String/Number global wrappers. Its native summaries
were number, but Rust correctly retained the documented missing-global recovery
for the constrained case (`vrun-b2617024-5b90-4378-8dd5-fcef871ad654`). The positive
identity fixture now declares both wrappers. The original input/captures remain
archived under `relation-key-identity-missing-wrappers`; no recovery suppression
was introduced. Native summary/errorType alone does not rule out that fallback.

An independent parameter-identity omission retains the writer's parameter scan
but emits parameter zero for every position. The final strict/parameter result
becomes `"wrong"` at `vrun-075d572a-a69f-4f45-8c4f-6f880df8c594`, proving the
behavioral regression. The runner restores the original source before further
work.

The private RelationTable now owns completed caches and provisional Maybe keys.
Immutable keys compute their fingerprint once, charging each generic part.
Only fixed-size fingerprints enter the standard hash table. Exact comparisons
and collision-bucket growth consult the same request Control, including cache
hits; equality retains the full native key representation. The table counts
entries, not distinct fingerprints, preserving native relation allowance.
Replacing an entry does not grow storage. A single record stays inline in its
bucket; collision removal uses bounded swap removal in private order.
Validation of this storage change and forced-collision behavior is pending.


The full library plus key-identity integration passes at
`vrun-ff1ff4f9-ac07-4cc9-9a22-e1505a5e8dce`. In a separate experiment every
fingerprint is forced to zero while retaining hash work; all 16 final-query
identity cases still pass at `vrun-8c3e0d3e-04cd-44d0-ae90-e4ac68229e0d`.
Exact comparison therefore distinguishes these keys even in one collision
bucket. The runner restores the exact source before subsequent checks.
The first wider compatibility submission names a nonexistent reporting test
and executes no tests (`vrun-964c5d16-526a-4baa-bfb3-ed6e9329673e`); its corrected
seven-root submission is pending. Formatting's seven reported hunks have been
applied. Clippy is also pending. These checks do not close general retained
failure copies or whole-evaluator accounting.


Relation-table all-target Clippy passes at
`vrun-d763dc8c-9d01-4871-bd6e-1109c4042d1e`, and formatting passes at
`vrun-8494cb38-84d0-42f5-a034-6783376415a4`. These receipts predate the tuple-edge
production increment and are not final-delivery gates.

## Tuple edge scans (native witness)

A shared warmup establishes a wide fixed tuple, a rest tuple and a source whose
first element is incompatible with both targets. Both comparisons reject that
first element; only the fixed target performs two full metadata scans. The
initial required-tuple native control passes at
`vrun-cf36048d-264f-434a-a96d-4685f8fe304f`. The expanded 32 cold queries cover
required, optional, readonly and variadic flags under strict/noCheck and pass at
`vrun-21d1b503-c66b-40cf-b40c-68d5b7dacf57`.

The variadic conditional initially stays deferred, so its initial number
expectation fails (`vrun-492a8a9a-5f4e-49b0-9e76-a9b448fcf8f0`). Its corresponding
Rust scope is cancelled before execution. The eager satisfies comparison
actually performs two reporting passes, exposed by the next counter assertion
(`vrun-775a2683-8398-490b-9cb9-9eabd45508bc`); the final oracle records both.
Every final root is number without acquired recovery. Required/optional/readonly
comparisons visit one element and scan 2w metadata entries versus three;
variadic reporting doubles both figures. The production change is prepared but
not yet applied; the pre-change resource regression is running.


The corrected seven relation/tuple compatibility roots pass at
`vrun-d928ba68-6c0f-4def-a38c-b40b29680c16`, with zero skips.

The next independent native control covers `typeArgumentsRelatedTo`: a pair of
wide generic interfaces witnesses only its first or last parameter, leaving all
others independent. Both queries perform the same variance warmup and reject
the same string/number property mismatch. All eight cold controls pass at
`vrun-73891d73-d171-48e7-b3bb-34f800c7f59d`; moving the witnessed parameter to the
end adds exactly 2(w - 1) visited independent arguments (reporting makes two
passes). A final-query work regression is staged; this loop is not yet changed.


The initial unequal-arity tuple resource check passes without the fix at
`vrun-80efc2bc-5700-47ec-aa62-86973a1d44ed`; it is not evidence for the new
accounting. After equalizing arity, native controls pass at
`vrun-0ed1d288-75d4-4943-b01e-0665a879b684`. The pre-change full library
`vrun-9769aba3-d23b-4b18-9060-465d5366f903` fails exactly the five new tests:
argument extra work is 73 at both widths; required/readonly/optional tuple
extra work is respectively 3/2/37 at both widths. The variadic pair instead
has decreasing extra work (-54/-1254), so its competing generic costs still
need to be equalized. All 204 earlier tests pass, with one existing ignore.
The exact inputs are preserved in `tuple-edge-argument-red-inputs`.

`TupleTarget` now retains the same request Control and owns checked
`getStartElementCount`/`getEndElementCount` traversal. Its existing total-fixed
count and inference/indexed-access callers inherit this accounting without
additional caller debits. Relation's separate prefix scan now uses that owner.
`typeArgumentsRelatedTo` guards each invocation and charges every visited pair
before reading variance or skipping an independent parameter.

The revised variadic witness uses T[] for its rest element, retaining a generic
target on both sides. Its 32 native controls pass at
`vrun-a7eb0ea8-b45f-47d4-9322-1fb6d79abe2e`; formatting passes at
`vrun-050de6c3-836f-4d3d-86b5-c8f19a66db35`. The first post-change library run
is cancelled (`vrun-1beaf643-f4fb-42d2-8e84-ad1b7a9a7a79`): its variadic
assertion still fails, and its 256-parameter variance warmup makes the argument
boundary test excessively slow. No GREEN is claimed. The argument fixture's
wide case is now 64; the same 2(w - 1) native skipped-argument difference passes
at `vrun-8d0bde98-0c54-4732-9c70-4a9491d91dad`. The original 256 inputs/captures
remain archived. The bounded retry is running.

A separate first-rest witness equalizes suffix scans and isolates prefix
visits for mutable/readonly targets. Its 16 cold native controls pass at
`vrun-e01bd442-bf94-42ab-b5be-ddb16919c1cb`; final-query resource coverage for
these prefix cases has not yet been installed.


The bounded library retry completes at
`vrun-3f2a1419-6d6c-4aff-8ed5-0a12bbeab6a0`: all 208 other tests pass, including
independent argument scanning, while only the variadic discriminator fails
(extra work -53/-773). Its REST-only target follows ordinary reference
normalization; its VARIADIC target follows tuple normalization. Making the rest
element generic did not equalize those paths. The final witness gives both
targets a variadic element, with one additionally ending in REST. Both now take
the same normalization path. The wide tuple cases are reduced to 64 for bounded
CI runtime; the 256 inputs/captures remain archived.

Both final tuple oracles pass 48 cold controls at
`vrun-59bffe4b-39b7-4d17-b36e-f148f7b1b85d`. The two prefix resource regressions
are installed, and the full library passes at
`vrun-361c4073-c897-4187-b59a-f930bd63d61c`: all seven new tests pass their work
cutoffs, exact boundaries, flat-depth checks and healthy successors. A separate
tuple-work omission is running, retaining polling and argument accounting.

The normalization investigation also identifies unchecked generic-tuple
normalization traversal and scratch allocation. A native fixture is prepared
that warms tuple element access and then rejects an empty source before tuple
element comparison. That separate operation is not yet changed or proven.


The separate normalization native control passes all eight cold queries at
`vrun-c71bf87b-6971-481c-9427-53c8f4f0638c`: both queries warm first-element
access, consume identical native instantiation/reference-cache work and return
number without recovery. Empty-source rejection visits no tuple elements.
Actual normalization callbacks are four for the short tuple and 2w for the
wide tuple, at widths 16/64. No production normalization change or resource
regression is claimed yet.


## Current tuple and argument accounting result

The shared tuple-edge debit is isolated by the required, optional and readonly
suffix cases. Removing only that debit (keeping deadline polling) fails those
three tests at `vrun-05a9adff-a83b-4b0d-9507-f0d2c0a2b65a`. The prefix and
variadic cases still pass the omission; they establish native compatibility,
exact resource boundaries and flat depth, but do not isolate this debit.
Adding common `.length` warmups preserves all 48 native controls at
`vrun-38f83f71-b5e5-4b24-a79d-cb2ea4e6bf7e` without changing that limitation.

A second omission preserves invocation guards and polling while removing tuple
edge and type-argument iteration work. Library
`vrun-8f5d8b7e-057b-41a4-b4be-200df2d2fd81` fails six tests: the three suffix
cases, independent arguments (73 extra units at both widths), and the then-new
normalization work/payload cases (24/24 units and 0/0 bytes). All other 207
active tests pass. The runner restores both source files before implementation.

`getNormalizedType` now guards its invocation and loop, charges each actual
`getNormalizedTupleType` element, and reserves normalization scratch before
allocation. Identity changes are recorded during the native ordered map,
avoiding a second unchecked equality scan. Deferred-reference argument copies
use the checked scalar-copy owner. `getNormalizedUnionOrIntersectionType`
retains its native ordered short circuits and charges each predicate visit,
recursive normalization and scratch reservation.

Installed-source full-library GREEN is
`vrun-173d7c0e-908f-4a16-bf34-72f3ebe16fc2`: 213 active tests pass, with the
existing ignored test. Formatting reported one layout hunk, now applied.
Independent normalization work-only and payload-only omission runs are pending.
These increments do not close tuple factory/key storage, completion/error
copies, other evaluator families or production calibration.


Normalization work-only omission is confirmed at
`vrun-31dcd5c6-2ed9-4bce-9ad4-e6d81ed7e12d`: only the work discriminator fails
(24 extra units at both widths); 212 other active tests pass. The entry guard,
checked reservation and polling remain installed in that experiment.

## Reachability scheduling and storage follow-up

`isReachableFlowNodeWorker` already uses explicit continuations but its local
stack, invocation records, reduction history and Shared-result map are outside
the request ledger. `getBranchLabelAntecedents` also scans active reductions
without charging. The prepared change uses the existing checked WorkStack and
allocation operations, retaining native cache order, last-node invalidation and
causal sequencing. Reads and reachability keep their separate native questions
and share only the existing branch-antecedent owner.

The native fixture reads a number immediately before a predecessor chain, then
assigns and reads it again afterward. Both requests perform exactly one ordinary
read-flow invocation and visit. Reachability alone visits the chain. The bounded
16/64 controls pass all 32 strict/noCheck queries, with number results, no acquired
recoveries and no diagnostics, at `vrun-810e1032-08ba-4a52-bb54-185f1dc5925c`.

| Chain | Heavy reachability entries | Heavy visits | Shared records |
| --- | ---: | ---: | ---: |
| Empty conditional branches | 3w + 1 | 4w + 5 | 2w |
| Unrelated assignments | 1 | w + 5 | 0 |
| Loops with break | 2w + 1 | 4w + 5 | w |
| Finally reductions | 3w + 1 | 4w + 5 | w |

Every cheap twin has one entry, three visits and no Shared records; neither
side hits the Shared cache. These are observed visits, not extra semantic
queries. The first 16/256 capture passes its traversal/final-result checks at
`vrun-2778f7c9-7669-4696-a55b-a55bd23a40fd` and remains archived. A strengthened
no-diagnostics assertion exposes duplicate global names across the two fixture
files (`vrun-f11dacd1-bbcd-48eb-a0ab-9f5d4314c964`); the final fixture makes
those names distinct. Resource tests and the production change remain pending.


Normalization payload-only omission fails only its new test at
`vrun-1c739320-af93-479b-8e46-393c7bb4b27e` (zero extra bytes at both widths),
while 212 other active tests pass. Work, polling, fallible reservation and the
capacity policy remain intact. Both experiments restore the exact source hash
`4e5a0fe9b7b7439b2e83d9ae8f66ce3c593d2f419edd57420ab4b96130d1f787`.
Existing flow and normalization native controls also pass at
`vrun-74c951cc-caea-4f8d-b313-c8ed2f708ac7` (three executed tests, zero skips).


The reachability pre-change library result is
`vrun-de9e9b25-5edb-40e9-ad5b-21d480895ef6`: all seven new resource tests fail
and all 213 prior active tests pass. Heavy-minus-cheap work remains -1 at both
widths in all four families; retained payload remains -168 at both widths in
the three growing-storage families. Every measured query itself returns the
native number result before the resource-growth assertion fails.

The prepared implementation is installed. WorkStack owns continuation capacity
and pop work; Invocations owns checked frame growth and failure-propagation
steps. Reachability checks reduction-vector and Shared-table growth before
mutation. The shared `getBranchLabelAntecedents` charges its actual ordered scan
for both flow clients. An invocation depth guard covers checker callbacks that
can reenter reachability; the graph traversal itself stays iterative and flat.
The full library plus 13 flow/return/tuple/relation compatibility roots is running.


Installed reachability accounting passes the full library (220 active tests,
one existing ignore) and all 13 selected flow, return, tuple, inference,
indexed-access and relation integration roots at
`vrun-8de3c0a0-0d38-4237-8016-0fa92b4beba0`, with zero platform skips.
Formatting passes at `vrun-8c9848b8-912f-4f46-9d4e-f2a2f18dfdc7`.

All-target Clippy then identifies an earlier tuple ownership mismatch at
`vrun-6c8714c7-afbc-4e7b-a875-d33f8e920c16`: TupleTarget now contains the
request's Rc control, so Arc is inappropriate for its shared metadata. Target
and reference views now retain Rc<TupleTarget>; query-local sharing and native
identity are unchanged. The Rc correction has separate tuple/identity tests
and Clippy underway. The preceding 14-root pass predates that correction.

The scoped design still fits the single semantic worker: the existing ledger
and explicit continuations need no new scheduler, locks or cancellation protocol.
Checked capacity belongs to storage owners, and traversal order remains with
reachability. Unchecked retained recovery copies and the rest of evaluator
accounting remain outside this increment; production budgets and final
release/service proof are still open.


## Literal-cache hashing follow-up (native evidence, implementation pending)

Regular and enum literal caches still hash variable-length canonical values
through ordinary maps. The next witness warms both values in an eager comma
initializer, then repeats eight short or long literals before returning number.
ASCII, Unicode, lone-surrogate and no-substitution-template cases cover the
exact retained byte values. All 32 strict/noCheck native queries pass at
`vrun-39f85197-e04d-45d8-b48e-6a76a03f286a`; strengthened exact counters pass at
`vrun-3ff67e37-8714-48c1-ab7c-722083a1b9fc`. Each query has eight cache hits;
the heavy-minus-cheap key bytes are 8(w * b - 1), where b is 1 for ASCII/template,
2 for Sigma and 3 for a lone surrogate. All roots are number with no acquired
recovery. The ten TS2695 diagnostics are the native eager comma operands.

Four final-query work regressions are staged, awaiting the prior tuple/Clippy
checks before installation. A draft extracts RelationTable's existing checked
collision/capacity mechanics for reuse by literal caches; it preserves exact
key equality as the decisive question and keeps byte fingerprinting with the
canonical string owner. No literal-cache production change or resource RED is
claimed yet. Source literal materialization, template-cache keys and retained
failure copies remain separate gaps.


The tuple Rc correction passes all four tuple/identity roots at
`vrun-bc4922e5-c766-4db8-8baf-39f132663ccc`; all-target Clippy passes at
`vrun-824cf32a-86e4-4619-ba91-5675aacb3f05`. The staged literal-cache work tests
are now installed and their pre-change library check is running.


Literal-cache RED is `vrun-fde7d819-6fd2-4c31-8340-4fef158dfd65`: all four new
work tests fail with zero extra work at both widths, and all 220 earlier active
tests pass. The installed change extracts the existing relation-table mechanics
into `resources::CheckedMap`. Relation keys retain their previous fingerprint
and equality; regular/enum literal keys add bounded byte fingerprinting and use
the existing canonical string equality with the same request control. Domain
keys retain primitive category and actual enum identity. Standard binding hashes
are unchanged; no opaque identity acquires a spelling.

The new four resource tests have passed within the running library scope.
Complete library/integration, forced-collision and Clippy results remain pending.
Formatting passes at `vrun-88ec55b7-6bd6-4e01-9b69-e7761ef73e4f`.


The complete literal-cache scope passes at
`vrun-c72d3959-ff10-4348-845f-fa0329bfce9b`: 224 active library tests (one
existing ignored test) and all eleven selected integration roots. Forced-zero
fingerprints for both literal and relation keys pass all five collision roots
at `vrun-801d7649-2f35-4c31-b93e-884a5cfb872d`. The runner restored both
production files byte-for-byte. These checks establish exact cache identity
under deliberate collisions, including enum and generated literal identities;
final all-target Clippy remains due after the next source increment.

## Template-cache accounting follow-up

The next paired witness uses empty text spans with 16 or 64 placeholders.
Number placeholders retain a template cache key; string placeholders collapse
to the string intrinsic before cache lookup. Both requested comma initializers
finish as number. This separates retained key vectors from text-byte copying.
Pinned cold strict/noCheck controls and pre-change resource tests are running.
The prepared change reuses the checked table's capacity/collision owner,
retains exact text and actual placeholder identities, and checks the two
Vec-to-Arc payload conversions inside the existing template factory. No
passing template resource or native result is claimed yet.


Template-cache RED is `vrun-3c54b8a4-8927-43ce-9bff-3665d8bea6a5`:
224 earlier tests pass, and only the new cache work/payload tests fail.
Their heavy-minus-cheap differences remain one work unit and zero payload
bytes at both widths. The checked key/factory change makes both pass in
`vrun-311e96ed-d2bc-4423-be41-8d9904b8554a` (226 passing active tests).
That scope deliberately introduces two independent span-traversal regressions;
only those fail, with work differences -39/-39 and payload differences
-1392/-1392. These differences include the fixed early-widening overhead,
not a claim of negative allocation. The factory formatting check passes at
`vrun-4547838e-7b80-40e8-aec7-de32ac24ea2a`.

The span controls visit one placeholder before an invalid `unknown` widens to
string, or all 16/64 valid string placeholders before collapsing to string.
Neither creates an additional template-cache entry. All sixteen cold native
queries pass at `vrun-ff740360-f2d4-4f0e-b6b2-98122d9b024c`. The earlier
`vrun-9720c71f-ee6e-4f21-9a2f-22095f3d82a1` correctly rejected an overly strict
no-diagnostics assertion: the invalid first placeholder emits TS2322 without
acquired errorType. The corrected assertion retains that diagnostic and all
number results. The original eight cache controls passed at
`vrun-6704e921-13f2-482d-ac57-5ac1175e44d6` and, with exact empty diagnostic
assertions, `vrun-51ebc3d6-7340-4ecf-a461-a8f11e8de9bb`.

The installed template evaluator now uses the existing checked work stack,
checks recursive invocations and actual predicate scans, reserves scratch and
node-cache capacity, and checks copied argument/text vectors. It preserves
native distribution, early widening, ordered acquisition and normalization.
Formatting passes at `vrun-7545293c-ac04-4b52-a969-2b13179dfeee`; its library
check is running, with new source-projection regressions intended to remain RED.

Source projection is tested by a missing *first* placeholder. The final query
must retain InvalidSemantic/UnresolvedSymbol at that exact node, so a test
cannot count a later normalization/cache operation as projection work. Native
controls pass at `vrun-dc2a788a-f9ba-442a-8c9d-fb0f31f79646` and, after adding a
healthy successor, `vrun-fd4300d7-47e9-43cd-82b1-b6658c9bf2f1`. Expanded controls
pass all forty queries at `vrun-252062fa-76b8-4a68-8e33-238c2ad4f302`: span-list
width plus ASCII, Unicode, lone-surrogate and supplementary cooked-byte width,
in strict and noCheck mode. Exact UTF-8 acquisition ranges identify Missing in
every case. Source projection production accounting is prepared, not yet
installed; its independent RED checks precede that change.


`vrun-f4a330b6-6360-4e33-a9e5-e5d3d86966a4` passes all 228 earlier active
tests, including both new traversal tests. Its two source tests fail before
the resource discriminator: the test expected macOS's /var temporary spelling
instead of the Project's canonical /private/var path. This is a harness failure,
not resource RED. The already-started expanded run
`vrun-afc1d0f9-eaa1-4b98-b0b6-ba9bae969530` was cancelled rather than repeating
that known mismatch. The harness now checks the canonical path, exact missing
node and absence of extra causes. Its corrected ten-case work/payload run is
pending; source production accounting remains unapplied.


The corrected missing-placeholder tests all pass without source changes at
`vrun-3c220fd8-74c8-4328-beca-30b99f2259dd` (238 active library tests, one
existing ignored test). They do **not** isolate source projection. The first
Missing produces a recovered native value; `NativeSequence` correctly retains
that evidence while continuing through later placeholders and normalization.
The earlier premise that the first missing name stops the evaluator was wrong.
These ten tests are retained and renamed as template **recovery** resource
checks. Their exact final failure and successor checks remain useful, but no
source-accounting RED is claimed from them. No failure behavior was changed.

A replacement witness uses const-enum constant-template evaluation. Its first
call expression is not a constant; native evaluation should stop there without
checking later substitution types. The source adapter has already projected
all expressions/texts before that first constant question. Separate 16/64-span
and four cooked-byte families are prepared, with a zero-demand native counter
for actual constant-template visits and ordinary template-expression checks.
Those controls are running. Source production accounting is still unapplied.


The replacement constant-template controls pass all forty queries at
`vrun-d094687d-4000-4712-af0c-b25e378f3475`, and again with enum names that do
not overlap the selected result markers at
`vrun-784cf571-c772-4e90-9faa-2b8203fa5287`. Every query visits exactly one
constant-template substitution, performs zero ordinary template-expression
checks and adds no template-cache lookup beyond startup's numeric-string
intrinsic. Each result is number, with no errorType or acquired failure;
TS2474 is the diagnostic-only const-enum rule. The initial capture is preserved
under `.cc/temp/completion-next/template-projection-initial-native/`.
Ten final-query regressions now test list width and the four cooked-byte
representations, retaining exact resource boundaries, flat checked depth and
healthy subsequent semantic/lexical queries. Their pre-change library run and
four template compatibility roots are pending. The earlier missing-placeholder
tests remain explicitly whole-query recovery checks.


The four template compatibility roots pass at
`vrun-91c3c3be-7e94-4136-81ee-0b4dfb30a482`; the final pre-change test formatting
passes at `vrun-9c82e837-1e0a-4748-ab21-032222447132`.

The replacement source-projection RED is
`vrun-a0649871-8cc7-4390-a0c0-dc40b7cd1a29`: all ten new tests fail specifically
at the resource-growth assertion, with zero extra work and zero extra payload
at both widths in every family. All 238 earlier active tests pass (one existing
ignored test). The selected roots themselves are Found number with no issues
before the resource discriminator, matching the pinned constant-evaluator
controls. The source accounting change is now installed: both template recipes
reserve their expression/type/child/text buffers, share checked cooked-byte
copying, and charge actual span and context-parent traversal. Interruptions
retain Source's existing Result transport and the same request Control; no
semantic recovery or source decoding rule changes.

The full library plus thirteen affected integration roots are running. A
subsequent forced-zero template/literal/relation fingerprint check will verify
exact identities, restore all three files byte-for-byte and then run all-target
Clippy. These results are pending; whole-evaluator coverage, budget calibration
and final release/service proof remain open.

The expanded template library check has passed all 248 active tests (one
existing ignored test), including all ten previously RED source-projection
checks. Its thirteen integration roots are still running; the terminal receipt
and collision check are pending.

The next cache investigation concerns `createTypeReferenceEx` / `getTypeListKey`.
The current reference table copies argument lists before lookup and delegates
variable-length hashing/comparison to an ordinary HashMap. Sixteen pinned
controls pass at `vrun-d1c22d34-922c-4caa-ac16-19a5d8c2149b`: 16/64 arguments,
cache hit/miss pairs, strict/noCheck. Both selections acquire the same number
of reference arguments and perform the same number of lookups; the hit adds
exactly one hit with the full argument width. All selected results are number,
without errorType or acquired failure. TS2695 on the discarded seed expression
is diagnostic-only. The first oracle build timed out
(`vrun-f7dcbdce-55e6-4d71-b66c-6080bfe439f0`); the next run exposed the harness's
incorrect empty-diagnostic expectation
(`vrun-9d3c3075-b6fd-43b3-a4c1-007c1add9d4e`). Neither is a resource regression.
A checked reference-key implementation and separate hit-work/miss-payload
regressions are prepared but not yet installed.

The complete template-family scope passes at
`vrun-09b682fd-a70c-4146-9f96-9cda1c32bb17`: 248 active library tests, one
existing ignored test, all thirteen selected integration roots and zero
platform skips. Formatting passes at
`vrun-0f423217-cb47-495a-a816-209cb6f0a9b1`. The three-family forced-collision
check is running. All-target Clippy will be deferred until the prepared
reference-cache change is installed, avoiding a redundant compile of the
superseded source; its eventual receipt remains required.

The forced-zero template/literal/relation fingerprint check passes all seven
selected identity roots at `vrun-1b583254-284b-4449-8b9a-2dd0861f9cc2`, with no
platform skips. The three original files were restored byte-for-byte against
the saved manifest. The subsequently started Clippy scope
`vrun-6a97760f-dd6d-414b-8b0a-5de54d0eaeb6` was cancelled before the next change;
no lint pass is claimed from it.

The reference-cache work witness now compares a warmed generic-alias lookup
with a warmed generic-reference lookup; the miss-payload witness retains the
original hit/miss pair. Borrowing the lookup arguments avoids copying on a hit,
so comparing hit work against a miss would conflate comparison work with the
miss's Arc copy. The additional native control originally used `void` seed
operands, which native checking deliberately does not acquire. Its failed
counter assertion is `vrun-e5161e5c-5ff6-4636-8831-8be51b764693`. The pending Rust
run `vrun-a7877562-3383-4ab9-81e5-0d45c32730c4` was cancelled, with no resource RED
claimed. After using actual seed operands, all 24 native controls pass at
`vrun-3caefe25-3fbb-4be4-a0db-85bd86bd6762`. Exact TS2695 counts preserve the
native diagnostic-only events; all results remain number without acquired
failures. The corrected source and all 24 native observations are saved in
`.cc/temp/completion-next/reference-cache-corrected-red-inputs/`; the earlier
cancelled run's snapshot contains only a partial in-flight native capture and
is not completion evidence. The corrected pre-change Rust run is pending.
Test formatting passes at `vrun-e38e323c-fc5a-4040-b555-23add8baf050`.

The corrected reference-cache RED is
`vrun-6e59db73-30bc-49cf-ac2f-c9807cc2dc04`: all 248 earlier active tests pass,
with one existing ignored test. Only the two new resource discriminators fail.
Lookup work has the same extra 13 units at 16 and 64 arguments; miss payload has
zero extra bytes at both widths. The root queries themselves remain Found
number without issues.

The installed reference-key owner now uses the existing checked table, hashes
actual target plus ordered TypeIds, and performs exact checked comparisons.
The table accepts a borrowed lookup predicate with the same checked fingerprint
and comparison policy, so a cache hit does not allocate an argument copy. A
miss converts the supplied Vec once through checked Arc storage and shares that
storage between the reference and its key. Canonical target insertion uses the
same key owner and checked argument sharing. Cache lookup still precedes input
flag use, and synthetic-this arity remains distinct. Formatting and the
post-change full-library/compatibility scope are pending.

The next source-literal controls pass forty queries at
`vrun-c6cf292a-4d13-44a8-9737-c0d88ac1e192` (and earlier with a local ambient
function at `vrun-eb3999a5-d4e9-49ed-b8c6-6f0209bac92f`). These place an ASCII,
Unicode, surrogate, supplementary or bigint literal in a later template
substitution. Native constant evaluation visits only the first, nonconstant
substitution; ordinary template checking remains uninvoked. The results are
number without acquired failures, with diagnostic-only TS2474. The final
fixtures use an ordinary function body. Source-literal resource tests are
prepared but not yet installed; no resource RED or fix is claimed for them.

The registered Rust wrapper now also accepts the single explicit path
`src/project/semantic/resources_tests.rs`, selecting only that module while
retaining two build jobs and two test threads. Mixed target paths, a missing
file and zero executed tests are failures. Existing full/library/integration
scopes keep their behavior. The wrapper regression RED is
`vrun-28baba36-edb2-49fb-b8ec-059cde60d0ff`; all 26 Rust/harness/semantic-wrapper
checks pass at `vrun-c22e67be-8afd-404e-9c96-83d176a6d38e`. The tested wrapper and
shared helper were installed in the canonical checkout as well as this
worktree, preserving their modes; the sourced common helper need not be
executable. The registry still reserves cost 2 and a 15-minute timeout. Saved
before/after hashes are in
`.cc/temp/completion-next/resource-wrapper-canonical-before/manifest.json`.
The first real resource-only run is still pending; this scope does not replace
full-library or final full-suite evidence.

The reference-cache change passes the full library (250 active tests, one
existing ignored test) and all nine selected integration roots at
`vrun-b3979847-0c0b-472b-93ce-7cd5daf6afae`. Formatting passes at
`vrun-3a2c64ab-d81d-44e4-b8a8-90c668b1b291`. Forced-zero fingerprints for
reference, template, literal and relation keys pass all eight selected identity
roots at `vrun-98d774b2-6fc8-452b-aabe-65a450ba0a3c`. All four source files were
restored byte-for-byte; all-target Clippy is running.

The source-literal matrix now also covers no-substitution templates, normalized
number payloads and paired true/false payloads with equal span counts. All 64
native controls pass at `vrun-18edf889-07c5-4e29-9e7d-27cbaf370992`; the earlier
forty-query inputs and observations remain saved separately. Sixteen resource
regressions and a captured-syntax context change are prepared, not installed.
The context keeps the same request Control available to all literal projection
paths, without putting semantic accounting into the public AST. No depth or
other recipe-buffer coverage is claimed by this prepared increment.

Before adding source-copy charges, the literal-cache witness is being tightened:
both selections project the same long-byte payloads, while only one selection
acquires those later literals into the native cache. Otherwise new source-copy
charges would mask removal of cache charging. The first revised native scope
`vrun-60ac2fd6-4892-4859-aa47-e6801409fee8` timed out during compiler-overlay
compilation before any query ran; its retry is pending.

The revised literal-cache controls pass all 32 queries at
`vrun-63c759ca-6f0c-4158-9b60-b618a588c26d`. Both selections contain and project
the same long-byte literal payloads. The chosen selection performs eight extra
warmed cache lookups; misses are identical, and the exact extra native key
bytes are eight times the cooked width. The cheap selection visits exactly
one constant-template span and does not check ordinary template expressions.
Its TS2474 plus two TS2695 diagnostics, and the chosen selection's ten TS2695
diagnostics, remain diagnostic-only. The prepared resource run will replace
the four old cache fixtures with these before adding the sixteen source-literal
regressions. It waits for the current Clippy scope and has not applied the
source projection change.

The reference-cache all-target Clippy scope passes at
`vrun-61ac901b-0305-461a-91c8-b4e8b92f7093`, covering the earlier literal and
template changes too. The source-literal RED is
`vrun-d2df77e8-d9ef-441f-9070-ddf580bf0e5f`: all sixteen new resource tests
fail with zero extra work/payload at both widths; all 112 earlier resource
tests pass. This is the first real execution of the narrowed resource-module
scope (128 executed, 139 filtered). Its four tightened literal-cache tests
also pass before adding source charges.

The captured-syntax context and shared checked literal-copy owner are now
installed. Root selection and logical-expression traversal needed two additional
context routes after the first compile attempt
(`vrun-dc8e5a00-61de-4405-a626-660916e0acf7`); that failed build is not resource
RED. The corrected resource and twelve integration scopes are pending.

A separate `getUnionKey` audit reproduced incorrect `keyof` summaries: warming
one interface/class/alias key union could make the next equal-key target inherit
its origin; plain unions could also absorb an Index origin in either order.
All eighteen pinned controls pass at
`vrun-9cd6c746-5c28-49ff-9a31-e41cc1dcfea6` across strict, null-loose and noCheck.
The six public-query regressions all fail before the fix at
`vrun-51a6777a-7278-4423-b9b9-c853661359ae`. The union-key owner now matches the
native Index-origin branch: actual origin identity plus current constituent
identities, distinct from plain, union-origin and intersection-origin keys.
The latter list also preserves native in-progress key sets; a separate
end-to-end witness for varying sets on the same resolving origin is not claimed.
The six-test GREEN is pending in the integration scope.

The corrected literal-projection resource run passes all 128 tests at
`vrun-3356e4a3-c92d-49dc-9e23-c9f211d92b19`. The twelve-target compatibility
run `vrun-bf259184-12dc-46ea-9cfc-e0578fb7b808` reached its 900-second runner
limit after ten targets passed, including all six new keyof tests; the saved
live output is `.cc/temp/completion-next/literal-projection-compatibility-live-output.txt`.
The last truthy-operator target had started and the tuple target had not run.
A complete retry with cached builds is pending; the timed-out scope is not a
passing integration gate.

The registered test wrapper now also admits one existing child-module file
under `src/project/semantic/resources_tests/`, selecting exactly that Rust module.
Mixed paths, missing/invalid paths and zero executed tests remain failures.
Two new selector assertions fail before the change at
`vrun-9185bad6-9e5f-4715-8941-68e4195c556f`; that run also has an unrelated
mock-child startup timeout. The initial wrapper attempt
`vrun-aa263516-5d28-49cf-8e03-64ae9e7fbfd6` additionally contains two test-path
editing errors and two startup timeouts; neither is product evidence.
All 29 wrapper checks pass at `vrun-ec30cf7c-d46b-4152-a508-cf04e4540983`.
The mock-Cargo timeout is now 60 seconds after repeated ten-second cold-start
failures; this does not change product deadlines, worker counts or the
registered runner timeout. A mistaken harness filename was separately rejected
before execution. The tested selector and description are installed canonically,
with before/after hashes in `resource-child-wrapper-canonical-before/manifest.json`.
The four literal-cache and sixteen literal-projection tests were moved unchanged
into child modules; the parent scope still includes them all.

All 72 pinned unary/await projection controls pass at
`vrun-85a89334-a491-41c3-bb3d-1af7667ea55b`. Nine operator families, two widths,
strict/noCheck and cheap/chosen selections each return number without acquired
failures. Constant evaluation visits exactly one substitution and never checks
the later unary/await expressions. Eighteen final-query regressions are installed;
the production entry/operand-storage patch is only prepared. Their RED is pending.

The complete twelve-target integration retry passes at
`vrun-acc8ba9f-c9ea-4e01-9b61-6af4ce1adc4e`, with zero platform skips. This
closes the literal-context and keyof-origin scoped compatibility checks.

The eighteen unary/await projection regressions fail specifically with zero
extra work and zero extra payload at both widths in
`vrun-74e461bc-5a64-4429-a0f7-cbfbf8007bb1` (18 executed, 267 filtered).
Their source/native snapshot is `expression-projection-red-inputs/`.

A second source-only witness uses unselected conditional branches. Its first
candidate used an unknown reference directly, but native error recovery retains
an alias and acquires its arguments; `vrun-009de247-9ddd-4ec5-9b55-3ce8163a0644`
rejects that premise. The original inputs and observations are preserved in
`type-projection-rejected-unknown/`; no source-only proof is claimed from them.
After moving the reference into the unselected branch, all 96 pinned queries
pass at `vrun-c2b7cfe8-af35-4a3e-8457-f965b758e52a`. They cover argument lists,
unions/intersections, tuples, keyof/indexed/array/readonly/parenthesized types,
and optional/named/rest tuple forms. Every result is number, without diagnostics
or acquired failures; the native trace acquires no type in the unselected branch.
All 24 final-query regressions fail at the exact resource discriminator with
zero/zero deltas in `vrun-5683d521-02be-4c22-8add-0895363aeb86` (24 executed,
285 filtered). Their snapshot is `type-projection-red-inputs/`.

The shared expression/type projection increment is prepared, not installed.
It uses the same captured-syntax Control for recursive recipe visits, checked
Box payloads and checked collection materialization; tuple and deferred-alias
traversal remain in their existing native owners. It does not yet cover every
Source location/name copy, retained recipe clone or unrelated recipe family.
The literal-cache byte-accounting omission check is running before installation.

The literal-cache omission succeeds as a discriminator at
`vrun-bfd26716-7e9e-4a97-878f-f0a5748179cb`: all four selected tests fail when
only LiteralKey's checked byte hashing/comparison is omitted. Source-copy charges
remain enabled, native cache identity remains exact, and original source bytes
were restored. The manifest records the changed and restored hashes.

The expression/type recipe increment is now installed. All eighteen expression
resource tests pass at `vrun-30e4d5e6-2efa-487d-80a3-898e617ae68c`; all twenty-four
type tests pass at `vrun-e36d9952-82aa-41b8-ab81-b03c0c4bc018`. The full library
passes at `vrun-16638c34-5525-414e-a562-da5555cd3984` (308 active tests, one
existing ignored test). All nine affected integration roots pass on unchanged
source in `vrun-6bd57ba8-c733-4b15-8f88-6699de8d7630`,
`vrun-0aee0fbb-af96-4883-a519-a6436d490b41`, and
`vrun-658c32a4-0e44-4419-a852-bf3564af845e`. The exact scopes/source hashes are
in `recipe-projection-compatibility-report.json` and its input manifest.
Formatting passes at `vrun-2846f1e6-15fe-4660-aff5-5f90888744a0`; the scratch
format-hunk applicator needed an end-of-file context correction before that pass.
A mistaken conditional-test filename was rejected before execution; it is not
compatibility evidence. All-target Clippy is deferred to the next small label
copy correction, rather than compiling the superseded source twice.

The next uncharged source payload is the exact text retained by
`getTupleElementLabel`. Forty-eight native controls pass at
`vrun-27ed351b-9d93-4ff3-bbcf-17f07b3b8082`, covering ASCII, Unicode,
supplementary, escaped ASCII/supplementary and leading-underscore names in
unselected tuple branches. Native acquires none of those branch types; final
answers remain number with no diagnostics or acquired failures. Twelve resource
regressions are installed, with RED pending; the copy-accounting patch is prepared
but not installed. Other name/location copies and retained recipe clones remain
separate accounting obligations. Earlier omission proofs retain their dated
scope; the final sensitivity audit must also check whether newer Source charges
mask any older evaluator-only discriminators.

All twelve label-copy regressions fail with zero/zero work and payload deltas in
`vrun-fd478983-c44e-4cd7-a661-5a9710630bd9`. Both tuple-label callers now use
the shared checked exact-text copy operation. The twelve tests pass at
`vrun-c7c0dc1a-786e-42fc-9b6b-42ffc6c7b7a0`, including exact budget boundaries,
flat depth and subsequent healthy queries. Formatting passes at
`vrun-761f7608-8420-44de-8375-4a809149a534`. The full-library/four-root and
all-target Clippy scopes are running; no result is claimed for them yet.

The next prepared increment covers native intersection traversal and storage,
including cached constituents before reductions, ordered-set scratch, recursive
cross-products, and checked intersection keys. The common ordered identity-list
hash/comparison would also serve reference keys. Production remains unchanged.
The first native-control attempt `vrun-3d3126d8-03e7-40f0-9fb2-a20245b4ca17`
timed out compiling its oracle before any query ran; its retry is pending.
The union-key cache and other evaluator/cache families remain separate gaps.

The first compiled intersection-control run
`vrun-d344f751-3004-4dc1-8ebb-7c31bd397022` records all 132 expected final
number results, diagnostics and clean acquisitions, then fails its comparison
assertion because an omitted zero-hit counter was subtracted as undefined.
The saved inputs and observations are in `intersection-native-first-observations/`.
The corrected assertion uses zero for absent counters; a fourth loose-null/noCheck
profile is added to the next registered run. That 176-query gate is pending.
All-target Clippy `vrun-5365affb-cb03-44e3-88a3-b399f918574d` was cancelled to
reduce contention with the full library run; it is not a passing check.

The label combined scope `vrun-0ca39323-7126-4173-bfd8-b6a0dc77930d` timed
out at 900 seconds. Its saved live output establishes all 320 active library
passes (one existing ignored test) in 788 seconds and the const-tuple suite pass;
the full combined scope is not a passing gate. The four integration roots will
run separately. Clippy and full-library execution will be serialized to avoid
competing during the long resource tests.

Twenty-two intersection resource regressions are now installed across both
null-checking and both noCheck states. They reuse the existing final-answer and
exact-boundary assertions; only fixture profile selection was generalized.
The production accounting patch remains prepared pending their RED and the
expanded pinned control. `intersection-red-inputs/manifest.json` binds the
pre-change source, fixtures and oracle.

The four label integration roots pass separately at
`vrun-c6822b1a-be85-48d7-9d2b-300248109241`. The expanded intersection native
gate passes at `vrun-c2aa7121-8d9f-43be-8676-4ce1488604e9` (176 cold queries,
all four option profiles). The twenty-two resource tests fail before the fix at
`vrun-0432a0e7-4b83-4faa-a73e-b922732404dd`: byte deltas remain zero, and work
deltas remain 13 or 16 at both widths. The native controls confirm full
constituent visitation even for any/never/null/undefined shortcuts, and seven
warm hits after initial unaliased reconstruction in the duplicate/unknown/empty
cases. These tests isolate the factory as a whole, not every individual debit.

The intersection increment is installed. Existing traversal and reduction
owners charge visits, scratch, ordered movement and cross-product preparation;
recursive factory invocations use the same depth control. The intersection
cache uses checked hashing and exact collision comparison, with native
NoConstraintReduction alias partitioning unchanged. Reference keys share the
same ordered-identity hash/comparison owner. Constituent and alias payloads are
charged before allocation. Green tests, collision checks and compatibility
validation are pending. Cache completion/error clones, union and other variable
keys, production calibration and the remaining whole-evaluator gates are open.

The first intersection Green attempt
`vrun-ab39243c-f660-4fdc-a979-bc33d8c0cfa7` failed compilation: the checked
membership call borrowed Model while requesting its mutable undefined intrinsic.
Obtaining that established intrinsic before membership preserves native order
and resolves the borrow. Formatting also exposed repeated identical hunks in
the scratch applicator; it now handles all identical occurrences. The corrected
registered resource and format scopes are running. Neither failed attempt is
resource-regression evidence.

All twenty-two intersection resource tests pass at
`vrun-7ac33190-1902-49c5-a1f1-44ddc502fa4e`, including all four option states,
exact work/payload limits, flat checked depth and healthy subsequent queries.
Formatting passes at `vrun-08cc655c-fd35-4c4b-a247-6988161fe236`.
The five-family collision experiment is running on a saved snapshot; full
library and broader compatibility checks follow only after source restoration.

The five-family collision experiment passes all six integration roots at
`vrun-654b234c-cdd3-4412-a3e4-a1b24f4012dd` and
`vrun-5fa5b35f-b01e-40f9-94ef-cb422bf43022`. The runner restored exact original
Rust bytes and removed its active-mutation marker. Its saved manifest is
`intersection-cache-collisions-manifest.json`. A separate sequential driver is
now validating the restored full library, three boundary roots and all-target
Clippy against one source-hash manifest.

The next name-copy candidate has 48 passing pinned controls at
`vrun-6177d6df-e5ec-4234-92b5-5b943186ea7a`: a call with an unused generic
parameter still returns number for ASCII, Unicode, supplementary, escaped and
underscore names, with strict/noCheck profiles. A twelve-test Rust resource
module and the checked exact-name copy patch are prepared but not installed.
The selected copy is the Source type-parameter recipe's normalized name;
written display names, constraints and other recipe payloads remain distinct
owners. No resource RED is claimed for this candidate yet.

The restored full library passes at
`vrun-a7260e9b-9f35-414f-8eb1-f572acf44874`: all 342 active tests, with the
one existing ignored test unchanged. The sequential boundary and Clippy checks
are still running against `intersection-restored-input-hashes.json`.

## Array spread cache and union boundary follow-up

The restored boundary scope failed at
`vrun-9190bded-6f07-425c-a6dd-b3307a0fc765`: intersection limits and base
constraints pass, while both union-limit tests exceeded the unchanged 30-second
semantic deadline. The sequential driver stopped before Clippy. An isolated
rerun reproduces both failures at `vrun-8b68207b-8002-4371-b854-03e313f1cc79`.
A three-second runtime sample is preserved as
`.cc/temp/completion-next/union-current-sample.txt`. It shows substantial repeated
array-spread scanning inside contextual lookup, alongside other lookup costs.

Pinned `checker.go:getSpreadIndices` caches even the no-spread result. Source
previously rescanned every array for every contextual element. The correction
adds a request-local cache keyed by the actual array root, charges each scan
visit and cache growth, and acquires spread indices after the parent's context
as native does. This does not establish whole-evaluator accounting or prove
that it is the only contributor to the deadline failures.

The expanded native array fixture passes at
`vrun-188dda58-a301-4176-8cca-da40fc1bce9d`: 60 selections across strict, loose
and exact-optional profiles, including first, last, middle, multiple and
empty-tuple spreads. Rust validation is pending.

The first spread-cache validation (`vrun-68bcc8b7-c641-41bb-9aa6-91545d39ca80`)
passes the flat 10,001-element array in both null-check modes and all five
then-compiled array suites, but still fails the subtype estimate at the
30-second deadline. The expanded six-test array suite and both contextual
compatibility roots pass at `vrun-16b9c904-f46d-4f17-9f47-34a282ebba5e`.
The second sample, `union-spread-cache-sample.txt`, shows repeated property
lookup during subtype removal and lexical reference scans in the flat array.

A second correction separates `Symbol.Name` reads from native ordering-header
projection. Binding's name owner now supplies both full-header construction
and exact-key reads; property-key reads preserve the instantiated target's
original name without allocating replacement identity headers. Previously every
key read projected the declaration inventory although `NativeSymbol::key` never
observed its stored declaration result. The correction preserves that separation
and the distinction between binding lookup failures and whole native failures.
Its boundary and generated-key validations are pending.

The exact-name read correction passes the union-limit, generated-literal-ordering
and known-symbol-property roots at `vrun-de542c35-9da2-49cd-aa33-812e5dc2dc63`.
Both union-limit tests now pass with the original 30-second per-request deadline
and original assertions: flat 10,001 elements, estimates just below/at/above the
native subtype threshold, and strict/loose options. No deadline, test worker
count or native limit changed. The pre-change source snapshots and both runtime
samples remain under `.cc/temp/completion-next/`; remaining final compatibility
and resource gates are unchanged.

## Type-parameter recipe name accounting

All twelve pre-change tests fail only the resource-growth assertion at
`vrun-2f353592-1885-40f7-84ea-06959d7cbc08`: each narrow/wide chosen-minus-cheap
work and payload delta is zero. After routing the recipe's normalized `String`
copy through the existing checked `push_text`/`reserve_string` owner, all twelve
pass at `vrun-058d5470-3bb6-47f8-a171-0e76da1acba1`. The tests execute both
noCheck states, exact budget boundaries, flat-depth checks and healthy subsequent
queries. The earlier 48 native controls establish the returned number type for
ASCII, Unicode, supplementary, escaped and underscore spellings.

The prepared draft used shared-text storage, but the existing recipe owns a
String. That draft was corrected before installation to preserve its storage
contract and reuse checked String construction. No new string-copy owner or
intermediate Arc allocation was introduced. Written display names and other
recipe allocations remain separate open accounting work. Formatting passes at
`vrun-5c22fbb1-c487-4f94-9b59-27c662879085`. The full library, six compatibility
roots and all-target Clippy are running sequentially against
`name-and-boundary-input-hashes.json`.

## Next union-cache accounting candidate

The native equal-width hit/miss matrix passes all sixteen selections at
`vrun-f0b51c38-d51a-4828-a463-c927c1e030fb`. Both queries warm the same plain
union and extra numeric literal, then either build a new union or request the
warmed union. The chosen query has exactly one extra hit, one fewer miss, equal
lookup/key-constituent counts and the same clean number result in strict, loose,
noCheck and loose/noCheck profiles. Counters only observe existing factory calls.

A draft routes Model's union cache through CheckedMap and the existing checked
identity-list/alias owners. A draft final-query regression compares width growth
in the hit-minus-miss work gap: constructing a miss propagates all constituent
flags, while a hit must compare its full key. Both source recipes have equal
width. Neither draft is installed yet; no Rust RED or GREEN is claimed. Union,
intersection and Index origins plus alias metadata require compatibility and
collision validation in addition to this plain-constituent work discriminator.

The name/boundary snapshot passes all 354 active library tests (one existing
ignored test) at `vrun-fd33f5bc-ea59-4c77-acbe-e81a229da18a`, and six integration
roots at `vrun-d9946190-4b4a-475e-bc06-be3d220cecbd` and
`vrun-bc97869f-b10d-4a6e-a9ad-dda3ddbdb6eb`. These cover private names, late
symbol tables, written type-parameter names, generic calls/generics and the
expanded array-spread matrix. All-target Clippy is still running.

All-target Clippy passes at `vrun-1edb2d38-ef62-40ee-ac9f-203e6d4df384`.
The sequential driver verifies that all four scopes used the same recorded Rust
source hashes. This closes validation of the name/boundary snapshot, including
the preceding intersection and Source increments; broader delivery gates remain.

The union-cache regression fails before the production change at
`vrun-bc5dab2d-9cae-429e-b5a6-dcc8dc43e34c`: its hit-minus-miss work gap falls
from -4 at 16 constituents to -52 at 64, exactly the 48 additional uncharged
TypeId comparisons. The production patch is now installed: union keys use the
shared checked identity-list/alias fingerprint and equality operations, and
CheckedMap owns fixed-fingerprint rehashing and collision-bucket growth. Origin
selection, actual identities, alias arguments and immutable payload sharing
remain unchanged. Targeted GREEN, omission and collision checks are pending.

The union-cache final-query test passes at
`vrun-e557f02a-79f4-455d-84c4-adc5047f94ee`, including strict/null-loose and
noCheck states, exact work limits, flat depth and healthy successor queries.
Removing only the actual TypeId comparison charges fails the same discriminator
at `vrun-cab5d738-0fce-4704-8b49-2ee67eb99e13` (-3/-51), while keeping Source
projection, hashing, fixed lookup charges and allocation accounting intact.
The omission runner restored the exact original bytes. The six-family forced
fingerprint-collision experiment is now running across nine integration roots.

The six-family collision experiment passes all nine query suites at
`vrun-deda010e-7ad3-4fc4-8847-e38eb7a15524`,
`vrun-6ee91127-a8bd-41e5-9fd2-3ddac161e626` and
`vrun-601f2439-f879-46eb-acf4-7cbab41232b0`. Union, intersection, reference,
template, literal and relation fingerprints were forced to zero while their
hashing charges remained active. The suites cover intersection constraints,
templates, generic calls, keyof origins, tuples, filtered origins, aliases and
union subtype reduction. Exact source restoration is recorded in
`.cc/temp/completion-next/union-cache-collisions-manifest.json`.
Restored-source formatting passes at
`vrun-e765754a-c8da-4240-8601-232e9f4990c6`; the full library, boundary/origin
suites and all-target Clippy are running sequentially against a saved Rust hash
manifest. These pending checks are not counted as passes.

The restored union snapshot passes all 355 active library tests (one existing
ignored test) at `vrun-b17b852b-2a3c-4768-8a4c-524a44ce722b`.

## Indexed-access keys and written names: next isolated controls

The initial indexed-cache discovery fixture lacked its minimal globals and put
both width variants' aliases in the same global scope; its diagnostic assertion
fails at `vrun-8ab8398a-2a15-4e0e-a546-e0f0342eaf3c`. This was a fixture failure,
not a semantic or resource regression. The corrected fixtures scope their
aliases and supply the same minimal native globals as existing controls.

Both corrected native suites pass at
`vrun-c354011a-5482-4c74-80b8-4dbc4789f53d` (16 indexed-access selections and
32 written-name selections). The strengthened indexed observations pass at
`vrun-daf26acc-a229-4a13-ab4c-431ec51120cf`: the chosen query performs exactly
one extra deferred indexed-access miss, hashing N + 1 extra alias arguments,
while native instantiation counts and type-reference cache outcomes stay equal.
Both final roots are number, without acquired failures, in all four option
states. This is a miss-key witness, not yet hit-comparison sensitivity.

The written-name pairs have identical decoded parameter names, but the chosen
name uses source escapes. ASCII, BMP, supplementary and underscore names all
return number without acquired failures in both noCheck states. This separates
DeclarationNameToString's raw spelling copy from the already-accounted normalized
parameter-name copy. Production patches and final-query RED tests are prepared;
they remain uninstalled while the union snapshot's remaining validation runs.

The union snapshot's final checks pass on identical recorded Rust sources:
`vrun-cd2f57ce-73e5-4f5e-a87d-5f7d3b8e0bc4` covers union/intersection limits
and base constraints, `vrun-2198ea2f-3f7d-488e-b9f4-7bf4858ec7bf` covers
filtered/keyof origins and native generic aliases, and
`vrun-a4a2c378-b284-4d76-9909-82361e665b1e` is all-target Clippy. Together
with the full library, formatting and six-family collision checks above, this
closes the union-cache increment's stated validation. Whole-evaluator coverage,
production calibration and final release/service replays remain open.

The indexed-access/written-name oracle compatibility scope also passes all
12 tests at `vrun-0f510951-d428-4260-a3e4-1e876eb456fc`, including the oracle
review harness. The new resource tests are now installed and their pre-change
runs are in progress; neither production draft has been applied yet.

All eight written-name regressions fail before the patch at
`vrun-cb094b35-db49-4347-af5e-65d6cc070a8b`, with zero narrow/wide incremental
work and payload in each case. The single DeclarationNameToString owner now
accepts the existing capture's Control, uses the existing controlled token-span
operation and checked String construction, and applies the same accounting to
the native missing-name marker. All five callers pass their capture's ledger;
there is no separate Source spelling rule or caller-side precharge.
All eight tests pass at `vrun-6d6a5e27-6770-406a-8c15-fe6637d785a4`, covering
both noCheck states, exact under/at/over work and memory limits, flat depth and
healthy successor queries. Written-symbol, indexed-access and native-alias
compatibility suites pass at `vrun-3d555996-9496-4c8e-a6f4-c40eadd33093`.
The six formatting hunks reported by `vrun-eab782c4-57fa-4ab3-be3d-89bd726ec96a`
were applied; the final formatting check is pending.

The first indexed-access resource test passes before its proposed fix at
`vrun-07ebe0e6-983d-4a22-8cfa-8e0b45efec1b`. Its native counter observations
remain valid, but its resource-growth assertion does not isolate cache hashing
from other argument-dependent work. It is not counted as a RED regression.
The original fixture, test, native capture and hashes are preserved under
`.cc/temp/completion-next/indexed-cache-first-witness/`. The revised pair uses
an intersection alias, whose cache already charges argument keys, instead of an
erased alias. Its native and pre-change Rust checks are in progress. Indexed
production remains unchanged until that discriminator is established.

The anonymous-intersection comparison fails before indexed key accounting at
`vrun-c1dbe57d-e93f-4e75-8c3f-92a0fbf41466` (-46/-142 work gaps), but still
fails after the patch at `vrun-b86732ce-8fb2-4557-899c-0034d5009d26`
(-22/-70). Its object component captures the alias's generic context, adding
another width-dependent operation to only that side. No assertion threshold
was relaxed. The fixture and captures are preserved in
`.cc/temp/completion-next/indexed-cache-anonymous-pair/`.

Replacing that anonymous component with a global nongeneric interface preserves
the native cache observations and removes the unrelated width-dependent work.
Native controls pass at `vrun-d5605ad3-d5b3-4ae0-98ee-60f0c054c018`. With the
original indexed cache restored, the revised final-query regression fails at
`vrun-edd0e76f-8a81-4add-835e-e392f0b72657`: -27 at 16 arguments versus -75
at 64, exactly the 48 missing argument visits. The indexed patch is reapplied:
getIndexedAccessKey uses the shared checked identity/alias key builder and
CheckedMap; complete incoming flags, actual operands and alias arguments stay
in the key while only Persistent flags survive on the operator. Formatting's
four reported hunks were applied. The revised GREEN is pending.

The final indexed-access regression passes at
`vrun-23cb6081-647c-4121-b771-f0e5c4c517c4` in all four option states,
including exact work boundaries, flat depth and successor queries. Omitting
only indexed-key hashing charges fails at
`vrun-32207fa6-89a5-4a2b-b88d-1df6d4cc01ee` (-25/-73), with Source,
intersection, equality and allocation accounting still enabled. Exact original
bytes were restored. Seven-family forced collisions then pass six identity
suites at `vrun-8ea39eb9-cde3-4c85-b96e-4a88d08e21fe` and
`vrun-46574902-bdb1-4fdf-94a5-d6640b985f9e`; the collision runner also restores
all input hashes. Restored formatting passes at
`vrun-7054fd76-9c20-486b-883a-4478925e5eca`. Full library, six compatibility
roots and Clippy are running sequentially against one recorded Rust snapshot.
The expanded native scope passes at
`vrun-0f136181-2696-4dd6-9cf6-51aa81d329db` (16 indexed-access and 176
intersection selections); the new counter observes alias arguments without
changing the earlier intersection expectations.

The next unresolved-alias cache discovery initially fails its filesystem guard
at `vrun-d5df8fe1-350a-4c96-bb60-66cc0f712a50`: adding the healthy-successor
selection overlapped compiler capture. That run is invalid evidence, not a
semantic failure. The now-frozen fixture is being replayed. No unresolved-alias
production changes or Rust tests are installed yet.

The frozen unresolved-alias native control passes at
`vrun-33f2fdf5-2a37-400a-b155-5dbc9c968512`; its strengthened exact-diagnostic
check passes at `vrun-1186fd7e-0a2b-4f78-8025-f400d5bfb7fc`. All sixteen
selections preserve diagnostic order 2304, 2695, 2304 and the same final native
number spelling. Equal-width keys produce exactly one extra errorTypes hit,
one fewer miss and N additional hit arguments for the chosen query. This is
recovery, not a clean public number result: the Rust regression must retain
both unresolved-name sites before applying product resource limits.
Its final-query test and a checked errorTypes-key patch are prepared but remain
uninstalled while the indexed/written snapshot finishes validation. The draft
reuses the existing alias hashing/equality owners and CheckedMap and charges
the fixed intrinsic-alias table/header publication. Variable alias construction,
unresolved-name paths and failure payloads remain separate open owners.


The restored indexed/written snapshot now passes all 364 active library tests
at `vrun-1b80a65a-9a75-447a-b297-06d075099bad`, six compatibility roots at
`vrun-dcedc153-3d28-418e-a1ce-c277abbcb52e` and
`vrun-b52b2a74-8319-4d9f-b8dc-ab6d74e16e71`, and all-target Clippy at
`vrun-6cd5a4c6-335c-4117-88b9-9f35c6aa0ead`. The driver's recorded input
hashes stayed unchanged throughout those checks.

The first error-alias Rust run, `vrun-a136d8f8-e21d-4eeb-b7d3-33754189fcbd`,
fails its recovery assertion before reaching resource growth. The fixture's cast
has two annotation acquisitions, not one: native `checkAssertion` calls
`checkSourceElement` (through `checkTypeReferenceNode` and
`checkTypeReferenceOrImport`, which gets its type), then `getTypeFromTypeNode`
again. Both acquired recoveries retain the cast's location. The corrected test
requires both ordered causes rather than deduplicating them. This first run is
not resource RED evidence.

A normalized-name isolation fixture now passes 48 native result selections at
`vrun-01dc9ace-ab03-4a08-b905-ffd0bc2b81de`, and 72 selections including
exact written-name displays at `vrun-5d828919-0c02-4ef9-9928-d5e034a41368`.
Computed member names keep written-name growth equal on both sides while only
the chosen normalized parameter name grows. This prepares an omission audit:
the older fixture grows both names and may no longer distinguish normalized
copy accounting after the written-name increment. No omission verdict is yet
claimed for the revised fixture.


The corrected error-alias regression reaches its resource assertion and fails
at `vrun-d204149f-4128-42fd-8ed3-4a825a94cb00`: the chosen-minus-cheap work
gap remains 6 for both 16 and 64 arguments. The cache now uses CheckedMap and
the existing checked alias-key hashing/equality owner. Its miss reserves the
intrinsic-alias table and fixed alias header before publication. All four option
states pass at `vrun-b4874e35-0c02-42f4-8e99-ae045be2cefd`, preserving the
primary failure, both ordered cast causes, exact work boundaries, flat depth and
healthy successor. Omitting only the error-alias comparison charges fails at
`vrun-863c9aaa-8555-43a2-87b3-ab901493b6b3` (5/5 gaps); exact source bytes
are restored. General alias construction and error payload accounting remain
open. Collision and restored compatibility checks for this increment follow.

The normalized-name audit confirms masking: all twelve older tests still pass
with only normalized-name copy charges omitted at
`vrun-f6cbe81c-7242-443c-a723-f6c7dcd5b62b`. That run invalidates their use as
isolated omission evidence on the expanded implementation, not the native
spelling controls. The revised fixture's intact/omitted pair is in progress.


The initial revised name fixture run `vrun-18496070-09d7-4911-b92e-1e5a9f80baa7`
selects a later interface reference because its type name contains the query's
marker. It fails result spelling before resource assertions; no omission claim
is based on it. Renaming the interface identities so markers identify only the
intended variables preserves all 72 native selections at
`vrun-f9fa96b4-9ec3-4576-ad39-a77e8d6ecdeb`.
The revised twelve Rust tests pass intact at
`vrun-43c6c823-6a7c-49ba-b99c-c0c3331ecc5c`. Omitting only the normalized-name
copy charges then fails exactly those twelve growth assertions at
`vrun-de9c827d-f456-462b-b387-e23eb5c4993f`: work remains -3/-3 and payload
remains a fixed -4, -5 or -7 depending on encoded width. Exact Source bytes are
restored; the stronger fixture replaces the older tests' inputs. Both noCheck
states, exact under/at/over budgets, flat depth and successors remain in the
intact tests. The earlier marker variant and hashes are archived separately.

The next raw type-alias instantiation control passes sixteen native selections
at `vrun-60dc907c-b604-4af1-9d6a-1d1d80620d8b`. Both requests warm the same
local alias and supply equal-width argument vectors; only the chosen request
repeats that key, adding one cache hit and N hit arguments. Both final native
results are number with only diagnostic 2695. The Rust pre-change discriminator
is running; no ordinary alias-cache production patch is installed yet.


The ordinary alias-cache regression fails before the patch at
`vrun-8eedd841-c857-4fd1-b0ae-b86212269dda` (6/6 work gaps). The cache now
uses the shared identity/alias key builder, checked exact comparisons, checked
argument copies and CheckedMap capacity. Its actual symbol, raw arguments and
enclosing alias remain the key; defaults are still filled only after lookup.
Declaration seeding consumes its completed parameter tuple after creating the
key instead of cloning the whole tuple twice merely to retain its type result.
All four option states and exact work boundaries pass at
`vrun-4786a47c-5cab-4f59-96ef-631863b5ac3d`. Omitting only argument-comparison
charges fails at `vrun-72cb0376-ac5d-4d4a-a3ec-c550163600d2` (7/7 gaps), with
exact restoration. This does not cover parameter-cache or failure-payload copies.

Forced-zero fingerprints across nine key families preserve unresolved alias,
native generic alias and alias-context suites at
`vrun-5f56240a-9db7-49e3-bf40-9e774c222086`, and alias diagnostics at
`vrun-630f1b30-daf3-4ac2-baab-de74ed3bf70d`. All original Rust hashes are
restored. The ten whitespace hunks from
`vrun-7bce3fbf-19d9-4203-8d79-85b44ccb6ead` are applied afterward; final
formatting and combined library verification remain pending. An earlier attempt
to read that receipt before it was written made no source changes.

The anonymous-object control passes sixteen native selections at
`vrun-12785733-318f-4d15-b359-638bce41addc`. Both sides warm omitted defaults
and then supply a full raw argument vector, giving equal alias-cache misses.
Only the chosen effective vector repeats the defaults and hits the object
instantiation cache. Native retains all outer parameters for this generic alias
body even though the object has no members. Both final results remain number.
Its Rust pre-change discriminator is running; the object-cache production draft
has not yet been applied.


The anonymous-cache regression fails before the patch at
`vrun-20462fea-7381-4a75-8655-85d27b34974d` (-11/-59 work gaps).
`getTypeInstantiationKey` now uses the shared checked key builder, exact ordered
argument/alias comparisons, checked key copies and CheckedMap. Actual target,
effective arguments, alias and SingleSignature state remain distinct. The
existing target seed remains non-overwriting. All four option states and exact
work boundaries pass at `vrun-bf9ca7b9-1373-4077-9037-15c0505e46fc`.
Omitting only argument-comparison charges fails at
`vrun-c52df9d5-ac89-4a44-858d-62bd49cf5f1d` (7/7 gaps), with exact source
restoration. Mapper payloads and other anonymous-factory work remain open.

The conditional-root native control passes 32 selections at
`vrun-9c9e0c9f-4a98-48de-9002-adcec138028f`. Distributive and tuple-wrapped
checks both preserve equal raw alias misses while only the chosen effective
argument vector hits the conditional cache. Changing the final unused argument
keeps the native branch result unchanged; the final public selection is number.
The conditional Rust RED is running; its production draft is not yet applied.


Both conditional regressions fail before the patch at
`vrun-c354bdd5-dd6c-467b-9cbd-3b7678fa796d` (-21/-21 gaps in both
root states). Each conditional root now owns a CheckedMap; its
`getConditionalTypeKey` retains effective arguments, alias and forConstraint,
with shared checked hashing/comparison and checked retained argument copies.
The root-seeding and ordinary-instantiation paths use that same key owner.
Both tests pass all four option states and exact work boundaries at
`vrun-1b2081f5-57f8-449a-b049-04ac706aa99c`. Omitting only argument-comparison
charges fails both at `vrun-8167f6fc-8b80-42d5-be0b-d271ed7fa456`
(-20/-20 gaps); exact source bytes are restored.

The combined snapshot applies the fifteen whitespace hunks reported by
`vrun-4c61a921-5e2d-4afe-8df2-990d0c03023a`, then passes formatting at
`vrun-7489b276-891e-4f81-8cb6-0c74d5ae6253`. Forced-zero fingerprints across
eleven key families preserve anonymous types, instantiation expressions and
native generic aliases at `vrun-00470494-b4c0-4b47-ab9e-d581b7828bfb`, and
conditional types/completions/relations at
`vrun-1ca1d767-c819-4e0f-a1bd-a13dc8fbdbd5`. Every recorded original Rust
hash is restored. The full library, twelve restored compatibility roots,
all-target Clippy and six oracle harness files are queued/running in sequence.

The next signature-cache native control passes sixteen selections at
`vrun-f50e47e2-e55a-4195-91d2-af8f208681b1`. Two explicit generic calls share
the actual signature; the chosen second call repeats all filled arguments while
the cheap one changes the final argument. The chosen path adds exactly one hit
and N compared-key arguments; both final native types are number with only
2695. Its checked-key draft retains signature identity and ordered arguments;
copying the cache key through the common checked copy owner would let the miss
move the original argument vector directly into the factory. The draft and
Rust discriminator remain uninstalled until the combined snapshot completes.


The combined library run `vrun-bdda37c9-0e94-4a9d-be3f-0a0da1e0d6f7`
finishes with 368 passed, one failed and the existing one ignored. The only
failure is the older reference lookup growth test: -5/-53 gaps. Its erased-alias
hit now charges key copying as well as hashing/comparison, while reference hits
borrow their existing arguments. The test's comparison with a different native
factory no longer isolates reference work. The validation driver stops here;
restored compatibility, Clippy and the combined harness scope have not run.
The queued signature RED also stops without installing its test or production.

A scoped Ousterhout/post-implementation review keeps the request ledger and
checked-cache design, but changes the isolation method for this regression.
Two observed costs justify the change: written-name accounting masked all twelve
older normalized-copy tests, and alias accounting broke the unrelated reference
comparison. A revised paired fixture remains a feasible smaller alternative,
but would still depend on future accounting in the comparison factory. That
unapplied draft is preserved separately; no assertion threshold is relaxed on
the existing pair.

The chosen observer is private to library-test builds, request-local, and owned
by Control. It records successful work/byte charges in one selected named scope;
nested scopes restore their parent through RAII. It adds no production budget,
query option, global state, scheduler or semantic behavior. The reference hash,
exact comparison and retained argument conversion each mark their existing
owner, with no caller-side charge or observation plumbing.

The final-query test measures total usage T and that owner's usage O, then runs
without observation at limit T - O + M - 1. M comes from the pinned query's
argument-width obligation: at least one work unit per hashed/compared identity,
or one scalar TypeId payload slot per retained argument. Correct accounting must
produce a final resource failure. Omitting the selected charge permits a final
healthy result and therefore fails the test. Exact total under/at/over limits,
flat depth and a distinct healthy successor remain covered. This tests a minimum
resource obligation, not equality with a compiler CPU/allocation counter.

All 56 expanded native selections pass at
`vrun-8eb39d6c-f840-405a-ba35-bde26fce6b6e`; the three observer-based tests
pass at `vrun-15d58944-3e5f-4a85-b481-b89df22f09b4`. Independent hash,
comparison and payload omission trials each fail only the corresponding final-query
test: `vrun-c2075b83-611f-45e1-a2f5-929a7e0a96af`,
`vrun-e3eed0f0-7b5c-401a-9f62-500f389cd8ee`, and
`vrun-7a7bb66b-051c-476c-bc75-35bbb7422870`. Source hashes are restored. Production compilation,
restored library/compatibility checks and final budget calibration remain open.


The next signature increment now has test-only owner scopes in its draft and
uses the same final-query minimum method as references. The tuple-target draft
also retains native ordered flags, readonly state and actual label declarations,
with separate hash, comparison and retained key payload observations. Both are
uninstalled while the restored snapshot runs. This does not cover tuple target
member construction or normalization scratch.

The first tuple native run, `vrun-fc9ea2d7-0223-492b-9213-daf4a0b25cd3`,
exposes duplicate global aliases across the two width files: the wide query can
acquire the narrow declaration. That fixture failure is preserved under
`.cc/temp/completion-next/tuple-cache-global-collision/`. Local function scopes
now separate the aliases without changing production lookup or recovery.
The corrected control covers required/optional/rest/variadic tails, mutable and
readonly targets, and labeled versus unlabeled elements in all four checker
option states. Its capture must establish an actual width-N hit and miss before
these cases can serve as resource obligations.


The corrected tuple native matrix passes all 128 selections at
`vrun-9ce2d1e8-3648-4cf9-8932-3c7e318135ab`: every selection has a width-N
hit and miss, final number and no diagnostics. The harness now also asserts the
observed empty diagnostics explicitly. This control supersedes the failed global
fixture, not the original failed receipt.

The restored observer snapshot passes formatting at
`vrun-20589657-a6de-421b-a27c-d6df5edee00b` and the full library at
`vrun-41c3d848-b549-495e-a626-fee3a3170083`. Its anonymous/instantiation/generic
alias compatibility scope passes at `vrun-6f16f898-698d-426c-8a3d-6ecf01db92a0`;
the conditional types/completion/relations scope passes at
`vrun-0745d066-b7ee-40eb-b746-fff3cc150a2f`. Further combined checks are still
running with Rust sources frozen.


The active-mapper cache audit identifies another variable alias key owned by
`instantiateTypeWithAlias`. A direct generic variable reference supplies no
explicit alias, so its first native witness fails at
`vrun-1ca7a93e-7885-45c4-84de-2b971761d28e`; it is not resource RED.
A global wrapper alias forces the actual explicit alias and its width-N
arguments. All eight corrected native selections pass at
`vrun-b3a87b43-7365-4f72-97d6-ec6d50644bf8`, with final number and no diagnostics.
The checked-key draft uses the shared alias hash/comparison owner and preserves
mapper frame lifetime. Invalidation's sparse-table scan still needs an
indivisible-operation bound and separate end-to-end resource proof; the hash
witness does not establish either. The implementation remains uninstalled until
the restored snapshot and new final-query RED checks finish.


The combined restored snapshot is now complete: alias compatibility passes at
`vrun-6f34b68a-e6be-4c2a-9c83-3b04606f18ad`, union/intersection boundaries
and parameter names at `vrun-947cf9bf-2aff-416a-b455-297fb81d3374`, all-target
Clippy at `vrun-aee20611-066b-481f-bc27-bd1973740932`, and all seven selected
oracle harnesses at `vrun-0f4f6888-04ab-4d10-9c6b-ce1653623461`. The driver
verifies unchanged Rust hashes throughout. Its closing log incorrectly says six
harness files; the receipt and manifest contain the actual seven paths.
The tuple harness with explicit empty-diagnostic assertions passes again at
`vrun-dd44cbd1-1ec1-4378-9d43-3fc6291b2840`.

An inference overload example invokes invalidation with no active mapper frames
(`vrun-870099ac-23bf-4440-ae92-7747a04c5714`, failed control). A context-sensitive
callback establishes live-frame invalidation in all eight option/width cases;
together with alias hashing, its sixteen native selections pass at
`vrun-3cb9cc3c-c0bf-4a87-8452-56584d996510`. Those frames hold no entries,
so nonempty-cache teardown remains a separate control obligation.


The new final-query observer tests establish independent pre-change failures:
all three signature obligations fail at `vrun-13e68ee6-527c-482c-bf92-65c70de34a12`;
all 48 tuple obligations fail at `vrun-bd3e4c81-9b76-4d04-bdee-0053d8910400`.
Every failure is the final resource-outcome assertion after the healthy baseline;
none is a compiler, selection or unsupported-feature failure.

A two-input context-sensitive callback establishes nonempty mapper invalidation.
All 24 combined native selections pass at
`vrun-57791756-c15f-4214-a618-da0792579e2b`. Empty invalidation visits one live
frame with zero entries; nonempty invalidation visits two frames and one retained
entry, in every option/width state. The harness now pins those exact counts.
Five final-query Rust tests are installed before production changes: alias key
hashing, empty/nonempty frame visits, sparse-table scan and retained-entry drop.
The table's indivisible sparse scan still needs final size/latency calibration;
these tests establish charges, not an interruption point inside the standard
library's bucket scan.


All five active-mapper pre-change tests fail at the intended final resource
assertion in `vrun-22c622e1-9b27-4266-8320-7d73326e1c71`. The installed fixes
move signature, tuple and active-mapper keys to checked storage; key identity and
native cache barriers remain unchanged. Signature misses move their input
arguments to the factory after copying the retained key, removing an unchecked
second copy. Mapper frames reserve checked storage, and invalidation charges
frame visits, sparse bucket capacity and occupied entries through their owners.
The original production files and all input hashes are archived under
`.cc/temp/completion-next/signature-tuple-mapper-pre-production/`.

Formatting passes at `vrun-e2ff1d23-ce0f-4fcd-8c45-e8ffb2220768` after applying
31 reported hunks. All 56 final-query obligations now pass: three signature tests
at `vrun-b738ce27-eb34-41fd-871c-0edb7a3cd11f`, 48 tuple tests at
`vrun-c423e4e4-4371-4897-858a-28023def85f7`, and five mapper tests at
`vrun-5a6efc16-a01f-4054-b401-6953b01b762c`. The three-harness native replay
passes all 168 selections at `vrun-fe764ed4-34a0-43ab-b236-14e1b811d684`.
Ten isolated omission trials are in progress before collision/compatibility
validation. These scoped passes do not close the whole-evaluator or calibration
gates.

The next union audit finds unchecked keys in the union-of-union pair cache,
subtype reduction cache and the model's two resumed reduction-answer tables.
Sixteen native controls pass at `vrun-dd0ea569-8291-4d99-8c0c-0b4ab883fd54`:
the pair question hashes exactly width alias arguments once; the subtype
question hashes two width-sized keys, with one miss and one hit. Both final
results are number. Pair selection has no diagnostics; subtype selection has
only the two recorded comma-expression diagnostics 2695. The draft keeps these
native questions distinct, shares ordered-identity key operations, and retains
subtype answers as immutable Arc slices to avoid an unchecked Vec clone on hits.
It remains uninstalled while the signature/tuple/mapper snapshot is validated.


All ten signature/tuple/mapper omission trials fail exactly their selected
final-query assertions and restore the complete Rust source hashes. Signature
hash/comparison/payload each fail one test; tuple hash/comparison/payload each
fail their sixteen flag/readonly/label variants; mapper hashing, scanning and
entry drop each fail one test, and frame visits fail both empty/nonempty tests.
The exact runs and mutant hashes are in
`.cc/temp/completion-next/signature-tuple-mapper-omissions-manifest.json` and the
validation index. No omission remains installed.

Forced-zero fingerprints across fourteen cache families preserve all nine
selected signature, tuple and inference suites. Exact originals are restored.
The three collision scopes pass at
`vrun-8de51c4f-b9a4-4670-9586-1bf71f2444c3`,
`vrun-e7b19e86-ecff-4d97-a7c1-e62e158a69a1`,
`vrun-97f35892-7531-458d-846f-fdea7905f122`.
The full restored library/compatibility/Clippy replay is now running.


The strengthened union control, with exact native key/hit/miss counts and
recorded diagnostics, passes at `vrun-9afb09e6-7eb8-4bc1-a1b8-f4f9c840df2d`.
Its implementation draft and four final-query regressions remain uninstalled.


A further exact-name cache audit is prepared independently of the running Rust
snapshot. All 96 native unresolved-name controls pass at
`vrun-f4f70575-91e2-4e00-a200-2d49839fb035`: bare/qualified paths cross ASCII,
BMP, supplementary, escaped ASCII/supplementary and leading-underscore names,
with widths 16/64 and all four checker-option states. Exact decoded maximum
path bytes, cache hit/miss byte obligations and diagnostics are pinned. Native
continuation returns number, but the product must retain InvalidSemantic and
all actual unresolved-symbol recovery locations.

The audit finds unchecked Source component copies and vector growth as well as
evaluator path concatenation, full-path keys, retained key copies and terminal
symbol header bytes. The uninstalled draft reuses Source's shared-text owner,
adds checked exact-text storage over CheckedMap, and uses existing checked
string operations for the path. It preserves publication before Parent and
declaredType installation. Request Control enters through the existing member
state constructor. It does not reinterpret names, suppress recovery or make
unresolved aliases healthy. This draft must be regenerated after the union
state constructor change, since both touch Evaluator initialization.


## Signature, tuple and active-mapper cache checkpoint

The restored combined snapshot passes formatting
`vrun-b6c1ad01-c17f-4af1-ab97-30c6131be33f`, the full library
`vrun-b86938ea-00f0-4b59-a40b-ba2c4572a6c1`, nine signature/tuple/inference
compatibility roots (three scopes recorded in the validation index), and
all-target Clippy `vrun-d7f5b36e-056f-44f0-9e7c-cded55dda0ee`.
The earlier ten independent omission trials each fail exactly the intended
final-query assertions; forced-zero fingerprints preserve all nine roots;
all mutations are exactly restored. The combined native gate checks 168
selections at `vrun-fe764ed4-34a0-43ab-b236-14e1b811d684`.

Signature instantiation and tuple target caches now own controlled key hashing,
comparison and retained payload, using actual ordered identities and native
readonly/label distinctions. Active mapper keys and explicit inference cache
invalidation own frame visits, sparse scans and entry releases. Normal mapper
frame retirement is a separate remaining operation under investigation.
This checkpoint does not close whole-evaluator accounting, payload/recovery
copies, finite production budget calibration or final service/teardown gates.


The four union-reduction final-query regressions fail exactly their resource
assertions before the fix (`vrun-d62d5b66-b600-4aeb-92f2-e323cf491f65`), then
all pass with controlled pair keys, shared ordered-list key storage and Arc
subtype results (`vrun-5b9643f9-527d-43bf-9def-0b7b3021c673`). Formatting passes
`vrun-42122510-0436-4fa9-934e-a6f13c8ac208`. Independent omissions, collisions
and combined compatibility remain pending.

Normal `popActiveMapper` is distinct from explicit inference invalidation.
A native probe confirms two retired frames with exactly 16/64 total retained
entries in the existing alias fixture, across all four option states. Exact
control passes `vrun-8dc98e3c-fed3-41d1-8dd9-ff5e45f50563`. The draft clears through the existing checked
map owner before normal frame release; it never charges from a destructor.
Its Rust regression and implementation remain uninstalled.


All nine unresolved-name resource regressions fail at their intended final
resource assertions before the implementation
(`vrun-18f4e1a3-66d4-4e29-959b-e273f866cf58`). The first implemented replay
passes eight and exposes a unit mismatch in the path-work test
(`vrun-0685678a-35ef-42a7-a566-568c2cf61b5f`): push_text appends one bounded
Unicode scalar per work unit, whereas key hashing and copying traverse bytes.
The test now uses the native fixture's repeated scalar count for path work and
encoded byte count for the other obligations; no production debit changed.
The corrected nine-test matrix passes
`vrun-20df8480-e92d-40e1-ada9-a60c02e81ce5`, preserving the exact InvalidSemantic
primary location and ordered causes, exact under/at/over limits and healthy
successors for all 96 name/qualification/option/width cases.

The three normal mapper-retirement tests fail at their resource assertions
before implementation (`vrun-e09f4d34-e1e1-4746-a568-d44c174432f0`). Normal
retirement now uses the existing checked clear owner, followed by frame release.
This does not add resource acquisition to unwind cleanup or change the policy
for abandoned request teardown. GREEN and independent omissions are pending.


Normal mapper retirement passes its three final-query regressions
`vrun-5355f545-8ff4-4bff-8614-db7b6b7060ec`; the existing five mapper cache and
inference invalidation tests also pass
`vrun-6ff7507a-d7cb-443e-a929-dfef83a659cf`. The combined union/name/pop native
replay passes all 120 selections in three harnesses
`vrun-78b9fd36-639d-4eb3-b3ac-b54976695f50`. Thirteen independent omission
trials, forced collisions and restored full-library/compatibility/Clippy checks
are running against frozen Rust sources.

The next composite-property key increment is drafted, not installed. It reuses
NativeStringValue's controlled canonical fingerprint and equality; it does not
add identifier normalization, generated-name spelling or ordering. The existing
composite owner keeps actual type identity, augmentation policy, publication
before flag inspection and first-writer propagation. CheckedMap owns bucket
scans and storage. Embedding request Control into immutable binder identities
would couple loading and request lifetimes; explicit prepared keys keep that
capability in the request cache instead.

The native fixture's first policy assumption was wrong: intersections use
non-augmented lookups even after inventory population. Failed control
`vrun-b98c994f-a342-4ae1-b700-0a6450b75270` is preserved. The corrected control
passes all 96 name/layout/option/width selections
`vrun-b598d9d2-aae0-4613-81a5-93c90690ffa0`, with number, no diagnostics and
exact augmented/non-augmented lookup counts. A strengthened byte/hit/miss replay
is pending. No Rust composite test or production change has been installed.


All thirteen independent union/name/pop omission trials now fail exactly their
intended final-query assertions. Each trial restores the same complete Rust
snapshot. The six unresolved-name dimensions cover nine tests: hash, equality,
retained key bytes, path work/bytes, header work/bytes and Source work/bytes.
The first Source omission had an ill-typed Arc conversion and failed to compile
(`vrun-32c2a4ca-266e-4e23-8e83-305fb3e51ef6`); this is excluded from resource
proof. The corrected mutation fails exactly both Source assertions
(`vrun-61a837a7-123b-4386-8e50-4ca58bf112b5`). The combined manifest is
`.cc/temp/completion-next/union-name-pop-omissions-combined.json`.
Forced-collision and restored compatibility/library/Clippy checks remain active.

The strengthened composite control passes
`vrun-8467579e-74f4-4a9a-a0cd-dfa2f4ef88a7`: all 96 selections assert exact
native name-byte lengths, lookup/hit/miss bytes, augmentation-policy counts,
number and no diagnostics. Its queued Rust RED installer waits for the running
restored snapshot's final Clippy success. It will generate the draft against
restored production and install only the two tests; production application is
separate and requires inspecting their results.


The restored union/name/pop snapshot passes the full library
`vrun-6d9c52b4-54b1-4283-bc8e-ea4e0c6babe1` and all nine compatibility roots:
union scope `vrun-f92ecae3-f61a-49d6-afa1-2995fc4eda75`, name scope
`vrun-2ea3fafe-0908-4dad-b926-50c52f686924`, inference scope
`vrun-861ed11a-83ca-4ab9-94aa-80e4de0321c6`. All three forced-collision scopes
also pass and restore exact originals (recorded in the validation index).
All-target Clippy is running; this snapshot remains frozen until it completes.

The composite native matrix now includes 40 opaque-name selections: unique
symbols and declaring-class private fields on union/intersection receivers,
plus a same-spelled/different-identity conditional whose final result changes
from number to string if keys merge. The first identity witness made only cold
misses (`vrun-0e0c0d30-b8ca-4a09-ae35-8b868156f994`), so it could not prove
comparison work. Warming both value reads before the conditional establishes
actual cache hits; all 40 corrected selections pass
`vrun-54693818-30ff-4ee5-9b34-702df8bba08a`.
The strengthened combined 136-selection replay passes `vrun-93bd4584-683c-4caa-80f0-2d487fdec818`.
Opaque checks assert work-bearing bytes and hit/policy counts without assigning
public meaning to decimal compiler identity spellings.

After the current restored Clippy gate, the queued composite workflow checks
both fixed-name tests' exact RED, adds the opaque tests and ESNext fixture
option, checks all four exact RED assertions, then applies the reviewed cache
change and runs formatting and scoped GREEN. Each next step requires successful
native controls and the expected preceding verdict. It stops on any mismatch;
it does not mark the broader T/Q3/Q4 work complete.


Composite fixed-name RED is `vrun-30579c1d-606f-4d30-8f43-5c9fd4a57387`;
all four fixed/opaque RED assertions fail as expected in
`vrun-5405762c-7274-4040-9808-620041928683`. The checked cache then passes three
and exposes an invalid comparison-work assumption in the opaque test
(`vrun-40b45f0f-db4e-4cc3-8628-fa24516143d8`). Diagnostic replay
`vrun-733f44f8-0780-4bc9-be67-2cf6df004a6b` confirms the first unique-symbol
hit shares canonical byte storage and uses equal_bytes' existing pointer
shortcut. Requiring width byte visits on that hit would prohibit a valid
bounded operation. No production charge or shortcut changes.

Opaque comparison tests now require the one/two actual wide-key hits pinned by
the compiler; hashing still requires width bytes, and the fixed-name matrix
retains full-byte comparison obligations. All four tests pass
`vrun-8cd5d275-ffaa-489a-a389-f01f20a7b97c`. Shared test diagnostics identify
owner usage and the selected request when a required debit is absent, while
still asserting the final query's resource error.

Independent canonical-name hash omission fails exactly the fixed and opaque
hash tests (`vrun-67b1beba-baf3-443b-9434-b4e8648132ea`), leaving fixed key-field
charges enabled. Independent canonical equality omission fails exactly both
comparison tests (`vrun-40d61add-f3c7-463f-b1ed-ca077b5ec261`), leaving type and
policy comparison charges enabled. Exact sources restore after each trial.
Forced fingerprints also preserve all six selected property/private/discriminant
compatibility roots and the complete four-test resource matrix, including the
publicly observable opaque-identity distinction. Exact runs and source hashes
are in the validation index and composite cache manifests. The restored full
library/compatibility/Clippy replay is running.


The composite snapshot's restored full library passes
`vrun-e67e36d6-b11a-4d3e-8616-dd90b65a70d0`; both three-root compatibility
scopes pass `vrun-e78b6696-b2cf-43a4-801f-5b2d0398e382` and
`vrun-eb8b6f86-bf70-4669-95d4-92a5289a72e0`. All-target Clippy passes
`vrun-98f0307d-56ac-4a16-92df-d2bfaf0c8035`, with unchanged Rust hashes
through the restored replay. This closes composite key accounting, not
property payload construction/copying or the broader delivery gates.


## Object and widening name-table accounting

Request-local NameMap now routes grammar duplicate checks, strict-only spread
checks, object-member positions, widening child contexts, missing-property
caching, sibling positions and widened output membership through the existing
NativeStringValue canonical hash/equality owner and CheckedMap storage. Names
retain shared bytes and opaque identities. Ordered projections keep first
insertion position and last value. Context constructors seed the same request
control; no capability is attached to immutable binder names. Object-member
snapshot headers and the affected vector/context growth are checked. Property
payload/recovery cloning and whole-evaluator coverage remain separate gates.

The native matrix passes 216 fixed-name selections over six name forms, two
widths, strict/null-loose, noCheck and exact-optional states
(`vrun-25ff0225-733b-47d0-930c-3c7f16475261`). Twenty-four additional selections
cover same-spelled distinct unique-symbol identities and widening
(`vrun-71fb4cde-2d7a-4ea5-9d38-70cc8ccce79e`). A conditional returns number
only while the two opaque keys remain distinct; collision tests retain this
public result. Exact native lookup/hit bytes and diagnostic sequences are saved
with compiler/input hashes.

The first native pilot had module-local library declarations and therefore
never reached widening. Its partial capture and fixtures are archived; the
run was cancelled (`vrun-ecf52ff5-0903-4b7e-8c11-096576914bf5`). A second pilot
confirmed that merely adding declarations without moving them out of module
scope was insufficient. Global declarations plus separately named query
functions fix the fixture. Another initial expectation incorrectly added an
underscore to native fixed-name bytes; failed control
`vrun-96970c84-9891-4e73-a818-1322b77e4b9f` is preserved. Native table keys keep
the exact three source underscores in this fixture.

Three initial final-query resource tests fail without the owner
(`vrun-0abfb4d3-eaf6-49f2-95ec-dd4be80880fb`). The first GREEN attempt exposed
an invalid test requirement for a comparison in loose spread checks
(`vrun-832a02ba-5be7-48a0-8c97-bae1e825ad22`): native checkSpreadPropOverrides
is strict-null-only, and the loose fixture has separate cold tables. Corrected
fixed-name tests pass `vrun-2dd389d9-eb96-433b-b05b-fa0ac9d64efb`; all six
expanded fixed/opaque tests pass `vrun-1a89d097-2fae-4528-8079-1bf52bd51655`.
Each requires a final resource error, exact total-limit boundaries, flat depth
and a healthy successor on the same Project. Shared-key comparison can use the
existing pointer shortcut; cold opaque misses do not claim comparison work.

Independent hash, equality and table-growth omissions each fail exactly their
two intended tests (`vrun-35506697-dba1-4a98-9e42-b2d15a820a2b`,
`vrun-55feb481-7f93-4b0b-9d26-75ee4fba4a7f`,
`vrun-fbd3baf0-2c01-4d44-879c-8d2b3d0acc2b`) and restore exact source bytes.
The forced-collision and restored library/compatibility/Clippy replay is running.


The object-name snapshot completes its restored replay: forced-collision resource
matrix `vrun-0a7901d4-16f7-4579-8ce4-a6e948b054f9`, collision compatibility
`vrun-7bfd36f9-ea53-40a9-a7b0-b0d66da2af16`, full library
`vrun-baeca9c9-28b1-4bc0-9b47-6cb14c8de341`, restored compatibility
`vrun-5fc7f764-d0f7-49d3-8c91-79ccc7cc3d60`, and all-target Clippy
`vrun-90084e1f-f03f-46e3-8f35-b0eb3fd4212a`. Exact Rust hashes remain unchanged
through the restored replay; all omission/collision mutations restore.

The next containment control observes native couldContainTypeVariablesWorker
without new semantic demands. Eight/24 nested reference arguments make 17/49
calls at peak 9/25; 16/64 flat alternatives make 37/133 calls at peak 2. All 16
strict/null-loose/noCheck selections return number without diagnostics and pass
exact counter checks (`vrun-a11d2795-68a5-4b6b-985b-2d936f2b89de`). The scoped
Rust visit test fails with no entry debit
(`vrun-678e2351-17e7-47e5-b1a8-adc4aceeab07`). Existing whole-query depth
boundaries pass at 66/178 before the new guard: earlier source/instantiation
work dominates those peaks, so they are not independent evidence for this
owner's recursive depth. The new scoped entry guard is installed; GREEN is
pending. Broader cache/payload accounting and final resource gates remain open.

## Containment scoped closeout (2026-10-15)

The final containment fixtures separate deep and flat declarations into different
source files, so an unrelated deep declaration cannot dominate the flat query's
parse depth. All 16 pinned-native selections pass
(`vrun-3c67c817-d865-41c7-b780-102310aeebb6`); restored resource tests pass
(`vrun-b5feab4d-ecb5-490c-8a27-63de03a806f1`) and the three generic-alias,
call-inference and mapped-type compatibility roots pass
(`vrun-7a5c2c08-2ad8-4548-bcd6-5ac07202f8fc`). Removing only the containment
entry guard fails exactly the intended visit test
(`vrun-4133bda7-9a12-40d6-b30b-825c027a3e47`); production is restored and
formatting passes (`vrun-443967d9-28a8-47c8-9da2-fc8f388e40f1`).

The request-wide under/at/over depth checks are compatibility evidence, not an
independent proof of this owner's depth guard: earlier parsing/instantiation
still determines peak depth, and that depth test also passes the guard omission.
Full-library and Clippy checks after containment join the next coherent batch.
Header/name construction, remaining evaluator accounting, production calibration
and the final service/stack/corpus gates remain open.

## Native symbol header allocation (2026-10-15)

`NativeSymbol` now owns publication of every fresh header, including created,
instantiated, declaration-replaced, empty-type-literal and bound-observation
headers. All request callers pass their existing Control; binder inspection
passes None. `resources::shared` charges a record move and its payload before
Arc allocation and forces a deadline check afterward. Headers retain no request
capability. Fixed canonical name payloads are shared instead of being copied by
`created_with_key` and `with_identity_and_key`. Latent declaration failures are
shared too, so cloning a header cannot deep-copy their names/reasons. Observing
those failures remains lazy and preserves their original kind. Allocation of
child payloads before these factories and copies when consuming failures remain
outside this increment.

The chosen boundary extends the existing factories instead of requiring callers
to precharge headers or storing a request controller inside immutable identities.
The exact name-family decision lives once in NativeSymbolName::from_key; fixed
byte sharing delegates to the existing NativeStringValue representation.
Reserved-name construction reuses its existing internal-name owner with explicit
optional control, retaining the loading policy.

The 32 fixed-name native selections pass
(`vrun-6a0933b3-ad40-4bdd-93eb-bd96962a3933`); the strengthened 56-selection
fixed/private/unique replay passes
(`vrun-ed0cc863-d8b1-419c-a090-8ceb23ee5a12`). Before summary, the native
newSymbol counter is 24 + width for object/spread/generic-member queries and
24 + 3*width for mapped-member queries, at widths 16/64 in all four strict-null
and noCheck combinations. Opaque controls preserve declaring-class private names
and distinct same-spelled unique symbols. The instrumentation only observes the
existing factory; it adds no semantic acquisition. An initial native probe
incorrectly expected TS2695 on member reads; those reads have no such diagnostic.
The corrected diagnostic distinctions are pinned in the final harness.

All four resource tests first fail at their intended final-query budget assertion
(`vrun-92b16f5a-0e73-4e78-8274-80a9ca266d7d`) and pass after the change
(`vrun-f4f53f90-4cc9-4d66-95f4-c0995eddc294`). Independent work-only and
storage-only omissions fail exactly their two intended tests
(`vrun-f0351dde-ba1a-4637-9a98-ddbd4c495e2e` and
`vrun-93c5bd69-9f99-4b9b-b3fb-8e2fcc252a66`). Restored tests pass
(`vrun-e642d049-02b6-46da-a10c-dc65ee0187af`), as do the seven object,
widening, generic-alias, mapped, private-access, late-symbol and generated-order
compatibility roots (`vrun-c715bce5-40b0-47c7-91e4-ee62b8d0a622`).

The full library run `vrun-6bb07e32-2f83-4173-8610-ddab68263499` is not GREEN:
457 tests pass, one is ignored, and the old anonymous-cache whole-query work-gap
test fails with equal gaps of eight at widths 16/64. Its comparison operation
still has its original correct charges; a cache miss now also pays for headers,
so subtracting complete request totals no longer isolates argument comparisons.
The replacement test observes only same_types within the existing key comparison
and enforces a native-derived width minimum on the final query. This is cfg(test)
observation wiring, not a production semantic or accounting change. It passes
(`vrun-798be042-d83c-4528-b744-48dd754d2f83` initial observation,
`vrun-6684e93d-4668-4eaf-918f-41d46b597c75` argument-work-only omission RED,
`vrun-3528ab68-b346-47af-9b3b-436f7ead89d4` restored GREEN). The omission
keeps header, hashing, alias and all other accounting intact. Full-library replay
and Clippy will include this test correction and the next owned-name increment.

Input hashes and exact restored omission snapshots are under
`.cc/temp/completion-next/symbol-header-proof-input-hashes.json`,
`symbol-header-omissions-manifest.json` and `symbol-header-proof-intact/`.
The native oracle snapshot there predates the next opt-in symbol-name hook.
Native key creation/ordering, remaining evaluator accounting, calibration and
final integration/corpus/service/stack proof remain open.

## Owned native symbol names (2026-10-15)

`NativeSymbol::name` now owns the bounded byte copy for consumers needing an
owned `Symbol.Name`. Its two request wrappers retain their original contracts;
the unresolved-alias path moves the returned bytes into String::from_utf8
instead of making a second copy. Unsupported/generated name observations still
fail before copying. The retained-binding test explicitly supplies no request
control. Returning shared bytes through every import/display/alias consumer was
considered; centralizing the existing owned operation avoids moving conversion
and budget knowledge into those callers.

The new scalar-result alias fixtures pin exact decoded ASCII, Unicode,
supplementary, escaped and leading-underscore names at widths 16/64/1024. The
large names span multiple copy chunks while retaining the same shallow tree.
All 72 pinned-native selections pass
(`vrun-24ebcff9-f1a0-4dd1-947b-60585a0c3fe7`), including exact alias-name byte
maxima, final number, TS2695 and no errorType/demand failure in four strict-null
and noCheck combinations. The hook reads the existing alias header; it adds no
symbol lookup or type acquisition. An earlier 48-selection probe used embedded
rather than leading underscores and is superseded by the final fixtures.

The unresolved-name fixture matrix is reused unchanged for created-symbol
recovery: bare/qualified names, six decoded forms, two widths and four option
states. Its existing native controls and exact port failure contract remain the
baseline. Tests assert the final InvalidSemantic reason, primary location,
ordered causes and successful subsequent query, or the precise pathless resource
failure when the copy is denied. This does not relabel native recovery as a
healthy answer.

All four tests fail for missing copy accounting before production changes
(`vrun-58a74d32-b3ef-4c03-95f1-d909594f65eb`) and pass after installation
(`vrun-6d28e265-58e6-43fb-b7a1-7568c4d7cddf`). Work-only omission fails exactly
the two work tests (`vrun-299dd27d-e875-484e-bdb5-fa6c2167c18e`); storage-only
omission fails exactly the two storage tests
(`vrun-ccb3d6e4-5fa3-4ca7-90e6-cb855cffab52`). Both preserve unrelated
accounting and deadline checks. Production is restored byte-for-byte; all four
tests pass (`vrun-0535af85-df36-4365-ae62-97f00859e990`), as do the nine older
unresolved-name resource tests (`vrun-4154cbb9-9942-4a0e-a35f-2c69a03f5bc5`)
and five alias/unresolved-alias/symbol-location/generated-order/late-symbol
compatibility roots (`vrun-b67f37e4-50d0-4908-a352-7fe8be0b2183`). Formatting
passes (`vrun-d65cd3b9-c957-47fc-a8cd-cc192ec02e18`). The combined header,
anonymous-test and owned-name snapshot is undergoing full-library replay, with
all-target Clippy next; neither result is claimed here.

Exact proof inputs and restored omission hashes are recorded in
`.cc/temp/completion-next/symbol-name-proof-input-hashes.json`,
`symbol-name-copy-intact.rs` and `symbol-name-omissions-manifest.json`.
Native key construction/ordering, other evaluator caches/copies/loops,
production limit calibration and final integration/release/service/stack proof
remain open.

### Combined header/name closeout

The combined snapshot passes the restored full library
(`vrun-17c7c19b-e404-4a25-9317-725fbc022e05`) and all-target Clippy
(`vrun-7b04f35d-0e33-41b2-ac2a-b7379f051978`). This closes the earlier
anonymous-cache test failure and completes the bounded header/owned-name
increments. Both proof drivers have finished, all omission mutations are
restored, and no automatic source mutation is pending. The validation index
records their terminal receipts. This is not a final T/Q3/Q4 delivery gate:
production work/byte/depth limits remain unset, and the broader accounting,
calibration, full integration, release corpus and service/stack obligations
listed above remain open.

## Native key construction, observation and ordering controls (2026-10-15)

Fixed native byte construction now distinguishes borrowed slices, owned vectors
and existing shared Arc payloads. Copying/conversion occurs under the explicit
operation Control; shared payloads remain shared. The old generic Into<Arc>
contract could allocate before reaching the factory. Literal text construction
also avoids its intermediate Arc<str>. Numeric constant formatting and all
request key factories reuse the same controller. Captured loading and retained
inspection supply None explicitly.

The existing from_parts canonical byte/identity coalescing now lives in one
private NativeStringBuilder. Private/unique factories and the known-symbol
fallback append borrowed byte runs through it, eliminating unchecked prefix
concatenation and temporary Arc parts. The established private/unique formatting
policy and actual identities are unchanged. Required control accessors on the
shared name/module-export adapters keep request and loading policy explicit;
immutable inputs, keys and headers retain no controller.

Key-family observation passes control through unique-name slicing and display
length. Native symbol ordering uses the existing checked byte-comparison and
canonical-equality owners, guards recursive comparison, and retains one checked
lazy rank-publication owner. Declaration/name/identity precedence and the adopted
generated-name ordering policy remain unchanged. Ordering never supplies literal
comparison or spelling. The fixed-key rank table retains exact identities and
left-before-right publication. These controls do not cover other raw native
symbol-table hashing, general cached-value clones or presentation allocations.

All ten final-query tests first fail at the intended missing-budget assertions
(`vrun-813be8d9-00c7-4b6c-af50-81a27ec2f72a`), then pass
(`vrun-cbf4387f-c306-4def-a2d1-29d074ae796c`). They cover fixed/private/unique
construction, unique-key observation and rank work/storage. The fixed-name matrix
uses six decoded name forms, widths 16/64, strict/loose/exact-optional and noCheck
states; the opaque matrix preserves declaring-class private keys and distinct
same-spelled unique keys. Final roots remain clean number, each resource ledger
is limited in its own request, exact total under/at/over boundaries are checked,
flat width preserves peak depth, and subsequent semantic requests remain healthy.
The fresh 112-selection native replay passes
(`vrun-f69cdb12-f132-41ae-be1c-bf0992338576`), pinning existing decoded-name
and opaque-key observations without adding a new compiler hook.

Seven restored counterfactual trials detect the intended regression only:

- Borrowed fixed-name work/storage separately:
  `vrun-879b5bd0-70ce-45dd-a56d-5f386c1b5957`,
  `vrun-af51c4d7-6ac6-45c4-bf17-d2d2fb7144a0`.
- Private/unique/unique-observation controller routing:
  `vrun-3b11f77d-3779-4b4a-832a-94195f7116d8`,
  `vrun-2ae71a78-c6e5-4668-8f9d-b96636757b13`,
  `vrun-2f761fcb-560f-4f33-911a-eb69740235fe`.
  Each routing omission fails its separate work and memory tests. These are
  routing regressions, not quantity-only ablations or worst-case deadline proof.
- Rank work/storage separately:
  `vrun-fd1842cc-54e6-4edf-9643-e72d8ad58de7`,
  `vrun-100b22ca-bfeb-4c56-b1bb-0e9fc577fd56`.

All mutations are restored byte-for-byte; all ten tests pass
(`vrun-b3f2e6d0-6177-4676-845f-5714dd7dab57`). Eight generated-key/equality/
operation/ordering/parameter-name/private/object/widening compatibility roots
pass (`vrun-8dbe6934-e559-41d4-bb3e-7b5aac05c6b3`); formatting passes
(`vrun-e5272641-bd95-43fe-9c87-16375a630261`). The first full-library replay
was interrupted with no output or recorded cause
(`vrun-ae3afe5c-e8b7-4dab-9466-75280e13c4fc`), so it is not GREEN. A retry
is running on unchanged input hashes, followed by all-target Clippy.

The exact inputs, original pre-increment sources and restored mutation hashes
are under `.cc/temp/completion-next/native-name-proof-input-hashes.json`,
`native-name-before/`, `native-name-proof-intact/` and
`native-name-omissions-manifest.json`. Broader evaluator accounting, older
omission sensitivity, production calibration and final full integration,
release/corpus/service/stack/teardown gates remain open.

### Construction/ordering validation closeout

The unchanged-input full-library retry passes
(`vrun-613d4bc9-a031-4ad5-98b7-6a07923d2e4c`). Clippy initially reports one
redundant fixed `"true".to_owned()` after the borrowed-text signature change
(`vrun-b117827c-46b6-402a-af6f-1ee1b2bbe45c`). Removing that allocation is
mechanical routing, so no new behavioral test was added. Existing JSX/property
inference roots pass (`vrun-d704980f-1c22-45e7-b99c-14484dcde978`), formatting
passes (`vrun-b9c30000-ac21-494b-928d-d8024a8206f9`), and all-target Clippy
passes (`vrun-b1d5ee32-7261-4b6c-b907-1198f75b88e2`).
The sole lint delta and its before/after hashes are recorded separately in
`.cc/temp/completion-next/native-name-lint-delta.json`; the original proof
manifest is preserved. This closes the bounded native key increment. Remaining
evaluator tables/payloads/loops, production calibration and final delivery
gates are still open.

## Fixed identity table pilot: alias parameters (2026-10-15)

`resources::FixedMap` adapts pure, bounded, allocation-free identity Eq/Hash
to the existing CheckedMap collision and backing-storage owner. Its marker is
implemented at the identity type owner; Copy alone is insufficient because
borrowed strings can contain unbounded text. Explicit construction keeps the
query controller, and the adapter has no Default fallback. Stored immutable
identities retain no controller. AliasesState now initializes its parameter and
instantiation tables explicitly; alias cache presence, raw identity, defaults,
negative/error results and native acquisition order are unchanged.

The selected representation reuses existing checked collision/growth policy
rather than adding another hash table implementation. Ordinary unique entries
use CheckedMap's inline SmallVec bucket; the retained fingerprint and bucket
metadata are an overhead to include in release calibration. This pilot covers
bounded probes, exact collision comparisons and requested backing-payload
allocation. Variable cached parameter/result/recovery clones, bulk resize
latency, allocator metadata/RSS and teardown remain separate obligations. The
119-field raw-table inventory is not 119 confirmed omissions: some fields already
reserve backing through shared helpers and must be audited before migration.

Twelve pinned compiler selections pass
(`vrun-bd57bbb8-1f5f-4d71-a54e-1b19469a7f5f`): 16/64 distinct non-generic
aliases have empty parameter lists; same-spelled aliases in separate namespaces
retain different declaration identities and arities. Strict/loose null checking
and noCheck variants preserve clean number roots. Four complete-query budget
tests first fail at the missing-stop assertions
(`vrun-b8ebab53-5fe5-43d2-b92e-a448cc7a6eb6`), then pass
(`vrun-64b3933a-d820-42b4-9a93-d6891bbd6b0b`). Limits cover total under/at/over
boundaries, both ledgers independently, flat-width depth and healthy successors.

Removing only alias-parameter controller routing fails exactly these four tests
(`vrun-dfd3e207-21e4-4c71-a34b-975fb7a350fd`). This is a routing omission,
not a quantity-only ablation. Forced-zero fingerprints preserve all four tests
(`vrun-2e575bc2-575a-4547-aee5-6cdb0d63a354`) and four alias/generic/recovery
compatibility roots (`vrun-f3ef1e13-4492-4e3b-b164-5472f1a97f26`). Exact
restoration passes the four resource tests
(`vrun-4d741788-7273-466a-81ac-9a7a276a4019`), the older alias-key regression
(`vrun-128416b2-79f9-4953-94b7-c2a42e427091`) and the same four compatibility
roots (`vrun-248cc03d-4065-421c-ac28-ddaedfa265d0`). Formatting passes
(`vrun-703d0a17-d8e8-46d2-b5b5-8e8364dfd762`). The proof driver has completed
and no omission mutation is pending.

Original sources, exact native/proof input hashes and mutation restoration are
under `.cc/temp/completion-next/fixed-map-before/`,
`fixed-map-native-input-hashes.json`, `fixed-map-proof-input-hashes.json` and
`fixed-map-proof-manifest.json`. This pilot has scoped proof only; the latest
full-library/Clippy gates predate it. Remaining evaluator accounting, production
calibration and final whole-project/release/service/stack/teardown proof remain
open.

## Promised/awaited fixed type-key backing (2026-10-15)

PromisesState now constructs both cachedTypes equivalents explicitly with the
query controller through FixedMap; TypeId's bounded marker observes only its
arena index. The existing positive-only installation policy is unchanged: nil
results remain misses, and no new cached recovery/fallback is introduced.

Eight pinned selections pass with exact counts
(`vrun-7be9ec58-265a-4260-b3d3-ca7ddbc768be`): 16/64 promised reference slots
and 32/128 awaited slots, including the unwrapped object types. All four
strict/loose/null/noCheck states retain clean number. The initial compiler trial
(`vrun-e817fa29-7dd6-42e5-b132-fc985a8dd842`) exposed duplicate global Promise
value declarations in two fixture files, not a product bug; shared globals were
moved to one declaration file. Corrected compiler replay passes
(`vrun-dfc62f78-536e-4b56-8255-3c20277683ad`) before the exact-count refinement.
The equivalent Rust fixture still concatenates those same source bytes.

Four final-query budget tests fail at the intended missing-stop assertions
(`vrun-d12b3574-fe7b-43bb-b0bb-8569a75f469e`), then pass
(`vrun-ecc50760-257e-4796-a9cd-a9a9a55100bb`). Requiring the higher awaited
slot count also passes (`vrun-c1059131-0894-40e8-b3fd-96c292f1ccb3`). Each ledger
has total under/at/over limits, both flat widths preserve peak checked depth,
and a subsequent semantic query remains healthy.

Independent promised/awaited controller-routing omissions each fail exactly
their two budget tests (`vrun-897f2f58-ccea-4f81-9750-a2fea7dc4184`,
`vrun-127fe37b-7428-4cea-893f-ddc6510c3aa8`). These are routing regressions,
not quantity-only ablations. Forced-zero fixed fingerprints preserve all four
tests (`vrun-9f0bed25-5e9a-42b9-a08f-84e9ee89b01c`) and all four promise
inventory/factory/await/recovery compatibility roots
(`vrun-dadbf213-422f-4e5f-b92c-78b5946c4014`). Exact restoration passes the
resource tests (`vrun-2d54f2c5-fc40-4597-9b98-71f477a7051a`) and the same
compatibility roots (`vrun-c389c750-c6c9-4540-aa7f-031f8f3dd8d0`). Formatting
passes (`vrun-7f32fa10-6516-4747-bd70-953175f125de`). The driver has completed
and no automatic mutation remains.

Exact originals, native/proof hashes and restoration are in
`.cc/temp/completion-next/promise-cache-before/`,
`promise-cache-native-input-hashes.json`, `promise-cache-proof-input-hashes.json`
and `promise-cache-proof-manifest.json`. This is scoped backing-table proof.
Awaited recursive entry, stack scans/growth/retirement, then-signature scratch
loops, cached/recovery copies and bulk resize latency remain open. Whole-library
and Clippy gates still predate the FixedMap increments; production calibration
and final whole-project/release/service/stack/teardown proof remain required.

## Awaited invocation and stack controls (2026-10-15)

Complete getPromisedTypeOfPromiseEx and getAwaitedTypeNoAliasEx invocations now
enter the scoped work/depth ledger before any, existing Awaited or cache paths.
PromisesState uses the existing WorkStack for allocation, bounded identity
membership and checked retirement. WorkStack membership delegates to the shared
contains_item owner. Both the union and promised-value branches retain their
native membership and same-type short circuits; all denials propagate through
the established worker abort transport. Drop performs no resource check.

Twenty-four pinned selections pass
(`vrun-8a259b22-3b39-4462-ba74-b9b1cedd1c98`), extending the initial 16
(`vrun-f00dc3a3-1fb3-48de-b4cd-633b054d3657`). Concrete Promise chains have
8/32 layers; union chains have 4/16 layers and twice as many stack frames. Both
have native awaited peaks 9/33 and pointer-comparison totals 28/496. Union
branches perform 13/49 awaited calls, compared with 9/33 for concrete chains.
Flat 16/64-property controls have native peak 2 and no stack comparisons. Four
strict/loose/null/noCheck states preserve clean number roots and zero demand
failures. The ordered warm declarations keep alias construction shallow.

All six initial final-query tests fail
(`vrun-f1e10a3e-b4c6-4798-afdf-a797b1ac436a`), including native awaited peak
33 versus checked peak 21, then pass
(`vrun-abaa0898-9fee-4268-801c-557642fbac99`). Expanded union/flat controls pass
(`vrun-c87b1f54-9791-4678-b0a8-4385e7f11266`). Work/storage limits cover
owner obligations and total under/at/over boundaries. Depth limits reject below
the native recursive requirement and just below observed total peak, accept at
and above it, retain flat-width depth, and permit healthy successor requests.
The successor selector is factored from the existing shared test helper without
changing its behavior. Boolean condition bindings and an equivalent depth
inequality were mechanically adjusted for lint; no new behavior was introduced.

Six independent counterfactuals produce exactly their intended failures:

- Awaited invocation guard: awaited-entry and depth tests fail
  (`vrun-961f4385-58e9-45d3-b03c-65e04915e34d`).
- Promised invocation guard: only its entry test fails
  (`vrun-e45ddad0-6481-4a9b-ab76-98475eafa2f7`).
- Membership controller routing: only scan work fails
  (`vrun-76738263-807b-487f-81a4-b06a30dd5b66`).
- WorkStack push controller routing: only stack-storage fails
  (`vrun-15dabbd0-86ed-435a-b530-892a2a4b4179`).
- Normal pop work debit, retaining deadline checks: only retirement fails
  (`vrun-d130ec03-1ecc-4079-abca-0eb41570b62b`).
- Awaited depth tracking only, retaining invocation work: only depth fails
  (`vrun-9ed0a998-4f92-4a73-931b-2ca2db65f693`).

Routing trials remove the named operation's controller and are not quantity-only
ablations. The depth-only trial establishes owner-specific recursion sensitivity,
unlike the earlier containment fixture. Exact restoration passes all six tests
(`vrun-e785265c-dc48-4944-a9f6-392e13d03827`). Four promise compatibility roots
pass (`vrun-ed20908c-cba7-4258-96f2-759eff2b5461`), as do the older alias-key,
fixed-map and promise-cache resource modules
(`vrun-048dccfd-4f94-47ec-8588-375c6556ba4b`,
`vrun-7b3ae86c-cfc9-43e4-983b-701b8789d509`,
`vrun-8c12a8f6-2f79-4694-b801-d35ccad4dd3d`). Formatting passes
(`vrun-05c027d9-79e6-44d0-b781-b38925eceae0`).

The combined fixed-map, promise-cache and awaited-stack source snapshot passes
the full library (`vrun-3753dfc3-09ca-41a4-918c-7b3f22de3e60`) and all-target
Clippy (`vrun-fd39a77b-41cf-41a5-8091-21efae684fe5`) on verified unchanged
source hashes. All drivers have completed, all mutations are restored and no
automatic source mutation is pending. Exact sources, native inputs and trial
hashes are in `.cc/temp/completion-next/promise-stack-before/`,
`promise-stack-native-input-hashes.json`, `promise-stack-proof-input-hashes.json`
and `promise-stack-proof-manifest.json`.

### Ownership checkpoint and next work

A bounded post-implementation/Ousterhout review confirms that immutable signature
parameter/origin lists still deep-clone through ordinary Signature::clone and
owned call/construct inventory projection. Contextual fields already share Arc
state; preserve their OnceLock/private-factory rules and the mutable by-value
flags/predicate/return header. Shared immutable lists plus controlled owned
projection are the next increment. MetadataView.alias and union constituents
already share Arc payloads and must not be redundantly redesigned. DemandError
needs its own treatment: make_mut alone would retain its untracked variable
copy, and ordered recovery/localization rules must survive any sharing change.
The local review is `.cc/temp/completion-next/resource-copy-design-review.md`.

Thenable scratch loops, other confirmed caches/copies/loops, bulk resize and
normal/abort teardown, production work/byte/depth calibration and final full
integration/harness/semantic/release/service/stack proof remain open. Production
work/byte/depth maxima remain unset. This checkpoint is not T/Q3/Q4 completion.

## Immutable signature payloads and owned projections (2026-10-15)

Ordinary Signature::clone now shares immutable parameter and origin Arc slices.
Its by-value flags/predicate/return header and shared contextual OnceLock/private
factory replacement rules are preserved. SignatureData distinguishes owned Vec
from an existing shared slice through a non-allocating input carrier. The existing
newSignature owner checks owned array publication; shared snapshots reuse their
storage and immutable handles retain no controller. Original Vec/child acquisition
is a separate obligation, not retroactively proved by publication.

Owned call/construct inventory projection copies bounded handles with checked
visits/storage. Expanded-parameter fallbacks and rest prefixes use checked exact
name copies. Public graph origin snapshots and parameter record backing/name
copies are checked; previews still make no child query. Native signatures keep
their actual IDs, raw parameter symbols, transforms, provenance order and factory
mutation behavior.

The final native replay passes 32 scalar/graph selections
(`vrun-aa1e2de0-06cb-4a21-9fc0-503ae55c4aae`): callback parameter width and
callback inventory width independently vary 16/64 under strict/loose null and
noCheck states. Scalar roots remain clean number; graph roots retain their named
interface types. The initial native run
(`vrun-480055a4-8cc6-4223-b748-02b966523f1c`) incorrectly expected TS2695 for
an effectful await left operand; actual diagnostics are empty. Correcting that
assertion passes 16 scalar selections
(`vrun-f4ddef99-51a3-4b3e-81b4-4ae14f406313`) before adding graph controls.

The initial Rust syntax error (`vrun-cffd097e-40e4-4975-8adc-b5544a614194`)
is not behavioral RED. After its cast parentheses are corrected, all five
publication/owned-inventory tests fail at the intended missing-stop assertions
(`vrun-1f1a9778-508f-462e-9184-bcb789b9b19b`). The array prototype passes
those five while four public snapshot tests fail
(`vrun-79aea5fb-86fe-4b6a-a4e6-e2e57d0ed75a`). Origin snapshots then pass.
The two parameter tests initially observed an unused expanded-list copy owner
(`vrun-e9e519a5-2526-4cee-8bbe-e951c06eed50`); increasing graph depth did not
change that (`vrun-a01547a7-4fb7-42c4-9c00-7df99f51de9c`). Actual graph
inspection identified the public record copy. Those tests now measure that
operation and its actual public record size; depth-8 graphs assert full parameter
or signature counts. Location.file is exact normalized String data, not PathBuf.

All nine final-query tests pass
(`vrun-d5fd7a75-e42e-4760-a8d5-dd8b402364e4`), checking independent work/byte
limits, total under/at/over boundaries, flat width and healthy successors. Six
independent trials detect exactly their intended regressions:

- Parameter publication routing: its two work/storage assertions fail
  (`vrun-c9652350-dec6-4b29-ace4-fb1f9a0f8b92`).
- Origin publication routing: its storage assertion fails
  (`vrun-694108a8-fdc3-402a-9737-d916ece4b8e1`).
- Owned inventory work debit, retaining deadline checks: only work fails
  (`vrun-09ce4a5c-0440-4ca2-80b8-a997f6aafcf6`).
- Owned inventory reserve-controller routing: only storage fails
  (`vrun-12ebbf06-7662-4b9e-9bf5-4fb6e8e7b25f`).
- Raw origin snapshot regression: its two assertions fail
  (`vrun-2869bec4-9932-4c97-b0fa-1f392634cffe`).
- Public parameter record controller routing: its two assertions fail
  (`vrun-0fe09a6f-f76c-4d34-91e3-53825990144f`).

Routing trials are not quantity-only or worst-case deadline evidence. Exact
restoration passes all nine (`vrun-1ce2d517-744b-48c2-b176-ff89d353918a`).
Ten signature/constructor/parameter/context/inference compatibility roots pass
(`vrun-d2cf43e0-1f7c-412c-8dc5-750c65408dc4`), as do older signature-cache,
parameter-name, promise-cache and awaited-stack resource modules
(`vrun-8dc05cfd-186e-473d-91b6-76a56c71b396`,
`vrun-35e66887-4b86-4d17-bf8c-a1b16031cfed`,
`vrun-f2a2eaf5-3c53-49b2-9a17-88927232970f`,
`vrun-0f240e20-51b1-4491-888b-22c5d2a0ee18`). Formatting passes
(`vrun-e04160ef-7135-47d0-8915-35a666dce724`).

The frozen restored snapshot passes the full library
(`vrun-f4fcc95a-ab30-446d-98bd-2b5833312691`) and all-target Clippy
(`vrun-fdc35746-6900-47f4-8e07-92c01fc394ec`). The proof driver has finished;
no automatic mutation remains. Original sources and exact inputs/trial hashes
are in `.cc/temp/completion-next/signature-payload-before/`,
`signature-slice-routing.json`, `signature-payload-native-input-hashes.json`,
`signature-payload-proof-input-hashes.json` and
`signature-payload-proof-manifest.json`.

This closes the stated array/owned projection increment, not all signature or
evaluator allocation. Remaining thenable scratch, other caches/loops/child and
recovery copies, bulk resize/teardown, finite production budget calibration and
final full integration/harness/semantic/release/service/stack proof still block
T/Q3/Q4 completion.

## Thenable scratch and borrowed awaited constraints (2026-10-15)

`getPromisedTypeOfPromiseEx` now checks each receiver-filter, accepted first-
parameter and callback-value visit before its semantic acquisition, and checks
all three scratch arrays before growth. Candidate indices retain the original
signature inventory and native order without copying another header list. Nil,
Any, rejected-this output and ordered recovery short circuits are unchanged.
`isAwaitedTypeNeeded` borrows its existing Arc union (or a scalar slice), checks
each `someType` predicate visit and stops at the first match. `isThenableType`
uses the existing shared signature getter for its emptiness question. These
remove two unnecessary owned projections without changing native acquisitions.

The thenable compiler matrix passes 24 selections
(`vrun-b447f454-c576-4331-874c-5d6cdeea2eb8`): widths 16/64 with all receivers
accepted, mixed absent/void/compatible/rejected receivers, or wide callback
inventories, in all four strict-null/noCheck combinations. Six final-query tests
fail before production (`vrun-bfc32296-64f0-44a6-9f5c-f983a3953c61`) and pass
after it (`vrun-2990bd54-aedf-433c-aff5-57cea541614e`). Each checks an independent
work/storage lower bound, total under/at/over limits, flat peak depth and a clean
successor. Six independent omissions each fail only their intended assertion:

- Candidate work: `vrun-35b44d71-14d8-46a8-bd5b-6eb2d8cabc99`.
- Candidate storage routing: `vrun-8748fb9e-3417-47a6-b47f-e92d4509353c`.
- First-parameter work: `vrun-bcbee1a6-06a7-4d70-ad56-eecffa3849ba`.
- First-parameter storage routing: `vrun-dfb0a899-fba3-4056-b4bb-c0f860088c15`.
- Callback work: `vrun-09149bc3-16b3-4da0-890c-ac7882e6ea73`.
- Callback storage routing: `vrun-85ab8d0a-29c2-4a9f-b8ed-b77554c265c0`.

Work trials retain forced deadline checks; storage trials test controller routing,
not allocation-quantity calibration. Exact restoration passes all six
(`vrun-15bfbb1f-c126-48de-921b-ca2ea7517f2c`).

The awaited constraint fixture initially exposed duplicate script globals
(`vrun-17e00761-7cfd-49aa-afd7-b79026b23a01`), then a missing global Awaited
alias (`vrun-a1ef0165-6ec7-434a-8fcb-86ce868dbb10`). A third failed assumption
was that a thenable written last must be visited last
(`vrun-8a700859-86aa-448a-a1a2-812cce8fe448`): native identity order, rather
than written order, governs the union. The controls now distinguish a complete
nonthenable scan from written-first/written-last matches, the nonunion case,
and unconstrained/any/unknown/empty-object skips. A minimal admitted identity
Awaited alias avoids unrelated global recovery; selected expressions discard
the awaited value. The module-isolated matrix passes 64 selections
(`vrun-2205d301-ef05-46b0-8cce-0673f533d4b0`). The final script fixture uses
unique helper/global names, matching the Rust concatenated-source setup; all 64
pass again (`vrun-a4f97c31-9a09-49d6-bf30-18d24d715ca5`). The full scans visit
32/128 alternatives over the two native isAwaitedTypeNeeded calls; scalar scans
visit one each, and top constraints skip the predicate entirely.

The final-query scan assertion fails before production while its skip control
passes (`vrun-00e37896-0af1-4ce1-8988-88b5d29db0cd`). Both pass after the change
(`vrun-40442a2a-3782-46ef-bc44-1710eb0d85da`). Omitting only its work debit,
retaining deadline checks, fails only the scan assertion
(`vrun-3abd4126-e552-40e5-8208-e8cd20bbd16d`). Exact restoration passes both
(`vrun-c89cb689-979a-4dfb-b697-ece897b35957`). The restored thenable six,
signature payload nine, promise stack six and promise cache four tests pass
(`vrun-f8af2504-cad3-483d-868a-035151df758a`,
`vrun-5e81fdcb-04bb-406d-927c-dfc979a4971d`,
`vrun-8f4ff1fe-e4c1-460b-a11c-41235396709a`,
`vrun-533a9fc2-a8f9-46f8-affa-e8dba56ec5c2`). Five promise/await/iteration
compatibility roots pass (`vrun-26045344-b2cc-4265-92e9-712d269e792d`), and the
combined 88-selection native replay passes
(`vrun-007b0f44-4606-4ff2-92be-f69c13dff5b1`). Formatting passes
(`vrun-47ac4732-f7e7-4a01-82a4-da3b40f3a60d`). The frozen restored source passes
the full library (`vrun-7ad4a069-1025-4e94-9c77-a529cbe18da1`) and all-target
Clippy (`vrun-3a7667ca-7a39-4bb6-92ee-27c15fe5d887`). The driver is finished;
all mutations are restored and none are queued.

Exact original/intact sources, test/input hashes and per-trial hashes are under
`.cc/temp/completion-next/thenable-scratch-*` and `awaited-constraint-*`.
These close only the stated operations. Other evaluator caches/loops/payloads,
recovery and source recipe copies, bulk resize/teardown, finite production budget
calibration and final whole-project/release/service/stack proof remain open.

## Immutable captured recipes (2026-10-15)

Source Expression and TypeExpression now clone a single shared immutable record,
including their locations and recursive forms. Their records/forms no longer
implement deep Clone and expose no mutable dereference. The existing expression
worker builds an owned record, installs the actual pre-parenthesis cache root,
then publishes it; the special type-query this producer supplies its own actual
root. TypeExpression's sole producer also publishes only its completed record.
The existing resources::shared owner checks each record's work and fixed payload
through CapturedSyntax's request controller, which is absent from the shared data.
Child construction, owned output/location copies and teardown are separate debts.

The original projection compiler controls pass 168 selections
(`vrun-b7ae0dc0-3281-4ca8-810d-a3d6998ed593`). Four new final-query work/storage
assertions fail before production at missing-limit assertions
(`vrun-361a7209-b615-4fbe-b8f5-20b81679b4e8`). The first prototype compile
fails because two spread-argument branches move operand.location
(`vrun-2d49159d-c0fc-4a34-a9e3-08d9868ee582`); that is not behavioral RED.
Both now retain the captured operand while borrowing its range. Synthetic
branches borrow their existing ranges too, eliminating their unnecessary copies.
This wiring preserves the existing iteration operation and adds no new copy API.

The next run passes type-record checks but exposes the expression fixture's
missing healthy successor (`vrun-333ae4b7-8dcb-4552-aa85-0b3cd1f8a6f4`). Each
expression fixture now includes a uniquely named number control, separately
verified against the pinned compiler. The updated 172-selection compiler replay
passes (`vrun-c41560a1-b667-4269-bcc1-35ebef058921`), including all existing
unselected-operand/branch checks and their actual diagnostic expectations.
All four final-query assertions pass
(`vrun-723682b4-7323-40c7-9e08-296e2d06ca38`). They cover every existing type/
unary/await projection family at widths 16/64 and both noCheck states, independent
work/storage lower bounds, total under/at/over limits, flat depth and successors.
Storage minima count nominal Location/form record fields, excluding padding,
Arc metadata and separate child buffers.

Removing the type-record controller fails only its work/storage tests
(`vrun-d81b3282-bda7-40fc-9752-5aec6060659b`); removing the expression-record
controller fails only its two tests
(`vrun-c27e9f6f-c81a-4097-b85f-8836a0c23860`). These are routing trials, not
quantity-only or worst-case latency proof. Exact restoration passes all four
(`vrun-5b0464db-10cd-496b-8f5e-77a42e6a5673`). Older type/expression projection
modules pass (`vrun-f95d0cae-0a28-4f6d-9fdb-db6a0f2130fe`,
`vrun-9bad1cbd-1b6b-4269-9ffc-c63ac39ecf07`), as do ten argument, source,
operator, await, contextual, signature, type-query and return-body compatibility
roots (`vrun-01491627-12ae-4c35-9d5b-c526ebb7ae72`). Formatting passes
(`vrun-0c007427-7e94-4545-99d7-94790f89149e`). The proof driver is finished;
all mutations are restored. Design choice, original/intact sources and exact
proof/native inputs are under `.cc/temp/completion-next/source-recipe-*`.

The latest full library/Clippy gates precede this representation change. A fresh
release corpus diagnostic is running on its frozen restored source; full final
checks, other evaluator caches/loops/payloads and recovery copies, finite budget
calibration and service/stack/teardown proof remain open.

## Fresh repeated release corpus and memory observation (2026-10-15)

The local corpus wrapper now executes the eight frozen journeys twice on each
path. It records timed CLI maximum resident bytes with application stderr kept
on a separate pipe, and samples service RSS after each query (not service peak
RSS). The timer owns a private process group; its timeout stops the application
beneath it too. Eight helper/comparison tests pass, including exact output/exit
status/literal arguments, measurement units and a real timeout cleanup
(`vrun-1b936d4e-8913-42ce-84e9-455ba0d8f35c`). The earlier three-test helper
snapshot also passes (`vrun-0052fec2-04f9-4984-a19b-4dc736d13faa`). Fixed build
jobs remain two and the wrapper-owned Node heap remains 512 MiB.

The fresh restored recipe snapshot builds a release CLI with SHA-256
`f63f4694ac17f9b39322245d2ba8e0d15d4aa4cbda29305af61c25ba51579899`.
All 32 executions pass: three Q3/five Q4 journeys, two rounds each, through
both direct and service paths. All five Q4 roots remain Found without issues;
Q3 retains only its declared flow/JSX exclusions. Paired response bytes match,
and a separate post-capture check confirms first/repeated bytes match in both
modes. All service requests retain generation 1. Stop succeeds and the process
exits in about 1.25 seconds on this capture.

Direct calls take 24.8–34.4 seconds and peak at 3,855,425,536–4,122,472,448
resident bytes, including full project loading. Service clients peak at
1,413,120–1,748,992 bytes; this does not include the daemon. Service samples
after the requests are 3,169,275,904–3,485,097,984 bytes. Warm service Q4 calls
are approximately one second. These are host observations on a 16 GiB machine,
not calibrated production caps or an isolated speedup experiment. Raw timer
output, load averages, source/build/tool/native hashes and all result bodies are
preserved in `.cc/temp/completion-next/corpus-resources-{direct,service}.json`,
`corpus-resources-build-inputs.json` and `corpus-resources-local-receipt.json`.
The older current captures were copied before replay into `corpus-resource-before/`.

The corpus registrations exist only in this worktree; canonical cctl discovery
still has the six older commands. This execution exercises the changed wrapper
under the project-setup wrapper-development exception and is local evidence,
not a registered Command Center gate. A new fixed one-process/512 MiB lexical
load-baseline profile is prepared and running against the same source-bound
binary. It checks the complete lexical empty-name result, repeats direct/service
responses and generation identity, and records/cleans up its separate service.
This separates shared project loading/retention from evaluator work before
selecting limits. The broad recipe library/Clippy replay is also running.
Logical evaluator usage, maximum unchecked operation, stack/drop/queue headroom,
remaining accounting and final full gates remain open.

The lexical load baseline completes all four direct/service selections on the
same release binary, preserving complete lexical coverage and identical empty
results. Direct peaks are 4,094,103,552 and 4,110,954,496 resident bytes, close to
the semantic direct peaks. Initial lexical loads take 26.7–28.2 seconds; the warm
lexical service request is about 1.04 seconds. Service samples after lexical
requests are 2,860,912,640 and 2,864,672,768 bytes, below the semantic samples.
This identifies project loading as the dominant direct-process peak, rather
than attributing all 4.1 GB to the evaluator. Samples and maxima are not heap
accounting and cannot simply be subtracted into an exact evaluator quota.
These baseline observations overlap the registered Rust replay; timings are not
isolated. The separate service stops successfully and exits. Full source,
binary/input/tool hashes, raw output and stop evidence are in
`.cc/temp/completion-next/corpus-load-baseline.json`.

The recipe full library passes (`vrun-9d89791e-73a3-450e-b36c-d427128a2272`).
All-target Clippy (`vrun-f17c67de-3809-46de-895d-5124867fa355`) exposes a large
CallArgument variant: Captured is now 8 bytes, whereas Synthetic is at least
241 bytes because TupleElementLabel.name has a Result carrying inline
DemandError eligibility data. The selected follow-up moves error plus optional
issue into the error's existing single box, keeping the property question in
the small handle. It adds no call-argument allocation and changes no sequencing,
localization, leaf eligibility or public error fields. Owned deep error clones
remain a separate open obligation. This is a mechanical storage/wiring change;
existing recovery and argument/tagged-call behavior checks validate it rather
than adding a test that merely asserts its private layout. Its scoped replay is
running. The earlier release/baseline measurements predate this layout follow-up.

## Logical usage diagnostic after the error layout follow-up (2026-10-15)

The private failure payload now occupies the existing error box; the property
question remains in its handle. Error/issue mutation and owned conversion still
use their original policies. An initial compatibility command named a nonexistent
test file and executed no tests (`vrun-2779f9ec-584b-4550-ace7-2d91d33a36da`);
the corrected nine recovery, call-argument and tagged-call targets pass
(`vrun-a4d54d6d-b47b-485d-aa75-68f79fc54dd9`). No behavior test was added merely
to mirror this mechanical private layout. The all-target Clippy replay is running.

A temporary test-only module collected the real corpus request ledger through
the registered single-resource-module profile. It used the existing worker and
request operations, a single shared loaded project and a 180-second diagnostic
deadline to permit debug execution. Production still uses its 30-second deadline.
Each public answer is compared exactly with the already native-validated release
capture. All eight match, and every logical depth returns to zero
(`vrun-c8119fad-bcf3-4d7a-a103-cf86cec9dd2e`). Project loading took about 225
seconds in this debug diagnostic. Type queries then took 2.7–5.9 seconds; member
queries took 13.5–47.9 seconds. These are not release latency observations.

| Query | Operation | Work units | Cumulative payload bytes | Peak logical depth |
| --- | --- | ---: | ---: | ---: |
| annotated-member | member | 146,586,678 | 950,079,081 | 154 |
| contextual-member | member | 146,591,302 | 950,079,672 | 154 |
| jsx-member | member | 94,750,339 | 297,969,636 | 84 |
| annotated-member | type | 20,798,782 | 69,678,137 | 52 |
| contextual-member | type | 21,655,123 | 74,156,671 | 65 |
| inferred-factory | type | 46,692,972 | 121,739,125 | 45 |
| jsx-member | type | 19,683,448 | 62,750,347 | 45 |
| predicate-callback | type | 26,353,849 | 104,099,601 | 61 |

The diagnostic records a 24-byte DemandError, 32-byte
NativeCompletion<TypeId>, 136-byte Signature, 88-byte TypeView and 8-byte captured
Expression handle on this build. These are private layout measurements, not
allocator metadata or public ABI promises. The probe's only instrumentation was
a temporary cfg(test) module and its module declaration; both are removed and
all original source hashes restored. Exact inputs, instrumentation/source/result
hashes and its receipt are in `.cc/temp/completion-next/corpus-logical-*`.

These counters are incomplete pending the remaining cache/loop/copy audit.
No production work, byte or depth cap was selected from them. Byte counts are
cumulative requested payload and logical frames are not native stack bytes.
Finite limits still require coverage, release validation and worst-case
unchecked-operation/stack/unwind/queue/teardown headroom. This diagnostic does
not close those delivery gates.

The error-layout follow-up passes all-target Clippy
(`vrun-0242523e-3cf6-4fdb-a4dc-016b7ee3f77b`). The failure-only issue/provenance
payload occupies the existing box; no extra call-argument or failure allocation
was added. Its variable deep clones remain open. The logical diagnostic above
already includes this smaller layout.

## Shared type-parameter names and controlled owned projections (2026-10-15)

Parameter snapshots now share a single immutable record containing their exact
decoded and first written names. Constraints, defaults, variance, Const and this
factory state remain in the by-value header. The existing
getDeclaredTypeOfTypeParameter model factory distinguishes owned ParameterData
from an existing header through a nonallocating input carrier, checks publication
of owned names and reuses existing name storage. Record data retain no controller
and have no Clone or mutable dereference. Fixed marker/this spellings copy through
the existing controlled text owner before record publication. Ordinary view/getter
clones no longer copy name buffers.

Five consumers require owned names (merged-infer validation, mapped headers,
summary output and full/skeleton graph records). Checked decoded/written copy
operations now own those projections. Existing signature/operator parameter
records and signature summary fragments use the same operations, including
preview paths without any added child acquisition. ParameterValue names and
other property/error/source/output copies remain separate obligations.

Eight compiler selections verify 16/64 actual named parameter constructions
(`vrun-649e3f03-6a3f-4e11-9722-8a8b38077fef`). The extended matrix passes 56
selections (`vrun-16db4c65-acf3-4f41-9243-dcd0415eb195`), covering all four
null/noCheck states and ASCII, Greek, supplementary, escaped and underscore
spellings; first written summaries and decoded graph fields stay distinct.
The first two publication assertions fail before production
(`vrun-1b287725-9160-4730-a586-b632388fac76`). The first prototype compile
identifies five owned moves (`vrun-60e437b6-612a-4b48-b22f-b668ad892f15`), not
behavioral RED. Temporarily restoring the original three implementation files
establishes RED for all six publication/public-copy assertions
(`vrun-2b0a7a9a-d2c5-467a-a4b7-801dbac67462`); exact prototype restoration
follows before adding checked owned projections.

The next run passes publication but exposes a test locator collision:
rfind(NamesParamΩ1) selects NamesParamΩ13
(`vrun-fec5716b-ce44-4cb7-b84d-931710f3ad99`). Using the exact declaration
suffix fixes that locator without changing production name rules. All six tests
then pass (`vrun-f489429b-0424-4f9d-9ba3-f3ee922f3fba`), including nominal
record/name work/storage minima, total under/at/over limits, flat record width,
all spelling variants, exact graph/summary fields and healthy successors.

Three independent controller-routing omissions each fail only their intended
pair: publication (`vrun-63377480-9b93-43a9-8d8f-65acdd9b1d73`), decoded output
(`vrun-f78663f1-645c-41a6-a9fc-f66be2ee41ab`) and written output
(`vrun-0ccc51ac-07b4-4ada-9621-b4a22411a8ca`). They do not establish separate
allocation-quantity or deadline bounds. Exact restoration passes all six
(`vrun-249c2f95-7856-4a59-956b-74fbac50923a`). Older parameter-name, signature
payload and source-recipe modules pass
(`vrun-d2f82176-b79b-464b-a1b8-18fb320adbd8`,
`vrun-6ed28fb4-6ed4-4e80-a9e5-5471474fb5fe`,
`vrun-f411c33e-fd3c-4187-af19-33431b0e734e`), alongside ten generic,
parameter/query, conditional, mapped, this and signature compatibility roots
(`vrun-ca58ad28-b835-4f09-8842-78250d90c74a`). Formatting passes
(`vrun-3066d182-ec9a-46a1-8585-5d83f10651ac`).

The proof driver has finished; all mutations are restored. Exact design choice,
original/intact files and native/proof/trial hashes are in
`.cc/temp/completion-next/parameter-payload-*`. Full library/Clippy are running
on the frozen combined snapshot. The earlier release/RSS/ledger captures predate
this name-sharing increment. Other cache/loop/copy accounting, finite production
limits and final whole-project/release/service/stack/teardown gates remain open.

The frozen parameter-name/failure-layout snapshot passes the complete library
(`vrun-c01826e7-de2f-408f-a84c-6b3188f0e0a2`, zero skips) and all-target Clippy
(`vrun-b4cd436d-bff6-40b5-83f5-95a8e93b4e07`). No source mutation occurred
between their input-hash checks. The next constraint-table increment is separate.

## Constraint table resource backing (2026-10-15)

The existing constraint owner now retains the request controller in all four
fixed-identity tables: parameter associations, raw constraints, resolved base
states and inferred membership. The latter is a unit-valued FixedMap; no new
set implementation is introduced. Raw absence, native safety/circular sentinels
and product failures stay distinct, and resolved-base publication preserves an
already installed property. Only backing storage changes; association Location
and cached recovery payload copies remain separate obligations.

The initial compiler matrix passes 24 selections
(`vrun-af52a66a-84ec-47a0-a08b-35b3be936121`). Eight resource assertions fail
before production (`vrun-038e0bb9-056e-435b-9d1e-813e3fedcbfc`). The first
prototype passes accounting but six assertions expose an unsuitable depth
control: a comma chain is a nested binary expression, with peak 23/71 at 16/64
(`vrun-5318d2a4-7047-44c8-ae64-7241aa51a503`). This is not a table-accounting
failure. Those original inputs/results are preserved in
`.cc/temp/completion-next/constraint-cache-initial-comma`.

Replacing that chain with a genuinely flat property list preserves the native
slot counts and number results. The revised 24 selections pass
(`vrun-444856dd-4df4-433b-afd0-31c176e73a84`), including exact 2695 diagnostics
for discarded object operands and no diagnostics for inferred aliases. The
original backing fails all eight final-query assertions on these inputs
(`vrun-19b589ff-f31e-423a-8597-9e2d6e7bfa91`); checked backing passes all eight
(`vrun-976ea525-6c85-4c96-9cb2-565e489dcf9c`). The matrix covers numeric,
absent and inferred constraints, 16/64 widths, null/noCheck options, work/storage
minima, total under/at/over limits, flat depth and healthy successors.

Each independent controller omission fails only its intended work/storage pair:
parameters (`vrun-27b1c8ce-feb4-44bf-8365-fd48afaa9e58`), bases
(`vrun-2e8d4b57-1058-4b10-bf39-78b5ba013993`), raw
(`vrun-600f1dd7-26e7-4e6b-86aa-13675db6bbd1`) and inferred
(`vrun-bdcd1534-a8ec-461e-b334-15a880c346dd`). These are controller-routing
proofs, not independent allocation-quantity or deadline proofs. Forced zero
fingerprints, with hash work retained, pass all eight resource tests
(`vrun-1189b967-6057-43b8-b408-218bce4a95ce`) and seven compatibility roots
(`vrun-1f7af51d-5612-432d-9ed7-b6e2f6220ca9`).

Exact restoration passes all eight (`vrun-e6d0a43a-ff5c-491d-b62a-5a9132e9ec5f`)
and the seven roots (`vrun-03d0e004-7cfa-4bb6-b2aa-de1bf376d30a`). Older
parameter-payload and promise-cache modules pass
(`vrun-af548967-8813-49c4-8098-2d001008f1b1`,
`vrun-03317b17-31ce-4ba1-be61-922b3fb86ca4`). Four existing pinned-native
base-constraint tests pass (`vrun-5633c7d2-51af-46a3-b319-bf8bf0b8cec4`),
including raw/base distinction, circular/error/this states and the 9/10/11,
50/51/52 and repeated-identity approximation boundaries. Formatting passes
(`vrun-7bc1dc09-c8bb-4e2f-a515-a74c17164606`).

The proof driver has ended and both production files are exactly restored to
the checked implementation. Native/proof inputs, design choice, original/intact
files and trial receipts are in `.cc/temp/completion-next/constraint-cache-*`.
The last full-library/Clippy snapshot precedes this migration. Shared collection
growth work/postchecks are the next scoped increment. Other copies/caches/loops,
finite production caps and final compatibility/release/service/stack/teardown
proof remain open.

## Shared collection growth work and completion checks (2026-10-15)

The existing sequence, map, set and String reservation operations now route
relocation through one private grow_storage owner. Before growth it debits the
prior live records (String counts stored UTF-8 bytes); after successful growth
it forces a deadline observation. Each existing capacity owner still selects
and charges requested payload before allocation. Allocation failure stays a
memory failure. No-controller loading behavior is preserved. No check runs in
Drop and no allocator, TLS state, scheduler or public tuning interface is added.

Set reservation/insertion now require the existing BoundedKey contract. Their
callers use TypeId or the parser FailedProbe record, whose identity consists of
fixed tags, offsets, grammar bits and booleans. No identity or parser rule changes.
Raw map callers with variable Eq/Hash remain a separate rehash-payload audit;
counting relocated records does not account for those key bytes. Sparse bucket
scans, maximum indivisible allocation/rehash latency and teardown still need
production headroom measurements.

The native replay passes 60 selections at
`vrun-b8a7ae87-cacc-41c2-8672-9d6dad2bd0d1`: twelve existing distinct-alias
controls and 48 new scalar-conditional controls. The latter use 16/64 ASCII,
Greek and supplementary prefixes before exact high/low sentinels, with all four
null/noCheck option states. The public UTF-16 projection attempts the valid
prefix before retaining the lone sentinel; that abandoned scratch still spends
work. No decoder, string identity or semantic result rule changes.

All four final-query growth tests fail before production
(`vrun-473070c8-961c-4905-959d-5c85e767ba8a`) and pass afterward
(`vrun-5ab2c2f1-0b97-45e0-b59d-91577d28cb94`). Each checks a conservative
prior-record/byte minimum, total under/at/over limits, flat depth and a healthy
successor. Independent zero-prior-record omissions retain allocation and pre/post
deadline checks, and each fails only its corresponding test: sequence
(`vrun-a0362a9d-109b-4a58-83ca-f5c4b716489d`), map
(`vrun-e1e12b8e-7082-42f4-9911-1f31a081dc3f`), set
(`vrun-a28f5584-21f3-455e-a728-f6346c56b317`) and String
(`vrun-284cfd7a-1bb0-4904-b502-7d3dbc6dc434`).

A separate wrong-unit trial counts String scalars instead of UTF-8 bytes. It
fails only the String test (`vrun-0846181d-449c-4be1-8b6f-a6623d6f08ed`): the
Greek prefix observes 14 units against a native-source-derived minimum 16.
This establishes byte-unit sensitivity beyond simply removing the controller.
These trials do not independently prove the necessity of a postcheck or a
real-time bound on the standard-library operation.

Exact restoration passes all four (`vrun-e460ac85-d472-43db-8aff-cbef823aa756`).
The older constraint, fixed-map, signature-payload and source-recipe modules
pass (`vrun-69790919-93d7-4589-a49d-97e04d057d8e`,
`vrun-7c9bbf5f-3a54-4101-bffe-1536ca55432e`,
`vrun-3b88359a-f2fa-477c-b74e-1452ee518745`,
`vrun-a6a8c547-e03f-428d-b3e6-37275d044bcd`). Six compatibility roots pass
(`vrun-ef956265-720c-42d9-81f0-a7fd6b81bd29`), as does formatting
(`vrun-407c53b9-d2de-43a4-9bfe-451496790288`). All proof mutations are
restored. Native/proof hashes, original/intact files and receipts are in
`.cc/temp/completion-next/collection-growth-*`.

The full-library/Clippy driver is running on the combined constraint/growth
snapshot, with Rust, Cargo and existing fixture input hashes checked at each
gate. Production quotas remain uncalibrated; the earlier corpus captures predate
these changes. The remaining cache/loop/copy audit, finite caps, final full
compatibility/harness/semantic/release and service/stack/teardown proof still
block delivery.

The combined constraint/growth snapshot passes the full library
(`vrun-7e6fb19d-56a8-4f84-8bf2-8c1769df7a33`, zero reported platform skips)
and all-target Clippy (`vrun-31a68cb9-0435-4328-ab9d-4f2cfe3f5511`). Rust,
Cargo and all existing fixture hashes remain unchanged throughout both gates.
The eight-query unique-symbol native matrix passes separately
(`vrun-c7af6ad4-5d94-4b7c-baa7-2cffeb91006c`), verifying distinct symbol and
node result sets before summary. Its two-table Rust migration is the next
increment; this broad verdict does not include that migration.

## Unique-symbol cache backing (2026-10-15)

UniqueSymbolsState now retains the request controller in both existing native
questions: symbol-owned unique types and UniqueKeyword type-node results.
Both use FixedMap. ExpressionRoot's existing FileId/Site identity implements
the bounded-key contract at its source owner. Uniqueness, validity, symbol-ID
observation, source spans and child acquisition remain with their original
operations. Public property-key copying and NativeCompletion recovery clones
are separate obligations.

The new native fixture forces 16/64 distinct unique declarations through computed
keys in a flat object, with a final number result. It does not reuse the older
opaque-key fixtures, whose widths vary one name's length. Eight fresh compiler
queries pass (`vrun-c7af6ad4-5d94-4b7c-baa7-2cffeb91006c`), across null/noCheck
profiles, with exact 2695 diagnostics, distinct result IDs and matching symbol/
type-node result sets. Observers read the fields installed by the existing
native call; no new semantic or ID acquisition is added.

All four work/storage assertions fail with original backing
(`vrun-e4a4ab48-db37-442e-9985-74cc7ec10241`) and pass with checked backing
(`vrun-f1e2ecea-fd8b-4d68-a61e-396a283ea633`). Each checks conservative slot
minima, total under/at/over limits, flat depth and healthy successors. Independent
symbol and node controller omissions each fail only their intended pair
(`vrun-4945fcc9-7536-4cc6-b095-8d68ccf831d8`,
`vrun-7093c34d-9a75-485f-914a-c46664fc1e84`). These establish routing
sensitivity, not allocation-layout or independent deadline bounds.

Forced zero fingerprints retain hash work and pass all four resource tests
(`vrun-4fff9702-fbdf-4a28-b7cd-a8e24bdd71b9`) and seven compatibility roots
(`vrun-f1d3d5cf-51d5-41f3-8780-418354cf2039`). Exact restoration passes all
four (`vrun-a34d3305-96ee-4c1e-9410-7289b287726d`) and the seven roots
(`vrun-1a686685-b767-4bca-9650-2fc99932716e`). Older growth and constraint
modules pass (`vrun-cd2d4a27-3173-41f7-94e2-d032c1db19d6`,
`vrun-b5db636b-3b49-419a-b208-a2c058b517ca`), alongside two native known-symbol/
symbol-identity harnesses (`vrun-79b99528-dda0-4324-867d-0ad557007674`) and
formatting (`vrun-34c2fd1f-db0d-4b63-a0e4-52bc31e102fa`).

The proof driver has ended, with all mutations exactly restored. Native/proof
inputs, rationale, original/intact files and receipts are in
`.cc/temp/completion-next/unique-cache-*`. Full library/Clippy are running on
the frozen unique-cache snapshot. A resolved-call cache candidate is prepared
but requires native controls before any Rust installation. Other resource
coverage, finite production caps and final compatibility/release/service/stack/
teardown proof remain open.

The frozen unique-cache snapshot passes the full library
(`vrun-96e4bacb-e499-46df-90cd-76d60df434ac`) and all-target Clippy
(`vrun-a9436327-0d9d-456a-a40e-2a43fd8c963f`). The native resolved-call
matrix passes eight fresh queries (`vrun-52c05162-131d-4a0a-9185-2b81a6220e32`),
with 16/64 distinct healthy installed call-site properties. Its separate Rust
migration now passes two final-query tests after RED; those changes are not
included in this broad unique-cache verdict.

## Resolved-call cache backing (2026-10-15)

The existing getResolvedSignature owner now uses checked backing for the actual
CallRoot property. Its resolving, installed, failed, contextual save/restore and
deferred-retirement decisions remain in place. CallRoot's existing file/span/
kind identity implements BoundedKey at its source owner. FixedMap's new remove
adapter prepares that same key and delegates to the established CheckedMap
removal operation. The existing optional any/silent-never signatures stay lazy.
Cached recovery copies and last-child/abandoned teardown remain separate debts.

Eight native queries pass (`vrun-52c05162-131d-4a0a-9185-2b81a6220e32`),
covering 16/64 distinct installed healthy call sites and all null/noCheck states.
The observer reads existing signatureLinks after the native call without
acquiring another signature, ID or checker question. Two final-query resource
assertions fail before production (`vrun-6514c6c8-ecc9-4d05-af31-41b84f5a6d42`)
and pass afterward (`vrun-ac23e9cf-a673-4f0f-900f-24bbf27ba6ad`), checking
slot minima, under/at/over quotas, flat depth and healthy successors.

Removing the cache controller fails both intended tests
(`vrun-ce5f28c3-e071-4430-a529-025854fed472`). This proves controller routing,
not separate allocation-layout, removal-specific quantity or deadline bounds.
Forced zero fingerprints preserve hash work and pass both resource tests
(`vrun-4b6fd253-929f-4dca-a8a0-863c8e394569`) and eight call/new/tagged/
optional/cycle/contextual/recovery/generic compatibility roots
(`vrun-79889692-d9c0-41b0-bef7-2203ab42ff8d`). Exact restoration passes both
resource tests (`vrun-a904acf3-ea3a-4367-bdee-0e310f3992c8`) and all eight
roots (`vrun-aa9dc3d9-bc06-48fe-af8d-474651120082`).

Older unique-cache and collection-growth modules pass
(`vrun-b1449fe8-bf95-4f6d-8e3e-f5c75da91928`,
`vrun-ad22acfe-6e73-49e4-b942-8ba2097c729b`), alongside two native call/
recovery harnesses (`vrun-45a3d6be-6e21-4f54-8911-c35511ba4649`) and formatting
(`vrun-026cef7d-bd0f-4e42-8727-20e7d2a78785`). The proof driver has ended
and both mutated files are exactly restored. Inputs, rationale, original/intact
files and receipts are in `.cc/temp/completion-next/call-cache-*`.

The latest full-library/Clippy snapshot precedes this migration. The next scoped
increment controls owned public key text/private origins; 48 independent native
inventory selections pass before its Rust tests. Other coverage, finite caps and
final full-project/release/service/stack/teardown gates remain open.

## Controlled public key projections (2026-10-15)

The existing public property-key projection now copies fixed/private text through
one private operation using checked byte storage. Exact String::from_utf8
admission and the original Unsupported messages remain; validation has explicit
work and a completion check. No replacement, normalization or child acquisition
is introduced. Private retained origins reuse the existing checked location/file
copier, generalized from signature-only naming. Its signature observation stays
at the existing consumer so the older proof retains the same question boundary.

Forty-eight fresh native inventories pass
(`vrun-4556699c-85b2-4a83-8b57-a8523a604787`): fixed/private keys, 16/64 widths,
ASCII/Greek/supplementary text and four null/noCheck states, including 17/65
private overload declarations. Four final-graph tests fail at the intended quota
boundary before production (`vrun-d587df87-c200-4d7d-8bc3-6bed338643a4`) and
pass after it (`vrun-e5b2d6ad-5f5d-4412-b9f4-49c2abca936e`). They assert exact
names, kinds, source files and UTF-16 name ranges; native-derived copy/record
minima; total under/at/over limits; flat depth and healthy successors. The origin
minimum covers retained record slots, not a separate filename-byte/RSS bound.

Omitting only text-copy routing retains UTF-8 validation work/postcheck and
Unsupported handling, but fails just the two text assertions
(`vrun-98cde6ea-31ff-4226-ba36-b2fd5dd701e0`). Restoring the old origins.to_vec
fails only the two origin assertions
(`vrun-64ad8eee-ea60-40d2-b7de-e5b44dd4b8e4`). Exact restoration passes all
four (`vrun-0e13706e-63c7-4133-9622-4f36f3e42ad7`). These establish scoped
routing, not independent UTF-8-validation deadline or allocator bounds.

Older signature-payload and call-cache modules pass
(`vrun-8a684fb1-45b6-430a-b6ef-785441502900`,
`vrun-ade48ede-a4a4-4407-a0a3-be83ad7390d8`), as do seven private/member/value/
property-flag/presentation/label/signature compatibility roots
(`vrun-1083657c-2a4b-456b-8ede-8e210896e8f7`), two native private/signature
harnesses (`vrun-3e07a84d-d4af-4c14-83ce-a079bf0af0fd`) and formatting
(`vrun-5462d244-cef8-4169-b718-3399872ac352`). All mutations are restored;
exact inputs and receipts are in `.cc/temp/completion-next/key-projection-*`.
Full library/Clippy are running on the frozen combined call/key-projection code.

A subsequent read-only review finds one Property construction site and no key/
origin mutations, while ordinary Property clones and the renderer's preview/
actual copies still clone those variable buffers. A shared immutable metadata
record with explicit owned projections is prepared, not installed. Its 72 native
inventories pass (`vrun-87a274a8-deb0-4ba5-b431-9c418167bf84`), adding symbol
keys and their distinct origins. Other coverage, finite production calibration
and final full-project/release/service/stack/teardown gates remain open.

The combined resolved-call/public-key-copy snapshot passes the full library
(`vrun-f3f60686-cc4e-4fea-9f50-2d83147260e6`, zero reported platform skips).
All-target Clippy follows on the same Rust/Cargo/existing-fixture hashes.
The immutable Property metadata draft and its eight tests remain unapplied.
Production work, byte and depth caps remain unset while coverage and final
release/service/stack/teardown obligations are open.

The frozen combined resolved-call/public-key-copy snapshot passes all-target
Clippy (`vrun-c9f208b3-926f-4238-8817-560dad1b0a3b`) after the full library
pass. Rust/Cargo/existing-fixture hashes remain unchanged across both gates.
The separate immutable property-metadata installer then establishes all eight
intended RED assertions (`vrun-c13bce13-e8fc-4f65-9e95-084813342196`) and applies
the prototype. Its first GREEN/compatibility checks are not yet complete.

## Immutable property metadata and controlled output (2026-10-15)

Property now shares finalized TypeKey/origin metadata in one immutable Arc
record. Its symbol/type/display/flag header remains by value; widening keeps
mutating only read_type/display. The sole project_property_symbol producer uses
owned PropertyData. Metadata have no Clone, mutable dereference or controller,
so ordinary Property/inventory clones no longer copy key strings/origin arrays.
The renderer's existing preview/actual closure captures the operation controller
and performs explicit owned key-text, key-origin and property-origin snapshots.
The retained-location copier is the same associated operation for all consumers.

Seventy-two native metadata selections pass
(`vrun-87a274a8-deb0-4ba5-b431-9c418167bf84`): fixed/private/symbol keys, 16/64
widths, ASCII/Greek/supplementary text and null/noCheck states, with distinct
symbol origins and 17/65 private declarations. All eight final-graph quota
assertions fail before production (`vrun-c13bce13-e8fc-4f65-9e95-084813342196`).
The first prototype compiles, but passes 7/8
(`vrun-88cd15d9-4836-4e5f-9696-690bd420ea49`): its work minimum wrongly counts
UTF-8 bytes for resources::push_text, whose established work unit is a Unicode
scalar. This is a test-unit correction, not a new production debit or semantic
change. Storage retains byte units; acquisition's copy_slice remains byte work.

With that correction all eight pass
(`vrun-1470c7fb-a129-4f75-8ba9-df56d110f986`), asserting exact key/name/origin
values and ranges, conservative record/scalar/byte minima, total under/at/over
limits, flat depth and healthy successors. Four independent routing omissions
each fail only their intended pair: record publication
(`vrun-6ccd4336-9578-4b09-b7a8-881f3a20931e`), key text
(`vrun-3272e352-9a32-4bd5-a663-9b4f610393f1`), key origins
(`vrun-7c38f60d-1c3d-4490-8290-89c88c9037dd`) and property origins
(`vrun-7c024fd6-d384-4dbb-91f3-48504d40eced`). They do not establish Arc
allocator overhead, independent allocation quantity or deadline headroom.

Exact restoration passes all eight
(`vrun-251a6254-c16a-4b5f-b4b5-154635c3801e`). Older key-projection,
signature-payload and call-cache modules pass
(`vrun-273a3fae-dccc-4590-9592-c84fd58d5e21`,
`vrun-6f797e4d-ec45-4eb6-b466-53f926c8c0fb`,
`vrun-6de72633-e0c8-4857-ad8b-17a4672f6767`), alongside nine compatibility
roots (`vrun-a5f25c9e-0807-4b02-b8cd-9b2ec83fb9aa`), native private/property-
flag harnesses (`vrun-23272bf5-4601-4d71-9a8f-36d0f315bfce`) and formatting
(`vrun-9cd21301-268c-45fd-9f6a-05afba371136`). All mutations are restored.
Exact inputs, original/intact files and receipts are in
`.cc/temp/completion-next/property-metadata-*`.

Full library/Clippy are running on the frozen metadata snapshot. Initial
origin/declaration acquisition, renderer graph/checkpoint/public-output clones,
recovery payloads and other cache/loop coverage remain separate obligations.
Finite production caps and final compatibility/release/service/stack/teardown
gates remain open. The next prepared candidate is four existing signature
association tables, subject to fresh native controls before Rust installation.

The frozen property-metadata snapshot passes the full library
(`vrun-33af0ea4-dd4c-437d-acb9-47de457acfa4`, zero reported skips).
All-target Clippy is still running as `vrun-e866d485-164a-4cd6-ad51-2f74f58b40c5`.

The first signature-association hook stops at an exact anchor mismatch
(`vrun-58a17a0f-aa65-41ac-828c-375c25fd4551`), before checking any compiler
result: the existing return observer already names its result. Reusing that
actual overlaid header preserves both observations. The corrected native gate
passes eight selections (`vrun-9bae8055-4001-47ba-8143-3e2ea920b21e`), checking
16/64 distinct declaration factories, their actual declarations and number return
slots under all null/noCheck states. Initial failed inputs remain archived.
Signature-association Rust installation is gated on the pending Clippy pass.

Read-only position review also corrects a suspected cost: SourceText::position
already uses UTF-16 checkpoints every 256 bytes plus line-start binary search.
It does not scan an unbounded full line. Preserve this owner; path copying and
consistent controlled declaration-anchor acquisition are the separate open work.

## Signature declaration and return association storage (2026-10-15)

Four existing SignaturesState tables now use FixedMap: source declaration
factories, declaration associations, return properties and reverse deferred-return
links. Their separate native questions, lazy return-state observations and
composite eager-return removal/replacement remain unchanged. The adapter gains
contains_key through get; SignatureRoot, SignatureId and DemandId identify only
fixed file/span/role/index fields. Cached Location and recovery copies remain
separate from backing storage.

Eight pinned native selections pass (`vrun-9bae8055-4001-47ba-8143-3e2ea920b21e`)
after the documented oracle anchor repair. All eight final-query quota assertions
fail before production (`vrun-e5beba4a-369f-4237-b8c9-7389646957f3`) and pass
afterward (`vrun-7b663ccb-e839-4d93-b66b-9cefba2a2ded`). They exercise 16/64
declarations, null/noCheck profiles, conservative work/storage minima, exact
under/at/over totals, flat depth and healthy successors. Four independent
controller omissions each fail exactly their intended work/storage pair:
`vrun-59c0fbe2-7bdb-4032-9fbd-b06c455e24b9`,
`vrun-9986aa9f-ca8a-4c0f-a4c8-f41aef1eed3b`,
`vrun-420219b1-3966-4324-bd7a-607adf960be5`, and
`vrun-44514ce1-47d2-40f6-862c-a903efb26457`.

Forced equal fingerprints retain hash work and pass the eight quota tests
(`vrun-9173cf58-6c30-40b0-b6dd-e111088badea`) plus ten signature/composite/return/
call compatibility targets (`vrun-9d240a45-7beb-45f1-a285-b5fdcc354180`). Exact
restoration passes all eight (`vrun-9f200c1d-6328-490d-a131-5dee6bbc808c`) and
the ordinary-hash ten-target scope (`vrun-3f355e7c-33a6-4417-b517-2c83e5586f67`).
Older signature-payload, property-metadata and call-cache modules pass
(`vrun-2ef2a245-8804-4ed3-b114-0594cf0f9111`,
`vrun-5a8ca0b2-4db5-41cd-bde8-0fb685b748d4`,
`vrun-40b02c06-543c-4c24-b8a6-665e03ad9600`), as do native signature/composite/
overload controls (`vrun-38bd5abf-c2aa-478e-9bd1-581966e20d6a`) and formatting
(`vrun-b8431f00-78b3-4276-b588-1dc63c0702de`). The proof has ended with all
mutations restored; exact files/hashes/receipts are in
`.cc/temp/completion-next/signature-association-*`.

The preceding property-metadata snapshot also passes full library
`vrun-33af0ea4-dd4c-437d-acb9-47de457acfa4` and all-target Clippy
`vrun-e866d485-164a-4cd6-ad51-2f74f58b40c5`. Those broad gates predate this
signature increment. The next native probes cover six structured-type tables
and the minimum-arity slot; no Rust change for either is installed yet. Related
scoped increments will share the next broad gate. Other accounting coverage,
finite production caps and final integration/release/service/stack/teardown
acceptance remain open.

## Structured-type cache backing (2026-10-15)

Six StructureState tables now use the same checked fixed-key owner: member
inventories, independently installed raw tables, apparent intersections, generic
flags, reduced types and regular object literals. Evaluator passes its existing
controller into StructureState::new; native flags/publication points, resolving/
failed states, nil/present table distinction and child payloads are unchanged.

Eight native selections pass (`vrun-47925281-786a-4e47-9a57-207a1cca8612`).
For widths N=16/64, each profile observes 8N+1 resolved member inventories,
2N+1 nil and 6N present raw tables, and N installed slots in each of the four
computed caches. Observers read existing fields/IDs without obtaining another
checker result. Final results remain number, with only diagnostic 2695.

All twelve intended quota assertions fail before production
(`vrun-32f051fe-ff28-470a-afea-b03bda8d2c46`) and pass after it
(`vrun-6ffba0ef-5f79-4b06-b2ae-44b133d60589`). They use conservative N minima,
exact total under/at/over limits, null/noCheck states, flat peak depth and healthy
successors. Six controller omissions each fail exactly their two intended
assertions: members (`vrun-8ba20799-c3f6-4e07-8580-8503589d0254`), raw tables
(`vrun-a6c16cdc-2279-4bb1-9b32-e5b1f83e07e2`), apparent intersections
(`vrun-a9d6bc08-b871-42ee-aecb-f636e22bd57b`), generic flags
(`vrun-0ff84a65-ff1c-4b36-ae6b-8c0236a51762`), reductions
(`vrun-b502918e-71cd-4426-8185-936f65e04c15`) and regular objects
(`vrun-ca0ab25e-b882-47c4-b343-10afbddeb240`). This proves route sensitivity,
not exact allocator/RSS quantities or independent completion-check headroom.

Forced collisions pass twelve quota tests
(`vrun-8c9f2966-d261-4672-a5d8-d4216d8856ce`) and four relevant compatibility
roots (`vrun-2b41a31b-e256-499e-80ef-55109f5908a8`). Exact restoration passes
the twelve (`vrun-8f0bd71f-caee-4012-b4ad-dc1d4222d231`) and ten normal
compatibility roots (`vrun-00ce6732-fd89-4839-89c6-e8a58b4e40fb`). Older
signature-association and metadata modules pass
(`vrun-6dcc608d-43da-4db0-9a22-4a96cce040b3`,
`vrun-14cb56db-3554-4689-89aa-0c6686a1970b`), alongside native apparent/
composite/structured-relation controls
(`vrun-5db6c88d-354d-414c-9b63-7e3ba4549a25`) and formatting
(`vrun-8d1bf217-79eb-4ead-95de-21021f3d03cb`). The proof is complete and
all mutations are restored. Exact inputs, snapshots and receipts are in
`.cc/temp/completion-next/structure-cache-*`.

The separate minimum-arity native probe also passes eight selections
(`vrun-89cc51b2-3e86-4d44-92f1-0533dc2478d9`); its Rust increment is active.
A single frozen full-library/Clippy sequence is queued after that proof for the
combined eleven-table batch. These remain scoped accounting increments; other
copy/cache/loop/recursion coverage, production calibration and final delivery
gates remain open.

## Cached minimum argument count (2026-10-15)

SignaturesState.minimum_arguments now uses the existing fixed-key backing,
with its request controller. getMinArgumentCountEx still bypasses both cache
lookup and publication for syntactic-only VoidIsNonOptional mode. No native
arity, rest/tuple optionality, signature identity or completion policy changes.

Eight native selections establish N distinct installed zero-minimum slots at
N=16/64 under null/noCheck states (`vrun-89cc51b2-3e86-4d44-92f1-0533dc2478d9`).
The two intended quota REDs reproduce
(`vrun-b8f9e57e-4cd7-42e1-88a2-114437a8c7d9`); GREEN passes
(`vrun-50a4cbcc-a7ca-4f93-8440-8cba7e28c4a4`). They cover conservative slot
work/storage minima, total under/at/over limits, flat width and healthy
successors. Controller omission fails both
(`vrun-865bc6e7-3f46-4fff-8f4c-678a42e1fc45`), and exact restoration passes
(`vrun-1c1168a4-8225-43a3-8016-67988c67e802`).

Older association and structure suites pass
(`vrun-b982d305-e307-47c1-89bb-9c8c78b3c550`,
`vrun-1d706e01-e12f-4069-bd7d-4db00a7df39d`), as do eight call/signature
compatibility targets (`vrun-5ddb3735-2c79-4dec-bfbe-c9fd3b612757`), native
call/structured-signature controls (`vrun-42779568-e2ce-4325-b812-5d5f91a07287`)
and formatting (`vrun-5bc6c07d-f41f-4f55-9d4f-55b257989c4b`). The proof is
complete; omission restored; exact files/receipts are in
`.cc/temp/completion-next/signature-arity-*`.

All eleven signature/structured/arity tables have completed scoped evidence.
Full library `vrun-521ec97c-cd5f-4bc8-9f49-f71032f37ca7` is running on frozen
Rust/Cargo/fixture hashes, followed by Clippy only if it passes. No source
mutation is queued. Remaining accounting, calibrated production limits and final
all-project/release/service/stack/teardown acceptance are not closed.

The frozen eleven-table batch passes the full library
(`vrun-521ec97c-cd5f-4bc8-9f49-f71032f37ca7`, zero reported skips) and
all-target Clippy (`vrun-82b4f9c1-3cc8-4bc0-aff1-09ffd464ca79`). All Rust,
Cargo and existing/new fixture hashes are unchanged across both gates. The
library receipt arrived about 853 seconds after submission, within the
registered 900-second limit; no test partition, timeout override or registry
change was needed. This is a completed intermediate broad checkpoint, not
final whole-evaluator or release/service/stack acceptance.

The next source-anchor increment is active: one final-graph work test reuses
the 72 native metadata cases before any production edit. All seven identified
Source paths will forward their captured capability through the existing
binding/AST/lexer owner only after that intended RED. File/location copies,
origin collection, other cache/loop coverage and production calibration remain
separate work.

## Controlled declaration-name anchor acquisition (2026-10-15)

Seven Source operations previously rescanned declaration anchors through the
uncontrolled binding entry. CapturedSyntax now supplies its retained optional
controller to the existing declaration_anchor_controlled/getFirstToken/lexer
owner. All seven routes use it. Loading's None path and attachment construction's
already-controlled choice remain unchanged; no lexical/name/span rule is copied.
Denied infallible span acquisition uses the existing private request abort.

One new final-graph work test reuses all 72 native metadata controls: fixed,
private and symbol keys; 16/64 widths; ASCII/Greek/supplementary text; null/noCheck
states. It preserves exact names/files/UTF-16 ranges, tests conservative token
scan minima and total under/at/over limits, flat depth and healthy successors.
It is the sole RED on old code (`vrun-016d0557-a4f7-49d1-830b-1659b9e95461`);
the older eight pass. All nine pass after production
(`vrun-e62981c6-a0da-42d7-a028-079830c920de`). Removing only controller
forwarding, while retaining observation, fails only the new assertion
(`vrun-9199df66-8fb8-41e5-86f5-b1fc7c7b707c`). Exact restoration passes
(`vrun-af7e745c-b8f3-4f79-b642-65ef948727ee`).

Older parameter, signature and constraint resource suites pass
(`vrun-fb44cd86-b044-4680-bf8e-166674096f00`,
`vrun-98bbcdea-d20d-480e-97dc-72570f5d1cd9`,
`vrun-42cd081d-5f8e-4238-b0d0-f236ced2aeb4`). Eleven compatibility targets
pass (`vrun-0b4cb16e-2196-41d9-aa78-9ae81aca1195`), alongside metadata/private/
signature native harnesses (`vrun-23d4cebc-9762-4692-9553-69bbc23dedb1`) and
format (`vrun-9f1310f5-e355-497e-be51-c874ede1723c`). The proof is complete
and its sole mutation restored. Exact inputs and receipts are in
`.cc/temp/completion-next/source-anchor-*`. The preceding eleven-table full
library/Clippy checkpoint predates this increment.

The next ordered-origin collection work has 32 native-derived inventories,
including repeated declarations at one site, identical coordinates in distinct
files, synthetic empty origins and 16/64 overloads. A four-test Rust RED is active
before changing its collection/comparison/storage owner. This does not yet
close location/file copying, other coverage, calibration or final release gates.

## Ordered origin projection and exact location comparison (2026-10-15)

Property metadata now delegates its existing declaration question to the shared
reference_symbol_origins owner. One loop retains first-seen location order,
with separately checked invocation/traversal work, candidate comparison, exact
filename byte equality and vector retention. Coordinate-first comparison keeps
the same exact equality predicate. Sorted named-declaration metadata remains
a different operation. Declaration-array and source-file copying are separate
coverage obligations.

The pinned native harness establishes 32 inventories under null/noCheck states,
including two generic declarations at one site, their materialized mapped
property, synthetic empty declarations, tied coordinates in distinct files and
16/64 overloads. The source-derived expectation file pins names, order and
ranges (`origin-collection-native-pinned.json`); earlier exploratory captures
remain preserved. All four intended quota REDs reproduce
(`vrun-4796cec4-5978-4285-aad6-442ee0b55c5d`) and all four GREEN pass
(`vrun-4f8e986f-4cb7-4f7c-8cfb-8a3b9eaa6ce0`). They verify final metadata,
conservative debit minima, exact under/at/over limits, flat overload depth and
healthy successors.

Independent debit omissions each fail only their intended test: traversal
(`vrun-89be82c3-df90-4b2f-a68f-272166b938e5`), scalar comparison
(`vrun-f3191c10-ba4e-4743-8763-de9881b58c71`), filename comparison
(`vrun-24738575-6e37-47c2-8f92-64655618da4b`) and retention
(`vrun-ec41305d-a04e-45e0-90a6-8685457ee934`). Equality, data and declaration
order remain intact during these mutations. Exact restoration passes
(`vrun-4dd53a98-9176-4601-bb31-704856bec285`), alongside metadata/anchor and
signature-origin suites (`vrun-9bdc8f63-d741-4341-b484-385e9b39db0e`,
`vrun-5e609d5a-a162-4c00-8bc3-5631189a0e23`), eight compatibility targets
(`vrun-9a7a7a2e-1d02-406f-8c1c-5544922f4233`), three native harnesses
(`vrun-d3b8f586-bb9a-4f84-8a5b-6ac1811e667e`) and format
(`vrun-9c059171-1b55-4881-a9c9-16b4431a5570`). The proof is complete and all
mutations are restored. Exact sources/hashes/receipts are in
`.cc/temp/completion-next/origin-collection-*`.

Full library `vrun-3dbd914b-3a5e-462e-b4d5-ba06d793010c` is running on frozen
Rust/Cargo/fixture inputs for the combined anchor/origin snapshot, followed by
Clippy if it passes. No later source mutation is queued. This is intermediate
accounting coverage: source/recovery/output copies, remaining cache/loop/
recursion coverage, deterministic production caps and final all-project/release/
service/stack/teardown acceptance remain open.

## Combined source-anchor/origin broad checkpoint closed

The exact frozen Rust, Cargo and fixture snapshot passes the full library
(`vrun-3dbd914b-3a5e-462e-b4d5-ba06d793010c`) and all-target Clippy
(`vrun-74e32326-8f74-4db9-9301-5db928a61571`). The driver verifies unchanged
input hashes across both gates; source-projection-batch-broad-complete.json
records successful completion. This checkpoint predates the next symbol-cache
change and does not establish production calibration or final release/service/
stack/teardown acceptance.

Eight passive native symbol-cache selections also pass
(`vrun-2eb081d5-63f9-4775-ac4e-c261d99532e3`), establishing exactly 16/64
installed value and declared-type slots under each null/noCheck state. The
corresponding four Rust quota REDs are now running before migration.

## Value and declared-type cache backing closed as a scoped increment

Evaluator's value_types and declared_types now use the existing FixedMap owner
under the same request control. BoundSymbol's fixed native header ID has its
bounded-key marker at the binding owner; NativeSymbolIdentity reuses its
existing marker. Resolving, installed and failed state/publication behavior is
unchanged. Read and write queries remain separate; two former shared-table
conditionals now select the existing operation instead of requiring the same
container type. Cached recovery/location payload copies remain separate work.

Eight native selections establish exact 16/64 installed slots per table
(`vrun-2eb081d5-63f9-4775-ac4e-c261d99532e3`). Four final-query REDs reproduce
(`vrun-f190db4f-f070-4522-bf23-db9abd6d7b67`), followed by GREEN
(`vrun-d61593a1-f33f-4fa2-9b4a-bc6bd0472447`). Independent controller omissions
fail only their intended pair (`vrun-ad5f5665-aa33-4fd6-b3af-7f1b133bef73`,
`vrun-0fc3522d-5053-4007-af61-a85d8303b93e`). Forced-zero fingerprints pass
all four resource tests and four identity/recovery compatibility roots
(`vrun-1201cbc7-1462-4003-94ff-6f53d712ef0b`,
`vrun-8d1238d3-815c-4ef2-9ea5-5391cfb4e802`). Exact restoration passes
(`vrun-903e37b6-56cc-4e20-89bf-1adacafe551a`), alongside prior structure/origin
suites, eight normal compatibility roots
(`vrun-e8a6576e-c2f0-4e44-b211-a543614c07df`), native controls
(`vrun-0f740923-4000-4381-bfbd-7c60e18cf301`) and format
(`vrun-f1d2d482-fddc-4733-93b3-593c75ae9422`). No omissions remain active;
exact inputs and restoration manifests are in symbol-type-cache-*.

The next source-location filename-copy increment has 32 passing pinned Unicode
inventories (`vrun-16312923-2980-4430-a39c-283c65428abc`); its two Rust quota
REDs are active before source changes. Full library/Clippy still refer to the
preceding anchor/origin snapshot. Other accounting, finite caps and final
release/service/stack/teardown gates remain open.

## Initial source-location filename-copy checkpoint

The existing location factory now copies exact display names through checked
text append under CheckerInputs' borrowed request capability. Loading explicitly
has None; Evaluator and Source derive the same controller from the input view
and no longer accept a redundant constructor argument. Selection/reference
helpers reuse that view. SourceText still owns position conversion unchanged.
The request capability is not attached to retained binder identities. This
closes initial factory copies, not every later Location/path/error/renderer copy.

Two final metadata/quota REDs reproduce
(`vrun-a46ac941-d4a8-448d-aac4-9da385b72f72`), then GREEN
(`vrun-bfe86581-2d78-4b45-9171-db8939ddfca1`). Forwarding omission retains
exact copy/observation and fails only the two tests
(`vrun-fa584c37-881c-4c29-8138-88fb5e9cd470`); restoration passes
(`vrun-501d2d84-237f-4402-921c-e822854e5175`). Older origin, metadata/anchor and
symbol-cache suites pass, as do nine compatibility roots
(`vrun-0b461267-e3c6-4aed-98d1-4debccf1c2ca`), native origin/filename controls
(`vrun-a3cc3f1b-9a08-48ab-b832-922421ba271e`) and format
(`vrun-a1003a83-2563-45a7-8129-6b3e1487f992`). The tests assert final exact
native metadata, conservative scalar/byte debits, under/at/over quotas, flat
16/64 depth and healthy successors in four null/noCheck states.

The current finish-location-utf8-coverage.py phase extends the query filename
with a three-byte BMP scalar so all UTF-8 widths1/2/3/4 occur, while preserving
the initial pinned artifacts under location-file-initial/. It also removes an
obsolete private selection parameter exposed by the input-view change. Its
fresh native/resource/omission/selection gates are pending; no full-library or
Clippy claim yet applies to these symbol/location increments. Whole coverage,
finite production caps and final delivery gates remain open.

## Exact source filenames across UTF-8 widths

The updated pinned native filename matrix passes
`vrun-a5bfd49e-7516-46ca-9d52-65a200350e24`. Its query filename now contains
a three-byte BMP scalar in addition to ASCII, two-byte BMP and four-byte
astral scalars. The initial fixtures, harness, Rust test and native artifacts
are preserved in location-file-initial/; updated exact inputs are in
location-utf8-native-input-hashes.json. The same two final-query regressions pass
(`vrun-ee3856b5-aebf-479f-a3a5-bdfb360731af`), with sole intended forwarding
omission (`vrun-d0d19608-7199-42da-9521-ca5d41d3d31a`) and exact restoration
(`vrun-92a70480-7da2-4d37-a187-a857d97eb8c4`). Four selection compatibility
roots pass after removing the now-unused private candidate project argument
(`vrun-eb29e5bd-29db-453a-93fe-8c8299e3bb12`); both native filename/origin
harnesses and format pass (`vrun-4bcdb9ff-5ac8-4bab-86cf-6d27556c6ef5`,
`vrun-3e53882f-0bca-44a0-ae41-598c76b52146`). The earlier nine-root compatibility
and prior resource receipts still apply to the unchanged production copy policy.

The combined symbol-cache/source-location full-library and Clippy driver is now
active on frozen Rust, Cargo and fixture contents and inventory. No subsequent
Rust mutation is queued. This is another intermediate coverage checkpoint;
remaining copies, caches, loops and recursion, concrete production budgets and
final all-project/release/service/stack/teardown proof remain open.

## Broad library timeout and bounded coverage replay

The combined library invocation timed out at the registered server limit
(`vrun-793fd49b-8aac-44c9-9e23-2e3fa25b3da5`,900seconds). It is not a pass;
its empty terminal output cannot establish complete coverage. Clippy was not
started by that failed driver. Earlier partial live-log observations are not
counted as a completed gate. Rust/Cargo/fixture inputs remained unchanged.

The canonical registered runner supports a resource parent/module filter but
no non-resource-only filter. A narrow built-binary --list diagnostic (not test
execution, which stays registered) inventories577 test names:139non-resource,
106direct resource and332in47resource child modules. One pre-existing ignored
program/corpus diagnostic requires TSPARSE_PROGRAM_CONFIG/LISTING; active count
is576. The bounded replay temporarily gates only test-module declarations for
the first two partitions; every production statement and assertion body stays
unchanged. It restores both exact files in finally before running all47 child
scopes against the complete source. Expected and actual binary name inventories
plus per-partition receipts are recorded in library-partitions/. After full
coverage, Clippy runs on the original restored snapshot. No runner limit, worker
profile, canonical registry or test assertion is changed.

This replay is active; none of its pending partitions is a claimed pass. The
next header-array implementation remains a draft and is not queued. Other
accounting/calibration/final delivery gates remain open.

## Literal-cache regression exposed by complete partition replay

The partition replay passed the139non-resource-name partition (one existing
ignored corpus diagnostic),106direct-resource cases and18child modules before
the literal-cache scope failed (`vrun-c9773bea-ea6c-4ac6-80f2-a19b660d93b9`).
It is incomplete; Clippy did not run. Both temporary module-selector files are
exactly restored. Original coverage and failed receipt remain in library-partitions/.
A second unchanged replay reproduces the same failure.

Measurement-only diagnostics identify a stale subtraction
(`vrun-e411de06-9f7f-48ea-8c6d-c2fdd7accafd`): narrow chosen-minus-cheap work
is-77for ASCII/template after the new source-copy accounting, so the old cutoff
is77units below the wide cheap query itself. Unicode/surrogate deltas are
positive. This does not establish nondeterministic production accounting.
The temporary diagnostic was removed without changing the common growth test.

The four existing literal-cache regressions now observe only literal-key byte
hashing. Their minimum comes from the same pinned checker witness: eight warmed
hits, canonical key widths16/256, UTF-8/WTF-8 widths1/2/3. Exact comparisons and
all source-copy/other charges stay active. The shared quota helper accepts an
explicit healthy-successor callback so this fixture reuses its existing cheap
query instead of requiring a new declaration. All exact total boundaries, flat
depth and successor checks remain. Test-only hash observation does not change
production limits or debit rules. Four GREEN pass
(`vrun-28e6157c-12b4-4261-8406-b9c9269f438d`). Hash-only omission/restoration,
pinned32controls, current location/symbol regressions and format are now active.
The broader replay must restart for these observation/test changes; no earlier
partial replay is promoted to a full pass.

## Literal hash observer repair closed as a test-only increment

The repaired four-test suite passes
`vrun-28e6157c-12b4-4261-8406-b9c9269f438d`. Omitting only literal-key byte
hash accounting fails all four tests, leaving exact comparison and every source
copy/debit active (`vrun-2f8faa13-0e5b-4d97-bf46-ceb34d237fbe`). Exact restoration
passes (`vrun-aa0e4bd4-b4ac-4ded-ac6e-a53360611e37`), as do the pinned32control
selections (`vrun-d14a1d11-cc9d-4b74-87d2-f7b5730f2d16`), current location
and symbol-cache scopes (`vrun-9adb5498-51f7-4f21-b262-cf316e5f8abf`,
`vrun-62bd6fce-a567-46b8-b5f1-fc9364dc3dda`) and format
(`vrun-11a66224-b241-4065-a1ee-02fcc59bb4f4`). No production debit or limit
changed; the previous common growth assertion/diagnostic is restored unchanged.
Only literal-cache tests, a callback-preserving test-helper wrapper and cfg(test)
hash observation changed.

The complete bounded library replay is restarted in library-partitions-repaired/
from symbol-location-repaired-input-hashes.json. It repeats all partitions for
the observation/test change, and runs Clippy only after complete restored
coverage. The previous timed-out/incomplete replay stays preserved and is not
counted as a full pass. All resource debit mutations are restored. Header-array
work remains a draft; other accounting/calibration/final delivery gates remain
open.

## Symbol caches and source locations: complete broad checkpoint

All 49 registered library partitions pass on the repaired frozen snapshot:
138 active non-resource tests plus one existing ignored corpus diagnostic,
106 direct resource tests, and 332 tests in 47 resource child scopes. The exact
577-name inventory, disjoint coverage and receipts are recorded in
`.cc/temp/completion-next/library-partitions-repaired/coverage.json`; its
`complete.json` records the 576 active tests. The two temporary test-module
selectors were exactly restored before the child-scope replay and Clippy.
All-target Clippy passes (`vrun-abe0575f-0661-4499-9c7e-1e8b64e5c768`).
`symbol-location-broad-complete.json` binds this result to
`symbol-location-repaired-input-hashes.json` and verifies unchanged Rust/Cargo/
fixture contents and inventory. Neither the earlier whole-library timeout nor
the first incomplete partition replay is promoted to a pass.

This completes broad validation for the symbol value/declared-type caches,
source-location file copies and literal-cache observer repair. Header-array
acquisition is now in its separate scoped proof phase. A fresh registered
independent harness replay has also started for 396 of the current 398 harness
files; editing/navigation flows are explicitly deferred until a fresh product
binary is built. Its inputs and receipts are in
`.cc/temp/completion-next/current-independent-harness/`. Pending batches are
not counted as passes. Whole-evaluator coverage, finite production budgets,
maximum unchecked work, stack/teardown, final Rust/semantic checks and the fresh
release/service proof remain open.

## Declaration-header array acquisition

Native raw headers now expose a borrowed exact-size declaration iterator instead
of allocating an intermediate array. The evaluator copies that ordered scalar
array through one checked collection owner; checker-created headers use the
same owner. This preserves declaration order and duplicates before the existing
origin projection. Attachment and full typed-fact validation remain separate
operations; their failure order has not changed.

The existing pinned origin inventory supplies strict/null-loose and noCheck
controls for empty, duplicate, materialized, tied and 16/64-overload arrays.
Four end-to-end work/storage tests fail before the fix
(`vrun-5008159c-2567-46db-a5f2-549b33ca6eba`) and pass after it
(`vrun-1dc2adc0-9816-4a49-9a4b-a8224ed6d63b`). Removing only the raw route's
controller fails its two tests (`vrun-86224f83-81a8-4dbd-831e-9a18ce9527a1`);
removing only the created route's controller fails only its pair
(`vrun-b27a9844-34cd-49f1-91dc-62266f8350e5`). Restoring exact source passes
(`vrun-b9626244-f8c0-4257-999a-74d44a664c53`). Each test retains exact
under/at/over quota checks, native final metadata, flat-depth controls and a
healthy successor request.

The origin, Unicode filename and property-metadata resource suites pass, as do
nine integration targets for loading, materialization, member values, composite
properties, generics, private names, recovery spans, member references and JSON
facts (`vrun-4881f183-3b55-4c56-88cf-967050404c70`). Native compatibility passes
(`vrun-99908d0e-93c4-4775-a850-de655e70254b`), and restored formatting passes
(`vrun-46b6192c-0e08-42d6-b2d6-331571d61687`). The complete scoped receipts and
source hashes are in `header-declaration-proof-complete.json` and
`header-declaration-green-input-hashes.json` under `.cc/temp/completion-next/`.
No accounting omission remains. This is scoped proof, following the earlier
complete library/Clippy snapshot; final combined gates remain required.

Presentation sizing is now in its own proof phase. Its proposed counting sink
uses the existing serde wire serializer, charges emitted-byte work and avoids
retaining a temporary encoded copy. No spelling, escaping or byte-limit rule is
reimplemented. Full fact/materialization copies, other evaluator coverage and
production calibration remain open.

## Exact presentation size without discarded encoded buffers

Wire-size admission now uses the same serde serializer with a counting Write
sink. It charges each emitted byte through the existing request controller and
retains no encoded output buffer. The existing member/omission delta calculations
share one private sizing context; JSON escaping, field names, output limits and
semantic child acquisition remain unchanged. A required-child closure borrows
that context per invocation so it can still demand a child between its two
admission checks.

The final-query regression covers native-pinned origin metadata, ASCII and all
UTF-8 filename widths, four option profiles and 16/64-overload flat controls.
It fails against the old implementation (`vrun-02c42a5c-07d3-47bd-8d47-68528fff189c`).
An initial compile-only borrow error is preserved separately
(`vrun-23b89770-8fee-4c9a-98bf-188c93f21878`); the corrected implementation passes
(`vrun-3a8a07f6-bf00-46dc-811f-55910fd69601`). Removing only the sink's controller
fails the work test while keeping the serializer, exact byte counts and observer
(`vrun-85407884-77c7-4053-8592-1e1830cd12d8`). Exact restoration passes
(`vrun-13a089b6-2aff-4534-8b53-e2a4edf9071d`). The test checks complete-query
under/at/over work boundaries and a healthy successor; its minimum includes the
finished description's complete public JSON encoding.

Prior location/header regressions pass. Presentation plus graph-count integration
passes (`vrun-7b4db05a-c601-4e8e-ba15-ce76d100c1ad`), as does generated literal
output (`vrun-8da62982-5ceb-42b8-9715-13b4090dd875`), including the existing exact
output-byte boundaries. Its pinned native controls pass
(`vrun-4864c3b5-6d24-41cd-ba69-82e6fc019895`). Restored formatting passes
(`vrun-5d14b836-5ba3-42d4-9e33-110aac75e960`). Scoped receipts and current source
hashes are in `presentation-size-proof-complete.json` and
`presentation-size-green-input-hashes.json` under `.cc/temp/completion-next/`.
All omissions are restored. This removes the discarded encoded buffer; it does
not prove complete presentation accounting, bound every serde scan's latency,
or choose a production work/memory/depth maximum.

## Validated facts and declaration materialization

The ephemeral NativeInputs view now borrows the existing optional request
controller. Source supplies its captured capability; the two loading constructors
and retained-input test constructor explicitly supply None. No controller or
mutable ledger is stored on Project, binder products or native identities.

One fallible collection mechanism preserves the first ordinary error and checks
completed-element copying before growth. The complete native fact array is
still validated before raw-symbol syntax attachment begins. The separate property
route retains its existing per-declaration materialization order. Child preparation
runs outside the copy observer, so parsing or other child charges cannot mask a
missing destination-copy charge.

Six final-query work/storage regressions fail before the fix
(`vrun-9fe802f3-b3f2-4fe9-81b6-8ace88ea8823`) and pass after it
(`vrun-b4413651-380b-49f8-97ab-b4d787a0a063`). Their native origin inventories
cover four option profiles, exact declarations/origins and 16/64 flat controls.
Each isolated omission retains child preparation, values and observation: validated
facts (`vrun-2be72732-6783-4531-9a1a-221cedb30617`), raw materialization
(`vrun-4264d07c-5857-4ce4-8f35-b7b3967927ec`) and property materialization
(`vrun-bf6586af-2a5e-442a-b70a-7d3a031d83d7`) each fail only their own two
tests. Exact restoration passes (`vrun-286ec6bd-e924-46ed-802a-11947195861c`).
The property label denotes ReferenceSymbol::Property representations, rather
than only checker-created metadata.

All five prior resource scopes pass: headers, presentation sizing, origins,
Unicode filenames and property metadata. The same nine integration targets used
for the header increment pass (`vrun-abe8a4df-cfd2-4b4b-b026-b3f9e8bf0352`), as
do pinned native compatibility (`vrun-135027d2-ffda-4b84-9d0d-bd3d2bd3ce9a`) and
restored formatting (`vrun-7b91894f-8486-4822-984e-c21ff9d6d995`). Receipts and
source hashes are in `declaration-materialization-proof-complete.json` and
`declaration-materialization-green-input-hashes.json` under
`.cc/temp/completion-next/`. All omissions are restored. These are scoped
checks; the final combined library, Clippy and delivery gates remain open.

The next batch reuses the already checked FixedMap backing for 25 separate
lookup/insert/remove-only caches: nine signature, eight generic and eight
conditional fields. Mutable-entry and text-key questions remain separate.
A public-target regression also addresses the test helper's suffix collision
between distributive and nondistributive identifiers. This batch is in proof;
no pending result is counted as passed. Production limits remain unset.

## Additional fixed-key cache backings and conditional probe selection

Twenty-five existing caches now use the checked FixedMap backing: nine signature,
eight generic and eight conditional fields. Their native question, value,
provisional publication and ordinary failure behavior stay at the existing
operations. New key markers cover only bounded scalar identities/tags and fixed
pairs; text, mutable-entry and variable child-copy questions are excluded.
The request supplies the same controller at each state constructor; loading
still supplies None. Two former indexed generic-target reads use the checked
getter with their existing missing-entry panic policy, and the signature expansion
module now imports its own scratch HashMap.

The test helper formerly selected `distributiveChosen` with a bare suffix search,
which instead found the later `nondistributiveChosen` identifier. It now follows
the pinned oracle's exact `const name` anchor and uses SourceText for positions.
The general Fixture prefix selector is unchanged. A public target-location
regression proves that the two conditional variants are selected separately.

Seven intended REDs are recorded (`vrun-72592f7c-e3bf-463f-a448-9060764c9cd0`).
An initial compile-only failure is preserved separately
(`vrun-d048eaf4-ba68-45f5-95d5-9811f5183867`); corrected GREEN passes
(`vrun-23665d99-6843-411e-8de6-32fcb07cc5e5`). The six family routing/quota tests
reuse pinned signature/reference/conditional cache controls, four option profiles,
exact under/at/over boundaries and 16/64 flat companions. Each family controller
omission fails only its pair: signature (`vrun-f4ddf4f2-e936-4796-b750-3b21a8aaa84b`),
generic (`vrun-ebfaca28-7799-4d65-888c-4e810c80d4e7`) and conditional
(`vrun-d27a50b1-42a4-461d-b001-34e824eff7a1`). Restoration passes
(`vrun-a99f02c4-f0d5-4774-89d3-261533652600`). Restoring only the old selector
fails only the new public-location test (`vrun-3f72d768-a94b-409a-96ae-68ed09c2eec1`).

Forced zero fingerprints retain actual hash/comparison/capacity work and pass all
seven tests (`vrun-fbe0cb70-a446-4358-9c0d-918532af5ac6`) plus optional-call,
conditional-type and signature-mapper identity suites
(`vrun-943c8f53-8254-4ef9-9c71-db38d25b4fe3`). Exact final restoration passes
(`vrun-0a91bd04-fa74-4f80-afbc-03afc6764b0f`). All five prior resource scopes
pass, including the corrected existing conditional-cache tests. Six integration
targets pass (`vrun-2d2b3d60-d3c1-4f99-a3af-7d50678d8171`), and final format
passes (`vrun-9bcfb0d0-7695-4ffe-8d31-13b8922cac32`). Resumed drivers preserve
their earlier receipts separately; the complete validation index includes them.
The scoped final hash/proof files are `aux-cache-green-input-hashes.json` and
`aux-cache-proof-complete.json` under `.cc/temp/completion-next/`.

All trial mutations are restored. These tests establish backing and family
routing; they do not prove every native cache path, child payload copy, remaining
query loop or complete resource coverage. Q3 search scheduling/result collection,
property/error snapshots, other caches and maximum unchecked units remain open.
Production limits and final combined/release/service gates remain pending.


## Current independent harness replay and created-header diagnostic

All 396 source-only harness files in the frozen 398-file inventory pass in
198 registered, disjoint batches: 1,124 tests executed and passed, zero skipped. The two built-product editing/navigation
journeys remain deferred until a fresh final binary. The complete selected
inventory, per-batch receipts, and unchanged tool/script/fixture hashes are in
`.cc/temp/completion-next/current-independent-harness/`; the first/last batch
receipts and all intermediate outcomes remain indexed. This is complete
coverage of the selected source-only inventory, not the final delivery gate.
The later new Q3 resource oracle is an additional explicitly recorded scope.

The temporary CreatedProperty Clone panic diagnostic ran the late-symbol,
composite-property, mapped-type and mapped-cycle integration roots separately.
None engaged a deep copy-on-write clone. Exact source restoration is verified
in `created-cow-natural-diagnostic-restored.json`. These four observations are
diagnostics, not product pass receipts or proof that every COW route is unique.
The unresolved-symbol Parent mutation has a separate unique-Arc code trace;
remaining mapped/late COW and merge-declaration growth still require closure.

Q3 search scheduling is now under scoped resource proof: checked Work-list
factories, the existing explicit work stack and candidate-token scans share the
request's controller. The new regression matrix's original three behavioral
REDs are `vrun-4fca9fd8-7ad9-44f7-b016-e2baae60008c` (an earlier test-helper
compile failure remains recorded separately). Pinned 7.0.2 control
`vrun-a78b3707-2b92-4c27-be36-62e78273ca2f` passes all eight size/option
cases, including exact method origins, candidate and call-filter ranges.
GREEN, isolated omissions/restoration and compatibility are still pending.
This scope does not close result-ledger or import-tracker accounting, nor the
project-wide repeated posting scan's asymptotic cost.


Q3 scheduling follow-up: the frozen native-fixture regression matrix passes
`vrun-e1f575a7-8bed-4957-95a0-0dfc585e09cc`. Omitting only Work scheduling
charges fails exactly the two scheduling quota tests
(`vrun-fa8b7deb-8be6-45a7-be21-6aae4b22f2ec`); omitting only token-visit
charges fails only the token-scan test
(`vrun-650f9f83-4f45-48d8-9f67-c02d89f7a1ee`). All observation scopes and
unrelated accounting remain active. Restoration and compatibility are pending.
The next 26 fixed-cache backings are still draft-only; existing pinned controls
for their symbol-header, anonymous, indexed and structure fixtures pass
`vrun-6d01f895-8c94-42e2-bba5-d5fd0114385f` (five tests, zero skips).


## Q3 Work factories, traversal stack and candidate visits: scoped checkpoint

The frozen native-fixture regressions, including all exact total boundaries,
flat 16/64 candidate depth and healthy successors, pass
`vrun-e1f575a7-8bed-4957-95a0-0dfc585e09cc`. The two isolated omissions and
restoration above are complete; restored regressions pass
`vrun-bad53429-3ec8-414e-833b-e72daff62f74`. Eight Q3 compatibility roots pass
`vrun-d3f88d2d-dbe3-4373-9d66-d87437819241`; native scheduling/core controls
pass `vrun-dfdf6152-a8b7-452b-8465-855d3dcdcbf5`; format passes
`vrun-bcc146b7-633e-4542-9d6f-c063039c34a5`. Source hashes and the exact
scoped record are in `reference-scheduling-proof-complete.json` and its paired
hash file. All counterfactual mutations are restored.

This checkpoint covers Work-list allocation/construction, checked stack
allocation and pops, file and candidate visits, preserving depth-first native
continuation order. It does not close every transfer/checkpoint, location/key
copy, searched/reexport set, ledger, import-tracker or sorting operation.
The initial Work-list-to-stack transfer still needs an explicit per-item
checkpoint when spare stack capacity avoids allocation. Repeated global name
postings per file retain their existing cost and order; each visited token is
now checked. Production limits/calibration and final broad gates remain open.

The 26-cache batch has now started its pre-change 14-test RED phase. Native
controls already pass; no cache implementation is yet claimed complete.


The 26-cache pre-change run fails all 14 intended resource regressions
(`vrun-fa7b17f8-bb30-49dc-ae33-b40f18af0727`). Its first implementation run
fails compilation (`vrun-4ce3cf65-6d30-4ead-9a5c-3fe3f515a31a`):
PropertySymbolId needed its own scalar-identity marker, and the unresolved
symbol child retained an old caller reservation. The marker now lives beside
PropertySymbolId; the shared backing owns that child reservation too. The
corrected GREEN run is active. No passing behavior or completeness claim is
inferred from a compile failure or the read-only remaining-map inventory.


The corrected 26-cache implementation passes all 14 resource regressions
(`vrun-ef97e017-ef0c-428d-944b-e5aedf9425a1`). The property-symbol family
controller omission fails exactly its two quota tests
(`vrun-b17dd157-2752-45af-9877-1f1842f5a487`); remaining isolated family
checks, collisions and compatibility are still running. The new bounded-key
markers include NativeNodeIdentity and PropertySymbolId at their original
identity owners; neither marker changes equality, hashing or semantic meaning.


## Symbol/member/type-operation cache backing: scoped checkpoint

Twenty-six existing fixed-key caches now use the same checked backing: six
property-symbol caches, six symbol-table caches, two member-value caches,
three anonymous-type caches, four mapped-type caches, three indexed-access
caches and two keyof caches. Native questions, stored values and publication
order stay with their original operations. Scalar key markers live beside the
identity/tag definitions; the three old caller reservations now belong to the
backing. IndexInfo Arc identity, mutable-entry maps, sets and child deep copies
remain separate obligations.

All 14 regressions pass (`vrun-ef97e017-ef0c-428d-944b-e5aedf9425a1`). Each
of seven isolated controller omissions fails only its family's two tests.
Forced fixed-map fingerprint collisions pass all 14 tests
(`vrun-dbe38677-14b4-4567-8e7c-4792d3c27241`) and four broader identity
roots (`vrun-1e209b02-c9bd-4eee-a9dc-e77a99b67c54`). Exact restoration passes
`vrun-2de70bc1-f9df-4282-9109-ee7c172cb863`. Containment, property metadata,
previous auxiliary caches and Q3 scheduling pass their follow-up scopes.
Eight compatibility roots pass `vrun-24384f7a-2580-449d-89b8-71069c5d1fa0`;
format passes `vrun-191c06c3-5e1e-4e64-b7e2-17eed20db637`. The resumed
proof record and complete source hashes are in `remaining-fixed-proof-complete.json`
and its paired hash file; the earlier RED and compile-only failure remain in
the validation index. All omission/collision mutations are restored. This
backing proof does not establish each field's individual engagement, child-copy
coverage, whole-evaluator coverage, production caps or physical calibration.

The next import-inventory increment has started its three-test RED phase.
Its compiler controls include 48 member queries across four option profiles,
with flat 16/64 static/dynamic import pairs; exact groups and empty demand
failures pass `vrun-1ce3a0c4-010b-4bad-8221-adad1dd105ef`. That supersedes
the original strict-only import harness scope. These controls are separate
from the already completed frozen 396-file replay; final inventory evidence
must include the later harness and fixture changes.


## Shared module-site inventory: scoped checkpoint

BindingFiles::reference_module_sites now owns checked request visits, retained
record capacity and comparison work, with its three Source callers passing
the existing request capability. Loading passes None. Ordering uses the old
implicit/kind/span key plus the original request index on ties, preserving the
old stable result with an in-place sort and no hidden stable-sort buffer.

Three pre-change quota regressions fail as intended
(`vrun-d57272e7-56b3-495b-925f-34f2c78e990a`) and pass with the fix
(`vrun-1dd9099b-a826-4743-8299-2885ee2c3452`). Independent visit, storage
and sort-fee omissions fail only their intended test
(`vrun-9af92f36-0cf7-4650-910b-75aff7b8ff50`,
`vrun-cdace58f-9e3d-4567-b9ab-bf2c7654ad5c`,
`vrun-3fb8109b-abd6-4e56-ad9a-e753157cfccc`). Exact restoration passes
`vrun-65cc18fb-bfa3-4e85-a9ba-e50b342610ee`. The 48 native controls,
previous Q3 scheduling and 26-cache regressions, six compatibility roots
(`vrun-1a174329-2ece-4479-bc09-c48a932e6b4a`) and format
(`vrun-4d8e4f77-1cdb-430b-9fbb-0b74d183fcf9`) pass. Three additional
module-mode/diagnostic/import-helper consumer checks are running separately.
The exact scope/source hashes are recorded in q3-site-inventory-proof-complete.json.

The next posting-key/initial-transfer increment is draft-only. Its 64 pinned
compiler queries pass `vrun-b39c6367-842e-4ccf-a22f-2c7151f6dc8e`, covering
1/2/3/4-byte identifier scalars, widths16/256, 16/64 empty searched files,
four option profiles and a distinct healthy successor. No implementation pass
is inferred from those compiler controls. Standard-table lookup latency,
remaining Q3 ledger/import-tracker/selection work, other evaluator coverage,
finite caps and physical calibration remain open.


The additional three module-specifier/mode/import-helper consumers pass
`vrun-82216f31-92e8-403e-9740-8bc0bc41bbc2`, zero skips, on the restored
site-inventory source. The posting-key/initial-transfer driver has now started
its two pre-change regressions; that following increment remains in proof.


The posting-key/initial-transfer increment's two intended REDs are
`vrun-87807afa-7e35-432e-bd8b-91d83e213b8d`. A separate compile-only failure
(`vrun-32dc5178-284a-4fc9-a527-152e6df42451`) required an explicit borrowed
slice type after introducing a local result variable. Corrected GREEN passes
`vrun-ba27e8ca-0699-4726-9db1-5a95bc6397c7`. Key-fee omission fails only its
test (`vrun-d57d8920-1754-442b-b5cf-fca522fdcfac`); transfer-fee omission
fails only its test (`vrun-4ba53375-c35a-4dcc-8bcb-2da049508ec0`). Exact
restoration passes `vrun-6e55224b-5403-45d8-924d-fba52d46796b`, repeated
64 compiler controls pass `vrun-3b4bee4d-bfb9-460c-97c3-0d70da2a3207`, and
previous scheduling passes `vrun-6174dd34-a5a2-4fb0-b00e-b835dddba6dd`.
Other compatibility and final format/source hashes for this increment remain
active. No production limit or maximum lookup-latency claim follows from this
scoped work-unit proof.


## Exact posting-key bytes and initial stack transfers: scoped checkpoint

The posting/transfer source is fully restored. All six compatibility roots pass
`vrun-4534cef1-25d7-4ddc-b2a2-c117705def72`; format passes
`vrun-ba9b78a8-362d-4a5b-9c98-ef203d1b8547`. Together with the 64 native
queries, independent fee omissions, exact totals, flat file-count depth and
healthy successors above, these complete the named-owner increment. Source
hashes and resumed-tail receipts are in q3-posting-proof-complete.json and its
paired hash file; the initial RED/compile-only failure remain preserved.
The standard table's indivisible lookup latency and other query/evaluator
operations still need bounds or coverage. Production limits are still unset.

The next ImportTracker increment has begun its 21 pre-change resource tests.
No implementation pass for that following map/seen-set/traversal/buffer work
is inferred from the completed posting scope.


## ImportTracker traversal in proof

The direct-module backing, its mutable checked lookup, duplicate/seen identities,
both iterative stacks and retained output buffers now share the existing request
controller. Native semantic acquisition order and reverse-push DFS order remain
at their original operations; no new scheduler or controller is introduced.

All 21 pre-change resource regressions fail as intended
(`vrun-15aedd04-906d-437c-856d-6d931da351c1`). The first implementation run
(`vrun-c8438561-1c39-4673-b311-0a5c97268aee`) compiles and passes 14, with
seven test minima failures: the padding imports factory only, so it widens the
shared map for both selected modules but widens the subsequent traversal only
for export=. The helper now accepts native-family minima. Shared inventory still
requires all2N records for both; width-dependent traversal/output minima remain
for export=, with native-required nonzero minima for default. Exact groups,
limit edges, flat depth and alternate healthy successors are unchanged.
All21 then pass `vrun-a25c0756-821b-4b24-b9a5-2359452cc1d1`. Independent
omissions, collisions/restoration and compatibility are running. Source child
projections, cached error payloads, ledger work and other branches are separate
obligations; this is not a whole-query coverage claim.


ImportTracker follow-up: all eleven named controller/fee omissions fail only
their intended regressions. Exact restoration passes all21
(`vrun-a5614394-bd08-4155-870f-922686f99d17`). Forced fixed-map collisions
pass all21 (`vrun-2b99f49b-417e-48ae-ba5a-52fe87ddb2ed`) and broader
import/augmentation roots (`vrun-7b60dd8b-8ed6-4df3-87a8-8538eb70bae7`).
Normal fingerprints are restored; all21 pass again
(`vrun-5acacb64-2133-4486-bf7e-09cfd86040ae`), as do native48
(`vrun-b093f750-1abd-40c4-9deb-ed063005af4c`), Source inventory
(`vrun-970a5155-48e0-4403-8520-a51659cfe13a`) and posting regressions
(`vrun-6e385c43-44f5-4b80-bee0-e6f36bd48c5a`). Final six-root compatibility
and formatting are active. Source projections, cached error payloads, ledger
storage/copies/order, other native branches and production calibration remain
outside this named-owner proof.


## ImportTracker named-owner checkpoint complete

Final six-root compatibility passes
`vrun-c992a1dc-b1d6-45b9-bf44-579c6ec09a0c`. The family-specific test
correction needed nine formatting hunks; the original failed format observation
remains recorded (`vrun-65389cec-83d0-409b-b39d-9b0d438e3b94`), and final
format passes `vrun-bd6fb56d-a2cc-4685-9cfc-e4487a0f0994`. All controller,
fee and fingerprint mutations are restored. Complete named-owner receipts and
current source hashes are in q3-import-traversal-proof-complete.json and its
paired hash file. Historical minima/format failures remain failures.

The next Source-projection stage has started seven intended pre-change quota
tests. Its inventory/ancestor/name/specifier work is distinct from tracker
storage. It preserves the empty/all-implicit syntax-free early return and the
separate forEachImport/source-import questions. Reexport/implicit/ambient/type
branch engagement and independent omissions still need proof; no completion
claim for this following stage is inferred from tracker passes.


## Source import projections in proof

Seven pre-change regressions fail as intended
(`vrun-fff1a142-3c84-48cf-8532-14a11c07477e`) and all pass with the Source
owner changes (`vrun-659fb6e3-5106-4dec-bf33-3af01f8c6d8c`). Inventory
work/storage, ancestor visits, exact-name copy and fallible specifier-copy
omissions each fail only their intended test(s). Exact restoration passes all7
(`vrun-a0a5789c-dd9e-4517-9154-96a57c212bb9`), repeated native48 passes
(`vrun-72a28600-a148-4dd4-af0c-f26edb0a2f95`), as do the preceding21 tracker
regressions (`vrun-b82f7164-d218-4127-97bb-5e272263d461`) and Source inventory
(`vrun-743e7e4f-3915-478f-a510-c805abcf5bae`). Compatibility and final format
are active. Empty/all-implicit early returns, implicit suppression for the
source-import question, ambient-stop rules and attachment ownership are
preserved. Direct branch proof for implicit/ambient/qualified/type/reexport
cases remains a follow-up; whole-query coverage and calibration remain open.


The named Source checkpoint finishes with five-root compatibility
`vrun-01dd61f2-096a-4e7f-8409-3f15f2ce973f` and format
`vrun-b0a43ba4-8bcc-4ac8-b42c-4cb4dc6fd853`, both passed. A direct
namespace-reexport scan proof then establishes its existing default-module
consumer: saved pre-change Source RED `vrun-57a61826-8048-4798-834f-5fc21e7ff47a`,
GREEN `vrun-9a508b05-e027-44ed-9482-8c93b9ecd517`, fee-only omission
`vrun-49af3ded-40a9-4aaa-8934-1df7d70a8c83` (both intended failures),
exact restoration `vrun-44c94292-0284-4445-8fff-e952c85a1402`, native48
`vrun-6f56086d-299a-46d9-b95f-3b4f63667828`, and format
`vrun-1af4ba64-af0a-4ef5-825c-d61c1fc98681`, passed. All mutations are
restored. The following broader native import-branch matrix is in capture;
its outcomes are not yet implementation expectations or proof.

## Import-type, ambient and global branch controls — 2026-10-15

A separate clean compiler fixture adds `typeof import`, an actual multi-segment
qualified name, nested ambient namespace reexports, and an `export as namespace`
consumer. All sixteen native queries (four option profiles) have no diagnostics
or demand failures. The Rust query preserves each native group and exact range,
including the compiler's directed import-reference asymmetries.

- Extended native control: `vrun-9952c296-60f3-45a5-86b9-da332c37b51b`.
- Exact extended Rust outcomes: `vrun-4b579258-b944-4082-9e13-af92975f455c`.
- Ancestor/reexport owner work quotas, complete results at/above the limit and
  resource failures below it: `vrun-f6b7e2ce-a0bd-4732-ab35-5d2a4f2af936`.

The quota cases establish engagement of those existing owners; a one-visit
minimum does not establish a qualified-chain depth bound or complete implicit
helper/JSX import coverage. Those limits remain explicit follow-up obligations.

## Reference result-builder accounting — implementation in progress

Eight final-query tests fail against the previous builder because group storage,
occurrence keys, retained payloads and final collection/order have no owner
charges (`vrun-ceb365f7-c50e-44db-9ae5-af1a1a32a135`). The initial implementation
had four compile-only missing-borrow errors (`vrun-105c6a62-6734-48d6-a672-85d4cbcd96e9`);
corrected code passes all eight tests (`vrun-941d3895-d6ff-47a3-a0de-4909ee78a0c8`).
Independent fee omissions, collision identity checks and compatibility are
running; this is not a completed accounting or production-calibration claim.

## Reference result-builder scoped proof completed — 2026-10-15

Group lookup now uses the existing controlled fixed-identity map. Each group's
occurrence cache uses controlled exact location fingerprints and comparisons,
with checked scalar buffers for retained records and roles. Stored filenames,
occurrence names and Work locations copy through one retained-metadata owner;
Model origin copies reuse it. Final collection, stable sorting, sequence/location
comparisons and gap dedup charge the same request. The native declaration/role
policies and directed continuation order remain with their existing owners.

- Corrected eight-test GREEN: `vrun-941d3895-d6ff-47a3-a0de-4909ee78a0c8`.
- Independent group, occurrence, payload and finish omissions each fail only
  their intended work/storage pair: `vrun-003a8da2-b673-4115-bf01-b2de94233d98`,
  `vrun-ba885f83-06d3-420c-976d-6b0783840aed`,
  `vrun-72bf39c5-460c-4503-aef8-ee6d1ec9947f`,
  `vrun-be74f50a-a375-4a69-bec3-d0e1f4b20b8f`.
- Constant occurrence-key fingerprints preserve all final ranges in the resource
  matrix and three identity/filter/import suites: `vrun-099a70bf-82fe-43c1-8bfd-7ced2ebd5aea`,
  `vrun-e984cebd-ecd2-4a5f-8f12-5d534a2886fc`.
- Exact restored resource tests: `vrun-3f3f876d-76ca-4b64-a373-e0842a732334`.
  Prior scheduling, metadata, import-branch and Unicode posting scopes pass
  separately; their receipts are in the validation index.
- Five reference/import/filter/failure-cause compatibility suites:
  `vrun-d5d526e1-1f53-4636-b43f-6d13133b25b1`.
- Three pinned native controls: `vrun-2ab11665-628a-44ae-9abc-b3ee88a1f7bf`.
- Final format: `vrun-1f154817-4ede-4e3a-a175-31ba39cd7d8d`. An intermediate
  format-only failure retained one trailing blank line after module extraction.

All omission/collision mutations are restored. Stable-sort scratch is debited
conservatively before the standard-library call, with comparator and post-call
checks; its maximum uninterrupted move/allocation cost still needs physical
calibration. Gap error projection still performs deep copies, and search sets,
selection and other evaluator owners remain separate obligations. This scoped
proof does not set production work, memory or depth caps or complete delivery.

## Reference search membership — implementation in progress

The selected-symbol set now uses the checked fixed-identity map plus an ordered
scalar visitation list. The searched set stores exact `(file, symbol)` pairs,
and reexport visitation uses the same checked map. Native mark timing remains
unchanged. Search record allocation and fixed override-key copies are charged.

All six final-query quota tests fail against the old implementation
(`vrun-d4d39d04-d8c7-4d22-a60d-84553d7c4582`) and pass with the new owners
(`vrun-3bfed90e-acb7-4de2-a606-6e3e9f403fec`). Independent symbol, searched and
reexport omissions fail only their corresponding work/storage pair:
`vrun-9e28c1bf-54e2-4903-a089-27de3ed8d61c`,
`vrun-2723bf8d-7632-4208-9f0a-2ceeef3a7802`,
`vrun-4c39e24f-e025-46e5-b40f-51516c61301d`.
Collision and restored compatibility checks are still running.

## Reference search membership scoped proof completed — 2026-10-15

The restored six-test matrix passes (`vrun-2cec88f7-e706-42ca-8275-67ea2f13c3a5`).
Forcing every FixedMap fingerprint to zero preserves the six final-query cases
(`vrun-97ca3383-fc89-47b3-a6f8-0bef8881454b`) and both import/augmentation suites
(`vrun-f8f24f3c-a9f2-444b-98e7-9211911018a9`). Exact restoration is verified.
The prior ledger and scheduling suites pass, as do five compatibility roots
(`vrun-ea8eb611-4a66-416a-8712-159bae488255`), two pinned native controls
(`vrun-4f14cb69-6613-4a8d-936e-670f7fb527ff`) and formatting
(`vrun-c37f3dbe-87c3-4fcf-956c-80db1e712f28`).

This closes the three named search-membership backings. Shared named-declaration
projection, gap/error payloads, selection and other evaluator operations remain
separate from those sets. No production cap or calibration claim follows.

A new synthesized-helper import control also passes sixteen native queries
(`vrun-89d20805-e88f-4b2b-a870-1116a3265e05`). Its configuration uses ES2015 and
`importHelpers`, with a retained ambient `tslib` module. The compiler creates
the synthetic request for eligible external modules even without emitted helper
usage. An initial fixture attempt was rejected because TypeScript 7 removed
ES5 (`vrun-d609b821-1ee4-4e6d-b155-2d13a54bdad5`); it established no query result.
Rust direct branch engagement and quota/omission proof are still pending.

## Created-symbol mutation and declaration growth — 2026-10-15

Both retained CreatedProperty allocation sites now use the same checked record
factory. All four mutation sites use its explicit copy-on-write operation;
CreatedProperty and its value recipe no longer derive a deep Clone. A shared
record copies its declaration/type lists through checked scalar-copy operations,
retaining the original header identity and all scalar links. Native late/merge
declaration append now charges visits and backing growth at its existing owner.

The new compiler fixture contains flat all-late and early-plus-late method
overloads at widths16/64, across strict, loose and noCheck profiles. All eight
queries return number with no diagnostics/demand failures
(`vrun-bf3c5913-e1b4-4060-b765-6d74258cbb2e`). An earlier fixture failed only because
it omitted standard global declarations; that setup failure is preserved.

- Five final-query quota tests RED: `vrun-b2e151e8-2bd8-4883-92d7-e3bfe168056a`.
- GREEN: `vrun-0d46158e-84c9-4f62-8321-51a7d708cb7b`.
- Independent record, mutation and declaration fee omissions fail only their
  intended tests: `vrun-8b22a451-331a-40ac-9657-3a7df7855724`,
  `vrun-ca46bf19-5e9a-42ec-bd06-496babf2a9f9`,
  `vrun-7ed3b562-b668-4225-9ae9-786637423929`.
- Exact restoration: `vrun-26912b34-fe1e-4dad-b1e1-7c438a9499d8`.
- Four late/composite/mapped compatibility suites:
  `vrun-b2576c92-f6ff-4518-8c97-b9400c886e27`.
- Two native controls: `vrun-2bf22ce4-a117-4bc8-82ef-906520f2f679`.
- Formatting: `vrun-238d4d97-4b0f-4668-a32b-bd6740529dd3`.

The five tests establish retained allocation, mutation visits and declaration
growth. They do not establish natural engagement of the shared-Arc copy branch;
the earlier zero-clone diagnostic remains limited evidence. Other error/metadata
copies, evaluator owners, finite production caps, physical calibration and final
combined gates remain open. All omission mutations are restored.

## Reference gap/error copies — 2026-10-16, proof in progress

The frozen 32-query native fixture now has a passing Rust baseline
(`vrun-97054355-b5e8-4ae7-99fa-88425c86910a`): exact known reference groups,
Unicode UTF-16 candidate ranges, the documented Partial outcome with N unresolved
candidate gaps, and a healthy subsequent Q4 query. The strengthened baseline
also checks every retained cause's exact receiver range and missing name.

Six independent final-query quotas fail against unchanged production accounting
(`vrun-8cbde1e1-7a36-4db1-a733-e1f39b75fb9f`), while the baseline passes. Each
failure is the missing resource interruption, with zero observed fees for its
owner. Text work/storage minima use fixture name characters/UTF-8 bytes; path
minima use the captured file's actual canonical OS byte length; final cause-buffer
minima use native candidate count and retained ReferenceCause size.

Controlled error/cause metadata copies and cause-bound operations are installed,
with exact-path copying and move-only final cause projection. GREEN, isolated
owner omissions and compatibility are pending; this is not acceptance evidence.
Cached DemandError/NativeCompletion copies, named-declaration projection,
selection, the remaining evaluator inventory, finite caps, physical calibration,
and final combined validation remain separate delivery obligations.

The seven-test GREEN passes (`vrun-dae4157f-0f68-421c-a107-9e6bb5a8057c`).
Isolated text, path and final cause-buffer fee omissions each fail exactly their
own work/storage pair, with the remaining tests passing:
`vrun-f0236adf-0138-497d-85eb-0381d16685af`,
`vrun-bd8f7a41-1dee-4ab0-b4f6-98d0ef7e8e45`, and
`vrun-08a53853-ec18-4abe-8f50-3bdaedb48529`. Observation scopes and native
operations stay installed during each omission. All mutations are restored;
restored-source, compatibility, native, formatting and lint checks are running.


## Reference gap/error copy scoped proof completed — 2026-10-16

All three independent fee-omission experiments are restored. The seven-query-test
matrix passes again (`vrun-7c9771d7-082b-4b8e-90dd-0ea555777e98`), as do five
compatibility suites (`vrun-0fc8027f-8965-4603-a2a4-686dc0a31c47`), the prior
ledger matrix (`vrun-bfea2fb0-e189-4a6e-854e-20ab26713bdf`), and all 32 pinned
native controls (`vrun-f9844535-8fed-4a6f-b356-4ea71ef18fec`). The full fresh
native capture equals the frozen golden byte for byte; all 46 input hashes match.

Three supplemental transport tests (`vrun-859f1bd4-5325-4b6d-b016-041147d30bb8`)
prove private provenance preservation, 15/16/17-cause order and classification
before truncation, omitted-count overflow as OutputLimit, and exact non-UTF-8
OS-path copying on Unix. These supplement the final-query quota tests.

Initial lint found two inherited mechanical issues: a redundant Boolean form in
import tracking and an unnecessary into_iter on Option. Both were simplified
without changing evaluation order. The affected import-inventory tests pass
(`vrun-433c0bc4-3d61-49e9-9a22-df43bbf56fa7`). Final formatting passes
(`vrun-c277e6ea-f69d-48e7-8958-c7ba006984c3`) and all-target Clippy passes
(`vrun-2fdcc254-6ac2-41b5-bfca-39cadff78513`).

Evidence and source hashes are in q3-gap-proof-complete.json and
q3-gap-green-input-hashes.json under .cc/temp/completion-next. This closes the
controlled gap/error-copy scope only. Cached DemandError/NativeCompletion copies,
shared named-declaration projection, selection, other evaluator owners, production
caps, physical calibration and fresh final combined validation remain open.

## Named declarations and search normalization — 2026-10-16, proof in progress

The new native fixture freezes 16 queries over widths 16/64 and four option
profiles. Same-file overloads retain every declaration and the call. Split
cross-file overloads select a raw parent's member but acquire merged candidate
symbols; native relatedness rejects every candidate, yielding zero groups. All
candidates are eligible raw text matches, so this is an identity distinction.
Native capture passes (`vrun-4e591aff-f340-4986-a597-1b1709f17a63`).

The first Rust baseline found false matches (`vrun-c4490664-8275-42f1-b78c-261fb6f239c7`).
The expanded regression also failed filtered searches (`vrun-af565eb6-f9ea-4189-87b4-3aec9e84c32b`).
Candidate matching incorrectly repeated selected-seed combined-member
normalization. It now consumes the actual GetSymbolAtLocation result, as native
getReferencesAtLocation does. Unfiltered output also no longer adds an empty
seed group. All 16 native outcomes and all three non-default filter combinations
for the eight empty split cases pass (`vrun-8ee83579-d761-42e2-8a2e-e27d1aea0ab4`).
Six reference compatibility suites pass (`vrun-21f83d47-4932-41e3-a1f0-84e7d0593c5a`).

A supplemental 24-query native control covers computed-only use, mixed early/late
use and declaration seeds. Raw search omits computed-only uses; the documented
decoded inventory retains the actual native relatedOrigins/nativeRange facts.
Both raw output and those normalized groups are frozen. The first Rust check
failed before matching (`vrun-ee3b3705-f0c4-40ca-8249-4ef132178180`): search_key tried
to decode the native scanner's internal computed name as public UTF-8 text.
Initial search now uses its exact selected syntax key for the decoded inventory,
while still observing native name bytes in order. Import continuations retain
their own native name selection. No lossy conversion or error fallback is used.
All 24 Rust outcomes and healthy Q4 successors pass
(`vrun-b001a493-3bf2-4acb-a5be-679a1bfddfe8`).

Six independent quota tests now fail for missing named-declaration copy,
collection and sort fees (`vrun-2f48a38a-921f-4605-b4d7-871b6ae3d38b`); both semantic
baselines pass. Work/storage minima derive from retained native declaration
counts, exact name characters/bytes and NamedLocation size. The existing shared
projection is being changed to checked text copies, checked vector growth and
controlled stable sorting/deduplication. Stable-sort accounting is extracted
from the result ledger without changing its policy. GREEN and isolated owner
fee-omission proofs are running; this is not whole-evaluator acceptance.

## Named-declaration scoped proof completed — 2026-10-16

All eight tests pass with controlled name copies, declaration-vector growth and
stable sorting/deduplication (`vrun-6d8f587b-f036-414f-ab43-e24f0b5a4859`). Independent copy, collection and
sort fee omissions each fail exactly their own work/storage pair while the other
six tests pass: `vrun-ac0f7e1d-07cb-448c-9e5a-f7432e562dfa`, `vrun-2f37bd6b-9968-4611-92c4-a7e64ba94b36` and
`vrun-05bfca22-eb23-4bf9-833a-218b6382e166`. Observation scopes remain installed. Every experimental
mutation is restored byte for byte; the restored eight-test matrix passes
(`vrun-6affbe54-6aa0-499d-931a-db9332fa7dd2`).

The 24 late-member normalization controls pass again (`vrun-d699195b-6252-4bd3-815c-f76287f284cd`).
The prior result-ledger and gap quota suites pass (`vrun-5c03e096-eae9-4ab9-bb23-b1b2673a47e3`,
`vrun-4b09f796-dca0-4262-ad38-f78bf124490d`). Eight integration files covering reference selection,
contextual seeds, imports, augmentation, metadata and type selection pass
(`vrun-c87dba7b-0af6-4557-b1c8-d5f333d46af3`). All 40 fresh native queries match their frozen answers
exactly (`vrun-c20d5dcc-77d2-43e1-bc0f-ab0218d5d2e5`); both native input manifests match.

The formatter's 12 mechanical hunks were applied. Final formatting passes
(`vrun-85c15367-1511-4f1e-b0ad-343fe77864a9`), and full all-target lint passes (`vrun-845a8a22-fcc3-45c5-a172-b680f2ddcf2d`). Exact receipts,
omission-source hashes and final Rust-input hashes are retained under
.cc/temp/completion-next in named-declarations-proof-complete.json,
named-declarations-omission-source-hashes.json and
named-declarations-green-input-hashes.json.

This closes shared named-declaration copy/collection/order accounting and the
two discovered reference-search corrections. Q4 selection-specific containers,
accessor declaration projections, cached DemandError/NativeCompletion copies,
other evaluator owners, the natural CreatedProperty shared-copy witness,
for_each_import=false evidence, finite production limits, physical calibration
and fresh combined delivery validation remain open. T/Q3/Q4 is not complete.

## Accessor declaration baseline — 2026-10-16, accounting proof pending

Pinned native capture passes 32 getter/setter value queries over plain/computed
members, widths 16/64 and four option profiles, plus 12 contextual parameter
queries in getter-returned arrows (plain, literal and unique-symbol names).
Each value fixture independently inventories two actual getter/setter declarations
per member. Read and write arrays are number[]; the contextual parameters are
number, string and boolean. All native queries have zero demanded diagnostics
and failures.

The registered native run also replays the prior 40 named/late-reference queries
against their exact frozen results: `vrun-57984fd5-91d6-4ebe-8d8e-d82f77e89869`.
Its 952-entry before/after manifest includes all tools, validation wrapper trees,
compiler-hook files, the installed semantic API package, the native compiler
binary and all three fixture trees. This strengthens the earlier manifests,
which did not enumerate every supporting oracle module; it does not retroactively
expand their input-coverage claims. Frozen accessor goldens add two entries.
The pinned Go source commit and clean-tree checks remain enforced by the oracle.

All 44 Rust final-query outcomes and healthy successors pass in two tests
(`vrun-e52abc08-3363-46d0-8d3c-1c94358de7fe`). No accessor accounting change was
installed for that baseline. Twelve independent quota tests are now under RED
validation. Their draft separates raw bound-list scanning/collection, Bound and
Created value projections, Created accessor filtering, getter-signature projection
and accessor-recipe scans. Engagement and quota proof are not established yet.
Adjacent declaration consumers, cached failure copies, remaining evaluator
coverage, finite limits, calibration and final delivery gates remain open.

## Accessor declaration scoped proof completed — 2026-10-16

The missing-accounting RED passed the two native baselines and failed all twelve
quota tests (`vrun-d8364539-c0e7-4e5e-9930-6e986a49c67e`). The installed changes
extend the existing declaration owners: raw bound scanning and collection,
Bound/Created value projections, Created accessor filtering, getter-signature
projection and accessor-recipe scans. They preserve acquisition order and use
the existing request controller. All fourteen tests pass
(`vrun-153504db-4d9a-4973-8db6-352757dee20b`).

Seven independent fee omissions each fail only their own quota tests (one for
each work-only scan, two for each work/storage owner). Observations remain
installed, including inside the shared signature collector. Exact source hashes
verify restoration after every experiment; the restored matrix passes
(`vrun-65a4965a-c79c-4bba-ba24-db73e226a5ef`). Quotas cover just under, at and over measured totals,
owner-independent native-derived minima, repeated usage and healthy successors.
The fixture covers 44 native outcomes across plain/computed accessors and four
option profiles, including contextual literal/unique-symbol/plain signatures.

Prior named/header/materialization suites pass (`vrun-f7915e66-23b0-467f-9fe1-41b41da051b3`,
`vrun-a945d463-d2bd-4e29-90bd-855dfb48ebc0`, `vrun-cd4ab754-a333-42b0-ae93-25d762d3b51a`). Six integration files for
member values, late/global merges, initializers, reference metadata and alias
selection pass (`vrun-4a59c76c-60bf-4719-a232-eb0f581cbd9d`). A requested binding-only unit replay
was rejected before execution by the registered wrapper
(`vrun-3e7fe1de-983f-4026-9325-437883b7ce76`); it is not a passing test claim.
All 44 native queries replay exactly (`vrun-40b8ccb7-dafe-4ba1-ab12-183b2c9fcdda`), with the expanded
954-entry manifest unchanged before/after. Final formatting and full all-target
lint pass (`vrun-0611b387-e6f8-40bb-aae1-646e9aee1f04`, `vrun-440044f5-fd39-4e3f-9d22-d1aa9f936f44`).

The receipts and exact input hashes are retained in accessor-proof-complete.json,
accessor-omission-source-hashes.json and accessor-green-input-hashes.json under
.cc/temp/completion-next. Rejected raw binder entries, other raw binding
projection allocations and symbol_declaration_references remain unproven by this
scope. Cached failure copies, Q4 selection, other evaluator owners, finite caps,
physical calibration and final combined delivery validation remain open.
T/Q3/Q4 is still incomplete.

## Frozen native fixture relocation — 2026-10-16

The new whole-capture comparisons embedded the capture worktree's absolute input
paths. Five actual native queries in copied temporary fixtures failed at that
comparison, not setup/import (`vrun-a659af5c-6feb-43ae-9ba6-56ed9ae8a5fd`). The
shared comparison now rebases only apiProgramFiles[].file,
nativeProgramFiles[].file, programFiles[].file, inputFiles[], inputHashes[].file
and compilerOptions.configFilePath from the frozen config directory to the
caller's explicit physical root. It preserves every other field and byte.

All five relocation tests pass (`vrun-b679460c-6ea6-47e4-94a3-a9702bf77f0b`), including
negative controls for changed semantic data, source/literal text containing the
old root, exact filename case, wrong actual root and unrelated external paths.
The named, late-reference and accessor harnesses use this owner; the gap harness
now also replays its complete frozen capture. The combined replay passes all
116 original queries plus five relocated queries in ten tests, zero skips
(`vrun-8cabdeb3-c84f-42b7-b953-24937ee111ec`). All five raw golden files are unchanged,
and the 999-entry input manifest matches before/after. The prior accessor
Rust hashes also remain unchanged. This proves directory relocation on this host,
not cross-platform execution or whole-delivery completion.

Evidence: fixture-relocation-proof-complete.json,
fixture-relocation-native-input-hashes.json and
fixture-relocation-golden-hashes.json under .cc/temp/completion-next. T/Q3/Q4
remains incomplete; remaining ownership, finite caps, physical calibration and
final combined validation gates are unchanged.

## Per-file emit-helper cache accounting and identity — 2026-10-16

The resolveHelpersModule and checkExternalEmitHelpers caches now use the existing
FixedMap with the request controller. Module acquisition still precedes requested
bits, and publication still follows helper checks. Cached DemandError payload
copies remain a separate open obligation.

Sixteen native queries cover once/repeated object-rest assignments in 16/64
imported files under four option profiles (`vrun-15f5ef5b-eccb-46f2-8bd2-dff186252108`).
All return number without diagnostics or demand failures. The Rust baseline
passes all outcomes and healthy successors (`vrun-b628d064-6f28-4977-8fb4-a7bfb94f2d3c`);
an earlier test-helper tuple destructuring error was setup failure, not RED.
Four independent missing-accounting tests fail while the baseline passes
(`vrun-1b466037-76ac-4f05-89c8-d10d21d8240a`). GREEN passes all five tests
(`vrun-e010657c-5eb8-41bb-bf52-bca683a6cab5`). Each cache's work and storage fees
are then omitted independently with observations intact; exactly its two tests
fail (`vrun-950419d0-6eaf-4fb7-a782-cb545e9aa0f5`,
`vrun-cd4e10ff-5897-415e-b8e4-779d427246ed`). Exact restoration and the full
five-test suite pass (`vrun-20e1c7c2-2e6f-4cc0-9c38-8924e972ba0b`). Quotas retain
native-derived entry minima, under/at/over boundaries and healthy successors.

Twelve additional native queries use two physically distinct nearest tslib
modules, good/bad private-helper signatures in both orders, and a both-good
control. The initial harness incorrectly expected a top-level demandFailures
entry; its preserved failed receipt is `vrun-5f15e90e-cde3-4e0e-af4d-d58d309326b0`.
As the existing private-helper baseline already shows, native keeps number and
emits TS2304 with demandFailures empty. The corrected capture checks both enabled
PrivateGet acquisitions and both module inputs, and passes
(`vrun-28814764-6c4d-4ded-9c06-6c24d5d6766f`). Rust preserves the documented
InvalidSemantic/UnresolvedSymbol outcome, exact bad-helper path/range/private
provenance and retained cause locations; all three tests/twelve cases pass
(`vrun-c849b1dc-f8b8-4036-bf0e-d94960a405cd`).

Constant fixed-key fingerprints preserve every identity outcome
(`vrun-c9281aae-d051-47f2-be78-09d5cbac9497`). Deliberately collapsing each cache's
FileId keys separately erases the later bad helper's recovery, and exactly the
good-then-bad test detects the false Found result
(`vrun-c82d443c-4058-4226-b60d-a5c281552e1e`,
`vrun-b6beb929-4f7c-437f-b11a-71d7272b4fa4`). Both mutations restore byte for byte;
restored identity tests pass (`vrun-3c995a36-41e4-4176-8471-68ea0c109749`).

Four integration files for object rest, imports, private access and decorator
prefixes pass (`vrun-f86c3c06-4173-4aea-999c-481048118dac`). All 28 native queries replay against
unchanged goldens and input hashes (`vrun-f20f8d92-1530-4737-9e24-e896a7a03c71`); formatting passes
(`vrun-8e694726-40ba-48ef-b945-21f481eb6c3c`). Evidence and final Rust hashes are under
.cc/temp/completion-next in emit-helper-proof-complete.json,
emit-helper-fees-proof.json, emit-helper-identity-proof.json and
emit-helper-green-input-hashes.json.

This is scoped backing/identity evidence. The last full lint predates this
increment; post-change lint and final combined gates remain open. Cached failure
copies, other helper producers and evaluator owners, selection/catalog work,
finite production limits, physical calibration and the final release corpus are
still outstanding. T/Q3/Q4 remains incomplete.

## Directional enum cache accounting — 2026-10-16

RelationsState's enum-relation table now uses the existing request-controlled
FixedMap with its ordered pair of NativeSymbolIdentity values. Those identities
already own bounded allocation-free Eq/Hash. The get/insert sites in
isEnumTypeRelatedTo are unchanged: successful and quiet failed results replay,
reported failures rerun, and publication follows every acquired diagnostic read.
The explicit RelationsState initializer retains the other fields' previous defaults.
This increment covers table preparation, collision visits and backing storage;
NativeCompletion recovery payload copies remain separate work.

The new native-enum-cache-resources matrix has 32 cold queries: 16/64 distinct
same-named enum pairs, four strict/noCheck profiles, success, failure, and a final
result change in both orders. Every pair is compared twice. Native returns the
exact match/fallback literal without diagnostics or demand failures
(`vrun-7151e007-fa0f-497f-8cb8-2e3f28935e36`); the entire captures and expanded input manifest are
frozen. Rust passes every final result and healthy successor (`vrun-7dd61e6b-ce54-4c60-adf1-9abd6727aac4`).
The two missing-accounting tests fail on absent interruption with zero owner fees,
while the baseline passes (`vrun-7d8465a3-13cd-4a87-b5e5-94ed6a0d2abe`). The checked table passes all three tests
(`vrun-01ef08cf-6ba0-4a30-8a8c-3a01eb3eb923`). Quotas derive entry minima from fixture width and payload size,
subtract only observed owner fees, check just-under/at/over total boundaries,
repeat usage and run a healthy subsequent query.

Omitting only this owner's fees while keeping observation scopes intact produces
exactly two quota failures and a passed native-result baseline
(`vrun-0d91d741-3824-4d05-b198-d9420b444af7`). Forcing all fixed-map fingerprints to zero preserves
all three tests (`vrun-f62440c4-dfeb-46ce-b485-fc2ecbf64b94`). Deliberately collapsing this cache's lookup
and publication keys makes the baseline report match where native requires
fallback (`vrun-5e661bde-b7dc-4562-b1b1-50ba53708de3`); the storage quota test also detects the reduced
storage. Every experimental file restores byte for byte, and the restored suite
passes (`vrun-c4fbabbd-6bf5-45e8-9f25-2a6c8b51e572`). This is not a separate same-pair reversal mutation
proof or an isolated reporting-path fee proof.

Five integration files covering enum behavior, relation recovery, reporting and
key identity pass (`vrun-417f72c5-793e-4b67-b8b9-803493694f26`). The new 32 queries and all 138 existing
enum-relation queries replay (`vrun-d87f2b8b-7848-4489-bae7-2e5d68740d7b`); complete new captures and
expanded input hashes remain unchanged. Formatting passes (`vrun-49f1c999-71fc-4170-bae0-ca6419c82420`). Full
all-target Clippy passes on the current source (`vrun-2bf66b9f-bb61-409f-ad70-2af364f9a5f6`), closing the
post-helper/enum lint gap at this checkpoint. It does not replace final combined
validation after remaining production changes.

Scoped proof: .cc/temp/completion-next/enum-cache-proof-complete.json and
enum-cache-experiments-proof.json; current Rust/Cargo input hashes are in
enum-cache-green-input-hashes.json. The read-only
remaining-owner-refinement-2026-10-16.json identifies variance cache/marker/vector
work, late-symbol storage and the actual anonymous-instantiation caller of the
remaining source declaration projection. Those leads are not implementation proof.

Cached error/completion copies, selection/catalog work, remaining containers,
loops and recursion paths still require closure. Natural CreatedProperty shared
Arc copying and direct for_each_import=false witnesses remain unproven. Work,
allocation-byte and depth defaults are still unbounded; finite calibration,
stack/drop/join/queue measurements, a fresh release corpus and final combined
registered gates remain open. T/Q3/Q4 is incomplete.

## Late symbol cache and ordered late tables — 2026-10-16

The raw-to-late symbol cache uses the existing FixedMap with exact
NativeSymbolIdentity keys and label symbol/late-cache. Provisional raw declaration
publication, name acquisition, parent forcing, and final raw-symbol fallback keep
their original order. The sixteen existing native computed-accessor cases provide
a passed baseline (`vrun-a48c3edc-cc72-4a09-abe0-055920b7ecfc`), two intended missing-accounting failures
(`vrun-f8ba67f3-86cf-4006-ade7-0e254496bd13`), and three GREEN tests (`vrun-6c6719af-b0a7-4f99-9286-b87d07863e7d`). Independent omission of
only this owner's fees, retaining observations, breaks exactly the work/storage
quotas (`vrun-e90757fb-1809-456b-ab0a-dafec548e1dd`). Minima use N distinct native properties, not an
unproven assumption of 2N raw getter/setter identities; each quota checks under,
at and over its boundary, repeat usage, exact final type and healthy successor.

The first twelve nested-setter identity cases passed native and Rust, but used
void on their first operand. Native checkVoidExpression defers operand checking;
collapsing keys therefore passed (`vrun-7c29802b-6dd1-44a6-8be9-7e78fa76a382`). That experiment
is preserved as a rejected identity witness. A further twelve cases call an
overloaded function to acquire the first setter parameter before the final read.
Native passes (`vrun-86349c17-4444-40a4-bd53-cc1dfabacf92`); the combined 24-case Rust baseline
passes (`vrun-1cc9a0a2-4c0f-4aad-ad04-2bae906bb612`). All fixed-map fingerprints forced to zero
preserve results (`vrun-6f1e774f-e48b-45b5-8ac1-88a7ba980a69`); collapsing raw keys swaps 61 and 62
in both acquisition orders while the equal-type control passes
(`vrun-cf6bd21d-3c2e-491c-9d31-608d55704477`). Every experimental file restores exactly and both
restored suites pass (`vrun-5bb72180-1415-4b58-84f2-fddd72d5e48f`, `vrun-13cd1b95-b0c7-4ebd-b0ab-02a6cba81812`).

The raw fallback has a separate 16-case witness: immediate checking of a class
expression consumed by an overloaded call, 16/64 broad computed methods,
instance/static and four option profiles. Native checkObjectTypeForDuplicateDeclarations
calls getSymbolOfDeclaration, whose getLateBoundSymbol stores the original raw
symbol when the broad name cannot be late bound. The initial comma-form fixture
produced native2695 (`vrun-6d27bd39-b0e2-40df-aba5-10d7b25d45d2`); its rejected inputs/capture
remain in scratch. The call-consumed fixture was frozen only after native passed
(`vrun-a357fae6-f67f-4ea1-bd0a-73d38b6755dd`). Rust passes all final values and independent
work/storage quotas (`vrun-ce69a73b-0873-4900-b9c5-f08fe7f10715`). Fee omission breaks only two quotas
(`vrun-c45f365f-5b93-4597-be05-6673dd048df8`); explicitly denying the raw fallback breaks the
public-query baseline (`vrun-fa90f30a-0635-4089-a31d-ca46a9da761b`). Exact restoration and the
three-test rerun pass (`vrun-baac9d91-2883-4627-9039-478069f60e3f`).

LateSymbolTable now owns its ordered entry array, checked growth and final shared
publication. Early/late probes use the existing NativeStringValue canonical_equal
with the request controller, preserving exact byte and opaque-identity rules.
Property-key Clone only retains immutable Arc payloads; this change does not
invent deep-copy charges. Nil/present-empty selection, first-entry retention,
conflicting fresh symbols, index merging and acquisition order stay unchanged.
Entity and index lookups also pass the request controller to that shared owner.
The first GREEN attempt exposed those two missed callers as compile errors
(late-symbol-table-green-compile-failed.json); that is not the behavior RED.
Five independent lookup/growth/publication quotas fail for missing interruption
with baseline passed (`vrun-b5ff520b-e727-4e18-9179-c741ad4f6664`), then all six pass
(`vrun-654bfc4f-c4de-4111-aa9e-5b3089290ba3`). Independent lookup, growth and publication fee omissions
preserve observation scopes and break precisely their respective quota tests
(`vrun-1f67938b-6bcb-45f1-b7f7-5a8ab3157ba5`, `vrun-3bfde8aa-caff-4f21-b393-1d81e63520bb`,
`vrun-8d99cdad-6c40-420c-ad68-e558ab33526f`); exact restoration passes all six
(`vrun-7f4925ec-86f7-4392-bed0-8c135a77f5fd`). The literal accessor matrix isolates these fees;
early-conflict/index branches are covered by compatibility, not separate fee
isolation claims.

All three cache/identity/fallback resource files replay on the combined source,
and five integration files covering late symbols, generated keys/order, private
access and global merges pass (`vrun-9241d30a-2b5d-4684-9d6f-ee48e1b703bc`). Eight native harness files
replay the new fixtures, accessor matrix and existing late-symbol tables, owners,
metadata and meanings (`vrun-588008d9-6052-4046-af8c-1daedc542a81`); frozen expanded native input hashes
remain unchanged. Formatting (`vrun-10ee0699-e47c-40a0-85d9-e69efd44f782`) and full all-target lint
(`vrun-0db8df39-3b9e-4e1d-94cc-39d275ee5879`) pass. Proofs and exact Rust/Cargo hashes are in
.cc/temp/completion-next/late-symbol-and-table-proof-complete.json,
late-symbol-and-table-green-input-hashes.json and its linked experimental proof
records. The original cache fee-omission receipt remains separately recorded.

These are scoped accounting proofs. Cached error/completion payload copying,
initial failure construction/observation, raw native table backing, variance
state, selection/catalog and remaining loops/recursion still need closure.
Natural CreatedProperty shared-Arc copying and direct for_each_import=false
witnesses remain unproven. Finite production work/byte/depth caps, calibration,
stack/drop/join/queue measurements, fresh release comparisons and final combined
registered validation remain open. T/Q3/Q4 is incomplete.

## Active string-mapping application depth — 2026-10-16

applyTargetStringMappingToSource now enters the existing request depth guard
before descending through mapping.target. The guard stays alive through inner
application and outer mapping construction, then unwinds through the existing
owner. Mapping acquisition and factory identities are unchanged; no production
limit was selected by this increment.

The first native64/Rust baseline used comma chains to warm every alias
(`vrun-d7ada8e7-e8cf-4a64-a346-973ec67af9c4`, `vrun-016d1a86-2c5d-4b87-86f4-1ba44a92319b`). Its depth assertion failed both
before and after the tentative guard. The diagnostic run `vrun-847275db-d69d-4da3-acaa-07b0172dad19`
showed a 16-layer case with flat/nested peak23 and a cap31 returning Found43.
The tentative guard was restored to the exact pre-change hash before trying a
new witness. Those initial receipts are preserved and are not depth proof.

A separate frozen subfixture consumes a flat array of warming calls before the
final mapping call, preserving the first64 captures. Its native64 cases pass
(`vrun-b7bcfc4f-2437-4b3a-a7a0-232ba44b1115`): 16/64 layers, alternating mappings ending
in Uppercase/Lowercase, matching/nonmatching strings and four option profiles,
with equally long independent one-layer controls. Rust passes all128 native
baseline results (`vrun-404ee2c3-4bd1-4db7-84d8-d75151309a55`). The 16-layer cases retain whole-request
under/at/over boundaries; the 64-layer cases isolate recursive application by
requiring interruption at a cap that permits the matching flat input.

The wide missing-depth RED `vrun-992e3d63-dd65-42a4-a85a-c707aebdc47c` returns a selected native result at that
cap, rather than the required SemanticDepthLimit. With the guard, both tests
pass (`vrun-5b9bed4a-88a0-497e-94ae-2da9dce54dba`). Omitting only this guard's depth increment, while retaining
its work(1)/poll, breaks the depth test with the native baseline still passed
(`vrun-b300954f-4155-459e-9e80-5f5d91fa5466`). Source restoration is byte-exact and both tests rerun
GREEN (`vrun-509dba77-4f13-4bce-8923-34debae704ad`). The tests assert exact final type and no issues,
under/at/over active-depth boundaries, zero depth after successful completion,
and a healthy subsequent query after each attempted operation.

Four template integration files pass (`vrun-2ff4da45-1371-45a7-8b55-72e74cc0dcd1`); both new native
fixtures and existing completion/matching harnesses replay with expanded input
hashes unchanged (`vrun-db494a90-9f95-4efd-8754-60500501c39f`). Combined formatting `vrun-10ee0699-e47c-40a0-85d9-e69efd44f782` and full
all-target lint `vrun-0db8df39-3b9e-4e1d-94cc-39d275ee5879` pass after this change. Evidence is in
.cc/temp/completion-next/mapping-depth-experiments-proof.json and the combined
late-symbol-and-table-green-input-hashes.json. This proves one actual recursive
path's logical accounting. It does not prove other recursion cycles or physical
stack/unwind/drop/join/queue bounds. Finite production limits, remaining evaluator
coverage, current release measurements and final combined delivery gates remain
open. T/Q3/Q4 is incomplete.

## Full bound-symbol declaration projection — 2026-10-16

Source::symbol_declaration_references now uses the existing controlled collect
operation for the full validated Symbol.Declarations array consumed by
getObjectTypeInstantiation. Binding's existing validated-facts owner finishes
before this projection starts; failures still take the same binding_failure_at
path. Every native record, including nonmember entries, stays in its original
order. The source operation receives its existing request capability; immutable
Project data and identities gain no controller.

The native16 fixture instantiates 16/64 distinct generic anonymous aliases at
global or generic-function scope under four strict/noCheck profiles. An overloaded
call consumes a flat array of those typed expressions before the final literal.
Native (`vrun-d6a56e07-36a5-4cb4-8188-66a5731795fe`) and Rust (`vrun-5e4aac5d-cd7b-4c3a-afd9-bf326c71c76b`) return the exact
literal17 without issues. Before implementation, the baseline passes and both
quota tests fail on missing interruption with zero projection fees
(`vrun-e33e1a05-5314-4127-b516-5abcd8879c94`); after implementation all three pass (`vrun-1c8d3b4e-a9b5-405a-9ddd-70214c3c2e74`). Work and
storage minima derive from N projected records and DeclarationReference's
payload size. They exclude binding's separately observed validation/copy fees,
check under/at/over total boundaries, repeat usage and a healthy subsequent query.

Omitting only this projection's fees while keeping observation intact breaks
exactly the two quotas, with native results passed (`vrun-5bfe7e4c-9a43-44c9-806d-b80491086ca5`).
Denying the source projection breaks the public-query baseline
(`vrun-8885b379-df1e-4188-89ec-05d9ee00cbc9`), proving engagement of the Bound TypeSymbol path whose
only caller is object_outer_parameters_completion. Experimental source restores
byte for byte; the three tests pass again (`vrun-1f1fd5eb-045b-4d65-88e6-b7c6d96d6808`). Three anonymous,
function-symbol and conditional-relation integration files pass
(`vrun-846e9bc2-9c38-49c7-ab71-721e97dc8c37`); the new fixture and three existing native harnesses
replay with expanded input hashes unchanged (`vrun-8e4511ea-5c2f-4d9d-8eec-14daf9ced2cc`). Combined
formatting `vrun-10ee0699-e47c-40a0-85d9-e69efd44f782` and full all-target lint `vrun-0db8df39-3b9e-4e1d-94cc-39d275ee5879` cover this final change.

Evidence: .cc/temp/completion-next/source-declaration-projection-experiments-proof.json
and the combined late-symbol-and-table-green-input-hashes.json. The Property
filtered projection, deferred/mapped singleton arrays, outer-parameter cache
payload copies and other anonymous operations remain separate. This is not a
claim of whole evaluator coverage or completion. Finite production caps,
physical/service calibration, current release corpus comparisons and final
combined registered gates remain open; so do the prior natural shared-Arc and
for_each_import=false witness limitations. T/Q3/Q4 is incomplete.

## Source-free recovery observation — 2026-10-16

SemanticError::observe_recovery now receives the existing request capability,
borrows the original path lazily, and uses the established exact OS-path and
Location copy owners. It checks each retained cause during observation. Typed
source-free primaries and children are bound at the actual consumer; located
primaries, declarationless global evidence, cause order, omission metadata and
classification remain intact. The dispatcher preserves an already acquired
whole failure before attempting this bookkeeping. No controller is stored in
public errors, immutable identities or Project.

The first 32-case native matrix used repeated identifier callees and passed
`vrun-0190716b-9278-4555-8dba-2ac4f1c1ac48`. The initial Rust baseline
`vrun-b6f1f600-ab0f-4b47-960f-02bcec82cade` disproved its one-cause-per-call hypothesis:
the 16-call case retained 16 causes and 131037 omissions. The additional native
trace `vrun-ebafa8e7-5505-4a92-bdd7-6b884a164cd0` records effect-signature recovery, with 3 singleton
return acquisitions and 240/912 expression/type events at 16/64 calls. These event
counts are not a count of transitive cached dependencies. The design explicitly
permits multiplying cached acquisition evidence and OutputLimit when exact
omission metadata overflows. This rejected quota witness and its frozen inputs
remain preserved; no semantic deduplication or count clamping was introduced.

A separate direct-callee fixture removes identifier flow history while retaining
the same source-free singleton. Its 32 cases cover 16/64 calls, four option profiles,
and ASCII/Latin/BMP/astral file names and source prefixes. Native passes
`vrun-2f467676-021d-481f-a390-d262f1a9ddfd`, yielding literal 17 with no diagnostics or demand
failures. A test-only eager then_some subtraction failed at width 16
(`vrun-5feaae6b-c582-4ca3-a48d-97707db53f41`); it was fixed before the accepted Rust
baseline `vrun-d1c3b036-d7fc-4db6-8658-0c1a1558b998`. The baseline asserts the directed InvalidSemantic
outcome, exact UTF-16 ranges, ordered retained causes, omission count and healthy
subsequent query. That test arithmetic failure is not an accounting RED.

Before production changes, five independent quotas fail on absent work/storage
charges while the baseline passes (`vrun-2051ddca-d0fb-4be5-978f-40d4d79b2cd1`); the implemented owner passes
all six tests (`vrun-6f928461-bf42-4be3-bbcd-6e9cf492edf1`). Path minima use each actual canonical OS-path byte
length; location minima use native-fixture character and UTF-8 lengths. Cause
scan work has its own observation scope. Every quota covers under/at/over total
boundaries and repeated usage. Independent omissions retain observations and
break exactly path's two quotas (`vrun-aec0c529-bd02-4e88-b6aa-4c7ade8d4d40`), location's two quotas
(`vrun-ec1b3593-abcd-4a88-9fbd-0ed1b3695b1d`), or scan's one quota (`vrun-8d03c93d-5553-4bc1-8ffc-6c224af2a340`). Denying
primary binding breaks the final-query baseline (`vrun-52efa5d2-0e6e-4e5c-b921-d93d0780ef58`). Exact
restoration passes all six (`vrun-49fa1e65-3700-4103-9c0c-f1cbd749985c`).

The unknown-signature provenance integration tests did not engage an unbound
child: denial `vrun-bd62dcbd-1fd7-4cf9-9251-2d2e91fde579` passed, and restoration
`vrun-54b66081-f93c-4b36-8be1-6cca6409d227` passed. The native relation-recovery fixture did
engage it: an earlier located unresolved property precedes the source-free
relation-depth event. Denial `vrun-7e013489-2897-49cc-a0ef-8cc71444eb8f` fails at
strict/before100; restoration `vrun-390d8b15-a0b9-4a1d-8e66-505476a80a19` passes.
The 27-case native relation matrix passes `vrun-38c4ac51-92ff-4ebe-9bf4-8c19fd5236a5`. New
resource tests use its before99/before100/before101 cases under three profiles, preserving
the native limit's distinction from a product resource interruption and retaining
a healthy successor. Test-only scopes separate child-copy fees from primary
copies and scans; stripping those observations proves production statements are
unchanged. All three child tests pass (`vrun-c2d66a1a-e53b-4c1a-98c2-56c454f004eb`); omitting only
the child's copy fees breaks both quotas with the baseline still passed
(`vrun-ff9162fc-2d63-42ec-bd0c-b7b86a249940`). Exact restoration and the child and primary
reruns pass (`vrun-40f307cc-fdc2-44ab-b06c-b538876e8786`, `vrun-fed682de-4912-404f-85b9-f50eedac76c9`).

Supplemental worker-boundary fault injection covers six whole semantic failure
families at zero work/byte budgets (`vrun-5147ae39-d1eb-4d6b-b0a5-fbcfebacd90e`). Removing only the
precedence guard replaces the acquired Internal failure with SemanticWorkLimit
(`vrun-d0e52c30-da92-4883-8885-b40beacfa7ac`); exact restoration passes
`vrun-d1c1cb95-9d92-4e4d-a2cc-d2e6b537b93e`. This is a synthetic precedence proof, not a claim
of natural terminal-cache reachability.

Existing gap quota/provenance files pass (`vrun-176c3399-de7b-4e3d-aa37-cec7441a6b5d`,
`vrun-b09651ad-b139-4c36-8e48-5bb52fc72794`); five recovery, entity, private-access,
constant and unknown-signature integration files pass (`vrun-9dd73dc2-45f4-4b64-b177-2b53d37518cc`).
The bounded non-resource library partition passes `vrun-245ade10-b9ca-4f9e-b480-c91fb2042ceb`:
139 listed tests, 138 active, including
9 private provenance tests. The pre-existing ignored
program diagnostic remains ignored. Binary listing was inventory only; execution
used registered validation. Only the resource test-module declaration was
temporarily gated, its exact bytes restored in finally, and the full binary name
inventory re-established after the scoped control
(`vrun-79cd9033-f16d-4e1c-94f7-4495ffacddfd`).

Seven native harnesses replay against the expanded manifest, including all their
fixture roots (`vrun-3e0c6b06-9088-46ff-ab03-75c675e90b98`); formatting `vrun-65d002fc-013c-4b72-981f-2c4fe0ee6eed` and
full all-target lint `vrun-6087fd3b-b40d-4d1d-8663-4cdc5b5976ea` pass on the final source. Proofs and current
Rust/Cargo hashes are in .cc/temp/completion-next/recovery-observation-proof-complete.json,
its linked experiment records, recovery-observation-child-native-input-hashes.json,
and recovery-observation-green-input-hashes.json. The earlier limited input
manifests and rejected experiments remain point-in-time evidence.

This closes the observed primary/child metadata copies and retained-cause scans.
The location_file scan that finds the captured path remains unaccounted; checked
path copying does not establish coverage of that lookup or its filename equality.
Cached DemandError/NativeCompletion deep copies, initial error construction and
sequence appending remain separate. Remaining tables, variance, selection,
containers, loops and recursion still need coverage. Natural CreatedProperty COW
and for_each_import=false witnesses remain open. Production work/byte/depth caps
are still absent; physical/service calibration, fresh release comparisons and
final combined gates remain required. T/Q3/Q4 is incomplete.

## Installed signature-return completion copy — 2026-10-16, proof in progress

The frozen direct-callee fixture reaches the Installed branch of
getReturnTypeOfSignature. Denying only that replay produced its expected probe
diagnostic in all six observation tests (vrun-346a37b1-2306-43f8-b3e6-2fe4e2428f7f);
exact source restoration passed all six (vrun-a66f7041-2e03-4c30-995d-28b8333b8828).
This establishes branch engagement, not accounting.

The new final-query baseline passes before implementation. Five independent
quota tests fail for absent message work/storage, payload-shell work/storage,
and SemanticError box storage (vrun-0b21a31c-58bb-427b-8591-4886e0f5ed02). They use
the same 32 native-derived outcomes, exact ordered causes and healthy successors.
No import/setup failure is counted as RED.

A controlled fixed-value NativeCompletion copy and its private recovery-copy
owner are now installed for this one replay branch. They reuse the established
semantic/location/text copy owners, charge both boxes, and preserve the optional
TypeIssue and failed-property identity. Whole failures are excluded by the
private completed-value invariant; this does not change outer Err transport.
GREEN, supplemental field checks, independent omissions and compatibility/final
gates are still pending at this entry. Publication, other cache copies, initial
construction and sequence appending remain separate. T/Q3/Q4 is incomplete.

## Return-copy and source-lookup proof update — 2026-10-16, final gates pending

The source-free installed-return copy passed corrected GREEN and its independent
shell, semantic-box, text and routing omissions; completed-return-fees-proof.json
records exact restoration. The separate located-return matrix passed its native
and Rust baselines, natural primary/child branch denial, eight quotas, four
independent field-fee omissions and replay-routing omission. Its restored Q3 gap,
source-free, supplemental, observation, child and precedence controls pass;
located-return-fees-proof.json records the receipts. Optional TypeIssue remains
supplemental-only. These proofs do not establish every completion/cache path.

The previously open captured-source scan now delegates to the existing exact
byte-equality owner. A new 64-case native matrix varies four Unicode families,
four option profiles, early/late targets and 16/64 unrelated files; early targets
also follow a distinct same-byte-length decoy. Native initial/frozen passes are
vrun-1990f705-c310-4832-931b-196f1876c7bc and
vrun-a15acf0f-c486-4b77-8fe7-418592a15872. The exact file-order and final-query
baseline passes vrun-c9dbdad5-ee9e-406b-bdfe-5a141af13dbb. Two absent-work tests
fail as intended before implementation (vrun-f02ce14b-c425-4d13-9033-79579eaf6d5f)
and pass afterward (vrun-33124476-6f66-4aa1-85db-4190758b0f7b). Visit and comparison
fee omissions independently fail only their own quota; control-routing omission
fails both while the baseline passes. Exact restoration and related controls
pass; source-lookup-fees-proof.json records the full sequence. This closes the
previous lookup accounting hole at the behavior-test level, pending final gates.

The located matrix also naturally reaches recovered signature-return publication:
denial vrun-3b1feef6-ccdb-4a5b-a4a4-560d3c6098db engages all nine tests, and exact
restoration passes vrun-530df419-752b-4e3f-b29e-02f09fc483aa. Test-only inclusive
observation separates publication charges from replay. A missing Control import
was a preserved compile-only setup failure; corrected observer RED
vrun-93741208-3225-4f43-ae6c-3eeeb97d809f fails only the two intended assertions,
and GREEN vrun-6f98bec5-2122-4d1c-9ead-4c19b2c89096 proves nesting, selection,
work/byte counts and unwind restoration without changing production accounting.
Publication's final-query baseline passes while its two missing-fee quotas fail
(vrun-152200a1-d55e-4900-8cff-632720f5293f). The successful publication arm now
uses the same controlled copy and passes vrun-163ff9dd-2cc9-48f1-aaaf-641387e5b203.
Its isolated routing omission and restored compatibility replays are still
running at this entry. Outer Err publication and other caches remain separate.

Current combined compatibility/native replay, bounded private-provenance library
validation, formatting and full lint remain pending. Earlier full lint predates
these changes. No production work/byte/depth caps or whole-evaluator calibration
claim is made. Whole T/Q3/Q4 remains incomplete.

## Installed-return copy and captured-source lookup — scoped closeout, 2026-10-16

This entry supersedes the pending proof status above. Installed signature-return
replay now copies its fixed native value and recoverable DemandError through one
controlled operation, preserving public/private provenance and both payload
boxes. NativeCompletion's private representation excludes whole failures; its
producers, sequencing and current observation callbacks were inspected. This
invariant does not apply to outer Err caches, which are not migrated here.

The source-free final-query matrix passed its baseline while five fee tests
failed for missing text work/storage, shell work/storage and semantic-box storage
(`vrun-0b21a31c-58bb-427b-8591-4886e0f5ed02`). The first implementation attempt failed only to compile a
supplemental assert_eq on a non-Debug identity; that receipt is preserved and is
not RED evidence. A boolean identity assertion fixes the test without adding a
production trait. Corrected GREEN `vrun-30e05fd7-70c8-4b15-802b-2a2b5f7dda11` and supplemental
metadata checks `vrun-4b7d76da-5b3a-4f20-950b-238a035a0399` pass. Independent omissions break exactly the
shell pair (`vrun-c48c3236-85d4-4df8-beca-1374136a7adf`), semantic-box test
(`vrun-05831100-5c50-477c-9f67-55bf3d474768`) or text pair (`vrun-038c6b9c-8139-4153-84a5-8fc7ac313a5b`);
reverting only replay routing breaks all five (`vrun-6c27ccd7-907f-47dc-a17e-fa7af97a1b51`).
The baseline remains passed in each omission. Exact restoration and prior
observation/child/precedence controls pass; completed-return-fees-proof.json
records all receipts. Supplemental metadata tests cover counts 0/15/16/17,
classification, exact omission arithmetic, Unicode locations/messages, optional
TypeIssue, read/write failed-property identity and non-UTF8 OS paths. Optional
TypeIssue remains supplemental-only, without a natural completed-replay witness.

The new native-located-return-copy-resources fixture establishes 32 cases:
16/64 direct calls, four compiler-option profiles, and four Unicode families.
Native initial `vrun-94d331c8-022f-4949-b33b-b788022b6f4b` and frozen replay
`vrun-2639d022-be2c-45e8-a898-541fb76301dc` yield literal 17, exactly two unresolved-name
diagnostics, and no demand failures. The Rust baseline `vrun-7650a2da-a2a8-4caf-865b-07a27dc5c404`
asserts the documented InvalidSemantic result, exact primary/child declaration
ranges, alternating missing-name acquisition order, 2N-1 causes before the cap
of 16, exact omitted counts, and a healthy subsequent Q4 query. A natural-branch
denial `vrun-151770a2-c60e-452b-9f63-9298f63fd312` proves both located primary and child
metadata reach the controlled replay; exact restoration passes
`vrun-eb3a3eba-c088-477b-8528-1d9ae2b4657b`.

Test-only observation scopes separate cause buffers from copied locations;
removing only those inserted lines reproduces the prior runtime statements.
Eight located field quotas and the baseline pass `vrun-a2aafdf0-d91f-4974-a3c7-f478ecbee1ee`.
This expands proof of the existing controlled copy; it does not claim a new
missing-behavior RED. Independent text/path/location/cause omissions break only
their two work/storage tests (`vrun-d823d26f-82fe-4098-931d-2ee9a6a44d38`,
`vrun-686a02ca-a014-49e6-88a2-ac548ea6e933`, `vrun-91d16d1d-b60a-40d6-b4ff-37e7e04fbf4e`,
`vrun-5147e2ea-f96c-49d3-adb8-9d5dbeb19f1c`). Replay routing omission breaks all eight
(`vrun-2e507e28-6064-4fb0-b948-e92f5f1add4a`) while the baseline passes. Exact restoration
passes `vrun-af48f34d-0e71-47e9-872d-03923aea1497`, followed by source-free, supplemental, Q3 gap,
observation, child and precedence replays. located-return-fees-proof.json records
these results. Path minima use actual canonical OS bytes; text/location minima
use fixture characters/UTF8; cause minima use retained count and payload size.
Every quota exercises under/at/over total budgets and deterministic repeated usage.

The previously open location_file scan now uses the existing exact equal_bytes
owner. That owner already charges each candidate before its length/shared-storage
shortcuts and bounds equal-length byte comparisons. No duplicate visit fee,
normalization, new cache or controller on Project is introduced. Captured order,
first matching source, lazy path observation and the missing-source invariant
remain unchanged. Both initial located errors and recovery observations use this
one association operation; initial error payload allocation remains separate.

The source-lookup fixture has 64 native cases across four Unicode families and
four option profiles. The target precedes or follows 16/64 unrelated files;
early cases also place a distinct same-byte-length filename before the target.
Native initial `vrun-1990f705-c310-4832-931b-196f1876c7bc` and frozen
`vrun-a15acf0f-c486-4b77-8fe7-418592a15872` establish exact program file order and
unchanged literal/diagnostic/demand outcomes. The Rust baseline
`vrun-c9dbdad5-ee9e-406b-bdfe-5a141af13dbb` verifies that order, exact directed failure
provenance and healthy successors. Independent visit-work and filename-byte
quotas fail before implementation (`vrun-f02ce14b-c425-4d13-9033-79579eaf6d5f`) and pass after
routing to the shared owner (`vrun-33124476-6f66-4aa1-85db-4190758b0f7b`). Wide late matches
make visits dominate; early matches make equal-length comparisons dominate.
The observed-work upper bound also requires stopping at the first match.
Omitting only visit fees fails only the visit test
(`vrun-f2c7cc0d-483c-4351-bfd3-f8b37ed45b1b`); omitting comparison fees fails only the
byte test (`vrun-e0bb1fa6-af18-42d9-920f-455b10304383`). Removing only lookup's control
routing fails both (`vrun-871299ad-aa11-489b-a032-7dad2701744f`). Observations remain
installed throughout. Exact restoration and located/observation/child/terminal
precedence controls pass; source-lookup-fees-proof.json records all results.

Successful signature-return publication is now migrated as well. Denial
`vrun-3b1feef6-ccdb-4a5b-a4a4-560d3c6098db` engaged the recovered publication in all nine
located tests; exact restoration passes `vrun-530df419-752b-4e3f-b29e-02f09fc483aa`.
The existing exclusive observation scopes could not separate this site from
nested shared owners, so a test-only inclusive scope was added. A missing Control
import caused a preserved compile-only setup failure; corrected RED
`vrun-93741208-3225-4f43-ae6c-3eeeb97d809f` fails the two intended nested-charge
assertions while the existing exclusive and selection controls pass. GREEN
`vrun-6f98bec5-2122-4d1c-9ead-4c19b2c89096` proves work/byte nesting without double
counting, selection, exclusion after scope exit, and restoration through unwind.
The observer module and fields remain entirely absent from production builds.

The publication-site scope contains no production charge; replacing its inserted
block with the prior arm reproduces the exact prior runtime text. With that
observation installed, the final-query baseline passes and independent work and
storage quotas fail on zero publication charges (`vrun-152200a1-d55e-4900-8cff-632720f5293f`).
Minima derive from one publication's two missing names, path/location payloads,
retained child and two boxes. Existing replay charges cannot satisfy this site.
Replacing only its successful cache copy with the established controlled owner
passes `vrun-163ff9dd-2cc9-48f1-aaaf-641387e5b203`. Reverting only that routing, retaining all
observations, fails both quotas and leaves the baseline passed
(`vrun-6ae20b5e-0fd0-463a-bddc-d3f2988758d8`). Exact restoration passes
`vrun-7f8e59bd-e2f6-4785-8bac-11ed6592e48a`; exclusive observer controls, source-free/located
replays, source lookup, child binding and terminal precedence all pass afterward.
signature-return-publication-fees-proof.json records these results. Outer Err
publication is unchanged and remains separate from completed-recovery semantics.

On the combined source, three remaining resource/provenance files and seven
integration roots pass (see return-and-lookup-proof-complete.json); six native
harnesses replay against source-lookup-native-input-hashes.json
(`vrun-35386af6-ca6f-4e74-b4be-01210e4f27fd`). The bounded non-resource library partition also passes
`vrun-7bdcba0e-199e-4262-9a71-f58132ee349b`, including private provenance tests; its exact
module declaration is restored, a scoped control passes
`vrun-481955ec-1070-4663-b199-e14f8b44e51b`, and the complete test-name inventory
is restored (return-and-lookup-nonresource-proof.json). Direct binary listing was
inventory only; all test execution used registered validation. Registered
formatting `vrun-40aef365-aa26-4e92-9c19-a3f185459f83` and full
all-target lint `vrun-744fbd05-21ca-421b-b9a6-daa913059af9` pass. Current Rust/Cargo inputs are frozen in
return-and-lookup-green-input-hashes.json. All experimental mutations are restored
and no ACTIVE-OMISSION marker remains. Earlier manifests and intermediate
failures remain point-in-time evidence, not claims about final whole coverage.

Only installed signature-return replay, successful publication and this source
association are closed. Other completion/outer-error copies, initial errors, sequencing,
remaining tables/variance/selection/containers/loops/recursive paths and the
natural COW/importfalse witnesses remain open. Production work/byte/depth caps
are still absent. Current physical/service calibration, fresh release corpus
comparisons and final combined gates remain required. T/Q3/Q4 is incomplete.

## Variable-value cache accounting — behavioral proof, final gates pending

The previous located-return fixture engaged recovered value-cache publication,
but not recovered value replay or either generic-demand cache branch
(general-completed-cache-branch-proof.json). These unengaged trials do not prove
coverage. A separate native-value-cache-copy-resources fixture initializes one
variable from a located union return and reads it 16/64 times, across four Unicode
families and four option profiles. The first native candidate omitted TS2695;
its failed receipt and capture are retained. Pinned checkBinaryLikeExpression's
comma rule explains that diagnostic for each bare side-effect-free operand.
Corrected `vrun-ed1e730a-1d6c-4214-80cf-7e024f60f3cd` and frozen
`vrun-c7dbf801-cd8d-4702-b01f-0c6d81fd9212` native runs pass all 32 cases: literal 17,
no errorType or demand failures, two unresolved-name diagnostics plus N TS2695.
The Rust baseline `vrun-ac85f332-ab06-4769-b6de-1ca5f87821c1` proves exact directed failure
locations, alternating declaration order, 2N-1 child causes, cap16/omissions,
UTF16 coordinates, exact missing names/path and a healthy successor query.

Separate branch denials engage recovered value replay
(`vrun-fa37a5e7-9ccc-440d-bf9e-31bb71800b48`) and successful publication
(`vrun-253eca9a-2252-4cb9-943d-aa9aae7f4c33`); exact restoration passes
`vrun-718bd7cb-cee7-49e6-a3ae-f5ee52c708b3`. Test-only inclusive observations surround
only the copy expressions, excluding table insertion and other owners. Reversing
those inserted expressions restores the prior runtime copies exactly
(value-cache-observation-refinement.json). The semantic baseline passes while
four quota assertions fail on zero charges (`vrun-39391e90-3c93-4565-8ffb-65e3e5b44603`).

Both successful-copy sites in value_type_worker now use the existing controlled
NativeCompletion copy. Outer Err handling remains separate. GREEN
`vrun-426b546e-44fd-4754-b179-45d9f78d7641` passes the baseline and work/storage quotas for both
sites. Minima derive from the fixture's missing-name characters/UTF8, actual
canonical OS-path bytes, primary/child filename copies, one cause slot and both
payload boxes. Replay requires at least N-1 copies; publication requires one.
Quotas cover under/at/over total budgets and deterministic repeated usage.

Removing only replay routing fails exactly its two fee tests while publication
and the baseline pass (`vrun-478c2cce-aace-4087-8cdd-e2c8af019c65`). Removing
only publication routing fails only its pair (`vrun-9051aafa-cfbf-4e5a-802d-14efdc7de966`).
Observations remain installed; both experiments restore exact source. Restored
`vrun-85dfb703-54a7-4fcc-814e-4b34dd933204` and prior signature-publication, located metadata,
private provenance, observer nesting, recovery observation, child binding and
terminal-precedence controls pass. value-cache-fees-proof.json records these
results; underlying shared copy owners retain their earlier independent fee
proofs. No new shared copy policy or allocation owner was introduced.

This is behavioral proof, not final acceptance. Combined compatibility/native
replay, formatting and current full lint remain pending. Generic-demand and other
cache copies, initial/outer errors, sequencing, remaining containers/loops/actual
recursion, finite production caps and fresh physical/service calibration remain
open. T/Q3/Q4 is still incomplete.

## General-demand cache accounting — behavioral proof, final gates pending

The optional-parameter fixture passes native32 and its directed-failure baseline.
Separate denials prove recovered generic-demand replay/publication in strict
profiles and bypass in all 16 loose cases (demand-cache-branch-proof.json).
Copy-only observations preserve the prior runtime copies. After a preserved
compile-only stray-comment-character failure, corrected RED
`vrun-ed6eab0e-5eca-4c3f-aa57-076df4a91c4d` has four missing-fee failures and both native
profile baselines passed. Routing both successful copies through the existing
controlled owner gives GREEN `vrun-df9c1b1e-8314-4eb5-a18e-71e99e6cb1c2` for all six tests,
including zero recovered-wrapper copy fees in loose profiles.

Independent replay `vrun-17fd44f8-2522-42be-8f31-28da19e46541` and publication
`vrun-a12d8990-6e29-4cc8-98d6-b6b84923b21d` omissions each fail only their
own two quotas; the opposite pair and both baselines pass. Exact restoration and
nine resource/control files pass; demand-cache-fees-proof.json records receipts
and hashes. Cause order, exact UTF16 locations, omission counts, healthy successors
and under/at/over quotas remain proven. No outer Err or recipe/location copy is
migrated. Combined compatibility/native replay, formatting and full lint for this
and the variable-cache increment remain pending. Whole T/Q3/Q4 is incomplete.

## Single-user resource-policy reconsideration — 2026-10-16

The user clarified that the initial tool will be used only by them and asked
whether whole-process memory thresholds could replace the detailed accounting
and verification work. Further accounting instrumentation is held while that
simpler failure contract is considered. This is a design discussion, not an
implemented replacement policy or a completed T/Q3/Q4 delivery claim.

The current variable/general-demand cache batch has passed its focused native,
RED/GREEN, routing-omission and restoration controls. Its combined nine-root
integration run 455cccf6 and six native harnesses 24032cc7 pass. Bounded library
3f70f8b6 passes 138 active tests, including nine private provenance tests; the
existing ignored test remains ignored. The module declaration and 784-test
inventory are restored, with control 7648ceac passed. Initial formatting
750a8773 reported 28 hunks in the allowed files; final format 7a94467e passes.
Full all-target lint 4f548d02-86b9-4eeb-a28d-45cca47c62b7 was deliberately cancelled
during this design reconsideration. It is NOT a lint pass. The finish driver
terminated at its required-pass assertion, and the final closeout script has
not run. The latest full lint pass predates these four cache-copy changes.

Final Rust/Cargo hashes in value-and-demand-cache-green-input-hashes.json and
native hashes in demand-cache-native-input-hashes.json were verified unchanged
after cancellation. All experimental mutations are restored, ACTIVE-OMISSION is
absent, and no writer or validation job from this work is pending. The declared-
cache fixture and three prepared drivers remain scratch-only and unexecuted.
No inherited implementation was rolled back.

A simpler candidate is to retain native cycle semantics and explicit depth
protection, but enforce elapsed-time and physical-memory thresholds at the
process boundary. The existing service cache is rebuildable; terminating it
would discard the active query and warm cache, then allow normal startup on a
later request. Service ProjectRequest::Edit selects edits without applying
writes; application is a distinct CLI operation. This candidate would change
the current same-process survivor and deterministic resource-budget obligations.
It therefore needs an explicit revised contract before replacing those delivery
gates. On macOS, a physical-memory watchdog should be described as a monitored
threshold with possible overshoot, not an exact portable OS-enforced RAM cap.
No memory-limit mechanism or replacement lifecycle has been implemented or
validated by this discussion.

## Accepted single-user process limits — 2026-10-16

The user explicitly authorized replacing exhaustive allocation/work accounting with process-level memory and wall-clock protection, removing the obsolete code/tests, and completing T/Q3/Q4. This supersedes earlier fee-coverage, omission-experiment, exact allocation calibration, and same-process-survival gates. Retain native cycle semantics, actual recursion guards, distinct failures, exact names/provenance, and compiler-derived final outcomes.

The CLI/service query process is disposable on a limit failure. A subsequent service request restarts and rebuilds its cache. File-application operations must not be interrupted by the query watchdog. Physical-memory sampling is a threshold with possible overshoot, not a hard OS reservation. Current proposed defaults are 4096 MiB and 30 seconds; final corpus validation must establish practical suitability.

New process tests: RED compiled and failed all five expected assertions; GREEN vrun-a16ab591-11b5-4e14-b2aa-b6ebc8548340 passed tests/process_limits.rs. Covers direct and service memory/timeout failure, successor recovery, exact direct/service successful result, invalid direct configuration. Full cleanup, semantic regression, final release/corpus and combined gates remain outstanding. No validation is running at this checkpoint.

## Single-user cleanup checkpoint — 2026-10-16

Accepted process policy is installed and obsolete allocation/work ledgers and
observation infrastructure are removed. Standard maps/vectors/copies replace
charged adapters. Native cycle/depth behavior, provenance and baseline tests
remain. Q3 reference_gaps baseline PASS vrun-d06899e5-f149-4ff8-8e6e-e78b5b7a021e
on cleaned production. Six additional identity/import/depth modules and the
flat speculative-parser case were rescued from the former quota suite.
Before snapshot: .cc/temp/process-limits/before.tar.gz (preserves inherited work).
Stable semantic-green snapshot: .cc/temp/process-limits/semantic-green-source.tar.gz.
Intermediate cleanup compile failures are archived; some automatic transforms
were corrected from the before snapshot. Do not rerun cleanup generators: their
input states are historical and several are not idempotent.
SOLE active validation at this entry: exec71983, tests/process_limits.rs, receipt
.cc/temp/process-limits/lifecycle-red.json. Seven tests: original five limits/
restart/config tests plus invalid service configuration and idle cancellation.
Expected new RED is missing service configuration text (currently EOF). Do not
mutate live source until terminal. Then return Response::Failure for watchdog
setup errors in service handle, GREEN, finish cleanup/format/lint, retained
semantic/depth tests and broad final validation. No fee omission experiment is
active; no ACTIVE-OMISSION marker. Delivery still incomplete.

Process lifecycle GREEN vrun-497f0a27-c884-46db-a477-5ac185c15ccb: all seven
tests pass after preserving service watchdog setup errors as Response::Failure
and shutting down the misconfigured service. RED b35286a0 compiled, ran seven
tests (six passed), and failed specifically because the service returned EOF
instead of the invalid configuration name. Earlier lifecycle-red5d875ca7 was
a compile failure, not behavioral RED; repaired one function-valued field call.
Removed nine now-unused controller fields, private Vec/ordering adapters, and
infallible controlled AST walk/module-classification wrappers. Formatting hunks
were applied from registered results (initial EOF-context mismatch repaired;
no direct formatter invoked). These last cleanup changes still need validation.
SOLE active validation: exec99030, registered all-target lint, receipt
.cc/temp/process-limits/lint-1.json. No source mutation until terminal.
Next: fix reported lint/compile issues; rerun process and retained semantic/depth
suites; fresh release/corpus, bounded broad Rust/harness, canonical gates.
No fee-accounting work resumes. Whole delivery remains incomplete.


## Simplified protection: cleaned-source regression checkpoint — 2026-10-16

Registered test vrun-61ecf9ed-dba2-4d7f-8d07-d1c02a7e3e4e passed the cleaned
library plus all seven process lifecycle tests. Captured library output records
176 passed and one pre-existing ignored environment-dependent program-listing
test; the wrapper's “0 skipped” line counts explicit platform SKIP reports and
does not count that ignored test. The retained native reference gaps, exact
ranges/provenance, identity, import branches and recursion-versus-flat boundaries
passed. Four unused parameters exposed by helper cleanup were removed afterward.
Formatting passed vrun-d51a65bb-f9c1-4db9-a829-37bcb113cc28 before that final cleanup.

All-target lint-6 was intentionally cancelled while checking integrations to
reconcile obsolete CLI help and user documentation; it is not a full lint pass.
New docs/types.md describes the implemented query modes and process policy.
The final all-target lint and combined regression gates remain required.

The registered test wrapper now admits src/main.rs for only --bin tsparse.
Its behavior-level wrapper tests failed on the missing scope (15 passed, 2 failed,
vrun-f479bca7-12dc-40cf-b745-f7738a56f551), then all 17 passed after the change
(vrun-679a9a78-55e3-459c-88d8-95ab1a3d984f). The tested minimal wrapper edit was
also installed at the canonical path used by cctl; before/after hashes and its
backup are under .cc/temp/process-limits/canonical-test-wrapper-*.

Candidate defaults are now 6144 MiB and 60 seconds. The older corpus reached
3931.5 MiB RSS and 34.4 seconds wall time, making the earlier 4096 MiB/30-second
candidate too tight. These older measurements justify headroom, not a current
pass. Fresh release and exact paired corpus measurements are still pending.



## Process-policy evidence on the cleaned release — 2026-10-16

The sampled defaults are now 6144 MiB and 60 seconds, with the retained depth
limit of 1024. Release binary `fe8d2a5f8c7a9111fe7d985c256ae7824403cfb9dfe8ff0eab5cb52f802a47bc`
passed 16 direct and 16 service comparisons against the frozen native corpus
(`vrun-d4ea223b-c68c-446f-91f1-96f88c83cf8f`). Direct maximum wall time was
32.626 seconds and peak RSS 3.837 GiB. Service median wall time was 1.252 seconds;
the maximum was 30.491 seconds, and all requests used one generation. The largest
after-query service resident sample was 2.840 GiB; that is not a service peak.
Both corpus services stopped and exited cleanly.

Four additional paired lexical requests passed (`vrun-8d618fa6-01dd-4a88-b95c-ee7c51a33032`),
with direct peak RSS 3.832 GiB. This baseline includes ordinary checker
initialization and makes no member/type demand; its former “no checker” label
was inaccurate. Its script now binds the current completed corpus report instead
of a historical binary report. The initial stale-report rejection is archived
as `vrun-94c3144a-ce72-4e57-8b64-71ebe519067d`.

All eight lifecycle tests passed (`vrun-8c34b4ea-5113-4189-91c6-0f20f02e5dd2`).
The additional live-growth case succeeds for a small query under 32 MiB, fails
with SemanticMemoryLimit for a 50,000-declaration flat project at that threshold,
and then returns the expected type for that same large project under defaults.
This supplements initial denial, deadline, invalid configuration, service
restart and normal watchdog cancellation coverage. No fee-omission experiment
was added. Subsequent edits only move a test comment and remove a blank line.

Obsolete accounting probes are archived with per-file hashes in
`.cc/temp/process-limits/obsolete-probes-removal.json`: 72 oracle files, 64 orphan
fixture directories and two duplicate accessor baselines were removed; shared
fixtures and semantic/depth baselines remain. The archive holds all 632 removed
files. `.cc/temp/process-limits/process-policy-evidence.json` binds measurements,
source hashes, receipts and limitations. Memory sampling can overshoot; this is
not a hard OS cap. Linux monitoring has not been exercised on this macOS host.

Delivery remains incomplete pending the final combined Rust/harness, lint,
format, semantic and release-build reconciliation gates.


## Final harness and static gates complete — 2026-10-16

The complete retained harness replay passed all 352 files in 44 registered
batches: 1,084 executed/passed tests, zero skips. Reports and input hashes are
in `.cc/temp/process-limits/final-harness/`.

The Rust replay found one obsolete assertion in `tests/references_cli.rs`: it
required help to say JSON values/properties cannot be reference targets. The
implemented member-query profile supports those values; selection inside JSON
files remains unsupported. Only that expected phrase changed. The original
failure is `vrun-ea8ba199-21aa-4fd5-a279-59b19ca94941`; focused GREEN is
`vrun-6372d585-26ab-467e-ae8e-13190bfd832d`; the interrupted eight-root batch also
passed on retry. Production, harness, fixture and wrapper inputs are unchanged.
The exact before/after test hashes and receipt are preserved in
`.cc/temp/process-limits/help-assertion-reconciliation.json`; original input
snapshots were retained alongside explicitly reconciled snapshots.

After that test-only correction, final formatting passed
`vrun-37bb8fc1-2d39-4dce-ab31-35c3964acc22`, and all-target lint passed
`vrun-2be637d9-1143-41ac-a705-81fe6baf67c0`. The Rust replay has passed the
library (174 executed, one existing environment-dependent helper ignored),
CLI/editing, member references, navigation/parser, all eight process lifecycle
tests, project/service and initial reference/type-access groups. Two existing
filesystem-dependent cases reported explicit skips because this host refuses
non-Unicode directory/file names. Later Rust batches, the canonical semantic
gate and final benchmark-build reconciliation remain pending.


## T/Q3/Q4 final closeout — 2026-10-16

The full retained Rust replay completed all 284 roots (library and CLI unit
tests plus 282 integration roots) in 37 registered batches. The only test change
during final replay corrected the stale JSON-reference help expectation;
production source was unchanged. Its original failure and passing reruns remain
recorded. Batch 11 was interrupted by a CC connection failure, recovered as
`interrupted`, and passed on retry; that interruption is not a test failure.
Every completed batch after that restart reported zero skips.

The combined result includes the earlier full retained harness pass: 352 files,
44 registered batches, 1,084 executed/passed tests and zero skips. Input snapshots
were verified against current source, tests, tools, fixtures and build inputs;
the help-test-only reconciliation is explicit. The two filesystem skips and one
ignored library helper are identified in the final artifact. No Cargo doctest
execution is claimed.

Final canonical gates passed:

- Format: `vrun-37bb8fc1-2d39-4dce-ab31-35c3964acc22`.
- All-target lint: `vrun-2be637d9-1143-41ac-a705-81fe6baf67c0`.
- Semantic: `vrun-998282f2-1a1b-40ad-b138-26e3e645e6e7`.
- Release binaries/examples/benchmarks: `vrun-3afbeb31-d589-440f-aaf5-faa1b386921a`.

The final release SHA-256 is
`fe8d2a5f8c7a9111fe7d985c256ae7824403cfb9dfe8ff0eab5cb52f802a47bc`,
exactly matching the measured direct/service corpus binary. No production input
changed after those measurements, so the current 6144 MiB/60-second calibration
and exact corpus outcomes remain applicable. The process policy, sampled-memory
limitations and measured service behavior are documented above and in
`.cc/temp/process-limits/process-policy-evidence.json`.

All four historical omission proofs still record `restored: true`, and no
`ACTIVE-OMISSION.json` exists. The validation service has no active job from this
work, and the replay/build/test processes have exited. No experimental mutation
is installed. The completion document, validation index and continuation
checkpoint now record this closeout. No further T/Q3/Q4 gate remains under the
approved policy; this does not mark the broader proposal ticket complete.
