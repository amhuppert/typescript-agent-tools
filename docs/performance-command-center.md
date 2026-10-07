# Command Center parser and reference performance baseline

This report records the implementation before optimization. See [performance improvements](performance-improvements.md) for the implemented changes, fresh comparisons, and verification.

Measured 2026-10-06. All 5,316 project roots parse successfully. Parsing the 53.0 MB source corpus takes a median **4.10 seconds**. Reference queries expose substantial repeated work when materializing large results; project loading also has a much larger cost than parsing alone. The original project does not load unchanged: standard libraries and JSON modules are outside the documented supported profile, and a valid TipTap module augmentation produces a binding error. The reference timings below use explicit benchmark shims and must not be presented as successful analysis of the unchanged project.

## Scope and method

- Parser revision: `d0e04f1728cf8b5ed083bb667145f512928716dc`. Command Center revision: `7706073d233d45c69b60421d7bb16fc0f778aeb4`; its Git worktree was clean.
- Host: Intel Core i9-9880H, 2.30 GHz, 8 physical cores / 16 logical CPUs, 16 GiB RAM, macOS 15.7.7. Rust 1.96.0; release optimization, thin LTO, one codegen unit. TypeScript oracle: pinned 7.0.2; syntax AST sampling uses the repository’s pinned 6.0.3 projection API.
- TypeScript 7.0.2 selects the root files from the real `tsconfig.json`. This includes tests, scripts, TSX, declarations and generated `.next/types` files. Dependencies and standard declarations are measured in an additional program-graph pass.
- Parser sources are read before timing. One warm-up and five measured sweeps use `ParseOptions::for_file` and the default depth budget of 256. A sweep includes parsing and AST disposal. Per-module samples time parsing through the returned AST, excluding disposal, file I/O, option construction and JSON serialization. Arena node counts include speculative nodes, so they are not counts of only the final tree.
- The reference workload contains **251,748 requests** spread across every project module. It covers 242,480 positions, 1,600 project-wide exact-name requests, 7,667 file-scoped name requests and one missing-name request. Positions cover declarations, imports, exports, parameters, locals, type references, generics, destructuring, JSX, properties and empty positions; explicit cases cover invalid positions and outside-project files.
- Each reference request is timed once; every 128th request is repeated three times. **1,967 repeated samples** assert identical result groups and locations, or identical error kinds and locations. Total calls: 257,649. Query input construction, inspecting/dropping results and JSON serialization occur outside each timer. Distributions use the median of the repeated samples where available and the single sample otherwise. They are distributions over this workload, not repeated-run confidence intervals.
- Requested heap is measured in a separate process with the repository’s allocator accounting pattern; it excludes allocator overhead and stack reservations. Other sessions used the host during this investigation; the broad sweep also overlaps the separate dependency and heap checks briefly. Treat absolute timings as a baseline under host load, not an isolated-machine comparison or evidence of a speedup.
- A late `pmset -g therm` read reports `CPU_Speed_Limit = 33`, with 16 CPUs available; another project’s registered tests were active. The fresh CLI reference calls vary from 34–39 seconds despite small differences in query work. CPU throttling and host contention materially limit absolute timing comparisons; repeat on a cool, idle machine when measuring an implementation change.
- All 5,316 root source hashes were unchanged before and after the investigation. No production parser or reference implementation was modified.

## Parsing

| Measurement | Result |
| --- | --- |
| Project roots / bytes | 5,316 / 53,003,740 |
| Successful at default depth | 5,316; no syntax or depth failures |
| Sweep median / range | 4.101 s / 4.048–4.240 s |
| Throughput | 12.33 MiB/s |
| Per-module p50 / p95 / p99 / max | 0.327 ms / 2.263 ms / 5.834 ms / 23.453 ms |
| Arena nodes | 6,159,574 |

| Source category | Files | Bytes | Sum of module medians (ms) |
| --- | --- | --- | --- |
| declaration | 6 | 61,232 | 2.1 |
| TS | 2344 | 18,515,325 | 1108.6 |
| test | 2199 | 28,816,273 | 2061.3 |
| TSX | 767 | 5,610,910 | 361.4 |

