# JSON facts retained at the Project boundary

This is a bounded mechanical ownership change researched with the Ousterhout
skill. The envelope is one successfully loaded local Project generation, using
the existing parser/input limits and captured source. It retains the JSON facts
required by the design; it does not implement JSON type evaluation, semantic
member lookup or Q3 canonical relationships.

`binding/json.rs` replaces the existing `JsonModule` implementation in
`binding/mod.rs`. The existing lossless `parser/json.rs` remains the only JSON
reader. The existing linker keeps its narrow `getPropertyOfType` question;
`JsonFacts` owns exact object lookup and primitive/array apparent-interface
selection. No evaluator runs during loading.

Each file owns one arena with a private typed `JsonValueId`. Every value retains
its exact captured-source span and scalar kind or container structure. Empty or
trivia-only text has an Empty root with no span. Array children retain their
source order. Object keys retain every source-ordered occurrence, exact cooked
`JsonString`, raw key span, optional marker, own value ID and final duplicate
value ID. Overwritten object subtrees survive independently of the final child.
Scalar literal payloads remain only in captured source; imported widened types
do not require copied strings, numbers or booleans.

The same parser-owned exact key equality determines duplicate identity. Decoded
`a`/`\u0061` and astral/pair-escaped keys can share a final child while preserving
different raw spans. Unpaired high and low surrogates remain different UTF-16
identities and cannot merge into replacement text. This does not broaden the
existing supported-name conversion policy or add an unrepresentable-name query.

Two representations were considered: keep a second complete parsed JSON tree,
or extend the existing compact value arena. The arena avoids retained scalar
payloads and a second recursive collector while preserving the current linker
operation. Capture uses a breadth-first queue, so extracting the already parsed
tree introduces no additional recursive walk or unchecked narrowing to `u32`.
Allocation failure and parser/input errors keep their existing failure policy.

The old object lookup scanned distinct keys. Scanning all retained occurrences
would worsen duplicate-heavy files, so each object also owns a derived vector
of first-occurrence key indices. Lookup stays O(distinct keys), uses those keys'
already-computed final links, and stores no duplicate name inventory or lookup
map. The temporary exact-key map is dropped after capture. Retention now costs
O(all values + all keys + distinct keys), rather than only reachable final-key
values. No heap/latency measurement or storage-gate conclusion is claimed.

`FileFacts`, retained binding files and JSON `ValueTarget` handles move this one
fact product into the final Project. Linking's synthetic default, named import,
library/interface and unsupported property-reference policies are unchanged.
The typed handle is private and never serialized. The linker does not infer an
array index's type simply because array children now survive.

## Validation and current boundary

This change uses the authorized mechanical-wiring TDD exception: no new public
behavior exists to fail first, and no public fact-inspection API was added for
tests. Registered `tests/reference_json_facts.rs` first passed against the old
owner in `vrun-1623fe4d-355e-4b88-b1b2-53e4217d3f0e`, then passed after the
change in `vrun-54cdc85d-c242-4322-a337-f991490b5bb6` (three tests, zero skips).
It pins present/absent named imports, ordinary lexical groups, existing
unsupported expression properties, navigation beside JSON and syntax failures,
with five line-break forms and leading BOM. The compiler baseline is separately
frozen in `types-json-evidence.md`.
The existing fifteen-test `tests/reference_json.rs` suite also passed in
`vrun-627d0a29-bb2f-4fe8-a77a-8ccf28111131`, with zero skips.

Three private tests in the new owner inspect a successful final `Project::load`,
not constructor outputs. They cover recursive/scalar/empty roots, every array
child, overwritten subtrees, raw spans, decoded/numeric/empty/astral/UTF-16 keys,
optional syntax, final duplicate links and 2,048 flat duplicate array values.
They require the full registered Rust checkpoint: the explicit test-file runner
does not execute private module tests. Full registered checkpoint
`vrun-a7afe484-7a17-4fd9-9f4c-5eb6c8896366` executed and passed all three;
its library phase passed 109 tests with one existing ignored test. That run also
passed both JSON integration files (fifteen existing tests and three new public
controls). Its overall status failed later in a protocol request-limit test;
the private/public JSON results do not imply overall GREEN. The first attempted full
checkpoint, `vrun-14d75a96-238f-4a04-a079-14d4202b98de`, stopped on unrelated
new Semantic error exhaustiveness in the reference-batch example before tests;
it is not private-test or behavior-fix proof.

The protocol failure's cause is unproven: its original assertion omitted the
full response. A diagnostic-only assertion change and coordinated sole-build
rerun of `tests/reference_service.rs` passed in
`vrun-20762bb8-607f-45e5-bc4b-8f0abb0d6e1b`, with zero skips. Concurrent
rebuilds replacing the service binary are a setup hypothesis, not a claimed
product defect/fix. This separate pass does not relabel the failed full run.

Initial public control runs had one nonexistent error-kind variant and one
incorrect ordinary-property expectation; those were test mistakes. The first
production wiring run had an overly broad private constant reexport. None is
reported as valid missing-behavior RED evidence. Those historical failures remain
failed runs; the later full checkpoint below passed the final retained stage.

Future imported JSON types still need option-dependent null/empty-array/array
union construction, recovery/admission provenance and common member recipes.
Token projection, direct JSON Q3 selection, relationships, refresh/resource
gates and complete semantic storage remain open. Recursive fact retention closes
none of those operations by itself.

## Native consumer attachment added later

The first native JSON scalar/object consumer is recorded in
`types-json-native-evidence.md`. To support it, this same owner now exposes
borrowed initializer and final-property views, preserving source order and every
origin, plus each object's child parent-occurrence attachment. That attachment
has no name/equality decision: it points to the original occurrence, whose
existing final-value links determine duplicate origins. Arrays and root values
have no own property origin. Native ambiguity consumes this operation rather
than inferring origins from a public location or searching a second inventory.

Correction (2026-10-12): no product path consumed the parent-origin view. Product
duplicate grouping uses the binder's JSON property symbols and their
Declarations (`NativeInputs::json_node_symbol`). The unused view was removed; the
private final-Project tests now assert each duplicate group through those binder
symbols, so a binder regression that splits duplicate keys fails them.

The three private final-Project tests now also assert exact parent-origin groups
for own/last handles and no root origin. Final full registered Rust checkpoint
`vrun-a425dbbd-6781-4820-8141-909887ea7a5f` passed with two existing macOS
filename skips. The full command is unfiltered `cargo test`, including these
three nonignored private tests with the final parent/path assertions. The
bounded receipt summarizes success without preserving individual test-name
output. The seven public native JSON controls passed in that same checkpoint.
This closes the retained-fact representation assertion delta and compatibility
stage; the remaining semantic/storage/resource gates remain open.
