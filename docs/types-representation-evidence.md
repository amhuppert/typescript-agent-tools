# Captured-source representation spike

This disposable experiment compares eager partial structural descriptions with per-request reprojection from exact captured source. It measures a narrow primitive annotation consumer and proves deterministic replay of the partial description graph on an 18-file sample. It does **not** select the final representation, implement the native checker, migrate the binder, or satisfy the complete representation gate in `docs/types-design.md`.

The Ousterhout designing, reviewing, and evolution/performance guidance was applied: both candidates share the parser, traversal, projection, and consumer; the source representation hides its work from the consumer; measurement scope and missing semantic obligations remain explicit. No production source, shipping command, or API was added for this spike. Test-first was skipped because this is throwaway measurement scaffolding. Initial compile/setup failures are not behavior-level RED evidence.

## Inputs and operating envelope

`capture.py` verifies the Command Center checkout revision is exactly `92fe6ab06fbea2c4b6a305689f8cf336173eef81` and checks each selected corpus file against the previously captured native-program manifest before copying its exact UTF-8 bytes. Original canonical filenames remain separate from capture paths and determine parser mode. The full baseline contains 7,639 program sources and 5,313 tracked TypeScript files; this spike parses **13 selected corpus files plus 5 synthetic controls**, totaling **6,804,524 bytes**. It is a selected sample, not a whole-project or random sample.

The selection includes all five baseline journey files, the three largest tracked TypeScript files, the smallest tracked TypeScript file of at least 2,048 bytes, `lib.dom.d.ts`, `lib.es5.d.ts`, the largest external declaration file, and the largest generated TypeScript input. The manifest records exact per-file bytes, hashes, classifications, capture paths, and canonical original paths. In particular, a library marked external in the baseline is still distinguished from the selected largest non-default external declaration file.

| Input | Bytes | Final sites | Primitive annotations |
| --- | ---: | ---: | ---: |
| `src/lib/specs/execution-service.ts` | 101,599 | 9,649 | 142 |
| `src/lib/specs/workflow-cleanup-port.ts` | 1,891 | 108 | 0 |
| `src/lib/agent-gateway/route-handlers.ts` | 4,130 | 403 | 4 |
| `src/components/CollapsibleText.tsx` | 1,902 | 254 | 1 |
| `src/lib/notepads/service.ts` | 31,656 | 3,388 | 63 |
| `src/lib/workflow-graph/workflow-manager.test.ts` | 342,467 | 34,466 | 53 |
| `src/lib/workflow-graph/execution-loop.test.ts` | 318,517 | 38,251 | 50 |
| `src/lib/workflow-graph/iteration-orchestrator.test.ts` | 315,399 | 36,280 | 159 |
| `src/features/tickets/components/TicketChildSummary.tsx` | 2,052 | 315 | 0 |
| `lib.dom.d.ts` | 2,349,483 | 109,605 | 6,781 |
| `lib.es5.d.ts` | 218,947 | 10,297 | 1,316 |
| `node_modules/@cursor/sdk/dist/esm/vendor/cursor-sdk-shared/delta-types.d.ts` | 1,860,055 | 203,116 | 10,978 |
| `.next/types/validator.ts` | 146,385 | 10,837 | 4 |
| `synthetic-annotated.ts` | 520,560 | 105,002 | 15,000 |
| `synthetic-shallow.ts` | 588,961 | 100,017 | 1 |
| `synthetic-deep.ts` | 317 | 141 | 1 |
| `synthetic-roles.ts` | 152 | 37 | 5 |
| `synthetic-reparse.ts` | 51 | 15 | 1 |
| **Total** | **6,804,524** | **662,181** | **34,559** |

The annotated control has 5,000 sets of explicitly annotated number/string/boolean exports. The shallow control contains 100,000 array elements. The supported deep control has 128 nested tuples. The roles control includes a leading BOM and shebang, CRLF, CR, LF, U+2028, U+2029, and explicit parameter/return annotations. The reparse control exercises top-level `await` reparsing: its arena has 20 nodes and its final traversal contains 15. Only final-tree sites enter descriptions.

The initial 300-tuple control fails this parser with `Maximum syntax nesting depth exceeded at bytes 189..190`, despite the analysis grammar-entry budget being 1,024 and the deep parse stack being 256 MiB. Native TypeScript 7.0.2 accepts the retained 300-tuple source with `--noCheck --noEmit`. This is an observed parser resource failure on valid native syntax, not a grammar rejection. It is retained as an excluded control in the manifest and `limit-control.json`; it was not silently counted as a successful sample or benchmarked as a supported demand. A subsequent invocation of the same saved driver against the exact retained excluded source reproduces that resource error and exits 101; `limit-control.json` records its source hash, argument array, and output. No under/at/over grammar-budget conformance claim is made by this spike.