Tests account for most bytes and parsing time. The slowest modules are large workflow tests rather than small modules with runaway parsing. Full sweep time exceeds the sum of per-module parse samples because sweeps include AST disposal and iteration overhead.

| Slowest module | Bytes | Median parse (ms) |
| --- | --- | --- |
| `src/lib/workflows/conversation/actor-implementations.test.ts` | 309,170 | 23.453 |
| `src/lib/workflow-graph/execution-loop.test.ts` | 318,517 | 23.447 |
| `src/lib/workflow-graph/iteration-orchestrator.test.ts` | 315,399 | 22.050 |
| `src/lib/workflow-graph/workflow-manager.test.ts` | 342,467 | 21.100 |
| `src/lib/workflow-graph/execution-route-handlers.test.ts` | 264,757 | 18.617 |
| `src/lib/workflow-graph/runtime-edits.live.test.ts` | 152,379 | 11.771 |
| `src/lib/specs/route-handlers.ts` | 175,539 | 11.594 |
| `src/lib/specs/route-handlers.test.ts` | 174,334 | 11.503 |
| `src/lib/specs/review-service.ts` | 169,254 | 11.050 |
| `src/lib/workflow-graph/execution-loop-parallel.integration.test.ts` | 117,845 | 9.650 |

The complete per-module results, including all five samples, are in the evidence directory. Among files over 10 KB, the lowest measured throughput is `DebugActionCard.test.tsx` at 7.85 MiB/s; `native-write-scalar.ts` and the Cursor worker entry test are additional useful profile inputs.

The native compiler’s complete program contains 7,639 files, including six JSON inputs. All **7,633 supported TypeScript sources**, totaling **75,723,952 bytes**, were exercised. Default-budget sweeps range from 5.678 to 6.438 seconds, including one depth rejection; these are not timings of 7,633 successful default-budget parses. `@cursor/sdk/dist/esm/types/conversation-types.d.ts` exceeds depth 256 at byte 276,864 and succeeds at depth 1,024 in 31.94 ms. This is the existing documented distinction between the general parser budget and the deeper reference-loading budget.

Allocation accounting on the eight selected slow or low-throughput modules shows 1,619–24,976 allocations/reallocations per parse. The 318,517-byte execution-loop test retains 5,597,867 requested AST bytes and peaks at 8,218,394. See `parser-memory.csv`; these counts were not collected in the parser latency runs.

## Reference loading and compatibility

| Configuration | Outcome |
| --- | --- |
| Original project | Unsupported: explicit non-empty `lib`; rejected in 2.06 ms |
| Same roots, libraries disabled | Unsupported: JSON import in `artemis-bug-reporter-execution.ts` |
| Libraries disabled, JSON ambient shim | Binding: `Commands` in `conversation-mention-node.ts:44:13` conflicts with a local declaration |
| Libraries disabled, JSON + TipTap shims | Loads and serves the full query workload |

The TipTap rejection is a compatibility finding: TypeScript 7.0.2 returns **zero semantic diagnostics for that file**, including its `Commands<ReturnType>` augmentation. The original project also passes the pinned native compiler’s `--noCheck --noEmit --incremental false` syntax check. No fix was applied as part of this baseline.

The benchmark config is outside the Command Center checkout. It copies the original compiler options, removes `lib`, sets `noLib: true`, preserves the native-selected roots through absolute `files`, and makes `@/*` paths absolute. It then sets `resolveJsonModule: false` and adds precisely these benchmark-only declarations:

```ts
declare module "*.json" { const value: any; export default value; }
declare module "@tiptap/core" {
  export interface Commands<ReturnType = any> {}
  export class Node {}
  export function mergeAttributes(...args: any[]): any;
}
```

These shims alter JSON and TipTap symbol identity and resolution; additional TipTap exports are unresolved. Standard-library globals remain unresolved. Their errors are counted separately, never treated as fast successful lookups. Source files and local import paths remain unchanged, but result filenames have a longer relative prefix because the benchmark config is in the worktree; that can increase result copying and comparison costs. This run is performance evidence for the supported engine on the real source graph, not a whole-project semantic equivalence result.

