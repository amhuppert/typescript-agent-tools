# Relative path preparation performance

2026-10-17

Project loading now prepares importer directories once and constructs each
distinct relative spelling once per target. This removes repeated path
normalization and temporary allocations from module-specifier capture. The
change applies to direct project commands, the first request to a service, and
the full reload after an edit.

## Measured impact

On the real Command Center project, the repeated alternating direct-load series
reduced median wall time **8.7%** and retired instructions **6.6%**. Values below
are medians with complete ranges; direct loads have three samples per binary.
This series used Command Center revision `976153bb38de`. The cold-service row
is one before/after pair from the same faster conditions and revision.

| Measurement | Before | After |
| --- | ---: | ---: |
| Direct load wall time | 11.01 s (10.91–11.03) | 10.06 s (10.00–10.28) |
| Direct load CPU time | 20.49 s (20.31–20.74) | 19.78 s (19.60–19.93) |
| Direct load retired instructions | 108.39 billion (108.33–109.03) | 101.22 billion (101.16–101.90) |
| Direct load peak physical footprint | 2.63 GiB (2.63–2.64) | 2.66 GiB (2.64–2.67) |
| Cold service, first request | 10.78 s | 9.79 s |
| Service physical footprint after loading | 1,697 MB | 1,679 MB |

The initial series used revision `92fbfd94221b` under much stronger CPU
throttling. Its direct-load
medians were 25.63 → 23.19 seconds (9.5% lower), with full ranges 23.52–25.65
and 23.02–23.31 seconds. Retired instructions fell 6.6% there too; peak physical
footprint was 2.65 GiB for both. The first cold-service pair took
24.02 → 22.57 seconds, retaining 1,728 → 1,689 MB. These samples are kept
separate because both the live checkout and host conditions changed.

The edit experiment used a fixed APFS clone of `92fbfd94221b`, with three
successive edits in each of two services per binary. Median refresh time fell
**8.7%**. These are separate from the live-checkout figures: the clone has a
longer absolute path and retains the earlier inputs.

| Clone measurement | Before | After |
| --- | ---: | ---: |
| Cold service, first request (2 samples) | 13.64 s (13.62–13.65) | 12.06 s (11.87–12.24) |
| First request after an edit (6 samples) | 14.43 s (14.05–14.87) | 13.17 s (12.79–13.31) |
| Physical footprint after refreshes, full range | 1,107–1,277 MB | 1,092–1,270 MB |

Warm queries remained effectively unchanged. Each cell is the median of six
requests within one service; the two conditions are shown separately.

| Warm query | Strong throttling, before → after | Recovered CPU speed, before → after |
| --- | ---: | ---: |
| Small-file outline | 355 → 367 ms | 133 → 134 ms |
| References to `getTicketService` | 485 → 475 ms | 179 → 180 ms |
| Type at `service-factory.ts:201:17` | 476 → 492 ms | 185 → 186 ms |

There is **no demonstrated physical-memory saving**: direct-load peak footprint
changed by +0.1% in the first series and +1.0% in the repeat, while retained
service footprints were slightly lower. The clear allocation result is reduced
temporary traffic in the regression fixture, described below.

## Why this optimization

