# Single-thread performance opportunities

2026-10-17. An investigation, not a measurement of changes: nothing below has
been implemented. It profiles the three workloads that matter on Command Center
(a load, a warm member search, and the memory a loaded project retains), names
the code responsible for each cost, and ranks what to do about it on one
thread, before more threads are added. Each item notes how it fits a later move
to more parallelism.

## Summary

- **A load's critical path is one thread.** The loader thread was busy for 77%
  of a sampled load; the four capture workers were idle a third of the time.
  Everything on the loader thread is single-threaded today, so speeding it up
  shortens the load one for one. About 30% of its busy time is allocating,
  freeing, and copying memory; 14% is filesystem calls; 5.5% converts byte
  offsets to line/column for every indexed occurrence.
- **Half the capture workers' CPU is binding, a fifth parsing, a seventh
  navigation, a tenth reading files.** The last two are the surprises:
  navigation catalogs are built for 2,313 files that are then discarded because
  they are not searched, and every file is read into an unsized buffer that
  grows by doubling.
- **A member search re-parses files.** 26% of a 10 s member search is lexing and
  parsing files again and rebuilding a per-node attachment map with SipHash,
  one insert at a time.
- **Retained memory is dominated by six records.** Of 1,410 MiB retained,
  occurrences take 232 MiB (120 bytes each, 2.0 million), declarations 233 MiB
  (216 bytes, 0.97 million), native symbol headers 204 MiB (192 bytes, 1.08
  million), flow graphs 157 MiB, native symbol tables 110 MiB (295 thousand
  hash maps, 85% of them holding one to four entries), and a per-node scope map
  67 MiB. Shrinking those records to what they usually hold would plausibly
  remove 450–550 MiB.
- **One theme runs through load speed and memory: a name is allocated many
  times.** Each identifier occurrence allocates a `String` during collection;
  each declaration clones its name into a `Site`, again into a table key, and
  again into an `Arc<[u8]>` native name. Sharing one allocation per distinct
  name per file removes allocation work on both the workers and the loader
  thread, shrinks four records, and is parallel-ready because the interning is
  per file.

The recommended order is at the end: cheap and safe items first (sized read
buffers, exact-size retained slices, cached file views, direct `package.json`
reads, FxHash on lookup-only maps), then the index records, then the retained
records, then the name-sharing theme, then the collector's per-node maps and
tree walks, then the member search, then the parser and lexer.

## Method and caveats

- **Binary.** A release build without LTO, 16 codegen units, line tables, and
  frame pointers (`.cache/target-prof/release/`), so that `sample` attributes
  time to functions the release build inlines. Its outline output was
  byte-identical to the release binary's. One load was also sampled with the
  release binary (thin LTO, one codegen unit); its coarse shares agree (linking
  plus the specifier environment 73–74% of the loader thread).
- **Workloads.** A direct `--outline --project` load of Command Center
  (`tsconfig.json`, 6,473 files in the binding snapshot, 4,160 searched); a warm
  `--references --name AgentTaskRunner.run` member search against a private
  service of the same binary; and a counting-allocator probe of
  `Project::load` that drops the loaded project field by field.
- **Sampling.** macOS `sample` at 1 ms. Figures are sample counts and shares
  of one thread group's non-wait samples, not seconds, and come from one run
  each on a host that was also running other agents. The sampled load took
  16.7 s against about 9.4 s unsampled; the sampled member search 18.3 s
  against 9.9–10.2 s. Shares, not durations, are the evidence. Estimates of
  "share saved" are judgments from reading the code, not measurements.
- **Memory.** Requested heap from the counting allocator (bytes asked of the
  allocator, not RSS). The probe loaded in 8.8 s, retained 1,410.3 MiB, peaked
  at 2,412 MiB over its baseline, and the process peaked at 2,673 MB physical
  footprint. Record sizes are `size_of` in that build. Requested-heap savings
  overstate physical savings on macOS; judge any change by `footprint -p` and
  `time -l` as the earlier reports do.
- **Thread naming.** "Loader thread" is `tsparse-project-load`; "capture
  workers" are the four `tsparse-capture` threads; "semantic thread" is
  `tsparse-semantic`, which answered the member search alone.
- **Evidence.** Samples, attribution scripts, and the probe's output and source
  are under `.cc/temp/perf/single-thread/` (session-local, not committed).

## Where the time goes

### Loader thread (8,497 non-wait samples of 10,919)