| Measurement | Result |
| --- | --- |
| Initial five project loads | 25.44 s, 26.20 s, 20.55 s, 15.65 s, 15.76 s |
| Reference index initialization, median of five loads | 20.55 s |
| Later sweep load | 20.45 s |
| Peak requested heap | 925,401,721 bytes (882.5 MiB) |
| Retained requested heap | 273,153,626 bytes (260.5 MiB) |
| After project drop | 0 outstanding requested bytes |

The **4.10-second parsing sweep** starts with the 5,316 project files already in memory and measures parsing and AST disposal. The **20.55-second reference index initialization** times `ReferenceProject::load`: reading configuration and source files, resolving imports, parsing project files and their dependencies, extracting syntax facts, binding symbols and aliases, and building the index used by reference queries. It includes parsing; it is the total initialization time, not an additional cost on top of 4.10 seconds. Neither measurement includes running reference queries or serializing results.

The different file sets and timing boundaries prevent subtracting 4.10 from 20.55 to obtain an exact binding/indexing cost. The 10-second load profile samples only part of a load: it identifies scanner work, source indexing, allocation and filesystem probes/canonicalization, but should not be read as whole-load percentages.

## Reference queries

| Outcome | Requests |
| --- | --- |
| Answered | 169505 |
| UnsupportedTarget | 69863 |
| Empty | 5317 |
| UnresolvedTarget | 7061 |
| OutsideProject | 1 |
| InvalidPosition | 1 |

| Successful query subset | p50 (µs) | p95 (µs) | p99 (µs) | Max (µs) |
| --- | --- | --- | --- | --- |
| Position | 19.48 | 35615.99 | 222785.63 | 781295.49 |
| Name (project + file) | 41.95 | 3970.98 | 103272.20 | 315268.28 |
| All successful | 20.42 | 33735.34 | 222299.45 | 781295.49 |

59,112 successful requests return occurrences in multiple files. The largest returned result contains 73,719 occurrences. Successful timings are reported separately from empty results and errors so unsupported targets do not make the engine look faster.

| Returned occurrences | Successful requests | p50 (µs) | p95 (µs) |
| --- | --- | --- | --- |
| 1–9 | 108064 | 13.32 | 40.94 |
| 10–99 | 28749 | 52.73 | 218.61 |
| 100–999 | 13843 | 853.31 | 2503.89 |
| 1,000–9,999 | 10064 | 9689.19 | 28656.08 |
| 10,000+ | 8785 | 129040.57 | 257225.33 |

| Slow query (distinct names) | Kind | Groups | Occurrences | Time (ms) |
| --- | --- | --- | --- | --- |
| `expect` | ImportSpecifier | 1 | 73,572 | 781.295 |
| `it` | CallExpression | 1 | 28,632 | 309.783 |
| `describe` | spread | 1 | 7,818 | 188.380 |
| `result` | project-name-spread | 5832 | 19,596 | 144.383 |
| `vi` | spread | 1 | 9,303 | 135.508 |
| `z` | PropertyAccessExpression | 1 | 13,754 | 129.095 |
| `path` | spread | 2 | 4,300 | 128.697 |
| `object` | PropertyAccessExpression | 1 | 2,427 | 88.173 |
| `afterEach` | file-name | 1 | 1,802 | 85.770 |
| `screen` | spread | 1 | 7,024 | 81.855 |
| `input` | project-name-common | 4243 | 19,425 | 79.373 |
| `filePath` | spread | 1 | 5 | 78.289 |

Every request, its source location, outcome, result size and timing is recorded in `reference-queries.csv`; `summary.json` contains per-AST-category distributions and the slowest position queries.

## CLI costs

| Operation | Input / result | Median wall time (ms) |
| --- | --- | --- |
| parse-check | vitest.node.setup.ts | 23.16 |
| parse-check | src/features/agent-profiles/agent-profile-draft.ts | 18.11 |
| parse-check | src/lib/state-store/spec-delivery-plan-repo.ts | 21.55 |
| parse-check | src/lib/workflow-graph/workflow-manager.test.ts | 76.02 |
| parse-check | src/features/session-workflow/components/inspector/ContextDetail.test.tsx | 38.97 |
| reference-cli | K: 1 occurrences | 37986.62 |
| reference-cli | expect: 73,719 occurrences | 34234.83 |

