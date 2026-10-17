# Compiler performance research synthesis

2026-10-17

The strongest compiler optimizations eliminate repeated work, make semantic
identity cheap, and reduce the amount of memory touched by common operations.
Parallel execution increases throughput once those foundations are in place.
The Astra, Fable, and Argon reports agree on these principles; their main
differences concern how much architectural machinery to introduce around them.

For this Rust implementation, the most useful sequence is to reduce the cost of
freshness validation, reuse unaffected work after edits, reduce retained data,
and improve reuse during expensive semantic queries. A native-language rewrite
is already irrelevant. Fine-grained query tracking, shared semantic stores, and
persistent disk caches remain options whose value depends on the measured
workload and the cost of preserving TypeScript 7.0.2 behavior.

## Sources and scope

| Source | Strongest contribution |
| --- | --- |
| [Astra](compiler-performance-astra.md) | Work avoidance, memory ownership, incremental dependencies, service scheduling, and measurement methodology. |
| [Fable](compiler-performance-fable.md) | Concrete checker mechanisms: interning, relation and instantiation caches, variance shortcuts, flow analysis, and implementation details. |
| [Argon](compiler-performance-argon.pdf) | Data layout, a declaration-index alternative to fine-grained incrementality, and separation of strict subtyping from compatibility rules. |
| [Repository performance measurements](../../performance-command-center.md#current-state-2026-10-17) | The workload-specific evidence used to prioritize changes here. |

The research reports share several underlying sources. Agreement between them
is therefore corroboration of a design principle, not three independent
performance experiments. Their published speedup figures cover different
implementations, feature coverage, machines, and workloads; those figures are
neither additive nor predictions for this repository.

## Techniques shared by all three reports

| Technique | Mechanism | Main trade-off |
| --- | --- | --- |
| Demand-driven computation | Resolve declarations, members, signatures, and types only when an operation needs them. | First-use latency and the complexity of circular resolution. |
| Incremental reuse | Retain unaffected results after edits and stop propagating changes when the relevant result remains unchanged. | Dependency tracking, validation cost, and retained cache state. |
| Interning and canonical identities | Store repeated names and suitable type forms once; use compact IDs for identity and cache keys. | Canonicalization cost and preserving distinctions that affect semantics. |
| Relation and instantiation caching | Reuse completed type comparisons and generic expansions. | Correct keys, recursion state, cache lifetime, and memory growth. |
| Compact representation | Use contiguous storage, small references, bit flags, and separate storage for uncommon fields. | Migration cost and access overhead when data is split poorly. |
| Allocation by lifetime | Use arenas, scratch storage, and inline collections to reduce allocation and disposal work. | An arena can retain data long after its last useful access. |
| Fast paths and bounded complexity | Check identity and simple cases first; detect cycles and control pathological expansion. | Fast paths and limits must preserve the required behavior. |
| Delayed diagnostics | Perform speculative semantic checks without constructing full error messages. | Enough failure information must remain available for accurate diagnostics. |
| Parallelism with explicit ownership | Share immutable inputs and isolate mutable worker state. | Duplicate computation, memory consumption, synchronization, and determinism. |
| Interface boundaries | Let consumers depend on declarations or signatures instead of implementation details. | Inferred exports, augmentation, and other cross-file semantics complicate the boundary. |

### Avoiding work and preserving reuse

Laziness, memoization, and incrementality solve different problems. Laziness
avoids computing an answer nobody requests. Memoization avoids computing the
same answer repeatedly in a valid context. Incrementality keeps useful answers
when the surrounding program changes. A compiler can implement one well and
still lose most of its time through the other two.

A body edit to an explicitly typed exported function may leave its public type
unchanged. Downstream type computations can then remain valid even though local
analysis and source locations need updating. This requires tracking the facts
consumers actually use. File imports alone do not capture all dependencies:
declaration merging, global declarations, module augmentation, resolution inputs,
and contextual typing can affect seemingly unrelated results.

All three reports describe incremental query engines. Argon additionally
presents a compact declaration index with cheap recomputation as an alternative.
The engineering choice depends on whether dependency bookkeeping costs less
than the recomputation it avoids.

### Cheap identity and effective caches

Interning has a compound benefit: fewer allocations, smaller retained graphs,
cheap identity checks, and more opportunities to reuse cached comparisons.
Canonical union constituents, literal values, and generic argument lists are
common candidates. Interned names also avoid repeated string hashing during
symbol lookup.

Identity is a fast path, not a complete replacement for structural
assignability. Distinct type IDs may still describe compatible types. Conversely,
superficially equal shapes may differ in freshness, provenance, contextual
behavior, or ordering. Canonicalization must preserve every distinction the
pinned compiler observes, and name interning must retain exact names.

A relation cache needs more than an arbitrary pair of integers. Its scope or
key must account for the relation kind and relevant context. Results obtained
under unresolved recursive assumptions cannot be published as unconditional
successes. IDs retained across edits also need an explicit identity and
invalidation contract; an arena index alone does not supply one.

The useful next step for an existing checker is to measure cache hits, misses,
construction cost, and reuse across requests. Another cache is worthwhile only
when its avoided work exceeds its lookup, maintenance, and retention costs.

### Compact data and controlled lifetimes

Memory layout affects CPU performance because compiler work repeatedly follows
references through syntax, symbol, and type graphs. Compact IDs and contiguous
arrays put more useful data in each cache line. Separate payload arrays prevent
uncommon, complex variants from enlarging every common node. Small inline
collections avoid a heap allocation for short argument or member lists.

Structure-of-arrays layouts are especially useful when a pass reads only one
or two fields across many records. A compact array of structs may be better
when operations need most fields together. The access pattern decides; changing
every structure to parallel arrays is not a universal improvement.

Arenas simplify allocation and bulk disposal, but their boundaries matter.
Request scratch data, file syntax, program state, and process-wide interned
data have different lifetimes. Retained memory and temporary allocation traffic
need separate profiles because they call for different changes.

### Complexity and parallel execution

Recursive structural comparisons, union reduction, conditional types, and
generic instantiation can dominate a checker even when parsing is fast. Start
with exact cheap cases, reuse completed results, and handle recursive states
according to the compiler's rules. Union-find is useful for equality constraints;
it cannot replace TypeScript's directional structural relations.

Parallel tasks should be large enough to amortize scheduling and have clear
ownership. Parsing files and processing independent modules are stronger
candidates than spawning work for individual type comparisons. Immutable shared
inputs with worker-local inference and recursion state provide a practical
starting point. Sharing completed semantic facts can reduce duplication, but
only where those facts are independent of worker context.

## Techniques developed in only one or two reports

The table records substantive treatment of each technique. A general mention of
incrementality, profiling, or parallelism does not imply treatment of every
specialization below.

| Technique | Reports | Assessment |
| --- | --- | --- |
| Hot, warm, and cold project residency; eviction under a memory budget | Astra | Useful when inactive projects retain substantial semantic state. Adds promotion latency and lifecycle complexity. |
| Content-addressed semantic summaries on disk | Astra develops the proposal | Promising for repeated startup and reuse across worktrees. Requires keys covering compiler version, options, dependencies, libraries, and resolution state. |
| Persistent maps, prefix tries, disk indexes, and succinct trees | Astra | Useful for particular snapshot, search, or storage workloads. They are not interchangeable replacements for hot symbol maps. |
| Adaptive worker counts, task priorities, cancellation, and batched API boundaries | Astra develops these mechanisms | Useful for keeping an interactive service responsive under memory and CPU pressure. |
| Explicit worklists and fixed-point processing | Astra and Fable | Useful when only a small portion of a cyclic analysis changes. Astra further develops strongly connected component scheduling and solving. |
| Detailed profiling and continuous performance regression checks | Astra and Fable | Essential for selecting changes and detecting regressions. Argon includes profiling examples but a less developed measurement process. |
| Durability levels and position-independent semantic inputs | Fable | Avoids repeatedly validating stable dependencies and invalidating semantic facts solely because positions changed. Durability still needs a way to recognize actual dependency changes. |
| Scanner and parser tuning, including SIMD scanning and fewer passes | Fable | Worth pursuing when scanning or parsing accounts for a material portion of elapsed time. |
| Precomputed variance for generic comparisons | Fable and Argon | Can replace repeated structural expansion with argument comparisons where TypeScript permits it. Preserve its fallback rules. |
| Discriminant lookup maps and detailed flow-cache mechanisms | Fable | Targeted ways to avoid repeated union scans and control-flow walks. Their value depends on the dominant semantic workload. |
| A declaration index followed by lazy or parallel analysis | Argon develops it as an alternative architecture | Worth comparing with a fine-grained query engine before committing to dependency tracking everywhere. |
| Strict subtyping plus a separate compatibility layer, called the Judge and Lawyer | Argon | An interesting module boundary, but neither a demonstrated speedup here nor an established route to TypeScript parity. |

Fable also describes immutable red/green syntax trees with a lazily constructed
navigation facade. That syntax representation is distinct from the red-green
query-validation algorithm discussed across the reports. It earns its cost when
sharing syntax across edits materially reduces parsing or memory use.

## Judgment on usefulness

**Work avoidance has the greatest potential for interactive tools.** It can
remove an entire project rebuild or repeated validation pass from an operation.
Start at boundaries with clear dependencies, such as captured inputs, unchanged
files, and declared interfaces. Track finer semantic dependencies when the
coarser approach leaves substantial repeated work.

**Canonical identity and memoization are the strongest checker-specific
techniques.** They improve repeated semantic operations without requiring a
wholesale change to the type system. Improvements should extend existing owners
of type construction and relations, preserving TypeScript's context and
recursion rules.

**Compact representations are the strongest broadly applicable memory
technique.** They can improve allocation, retention, and locality at once. Their
priority relative to caching depends on whether the workload is dominated by
repeated computation, temporary allocation, or retained data.

**Algorithm changes should target observed repetition or poor scaling.**
Variance shortcuts, discriminant indexes, and worklists can be substantial wins
on matching workloads. A generic rewrite of the solver is a much larger bet.
The Judge/Lawyer proposal deserves evaluation as a design option only if it
simplifies the actual compatibility rules and passes differential tests.

**Parallelism follows the ownership and memory decisions.** Adding workers can
improve elapsed time while increasing total CPU work and memory. A shared
semantic store becomes attractive when duplication is measured; it need not be
the first architectural change in a compiler that has other dominant costs.

Persistent semantic caches and compressed cold representations have a later
place in this sequence. They are most useful when repeated startup or inactive
workspace retention remains expensive after simpler reuse and layout changes.

## Priorities for this implementation

The measurements below come from the repository's 2026-10-17 Command Center
benchmark: parser revision `9f53da6`, 4,154 searched files, an Intel i9 host with
16 GiB RAM, and release builds. Background activity and small load sample counts
limit precise comparisons. These are recorded observations, not new
measurements or projected improvements.

| Observed cost | Recorded magnitude | First investigation |
| --- | --- | --- |
| Freshness validation on warm navigation requests | About 440 ms, versus roughly 0.25–17 ms for the corresponding snapshot queries | Separate input-validation work from query work and identify checks that can be reused safely. |
| First request after an edit | 19.8–21.0 s for a full rebuild | Determine which file, binding, resolution, and semantic state can survive the edit. |
| Cold project load and retained service state | About 18–21 s for direct loads; 1.87 GiB retained after a cold service load | Attribute retained bytes and construction time to concrete structures. |
| Warm semantic member searches | 12.3–16.0 s for five selections, all partial; up to 1.4 GiB transient growth | Measure candidate counts, repeated receiver resolution, type construction, and cache reuse. |
| Service accept polling | An additional 0–20 ms before accepting a request | Consider a blocking or event-driven accept mechanism as a small independent improvement. |

The implementation already runs natively in Rust, uses a borrowing syntax arena,
and retains project snapshots in a service. The opportunity is to improve the
representation and reuse within those facilities.

1. **Make unchanged requests cheaper.** Freshness validation dominates otherwise
   quick navigation queries. Metadata checks or filesystem notifications are
   candidate mechanisms, but they must preserve the required freshness contract
   across source, configuration, directory, and resolution changes. A missed
   change must not become a successful stale answer.
2. **Reuse unaffected work after edits.** Start with reusable syntax and file
   facts where their dependencies are explicit. Extend reuse to binding and
   semantic results only as their invalidation rules become proven. Public
   interface fingerprints can stop downstream recomputation where they include
   every externally observable effect, including global and module augmentations.
3. **Reduce load cost and retained state.** Use memory profiles to choose between
   compact records, shared immutable data, fewer duplicated strings or lists,
   and deferred construction. Arena allocation alone does not establish that
   retained data is necessary or compact.
4. **Improve expensive semantic searches.** Establish whether repeated receiver
   resolution, relation checks, union construction, or candidate enumeration
   dominates before selecting a cache or algorithm. Keep result completeness
   visible: the measured searches have hundreds of gaps, so dropping additional
   work cannot count as a performance improvement.
5. **Add measured concurrency.** Compare worker counts using both elapsed time
   and peak memory. Preserve output ordering and exact results. Avoid multiplying
   semantic state before measuring how much it costs per worker.

File-only commands already complete in roughly 6–43 ms in this benchmark.
Scanner micro-optimizations therefore have less direct support as the first
priority, although aggregate parsing may still contribute materially to cold
project loading.

## Measurement and correctness criteria

Performance changes need separate measurements for cold loading, warm requests,
edits, and long-lived service operation. One full-build number cannot establish
that an interactive tool improved.

| Workload | Measurements |
| --- | --- |
| Cold project load | Wall time, CPU time, phase times, peak footprint, retained bytes, and allocation volume. |
| Unchanged warm request | Validation time, query time, filesystem operations, IPC, and serialization. |
| Private body edit | Reparsed files, rebuilt facts, invalidation fan-out, latency, and retained state. |
| Public signature or augmentation edit | Affected consumers, recomputation, and equality with a clean rebuild. |
| Semantic member search | Candidates, type resolutions, relation calls, cache hits and misses, peak memory, and result gaps. |
| Repeated edits and queries | Latency distributions, cache growth, eviction, cancellation, and steady-state memory. |
| Parallel execution | Wall time, total CPU, peak memory, duplication, and deterministic results across worker counts. |

Use representative projects together with pathological recursive, union,
conditional, and mapped types. Compare baseline and candidate under the same
toolchain and workload, repeat measurements, and retain distributions instead
of the best run. Instruction counts can help identify small CPU regressions;
they complement elapsed time and memory measurements.

For this repository, TypeScript 7.0.2 and the documented compatibility limits
define correctness. Extend the existing owner of each rule, preserve exact
names, and keep I/O and resource failures distinct. A performance limit must
not turn an unfinished computation into success. Follow the pinned-compiler
comparison procedures in [validation](../../validation.md), and assert final
query or project outcomes for every input variant the changed rule distinguishes.

Incremental behavior additionally needs equivalence with a clean rebuild after
each change. Test body edits, signature edits, additions and removals, resolution
changes, augmentations, failed refreshes, and transitions back to valid input.
That evidence determines whether reuse is correct; a cache hit rate alone does
not.
