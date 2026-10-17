# Retaining the completed lexical binding product

**Historical ownership stage.** The 2026-10-13 storage checkpoint supersedes
the retained `Completed`/lexical Store design below. Native checker consumers
now use the independent raw binding snapshot; no production consumer remained
for the completed lexical Store or its saved demand vectors. The linker still
performs its full checks and rejects in-progress work before discarding those
construction-only records. The published Project keeps the raw native snapshot,
lexical Index and immutable file facts. See
[current storage measurements](types-storage-performance.md) for the replacement,
compatibility evidence and limits. The receipts below describe the earlier
stage, not the present representation.

This is an implemented ownership stage of T/Q3/Q4, using the
`ousterhout-software-design` skill. Its envelope is one immutable local Project
generation, with complete replacement on refresh. It preserves the existing
lexical behavior and input failures; it is not the complete proposed
`BoundProgram`, a native type checker, or closure of either semantic gate.

## Ownership change and publication boundary

Previously `link::link` returned only the lexical Index and dropped Store,
scope/declaration associations and completed resolution answers. Project also
dropped the remaining ProgramFile facts. Reconstructing these later would
introduce a second lexical binder and lose the existing merge/tolerance decisions.

The chosen increment moves those already computed facts into private
`binding::retained::Bindings`. Keeping only the old Index would leave that
ownership problem. Migrating a complete semantic representation now would also
change collection and checking behavior before their baseline is complete.
The retained lexical product establishes one owner without making that larger
change.

`link::link` still runs the same sequence: mixed-export and module-declaration
checks, module/augmentation creation, request resolution, declaration binding,
augmentation/global merges, hoisting and merge checks, searched reference
projection, and alias conflict checks. Only then does `Linker::complete` move
the product out. Store's existing earlier freeze ends writes/merging; it is not
the publication boundary. Alias conflicts can force additional answers after
Index creation, so the final retained demand states are captured after them.

Project captures FileLookup and transfers navigation to its current owner,
then consumes its ProgramFile vector through `Linked::retain_files` before
publishing the Project. The linked-to-retained transition consumes ownership;
it does not clone ProgramFile or create a queryable partial Project.
ProjectSession's existing generation replacement/failure policy is unchanged.

## Exact transfer

| Existing product | Retained owner / disposition |
| --- | --- |
| Store tables, symbols, sparse parents, modules, request targets, interface-name facts | Moved once into `Completed.store`; `Store::find` remains the only canonical-symbol lookup. There is no dense remap or checker lookup copy. |
| Symbol own sites, aliases, type-parameter lists, member/export table links, flags | The same Store records; own origins are not replaced by directed lexical projections. |
| Scope tables and declaration-to-symbol arrays | Moved in original FileId/file-local-ID order. Original IDs may redirect through Store's parents; consumers must use its existing canonical lookup. A missing table/symbol association keeps its prior meaning. |
| File modules, exact ambient module map, ordered patterns, module/global augmentation associations | Retained. Temporary ambient fragments become associations to their existing final canonical symbols before fragments are dropped. |
| Final alias/reference resolutions | `Demand::Resolved(Resolution)` preserves Found's per-meaning targets and type-only provenance, GlobalThis, ValueTarget identity/access, NotReference and exact Unresolved text. |
| Unvisited alias/reference tasks | `Demand::NotDemanded`; distinct from every completed result. No unused alias or dependency reference is forced by retention. In-progress states at successful completion are an internal invariant failure, never a tolerated absent result. |
| ProgramFile searched membership, module/declaration/JSX facts, exact module requests and raw loader resolutions | Moved into one retained File record. Raw loader outcomes stay distinct from Store's settled Module/Script/Ignored request answer. |
| Existing Diagnostics product | Moved with its established reporting/tolerance policy and directive coverage. It does not certify semantic admission. |
| Scopes, declarations, refs, aliases, stars, ambient declarations, namespace instantiation facts | Moved in their existing file-local identity order. Retained declaration records still include some consumed validation metadata; this stage does not promise compact complete semantic records. |
| Narrow JSON module and apparent-interface facts | Moved unchanged; not upgraded to recursive array/key/member recipes. |
| ProgramFile paths and navigation | FileLookup already owns searched spellings; Navigation already owns its captured catalogs. These fields do not remain in binding. |
| Facts.names, mixed-export and misplaced-module validation lists; raw comment-directive starts | Dropped after all checks. Diagnostics already owns directive coverage. Name attachments are consumed as described below, so no dangling NameId remains. |
| Resolution visitation stack, invalidation journal, cycle work, conflict work, fragments, reverse value-creation map, optional export memo | Builder-only scaffolding/caches are dropped. Existing value-symbol-to-ValueTarget associations survive. No new operation resumes pending lookup from an incomplete saved task stack. |

The lexical Index remains the existing derived accelerator and directed group
origin cache. Its flattened `Target::Symbols` does not replace the richer
retained Resolution. Existing result order, role/call aggregation, declarations,
query failures and reporting filters are unchanged.

## One searched inventory and exact source attachments

`Facts.refs` originally stored a NameId into `Facts.names`. Clearing names
without replacing that association would leave an invalid frozen record.

Index construction now returns a temporary NameId-to-OccurrenceId permutation.
It follows the existing stable start-position sort and keeps each source row's
identity even if rows tie. Its file-position occurrence array is not deduplicated;
the existing separate public by-symbol projection still combines physical
locations' roles/call bits. Retained searched references use that exact
OccurrenceId, whose existing row owns the name/span. They do not search by span
again or reinterpret a shared physical span as another semantic role.

