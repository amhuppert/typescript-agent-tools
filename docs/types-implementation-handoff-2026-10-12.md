# T/Q3/Q4 implementation handoff — 2026-10-12

**2026-10-13 continuation:** start with the
[completion correctness checkpoint](types-completion-correctness-2026-10-13.md)
for current fixes, completed replay scopes, corpus results and remaining work.
The [storage checkpoint](types-storage-performance.md) records the separate
performance measurements. This dated guide preserves historical context;
its failure lists and unexecuted test counts are superseded by those later
receipts. T/Q3/Q4 delivery remains incomplete.

Continue the selected native types deliverable in the existing worktree. The
checker and query operations have substantial implementations and scoped
compiler evidence; **T/Q3/Q4 delivery is still incomplete**. The latest completed
increment adds recursive kind-predicate resource checks and independently proves
filter traversal and scratch accounting.

This is a resume guide at the recorded checkpoint. Read the live artifacts and
new user steering before changing code; later status entries supersede this file.

## Start here

1. Read [AGENTS.md](../AGENTS.md) and the current
   [session charter](../.cc/session-alignment/charter.md). The original ticket is
   a proposal, but the user subsequently selected and authorized the native
   T/Q3/Q4 design and repeatedly requested implementation. Preserve that selected
   scope; other proposed queries/edits are separate work.
2. Read the opening and latest dated sections of
   [implementation status](types-implementation-status.md). It is a long receipt
   history: early lists of missing operations are superseded by later increments.
   Read the linked completion correctness checkpoint for the latest evidence
   before using the historical sections below.
