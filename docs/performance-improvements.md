# Command Center performance improvements

Measured 2026-10-06, following the [original baseline](performance-command-center.md). The implemented reference index prepares ordered locations and combined roles once. On fresh matched queries, median result construction is **1.8–10.8× faster**, depending on result size. This costs **43.1 MiB of additional retained heap**. Project initialization remains around 22–25 seconds. The parser also avoids one temporary allocation per ordinary numeric literal; whole-corpus timing does not establish a parsing speedup.

## Implementation

- `src/references/binding/index.rs` captures UTF-16 positions, sorts declarations and occurrences by their public location, and combines duplicate locations’ role bits when the index is frozen. It retains compact file IDs and interned names alongside positions. Stable sorting preserves the first spelling at a shared location.
- `src/references/query.rs` constructs the public result vectors directly from that prepared index. It no longer rebuilds a `BTreeMap<Location, Occurrence>` or allocates filename copies inside the group-sort comparator. Both position and name queries share this path.
- `src/parser/mod.rs::literal_mode` classifies radix prefixes and scientific notation directly from the token, preserving the existing scanner-rule owner and final flags while removing the temporary lowercase string. BigInt cooking retains its existing operation.
- `benchmark-build` is an optional two-job release build registration; merge-gate selections are unchanged. Its new wrapper was verified directly because CC resolves unmerged registrations and wrappers from the canonical checkout.

## Matched reference queries

The comparison selects **2,514 requests** from the original 251,748-request plan: every 128th request, extra samples in each result-size band, the largest group counts and slowest baseline cases, and explicit error cases. It contains 1,890 answered requests, 525 unsupported targets, 50 unresolved targets, 47 empty results, and one each outside-project file and invalid position. Errors are excluded from successful-query timing.

Each fresh process loads one immutable project and times each query three times. Results are checked for exact equality within a process; complete serialized-result fingerprints, including names, locations, roles, declaration/group ordering, and typed error details, match across both versions and both runs. The process order was baseline, optimized, optimized, baseline. The first baseline preceded the release rebuild and briefly overlapped scoped test compilation; subsequent comparisons ran sequentially without our validation or build processes. A per-query median is computed in each process, then the median of the two process medians is used for the table. These are workload distributions, not confidence intervals.

| Returned occurrences | Answered samples | Baseline p50 (µs) | Optimized p50 (µs) | Ratio |
| --- | --- | --- | --- | --- |
| 1–9 | 944 | 11.02 | 5.96 | 1.85× |
| 10–99 | 332 | 38.35 | 10.38 | 3.69× |
| 100–999 | 222 | 972.47 | 119.94 | 8.11× |
| 1,000–9,999 | 211 | 9891.22 | 1237.57 | 7.99× |
| 10,000+ | 181 | 190418.52 | 17562.60 | 10.84× |

| Operation | Baseline | Optimized |
| --- | --- | --- |
| Initialization, first / second process | 24.94 s / 21.91 s | 24.70 s / 23.05 s |

Initialization includes configuration and file I/O, module resolution, parsing, binding, and preparing the index. Query timers exclude input construction, result inspection/destruction, and JSON serialization. There is no filesystem access to source files during queries. The speedup applies to reused projects; fresh CLI calls still pay initialization.

## Full workload verification

All **251,748 original requests** completed. Every request matches the baseline outcome, group count, occurrence count, declaration count, and distinct-file count. Every 128th request is repeated three times with exact result/error equality. Together with the matched-query fingerprints and compiler oracle, this checks preservation beyond aggregate counts; counts alone do not prove every location is identical.

| Outcome | Requests |
| --- | --- |
| Answered | 169,505 |
| UnsupportedTarget | 69,863 |
| Empty | 5,317 |
| UnresolvedTarget | 7,061 |
| OutsideProject | 1 |
| InvalidPosition | 1 |

| Successful query measurement | Optimized workload |
| --- | --- |
| p50 | 8.73 µs |
| p95 | 3.07 ms |
| p99 | 17.92 ms |
| Maximum | 36.46 ms |

Use the matched-query table for before/after ratios. The original broad sweep ran under different host conditions, so its distribution and total wall time are not a controlled speed comparison.

## Parsing and allocation

Four fresh warmed sweeps used the same 5,316 roots / 53,003,740 bytes, in baseline/optimized/optimized/baseline order, with five recorded sweeps per process and the default depth budget. Every module succeeds in both versions, with identical byte counts and arena-node counts. Source reading and JSON output are excluded; AST disposal is included.

