# Unchanged-file and resolution reuse

2026-10-17

Refreshable sessions now retain immutable file products and exact resolution
decisions between successful generations. Every refresh still rebuilds program
membership, assigns FileIds, binds the program, and validates the final snapshot.
One-shot CLI commands retain no refresh cache.

## Measured impact

In the primary alternating run on Command Center, small-edit refreshes were
**25.3% faster**, from a median 9.99 s to 7.46 s. File reuse supplies almost all of this gain. Retaining the
reusable inputs increases steady service memory substantially.

| Measurement | Before | After |
| --- | ---: | ---: |
| First outline after a small edit, median (range), six samples each | 9.99 s (9.92–10.13) | 7.46 s (7.36–7.49) |
| Retired instructions, first load plus three edits, median of two services | 468.95 billion | 282.23 billion |
| Retained service physical footprint after edits, median (range) | 1,103 MB (1,053–1,206) | 2,014 MB (1,893–2,042) |
| Peak physical footprint, first load plus three edits | 2.674 GiB | 2.578 GiB |
| Direct outline, median of two processes | 9.43 s | 9.38 s |
| Direct peak physical footprint, median | 2.681 GiB | 2.533 GiB |
| Warm outline, median of twelve requests | 173 ms | 174 ms |
| Warm references, median of twelve requests | 180 ms | 179 ms |
| Warm type, median of twelve requests | 183 ms | 181 ms |

Total process instructions fell 39.8%; that measurement includes the first load,
three refreshes, and service shutdown, not just a single refresh. Direct and
warm latency are essentially unchanged. Direct peak physical footprint fell
5.5%, while retained service footprint after edits rose 82.6%. Memory measured
after the third edit was 1,053–1,062 MB before and 1,893–1,903 MB after.

The intermediate file-reuse version took 7.53 s per small-edit refresh. Adding
resolution reuse and the final lifetime/one-shot corrections brought this to
7.46 s: about 1% more improvement in this run. The corresponding whole-service
instruction counts were 282.61 versus 282.23 billion, only 0.14% apart. These
results do not establish a substantial additional speed gain from resolution
reuse on this workload, though deterministic tests prove the workers are skipped.

A separate broad-edit check appended a comment to all 4,199 tracked `.ts` and
`.tsx` files. Its single refresh took 10.18 s before and 9.28 s after, with
identical outline output. Dependencies outside those tracked files remained
unchanged and eligible for reuse. This single pair is a regression check, not
a general estimate of broad-edit performance.

The final release was checked again after the review cleanup, in
baseline/final/final/baseline order. This separate series ran under changing
CPU speed limits after compilation, so its wall times are reported separately:

| Final-build verification | Before | Final |
| --- | ---: | ---: |
| Small-edit refresh, median (range) | 13.04 s (11.95–15.28) | 9.22 s (8.89–9.59) |
| Retired instructions, first load plus three edits | 472.88 billion | 282.80 billion |
| Retained physical footprint after edits, median (range) | 1,217 MB (1,089–1,251) | 2,044 MB (1,950–2,156) |
| Peak physical footprint across that service lifetime | 2.669 GiB | 2.574 GiB |

Every output and generation check passed. The final binary retains the roughly
40% instruction reduction and the memory tradeoff; the variable CPU limits
prevent using the difference between the two series to quantify the small
review cleanup's wall-time effect.

A heavier warm reference search for `AgentTaskRunner.run` returned identical
394,916-byte partial results (exit 1) from the baseline and final release.
In this single pair under the same constrained host conditions, it took
14.12 s before and 14.18 s after. Peak physical footprint across first load
plus that query **increased from 2.90 to 3.40 GiB**; retained footprint after
the query was 1,775 versus 2,279 MB. The cache therefore costs memory during
larger semantic queries too, even though the outline-refresh workload has a
lower peak. This sample passed under the unchanged default process limits.

## Implementation

Source text, line/UTF-16 indexes, navigation catalogs, JSON facts, and collection
facts are immutable shared inputs. The cache compares freshly read decoded text
exactly, under the same identity path and library/JSON role. It rechecks the
collection's whole configuration dependency, effective module status, after
package format is known. Per-generation request lists are cloned before adding
synthetic helpers/JSX imports or filling resolution fields.

Collection freezes into two lifetime groups. Published projects share semantic
facts; reusable sessions additionally retain binding recipes. Ordinary Project
loads can still release those recipes after binding. Obsolete cache entries are
released after program construction, before binding. A parse/load failure keeps
the old cache; a later failure discards the candidate and may require a cold
retry. No failed generation becomes a queryable snapshot.

Resolution reuse is qualified by the complete effective configuration options,
configuration paths, and file-name case policy. Each module/type-reference
lookup retains resolved paths rather than FileIds, its logical Host accesses,
exact filesystem observations, and symlink effects. All missing memo entries
must validate speculatively before any are adopted. Current Host and canonical
first answers remain authoritative. Failed or deferred validation falls back
to ordinary resolution, preserving demanded errors and FIFO behavior. Automatic
type discovery stays fresh. Only reached successful decisions survive.

## Measurement method

Measurements used a frozen APFS copy of `~/github/command-center` at revision
`976153bb38de1866e01d671a9c9efad115642d92`. All binaries used the same absolute
copy path, dependencies, configuration and generated type inputs. The original
checkout was never edited. After restoring the edited files, all 5,068 tracked
blobs in the copy matched that revision.