| Cost | Samples | Share | Where |
| --- | ---: | ---: | --- |
| `Linker::declare` (inclusive) | 2,105 | 24.8% | `Store::new_lexical_symbol` 255, `NativeSymbols::declare_symbol_with_header` 238, `String::clone` 101, `NativeSymbolName::declaration` 104, `Store::hold`/`hold_member` 145 |
| `resolve_and_index` and `index::Builder::finish` (inlined into `LinkState::finish_with_native_facts`) | 1,815 | 21.4% | sorts 364, `SourceText::position`/`utf16_offset` 468, `String::clone` 125, malloc/realloc 271, iterator loops 647, `Builder::intern` 176 |
| `SpecifierEnvironment::capture` (inclusive) | 1,483 | 17.5% | `capture_packages` 403, `index_collision_paths` 332, `Case::key` (part of 366), `stat` 81, `realpath` 83 |
| Filesystem calls (all callers) | 1,222 | 14.4% | `stat` 700 (`Host::file_exists` 375, `PackageInfo::read` 178, specifier environment 81), `realpath` 203, `read` 207, `getattrlist` 112 |
| `Resolver::lookup_name` (inclusive) | 469 | 5.5% | of which `BindingFiles::file`/`reference`/`storage` 306 |
| `Case::key` (all callers) | 366 | 4.3% | `observation_key` 175, `KnownSymlinks` 74, `enqueue_as` 37, `file_lookup` 24, `package_observation` 51 |
| `Linked::retain_files` | 305 | 3.6% | converting captured vectors to boxed slices |
| `drop_in_place` | 220 | 2.6% | `LinkState` and consumed captures, at the end of `link` |
| Allocation, free, copy (self time, all callers) | ~2,500 | ~30% | `memmove` 583, `free` 484, `malloc` 445, `realloc` 322, `bzero` 108, hashing 329 |

### Capture workers (12,373 non-wait samples of 18,576; idle 33%)

| Cost | Samples | Share | Where |
| --- | ---: | ---: | --- |
| `binding::collect::Collector::run` (inclusive) | 6,186 | 50.0% | includes flow below; own hashing (`Collector::node` SipHash 106, rehash 205), `Collector::name` 289, `Collector::new` 211, `reference_tokens::capture` 164, `SiteMap::from_entries` 218 plus its sort 191 |
| `Parser::parse` (inclusive, includes lexer) | 2,499 | 20.2% | lexer 1,950 (`nom` `process` 717, `next_controlled` 537, `scan_comment_end` 203, `id_continue` 190, `unicode_escape` 115); `Parser::make` 349 |
| `navigation::*` (inclusive) | 1,782 | 14.4% | catalogs built for every non-library file; discarded for unsearched files |
| `flow::Builder::run` (inclusive) | 1,772 | 14.3% | sort 333, `syntax::Facts::new` 441, `children` 123, `Graph::read` 173 |
| `read` syscall | 1,253 | 10.1% | `disk::read_regular` → `read_to_end` into `Vec::new()` |
| `Vec` growth (`_realloc` and its copies) | 890 | 7.2% | `Parser::make` 334, `flow::Builder::run` 106, `Collector::name` 56, `capture_member_kind` 49, `Collector::run` 46 |
| `Ast::literal_token_spans` (two calls per file) | 452 | 3.7% | `binding::collect` 235, `navigation::projection` 211 |
| `ModuleIndicators::of` | 232 | 1.9% | a full walk for the first JSX node plus a `statements` collect |
| `SourceText::shared` | 188 | 1.5% | line starts and UTF-16 checkpoints |
| Dropping the syntax tree | 275 | 2.2% | `Captures::with_reuse` closure |

### Member search, semantic thread (11,170 samples, none waiting)

| Cost | Samples | Share | Where |
| --- | ---: | ---: | --- |
| Re-parsing files | ~1,750 | 15.7% | `Parser::parse` 550 plus lexer 1,201, under `Source::syntax` |
| Building and probing attachment maps | ~1,190 | 10.6% | `CapturedSyntax::new` (SipHash 190, rehash 317) 886; `CapturedSyntax::attachment` 303 |
| Native symbol ordering and identity | ~1,040 | 9.3% | `ordering::try_sort_by` 394, `declaration_read_origin` 293, `native_symbol` 166, `with_identity` 103, `native_declaration` 83 |
| Property/member evaluation | ~1,500 | 13%+ | `property_symbols` ~700, `named_members` 226, `generics::members_*`/`inherit_*` 264, `structure::reduced_*` 110, `NativeCompletion::and_then` 151 |
| Allocation, free, copy | ~1,360 | 12.2% | `malloc` 478, `memmove` 430, `free` 284, `realloc` 171; `String::clone` 182 mostly in `property_symbols` |
| `BindingFiles::reference`/`file` | 345 | 3.1% | the same view construction as on the loader |
| `Source::enclosing_calls` | 234 | 2.1% | |
| `CheckedMap::insert` | 120 | 1.1% | |