The [compiler research synthesis](reports/performance-research/compiler-performance-consolidated.md)
recommends removing repeated work and separating temporary allocation traffic
from retained memory. The [preceding measurements](performance-command-center.md#current-state-2026-10-17-after-the-performance-work)
attributed about 1.4 seconds of a load to module-specifier capture. Sampling the
baseline again showed relative-path construction in `index_collision_paths`.
For every index-like target, this operation normalized and split each importer
directory, normalized and split the target again, and allocated the resulting
path before a set removed duplicate spellings. Sibling directories often
produce exactly the same relative spelling.

The design consultation with Astra xhigh compared this bounded change with a
directory trie,
incremental reuse of file captures after edits, and smaller retained declaration
records. Incremental reuse has greater potential for refreshes, but needs a
larger ownership change around consumed captures, resolutions, and file IDs.
Compact records target retained memory but span more consumers. Preparing and
deduplicating relative paths addresses a measured cost in every load through
one existing decision owner, without persistent cache state or invalidation.

## Implementation and limits

`paths::RelativePaths` owns normalized directory components for the duration of
module-specifier capture. It normalizes each target once, computes each
directory's common prefix and number of parent components, and inserts that
shape directly into a set. Only distinct shapes are rendered into strings.
Scalar relative-path calls use the same component comparison and rendering
owner, named for TypeScript's `GetPathComponentsRelativeTo` and
`GetPathFromPathComponents`.

Root comparison remains case-insensitive; other components use the host's case
policy. Results preserve the target's exact spelling. The existing final set
still determines collision probe order, with the same cwd, extension, package,
and filesystem-observation behavior. Prepared directories are scratch storage
and are released after capture.

Comparisons still visit every importer-directory/target pair. This change
reduces normalization and construction, not the worst-case number of
comparisons. `rootDirs` retains its per-directory transforms and order-sensitive
selection. Warm queries and file-only commands do not execute this capture.

## Measurement method

The benchmark uses `~/github/command-center/tsconfig.json`. The checkout began at
`92fbfd94221b4c4829b04acb2c4fbcd0ef681965` and advanced externally to
`976153bb38de1866e01d671a9c9efad115642d92` at 14:51:49, between the two direct
series. The first series finished at 14:50:15; the repeat began after 14:53.
The new commit removed Cursor billing code. Tracked status was clean at both
boundaries, and the benchmark made no edits there. It is a live filesystem: all
source,
configuration, generated inputs, and directory observations were not separately
frozen or hashed between samples. A successful load validates its own captured
observations; that does not establish identical complete inputs across loads.
The baseline is this project's
`d212fb3f90e6e7805171f9c3681b8932957cdaef`; both binaries use the release profile
(thin LTO, one codegen unit). SHA-256 hashes:

- Before: `53cc1fe9d6fc2e0b1c78ae1a3bc3e6dc714877a0ed503ba06345aafdb1dd9009`.
- After: `179271b24f199a4ff3e4acccf40c22ebb536736aefec618fe28b0209527d02a7`.

The host is the same Intel Core i9-9880H, 16 logical CPUs, 16 GiB RAM, macOS
machine used in the preceding report. This session's builds and tests finished
before timing. Other applications remained active, and CPU throttling was
substantial: the speed limit was 41 near the start, with a load average of 9.6.
It recovered to 100 between the two service pairs, prompting a second complete
direct-load series; both series are reported above.
Absolute times therefore should not be compared with the earlier report's
lighter-load timings. The before/after instruction counts provide additional
evidence of reduced work; the timing samples are descriptive, not a confidence
interval or a prediction for other machines.

Direct loads use `--outline --project tsconfig.json --direct` on
`src/features/tickets/components/TicketChildSummary.tsx`, three samples per
binary per series in before/after/after/before/before/after order.
`/usr/bin/time -l`
records CPU time, retired instructions, and peak physical footprint. Python
times each separate CLI process through a blocking `subprocess.run`.

Services run in the foreground with separate private runtime directories, in
before/after/after/before order. Their first outline request loads the project.
"Cold service" means an unloaded project snapshot; filesystem caches were not
flushed.
Each then answers six warm requests for each of outline, references to
`getTicketService`, and the type at `src/lib/tickets/service-factory.ts:201:17`.
Generation remains 1 throughout warm measurements. `footprint -p` measures
physical footprint after loading.

Edit measurements use an APFS clone of the same real checkout under
`.cc/temp/next-perf/command-center`, including its dependencies and generated
type inputs. The copy omits `.git`, `.worktrees`, and `.next` apart from its
generated type directories. Each service receives three successive exported-constant additions
to the small outline file; the next outline must include the new declaration
and advance the generation. The file is restored between services. All measured
outputs are compared byte-for-byte between binaries for the same query/edit.
Afterward, all 5,072 tracked files in the restored clone matched the Git blob
hashes at `92fbfd94221b`; the tracked live checkout remained clean.
The disposable clone was then removed; measurement artifacts were retained.

One cold-service attempt overlapped the live checkout update and failed because
inputs changed during indexing. The service was stopped and that round
restarted; it supplied no successful latency sample. The completed samples
were retained, with the two service pairs reported separately.

The harness, metadata, and raw results are session-local under
`.cc/temp/next-perf/`: `measure.py`, `metadata.json`, `direct.jsonl`,
`direct-repeat.jsonl`, `warm.jsonl`, `edits.jsonl`, and `direct-*.time`.

## Correctness and regression protection

The allocation regression first failed on the baseline through `Project::load`:
64 to 256 sibling index modules grew from 136,329 to 1,378,987 allocation
requests. After the change, the same sizes used 67,839 and 269,474 requests:
**80.5% fewer requests at 256 modules**, with approximately linear growth.
Requested allocation bytes were 6,601,563 and 26,210,367 after the change;
these are cumulative traffic, including full reallocation sizes, not live heap
or physical memory.

The test counts all load workers and checks the final references for every
module. Its budgets bound excess growth beyond the fourfold input increase;
they reject both one rendered allocation per pair and one stored shape per
pair. A warm-up excludes one-time initialization.

The pinned TypeScript 7.0.2 Go implementation supplies 172 path expectations,
including roots, normalization, separators, cwd, and Unicode case variants.
Rust checks individual paths and unions over directories. A reproducible Go
overlay verifies the fixture against the pinned source; see
[validation instructions](validation.md#relative-path-preparation).

Validation passed for the library, five focused project/module-specifier
integration targets, the relative-path and module-spelling/manifest native
oracles, Clippy, formatting, and the release build. The full registered semantic
check passed: 1,655 reference queries and the navigation comparison, with no
unexpected differences. The full all-integration Rust and full harness suites
were not run.

Astra xhigh and Opus 5.5 xhigh reviewed the implementation and follow-ups. Their
findings led to direct insertion instead of `BTreeSet::from_iter` (which first
collects all inputs), stronger allocation budgets, broader native fixtures, and
protection against stale oracle output. Neither review found a remaining
production correctness blocker.
