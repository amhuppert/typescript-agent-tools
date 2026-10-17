# Making Compilers — and the TypeScript Type Checker — Fast and Lean: A Practitioner's Report (October 2026)

The biggest gains in compiler speed and memory come from architecture, not micro-tuning. Choose compact, cache-friendly data (integer IDs, arenas, interning). Make the work demand-driven and memoized. Avoid algorithms that can go quadratic or exponential. Then use shared-memory parallelism over independent units. TypeScript 7 (the Go port, GA on July 8, 2026) shows this: it kept the checker's algorithms and semantics essentially unchanged and still got 8–12x faster full builds from native code plus shared-memory parallelism.\[1\] The remaining headroom in a TS checker is algorithmic (relation caching, variance shortcuts, union handling, instantiation control) and data-layout work, which the TypeScript team is only now starting.

## TL;DR

- **How fast compilers are built:** Flat, index-based, arena-allocated data structures (Zig, Carbon, Oxc, rustc). Pervasive interning so equality is a pointer or ID compare. Lazy, memoized, query-based computation with early cutoff (rustc queries, Salsa). Few passes over the data (the esbuild FAQ says it "only touches the whole JavaScript AST three times"). Parallelism wherever global state allows. Measure everything continuously.
- **TypeScript specifically:** TS 7.0 delivers 7.7x–11.9x faster builds at the default 4 checkers (VS Code: 125.7s → 10.6s) and 6–26% less memory.\[1\] It runs N independent checkers that each own private type state over a shared immutable AST; the cost is duplicated work and memory.\[2\] Inside the checker, the dominant costs are structural relations, generic instantiation, union reduction, and control-flow narrowing, and each is guarded by ID-keyed caches and hard limits.
- **What to do:** For users: adopt TS 7, use interfaces over intersections, annotate exported return types, avoid giant unions and deep recursive conditional types, use `skipLibCheck`, `types: []`, project references, `isolatedDeclarations`, and profile with `--extendedDiagnostics` and `--generateTrace`. For checker builders: lead with interning, ID-keyed relation caches, variance shortcuts, deferred and lazy resolution, compact node and type layout, and a parallelism model designed in from day one rather than bolted on.

---

## Part 1 — General performance engineering for compilers

### 1.1 Memory management: arenas, bulk freeing, GC pressure

**Arena (bump/region) allocation is the default for compiler IRs.** Compiler data has a lifetime structure. Tokens, AST, HIR, and types for a compilation unit are created together and die together. A bump allocator turns allocation into a pointer increment and deallocation into "drop the whole region".

- **rustc.** The rustc dev guide states that "since A LOT of data structures are created during compilation, for performance reasons, we allocate them from a global memory pool," each "allocated once from a long-lived arena". When compilation finishes "all the memory related to that buffer is freed". References carrying the `'tcx` lifetime point into those arenas.\[3\] Historically rustc also used an inner arena for type inference, "thrown away once type inference is over", to reduce peak memory.\[4\] In today's `GlobalCtxt`, the arenas are `WorkerLocal`, meaning per-thread, which matters for the parallel front end.\[5\]
- **Oxc** allocates the whole AST in a bump arena (`oxc_allocator`). Its parser benchmark shows the payoff in *drop time*. With a large-drop harness, "there is only a 0.3ms difference for oxc, but 7ms difference for swc," and "AST drop time can become a bottleneck in applications such as a bundler, where there are a few thousands of files need to be parsed." Oxc also uses `u32` span offsets instead of `usize` and mimalloc as its global allocator.\[6\]
- **Clang/LLVM** use `BumpPtrAllocator` for ASTContext nodes and long-lived IR, together with containers that avoid heap traffic (`SmallVector` inline storage, `StringRef` non-owning views, `DenseMap` open addressing). These are standard LLVM idioms documented in the LLVM Programmer's Manual. I did not re-verify specific numbers for this report.
- **typescript-go** also uses arenas inside a GC'd language. A May 2026 analysis of tsgo memory (zackoverflow.dev) shows a `symbolArena core.Arena[ast.Symbol]` and found that AST arenas made up about 595 MB, or 45% of live heap, on its test project.\[7\] A secondary summary of Anders Hejlsberg's talk says identifiers (about 30% of nodes) are allocated in chunks of 256 with interior pointers handed out.\[8\] I could not confirm this from a primary source.

**Managed runtimes.** In V8, Go, and the JVM, the enemy is *object count* and *pointer density*. The GC must trace every live pointer, and allocation rate drives collection frequency. Tactics that carry over:

- Allocate fewer, larger objects. Use slabs or chunks of same-typed structs (Go: `[]T` slabs with interior pointers; JS: typed arrays for hot numeric data).
- Prefer integer indices to pointers. They are invisible to the GC's tracer and halve reference size on 64-bit.
- Keep object shapes monomorphic in V8. One TS-specific fact: the old JS compiler used one constructor for all node kinds, so each JS `Node` cost about 160 bytes (Swatinem's analysis of tsc).\[9\]
- Generational GC suits compiler workloads well (lots of short-lived temporaries, a large long-lived AST and type graph), *provided* the long-lived graph is not constantly mutated. Write barriers on old-to-young pointers add cost. Arena-style chunking avoids most of it.
- **Escape analysis** (JVM C2, Go's compiler) can stack-allocate temporaries. Design hot helper functions so results do not escape: return values, not pointers, and pass scratch buffers in.

esbuild's author gives a concrete warning about JS worker threads. Because "JavaScript has a separate heap per JavaScript thread," his testing suggested worker-thread parallelism gets cut roughly in half, "presumably since half of your CPU cores are busy collecting garbage for the other half." Go's shared heap avoids this.\[10\]\[11\] That is one reason both esbuild and TypeScript 7 chose Go.

### 1.2 Data layout: data-oriented design, flat ASTs, compact encodings

**Struct-of-arrays and index-based trees.** Andrew Kelley's "Practical Data Oriented Design" talk is the canonical case study. Rewriting the Zig compiler around compact, index-addressed data produced a "20% reduction in wall clock time in the tokenizer/AST creation, and 40% reduction in wall clock time in the IR generation," per a golang-dev thread summarizing the talk. Zig's `MultiArrayList(T)` stores each field of `T` in its own contiguous array, carved from a single allocation.\[12\]\[13\] Mitchell Hashimoto's walkthrough of the Zig parser gives the arithmetic: a struct with 8 bytes of padded fields per element becomes 5 bytes per element in SoA form, "37.5% less memory". He also notes that "almost every structure (tokens, AST nodes, future IR nodes, etc.) is stored as a MultiArrayList". Zig puts rarely-used node payload in a side `extra_data` array rather than widening every node. Making each node larger "makes the overall parser slower".\[14\]

The general recipe:

- Nodes are `u32` indices into typed arrays: `tags[]`, `main_token[]`, `data[]` (two `u32` operands).
- Variable-arity children go into an `extra[]` array referenced by (start, len).
- Child references are 4-byte indices, not 8-byte pointers. The whole tree can be serialized or memory-mapped without pointer fixups.\[15\]
- A pass that only needs node kinds streams through `tags[]` and never touches the rest. That is the cache win.\[15\]

**Carbon's toolchain** applies the same philosophy end to end (token buffer → parse tree as a postorder array → SemIR as flat instruction arrays). Its stated targets are very aggressive, on the order of 10M lines/second for lexing and parsing and 1M lines/second for semantic analysis. A Hacker News commenter (Virgil's author) doubted the parsing target ("400MB/s") but thought the semantic-analysis target achievable.\[16\] Treat these as design goals, not measured results. LuaJIT's IR is an older precedent: 64-bit instructions with 16-bit operand references, "No trees, no pointers, no cry."\[16\]

**Roslyn red/green trees** (Eric Lippert's well-known description) separate an immutable, parent-less, position-relative "green" tree, which can be shared and reused across edits, from a lazily materialized "red" facade with parent pointers and absolute positions. This is the canonical design for an IDE-grade, incrementally reparsable syntax tree. Its cost is that red nodes are allocated on demand during navigation. rust-analyzer's rowan and Biome's CST follow the same pattern. That is also why Oxc warns its parser comparison with Biome is "not an apple-to-apple comparison… Biome's parser produces a CST instead of an AST, which requires a lot more work."\[6\]

**Compact encodings.** Store flags as bitsets (TypeScript's `NodeFlags`, `TypeFlags`, `ObjectFlags`, and `SymbolFlags` are all bitmasks, so union-of-kinds tests are a single AND). Tag pointers in their low bits: rustc's `GenericArg` is "an interned pointer, with the lowest 2 bits being reserved for a tag."\[17\] Move cold optional fields out of hot records.

**Cache lines and NUMA.** Keep hot fields of a node in the first 64 bytes. Avoid false sharing between per-thread counters, for example by padding per-worker stats. For NUMA, prefer per-worker arenas (rustc's `WorkerLocal`) so that each thread mostly touches memory it allocated.

### 1.3 Interning and hash-consing

Interning turns structural equality into identity equality and deduplicates memory.

- **Strings and identifiers:** a global table maps text to a `Symbol(u32)`. The rustc-dev-guide glossary defines interning as "storing certain frequently-used constant data, such as strings, and then referring to the data by an identifier (e.g. a Symbol) rather than the data itself, to reduce memory usage and number of allocations."\[18\]
- **Types (hash-consing):** rustc interns every `Ty` in `TyCtxt` (the "central data structure of the compiler," which "stores references to the various arenas and also houses the results of the various compiler queries").\[17\] Two `Ty`s are equal iff their pointers are equal.
- **TypeScript** interns at the level of *type construction*. Unions are cached by a key built from the sorted constituent type IDs (`getTypeListId(types)`, plus an alias suffix) and looked up in `unionTypes`. Subtype reduction is cached by the same list ID (`subtypeReductionCache`), and signature instantiations by the type-argument list ID.\[19\] Literal types are interned per value. In tsgo the checker holds `unionTypes map[CacheHashKey]*Type` and `unionOfUnionTypes map[UnionOfUnionKey]*Type`.\[7\]
- **Canonicalization prerequisite:** to hash-cons composite types you need a canonical order, such as sorted IDs for unions. In a parallel checker, ID assignment order affects that canonical order. This is exactly why TS 7 introduced `stableTypeOrdering` (see Part 2).\[20\]

### 1.4 Algorithmic techniques

- **Avoid hidden quadratics.** Pairwise union reduction is O(n²). The TypeScript wiki says so explicitly ("to eliminate redundant members from a union, the elements have to be compared pairwise, which is quadratic").\[21\] Use fast paths for literal-only unions, sorted-ID membership tests, and discriminant indexing.
- **Union-find** for unification-based inference (HM-style). It runs in near-linear time with path compression and union by rank. TypeScript does not use it; it uses local, bidirectional inference with candidate lists. ML-family checkers, Flow's older constraint engine, and many Rust trait-solver components rely on it or equivalents.
- **Worklists and fixpoints** for dataflow (control-flow narrowing, liveness). Bound iteration counts.
- **Memoization at every expensive pure function**, keyed by cheap integer tuples.
- **Lazy, demand-driven computation.** Compute a declaration's type only when someone asks. TypeScript's checker has always been lazy in this sense (`getTypeOfSymbol` resolves on demand with circularity detection). rustc's query system and Salsa formalize it.
- **Early termination.** Relation checks return at the first failing member unless an error report is needed. Error elaboration should be a separate, slower re-run, which TypeScript does.

### 1.5 Incremental compilation and IDE latency

**Salsa / rustc red-green.** The database keeps a revision counter. Setting an input bumps it. Each memoized query records its dependencies and the revision in which its value last changed. On re-query, Salsa walks dependencies to verify, re-executes only when needed, and **backdates** results that came out unchanged. This "early cutoff" stops invalidation from spreading.\[22\]\[23\] matklad's "Durable Incrementality" adds *durability levels*. Standard-library files are marked durable and workspace files volatile, so a change to a volatile input can skip validating everything that depends only on durable inputs. The practical lesson from that post: "The bulk of work using an incremental system such as Salsa is figuring out things like this — introducing effective early cutoff shields and preventing volatile details like positions from accidentally sneaking in."\[24\] For example, have a positionless AST query feed semantics so that edits to whitespace don't invalidate types.\[25\]

**Flow's "types-first"** is the most relevant comparison for a TS checker. Before it, Flow had to check files in dependency order, producing "peak memory blowups, long garbage collection pauses, amplification of non-linear time complexity, and low use of parallelism."\[26\] Types-first requires full annotations at module boundaries, extracts a *signature* from every file first, and then checks file bodies in parallel against signatures only.\[27\] The Flow team's "Improvements to Flow in 2019" post reported changes "that improved our recheck times by 70%", and the types-first post cited "rechecking speedups of ~6x in the p90 and ~2x in the p99"; Flow made types-first the only mode in v0.143 (January 2021). TypeScript's `isolatedDeclarations` (TS 5.5) is the same idea expressed as an opt-in constraint.\[28\]

**TypeScript's incrementality** is file-grained. `--incremental` writes `.tsbuildinfo` with per-file signatures (hashes of declaration output), so a change whose `.d.ts` shape doesn't change doesn't recheck dependents.\[21\] `composite` and project references give a coarser, project-level DAG. The checker itself is *not* query-incremental: a program edit discards the checker and re-derives types lazily. The language service hides this by reusing ASTs (the document registry) and only checking what's on screen.

### 1.6 Parallelism and concurrency

- **Embarrassingly parallel phases:** lexing, parsing, binding, per-file emit, and codegen. TS 7 parallelizes parsing, type checking, and emit; parsing and emit "can mostly be done independently across files".\[1\]
- **The global-state problem.** Type checkers memoize into shared tables: type IDs, relation caches, instantiation caches. There are three designs:
  1. **Fine-grained concurrent shared state** (rustc's parallel front end: sharded, lock-protected interners and query caches). rustc's working group reported that "compile times can be reduced by up to 50%" with `-Z threads=8` on real-world code, though "the effects vary widely".\[29\] The MCP measured about a 30% average clean-build reduction on 8 cores and no gains from going to 32 threads on 48 cores; the Rust Blog adds that in single-threaded mode the parallel front end is "typically 0% to 2% slower than with the serial front-end."
  2. **Replicated private state per worker** over shared immutable inputs (TypeScript 7: see Part 2). This is simple and deterministic, but costs duplicated work and memory.
  3. **Signature-first decoupling** (Flow types-first, `isolatedDeclarations`). This removes cross-file inference dependencies, so bodies check independently.
- **Work stealing** (Rayon in Rust, the Go scheduler) handles skewed per-file costs. A deterministic assignment, as in TS 7, trades some load balance for reproducibility.
- **Lock-free or sharded interning:** shard the hash table by hash bits, use per-shard locks or CAS insertion, and allocate interned values from per-thread arenas.

### 1.7 Hashing

Compiler hash keys are small integers, short strings, and tuples of IDs. Use fast non-cryptographic hashes (FxHash in rustc: a multiply-rotate per word, excellent for integer keys; wyhash, xxHash, or aHash for byte strings) and open-addressing tables (Swiss tables / hashbrown with SIMD control-byte probing; LLVM's `DenseMap`). For keywords, use a perfect hash or a length+first-char switch. Avoid generic maps keyed by *strings* built at runtime in hot paths. TypeScript's relation cache keys are strings like `"${source.id},${target.id}"`.\[19\] That string building and hashing is a known per-check tax that a native implementation can replace with packed integer keys.

### 1.8 Lexing and parsing speed

- Hand-written, single-pass lexers fused with the parser (Oxc: "The lexer and parser run in a single pass") with zero-copy slices of the source.\[30\]
- Pratt parsing for expressions avoids deep precedence-climbing recursion. Avoid backtracking. TypeScript's parser uses speculative `tryParse`/`lookAhead` for arrow functions and generics, which is a known cost center.
- **Lazy parsing of function bodies** (V8's preparser) skips body ASTs until needed. That is unsafe for a type checker that must see every body, but viable for an IDE's outline view.
- **SIMD** (simdjson-style classification, simdutf for UTF-8 validation and transcoding) mainly speeds up whitespace, comment, and string scanning and UTF-8 validation.
- **Benchmarks.** Oxc reports its parser is "at least 3x faster than swc and 5x faster than Biome". On one file (cal.com.tsx) it measured 4.0 ms vs 14.0 ms vs 18.7 ms, with peak memory 11.5 MB vs 16.6 MB against swc.\[6\]

### 1.9 Profiling methodology and tooling

- **CPU:** perf, samply, pprof (Go), Instruments, and VTune, viewed as flame graphs.
- **Allocation:** heaptrack, DHAT, Go's `-memprofile` / alloc_space, and V8 heap snapshots.
- **Tracing** in Chrome trace format, viewable in Perfetto. TypeScript's `--generateTrace` emits this.\[31\]
- **Benchmarking discipline:** count instructions, not just wall time, because instruction counts are much less noisy. Interleave runs. Pin the CPU frequency. Track results per commit.
- **Compiler perf dashboards.** rustc-perf (perf.rust-lang.org) gates every PR and was used to quantify the parallel front end's single-thread regression.\[32\] LLVM has its compile-time tracker. Oxc uses CodSpeed (instruction-count measurements in CI).\[6\] Ezno's author tracks "memory and CPU instructions" and parses rustc type-size output to find oversized enums.\[33\] The TypeScript team reruns its regression testing for TS 7 against real GitHub projects.\[1\]

### 1.10 Micro-level optimizations

- Replace virtual dispatch with a `switch` on a kind tag. Data-oriented ASTs make this natural. Jake Bailey's 2026 memory PR (below) "generates compact kind-based AST dispatch."\[34\]
- Keep call sites monomorphic in V8. Megamorphic property access is a major cost in JS compilers, which is why tsc used one Node constructor for all kinds.
- Use integer IDs instead of objects in maps and sets. Use bitsets for flag sets and visited sets.
- No exceptions or panics for control flow.
- Use builders or ropes for string assembly. No regexes in hot paths. TypeScript once ran "a regex on relationship cache keys" in a hot path and later removed it.\[35\]

---

## Part 2 — TypeScript, with a deep focus on the type checker

### 2.1 Architecture of tsc (Strada) and why it is shaped that way

The pipeline is scanner → parser → binder → checker → emitter. The binder creates `Symbol`s, builds symbol tables per container, and wires up the **flow graph** (FlowNodes) for control-flow analysis. The checker (`checker.ts`, historically 50k+ lines) is one enormous closure: `createTypeChecker(host)` returns an object of functions that share hundreds of closure variables (caches, counters, stacks).

**Why it is shaped this way:**

- Closure variables are fast in V8, faster than property lookups on a `this` object.
- Everything is lazily resolved through mutually recursive functions.
- One checker instance per program means every cache is global to that program.

**What it costs:**

- The checker is inherently single-instance and mutable. Every query can mutate shared caches, and type IDs are assigned in creation order. This is why the Go port could not "just add threads" and instead replicates whole checkers.
- Ryan Cavanaugh explained the Go choice this way: "Idiomatic Go strongly resembles the existing coding patterns of the TypeScript codebase, which makes this porting effort much more tractable. Go also offers excellent control of memory layout and allocation (both on an object and field level) without requiring that the entire codebase continually concern itself with memory management."\[36\]
- The cyclic, shared-mutable, GC-reliant graph is what defeated stc's Rust port. kdy1 wrote: "tsc uses a lot of shared mutability and many parts depend on garbage collection… Using Rust required me to use unsafe too much."\[37\]

### 2.2 TypeScript 7 ("Corsa", typescript-go): what changed and what it achieved

**Timeline:**

- **March 11, 2025:** Anders Hejlsberg's "A 10x Faster TypeScript".\[38\]
- **2025:** nightly native previews via `@typescript/native-preview` (`tsgo`).\[39\]
- **December 2, 2025:** "Progress on TypeScript 7". Type-checking was "very nearly complete": of about 20,000 compiler tests, roughly 6,000 produce errors in 6.0, and in "all but 74 cases" TS 7 also errors. `--incremental`, project references, and `--build` were ported.\[40\]\[41\]
- **2026:** TypeScript 6.0, the last JS-based release, serves as a transition. TS 7.0 Beta came on April 21 and the RC on June 18.\[42\]\[43\]
- **July 8, 2026:** TS 7.0 GA. npm is at 7.0.2.\[44\]
- The native-preview package passed "8.5 million weekly downloads". Nightlies are moving to `typescript@next`.\[1\] Visual Studio 2026 Insiders enabled TS 7 Beta by default,\[45\] and VS Code has a dedicated extension, with built-in support coming "in the coming weeks".\[1\]
- **Caveat:** 7.0 ships **no programmatic API**. A new API is expected in 7.1. Tools like typescript-eslint run side-by-side via `@typescript/typescript6` (`tsc6`).\[1\]

**Published benchmarks.** March 2025 preview, early port, `tsc` time:

| Codebase | LOC | TS (JS) | Native | Speedup |
|---|---|---|---|---|
| VS Code | 1,505,000 | 77.8s | 7.5s | 10.4x |
| Playwright | 356,000 | 11.1s | 1.1s | 10.1x |
| TypeORM | 270,000 | 17.5s | 1.3s | 13.5x |
| date-fns | 104,000 | 6.5s | 0.7s | 9.5x |
| tRPC | 18,000 | 5.5s | 0.6s | 9.1x |
| rxjs (observable) | 2,100 | 1.1s | 0.1s | 11.0x |

At the time, editor project load for VS Code dropped from about 9.6s to about 1.2s, and memory was "roughly half", with the note that "we haven't actively investigated optimizing this yet."\[38\]

GA, July 2026, TS 6 vs TS 7 at the default `--checkers 4`:

| Codebase | TS 6 | TS 7 | Speedup | Memory TS6 → TS7 |
|---|---|---|---|---|
| vscode | 125.7s | 10.6s | 11.9x | 5.2GB → 4.2GB (−18%) |
| sentry | 139.8s | 15.7s | 8.9x | 4.9GB → 4.6GB (−6%) |
| bluesky | 24.3s | 2.8s | 8.7x | 1.8GB → 1.3GB (−26%) |
| playwright | 12.8s | 1.47s | 8.7x | 1.0GB → 0.9GB (−11%) |
| tldraw | 11.2s | 1.46s | 7.7x | 0.6GB → 0.5GB (−15%) |

With `--checkers 8`, vscode drops to 7.51s (16.7x), sentry to 12.08s (11.6x), and bluesky to 2.01s (12.1x). Opening a file with an error in the VS Code repo went from about 17.5s to under 1.3s. Reported field results: Slack's CI type-check went from about 7.5 to 1.25 minutes, with 40% less merge-queue time. Canva's first-error-in-editor time went from about 58s to 4.8s. Vanta saw up to 9x. Microsoft's News team saved 400 CI hours a month.\[1\]

**Interpretation.** The memory story got *less* dramatic between preview and GA: "roughly half" in 2025 became −6% to −26% in 2026. The likely reason is that the GA numbers include 4 parallel checkers, each holding private type state. The zackoverflow measurement makes this concrete: on one project, checker memory was about 399 MB with 4 checkers versus about 52 MB single-threaded, and tuple types were duplicated 4x (102 per checker, 408 total).\[7\] In practice, **parallelism is being paid for in memory**. On memory-constrained CI, `--checkers 1` or 2 is a legitimate trade.\[1\]

**Where the 10x comes from.** Microsoft's official framing is "native code speed, shared memory multithreading, and a number of new optimizations."\[1\] Secondary summaries of Hejlsberg's talks split the speedup between native execution and concurrency; Arthur Artalar's dev.to review of a Hejlsberg interview, for example, says "Go's multi-threading capabilities brought an immediate 3x performance boost." That split is not in the official blog posts, so treat it as approximate. You can measure it yourself: `tsc --singleThreaded` on TS 7 versus TS 6 isolates the native-code factor, and `--checkers N` isolates scaling. One independent benchmark on a small codebase found `--checkers 4` (291ms) slightly faster than `--checkers 8` (297ms). Small projects saturate early.\[46\]

**How type checking is parallelized:**

- **Mechanism.** The official description: "TypeScript 7.0 creates a fixed number of type-checker workers with their own view of the world. These type-checking workers may end up duplicating some common work, but given the same input files, they will always divide them identically and produce the same results."\[1\]
- **Configuration.** The default is 4 (`--checkers`). `--builders` parallelizes project-reference builds, and the two multiply ("`--checkers 4 --builders 4` allows up to 16 type-checkers"). `--singleThreaded` disables all parallelism.\[1\]
- **Code.** In `checkerpool.go`, `checkerCount` defaults to 4 and is clamped to `max(min(checkerCount, len(files), 256), 1)`. Each checker gets the full file list (`c.files = program.SourceFiles()`), so it can lazily resolve any declaration it needs, but it is *tasked* with diagnosing only its share of files.\[7\]
- **Isolation rule.** Types from different checkers cannot be mixed. The Go API doc for `GetTypeCheckerForFile` warns that "it isn't possible to mix types obtained from different checkers."\[47\]
- **Shared vs. private.** The shared, immutable layer is the parsed and bound AST plus symbols, readable concurrently without locks because nobody writes. Everything the checker memoizes is private.\[2\]
- **Determinism.** Type IDs, and therefore union ordering and some inference tie-breaks, depend on creation order. TS 7 therefore makes `stableTypeOrdering` mandatory ("on by default, and cannot be turned off") and warns that "in rare cases, varying the number of `--checkers` may surface order-dependent results". Fix the checker count across environments.\[1\]
- **Language server.** The LSP server reuses a pool of about 4 checkers. A typescript-go issue (#2115) proposes splitting "foreground" checkers for interactive requests from diagnostic checkers.\[48\]

**Memory-layout work is only beginning.** Jake Bailey's draft PR "Various memory optimizations" (branch `data-oriented-compiler`, October 2026, *not merged* as of this writing) reports "22.15–29.66% lower memory usage across all nine scenarios." It does this by removing AST payload interfaces, moving optional symbol fields out of hot records, and generating kind-based AST dispatch.\[34\] This is exactly the Part 1 playbook (hot/cold splitting, tag dispatch, fewer interface headers) applied to tsgo. The team also says they "have gotten excellent performance and memory usage without using any unsafe primitives."\[49\] Further gains from compact type representations remain unexploited.

### 2.3 How the checker achieves performance internally

**Types and IDs.** Every `Type` gets a monotonically increasing numeric `id`, and every `Symbol` and `Node` gets a lazily assigned ID (`getNodeId`, `getSymbolId`). All caches key on these integers. `TypeFlags`, `ObjectFlags`, `SymbolFlags`, and `NodeFlags` are bitmasks. Checks like "is this any primitive" or "is this a literal or unit type" are single ANDs, and many relation fast paths (`isSimpleTypeRelatedTo`) are pure flag logic before any structural work.\[50\]

**Lazy resolution with circularity detection.** Declared types, signature return types, base types, and members (`resolveStructuredTypeMembers`) are computed on first request and stored on the symbol or type "links" objects. A resolution stack (`pushTypeResolution`/`popTypeResolution`) detects cycles\[51\] and reports "implicitly has type any because it… is referenced directly or indirectly in its own initializer".

**Deferred type references.** Type references to generic interfaces and classes (`Array<T>`, `Foo<X>`) are `TypeReference` objects pairing a target with type arguments. Their members are only instantiated when requested. Since 3.7, type arguments of references written in type nodes can be *deferred*, resolved lazily from the node, which made recursive type aliases like `type Json = … | Json[]` possible without eager expansion.\[52\] This is why named generic interfaces are cheap and structurally expanded anonymous types are expensive.

**Relation caching (assignability, subtype, comparability, identity).** There are separate relation maps: `identityRelation`, `subtypeRelation`, `strictSubtypeRelation`, `assignableRelation`, and `comparableRelation`. Each is a `Map<string, RelationComparisonResult>`, stored on the checker.

- **Key format.** The key comes from `getRelationKey`. For ordinary types it is `` `${source.id},${target.id}${postFix}` ``, where the postfix encodes intersection state. For identity, source and target are swapped so that `source.id < target.id`, which makes the relation symmetric.\[19\]
- **Generic references** with type-parameter arguments get a canonical form prefixed with `*` (`getGenericTypeReferenceRelationKey`, e.g. `"111=0-12=1"`).\[19\] One comparison of `Foo<T>` with `Bar<T>` for an unconstrained `T` then serves every instantiation. The team described this as: "you've done an abstract comparison that applies to every single relation of Foo and Bar with the same type argument."\[35\]
- **Result values.** Entries record Succeeded, Failed, or a "reported" variant, so that a cached failure can be re-elaborated for errors.
- **Implication.** Relations between *named* types (interfaces, generic references) are cache-friendly. Relations involving freshly constructed anonymous types (intersections, mapped-type outputs) get new IDs and therefore miss the cache. This is the mechanism behind the wiki's advice: "Type relationships between interfaces are also cached, as opposed to intersection types as a whole."\[21\]

**Coinductive recursion handling: `maybeKeys` and the source/target stacks.** `recursiveTypeRelatedTo` is the "assumed pairs" algorithm from the recursive-subtyping literature (Amadio–Cardelli; Brandt–Henglein's coinductive axiomatization):

1. Compute the key and check the global relation cache.
2. Check `maybeKeys`, the set of pairs currently assumed related on this comparison path. If the pair (or its "broadest equivalent" `*` key) is present, return `Maybe`. This means "assume true co-inductively".
3. Otherwise push the key onto `maybeKeys`, push source and target onto `sourceStack`/`targetStack`, and recurse into structural comparison.
4. On success, results computed *under assumptions* are not committed to the global cache until the outermost assumption resolves. On failure, everything pushed since `maybeStart` is discarded.

The 3.7.2 source shows the shape directly: `maybeKeys[maybeCount] = id; maybeCount++; sourceStack[depth] = source; targetStack[depth] = target; depth++;`.\[53\]

**Expanding types and recursion identity.** Purely coinductive assumption sets don't terminate on *expanding* generics such as `type Foo<T> = { x: Foo<Box<T>> }`, where every level produces new pairs. TypeScript cuts these off heuristically:

- **`isDeeplyNestedType`.** If the stack already contains `maxDepth` types with the same *recursion identity*, the checker sets `expandingFlags` and returns `Maybe`. A type's recursion identity is roughly its originating declaration, symbol, or node, shared across all its instantiations.\[19\]
- **Depth history.** `maxDepth` was hard-coded to 5 through TS 4.4. PR #46599 (Anders, TS 4.5) changed it to "Decrease recursion depth limit to 3 + smarter check for recursion".\[54\]\[55\] The smarter check only counts recursion-identity matches where type IDs *increase*, since deeper generated types have higher IDs, and it reduced false positives from hand-written nested types like `Foo<Foo<Foo<…>>>`.\[35\]\[54\]
- **Callers.** Some pass other depths, for example `isDeeplyNestedType(source, sourceStack, sourceStack.length, 2)` for expanding flags and `10` elsewhere.\[19\]\[56\]
- **Trade-off.** The source comment admits "we will generate a false positive when two types are structurally equal to at least maxDepth levels, but unequal at some level beyond that."\[56\] This is a deliberate trade of completeness for termination.

**Hard fuses:**

- **Instantiation:** `if (instantiationDepth === 100 || instantiationCount >= 5000000)` gives TS2589 "Type instantiation is excessively deep and possibly infinite". The depth limit was 50 before TS 4.5. The count is per statement or expression.\[57\]\[58\]
- **Tail-recursive conditional types** are allowed 1000 iterations (`if (tailCount === 1000)`).\[58\]
- **Relation depth:** a 100-frame depth fuse in `recursiveTypeRelatedTo` (counting source/target stack frames) historically produced TS2321 "Excessive stack depth comparing types". A separate relation-count budget produces "excessive complexity" failures.\[59\]
- **Divergence in tsgo.** typescript-go PR #4913 (merged August 18, 2026, "Improve recursion identities and isDeeplyNestedType") changed the depth-100 bail from an overflow error to `TernaryMaybe`.\[60\] Per a TypeScript issue, tsgo can now silently accept types that mismatch only beyond 100 levels, where TS 5.9 reported TS2321.\[59\] This is an edge case, but a real semantic divergence.
- **Policy.** The team will not raise the limits. Jake Bailey (September 2026): "We have no plans to change these limits. It would be a shame to make the compiler fast only to increase depths and go slow again."\[61\]

**Variance measurement: the single most important shortcut for generics.** To relate `Foo<A>` to `Foo<B>` without structurally expanding `Foo`, the checker measures each type parameter's variance once per generic declaration. Anders described it in PR #48080: "For a generic type T<A, B>, we determine the variance of A by structurally relating instantiations T<Sub, B> and T<Super, B>, where Sub and Super are internal marker types that have a known type relationship."\[62\] The implementation:

- Uses `markerSuperType`, `markerSubType` (constrained to super), and `markerOtherType` to detect *independent* parameters.\[56\]
- Records the result as `VarianceFlags` (Covariant, Contravariant, Bivariant, Invariant, Independent, plus Unmeasurable and Unreliable markers when the measurement passed through mapped or conditional types).\[56\]
- Thereafter, `Foo<A>` vs `Foo<B>` reduces to comparing the arguments. The checker "ignore[s] arguments for independent type parameters". If the variance is Unmeasurable, it falls back to structural comparison unless source and target are identical.\[56\]
- Only non-nested measurements are recorded permanently (PR #48080).\[62\]

The wiki's "Using Faster Variance Checks" (`strictFunctionTypes`) belongs here too. Method-style bivariance forces more structural checks.

**Instantiation caching.** Generic signatures cache instantiations by `getTypeListId(typeArguments)`.\[19\] Anonymous object types and type aliases cache instantiations keyed by the type arguments of their outer type parameters actually in scope (`getObjectTypeInstantiation`), which lets distinct mappers that agree on relevant parameters share results. Conditional types cache by the instantiated check and extends types. Mappers are lightweight (simple, array, deferred, merged, composite), which avoids eagerly materializing substitution maps.

**Control-flow analysis (narrowing).** The binder links each reference site to a FlowNode graph: assignments, conditions, switch clauses, labels for joins and loops, and calls for assertion functions. `getFlowTypeOfReference` walks *backwards* from the reference through antecedents.\[63\] That is demand-driven: you only analyze references you actually check.

- **Caches.** `flowTypeCache` keys on flow node ID. Shared flow nodes (multiple references through the same branch label) are memoized in `sharedFlowNodes`/`sharedFlowTypes`. Loop labels cache antecedent types in `flowLoopNodes`/`flowLoopKeys`/`flowLoopTypes` (in tsgo, `flowLoopCache map[FlowLoopKey]*Type`). Loops iterate to a fixpoint with the type cache temporarily cleared.\[7\]\[56\]
- **Limits.** In 2017 Anders added a recursion limit (PR #18246): "If control flow analysis leads to a stack depth of 2500 recursive calls, we now issue an error and disable control flow analysis for the remainder of the containing function or module body… The 2500 recursive call limit was experimentally determined." It sets `flowAnalysisDisabled`\[64\] and reports "Code is too complex to analyze". Current `checker.ts` lowers this to 2000: `getTypeAtFlowNode` reads `if (flowDepth === 2000) { // We have made 2000 recursive invocations.`, emits the `getTypeAtFlowNode_DepthLimit` trace event, and sets `flowAnalysisDisabled = true`.
- **Performance implication.** Very long functions with many sequential narrowings, and huge `switch` statements over discriminants, create long flow chains. Each new reference walks back through them, though shared-node caching bounds repeated work.

**Discriminated unions and literal fast paths.** Narrowing by discriminant filters union constituents by the literal type of the discriminant property. For large unions, the checker builds a **key-property map** from discriminant literal to constituent, so finding the matching member is a lookup rather than a linear scan (`getKeyPropertyName` / `getMatchingUnionConstituentForType`).\[65\] When relating to a union target, a cheap pre-pass handles literal and primitive sources by identity or flag membership before structural comparison of object members. That is why literal-only unions of thousands of members are tolerable while unions of large object types are not.

### 2.4 Known performance pitfalls and why they're slow

Each item below maps a rule from the official wiki "Performance" page to the mechanism above:

1. **Prefer interfaces over intersections.** An interface is a single flattened, cached object type with a stable identity. Its relations hit the relation cache, and conflicts are detected once. An intersection is re-created per use site and is not cached "as a whole".\[21\] When the target is an intersection, "every constituent is checked before checking against the 'effective'/'flattened' type." So `type Foo = Bar & Baz & {…}` multiplies comparisons. `interface Foo extends Bar, Baz {…}` does not.\[21\]
2. **Annotate return types, especially exported ones.** Inference re-derives (often anonymous, uncached) types, and declaration emit must serialize them.\[31\] Named types are "more compact than anonymous types" and reduce .d.ts read/write.\[21\]
3. **Prefer base types over large unions.** Each argument is compared to every member, and reduction is pairwise quadratic.\[66\] Intersecting large unions creates cross-products.\[21\]
4. **Name complex conditional and mapped types.** Named aliases give instantiation-cache hits. Inline repeated expansions get fresh IDs.\[66\]\[67\]
5. **Deep recursive conditional types and recursive template literal types** hit the instantiation depth and count fuses.\[68\] Template-literal inference over long strings is O(length) instantiations per level.
6. **Excessive generic instantiation.** The "Instantiations" counter in `--extendedDiagnostics` predicts checker time far better than file count.\[28\]
7. **Configuration:**
   - `skipLibCheck` (don't re-check .d.ts bodies).\[28\]
   - `types: []` to stop auto-including every `@types` package.\[21\] This is now the default in TS 6/7.\[1\]
   - Tight `include`/`exclude`.
   - `incremental`/`composite`.
   - Project references, where the wiki suggests "5-20 projects is an appropriate range". More projects re-check shared dependencies once per project.\[21\]
   - `isolatedModules` for transpile-only pipelines.
   - `isolatedDeclarations` to make .d.ts emit per-file and parallel. TS 7 notes that `--builders` parallelism is bounded by the project graph except "on codebases that leverage `--isolatedDeclarations` and separate syntactic declaration file emit".\[1\]\[28\]

### 2.5 Profiling tsc and tsgo

- `--extendedDiagnostics`: phase times (I/O, parse, bind, check, emit), plus counts of types, instantiations, symbols, and memory. Start here.\[31\]
- `--generateTrace <dir>` (TS 4.1+): `trace.json` (Chrome/Perfetto format, with `checkSourceFile`, `checkExpression`, `structuredTypeRelatedTo`, and `instantiateType_DepthLimit` events) plus `types.json` for mapping IDs back to declarations. `npx @typescript/analyze-trace <dir>` lists hot spots.\[21\]\[69\]
- `--generateCpuProfile`, `--listFilesOnly`, `--explainFiles`, `--traceResolution`, `--showConfig`.\[31\]
- Editor problems: a tsserver log (for TS 7, LSP logs from the native extension).\[70\]
- **Comparing TS 6 and TS 7:** run `tsc6` against `tsc`, and `tsc --singleThreaded` against `--checkers N`. For Go-side profiling, the typescript-go repo supports standard Go pprof tooling.\[7\]
- The V8 heap limit (`--max-old-space-size`) is only relevant to TS ≤6 / tsserver 6. Go's GC memory can be tuned with `GOMEMLIMIT`/`GOGC`.

### 2.6 Alternative and faster TS type checkers: lessons learned

- **stc (Rust, kdy1/Donny Kim).** An early from-scratch Rust prototype type-checked TypeScript's conformance suite in 2.13s versus 133.2s for tsc ("62x faster"). The prototype was incomplete, so the comparison is not apples-to-apples. kdy1 then tried a Go port ("tsc uses a lot of shared mutability… Rust required me to use unsafe too much"). He returned to Rust and open-sourced stc in October 2022, noting the Go port "was diverging from the official TypeScript compiler source code". stc was declared abandoned in January 2024; kdy1: "TypeScript was not something that I could follow up on in an alternative language."\[37\]\[71\]\[72\] **Lesson:** TS semantics are defined by tsc's implementation, quirks included. Chasing parity across releases is the real cost, and a from-scratch checker's early speed numbers are inflated by unimplemented features.
- **Ezno (Rust, Ben / kaleidawave).** This is a TS-*like* checker, not a parity effort. It is "not exclusively focused on checking performance"\[73\] and experiments with interpreter-like, effect-tracking type synthesis.\[74\] Its data design is instructive: types are `TypeId(u16)` indices into a `Vec<Type>`, a pure index-based type arena.\[75\] Its 2025 update says "Performance mainly comes from structure of the data" and points out that the event/effect system made it complex and not yet usable on real projects.\[33\] It was detached from Oxc due to differing timelines.\[73\]
- **tsgo (official).** A faithful port, chosen *because* of the parity lesson. Microsoft calls it "a port, not a rewrite"; type-checking semantics carry over on purpose.\[20\]
- **Oxc.** A Rust parser, linter, transformer, and resolver. Its home page advertises "True type-aware Linting powered by tsgo", meaning it delegates types to the official checker via tsgolint rather than reimplementing them.\[76\] An oxc/tsgolint issue notes a subtlety: TypeScript assigns each file to a checker, while their workload queue initially gave every checker the same queue.\[77\]
- **Biome** has explored its own lightweight type inference for lint rules. Josh Goldberg's survey concluded that Rust linters were fast "but no typed linting right now" and that reimplementations' "long-term feasibility as standalone projects is precarious."\[78\] The converged answer in 2026 is to call tsgo.
- **esbuild, swc, Bun, Deno.** Transpile-only type stripping (with `isolatedModules`), with type-checking left to tsc. Deno has historically bundled tsc for `deno check`. These tools are fast precisely because they *don't* check types.

### 2.7 Research literature relevant to a fast TS checker

- **Recursive subtyping.** Amadio & Cardelli, "Subtyping Recursive Types" (TOPLAS 1993), and Brandt & Henglein's coinductive axiomatization (1997/98) give the assumption-set algorithm that `maybeKeys` implements. Naive versions are exponential. Memoizing *failed* pairs and committing successes only after assumptions discharge (as TS does) is the standard fix. Later work on iso-recursive subtyping (Zhou et al., OOPSLA 2020, "Revisiting iso-recursive subtyping") offers more principled alternatives.\[52\]
- **Hash-consing.** Filliâtre & Conchon, "Type-Safe Modular Hash-Consing" (2006), is the canonical reference. Lean 4 and Coq kernels hash-cons terms for O(1) equality and sharing.
- **Local and bidirectional inference.** Pierce & Turner, "Local Type Inference" (TOPLAS 2000), is TypeScript's lineage. Bidirectional checking keeps inference local, linear-ish, and parallelizable per function, unlike global HM constraint solving.
- **Incrementality.** Hazelnut (Omar et al., POPL 2017) covers structure-editor typing. Self-adjusting computation (Acar) and incremental lambda calculus (Cai et al., PLDI 2014) underpin red-green / Salsa. Merlin and Dune show OCaml's per-file .cmi interfaces enabling fast IDE checks, the same idea as Flow's types-first and TS `isolatedDeclarations`.
- **Industrial checkers at scale.** Flow (OCaml, shared-memory heap via the `sharedmem` library, types-first) and Hack (shared-memory hash tables across forked workers, "saved state" for warm start) are the best comparisons for a TS checker at Meta scale. They chose shared, lock-free-ish heaps across processes because OCaml lacked shared-memory threads. TypeScript 7 instead gets shared memory from Go threads.

---

## Recommendations: a prioritized playbook for building or optimizing a TS type checker

**For teams using TypeScript today (highest ROI first):**

1. **Upgrade to TS 7** (`tsc`), keeping `@typescript/typescript6` aliased for API-dependent tools until 7.1. Expect 8–12x faster builds.\[1\]
2. **Tune `--checkers`.** Try 8 on big dev machines (16.7x on VS Code); use 1–2 on memory-tight CI. Pin the value across environments for determinism.\[1\]
3. Run `--extendedDiagnostics`. If Instantiations is high relative to file count, `--generateTrace` + analyze-trace will find the hot declarations.
4. Fix the hot spots using the wiki rules:
   - interfaces instead of intersections;
   - annotated exported return types;
   - base types instead of mega-unions;
   - named conditional and mapped types;
   - shallower recursion in type-level programs.
5. Configure: `skipLibCheck`, explicit `types`, tight `include`, `incremental`, project references (5–20 balanced projects), and `isolatedDeclarations` if you publish packages or are bottlenecked on .d.ts emit.

**For engineers building or optimizing a TS-compatible checker (ordered by impact):**

1. **Do not rewrite semantics. Port them.** The stc → tsgo history is decisive: parity is the hard part, and tsc's behavior *is* the spec.
2. **Make identity cheap everywhere.** Use dense integer IDs for nodes, symbols, and types. Intern unions, intersections, literals, and instantiations by sorted ID lists. Replace string relation keys (`"12,34*"`) with packed `u64`/`u128` keys in an open-addressing table such as FxHash + Swiss table.
3. **Protect the relation engine.** Use global relation caches per relation kind, coinductive `maybeKeys` with deferred commit, generic-reference canonical keys, recursion-identity depth cutoffs, and hard fuses (instantiation depth 100 / count 5M, relation depth and complexity budgets). Measure variance once per generic declaration and use it to avoid structural expansion. This is the single biggest algorithmic lever for real code.
4. **Stay lazy and deferred.** Resolve members, base types, and signatures on demand. Defer type-reference arguments. Analyze control flow backwards on demand with shared-node and loop caches.
5. **Use a compact data layout.** Flat, arena-allocated AST nodes with `u32` child indices, hot/cold field splitting, kind-tag dispatch instead of interfaces or virtuals, and bitflag sets. Jake Bailey's draft tsgo work suggests 22–30% memory reduction is available from layout alone, and Zig's experience suggests real CPU wins too.
6. **Design parallelism up front.**
   - Cheapest: replicated checkers over an immutable shared AST (TS 7's model). It costs memory roughly linear in the checker count.
   - Better long-term: a shared, sharded, thread-safe interner for *types*, with per-thread relation caches merged or promoted. This removes duplication but requires deterministic type ordering independent of thread schedule (TS 7's `stableTypeOrdering` shows ordering is the crux).
   - Best for scale: signature-first checking (Flow types-first / `isolatedDeclarations`), which turns cross-file inference into a DAG of cheap signature reads.
7. **Add query-level incrementality for IDEs.** Use red-green memoization with early cutoff on declaration signatures, and durability levels for `node_modules`/lib, so edits inside a function body never invalidate other files' types.
8. **Instrument from day one.** Use instruction-count CI benchmarks (CodSpeed or rustc-perf-style), trace events compatible with `--generateTrace`, and a corpus of real repos (VS Code, Sentry, Playwright, tldraw, Bluesky) to compare against tsc.

## Caveats

- Benchmarks are vendor-published (Microsoft, Oxc) on specific machines. Your speedups will vary with core count, project shape, and memory.
- The native-vs-concurrency split (e.g., the "immediate 3x" from multi-threading cited in Arthur Artalar's dev.to review) is from secondary reports of Hejlsberg's talks, not from the official posts.
- Some checker internals are quoted from source snippets and third-party analyses: the flow-depth constant (2500 originally; 2000 in current `checker.ts`), the exact tsgo file-to-checker assignment policy, and the 256-identifier chunking. Verify them against the current `checker.ts` / `internal/checker` before relying on them.
- Jake Bailey's memory PR is a draft and unmerged. tsgo's 2026 change to the depth-100 relation fuse is a real behavioral divergence from TS 6.

## Sources

1. [Announcing TypeScript 7.0 - TypeScript](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)
2. [TypeScript’s Monumental Shift: A 10x Faster Type Checker with Go](https://medium.com/codex/typescripts-monumental-shift-a-10x-faster-type-checker-with-go-670d7bcab42d)
3. [Keyboard shortcuts](https://rustc-dev-guide.rust-lang.org/memory.html)
4. [ty 模块：类型的表示 - Rustc 开发指南](https://rust-lang-cn.github.io/rustc-guide/ty.html)
5. [GlobalCtxt in rustc\_middle::ty - Rust](https://public-docs.ferrocene.dev/main//rustc-docs/rustc_middle/ty/struct.GlobalCtxt.html)
6. [bench javascript parser written in rust](https://github.com/oxc-project/bench-javascript-parser-written-in-rust)
7. [Why does tsgo use so much memory?](https://zackoverflow.dev/writing/why-does-tsgo-use-so-much-memory/)
8. [TypeScript Compiler Goes Native: 10x Faster with Go](https://medium.com/@vedhasagaran7/typescript-compiler-goes-native-10x-faster-with-go-7bc14f0b737a)
9. [Optimizing TypeScript Memory Usage - Swatinem](https://swatinem.de/blog/optimizing-tsc/)
10. [esbuild - FAQ](https://esbuild.github.io/faq/)
11. [FAQ - Esbuild - W3cubDocs](https://docs.w3cub.com/esbuild/faq/)
12. [Implementing a Struct of Arrays](https://brevzin.github.io/c++/2025/05/02/soa/)
13. [Implementing a Struct of Arrays -- Barry Revzin : Standard C++](https://isocpp.org/blog/2025/06/implementing-a-struct-of-arrays-barry-revzin)
14. [Zig Parser](https://mitchellh.com/zig/parser)
15. [17\. Abstract Syntax Tree: Data-Oriented Design](https://axion-resources.vercel.app/abstract-syntax-tree/data-oriented-ast)
16. [Modernizing compiler design for Carbon's toolchain \[video\]](https://news.ycombinator.com/item?id=37275526)
17. [rustc\_middle::ty - Rust](https://public-docs.ferrocene.dev/main//rustc-docs/rustc_middle/ty/index.html)
18. [rustc-dev-guide.rust-lang.org](https://rustc-dev-guide.rust-lang.org/appendix/glossary.html)
19. [checker.ts - GitHub](https://raw.githubusercontent.com/microsoft/TypeScript/master/src/compiler/checker.ts)
20. [TypeScript 7 is out - the native Go port, what changed, and ...](https://typescriptpro.com/blog/typescript-version-7-2026-07-08)
21. [Performance](https://github.com/microsoft/TypeScript/wiki/Performance/7caf72e2ffca0f9202238e81f84e8770c4fd98d1)
22. [When incremental computation beats recomputing everything](https://www.mrlatte.net/en/research/2026/08/25/incremental-computation-propagation/)
23. [The "red-green" algorithm - Salsa](https://salsa-rs.github.io/salsa/reference/algorithm.html)
24. [Durable Incrementality](https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html)
25. [Durable Incrementality](https://lobste.rs/s/te7x8n/durable_incrementality)
26. [Types-First: A Scalable New Architecture for Flow](https://medium.com/flow-type/types-first-a-scalable-new-architecture-for-flow-3d8c7ba1d4eb)
27. [Improvements to Flow in 2019. What the Flow team has been up to this…](https://medium.com/flow-type/improvements-to-flow-in-2019-c8378e7aa007)
28. [Why My TypeScript Typecheck Got Slow: Field Notes on tsgo, Project References, and isolatedDeclarations - DEV Community](https://dev.to/ahmed_mahmoud360/why-my-typescript-typecheck-got-slow-field-notes-on-tsgo-project-references-and-2lhp)
29. [Faster compilation with the parallel front-end in nightly](https://blog.rust-lang.org/2023/11/09/parallel-rustc/)
30. [Oxc Parser in 2026: The Rust-Powered AST Tool That Is Making Babel Replaceable for Real Codebases - DEV Community](https://dev.to/jsmanifest/oxc-parser-in-2026-the-rust-powered-ast-tool-that-is-making-babel-replaceable-for-real-codebases-2k7g)
31. [stevekinney.net/courses/enterprise-ui/scaling-typescript.md at main · stevekinney/stevekinney.net · GitHub](https://github.com/stevekinney/stevekinney.net/blob/main/courses/enterprise-ui/scaling-typescript.md)
32. [Enable Parallel Rustc Front End In Nightly Builds · Issue #681 · rust-lang/compiler-team · GitHub](https://github.com/rust-lang/compiler-team/issues/681)
33. [Ezno 2025](https://kaleidawave.blog/posts/ezno-25/)
34. [Various memory optimizations by jakebailey · Pull Request #64734 · microsoft/TypeScript · GitHub](https://github.com/microsoft/TypeScript/pull/64734)
35. [github.com](https://github.com/microsoft/TypeScript/issues/46697)
36. [Why Go? · microsoft/typescript-go · Discussion #411 · GitHub](https://github.com/microsoft/typescript-go/discussions/411)
37. [I'm porting tsc to Go](https://kdy1.dev/2022-1-26-porting-tsc-to-go)
38. [A 10x Faster TypeScript - TypeScript](https://devblogs.microsoft.com/typescript/typescript-native-port/)
39. [TypeScript 7.0 RC: Go Compiler 10x Faster \[2026\] - Tech Insider](https://tech-insider.org/ie/typescript-7-native-compiler-2026/)
40. [Progress on TypeScript 7 - December 2025 - TypeScript](https://devblogs.microsoft.com/typescript/progress-on-typescript-7-december-2025/)
41. [Progress on TypeScript 7](https://peerlist.io/saxenashikhil/articles/progress-on-typescript-7--december-2025)
42. [TypeScript - Category](https://devblogs.microsoft.com/typescript/category/typescript/)
43. [TypeScript](https://en.wikipedia.org/wiki/TypeScript)
44. [Runnable TypeScript Docs: TypeScript 7](https://coddy.tech/docs/typescript/typescript-7)
45. [TypeScript 7 Beta Now Enabled by Default in Visual Studio 2026 18.6 Insiders 3 - Visual Studio Blog](https://devblogs.microsoft.com/visualstudio/typescript-7-beta-now-enabled-by-default-in-visual-studio-2026-18-6-insiders-3/)
46. [TypeScript 7.0 Beta: Why the Go rewrite was inevitable, and what my benchmarks actually show](https://medium.com/@codewithbedant/typescript-7-0-beta-why-the-go-rewrite-was-inevitable-and-what-my-benchmarks-actually-show-823734438018)
47. [compiler package - github.com/microsoft/typescript-go/internal/compiler - Go Packages](https://pkg.go.dev/github.com/microsoft/typescript-go/internal/compiler)
48. [Refactor checker pool to limit checker creation for foreground tasks, segment diagnostic checkers · Issue #2115 · microsoft/typescript-go · GitHub](https://github.com/microsoft/typescript-go/issues/2115)
49. [A 10x Faster TypeScript](https://lobste.rs/s/3q0ei5/10x_faster_typescript)
50. [Faster exit from isTypeRelatedTo with identityRelation by ahejlsberg · Pull Request #36590 · microsoft/TypeScript · GitHub](https://github.com/microsoft/TypeScript/pull/36590/files)
51. [Limit deep generic type instantiations by ahejlsberg · Pull Request #15011 · microsoft/TypeScript · GitHub](https://github.com/microsoft/TypeScript/pull/15011/files/6e58f619ae310726d6aea1d2725787d0f9ce2d3c)
52. [Recursive data type](https://en.wikipedia.org/wiki/Recursive_data_type)
53. [https://raw.githubusercontent.com/microsoft/TypeSc...](https://raw.githubusercontent.com/microsoft/TypeScript/v3.7.2/src/compiler/checker.ts)
54. [Cherry-pick PR #46599 into release-4.5 (#46974) · microsoft/TypeScript@ae44862 · GitHub](https://github.com/microsoft/TypeScript/commit/ae448625b0252f20252c7a81015424eeb1c1f5bc)
55. [Undetected illegal assignment of nested array types · Issue #52912 · microsoft/TypeScript · GitHub](https://github.com/microsoft/TypeScript/issues/52912)
56. [Githubusercontent](https://raw.githubusercontent.com/Microsoft/TypeScript/master/src/compiler/checker.ts)
57. [Doom Ran Inside TypeScript's Type Checker. Here's How Types Execute Code - DEV Community](https://dev.to/gabrielanhaia/doom-ran-inside-typescripts-type-checker-heres-how-types-execute-code-50i1)
58. [Tuple Type in TypeScript - hrtyy.dev](https://hrtyy.dev/anything/typed-circuit/)
59. [TypeScript 7 (native) silently accepts mismatched types past the relater depth fuse](https://github.com/microsoft/TypeScript/issues/64390)
60. [Bump typescript-go submodule past microsoft/typescript-go#4913 (false TS2321 poisons later assignability checks) · Issue #77 · johnsoncodehk/typescript-native-bridge · GitHub](https://github.com/johnsoncodehk/typescript-native-bridge/issues/77)
61. [TypeScript 7.1 Iteration Plan · Issue #63703 · microsoft/TypeScript · GitHub](https://github.com/microsoft/TypeScript/issues/63703)
62. [Always fully compute variance structurally by ahejlsberg · Pull Request #48080 · microsoft/TypeScript · GitHub](https://github.com/microsoft/TypeScript/pull/48080)
63. [checker: remember complete flow-analysis results across walks (flow memo) by maschwenk · Pull Request #97 · maschwenk/tsrs · GitHub](https://github.com/maschwenk/tsrs/pull/97)
64. [Error on excessively large control flow graphs by ahejlsberg · Pull Request #18246 · microsoft/TypeScript · GitHub](https://github.com/microsoft/TypeScript/pull/18246/files/4f43ae207afe655f1dbda7fa0a732890fdf78313)
65. [github.com](https://github.com/microsoft/typescript/issues/62512)
66. [Scaling TypeScript](https://stevekinney.com/courses/enterprise-ui/scaling-typescript)
67. [How \`tsc\` Crashes With 14 Lines of Recursive Types (And What TS 6 Doesn't Fix) - DEV Community](https://dev.to/gabrielanhaia/how-tsc-crashes-with-14-lines-of-recursive-types-and-what-ts-6-doesnt-fix-52l0)
68. [How to Fix 'Type Instantiation Is Excessively Deep' Errors](https://oneuptime.com/blog/post/2026-01-24-fix-type-instantiation-excessively-deep-typescript/view)
69. [An approach to optimizing TypeScript type checking performance](https://www.geldata.com/blog/an-approach-to-optimizing-typescript-type-checking-performance)
70. [DEV Community](https://dev.to/t7yang/summary-of-typescript-performance-47jg)
71. [TypeScript type checker · Issue #571 · swc-project/swc · GitHub](https://github.com/swc-project/swc/issues/571)
72. [Open-sourcing the new TypeScript type checker](https://kdy1.dev/2022-10-27-open-sourcing-stc)
73. [The quest continues](https://kaleidawave.github.io/posts/the-quest-continues/)
74. [github.com](https://github.com/unvalley/ezno)
75. [A preview of Ezno's checker](https://kaleidawave.blog/posts/a-preview-of-the-checker/)
76. [Oxc](https://oxc.rs/)
77. [Type checking and linting can repeat work on different checkers · Issue #1175 · oxc-project/tsgolint · GitHub](https://github.com/oxc-project/tsgolint/issues/1175)
78. [Rust-Based JavaScript Linters: Fast, But No Typed Linting Right Now](https://www.joshuakgoldberg.com/blog/rust-based-javascript-linters-fast-but-no-typed-linting-right-now/)
