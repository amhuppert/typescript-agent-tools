# Project navigation storage

Plan step 4 asked which representation the project snapshot should keep for
outline and show. Both candidates were built behind the same operations
(`Project::outline`, `Project::show`) and passed the same tests; this records
the comparison and the decision. The design's
[snapshot ownership](navigation-design.md#complete-operations-and-snapshot-ownership)
section states the resulting architecture.

## Candidates

- **Eager complete records.** While loading parses each input, project its
  complete catalog (every record with its label, key, selector, kind, anchor,
  `--at` targets, range, and rendered signature), and keep the catalogs of
  searched files once provenance is known. Every query answers from memory.
- **Compact index plus selected-file projection.** While loading parses each
  input, project it without rendering signatures or the labels of unnamed
  sites, and keep, for each keyed site of a searched file, its key, nearest
  keyed container, kind, anchor, and ordinal, plus the few labels that differ
  from their keys. A name search, across files or narrowed to one, reads only
  the indexes; outline, show by position, and the source of a single match
  project the selected file's captured text again, with the shared parsing
  capability. (As compared on 2026-10-07, the index still rendered unnamed
  sites' labels, and a name narrowed to a file was selected in the
  reprojected catalog; the round-5 review fixes changed both.)

Both keep the catalogs' or indexes' text as the snapshot's captured
`SourceText`, shared with references, and neither reads files at query time.
The prototypes stored their records at exact size; an earlier index prototype
that collected in place kept the projection's whole record buffer and measured
like the eager records, so retained-memory numbers depend on that detail.

## Measurements

`cargo bench --offline --bench navigation -- CONFIG` measures, for one
configuration: three loads (median), requested heap during and after a
fourth load (allocation accounting, as in the reference benchmark), and for
a small (the smallest searched file of at least 2 KB) and the largest
searched file, the median of 20 outlines and shows by the last entry's
anchor, each with its peak requested heap above the snapshot; then
project-wide shows by name for each outcome, each the median of 20 from the
snapshot with its peak heap and of ten warm `ProjectSession` requests, which
also validate freshness; and the median of ten warm session outlines. The
name cases are a name no file declares (missing), the last key of the largest
file that several declarations share (ambiguous), and the last qualified
selector in the small file and in the largest file that selects exactly one
declaration (unique); the benchmark asserts each case's outcome, and a unique
match's file, before measuring it, so a changed project cannot silently
change what a case measures.

Conditions: Intel Core i9-9880H at 2.30 GHz, macOS 15.7.7, Rust 1.96, release
profile (thin LTO, one codegen unit). Requested heap is deterministic.

### Storage comparison (2026-10-07)

The two strategies at commit `656b44f`, against the baseline `cargo bench
--bench references` at commit `285854a`, before navigation joined the
snapshot (load median of five). The benchmark then measured project-wide show
by the last entry's name only, which was ambiguous in both projects, so
these tables compare the strategies for an ambiguous name and for no other
name outcome. Another process was running a test suite on the same machine
throughout, so times vary by up to about 20% between rounds; the strategies
and baseline ran alternately, and the tables give each one's median round.

**Command Center**, unchanged `tsconfig.json` at revision `7706073d2`: 5,312
searched TypeScript files with 666,913 declarations; the largest is
`src/lib/workflow-graph/workflow-manager.test.ts` (342,467 bytes, 5,358
declarations), the small one `TicketChildSummary.tsx` (2,052 bytes).

| | Baseline | Eager records | Compact index |
| --- | ---: | ---: | ---: |
| Load (median) | 8.2 s | 10.1–10.6 s | 8.1–9.6 s |
| Peak requested heap while loading | 966.7 MiB | 1,147.5 MiB (+180.8) | 991.7 MiB (+25.0) |
| Retained after load | 309.3 MiB | 490.1 MiB (+180.8, +58%) | 334.4 MiB (+25.1, +8%) |
| Outline, small file | — | 0.007 ms | 0.19–0.22 ms |
| Outline, largest file | — | 4.5–4.9 ms (1.3 MiB peak) | 31–34 ms (7.7 MiB peak) |
| Show by anchor, largest file | — | 0.017 ms | 26–29 ms |
| Show by an ambiguous name across the project (2,312 candidates) | — | 11.7 ms | 10.7–11.7 ms |
| Warm session outline, largest / small | — | 378 / 382–496 ms | 403–460 / 383–464 ms |

**TypeScript 6.0.3 compiler sources** (the mid-size fixture: its `compiler`
directory, 9.4 MB, with `noLib`, shorthand ambient modules for its Node.js
imports, and a stub for the build-generated diagnostics map): 79 searched
files with 47,827 declarations; the largest is `checker.ts` (3,151,774 bytes,
11,863 declarations), the small one `factory/utilitiesPublic.ts` (2,101
bytes). Three alternating rounds:

| | Baseline | Eager records | Compact index |
| --- | ---: | ---: | ---: |
| Load (median) | 918 ms | 1,068 ms | 954 ms |
| Peak requested heap while loading | 148.0 MiB | 162.8 MiB (+14.8) | 150.1 MiB (+2.1) |
| Retained after load | 55.2 MiB | 70.0 MiB (+14.8, +27%) | 57.3 MiB (+2.1, +4%) |
| Outline, small file | — | 0.006 ms | 0.13 ms |
| Outline, `checker.ts` | — | 9.2 ms (3.4 MiB peak) | 173 ms (60.0 MiB peak) |
| Show by anchor, `checker.ts` | — | 0.037 ms | 162 ms |
| Show by an ambiguous name across the project (396 candidates) | — | 0.43 ms | 0.61 ms |
| Warm session outline, largest / small | — | 18 / 7 ms | 175 / 7 ms |

### Name searches by outcome (2026-10-08)

The compact index only, after the round-5 review fixes, in three alternating
rounds on an otherwise idle machine; each cell is the range of the three
rounds' medians. Eager records were not measured again. They would answer a
unique name from memory, as they answered the ambiguous name above, without
the reprojection that a unique match costs the index.

Command Center at revision `7706073d2`. Loading took 8.0–8.2 s, with a peak
of 1,008.6 MiB requested while loading and 334.4 MiB retained. The retained
heap is the comparison's; the peak, 16.9 MiB above the comparison's, is
unchanged by these fixes (commit `8edbc1f` measures the same) and came with
the commits merged between `656b44f` and `8edbc1f`.

| Show by name across the project | Outcome | Snapshot | Peak | Warm session |
| --- | --- | ---: | ---: | ---: |
| `absentFromEveryFile` | not found | 5.6–5.7 ms | 0.13 MiB | 378–382 ms |
| `reason` | ambiguous, 2,312 candidates | 8.6–9.1 ms | 0.64 MiB | 381–385 ms |
| `TicketChildSummary.entry` | found in the small file | 6.8–6.9 ms | 0.13 MiB | 379–380 ms |
| `incumbent.haltReason.message` | found in the largest file | 32.2–33.0 ms | 7.66 MiB | 401–405 ms |

In the same runs: outline of the small file 0.19–0.23 ms and of the largest
29.8–30.1 ms (7.66 MiB peak); show by anchor 0.18–0.20 ms and 25.0–25.8 ms;
warm session outline of the largest file 397–401 ms and of the small one
368–376 ms.

Compiler sources: loading took 929–943 ms, with peaks of 152.1 MiB while
loading and 57.3 MiB retained (`8edbc1f` measures the same).

| Show by name across the project | Outcome | Snapshot | Peak | Warm session |
| --- | --- | ---: | ---: | ---: |
| `absentFromEveryFile` | not found | 0.31–0.32 ms | 0.00 MiB | 7.1–7.2 ms |
| `popErrorFallbackNode` | ambiguous, 3 candidates | 0.31–0.32 ms | 0.00 MiB | 7.0–7.2 ms |
| `canHaveDecorators.kind` | found in the small file | 0.43–0.49 ms | 0.06 MiB | 7.2–7.3 ms |
| `SymbolTrackerImpl.popErrorFallbackNode` | found in `checker.ts` | 159–162 ms | 60.02 MiB | 152–156 ms |

In the same runs: outline of `checker.ts` 161–172 ms (60.02 MiB peak), show
by anchor in it 160–161 ms, and warm session outlines of it 164–169 ms and of
the small file 6.8–7.1 ms.

Repeated at plan step 6 (2026-10-08, commit `dfc65e2`) on the same Command
Center revision, with other sessions keeping the load average near 3: loading
8.08 s (median of three), 1,008.8 MiB requested at peak and 334.4 MiB
retained; outline of the largest file 30.2 ms and of the small one 0.19 ms;
show by anchor 25.1 ms and 0.19 ms; project-wide shows by name 5.8 ms
(missing), 8.8 ms (`reason`, 2,312 candidates), 7.0 ms (unique in the small
file), and 34.1 ms (unique in the largest); and warm session requests of
372–407 ms. [Command Center
performance](performance-command-center.md#current-state-2026-10-08) adds the
CLI and service measurements.

A missing or ambiguous name reads only the indexes: its cost grows with the
project's declarations and, when ambiguous, its candidates, not with any
file's size. A unique name pays the same index search plus one reprojection
of its file, so it costs what an outline of that file costs: on Command
Center about 1 ms more than a missing name for the small file and 27 ms more
for the largest, and on the compiler sources about 160 ms for `checker.ts`.

## Decision

Keep the compact index with selected-file projection, and delete eager
records. The tradeoff is not small: on the unchanged Command Center
configuration, eager records retain about 181 MiB more than references alone
(+58%) and raise the loading peak by the same amount, for every query of the
session's life, while the index adds about 25 MiB (+8%). Construction is no
slower than eager records, since the index renders no signatures. The
outcome-specific measurements above do not change this.

What the index costs is query latency for the file a request selects:
outlines, shows by position, and a name that matches one declaration
reproject that file. On Command Center's largest file that is tens of
milliseconds instead of a few, and a transient 7.7 MiB; on a 3 MB
generated-style file such as `checker.ts`, about 160–170 ms and 60 MiB. For
the measured Command Center workload, a warm session request spends about
370–400 ms validating freshness, so even the largest file's reprojection is
under a tenth of a request. That is a property of that workload, not a
general bound: the compiler sources validate in about 7 ms, so there a
unique match in `checker.ts` is nearly all of a warm request, as an outline
of it is. Missing and ambiguous names read only the indexes; the eager
comparison measured an ambiguous name, which cost the same in both, and did
not measure the other outcomes. A successful unique-name show pays the index
search plus the reprojection of the selected file, which eager records would
not. Retained memory, by contrast, is paid by every resident service for its
whole life. Eager records are the simpler implementation, and would be
preferred if the memory difference were small; here it is several times the
index.

The index and the catalog are one projection (`navigation::projection`) in
two forms: the index omits signatures, the labels of unnamed sites, and
unkeyed sites, and each indexed site records its ordinal among the complete
records, so a single match is found in the reprojected catalog. The index
also owns name selection and each keyed site's key, selector, and candidate:
a reprojected or file-mode catalog derives them from the index of its own
records, so a project-wide search, a search narrowed to a file, and file
navigation apply one rule. Tests select dotted namespaces, nested
namespaces, ambient modules, and global augmentations across a project and
within a file, and a debug assertion checks that a single match's index
candidate equals the reprojected catalog's.