The baseline includes the preceding relative-path preparation optimization.
Its release binary SHA-256 is
179271b24f199a4ff3e4acccf40c22ebb536736aefec618fe28b0209527d02a7.
The intermediate file-reuse binary SHA-256 is
5216c9f9fdf01f0fbdbc76fd1c160290dfdf663476eea60c66b6c8795fb7d311.
The completed reuse build used for the primary table has SHA-256
3a0747017251d82e8abe16f5a7456133ec32cc94ebfdaf53fc74143816cae8d9.
After the review, the only production change was the memo-sharing comparison
cleanup described below; tests and comments also changed. The final release
binary has SHA-256
61eafb2b36ba5c761bd7cd6afa539cc55106238def6745f07bfdb7f7a7f519f3.
Its release build passed `vrun-c65b5cef`. A separate final-build verification
series is recorded in `edits-reviewed.jsonl`; its wall times are not pooled
with the primary measurements because the host reported CPU speed limits of
64–77% after the release build.

The host was an Intel Core i9-9880H, 16 logical CPUs, 16 GiB RAM, macOS 15.7.7.
A spot check during the primary measurements reported a CPU speed limit of 100%
and no thermal warning; the one-minute load average was 3.08. Release builds
use thin LTO and one codegen unit. No local build or test ran during measurement.

Primary small-edit services ran in baseline/intermediate/reuse/reuse/intermediate/baseline
order, each with a first load and three sequential edits. Each edit appended an
export to `src/features/tickets/components/TicketChildSummary.tsx`; the measured
request was its project outline. Direct and warm runs used baseline/reuse/reuse/
baseline order. Each warm service ran six requests apiece for that outline,
references to `getTicketService`, and type at
`src/lib/tickets/service-factory.ts:201:17`. No samples within a series were discarded.

Every service used a fresh isolated runtime directory, was explicitly launched
and stopped, and had its generation number checked. CLI output bytes matched
across versions for every measured query; edited outlines also contained each
new export. After each series the edited inputs were restored. Wall times
include the client request. `/usr/bin/time -l` reports process instructions,
RSS and peak physical footprint; `footprint -p` measures retained service
physical memory.
MB values retain the units printed by `footprint`; GiB values divide the byte
counts from `time` by 2^30. Allocation counts are not presented as physical-memory
savings. OS filesystem caches are warm; these are local measurements on one
real project.

Raw scripts, JSONL results, time reports, binary hashes and validation receipts
are under `.cc/temp/reuse-perf/`.

## Validation and review

The unchanged-source test failed before implementation. The resolution work
avoidance test also failed before its implementation: a body edit ran one
module and one type-reference worker instead of reusing both. Both pass now,
with final answers compared against fresh projects.

Registered checks passed: library tests, reference_session, project_service,
project_path_allocations; semantic/native navigation comparisons; native manifest
and module-specifier oracles; Clippy; formatting. The release build passed.
Astra xhigh reviewed both milestones without correctness findings. Opus 5.5
xhigh reviewed the first milestone; its one-shot-cache and peak-lifetime findings
were addressed. Its second review found no incorrect cache-hit behavior, but
requested tests for replayed symlink effects and current-load answers taking
precedence over old saved decisions. The added tests cover both module and type-reference
symlink effects, both manifest and canonical first-answer precedence, and
new higher-priority dependencies after an intervening cache-hit generation.
TypeScript 7.0.2 confirmed the contextual-spelling fixture. Temporary mutations
independently made both symlink variants fail, made the first-answer tests fail,
and made both module/type-root candidate tests return stale results when
replayed observations were omitted. All mutations were restored. The evidence
comparison cleanup adopts the saved Arc once equal evidence is confirmed,
then leaves an already identical owner in place. Subsequent comparisons take
the pointer fast path without extra reference-count traffic.

The focused test wrapper now accepts the exact resolution-reuse test file,
with the same two build jobs and two test threads. Its 19 harness tests pass,
including rejection of mixed/missing paths and zero executed tests. The new
wrapper was exercised locally before integration because registered wrappers
are resolved from the canonical checkout. Final integration checks still use
registered validation. This is a targeted validation set; the full repository
test and harness suites were not rerun.

Validation receipts include `vrun-90c564bd` (library, session, service and
module-specifier snapshot tests), `vrun-1279f0ce` (semantic/navigation),
`vrun-b2a0670d` (native manifest/module-specifier harness), `vrun-e24d6dc9`
(wrapper harness), `vrun-6ba36976` (format), and `vrun-d10788ca` (Clippy).
The final 11-case focused reuse run also passed after the same-pointer guard.
Review runs were `run-2abcafe8` and `run-5619ea01` for Opus, with two independent
Astra xhigh review checkpoints.

## Limits

Reuse spends retained memory to avoid recomputation. Sessions keep a second
name/reference-syntax inventory used to bind again; the published index already
contains a projection of that inventory. Sharing that inventory is a possible
separate memory optimization. The cache retains one reached graph, not an edit
history, and tests verify obsolete sources are released.

Freshness remains exact: source files and required resolution evidence are
reread; this is not a timestamp cache. Binding and semantic evaluation are not
incremental. Warm unchanged queries continue to use the existing validation
and query path. Broad configuration changes conservatively invalidate resolution
reuse even when only some settings affect a particular lookup. This includes
configuration diagnostic origins, so a layout-only configuration edit can
invalidate resolution reuse. A failure after program construction also drops
the candidate cache; a retry then loads cold. These are deliberate conservative
policies, not claims that reuse after either event would be unsound.
