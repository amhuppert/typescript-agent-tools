# Project storage checkpoint — 2026-10-13

This checkpoint reduces the retained cost of the existing native T/Q3/Q4
implementation. The implementation and measurements cover immutable Project
storage and allocation removal in the existing path owner. Native type/reference
delivery, resource-accounting calibration and compiler-conformance work remain
in progress. Since this checkpoint, the per-request fee ledger that calibration
was for has been dropped in favor of the CLI process watchdog
(`src/process_limits.rs`), so the accounting calibration below is no longer planned.

## Measured storage and load results

The final sequential benchmark comparison, after this session's builds and
validation finished, gives the following results. MB below means decimal MB;
heap columns count requested bytes, not RSS. Each load time is the median of
five loads. The unchanged benchmark also performs query and heap-accounting
loads separately.

| Input | Retained heap, before → final | Peak heap, before → final | Load median, before → final |
| --- | --- | --- | --- |
| 301 generated files | 23.30 → 12.83 MB (45% lower) | 27.53 → 23.84 MB | 225 → 230 ms |
| One file, 5,000 functions | 54.46 → 31.64 MB (42% lower) | 61.94 → 61.45 MB | 338 → 355 ms |
| Command Center corpus | 4,491.45 → 2,369.62 MB (47% lower) | 5,275.83 → 4,342.35 MB (18% lower) | 41.94 → 36.27 s (14% lower) |

This is a substantial retained-memory reduction, with a measured corpus load
improvement and roughly unchanged small-fixture load times. It is not a claim
that every operation becomes faster. Corpus benchmark CPU time, including its
several load/drop cycles and queries, fell from 321.12 to 258.66 seconds (19%).
The 2,000 corpus query positions retain the same distribution: 473 group-returning
queries, 1,145 empty results, nine OutsideProject and 373 UnsupportedTarget.
Mean lexical query time was 472.8 → 283.5 microseconds in these sequential runs;
host variation limits attribution of that timing difference to the code alone.

The alternating outline runs use two fresh processes per variant, in
baseline/storage/final/final/storage/baseline order. All six return identical
outline bytes and exit zero. Their medians are:

| Variant | Wall time | CPU time | Maximum observed RSS |
| --- | --- | --- | --- |
| Baseline | 29.67 s | 29.10 s | 5.16 GB |
| Storage only | 26.54 s | 26.42 s | 4.11 GB |
| Storage plus path fast path | 23.29 s | 23.27 s | 4.13 GB |

The final outline command is 22% faster by wall time and uses 20% less CPU in
this comparison. Filesystem caches are warm and host activity is uncontrolled;
two samples per variant support this observed improvement, rather than a
universal latency guarantee. The final path runs both take 23.24–23.33 seconds;
storage-only runs take 24.78–28.30 seconds and baseline runs 28.23–31.12 seconds.

All nine saved CLI journeys also match the baseline byte-for-byte on stdout,
diagnostic stderr after removing only `/usr/bin/time` metrics, and exit code.
The five type journeys return Found. The three member-reference journeys still
return the existing Internal failure, `resolved anonymous object lacks installed
member fields`, with exit 2. They are output-equivalence controls, not successful
member-reference delivery or evidence that failures are useful fast answers.
The interrupted CLI capture was resumed only for missing commands; completed
benchmark captures were preserved. The six alternating outline runs were then
collected together in the resumed turn.

The [raw measurement results](types-storage-performance-results.json) contain
the measured bytes/times, outcome counts, output comparison flags, binary and
source-manifest hashes, input identities and run qualifications. The comparator
asserts every outcome distribution and all nine stdout/stderr/exit comparisons;
it writes the results only after those assertions and the six outline checks
pass.

## Representation and ownership

The linker still builds lexical symbols, resolves the same work, creates the
same directed reference index, and completes the same conflict checks. After
those operations, production consumers need only the independent raw native
binding snapshot, the lexical Index and retained file facts. The unused
`Completed` lexical Store and saved resolution vectors are now dropped at that
boundary. The final no-active-resolution and no-in-progress-answer invariants
remain assertions before publication. No dependency demand is added.

Exact syntax-site maps now freeze to sorted boxed slices. Ordinary source
offsets and flow/container identities pack into 32-bit fields. A value or offset
that cannot fit selects a full-width representation for that file; it does not
truncate, merge identities, or reject otherwise supported input. Keys still
include syntax kind and both span endpoints. Stable ordering preserves the
flow builder's last write at a duplicate site. Scope collection retains its
existing transient HashMap so all 15 overwrite sites and both `entry/or_insert`
sites keep their distinct behavior before freezing.

