# Retained raw identifier provenance

This bounded ownership increment extends the existing lexical lookup decision.
It preserves the completed binding result and directed lexical reference groups;
it neither performs a second lookup nor forces an unvisited dependency. The
resource/representation gate remains open, including the cost of new retained
attachments.

`probe_entry` now returns the unchanged resolution together with its actual
value origin. A direct local value slot keeps its canonical direct symbol; an
eligible raw alias keeps its actual AliasRef identity even when the legacy
runtime projection cannot supply its target. Lexical lookup retains the first
eligible raw value identity independently of that projection. Ordinary
module/export probes project the unchanged resolution. `try_ref` attaches that origin
to its existing reference state. Invalidation clears it alongside the same
resolution, and successful completion moves it into retained Reference facts.
The temporary parallel vector is consumed at file retention; no second searched
name inventory survives.

`Bindings::value_reference` is the complete read acquisition operation: exact
raw lexical span, established canonical BoundValue, raw ReadOrigin and captured
flow Read. It distinguishes NotDemanded, unresolved target, missing flow and
other native symbol-family obligations. Alias identity is an opaque typed
handle, distinct for two imports of the same target. Its own `alias_flow` getter
returns the alias's actual file and captured native flow attachment, independently
of target declarations and display origins. A missing attachment remains an
explicit obligation; it is not reconstructed by source spelling or file ties.

Reference comparison asks a different native question from type acquisition.
The separate `reference_symbol` operation obtains raw identity without requiring
a value target or flow attachment. A completed unresolved lexical reference
with no eligible origin supplies the genuine native unknown-symbol sentinel;
the reader compares that sentinel as unequal to its healthy direct symbol.
An eligible alias retains its raw identity when its target fails. Actual builtin
symbols remain explicit pending obligations. `value_reference` follows the
eligible raw alias's completed VALUE target for its actual native type question.
That semantic target may differ from the legacy runtime lexical projection:
type-only access emits TS1361/TS1362 but can still have a healthy native type.
The actual alias target wins over a missing or same-named outer projected value;
unvisited and failed targets remain failures. The final paired runtime/type-query
controls and their behavior RED/GREEN are recorded in
the type-query implementation plan (archived in Git history).

The existing signature nine-test compatibility checkpoint passed with zero
skips in `vrun-2dae1ca4-3836-4849-9761-90e73856e1b2`; parent reader controls
consume direct provenance. This ownership/scaffolding move has no independent
public behavior change and did not manufacture a RED test. The native unknown
comparison then gained its own behavior proof: compiler seven-test GREEN
`vrun-b0a36f10-e549-46c5-b7b5-921c5698e520`, final eight-test RED
`vrun-ac55a708-3586-44f6-b2c5-aa3ee19bc73f` (seven pass, missing comparison
fails), and the established eight controls all passed inside twelve-test RED
`vrun-d7748e6b-8a68-4e1c-bad9-da00bf97013d` (four new assignment cases
failed). The complete twelve-test reader file subsequently passed with zero
skips in `vrun-78012907-ce23-41d6-9e28-f39272245edd`.

The independent Ousterhout audit found no concrete defect in the paired probe,
retry/invalidation, or distinct raw/value questions. Alias read setup, complete
failed-target/cycle/reset variants, and builtin symbol families still require
their own final reader controls; this unknown-condition proof does not close
those obligations or the retained-cost gate.

The subsequent alias read lane has its own fresh compiler twelve-test GREEN
`vrun-1d622ed6-f568-4fb8-ad53-39ac8a145875` and final fifteen-test RED
`vrun-8b9c0bbf-8784-4ebb-8f7c-f599c29da477` (fourteen pass, first alias use
fails on missing native own-container setup, zero skips). Only after that RED
did Collector attach Alias.flow through the existing capture owner and the
binder expose `alias_flow`. The reader owns actual container/start and
initialization decisions; the terminal complete-file receipt remains required
when citing that alias family as implemented. The separate type-query controls
above prove the eligible type-only alias target question rather than every
alias cycle/reset/builtin operation.