## Where the memory goes

Retained requested heap after `Project::load`: 1,410.3 MiB. Record sizes in
bytes: `Occurrence` 120, `IndexedOccurrence` 48, `IndexedSite` 64,
`ReferenceToken` 32, `Decl` 216, `Scope` 40, `NativeHeader` 192,
`NativeSymbolTable` 104, `NativePropertyKey` 24, `retained::Reference` 56,
`flow::Entry` 56, `flow::Container` 56, `Span` 16, `SourcePosition` 16.
Transient records: `collect::Name` 168, `store::Symbol` 280, `ast::Node` 152,
`ast::Value` 32, `index::Site` 48.

| Structure | Count | Retained |
| --- | ---: | ---: |
| `index.files` (per-file `Occurrence`s) | 2,026,125 | 232.1 MiB |
| `files[].decls` (`Decl`) | 966,819 | 232.6 MiB |
| `native.headers` (`NativeHeader`) | 1,077,235 | 204.2 MiB |
| `files[].flow` (graphs) | 565,935 nodes, 128,105 containers, plus read-site maps | 157.0 MiB |
| `native.tables` (one hash map per table) | 294,883 tables, 847,552 entries, 1,359,114 capacity | 110.2 MiB |
| `sources` (text, line indexes) | 6,473 | 77.7 MiB |
| `index.by_reference_name` (`ReferenceToken`) | 2,031,315 under 100,385 names | 68.7 MiB |
| `files[].lookup_scopes` (`SiteMap<ScopeId>`) | about one entry per syntax node | 67.2 MiB |
| `files[].refs` (`Reference`) | 1,215,945 | 65.1 MiB |
| `index.by_symbol` (`IndexedOccurrence`) | 859,036 under 166,403 symbols | 47.6 MiB |
| `specifier_environment` | | 23.1 MiB |
| `index.sites` (`IndexedSite`) | 166,934 | 19.9 MiB |
| `index.lexical_positions` + `reference_positions` | | 19.7 MiB |
| `navigation` (searched files' catalogs) | 4,160 | 15.1 MiB |
| `files[].module_decls/aliases/stars/ambient` | | 13.6 MiB |
| `files[].scopes` | 299,824 | 11.6 MiB |
| `native.declarations` map | | 11.0 MiB |
| everything else | | ~34 MiB |

Facts the probe adds about what these records hold:

- Every one of the 2,026,125 occurrences has `reference: Some(..)`; only 521
  have a multi-symbol target.
- Of 966,819 declarations, `hoisted` is empty in all of them, `type_parameters`
  is present in 31,221 (3%), `export` in 36,977 (4%), `interface_members` in
  25, and `member` in 468,162 (48%).
- Of 1,077,235 native headers, 5,599 spill their `declarations` SmallVec and
  577 have `late_members`.
- Of 294,883 native tables, 11,036 are empty and 249,736 hold one to four
  entries. Capacity exceeds entries by 60%.
- Flow nodes and containers account for about 37 MiB of the 157 MiB of flow
  graphs; the per-site read maps hold the rest. Measure before touching.
- Peak over retained is about 1,000 MiB: 2.0 million `Name` records at 168
  bytes (325 MiB), the linker's `Store` with 280-byte `Symbol`s, the `Token`
  vectors, and in-flight syntax trees. Sessions also keep the `Name` inventory
  as their reuse recipe, which is most of the service's growth from 1,103 to
  2,014 MB after edits in [refresh reuse](performance-refresh-reuse.md).

## Opportunities

Each item: what, the evidence, the estimated effect (as a share of the thread
group it affects, with memory in MiB where it applies), the effort, correctness
notes against [AGENTS.md](../AGENTS.md), and how it relates to adding
parallelism later. Items are grouped by workload; the ranking across groups
follows.

### Loader thread (the load's critical path)

**L1. Stop converting every indexed occurrence to line/column at load, and
sort on `u32` keys.** `Builder::finish` builds an `IndexedLocation` for every
occurrence of every symbol and every site (`SourceText::position` twice each),
then sorts each symbol's occurrences by `(u32, SourcePosition, SourcePosition)`,
a 40-byte key with `usize` fields. Within a file, byte order and position order
are the same, so `(display rank, start, end)` as three `u32`s sorts identically,
compares faster, and needs no conversion; convert to line/column when a query
returns an occurrence. Evidence: position conversion 468 samples (5.5% of the
loader), sorts 364 (4.3%). Effect: 7–9% of the loader thread; `IndexedOccurrence`
48 → 20–24 bytes (−20 MiB), `IndexedSite` smaller. Trade-off: the 2026-10-06
work moved conversion to load time for query speed; converting only returned
results costs on the order of 0.1 µs each, so a 10,000-occurrence result pays
a few milliseconds against today's 17.5 ms median for that band. Alternatively
compute positions on the capture workers per file. Effort: medium, one owner
(`index.rs`). Proof: the reference-result equality tests and the oracle; the
order is unchanged by construction. Parallel-ready: yes, per-symbol work on
immutable inputs.

**L2. One allocation per distinct name across `Decl`, `Site`, table keys, and
native names.** `Linker::declare` clones `decl.name` into a `Site`, clones the
`Site` again for `Declaring`, allocates a `String` key in `Store::hold` and
`hold_member`, and allocates an `Arc<[u8]>` in `NativeSymbolName::declaration`
for every declaration; the fixed internal names (`__type`, `__object`,
`__function`, `__class`, `__call`, `__new`, `__index`, `__constructor`,
`__computed`, `__missing`, `default`) are allocated afresh each time by
`NativeSymbolName::internal` and `Bytes(b"default".into())`. Evidence:
`String::clone` 226 samples, `NativeSymbolName::declaration` 104, `hold`/
`hold_member` allocation 145 (together about 5.5% of the loader). Fix: give
`Decl.name` a shared representation (`Arc<str>`, or a per-file `StringId` into
a per-file string table built by the collector) and let `Site`, table keys, and
`NativeSymbolName::Bytes` share it; make the internal names statics. Effect:
4–6% of the loader; fewer small allocations in three retained records (see M2,
M4). Effort: medium; touches collect, store, ordering, index. Exact names are
preserved: the bytes are the same, only shared. Parallel-ready: interning is per
file on the workers; the loader merges tables, so no shared mutable interner.

**L3. Pre-size the symbol vectors and shrink `Symbol`.** `Store::new_lexical_symbol`
pushes a 280-byte `Symbol` by value into a growing `Vec`, and `NativeSymbols`
pushes 192-byte headers; growth copies them. The totals are known before
linking (the sum of each file's declaration count), so `Vec::with_capacity`
removes the copies. `Symbol` carries `lists`, `late_members`, `aliases`,
`homes`, and `quoted_module_name` that are rarely used; side tables or boxing
shrink the hot record. Evidence: `memmove` under `new_lexical_symbol`,
`new_symbol`, and `declare_symbol_with_header` about 230 samples (2.7%).
Effect: 2–3% of the loader; lower peak memory (the `Store` is transient).
Effort: small for pre-sizing, medium for the split.

**L4. Cheaper filesystem work on the loader thread.** Four independent pieces:
(a) `PackageInfo::read` stats before reading (178 samples); read directly and
classify the error, treating a path as absent only on `NotFound`/
`NotADirectory` as AGENTS.md requires. (b) `capture_packages` walks every
ancestor of every file; files in one directory share all ancestors, so dedupe
by parent directory first (6,473 files, far fewer directories), which also
cuts `observation_key`'s `Case::key` and `paths::combine` allocations (175 and
85 samples). (c) `Case::key` normalizes and lowercases into a new `String` on
every call, and on this host every absolute path has an uppercase letter
(`/Users`), so the `Cow::Borrowed` fast path never applies; lowercase into a
reused scratch buffer and look up by `&str`, or key the memo by the normalized
path and fold only on a miss. (d) `realpath` under `for_any_importer` (83
samples) can be memoized per directory if it is not already. Effect: 4–6% of
the loader. Effort: small each. The remaining `stat`s (`Host::file_exists`,
375 samples, 4.4%) are TypeScript's extension probing; replacing them with
per-directory listings would change the observation model that validation
replays, so that is a design change for later, not a quick win.

**L5. Build the `BindingFiles::file` view once per file.** `BindingFiles::file`
copies about fifteen fields into a `File<'a>` on every call, and
`Resolver::lookup_name` calls it per scope step; the same happens on the
semantic thread. Evidence: `file` + `reference` + `storage` 306 samples (3.6%
of the loader) and 345 (3.1% of the member search). Fix: a `Vec<File<'a>>`
built once, or return `&ProgramFile` and read fields through it. Effort: small.
Effect: 3% of each.

**L6. Exact-size retained slices and deferred drops.** `retain_files`
converts captured vectors into boxed slices (305 samples, 3.6%), and `link`
drops the `LinkState` and consumed captures on the critical path (220 samples,
2.6%). The collector can emit exact-size slices once a file's collection ends
(moving the shrink to the workers), and the drops can run after the project
is published. Dropping owned data on a helper thread needs no shared state, so
it is the one threading change this report recommends now. Effect: 4–6% of the
loader. Effort: small.

**L7. Intern names per file, not per occurrence.** `Builder::intern` hashes
every occurrence's name (176 samples, 2.1%). If the collector assigns each
`Name` a per-file name index, the loader maps per-file indexes to global
`StringId`s once per distinct name per file. Pairs with L2 and C5. Effect:
about 2% of the loader. Parallel-ready by construction.

**L8. FxHash for lookup-only maps on the loader.** `Store::tables` are
`HashMap<String, Entry>` with SipHash, probed on every lexical lookup
(`probe_entry`, `lookup_name`); `Index.by_name`, `by_symbol`, and `sites` are
never iterated for output. `Store::tables` are iterated in `member_entries`
and `Linker::propagate`, but with `RandomState` their order already varies per
process, and outputs are byte-identical across runs, so no consumer can depend
on it; confirm no fixed-seed hasher is in use before switching. Evidence:
SipHash and `hash_one` 329 samples (3.9%) on the loader, part of it in these
maps. Effect: 2–3%. Effort: small; `crate::hash::FastMap` exists.

**L9. Fewer relative-path comparisons.** `RelativePaths::to` compares every
importer directory with every target (`index_collision_paths`, 332 samples,
3.9%). Grouping importer directories by shared prefix (a directory trie, which
[relative paths](performance-relative-paths.md) already considered) walks each
shared prefix once per target. Effect: 2–3%. Effort: medium. Benchmark; it is
not shown faster yet.

### Capture workers

**C1. Size read buffers.** `disk::read_regular` calls `metadata` twice and then
reads into `Vec::new()` with `read_to_end`, which grows by doubling, so each file
takes several `read` syscalls and reallocations, and freshly grown buffers fault
in pages (the load took 1.05 million page reclaims). Evidence: `read` 1,253
samples, 10.1% of capture CPU; copying 100 MB from the page cache should take
tens of milliseconds. Fix: `Vec::with_capacity(len + 1)` from the `fstat`
already made, as `std::fs::read` does. Effect: most of that 10%, plus the
loader's own in-order reads (207 samples). Effort: tiny; the bytes read are
unchanged. The observation record is unchanged too (same path, same content).

**C2. Do not build navigation for unsearched files.** `capture` builds
`Captured::typescript` for every non-library file; `program/mod.rs:298` then
keeps it only `if search`. 2,313 of 6,473 files are unsearched, and they
include the large `node_modules` declaration files. Evidence: navigation is
14.4% of capture CPU. Fix: decide `search` when a job is reserved (roots come
from the configuration's `files`/`include` and are enumerated first; confirm a
file first reached as a dependency can never later become searched) and skip
the catalog. Effect: perhaps 7–9% of capture CPU and transient memory. Effort:
small once `search` is available at reservation.

**C3. One pass per file for parents, literal spans, module indicators, and
call targets.** `Ast::literal_token_spans` walks and sorts the whole tree, and is
called twice per file (comment directives and the navigation projection; 452
samples, 3.7%). `ModuleIndicators::of` walks the tree for the first JSX node
(232, 1.9%). `flow::syntax::Facts::new` collects every node id into a `Vec` and
derives parents (441, 3.6%). `Collector::new` walks for call targets (211,
1.7%). The checker's `CapturedSyntax::new` recomputes parents again at query
time. The parser knows each of these as it builds nodes: literal tokens in
source order, the first JSX node, and each child's parent at `make`. Record
them in `Ast` (keeping `literal_token_spans` as the owning API, now reading
recorded data) or compute them in one shared pre-order pass. Effect: 6–9% of
capture; also removes query-time work (see S1). Effort: medium; `Ast` is the
owner. Proof: the conformance corpus comparison already checks the tree.

**C4. Replace the collector's per-node hash maps with node-indexed vectors.**
`Collector::node` inserts `(SyntaxKind, Span) → ScopeId` into a SipHash map for
every node, and `run` collects and sorts it into a `SiteMap`; `named` and
`call_targets` are `HashSet<NodeId>`; `flow::Builder` collects `reads` then
sorts them; `contains_this` is keyed by `(usize, usize)`. Evidence: hashing and
rehashing in `Collector::node` 311 samples plus `SiteMap::from_entries` 218
and its sort 191, and the flow sort 333 (together about 8% of capture). Fix:
`Vec<Option<ScopeId>>` and bit vectors indexed by `NodeId`; build the sorted
`SiteMap` from node ids in span order. Effect: 5–7% of capture. Effort: small
to medium. Enables M6.

**C5. Shrink `Name` and stop allocating its text per occurrence.** `Name` is
168 bytes and holds `text: String`, allocated for each of 2.0 million
identifier occurrences (`Collector::name` 289 samples, 2.3%). Identifiers
without escapes equal their source slice, which the span already locates;
escaped identifiers need an owned decode. Store a per-file `StringId` (L7) or a
`Cow` into the source, and move the large `reference` and `target` payloads to
side tables. Effect: 2–3% of capture CPU; 150–250 MiB of peak memory and of a
session's retained reuse recipe. Effort: medium. Exact names are kept: the
decoded text is unchanged, only its storage.

**C6. Reuse the parser's node buffer and shrink `Node`.** `Node` is 152 bytes,
pushed by value; the capacity heuristic (`source.len() / 16`, capped at 16,384)
leaves dense or large files reallocating (`_realloc` under `Parser::make` 334
samples, `memmove` 349). The parser is per worker thread, so its `Vec<Node>`
can be kept and reused across files. `kind: &'static str` duplicates
`syntax_kind` (16 bytes); `native_literal_text: Option<Vec<u8>>` and
`native_jsdoc` are rare and could be side tables. Effect: 3–5% of capture and
lower peak memory. Effort: medium; `Node` is public API, so removing `kind`
needs an accessor and a deprecation note.

**C7. Lexer fast paths.** The lexer is 15.8% of capture CPU: `nom`'s `process`
717, `next_controlled` 537, `scan_comment_end` 203, `id_continue` 190,
`unicode_escape` 115. Candidates: `memchr` (vendored) for `*/`, line breaks,
and trivia runs; an ASCII fast path for identifier continuation; checking for
`\u` once per token instead of per character; and the measured 4× `memchr`
version of `SourceText::shared` (151 → 36 ms on 36 MB). Effect: 3–6% of
capture. Effort: small per item. These are lexical-class decisions, so each
must live in the lexer owner (`lexer::line_break`, `whitespace`, `comment_end`)
and be proven on the 14,935-case conformance corpus plus the CR/LF/CRLF/
U+2028/U+2029/BOM variants AGENTS.md lists.

### Member search (semantic thread)

**S1. Cheaper syntax recovery.** `Source::syntax` re-parses a file the first
time a request touches it and builds `CapturedSyntax`: a `parents` vector and a
SipHash `attachments` map with `reserve(1)` before every insert, four inserts
per node. Evidence: 26% of the search (re-parse 15.7%, map build and probes
10.6%). Fixes, in order of effort: reserve once (`nodes × 4`); FxHash or a
sorted vector keyed by `(role, kind, start, end)` built from one walk; parser-
recorded parents (C3); and, optionally, a bounded cache of `CapturedSyntax` for
recently touched files in the service, which trades memory for the re-parse.
Effect: 10% from the map alone; up to 25% with a cache. Effort: small to
medium.

**S2. Sort without cloning.** `ordering::try_sort_by` is a merge sort that
clones each element on every merge level (`merged.push(values[left].clone())`),
and `NativeSymbol::with_identity` publishes a new `Arc` per instantiated
symbol. Evidence: `try_sort_by` 394 samples, identity and origin lookups about
650 (together 9.3%). Fix: sort a permutation of indices and apply it once, or
sort `Arc` pointers; keep the fallible comparison and the deterministic
schedule the comment requires. Effect: 3–4%. Effort: small.

**S3. Allocation churn in property evaluation.** `property_symbols`,
`generics`, and `named_members` clone `String`s (182 samples) and vectors and
allocate per member (malloc/free/memmove about 12% of the search). Audit the
hot paths for owned copies that could borrow or share an `Arc`. Effect: 5–8%.
Effort: medium; needs the evaluator's ownership rules kept intact.

**S4. The file view (L5)** also applies here: 3.1%.

**S5. Work avoidance across candidate uses.** The search resolves receiver
types at every same-named use; the 2026-10-17 report asks for candidate counts
and repeated-receiver measurements before choosing a cache. Not quantified by
this profile; listed so the order below leaves room for it.

### Retained memory

**M1. `Occurrence` 120 → about 40 bytes (−150 MiB).** `reference` is `Some` for
all 2,026,125 occurrences, so the `Option` saves nothing and
`ReferenceTokenData { key: Result<StringId, String>, syntax }` is inline for
everyone; `target: Target` holds a `SmallVec<[SymbolId; 2]>` though only 521
occurrences have more than one symbol; `span` is two `usize`s. Store one
`SymbolId` inline with a side table for the 521, a `u32` span, and the
reference key's `Err(String)` out of line. Effort: medium; one owner
(`index.rs`). Proof: result equality tests.

**M2. `Decl` 216 → about 100 bytes (−100 MiB).** `hoisted: Vec<ScopeId>` is
empty in every declaration here; `type_parameters`, `export`, and
`interface_members` are present in 3%, 4%, and 25 declarations; `native_span`
is an `Option<Span>` of `usize`s; `name: String` becomes shared under L2. Move
the rare fields to per-file side tables keyed by `DeclId`. Effort: medium.

**M3. Small native symbol tables (−50–60 MiB, and faster inserts).** 249,736 of
294,883 tables hold one to four entries, each in its own `HashMap` allocation,
and capacity exceeds entries by 60%. An inline representation for small tables
(a short sorted or linearly scanned vector, promoting to a map past a
threshold) removes most of that overhead and the `reserve_rehash` growth seen
in `NativeSymbols::insert` (96 samples on the loader). `entries()` consumers
already cannot depend on iteration order (L8). Effort: medium; one owner
(`NativeSymbolTable`).

**M4. `NativeHeader` 192 → about 120 bytes (−70 MiB).** `late_members: Vec` is
non-empty in 577 of 1,077,235 headers; `declarations` spills in 5,599;
`name: Option<NativeSymbolName>` holds an `Arc<[u8]>` allocated per header
(shared under L2). Side tables for the rare fields. Effort: medium.

**M5. `ReferenceToken` 32 → 8–12 bytes (−40 MiB) and `Reference` 56 → about
24 (−30 MiB).** `ReferenceToken` is a `FileId` and a two-variant enum over
`u32`-sized ids; 32 bytes means padding or oversized ids. `retained::Reference`
pairs an `OccurrenceId` with optional origins. Check each field's width.
Effort: small to medium.

**M6. `lookup_scopes` as ranges plus exceptions (−55 MiB).** The map holds an
entry for about every syntax node (67 MiB at 16 bytes each), but there are
299,824 scopes. Most nodes' lookup scope is the innermost scope whose range
contains them; a range table with explicit entries only where TypeScript's
rule differs (type parameters, computed names, and the other cases the
collector handles) would be a tenth the size. Effort: medium to high; the
per-node scope rule is TypeScript's `resolveName` container logic and must stay
in its owner. Pairs with C4.

**M7. Share the session reuse inventory with the published index (−300 MiB
or more in a service).** [Refresh reuse](performance-refresh-reuse.md) notes
that sessions keep a second name/reference-syntax inventory the published index
already projects. With C5 shrinking `Name`, sharing the rest closes most of the
1,103 → 2,014 MB growth after edits. Effort: medium to high.

**M8. Measure the flow read maps.** 120 MiB of the 157 MiB of flow graphs is
outside the node and container arrays; find out whether the `SiteMap<Read>`
entries are in the wide form or simply numerous, and whether declaration files
contribute graphs that no checker path reads. Measure first.

## Ranking by impact

| Rank | Item | Affects | Estimated effect | Effort |
| ---: | --- | --- | --- | --- |
| 1 | M1–M5 record shrinks | retained memory | −450–550 MiB of 1,410 (35–40%); faster inserts and sorts | medium each |
| 2 | L1 positions and `u32` sort keys | load critical path | 7–9% of the loader; −20 MiB | medium |
| 3 | L2 + L3 + L7 one allocation per name, pre-sized symbol vectors | load critical path; peak and retained memory | 7–10% of the loader; fewer allocations in three records | medium |
| 4 | C1 sized read buffers | capture CPU, loader wait | most of 10% of capture CPU; 207 loader samples | tiny |
| 5 | S1 syntax recovery | member search | 10–25% | small–medium |
| 6 | C2 skip navigation for unsearched files | capture CPU, transient memory | 7–9% of capture | small |
| 7 | L4 filesystem on the loader | load critical path | 4–6% | small each |
| 8 | L6 exact-size slices, deferred drops | load critical path | 4–6% | small |
| 9 | C3 + C4 one pass per file, node-indexed maps | capture CPU | 11–16% of capture | medium |
| 10 | C5 `Name` shrink | peak and session memory, capture CPU | −150–250 MiB; 2–3% | medium |
| 11 | L5 cached file view | load and member search | 3% of each | small |
| 12 | L8 FxHash on lookup-only maps | load critical path | 2–3% | small |
| 13 | S2 + S3 ordering and churn | member search | 8–12% | small–medium |
| 14 | C7 lexer fast paths | capture CPU | 3–6% | small each |
| 15 | C6 parser buffer reuse and `Node` shrink | capture CPU, peak | 3–5% | medium |
| 16 | M6 scope ranges, M7 shared inventory, M8 flow maps | retained memory | −55 MiB; −300 MiB in services; unknown | medium–high |
| 17 | L9 relative paths | load critical path | 2–3% | medium |

The loader-thread items together are about 30–40% of its busy time, which is
most of a load's wall time. Capture items are CPU and energy, and shorten a
load only through the loader's wait on captures (about 1.5 s of a 9 s load
in the 2026-10-17 report); they matter more for refreshes and for how many
workers are worth adding later.