Parse CLI samples use `--check`: they include process startup and reading the file, but no AST JSON. Reference CLI samples include a fresh project load, result construction and compact JSON; three invocations per selected query assert the expected group and occurrence counts. The immutable library project is reused for the broad query sweep. These are different usage patterns and should be evaluated separately.

## Improvements to investigate next

1. **Avoid rebuilding ordered locations on every query.** `src/references/query.rs::group` allocates a `BTreeMap<Location, Occurrence>`, clones each location into its key, rebuilds UTF-16 positions, and drops the duplicate keys when returning values. The query profile shows location conversion, string comparison and allocation/freeing as prominent work. Consider freezing occurrence ordering/deduplication and compact position metadata when the index is built; measure the extra retained memory against lower query latency.
2. **Remove allocation inside the group-sort comparator.** The `first` closure in `query::find` clones a declaration or occurrence `Location` for every comparison. A borrowed comparison should preserve ordering while avoiding repeated filename allocations, especially for project-wide names that select many shadowed symbols. Check both output size and group count when profiling.
3. **Reduce repeated filesystem identity work during loading.** `Loader::enqueue` canonicalizes the path before consulting its ID map, including already captured inputs; the resolver caches module lookups per directory/specifier/mode but shared files are still reached from many directories. Explore a per-load lexical-path-to-identity cache owned by the loader, preserving symlink/case rules and every distinct I/O failure.
4. **Profile scanner/AST allocation on actual large tests.** Start with the execution-loop and actor-implementation tests and the low-throughput scalar CLI handler. Lexer calls and allocations appear in the load profile, and individual parses make thousands of allocations. Measure direct scanner work, repeated speculative scans, identifier payloads and arena fields before choosing a representation change.
5. **Reuse a loaded project for sequences of CLI requests.** The library and `reference_batch` already support this. The measured CLI startup cost includes rebuilding the entire index for each invocation. Any persistent CLI service/cache needs an explicit invalidation contract; first quantify the benefit using the existing batch API.

Standard-library/JSON support and the TipTap binding issue are separate correctness/scope decisions. Resolve them before claiming that the unchanged Command Center project is supported. Keep the same corpus, shims and query selection for any before/after performance comparison, then prove changed behavior against the pinned compiler and the repository’s registered checks.

## Evidence and reproduction

Evidence and the investigation-only harness are under `.cc/temp/performance/`: `inventory.json`, `source-hashes.json`, `metadata.json`, `parse.json`, `parse-files.csv`, `program-parse.json`, `native-project.json`, `queries.json`, `supported-core-references.json`, `reference-queries.csv`, `summary.json`, `reference-memory.json`, `parser-memory.csv`, `cli.json`, process timing logs and the three `.sample.txt` profiles. The full early four-timings-per-position sweep was stopped after the five load samples; its query samples are not included in the final results.

```sh
python3 .cc/temp/performance/build.py
node .cc/temp/performance/inventory.mjs
node .cc/temp/performance/native-inspect.mjs
.cc/temp/performance/measure parse .cc/temp/performance/inventory.json .cc/temp/performance/parse.json
.cc/temp/performance/measure parse .cc/temp/performance/program-inventory.json .cc/temp/performance/program-parse.json
TSPARSE_PERF_LOADS=1 .cc/temp/performance/measure references .cc/temp/performance/supported.tsconfig.json .cc/temp/performance/queries.json .cc/temp/performance/supported-core-references.json
.cc/temp/performance/memory .cc/temp/performance/supported.tsconfig.json > .cc/temp/performance/reference-memory.json
python3 .cc/temp/performance/summarize.py
python3 .cc/temp/performance/cli_probe.py
python3 .cc/temp/performance/report.py
```

Run timing commands sequentially on an idle host for a cleaner comparison. The harness uses local pinned compiler caches and does not install dependencies or modify Command Center. It is temporary investigation code, so no test-first implementation was performed; repeat assertions, native checks, process exits and allocator balance provide the recorded verification. The report lives under `docs/`; large raw evidence remains session-local.