## Shared implementation and exact proof boundary

The standalone Rust driver calls the production `parse_with_options` with `ParseOptions::for_file(original_filename)`, `DEEP_MAX_DEPTH`, and a dedicated thread using `DEEP_PARSE_STACK_BYTES`. The public library does not expose the project's private analysis capability, so the driver reproduces its named public depth/stack constants without adding another parser. `Ast::walk`, `Ast::children`, and `Ast::token_span` own final-tree traversal, logical child fields, and token spans. Arena ordinals are temporary lookup indices; recorded sites are final-tree preorder numbers. `Ast::children` also excludes the `assertClause` alias according to its existing owner.

Both candidates use the same `inventory`, `project`, and `annotation_primitive` operations. An eager description stores one 40-byte `Term` per final node and one 4-byte `Site` per logical child edge. A term contains the static syntax-kind string reference, token start/end bytes, flags, child slice, and an optional direct `type` field link. Terms and edges are boxed slices. Reprojection parses the captured file and reconstructs that same description when first demanded, then caches it for the rest of the request.

These are **partial structural descriptions**, whose footprint is a lower bound only for extending this exact prototype layout with the missing facts. It is not a universal lower bound on a future full representation: smaller kind codes, different node inventories, or more compact fields could change the layout. They omit cooked literal/name/key payloads, most named semantic field roles, binding links, provenance, contextual recipes, flow facts, member/signature tables, factory state, and checker caches. Traversed expressions are present structurally, but the consumer does not evaluate expressions. This representation cannot serve the complete evaluator as it stands. Its footprint cannot be projected into full semantic term size or whole-project retained memory.

The consumer takes a declaration site and its direct primitive annotation site, checks the frozen `type` link and exact token span, and returns the annotation's primitive type. It does not infer the declaration's value type, resolve aliases or lexical names, instantiate generics, select overloads, traverse members, or apply flow. For example, the direct annotation of an optional declaration remains the primitive annotation type; this experiment does not answer the declaration's optional value type. Seeds come from actual AST fields, never from spelling-based binding guesses.

The implemented primitive mapping recognizes 11 keyword kinds. The selected native comparison samples exercised only `any`, `boolean`, `number`, and `string`; the other keyword outcomes are not independently proven here. All Rust seeds and all timed answer checks pass, but their expected labels are produced by the same primitive keyword owner, so those checks alone are not compiler conformance evidence.

Before timing, every term, flag, span, annotation link, child link, and seed is compared for exact equality after a second parse/projection. Equality is also checked during each ingest/projection iteration. This proves replay for the defined partial graph on the captured inputs, including the final reparse tree. It does not prove replay of binding identities, complete semantic roles, JSON duplicate origins, or absent data that the partial projection never records.

`native-annotations.mjs` independently asks TypeScript 7.0.2 for the type at the first up to three primitive annotation sites in each demanded file: **40 comparisons, 40 matches, no error types**. It uses one native checker for the pinned corpus and one for the controls, checks configuration/program/syntax/bind failures, and confirms exact original-versus-captured bytes. The native reader removes the roles control's leading BOM; the probe accepts only that one-code-unit difference, verifies all remaining text exactly, records both hashes, and subtracts one UTF-16 code unit when mapping its annotation offsets. Rust and the capture retain the BOM. These two shared native checker sessions are primitive-leaf evidence, not the fresh-request full-checker/prelude baseline required for broader queries.

## Measurement method

The production release library was built through registered `benchmark-build`, run `vrun-c702098b-9e69-406f-bb59-490f615d021d`. Only the disposable driver is compiled directly with `rustc`; exact argument arrays are in `build-commands.json`. Thin LTO is necessary to consume the registered release library's LLVM bitcode on this machine. The platform is macOS 15.7.7, Intel Core i9-9880H at 2.30 GHz, Rust 1.96.0 (`ac68faa20`, 2026-05-25).

Two separate binaries avoid conflating allocator instrumentation and elapsed time:

- `spike-time`: tracking disabled at compile time; **5 sequential rounds** per operation. Tables report the median. This is an in-process warm measurement with no process-launch, file-reading, CLI formatting, or service work in the timed regions. Eager and reprojection ingestion order alternates each round. CPU isolation and system-load telemetry were not collected; other work in the session may affect elapsed time.
- `spike-memory`: identical algorithm with global requested-heap accounting; **2 rounds**. It counts allocation/reallocation calls and requested bytes, not resident memory or allocator metadata. Its timing columns are retained in raw evidence but are not used for the timing tables.