Private flow/container identities use nonzero storage, with arena index zero
encoded as one. This reduces optional read attachments while preserving the
difference between a missing flow, an unreachable flow and an absent site.
Native headers inline their common single declaration using the already-pinned
SmallVec dependency. Immutable per-file arrays become boxed slices at
publication, preserving element order and IDs while releasing capacity slack.

The change intentionally keeps mutable builders and immutable query storage
separate. Sorting and shrinking can add publication work; dropping lexical
state brings its destruction into load time even for the existing one-shot CLI
that skips final Project teardown. End-to-end measurements, not record sizes
alone, decide whether the tradeoff is useful.

A subsequent one-file CPU optimization stays inside
`program::paths::normalize` and `Case::key`. Normalization now uses the existing
`has_relative_path_segment` predicate for the pinned `NormalizePath` /
`simpleNormalizePath` fast path. After slash conversion, an already-normalized
path returns its owned string without allocating/rejoining components. Key
creation reuses that string when the existing case-fold owner borrows it.
Paths needing reduction still take the same reducer. No caller changes, cached
path matrix, filesystem reads, probe pruning or freshness-policy changes are
introduced. A separate storage-only release is preserved to distinguish this
CPU change from the representation work.

## Reproduction and measurement

The pre-change source snapshot is `.cc/temp/perf-before/src`; its source
manifest, saved binaries and hashes are under `.cc/temp/perf-before/`.
All three saved variants (baseline, storage-only and final path optimization)
use the same Cargo release profile and pinned dependencies.
The existing registered broad benchmark build timed out at 900 seconds while
building the implicit release library test harness. The new narrow wrapper
builds only `--bin tsparse --bench references` with two jobs. Its local
`CommandCenter.json` registration is prepared but is not yet live in the
canonical registry. All three release variants were built using that wrapper under
the project-setup skill's wrapper-development diagnostic exception.

Saved CLI SHA-256 identities are:

| Variant | SHA-256 |
| --- | --- |
| Baseline | `cfa1df6edfdedc504e3c56b056e784aa927fe09e0c4055e00edf5aa86e20bdbf` |
| Storage only | `fe697cc3629e271241663d0137bbd347418088747f4031ef97f4567fdd08c5ab` |
| Final | `9242993dce638b559bba3b00da8782afb4641d1f6f0f7820418d51c1ffb842b3` |

The final source manifest covers 347 Rust/build-input files and matches the live
worktree. Against the resumed snapshot, 13 binding/path files contain the
production changes and five evaluator files have formatting-only changes.
Cargo manifests, lockfile and benchmark sources are unchanged. The complete
baseline build-input manifest is `.cc/temp/perf-before/build-input-hashes.json`;
the original frozen source manifest is preserved separately.

The unchanged reference benchmark measures median load time over five loads,
2,000 lexical query positions with outcomes counted separately, and a separate
load with requested live/peak heap accounting. Requested heap excludes allocator
overhead and is not RSS. `/usr/bin/time -l` captures process wall/CPU time and
maximum RSS. The benchmark's process RSS includes several load/drop cycles.

Inputs are the existing generated 301-file module/barrel fixture, a single
563,890-byte source with 5,000 functions, and the frozen 7,639-input Command
Center corpus at `.cc/temp/command-center-aa484f4-corpus/tsconfig.json`. The
corpus checkout was verified at `aa484f422f00dfb64967d2008fac3c4f9a92538e`,
with no tracked changes; its extra `tsconfig.native-corpus.json` is not the
configuration passed to these benchmarks. Config, lock and fixture hashes plus
the Rust 1.96.0 toolchain identity are saved in `.cc/temp/perf-inputs.json`.
The host has 16 GiB RAM and 16 logical CPUs.

The single-file case checks the sort/copy tradeoff that a many-small-file corpus
could hide. `.cc/temp/perf-measure.py` drives identical saved binaries over
those inputs, then captures outline, five type and three member-reference CLI
journeys. It preserves exit codes, stdout/stderr and output hashes even for known
failures; cross-variant comparison is a separate check.

The storage-only corpus run overlapped compiler/oracle/validation work. Its
retained/peak requested heap and outcome counts are useful, but its timing is
confounded: it took 71.36 seconds per median load versus the earlier baseline's
31.47 seconds. A one-second process sample was also taken during that run. The
subsequent CLI loop was stopped after the heap benchmark, and that partial run
is not claimed as a complete journey comparison. The final measurement sequence
waits for this session's builds and validation to finish, repeats the baseline
benchmarks, runs the final benchmarks/journeys, then alternates fresh-process
outline runs in baseline/storage/final/final/storage/baseline order. Filesystem
caches are not flushed. Other host activity remains uncontrolled.

## Final-Project regression proof

`tests/project_storage.rs` builds 256 modules plus an importing main file and
counts requested heap during `Project::load`. It then asserts one final lexical
reference group containing three occurrences, a Found `number` type answer, and no
issues. Before optimization those query assertions passed but the 8 MiB retained
budget failed with 15,678,694 retained bytes and 17,495,142 peak bytes:
`vrun-764a5ee1-4b9b-4d08-93c2-9634778debba`.

