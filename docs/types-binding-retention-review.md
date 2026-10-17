# Independent review of retained lexical bindings

No blocking defect was substantiated in this bounded ownership change. The
review used the `ousterhout-software-design` skill and its reviewing route. The
operating envelope is one immutable local Project generation, replaced as a
whole by the existing refresh owner. This is a review of retaining the existing
lexical product, not of a native checker or the proposed complete BoundProgram.

## Findings and proof limits

Two low-priority assertion gaps were found in the initial four private tests
and were closed by the owner during review:

1. `retained_searched_attachments_survive_projection_and_final_tree_reparse`
   includes an escaped declaration, but its searched reference spellings are
   ordinary identifiers. The dependency test separately asserted both decoded
   `Missing` and raw `M\\u0069ssing`. The searched test now combines ordinary,
   fixed-width escaped and braced escaped reference spellings with all five
   line breaks. All 15 final Project loads assert decoded name, raw span,
   retained resolution attachment and final public group/UTF-16 position.
2. The retained Store's own sites remain distinct from the Index's directed
   alias group projections by source inspection. The new tests exercise a
   merged class/namespace and type-only routes, but did not directly compare an
   alias-bearing symbol's own sites with its larger projected group, or assert
   that the target group lacks the alias symbol's sites. The fifth private
   test now uses the existing compiler-pinned pattern augmentation/alias case.
   After final Project::load it asserts separate target and augmented canonical
   IDs, each Store symbol's one own origin, asymmetric Index projections of
   one versus two origins, the retained alias and the final public group.

Neither gap was an observed behavior failure. The owner was notified, added
only narrow private assertions, and reran registered validation. This reviewer
made no production edits. Both review items are closed without a second lookup
implementation or a testing API.

## Verified ownership and lookup invariants

`link::link` retains its existing checks and demand schedule. It creates the
lexical projection, runs `check_conflicts`, and only then consumes Linker in
`complete`. Alias conflicts may demand more resolutions after projection;
capturing states at the earlier `Store::freeze` would be premature. The new
publication comment explains this ordering where it is enforced.

`Completed.store` is the original Store, including tables, sparse parent IDs,
modules, request answers and symbol records. `Store::find` still follows the
same parent chain; `symbol` and `interface_members` use it, and
`canonical_symbols` filters roots without densifying IDs. Declaration arrays,
per-meaning targets and table entries retain their original IDs. No second
canonical lookup, remap or checker inventory is introduced.

Store symbol sites and aliases are moved unchanged. `projected_sites` still
builds a temporary one-way lexical declaration projection, subsequently owned
by Index; it does not write those projected origins back into Store. Existing
both-way and one-way Store merges retain their established meaning. Keeping
these owners separate is justified: symbol identity/origins and lexical query
group display are different questions.

The successful transition consumes ProgramFile records after FileLookup has
captured searched path spellings and Navigation has taken its catalogs. The
remaining scope, declaration, reference, alias, request, loader-resolution,
diagnostic and narrow JSON records move into retained File records. The
assertion that navigation has already transferred makes that internal ordering
visible. No ProgramFile clone or partially published Project is added.

Reference attachments survive discarding `Facts.names`. `Builder::add_file`
returns a temporary original-NameId-to-sorted-OccurrenceId permutation. Its
caller supplies every searched name exactly once in original order, before the
existing stable position sort. The file occurrence array is not deduplicated;
the separate public by-symbol view keeps its prior location/role aggregation.
Thus even tied spans retain each collected row's identity. Only OccurrenceId
attachments survive, sharing the existing searched name/span owner.

Unsearched references retain exact decoded text through the existing string
interner and their original raw span. This adds no candidate, by-name or
by-symbol search entries. No lossy conversion or span-based re-selection is
introduced. Other retained records use their existing file-local IDs and do
not contain dangling NameIds after names are dropped.

## Failures and incomplete work

`Demand::NotDemanded` preserves `State::Unvisited`; it does not reinterpret it
as a missing symbol or force unused aliases/dependency references. Completed
Found, GlobalThis, Value, NotReference and Unresolved variants retain their
existing distinctions, including type-only routes. In-progress work at
successful completion is an invariant failure. Existing input checks still
return their errors before any Project is published; no new `.ok()`, fallback
parse or tolerated-error conversion appears in this change.

Builder-only scheduling, cycle/invalidation journals, conflict work, export
memo and reverse value-creation map are dropped. This product supplies no
operation that resumes pending tasks from those discarded structures. Native
semantic lookup must later be owned explicitly; private retained fields do not
by themselves provide that implementation.

The retained product preserves the collector's existing omissions. It is not
the complete semantic name-bearing inventory, flow graph, member recipe or
selection-role replay described by the design. Lexical `NotReference("property")`
does not prove semantic admission or unsupported status. Narrow JSON/value and
library-interface facts do not prove recursive JSON/member evaluation. Native
T/Q3/Q4 execution, semantic provenance, representation selection and both
semantic gates remain open.

## Verification boundary

The five private tests load a final Project and then inspect retained facts,
with public reference queries as compatibility controls. They cover five line
break forms with BOM removal and astral text; searched attachment/resolution
agreement; unsearched escaped names and undemanded work; type-only per-meaning
answers with sparse canonical merge identity; and failure before final alias
conflict publication followed by successful corrected loading. They are
mechanical ownership verification, so no artificial RED was required for a
new external behavior.

This reviewer read the transfer, Store lookup/merge operations, all retained
records, projection construction and Project callers independently. Terminal
registered validation evidence is supplied by the implementation owner below;
source inspection alone is not reported as a test pass. Heap/load measurements
and their limits belong to `types-binding-retention.md`, not this correctness
review.

Registered final full Rust validation
`vrun-5fbf35fd-c245-4fa9-ae89-5168bc201b54` passed. The owner reports that it
executed all five retained tests; this reviewer read those final tests and
independently confirmed the durable terminal status through `cctl validate
status`. Two existing macOS non-UTF-8 filesystem cases are explicitly skipped
because the filesystem rejects those names. The existing ignored library
listing diagnostic is not a retained-binding check. No skipped or ignored case
is counted as a new retention proof.