| Version | Process medians | Combined median / range |
| --- | --- | --- |
| before | 3.393 s / 3.755 s | 3.693 s / 3.374–3.799 s |
| after | 3.778 s / 3.736 s | 3.740 s / 3.733–3.839 s |

The ranges overlap, and the first baseline process is noticeably faster than the other three. This does not establish a corpus-wide parser speedup or regression. The allocation benefit is independent of this timing variance: the 2,048-literal behavior test failed at 4,116 allocations before the change and now passes a budget of 2,112, while checking the final literal text and flags.

| Selected module | Before calls | After calls | Removed |
| --- | --- | --- | --- |
| native-write-scalar.ts | 1860 | 1856 | 4 |
| DebugActionCard.test.tsx | 1619 | 1603 | 16 |
| entry.test.ts | 4751 | 4601 | 150 |
| execution-loop.test.ts | 24976 | 24325 | 651 |
| execution-route-handlers.test.ts | 19516 | 19223 | 293 |
| iteration-orchestrator.test.ts | 23514 | 23142 | 372 |
| workflow-manager.test.ts | 22635 | 22266 | 369 |
| actor-implementations.test.ts | 22875 | 22548 | 327 |

These allocation runs use the same filenames/options and a separate instrumented process. All eight retain exactly the same AST heap bytes and arena-node counts as the baseline; the removed strings were transient.

## Index memory tradeoff

| Requested heap | Baseline | Optimized | Change |
| --- | --- | --- | --- |
| Peak | 882.5 MiB | 933.9 MiB | +51.4 MiB (5.8%) |
| Retained | 260.5 MiB | 303.6 MiB | +43.1 MiB (16.5%) |

Both instrumented processes return to zero outstanding requested bytes after dropping the project. Accounting excludes allocator overhead, stack reservations, and RSS; it is separate from clean latency measurement. Positioned occurrence records are larger than the former reference IDs, which explains the retained increase. The larger records trade memory for fewer query allocations and position conversions.

## Validation and limits

- Red/green tests reproduce excessive allocations through the public parser and reference APIs. Coverage checks large result fields, many-group ordering, exact shared type/value results through both entry points, aliases and roles, UTF-16 columns across CR/LF/CRLF/U+2028/U+2029, a BOM on its own line, reverse discovery/file order, and captured contents after later disk edits.
- Registered full Rust tests, Clippy, formatting, and all 74 strict Node harness tests pass. One Rust platform case reports a skip because this filesystem refuses non-Unicode filenames. The TypeScript 7.0.2 oracle agrees with 1,167 markers/position queries and 365 name queries in 36 fixture projects.
- The focused numeric syntax comparison checks final AST structure, positions, flags and scanner hints against the pinned 6.0.3 projection API, plus acceptance of five valid/invalid cases by native TypeScript 7.0.2. The registered reference-semantic check cannot express this AST comparison, so it was run as a narrow diagnostic.
- All 5,316 original source hashes are unchanged. The Command Center corpus and explicit no-library/JSON/TipTap benchmark shims remain the same. The unchanged original config is still outside the supported reference profile; these changes do not add library, JSON, or TipTap compatibility.
- Host CPU speed-limit readings varied from 33 to 45, and other sessions share the machine. The repeated large query gains and allocation budgets are clear; small throughput changes need a cool, idle-host rerun.

The oracle also exposed a pre-existing first-line BOM mismatch: native TypeScript removes a leading BOM when reading a source file, while this loader retains it and counts it in first-line columns. A marked first-line identifier was at column 23 in the native oracle and column 24 here. The new fixture puts its BOM on its own line; the first-line mismatch remained a correctness candidate outside these performance changes. It was later fixed: every analysis read now drops a leading BOM as TypeScript's `decodeBytes` does (`source::decode` and `source::read_text`).

## Evidence and next candidates

Session-local evidence is under `.cc/temp/performance/`: `comparison-{before,after}-{1,2}.json`, `comparison-ids.json`, `comparison-queries.json`, `parse-{before,after}-{1,2}.json`, `memory-{before,after}.json`, `parser-memory-after.csv`, `references-after-full.json`, `numeric-oracle.json`, `improvement-summary.json`, the per-stage thermal/log files, and the build/compare/run/finish/report scripts. The temporary comparison harness is an investigation spike; repeat equality, cross-version fingerprints, complete workload metadata comparisons, source hashes, and allocator balance verify it without a test-first implementation.

The independent parser audit and correctness review are in `memory-bank/agent-runs/`. The review found no confirmed bug and its shared-target coverage recommendation is addressed by the exact public-result test. Remaining candidates are direct scanner/lookahead profiling, filesystem identity work during loading, and avoiding owned result copies where a future API can provide a suitable lifetime contract.