3. Read the delivery proof checkpoints in
   [types-design.md](types-design.md#evidence-and-implementation-proof).
   For resource work, read the latest checkpoints and hotspot inventory in
   [types-native-request-resources-plan.md](types-native-request-resources-plan.md).
   Verify each historical hotspot against current code before treating it as
   unchecked.
4. Choose one bounded missing owner or proof obligation, establish the relevant
   compiler control and final-query RED, then implement and validate it. Finish
   that increment with attributed receipts and updated status. A scoped pass
   closes its stated increment, not the whole deliverable.

## Workspace and execution constraints

Recorded worktree:

```text
/Users/alex/github/typescript-agent-tools/.worktrees/ticket-proposal-typescript-navigation-analysis-and-editing-tools-for-agents-f03075
```

Recorded branch:
`csm/ticket-proposal-typescript-navigation-analysis-and-editing-tools-for-agents-f03075`.
Recorded HEAD: `ea23d8bbbb34d5609aa9ba7057b479805c61ca7b`.
At handoff preparation, porcelain status had 833 entries: 81 modified tracked
paths and 752 untracked entries. Entries can represent directories. Much of the
native implementation, tests, fixtures, and documentation is untracked.

**Preserve this dirty worktree. Do not reset it, clean untracked files, or infer
that untracked files are disposable.** Recent increments were not committed,
merged, or published. Stay inside this worktree; Command Center owns session
lifecycle and merging. All tools/agents started by the previous increment were
collected to completion. Machine-wide validation jobs from other sessions may
still be active; they are not this handoff's outstanding work.

Use the current Command Center instructions and the `command-center:cc-cli`
skill for CLI operations. Validation goes through registered `cctl validate`
commands. Wrappers and registration resolve from the canonical project root,
with this worktree as cwd; editing a local wrapper does not update the live
registration. See the project-setup skill before changing that infrastructure.

TypeScript **7.0.2** defines expected semantics. Pinned Go source release commit
is `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`; installations/source live in the
canonical repository's `.cache`, not in this worktree. Read
[validation.md](validation.md) for discovery and comparison rules. The 6.0.3 API
is the syntax-projection comparator, not the semantic source of truth.

Production queries use the Rust checker. Compiler processes and Go overlays are
test-only evidence. Retain exact names, distinct failures, named compiler-rule
owners, and final-outcome tests as required by AGENTS.md. Preserve the selected
scope instead of substituting annotation-only answers, guessed `any`, or
spelling-based member identity.

## What is implemented

### Foundation and native queries

The earlier selected navigation deliverable S/C/Q1/Q2 and editing deliverable
E/E1/E2 have implementations and prior ticket receipts. Their compatibility
still needs repeating after final type wiring.

Current native owners and scoped controls exist for type construction and
presentation, aliases, members, arrays/tuples, generics, calls/inference,
relations, conditional/mapped/template operations, constants/operators, and
flow/read admission. Q3 member references, selection, filters and relationship
grouping, and Q4 bounded type graphs are connected to Project/Session/CLI paths.
This inventory does not establish full profile conformance or arbitrary
TypeScript parity. Consult the later status checkpoints for each family's
exact admitted behavior and outstanding proof.

Key entry points and ownership:

| Area | Where to continue |
| --- | --- |
| Loading and publication | `src/project/program/`, `src/project/binding/` |
| Shared immutable checker inputs | `src/project/binding/native_inputs.rs`, shared checker initialization; see the initialization plan below |
| Type queries and semantic worker | `src/project/semantic/mod.rs`, `Project::type_of` |
| Fresh evaluator, demands and caches | `src/project/semantic/evaluation.rs` and `evaluation/` |
| Native type storage and factories | `src/project/semantic/model.rs` and `model/` |
| Captured source and semantic recipes | `src/project/semantic/source.rs` and `source/` |
| Q3 roots/grouping | `src/project/semantic/reference_search.rs` |
| Product resource ledger/transport | `src/resources.rs`, `semantic/resources_tests.rs` |
| Compiler controls | `tools/type-oracle.mjs`, `fixtures/types/compiler-hook/`, maintained `tools/*.test.mjs` |

### Shared initialization and JSON: preceding completed increment

Loading and each fresh request use the same initializer over immutable raw binder
inputs. The former `QueryBindings` and alternate load-native resolver/cache
import were removed. Loading runs a temporary checker; requests replay fresh
initialization rather than cloning mutable completed checker state. Queries use
captured inputs and perform no filesystem IO. Read
[types-native-checker-initialization-plan.md](types-native-checker-initialization-plan.md)
for the A2 choice and lifecycle constraints; its old staged interfaces are
historical.

The recent JSON work:

- Keeps actual syntax/JSON declaration carriers, duplicate first declarations,
  installed last values, exact property origins, and native widening behavior.
- Uses the shared `getRelatedSymbol` grouping owner for transient/instantiated
  roots while retaining synthetic union identity; origin sets are not IDs.
- Preserves raw Script/Ignored resolution before lexical augmentation tolerance,
  and captures actual module headers after binding. The declaration association
  owner reads ambient/global/augmentation Module attachments too.
- Retains actual ValueDeclaration associations for JSON objects and properties.
  Omitting the object association had caused a spurious missing-Object recovery;
  this was fixed at the declaration owner.
- Preserves repeated native recovery acquisitions, rather than deduplicating
  causes: scalar/array selections have three/five ordered causes; the rich
  seven-array fixture retains a sixteen-entry prefix and eleven omitted entries.
- Proves direct/service agreement, failed refresh generation behavior, and
  subsequent healthy requests in `tests/type_json_initialization.rs`.

The connected eight-target library/JSON/augmentation/member/relation checkpoint
passed in `vrun-33fd3821-c3e4-4461-9f06-aca5ee5e813e`, including all 926
maintained load cases. Earlier library passes do not erase the latest loading
timeout described below. Recovery replay compiler controls passed ten native/API
pairs in `vrun-14196ff2-6fa4-4a9d-a67a-7b4e657eba96`.

### Latest resource increment

`Model::maybe_type_of_kind`, the sole `maybeTypeOfKind` owner, now enters
`resource_depth()` before any flag shortcut. Each actual invocation charges one
work unit and balances live depth. Native early returns, constituent order, and
unvisited tails are preserved. Arc views share payload; this predicate creates
no constituent scratch. Base constraints remain the separate native question.

`resources::Control` is shared within one request across Source/Evaluator/Model.
Its work, cumulative requested allocation payload, checked depth, sticky first
interruption and deadline are independent of native compiler counters and
presentation limits. `RequestAbort` is private whole-request transport; it
abandons the evaluator and never becomes an ordinary predicate `false` or
resumable semantic rejection. Loading supplies no request Control.

Production work/bytes/depth maxima remain `None`, pending calibration; the
default deadline is 30 seconds. Requested payload bytes are not RSS or exact
allocator usage. This increment is not complete resource safety.

The existing shared `TypeFilter` controller and `mapTypeEx`/alias mapping owners
already charge their native traversal/scratch paths. The new proof exposed a
masking defect in the old non-null filter witness: accounted mapping let it pass
even with all filter accounting removed. Preserve that old fixture as combined
filter/map coverage.

New maintained controls:

| Fixture/test | What it establishes |
| --- | --- |
| `fixtures/types/native-filter-only-resources/` and `tools/type-native-filter-only-resources-oracle.test.mjs` | Numeric `!== undefined` branches require eight unchanged wide filters. Their numeric facts return before nullable replacement mapping. Final type is number; eighteen TS2695 diagnostics are diagnostic-only. |
| `fixtures/types/native-kind-resources/` and `tools/type-native-kind-resources-oracle.test.mjs` | Identically warmed early/late symbol unions reject relational symbol operands before later filters/maps. Early visits two predicate entries; late visits 20/260 at widths 16/256, with three native levels. Direct symbol visits one. Numeric misses replay through the distinct constraint wrapper. Final type is number despite four TS2695 codes and TS2469. |
| `src/project/semantic/resources_tests.rs` | New filter-only and kind-width final-query tests; separate paired work/memory cutoffs, exact under/at/over limits, flat-width depth, repeated requests and healthy semantic/lexical successors. |

`inspectKindPredicates` is an opt-in zero-demand compiler overlay in
`tools/type-oracle.mjs`. It records existing fields, actual recursion and returned
booleans; it adds no type getter or identity allocation.

Important attributed proof:

| Receipt | Outcome |
| --- | --- |
| `vrun-e6968de9-e741-4abb-92aa-3e191cec099d` | Old non-null filter gate passes with all TypeFilter accounting removed: the witness was insufficient. |
| `vrun-0c6dc982-1540-452a-9362-7129d340844b` | New isolated filter test fails full omission: work delta stays 168 across widths. |
| `vrun-03ea5ffc-a93e-4650-8ff9-1245893f1d44` | Traversal/retention/completion work-only omission fails the intended work assertion; entry guard and checked scratch remain. |
| `vrun-c5c54b5e-b4d2-4e6d-9734-3ec681afb732` | Scratch-only omission keeps work and fails the intended byte-growth assertion. |
| `vrun-53b52d71-038e-4d98-a168-8cedf3dcbd98` | Unguarded kind predicate RED: work delta stays 31 across widths. |
| `vrun-d8f54d77-4a1a-46ec-8a04-883a87a036ff` | Four strict/noCheck compiler tests, 26 native/API pairs, zero skips pass. |

Every omission experiment restored the original source. The independently
reviewed model's final SHA-256 at this checkpoint is
`015197ea71dad7827e71bfb97955a14eaaea792f3c3e57d3e25afce0c2d30820`.
This is a checkpoint fingerprint, not a prohibition on later authorized edits.

Read the bounded reviews before reusing these witnesses:

- [Initial witness review](../memory-bank/agent-runs/next-evaluator-resource-increment-review-2026-10-12.md).
- [Final frozen-source review](../memory-bank/agent-runs/final-evaluator-resource-increment-review-2026-10-12.md).

The final review recommends retaining the guard/controller. Its pending work,
scratch and kind GREEN evidence was subsequently supplied. Predicate-specific
whole-request depth dominance, shortcut-only debit mutation proof, and changed/
origin filter resource paths remain open. Request-wide depth-minus-one failure
alone does not identify which owner sets the peak.

## Current validation state

| Checkpoint | Exact result and limitation |
| --- | --- |
| Latest library: `vrun-64e6e93e-3cfc-46ac-ae87-c494ca8b5fa1` | 144 pass, one fail, one existing ignored. All thirteen resource tests pass. Sole failure is `long_extends_chains_load_in_linear_time`, the 10,000-config loading timeout. Cargo stops before the integration targets in this combined invocation. |
| Independent public-query matrix: `vrun-684a469e-808e-4d5f-9c5b-6885d399a47b` | Fourteen explicit integration targets pass, zero reported skips; includes operators, constraints, inference, JSON, members, CLI and service. |
| Six-file harness: `vrun-787bdda4-17f1-4ff4-9f66-644672c5ca6f` | 42/44 pass, zero skips. All fifteen resource/filesystem tests pass; two old inventory expectations fail. |
| Oracle repair: `vrun-09e84fb5-8e07-4208-af8a-2c2c510f78c1` | All 29 tests in `tools/type-oracle.test.mjs` pass, zero skips. With the preceding fifteen passes, all 44 scoped harness tests have passing receipts across runs. This is not one complete harness-suite pass. |
| Full lint: `vrun-627b0bb3-2255-49e0-b1df-92702c26bf37` | Fails on unused implementation surfaces, large AnalysisError results and other Clippy cleanup. No blanket allowances were added. |
| Full format: `vrun-09811037-923a-434b-84af-44e9373df6b9` | Fails on broad formatting backlog; output truncates at about one megabyte before all files. New-test formatting diagnostics and whitespace checks are scoped only. |

Two compatibility repairs changed test expectations only:

- `tests/type_intersections.rs` keeps bare native root summary `B & A`, but graph
  children now assert the approved labels `B (cases.ts:4:11)` and
  `A (cases.ts:3:11)`. Native order and deduplication were unchanged. Its seven
  tests pass separately in `vrun-5148bdf4-f55b-40d7-b320-3141033167c1`.
- `tools/type-oracle.test.mjs` distinguishes decoded inventory from native shape
  eligibility. Namespaced JSX compound keys remain inventoried but native
  `isValidReferencePosition` rejects that node before symbol demand. The late
  member fixture retains its two exact line-two value literals, then rejects
  those nonproperty positions before demand. Seed/combined origins and matching
  groups were unchanged. Keep the new assertions against accidental gate bypass.

The loading timeout recurred independently in normal and omission runs. It is
not an accounting RED. Find its test in `src/project/program/tests.rs` and
diagnose the failure; thresholds were not raised. Shared-machine load may affect
timing, but its cause has not been established. Preserve failed receipts when
retrying. Local diagnostics/manifests are under `.cc/temp/`, particularly
`resource-increment-final-source-manifest.md`; they are supplementary and may
not survive a different checkout.

### Commands to resume

Check live registration first:

```sh
cctl validate list
cctl validate run test --queue-if-busy --require-match -- src/lib.rs
cctl validate run test --queue-if-busy --require-match -- tests/type_intersections.rs
cctl validate run harness-test --queue-if-busy --require-match -- tools/type-native-kind-resources-oracle.test.mjs
cctl validate run harness-test --queue-if-busy --require-match -- tools/type-oracle.test.mjs
```

The registered Rust wrapper accepts existing `src/lib.rs` and explicit
`tests/<target>.rs` roots; it does not currently accept private nested unit-test
files or arbitrary test-name flags. Thus private resource TDD uses the single
library root. Test/harness profiles pin two build jobs/test threads or workers.
Use one admitted file in the TDD loop and explicit wider paths at checkpoints.
Large result receipts point to `.cc/temp/cctl-artifacts/`; read their complete
JSON output rather than treating truncated text as the verdict.

Repeat the latest independent public-query matrix without the library root if
its timeout would otherwise prevent integration execution:

```sh
cctl validate run test --queue-if-busy --require-match -- tests/type_operators.rs tests/type_truthy_operators.rs tests/type_base_constraints.rs tests/type_intersections.rs tests/type_intersection_constraints.rs tests/type_inference_lifecycle.rs tests/type_operator_completion.rs tests/type_constant_positions.rs tests/type_json.rs tests/type_json_initialization.rs tests/type_cli.rs tests/type_flow_continuation.rs tests/member_references.rs tests/reference_service.rs
```

Run full lint/format/semantic and full test/harness gates when the corresponding
changes or delivery checkpoint justify them. Retrieve individual receipts with
`cctl validate status <run-id> --json`. A target's pass inside a failed library
run is scoped evidence; report the full failure too.

## Remaining work and next increment

### 1. Whole-evaluator resource accounting

The existing resource plan's hotspot inventory covers recursive/iterative
instantiation, conditional transitions, intersections/subtype scans, templates/
strings, relation caches/stacks and pairs, inference contexts/candidates, flow
work stacks, symbol/signature allocations, module/export caches, source recipe
walks, and diagnostic/presentation scratch. Some entry guards and storage owners
are already controlled; verify actual loops/allocations and shared-ledger use
before extending them.

A useful next code increment is one uncovered bounded owner, such as flow work
stack visits/growth or a cached relation/inference loop. Freeze native traversal
on an eager final-query witness with matched warm prefixes. Then:

1. Establish a final-query RED that unrelated accounting cannot satisfy.
2. Extend the existing operation/storage owner with the same Control, preserving
   native counters, checking order, cache/recovery behavior and ordinary cleanup.
3. Prove work/storage boundaries and healthy following requests. For an omission
   experiment, keep compiler behavior unchanged, disable only the selected
   accounting slice, inspect the intended failed assertion, and restore the
   exact source before compatibility checks.
4. Record scope and remaining gaps, including passes and unrelated failures.

Void operands can defer checking; merely obtaining the expected final type does
not prove traversal. The earlier witness defect also shows why full request
usage-minus-one limits are insufficient owner-isolation proof. A new fixture is
admitted only after pinned compiler observations show the intended operation.

### 2. Resource calibration and liveness proof

Complete origin/changed-filter paths and the predicate depth/shortcut proofs
listed above. Establish production deterministic limits, deadline intervals,
stack headroom, interruption/report allowance and bounded teardown from actual
boundary and corpus measurements. Native instantiation/relation/product limits
answer different questions; retain their reasons and ordering separately.
Prove serialized-service queued lexical latency after semantic budget/deadline
exit, panic recovery and subsequent healthy requests.

### 3. Representation, retention and replay costs

Read [representation evidence](types-representation-evidence.md),
[representation review](types-representation-review.md), and
[binding retention](types-binding-retention.md) when selecting storage or
investigating retained footprint. Earlier eager/reprojection numbers are a
partial primitive-annotation spike, not a representation decision for the real
checker. Compare both candidates with the same actual native evaluator; measure
load/refresh, first/repeated queries, peak/retained memory and old-operation
costs. Finish compact canonical-fact/semantic-role/JSON replay proof and document
the selected representation against the design's measured gate.

### 4. Complete semantic proof matrices

Close full T/Q3/Q4 profile, option, selection, root/relationship direction,
recovery-causality, cache/reset/Maybe, result/graph and boundary matrices.
Existing operation implementations and many scoped probes do not close those
matrices. Include positive/refused controls, reversed seeds and queries,
complete/partial documents, exact anchors, graph sharing/cycles and wide shallow
lists. The design's six proof checkpoints are the authoritative acceptance list.

### 5. Concrete corpus product journeys

Start at the [controlled baseline README](../fixtures/types/command-center/aa484f4-2026-10-12/single-threaded-native/README.md).
The completed baseline is revision
`aa484f422f00dfb64967d2008fac3c4f9a92538e` with an explicit
`tsconfig.native-corpus.json` extending the pristine config with
`singleThreaded:true`. It freezes 7,639 sources, hashes every consumed input,
and has complete three-member/five-type native captures. Parallel parsing had
unstable default-library external provenance; keep its historical artifacts
separate. This compiler profile does not change product worker policy.

The baseline directory contains `input-manifest.json`, `profile-provenance.json`,
`member-observations.json`, `type-observations.json`, `observation-summary.json`
and bound artifact/tooling hashes. Capture-specific external-corpus harness
prerequisites were removed after collection; archived scripts are evidence,
not currently registered root commands. Read the README and live tooling before
preparing a new registered capture.

Prove Rust admission, normalized Q3 groups and filters, useful annotated-member
and inferred-factory results, bounded Q4 graphs, JSX/predicate gap classification,
direct/service equality, freshness/failed refresh, timings and memory. The
predicate anchor is a filter property expression, not its enclosing call's
result. Every captured Q4 root has non-error type despite two startup TS2300
diagnostics; those observations neither establish a clean diagnostic project
nor justify rejecting all selected types. Native capture timings include
transport/replay and are not product latency acceptance.

### 6. Full validation and delivery

Resolve the loading timeout and full lint/format cleanup. Run complete registered
Rust/harness/member-type oracle validation and repeat prior S/C/E compatibility
after final wiring. Complete real-project and worker/service guarantees before
claiming delivery. The active ticket remains In Progress.

## Closeout for the next agent

Keep [implementation status](types-implementation-status.md) current with the
final implementation, exact receipts and their scopes. Preserve raw compiler
expectations and record causal fixes at their single owners. When a new increment
is ready, state what changed, why it is correct, what passed, and what remains;
use the full design's delivery gates to decide whether T/Q3/Q4 is complete.