## Recommended implementation order

The order favours changes that are cheap to prove, that make the later
measurements quieter, and that leave the data in a shape parallel work can use:
per-file immutable products, counts known before linking, no shared mutable
caches.

1. **Harness first.** Keep using retired instructions (`/usr/bin/time -l`),
   `footprint -p`, and byte-identical outputs (`.cc/temp/perf/warm.py`,
   `edits.py`) as the gate; instruction counts are stable on this shared host
   where wall time is not.
2. **Cheap and safe (days):** C1 sized reads; L6 exact-size slices and
   deferred drops; L5 cached file views; L4(a–c) direct `package.json` reads,
   directory-level ancestor dedupe, `Case::key` scratch buffer; L8 FxHash on
   lookup-only maps; C3-lite (compute `literal_token_spans` once per file;
   record the first JSX node in the parser). Each is a few lines with an
   existing test to extend. Re-measure the load.
3. **Index records (one owner, `index.rs`):** L1 with M1 and M5, since they
   change the same records and tests. This is the largest single load win and
   about 200 MiB.
4. **Retained binder records:** M2 `Decl` side tables, M4 `NativeHeader` side
   tables, M3 small tables. Each is a mechanical change inside one owner with a
   `size_of` assertion as its regression test.
5. **Name sharing, capture to link:** C5 and L7 on the workers, then L2 and L3
   on the loader. Do this after step 4 so the records it touches are already
   small, and design the per-file string table as the unit the loader merges.
