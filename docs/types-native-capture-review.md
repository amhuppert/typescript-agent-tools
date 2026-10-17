# Assignment marking and capture review

This Ousterhout review assumes an accepted immutable Project, retained
canonical bindings and one request-local evaluator. It reviews
[types-native-capture-plan.md](types-native-capture-plan.md) and the pinned
checker questions. The later source read includes `source/capture.rs`,
`evaluation/reads/capture.rs`, the Identifier setup and Start handling in
`evaluation/reads.rs`, `Graph::capture_extension`, and the linker's retained
raw local-export result. It does not claim a final production GREEN.

The selected lazy checker operation avoids a second name/assignment binder.
Source should project whole ordered syntax recipes using existing final
attachments; canonical reference identity remains with the completed Store.
Native ancestor-root caches, last-assignment sentinels and definite-assignment
facts are different decisions from graph container capture extension. Keeping
them within the complete reader operations avoids callers reconstructing those
choices from source order or variable spelling.

The exact capture boundary matters: function expressions, arrows and methods
or accessors on object/class expressions can extend their container, while a
class-declaration method retains its separate boundary. A Start tail extension
must keep the same invocation depth and Shared scope, rather than becoming a
recursive child. Raw local exports are declaration identity questions; following
an imported alias's runtime target would mark a different symbol. The existing
linker's raw result and invalidation are the appropriate owner.

The proposed scalar assignment-reduction operation is also necessary before
final identity admission. Returning declared for every matching union assignment
would create a false successful read; rejecting every intermediate difference
would lose healthy restored joins. Native ordered assignability, fresh Boolean
handling and final fallback belong in that complete operation and its actual
type-pair cache. Structured/pattern dependencies must remain errors while their
mandatory branches are implemented.

The native two-test GREEN `vrun-941598e6-04b7-4c0e-9d95-6478aa5ea6ed` and
six-test final Project RED `vrun-3772580f-bd50-4e1c-a1c4-08db992a9e98`
justify this increment. No additional plan defect is supported by the bounded
read. Source-backed implementation review, exact nested/export/sentinel
variants, scoped compatibility and representation/resource measurements remain
required before claiming the full owner complete.

The bounded implementation read found no additional incorrect successful
answer in the current scalar capture family. The candidate walk preserves
the native Identifier/ExportSpecifier early returns, whole type/enum/interface
exclusions and child order. Its assignment-position operation compares the
actual nearest function/source ancestors and extends raw Pos through the
specified statement Ends. `Outer` retains the native never-past sentinel
independently of the definite-assignment bit. Raw local exports consume the
linker's captured identity rather than following the alias value target.

The request-local root cache installs Marking before the walk and records
Failed separately from Ready. A failed ancestor is checked before reusing
partial symbol facts, preserving the original failure instead of converting
an incomplete walk to an already-marked success. The graph extension uses
actual function and parent kinds; its Start continuation remains a tail step
in the same invocation. These boundaries keep syntax, binding and type-read
decisions within their respective complete operations.

This source review does not prove the raw `Value` local-export question for
every legal merged/type-only alias shape merely from the existing lexical
export lookup, nor the full set of assignment-position statement variants,
nested marked-root reuse, line-break/BOM positions or resource costs. Those
need source-derived final controls. Assignment reduction is being implemented
in a separate coupled stage; until its final consumer passes, this review
cannot establish healthy restored union joins. Structured/pattern and other
explicit pending dependencies remain mandatory work, not accepted scope
reductions.

Native evidence was later archived at
`fixtures/types/native-capture-flow/evidence/`. Its new 268 observations
separate 66 ordinary demands, eight post-demand diagnostic controls, 54
boundary demands and 140 text-variant demands. The two source/hash-frozen
registered replays passed with zero skips:
`vrun-3ae4586b-04c6-432d-93db-652d2e4082f7` and
`vrun-98a39d74-dfa2-4363-b123-1355e4d27fb8`. Fixture/tool/procedure bytes,
all 416 installed API-file hashes, native binary and three pinned source
owners stayed unchanged. The archive explicitly leaves abbreviated historical
metadata provenance and per-request virtual-metadata capture unproved. These
receipts strengthen native controls, not the still-pending production GREEN.