Source strings, reference descriptions, earlier per-file retained descriptions, and the deep thread stack predate measured regions. Incremental peaks therefore exclude this existing live baseline; they are not process peaks or complete Project-load memory. The live allocation counter never resets, and each sample records peak above its current baseline and retained delta after returning. Allocation ranges in the two accounted rounds are identical for the reported operations. A global teardown check releases all retained description payloads and passes.

Runtime assertions remain enabled in the driver. The annotation link/span checks in both consumers and seed equality inside reprojection demands are timed. Full graph equality after ingest/projection is checked outside its measured region. These are the costs of this scaffold, not a minimum possible production implementation.

The stage names are intentionally narrower than production load: `parse-only` parses and drops an AST; `reprojection-ingest` parses/inventories and retains seed records; `eager-ingest` adds the description projection and retains descriptions plus seeds; `projection-only` starts with an AST and inventory already alive. They do not include configuration, libraries, module resolution, canonical binding, Project freeze/publication, refresh, or edit handling.

## Timing observations

Milliseconds below are medians from the tracking-disabled binary, rounded to three decimals. No instrumented elapsed times are pooled into them. Nanosecond ranges for every operation remain in `summary.json` and `time.tsv`.

| Input | Parse only | Reprojection ingest | Eager ingest | Projection only | One primitive demand with reprojection |
| --- | ---: | ---: | ---: | ---: | ---: |
| `execution-service.ts` | 3.988 | 5.684 | 5.950 | 1.846 | 7.323 |
| `workflow-manager.test.ts` | 19.821 | 24.835 | 29.361 | 5.552 | 27.539 |
| `lib.dom.d.ts` | 71.167 | 81.472 | 112.432 | 31.071 | 112.342 |
| `delta-types.d.ts` | 106.010 | 131.030 | 199.413 | 51.037 | 181.720 |
| Annotated control | 54.364 | 66.906 | 86.726 | 21.835 | 88.572 |
| Shallow control | 58.552 | 62.716 | 84.654 | 16.448 | 84.829 |
| Deep control | 0.064 | 0.069 | 0.078 | 0.010 | 0.078 |
| Reparse control | 0.009 | 0.011 | 0.012 | 0.002 | 0.012 |

The sum of per-file stage medians is 446.881 ms for reprojection ingest and 607.821 ms for eager ingest. This sum is not a measured whole-project load or a median of aggregate loads. It shows the scaffold's additional upfront projection work across this sample.

| Input | Annotation count | Eager all-annotation consumer | Reproject all annotations | Reproject up to 16 annotations |
| --- | ---: | ---: | ---: | ---: |
| `execution-service.ts` | 142 | 0.002 ms | 9.109 ms | 6.969 ms |
| `lib.dom.d.ts` | 6,781 | 0.510 ms | 112.385 ms | 115.999 ms |
| `delta-types.d.ts` | 10,978 | 1.080 ms | 184.865 ms | 191.364 ms |
| Annotated control | 15,000 | 0.457 ms | 91.071 ms | 85.082 ms |

The eager single and up-to-16 consumers complete below 0.001 ms in these samples. At that scale timer overhead and cache warmth prevent useful precise claims about ratios. The tables demonstrate the file parsing/projection overhead added to a deliberately tiny admitted consumer; they do not predict the latency of expensive native checking.

The cross-file request issues two up-to-16 primitive demands per file. Sixteen files contain admitted annotations; two contain none. The request-local reprojection cache performs **16 actual reparses**, even with two demands per file, and both alternatives return the same checksum for **322 answers**. The tracking-disabled median is **612.291 ms** for reprojection, range **564.657–632.406 ms**, and **0.017 ms** for the already-retained eager consumer. The `inventorySites` counter in the raw `ALL` rows describes the entire 18-file inventory, including the two undemanded files; it is not a count of sites actually reparsed in that request. Demand discovery is seeded in advance, so these figures exclude semantic selection and binding work.

## Requested-heap observations

Bytes below come only from the instrumented binary. Retained bytes are after a measured ingest returns; peaks include transient parser/inventory/projection allocations above the pre-existing baseline. Sources, the dedicated stack, canonical bindings, and full-checker storage are outside these totals.

| Input | Reprojection ingest retained | Eager ingest retained | Eager / one-reprojection incremental peak |
| --- | ---: | ---: | ---: |
| `execution-service.ts` | 5,120 | 429,672 | 2,284,617 |
| `workflow-manager.test.ts` | 1,280 | 1,517,780 | 8,029,339 |
| `lib.dom.d.ts` | 163,840 | 4,986,456 | 24,162,856 |
| `delta-types.d.ts` | 327,680 | 9,264,780 | 46,010,309 |
| Annotated control | 327,680 | 4,947,764 | 25,070,956 |
| Shallow control | 80 | 4,400,824 | 29,482,382 |
| Deep control | 80 | 6,280 | 29,323 |
| Reparse control | 80 | 736 | 4,554 |