6. **Collector maps and walks:** C4 node-indexed vectors, then C3 in full
   (parser-recorded parents, literal spans, call targets). M6 follows once the
   scope map is built from node ids.
7. **Member search:** S1 (reserve, FxHash or sorted vector, parser-recorded
   parents from step 6), S2, S3, then measure candidate counts for S5.
8. **Parser and lexer:** C6 buffer reuse and `Node` shrink; C7 fast paths
   inside the lexer owner with the full input-variant proof.
9. **Re-profile, then choose the parallel target.** By this point the linker's
   inputs are per-file, immutable, and compact, the symbol vectors are
   pre-sized, and names are interned per file. Linking (`resolve_and_index`,
   `declare`, the specifier environment: 73–74% of the loader thread) is the
   target; its hard part remains a concurrent memo for `Resolver::force` that
   detects cycles and merges in TypeScript's order.

## Rules to keep while doing this

- **One owner.** Fast paths in the lexer stay in `lexer.rs`; relative-path
  work stays in `RelativePaths`; position conversion stays in `SourceText`;
  the per-node scope rule stays with the collector. A record shrink changes
  storage, not the rule.
- **Exact names.** Shared or interned names keep their exact bytes. Nothing
  here lowercases, folds, or replaces characters outside the existing
  `Case::key` owner.
- **Failures stay failures.** Reading `package.json` directly instead of
  stat-then-read must still treat only `NotFound`/`NotADirectory` as absence
  and report every other error.
- **Done means proven.** Each change ships with a test that fails without it
  (a `size_of` bound, an allocation budget through the public API as the
  2026-10-06 work did, or a syscall count) and with byte-identical outputs on
  the Command Center journeys.
- **Measure physical memory.** Requested-heap savings from freed capacity or
  smaller records do not all reach the footprint; report `footprint -p` and
  `time -l` alongside.

## Evidence

Under `.cc/temp/perf/single-thread/`: `prof-load-sample.txt` and
`prof-member-sample.txt` (macOS `sample` output of the no-LTO binary),
`attr.py` and `callers.py` (attribution by function and by nearest project
caller), `probe-out.txt` (the counting-allocator breakdown), and `probe/`
(the probe's sources and the script that applied and removed it). The earlier
release-binary load sample is `gpu-review-sample.txt`. The no-LTO binary is
`.cache/target-prof/release/tsparse`; its outline output matched the release
binary's byte for byte.