Dependency references remain outside the searched occurrence array. Only their
own exact decoded StringId and source span survive, using the existing Index
string interner; those strings do not enter candidate or by-symbol indexes.
The temporary permutation is dropped. There is no second retained searched
name/token array, no parser NodeId, and no reparse/rebinding.

This preserves the current collector's inventory, including its omissions. It
does not add dual shorthand/member/parameter roles, computed/literal key sites,
private/member identity, or a complete dependency token inventory.

## Pending semantic work

`NotDemanded` records only the existing lexical load schedule. A completed
lexical NotReference such as `property` is also not semantic unsupported: its
receiver/key/member evaluation is still pending. Built-in GlobalThis and narrow
JSON/library-member ValueTarget records are identities, not complete type
recipes. Completed Unresolved text is preserved without claiming the new typed
failure provenance/causal recovery contract.

No retained-fact query API is added by this stage. Complete semantic-source
terms, per-file format retention where needed, flow graph/read/end attachments,
static/instance and private/protected members, recursive JSON/key origins,
selection-role replay, native checking and the full canonical/representation
checkpoint remain open. Future operations must consume these facts through
their existing binding decision owners; they must not copy Store lookup or
force pending dependency work as a new load policy.

## Validation and measurements

Ownership/schema wiring introduces no new externally observable behavior, so
it uses the session's mechanical-change exception to test-first. No artificial
RED or new public inspection API was introduced. Private Project::load checks
verify transfer at the final Project boundary, alongside existing public query
controls; registered compatibility checks protect the old behavior.

Registered final validation is terminal GREEN:

- Full Rust test: `vrun-5fbf35fd-c245-4fa9-ae89-5168bc201b54`. All five retained
  Project tests executed. Existing S/C/Q1/Q2/E, JSON, augmentation, session and
  reference compatibility checks also passed. The wrapper reports two existing
  macOS non-UTF-8 filesystem cases as skipped; the existing library-listing
  diagnostic remains ignored. Neither is counted as retention proof.
- Full format: `vrun-66ffc20d-b5b7-4a69-a8e6-3313190d534a`.
- Full lint: `vrun-21a54056-37ff-46a5-9e58-70bb88897296`.
- Full semantic compatibility: `vrun-444209d5-d9ed-46be-8638-c33729836262`.

The five private final-Project controls cover searched attachments after the
final module/await tree; ordinary, fixed-width escaped and braced escaped names
with BOM removal, astral text and all five line-break forms; exact unsearched
dependency names and undemanded work; type-only meaning provenance and sparse
merged identity; alias conflicts before publication; and distinct Store own
origins versus the directed Index alias projection. Public group and UTF-16
position controls accompany the retained-fact assertions.

Eighteen fresh pinned TypeScript 7.0.2 reference controls independently agree
with those final groups, with zero oracle anomalies. These are narrow compiler
diagnostics retained in `pinned-control.mjs` and `pinned-control.json`, not a new
shipping-test gate. The [independent ownership review](types-binding-retention-review.md)
found no blocking defect and closed two initial assertion gaps.

The existing release reference benchmark was captured before and after wiring
retention. Registered release builds were
`vrun-fecc75cf-b6b8-47d4-9e2c-72de5d2c7199` and
`vrun-b2e63b70-98ab-4316-be81-789c3483c411`. Saved binaries and source hashes pin
both measurements. After that retained build, only private test assertions and
formatting in `retained.rs` changed; all other final source hashes match.

Both corpora share 301 TypeScript files totaling 165,175 bytes, generated by the
existing 300-module benchmark. The first uses `noLib`; the second adds the normal
default libraries. Three before/after rounds alternate order. Each run records
a median of five load times, a separate allocation-instrumented load, and 2,000
lexical position questions. The table reports the middle of each set of three
load medians and allocation peaks; retained requested heap was identical within
each version's three rounds.

| Corpus | Load median before / after | Peak requested heap before / after | Retained requested heap before / after | Added retained heap |
| --- | --- | --- | --- | --- |
| 300 modules, no libraries | 81.15 / 82.91 ms | 8,504,000 / 8,633,919 bytes | 2,677,925 / 6,595,797 bytes | 3,917,872 bytes, **146.3%** |
| Same roots with default libraries | 203.86 / 208.05 ms | 34,353,771 / 34,354,283 bytes | 6,482,969 / 24,802,734 bytes | 18,319,765 bytes, **282.6%** |

All twelve runs have the same query outcomes: 1,175 answered groups, 532 empty
answers and 293 existing unsupported targets; no outside-project, invalid
position or unresolved-target result. The small change in peak heap does not
make the retained cost small: the former transient binding records now remain
alive after load. This fat mechanical representation has a material cost and
requires compaction or measured justification before semantic delivery.

The timing machine was not CPU-isolated; other compiler/harness work ran during
capture. The load differences (about 2%) are descriptive, not a performance
acceptance result. Heap figures count requested live allocations, excluding
allocator overhead, process RSS, stack and other application memory. No native
semantic demand, changed-generation refresh or full Command Center retained
measurement was made. The ratios cannot be extrapolated as a bound on other
programs.

The archive `.cc/temp/types-binding-retention` contains raw runs, both source
captures, binaries, the exact copied root inputs, vendored-library hashes,
fresh native controls and `measurement-summary.json`.
`provenance.json` hashes 386 artifacts and records the command arrays and
verification boundary; its SHA-256 is
`389aa84960a7d06b0676867e9d393b52367e22bb4bb0aee376646541431f2559`.
Full semantic execution and representation gates remain open.
