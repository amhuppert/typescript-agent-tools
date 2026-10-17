# Q4 implementation review

This is an Ousterhout review of the current presenter against the Q4 contract in
[types-design.md](types-design.md#q4-useful-bounded-type-descriptions). The
operating envelope is one request over a captured Project, with request-local
semantic identities and fixed display limits. It introduces no compiler process,
shared cache, or new producer requirement. Production, tests, and fixtures stayed
closed during this review; no validators were run. Findings below distinguish
code evidence from final witnesses that still need execution.

## Preserve canonical failure identity at inferred constraints

`presentation.rs::parameter_constraint_child` handles an inferred constraint
whose descriptor has no explicit constraint demand. Its error branch currently
calls `Issues::record_failure(ParameterConstraint(ty), failure)`, then passes the
returned issue to `failed_edge` with the original `ParameterConstraint(ty)`.
`record_failure` may already have replaced that fallback identity with
`PropertyValue(failure.failed_property)`. `failed_edge` records the fallback a
second time. This loses the canonical question identity, producing duplicate
equal issues and a different failure node for one original property failure.

This contradicts the failure-provenance contract in types-design: dependent
wrappers of one failed property question share one issue and one unsupported
node. The other graph edge path correctly carries both values returned by
`unsupported_failure`; equal source locations or messages are not identity.

The concrete unrun final witness proposed during review is:

```ts
declare const external: string | number;
class C {
  get value() {
    if (typeof external === "string") return external;
    return 0;
  }
}
declare const c: C;
type Box<T extends typeof c.value> = T;
type R<T> = T extends Box<infer /* query */ U> ? U : never;
```

Select the infer declaration `U` directly. Its summary is `U`; a depth-one graph
asks the inferred constraint through `Box`, which must consume the getter's
excluded flow dependency. This avoids conditional-summary acquisition before
the graph path under review. Establish the native demand and actual product
failure first, then require one located `flow_required` issue and one failure
node. Add the shared-property edge and independent-getter variants only after
the original path is witnessed. This input has not been executed in this audit.

The smallest repair reuses `unsupported_failure` at this branch and carries its
returned identity into `failed_edge`. A modestly stronger interface is preferable
for Q4 closure: return one immutable encountered-failure record containing the
canonical identity and issue, and let `failed_edge` accept that record. Summary
and blocked-collection callers can take the issue when they do not allocate a
failure node. The pairing belongs to the existing failure owner; no second
identity cache or new error classification is needed. This prevents the exact
mis-pairing above instead of requiring every new graph caller to remember it.

## Include actual conditional binders in the graph

Both the conditional skeleton and completed branch in `presentation.rs` use
`TypeNodeData::Operator { parameters: Vec::new(), ... }`. The conditional owner
already exposes canonical infer TypeIds through `conditional_parameters`, and
the summary uses them. The missing graph descriptors therefore concern an
admitted type, not an unavailable producer.

The Q4 operator schema includes bound parameter descriptors. Existing
`native-conditional-types/cases.ts` controls `inlineConstrained`,
`inlineTupleConstrained`, and `inlineNested` supply natural witnesses. For
`<T>() => T extends infer U extends string ? U : never`, a depth-two/three
expansion must identify `U` as bound by that conditional and retain its `string`
constraint. The nested case must associate each infer parameter with its actual
conditional scope. `tests/type_conditional_types.rs` currently requests expanded
answers but asserts only the root summary, outcome, and issues; it cannot detect
an empty `parameters` field.

Use the evaluator's canonical descriptors and constraint questions. The
presenter must not rediscover binders by scanning an extends operand's graph or
its printed summary. Specify descriptor/operand edge order in the graph contract
and account for names, defaults, constraints, and field omissions before child
demands. Preserve depth-zero laziness; a descriptor's existence is different from
its constraint value.

## Give collision qualification a presentation owner

The Q4 contract requires colliding labels to be qualified with declaration
locations. `summary` currently emits `alias.display_name` directly, while graph
skeletons copy `alias.provenance` without a document-level qualification step.
The Source spelling operation returns the actual declaration spelling; it
cannot decide collisions within a particular output document.

An admitted witness is two files that each export an object alias named `Same`,
imported under different local names:

```ts
// a.ts
export type Same = { a: string };
// b.ts
export type Same = { b: number };
// main.ts
import { Same as A } from "./a";
import { Same as B } from "./b";
type Both = A | B;
```

The constituent types retain distinct alias identities and origins, but both
node summaries are `Same`. This is separate from semantic raw-alias association
and does not justify changing that cache. The existing
`same_named_mapping_symbols_keep_distinct_graph_nodes` test verifies identity,
not readable qualification; the two ordinary aliases need a dedicated final
test with exact declaration locations.

The existing schema has `summary`, not a separate label. Recommended contract
revision: the top-level `TypeDescription.summary` remains the bounded native
type summary. A graph node's `summary` is its bounded structural label and may
qualify a colliding named alias/reference with its exact declaration location.
Keep `TypeAlias.name` and origins unchanged as identity/provenance fields.
Document this product graph distinction explicitly rather than silently changing
pinned root-summary expectations. Multiple instantiations of one alias are not
distinct declaration-name collisions; qualification compares actual established
symbol identity, never display text or newly allocated numeric symbol IDs.

One presentation-owned label operation should obtain established name,
declaration, and equality facts and decide qualification. Semantic factories and
text rendering must not duplicate this document-specific rule. Qualification
has a budget consequence: discovering a second label can change an earlier
node's summary. The candidate admission must reserve the total delta for both
labels before publishing either update. The current checkpoint only truncates
new nodes/queue entries; it cannot undo mutation of an existing node's summary.
Stage such label changes with the candidate record or extend rollback to cover
them. Do not append qualifiers after the final byte check or use a separate
unbudgeted text-rendering pass. Omitted-edge labels obey the same rule when they
are displayed. Exact identity fields remain whole; an overlong summary token
uses the existing whole-token elision policy.

Required final controls include two same-named declarations, repeated uses of
one declaration, different arguments of one alias, unchanged native root
summary, both graph discovery orders, and qualification under/at/over the compact
byte limit. A collision introduced by a record that is then rolled back must not
leave a stale qualifier or extra byte charge on an earlier node.

## Clarify omitted counts where the schema has none

The prose promises exact omitted counts for established wide lists. Tuple
elements currently emit a stable prefix and `omitted.fields = ["elements"]`,
but the schema has no `omittedElements`. Type arguments and fixed operator
operands are admitted as whole fields, also without count fields. This is a
contract/schema inconsistency, not evidence of faulty byte arithmetic.

Choose the public policy before extending tests: either scope the exact-count
promise to the existing count-bearing collections, or add appropriate count
fields and specify prefix versus whole-field admission. A wide aliased tuple
whose element prefix exhausts bytes is the natural control. Existing
`type_tuple_graph_demand` tests establish deferred-vector laziness at exact
header boundaries; they do not assert omitted prefix cardinality.

## Structures examined without a new correctness finding

The FIFO graph queue assigns IDs at discovery, expands at minimum distance, and
reuses previously admitted identities. Signature children use the shared record
traversal order, and index records visit key before value. Local checkpoints
remove staged nodes and queue suffixes while preserving encountered semantic
issues; final compact serialization supplies an independent upper-bound check.
No additional concrete FIFO, node-cap, or byte-accounting defect was established
in the inspected object/array/union paths.

Existing final tests cover shortest-distance scalar sharing, Array argument /
element / index-value sharing, 1999/2000/2001 potential records, exact compact
byte boundaries, retained failures after display rollback, whole-summary UTF-8
limits, and deferred tuple argument admission. These justify keeping the single
graph owner rather than introducing a second traversal or display cache.

Remaining proof should target the uncovered interfaces: whole signature and
index records at byte boundaries, conditional binder metadata, colliding labels,
and inferred-constraint failure identity. Several presentation regression inputs
still use an unsupported conditional type as a temporary failure witness; now
that conditional evaluation is implemented, preserve their original proof but
replace the maintained failure stimulus with a compiler-proved excluded F
dependency. Do not change a healthy T result back to Unsupported to keep an old
budget test green, and do not derive new byte expectations from current output.


## Implementation decisions after the review

The original inferred-constraint witness reproduces exactly two equal issues
(`vrun-f88853a8-a060-443d-bab3-f10c87ef51c2`); fresh native observations establish
healthy `U`, then `string | 0` on its constraint demand
(`vrun-4afdd563-9ab3-4888-999d-662f755097a3`). The repair pairs canonical failure
identity and issue in one private record accepted by graph failure edges.
Conditional binder graph tests reproduce all three missing metadata cases
(`vrun-a3a1027d-b481-455e-ad06-5f0a9516cd44`). The graph now obtains actual binders
from the conditional owner and admits complete descriptors before child demands.
These production changes are staged, pending combined Rust verification.

For labels, the chosen design differs from the dynamic-collision proposal above:
every named graph label carries its actual declaration location. The top-level
summary remains compiler spelling. Ousterhout review favored this over storing
and rolling back retroactive label changes, especially for already returned
omitted edges. Qualification is one local, budgeted operation for node and omitted
labels. Native prefix elision preserves the complete suffix whenever it fits.
Tests cover both discovery orders, repeated aliases, different instantiations,
ordinary nominal collisions, omitted edges, and the 512-byte qualification limit.

The count policy extends the schema instead of narrowing the promise:
`omittedTypeArguments`, `omittedElements`, `omittedOperands`, and
`omittedParameters` retain established cardinalities. Fixed operand and type
argument vectors admit atomically; tuple elements and bound descriptors admit
stable record prefixes. Counts are part of header reservations and collection
admission deltas, including removal of the count when a collection completes.

## Final targeted evidence after implementation

The inferred-constraint regression passes after its recorded duplicate-issue RED
(`vrun-0cc34b51-d72e-4372-acfc-6429abbf8d1a`), and all three conditional metadata
controls pass in the broader checkpoint. The graph count, qualification and
deferred tuple-boundary suites now pass together
(`vrun-34359f75-ae9d-4b82-ad47-fbce74599c33`). Count controls cover both 3,000
fully admitted entries and a 5,000-entry omitted suffix, with compiler evidence
for both sizes.

All 31 presentation controls pass
(`vrun-812b51dd-9bc8-4542-bc6b-062611f0c75c`). Their maintained failure stimulus
is now a compiler-proved getter dependency with a changed flow read, replacing
conditionals and unchanged flow joins that the implementation now supports.
The exact 262143/262144/262145-byte controls construct expected complete wire
shapes independently and compare the complete admitted JSON. They also retain
encountered issues when a failed node/property record or blocked collection
explanation is rolled back. Shared original getter failures collapse to one
edge; separate getter dependencies retain separate causal identities.
This is targeted Q4 evidence, not a claim that the whole T/Q3/Q4 deliverable or
its remaining signature/index boundary controls are complete.
