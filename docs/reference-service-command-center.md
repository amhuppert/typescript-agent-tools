# Reference service on Command Center

Measured 2026-10-07 against `/Users/alex/github/command-center` at
`7706073d233d45c69b60421d7bb16fc0f778aeb4`. Repeated CLI queries reuse the index
and take **0.71–0.94 seconds**, versus **10–12 seconds** for direct loading in
this run. At the time the unchanged configuration could not load; these
performance results use the compatibility adjustments described below. It has
loaded since standard libraries, JSON modules, and the augmentation fix landed;
[Command Center performance](performance-command-center.md#current-state-2026-10-08)
measures the service on the unchanged configuration (2026-10-08).

## Configuration and compatibility

TypeScript 7.0.2 selected **5,316 roots totaling 53,003,740 bytes** from the real
configuration. The benchmark uses those absolute filenames, including tests,
TSX, declarations, and generated files. Installed dependencies remain available
through their original paths. Command Center's tracked worktree was clean before
and after, and all 5,316 root content hashes stayed identical.

The following checks reproduced the existing limitations:

| Configuration | Result |
| --- | --- |
| Unchanged `tsconfig.json` | Exit 1: explicit `lib` is unsupported, at 4:12 |
| Libraries disabled | Exit 1: JSON import in `artemis-bug-reporter-execution.ts:2:29` |
| Libraries disabled, JSON shim | Exit 1: TipTap `Commands` binding conflict in `conversation-mention-node.ts:44:13` |
| Libraries disabled, JSON and TipTap shims | Loads and answers the measured queries |

The unchanged config produced identical diagnostics and exit status in direct
and service modes. Rejections are excluded from the successful-query timings.

All benchmark files live in this parser worktree under
`.cc/temp/command-center-service/`. The config copies the original compiler
options, removes `lib`, sets `noLib: true` and `resolveJsonModule: false`, makes
`@/*` targets absolute, and lists the native-selected roots explicitly. It adds
the exact JSON and TipTap declarations from the
[earlier benchmark](performance-command-center.md#reference-loading-and-compatibility),
plus a small source used only to test refresh. No Command Center file is edited.

These adjustments change JSON and TipTap symbol resolution, leave standard-library
names unresolved, and freeze the root list for this measurement. This is a test
of persistence on the real source corpus under the supported profile, not a
claim that the unchanged project is supported or semantically equivalent to
TypeScript. Wildcard root discovery and invalidation have separate session tests.

## End-to-end timings

Every sample launches a separate CLI process. Timings include startup, file I/O,
freshness validation, result construction, IPC where used, complete JSON output,
and exit. Direct measurements precede service startup. The small-function direct
value is the median of three loads (9.812–10.424 seconds); the other direct values
are single samples. Each warm value is the median of seven calls.

| Query | Occurrences | Direct | Warm service | Ratio |
| --- | ---: | ---: | ---: | ---: |
| `getPublishedManagedSkillBundle`, scoped to its source file | 14 | 10.011 s | 0.709 s | 14.13× |
| `cn`, selected at `src/lib/ui/cn.ts:15:17` | 974 | 11.165 s | 0.711 s | 15.71× |
| Project-wide name `expect` | 73,719 | 11.733 s | 0.945 s | 12.42× |

Warm ranges are 0.673–0.831, 0.695–0.769, and 0.935–1.153 seconds respectively.
The result sizes are 3,525, 192,087, and 14,929,177 bytes. The large response
exercises serialization and transport as well as index reuse.

| Additional measurement | Result |
| --- | ---: |
| Cold service, small-function query | 16.612 s |
| Three simultaneous warm clients, total elapsed | 2.276 s |
| Service RSS after warm queries | 677,528 KiB (662 MiB) |
| Full refresh after editing the benchmark-only source | 18.628 s |
| Direct load after that edit | 17.050 s |
| Next warm query after refresh | 0.818 s |

The host is an Intel Core i9-9880H at 2.30 GHz with 16 GiB RAM, running macOS
15.7.7. It is shared with other development sessions; the final thermal-status
read reported a CPU speed limit of 45%. Timings varied during preliminary runs.
The table records one complete final run, not the fastest samples across runs.
Ratios are observations under that load, not isolated-machine confidence bounds
or comparisons against the older implementation.

## Correctness and design checkpoint

All successful service responses matched the complete direct-mode output bytes.
The same PID and generation 1 served all 21 warm samples and the three concurrent
requests. Concurrent clients received their respective correct responses; the
service serialized their work without rebuilding.

Inserting a newline into the benchmark-only source changed the selected symbol's
locations. The next response differed from the old response, matched a new direct
load byte for byte, and advanced to generation 2 in the same process. A subsequent
real-source query still matched its original output. Cleanup stopped both the
compatibility-check service and the benchmark service; no benchmark service
remained running.

The real-corpus result supports keeping the resident-process design: ordinary
callers reuse the index without managing its lifecycle. It also confirms the
current limits. Warm calls still validate all captured filesystem observations;
they are not sub-100-ms lookups. Cold startup and post-edit refresh still construct
a full index. Memory residency is about 662 MiB in this run, including allocator
retention and process overhead, rather than only live index allocations. No
production change was needed for this measurement. Compatibility fixes and
incremental refresh remain separate work.

## Evidence and reproduction

The registered release build passed:
`cctl validate run benchmark-build --scope full --queue-if-busy`, run
`vrun-eacc09ed-a0ce-4834-87a4-19f208603a8a`. The binary includes the service changes
over base revision `3df7cdf938d1fa47d3eab77d9c15769f4dfd9f19`; its SHA-256 was
`9ba5bda4152766116332144d1db2cd58067ee5ecfb139f714a2572a79ce8f139`.

Session-local evidence and throwaway reproduction scripts:

- `.cc/temp/prepare-command-center-service.mjs`: native root selection, external
  configs/shims, and the source hash manifest.
- `.cc/temp/benchmark-command-center-service.py`: compatibility, timing, exact
  output comparisons, concurrent clients, refresh, and cleanup assertions.
- `.cc/temp/command-center-service-results.json`: final successful run, raw
  sample arrays, result hashes, process generations, and source-integrity checks.

After the registered build, run the preparation script with Node and the probe
with Python 3 from this worktree. Paths in the preparation script name this
machine's pinned compiler installation and Command Center checkout. The probe
uses its own private runtime directory and always stops its services.