After the changes, the same behavior and budget pass:
`vrun-317fb145-a81b-4fe0-959a-990f8ca36b8c` (one file, zero skips). This proves
retained requested heap below 8,388,608 bytes for that fixture. It does not put a
wall-time limit on a shared host or claim that arbitrary projects fit that budget.
Representation-only changes use the mechanical-change exception to additional
test-first semantics work; private boundary tests cover full-width fallback,
exact site identity, duplicate writes and optional flow identity. Existing final
Project compatibility tests cover the semantic outcomes.

## Compatibility and remaining work

Independent review found no remaining source-level blocker in the final
representation. It caught a draft scope migration that missed the two
conditional writes; that draft was corrected before production application.
The final review is `.cc/temp/perf-compaction-review.md`.

Fresh registered format passes (`vrun-d2b06a79-cb89-47b5-bf9c-e5a3d4a28a79`).
The complete library test target also passes
(`vrun-a02c0a55-53fa-4e86-a859-d2566ef8e1a3`), including the storage and private
resource tests. Its existing program-listing diagnostic remains explicitly
ignored because it needs external configuration/listing inputs; the wrapper
reports zero platform skips. This pass does not establish the outstanding
whole-evaluator accounting calibration.

The storage-only version also passes full Clippy
(`vrun-b1c32bcb-6e0b-48ba-8f82-8c80ebac5908`) and the 18-file integration
checkpoint (`vrun-029d46f3-4b01-40bb-9167-4aee4bb58293`): references,
augmentation, JSON, selection, JSX gaps, global merge, before-use and flow,
contextual patterns, iteration, module specifiers and snapshot freshness,
and Project/navigation/editing CLI paths. The only platform skip is the
existing non-UTF-8 working-directory fixture, which this filesystem refuses
to create. Final path-optimization receipts are recorded separately below.

The final path-plus-storage source passes these fresh registered checks:

| Check | Receipt | Scope |
| --- | --- | --- |
| Format | `vrun-89dcffcb-84b0-45f1-80f4-5d47db9c5646` | Full formatting check |
| Clippy | `vrun-1eedf219-c187-4d48-9677-407cbf1a4e66` | All Rust targets, warnings denied |
| Library | `vrun-25ef9084-6211-40cd-ac79-0b70220e1105` | Complete library target; same explicitly ignored listing diagnostic |
| Integration | `vrun-3433c77a-e833-4082-8ec8-93ef66290fab` | Previous 18 targets plus `project_storage` and `type_conditional_types`; same one platform skip |
| Pinned module-specifier and snapshot controls | `vrun-6d1cf09d-4e1c-4cc5-8e94-8816acdc7e84` | Four tests executed and passed, zero skips |

One private test was renamed to its narrower guarantee: dependency tokens stay
unsearched after semantic queries. It no longer inspects discarded resolver
states to prove that every unused dependency task was unvisited. No new eager
demand appears in the production change; the narrower test should not be cited
as proof of that stronger lifecycle property.

Before optimization, six integration targets already failed:
`type_lazy`, `type_qualified_aliases`, `type_unresolved_type_aliases`,
`member_reference_failure_causes`, `type_global_this_raw`, and `type_object_rest`.
Changing their expectations requires pinned compiler evidence, not agreement
with this implementation. Expanded global-merge Rust and TypeScript controls
passed on recovery (`vrun-043305e6-f35c-4848-a71c-325410244bf4` and
`vrun-4594ba5c-e265-4be0-8762-db79be35d1be`). The storage checkpoint must preserve
their results as well as parameter-property, JSX, contextual-pattern and
iteration-protocol fixes that were present at the resumed baseline.

The six failing targets were replayed individually after the final change. All
19 test statuses are unchanged: ten pass and nine fail. Independent comparison
found identical assertion payloads after normalizing only thread IDs, temporary
fixture PID/sequence paths and the optional backtrace hint. The receipt table
and comparison are in `.cc/temp/perf-final-compatibility-review.md`. This is
evidence of unchanged known failures, not a green full test suite.

The path review also identified a separate pre-existing discrepancy for bare
UNC/URL roots: native `NormalizePath("//server")`, `("http://server")`, and
`("file:///c:")` add a trailing separator, whereas the existing Rust reducer
does not. All these paths still take that unchanged reducer; this performance
increment neither introduces nor repairs the discrepancy. A future behavior
fix needs its own final-outcome reproduction and pinned controls. The reviewed
fast-path branch and case composition have explicit empty, root, relative,
dot-segment, separator, trailing-slash, drive, UNC/URL and Unicode test rows.