Across all 18 files, the partial term and child payloads total **29,135,892 bytes**. Retained seed vector capacities total **874,880 bytes** in both alternatives; eager ingest retains **30,010,772 bytes** of descriptions plus seeds. This excludes source text and collection headers. Dropping the accumulated eager description collection releases 29,136,916 accounted bytes, including its outer vector allocation, satisfying the payload teardown assertion.

The cross-file reprojection request reaches an incremental requested-heap peak of **56,685,037 bytes** and makes **481,608 allocation/reallocation calls**. Its request-local descriptions and cache are released before the measured region returns, yielding retained delta **0**. The eager primitive consumer has incremental peak/retained delta/calls **0** because the descriptions already exist before the measured request. An actual semantic request would still require checker/factory caches and result allocations; this zero is confined to this consumer.

## What remains open

The experiment supplies measured parse/projection overhead, a limited direct annotation consumer, partial structural replay, and a teardown check. It leaves the provisional captured-text reprojection choice unchanged.

Before choosing a production representation, the shared canonical binder must retain complete semantic meanings, binding/value/alias identities, type-only provenance, exact payloads, and final publication order described in `docs/types-binding-migration.md`. Both candidates then need the same complete internal description contract and native demand consumer. Factory, imported generic, contextual, flow, member/signature, JSON-origin, overload, checker-history, refresh, and service journeys remain unimplemented and unmeasured here. No cache disposal/session freshness or one-shot/service ownership contract was exercised.

The selection of large declarations and synthetic files makes this sample useful for cost bounds, but does not establish whole-corpus coverage or general latency. Hot in-process rounds are not fresh requests. The 40-byte structural term is a partial prototype layout, and its static string pointer is an in-process implementation choice rather than a serialization contract; its size does not lower-bound all possible complete representations. The 300-depth resource failure remains separate evidence against claiming all native-valid sources supported. No full native-checker journey, final representation gate, or production performance guarantee passes on the strength of this spike.

## Reproduction and provenance

All code and data are scratch artifacts under `.cc/temp/types-representation/`:

- `capture.py`, `capture-manifest.json`, `inputs.tsv`, and `sources/`: exact source capture and controls.
- `spike.rs`, `spike-time`, `spike-memory`, and `build-commands.json`: shared driver, distinct binaries, and exact compiler arguments.
- `time.tsv`, `memory.tsv`, `summarize.py`, and `summary.json`: raw evidence and separate aggregation.
- `native-annotations.mjs`, `native-annotations.json`, and `tsconfig.json`: the pinned native primitive comparison.
- `limit-control.json`: native-valid excluded depth control and observed tool failure.
- `machine.json` and `provenance.json`: environment, command argument arrays, exact artifact hashes, and source-hash limits.

Run argument arrays for the two measurement binaries are `[".cc/temp/types-representation/spike-time", ".cc/temp/types-representation/inputs.tsv", "5"]` and `[".cc/temp/types-representation/spike-memory", ".cc/temp/types-representation/inputs.tsv", "2"]`. The native probe uses the existing `find_semantic_compilers` bootstrap in `scripts/validate/common.sh`; it does not accept an unpinned compiler fallback. No shipping validation runner was invoked directly.

Key SHA-256 hashes:

| Artifact | SHA-256 |
| --- | --- |
| `spike.rs` | `d1547ff03b2335e0a94117bbec0fd5af3741d7a3a13ebd06de970054d8a6e945` |
| `capture-manifest.json` | `31c56d95ae07810d7bdb8a76ebe2e1a550b3249ff186ffd9701af44df56b9108` |
| `time.tsv` | `a3a1f082931c72191429a2aac08de0b157e323ec68e030aa9c1a7baa8b7e8635` |
| `memory.tsv` | `c78d9fb60e30c0f1facc681ad746bfde9091f691456e69beef299649ee617628` |
| Measured release `libtypescript_parser-4e647b3bc63d4aa4.rlib` | `f0a34963655ffd15f10f6abbf94ffeeda33d4f17eeac312cbcf20a1f949463bd` |

`provenance.json` records the remaining code, binaries, native compiler, oracle, and result hashes. The parser checkout has authorized uncommitted changes; the recorded current source-tree digest is a report-time digest, not a build-time snapshot. The exact measured release library and driver binary hashes bind the recorded results to their artifacts without claiming a complete reproducible build capture.
