# Bounded canonical target and mapper review

The reference/default follow-up read the final count/default operation, Source
Reference recipe and all-interface binder family. No new defect was found in
the admitted TS-only unique-interface family. Count uses complete default
presence without forcing default types; invalid count precedes child demand;
missing arguments substitute all remaining canonical parameters with native
errorType placeholders and fill in order; outer parameters are retained.
`isDeferredTypeReferenceNode` also covers references transitively inside eagerly
resolved type-alias containers. Those surrounding families currently refuse
their pending operations, so this is a required future extension rather than
a demonstrated healthy-result contradiction in this slice. Direct alias
context and structured/union mapper construction remain explicit obligations.

This independent read-only Ousterhout review covers the current private Source
and Evaluator generic target/member/mapper operations. The operating envelope is
one captured Project and one request-local Evaluator, with compiler-pinned final
public controls required for each admitted family. It is not a full generic or
native delivery verdict. Expected owners are TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

The boundary is proportionate: one canonical declared-symbol cache establishes
targets and parameter identities; declared member acquisition, instantiated
member acquisition, and exact mapper questions have distinct caches. Display
names and source origins do not define type identity. Complete structured
member acquisition is shared with ordinary type literals and signatures rather
than being reconstructed by Array or the presenter.

Three concrete failure-boundary issues were reviewed and corrected in the live
stage:

- Source previously reran the binder header lookup and erased every failure as
  Internal. It now consumes the already captured canonical BoundClassOrInterface
  association; missing AST attachments remain genuine invariants, while pending
  member families remain explicitly unsupported.
- Unsupported structured/union mapper work and recursive inventory work
  previously used Internal. They now preserve a located unsupported semantic
  demand through the actual target association. No successful empty inventory
  replaces missing native work.
- Source member acquisition constructed a located DemandError, converted it to
  AnalysisError, and the evaluator converted it back without its exact TypeIssue.
  This prevented the designed partial collection result. Its operation now
  returns DemandError directly; parser/I/O errors retain their whole-operation
  error classification. This is the same provenance boundary already used by
  lazy member reads.

The implementer additionally identified a distinct correctness risk: an
argumentless type reference to a newly admitted generic declared target must not
return the target as if arity/default resolution succeeded. A declared generic
root and a source TypeReference ask different native questions. The planned
whole reference operation must preserve TS2314 recovery, defaults, explicit
arguments and actual mapper identities; canonical TypeParameter references must
continue to reuse their established identity. That operation requires its own
fresh controls and final Project RED/GREEN. This review does not close it.

The early target protocol still needs broader proof. TypeScript
`getDeclaredTypeOfClassOrInterface` publishes its target before outer/local
parameter and this acquisition. The admitted private stage publishes before
local parameter construction and poisons an escaped target if that construction
fails; its global Array/ReadonlyArray classification happens earlier. This
classification uses retained header metadata in the current family. Future
classification/base/context operations must preserve native publication and
barrier order rather than treating this current acyclic header path as a
universal protocol.

Remaining limits are explicit: single own interface declarations with no bases
are the initial inventory family; class, merged, inherited, method/accessor,
signature instantiation and full structured/union mapper operations remain
pending. ContainsThis and nongeneric thisless-interface proofs are separate
native questions, not a test of printed name or property emptiness. The complete
transient-member identity, alias/mapper/recovery schedules, representation cost
and semantic/resource gates remain open. No additional validation was run for
this read-only review; the implementation lanes own their registered final
behavior verdicts.
