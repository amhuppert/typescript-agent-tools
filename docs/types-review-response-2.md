# T, Q3, Q4: response to the second design review

2026-10-10, against implementation base `ea23d8b`. Responds to
[types-design-review-2.md](types-design-review-2.md). The revised
[design](types-design.md) and [evidence](types-design-evidence.md) are the current
proposal. The first review/response remain historical; their readiness verdict
does not certify this revision.

The five high and eleven medium findings identify real design defects. All are
addressed, along with the low-severity refinements. Several suggested remedies
needed correction after checking the pinned compiler. This response changes
contracts and ownership, not production code, and does not claim implementation
conformance or a new full-design approval.

Applied the Ousterhout software design skill to the existing operating envelope:
a local native Rust CLI/library, immutable project snapshots and a serialized
service. The corrections put each compiler decision inside the operation that
owns it; they do not add a generic query engine, concurrent checker or persistent
type database. The review's verification limits remain visible. Three bounded
follow-up investigations checked flow, core semantics, and references/selection
against source and probes, followed by consistency checks of the revision.

## High-severity findings

### H1 — Accepted: shared overload context is not candidate-local

Calls owns the shared call-site mode and once-per-node contextual assignments,
while each candidate retains its inference context and speculative diagnostics.
The design now names `chooseOverload`, contextual function checking and parameter
assignment, and preserves their pass/argument order. A rejected candidate can
establish a callback parameter type used by the winner. Unsupported, invalid
recovery and product resource failures cannot masquerade as inapplicability.

New probes reproduce the string-versus-any, generic winner and failed-overload
cases. A further 13 fresh API sessions show an explicit outermost-first
`getResolvedSignature` prelude stabilizing callback/member queries in tested
forward/reverse/singleton orders. **Diagnostics-first alone did not stabilize
the API checker.** The design therefore defines its fresh-request schedule
explicitly, rather than equating it to a whole-project diagnostics pass. The
whole Q3 schedule and limit histories remain checkpoint-1 proof obligations.

### H2 — Accepted, with a corrected never rule

Evaluation owns function-end reachability, return aggregation, effect signatures
and switch exhaustiveness over Binding's flow graph. Demanded discriminant/effect
types pass ordinary admission. Blockers cannot mean “reachable” or “no effect”.

The proposed blanket statement that declarations/class methods never infer
`never` was too strong: both infer it from explicit `return fail()` when
`fail(): never`. The function-form distinction applies to the empty return
aggregate/trailing-call case. Fresh strict and non-strict-null probes confirm
that difference. The design requires fixtures for both forms, exhaustiveness,
infinite loops, effect signatures and null-dependent implicit returns.

### H3 — Accepted: one flow constructor and one restricted interpreter

Binding owns the compiler graph; Semantic source exposes handles. Evaluation
owns compiler read setup, graph interpretation, admission and predicate
short-circuits. The region/syntax screen is removed. Supported identity cases
can return initial or declared types as the actual operation requires.
Constraint positions, default parameters, optional roots, captures, destructured
pseudo-references and aliased conditions are separate compiler questions at that
owner. Initializer assignment refinement uses the declared **union** test,
not whether an annotation exists.

The suggested parameter-category/no-textual-reference shortcut is not adopted.
Fresh probes show `{}` and unconstrained `T` can acquire predicates, including
through local aliases. Use the compiler's eligibility short-circuits and the
same restricted flow interpreter to prove no refinement, or return
`flow_required`. Ordinary object filter callbacks have positive controls.
F extends this interpreter with narrowing; it does not introduce another graph.
Predicate-heavy and JSX query refusal costs must be measured separately.

### H4 — Accepted, with operation-specific outcomes and a finite-budget caveat

Compiler limits, reset points and cache protocols now belong to their named
operations. Error-producing instantiation/conditional/union/subtype limits
propagate invalid dependencies. Relation overflow can produce False and reject
one overload; recursive assumptions can produce Maybe. Neither is automatically
a whole-query failure. The former blanket recursion and resource rules are gone.

Product work/memory/depth/deadline limits remain distinct hard failures. The
suggestion that one finite budget exceed *all* compiler work is impossible for
arbitrarily many individually valid operations. Instead it must admit isolated
compiler-boundary fixtures and the measured corpus; larger queries fail
explicitly. The evaluation-order baseline is specified because those counters
and caches can depend on history. Just-under/at/over tests assert final outcomes.

### H5 — Accepted: remove wildcard computed candidates; name completeness

Binding captures one token/anchor inventory. Reference search owns the compiler
candidate-syntax/meaning rule and consumes shared exact member keys. Arbitrary
`[key]` sites are not wildcard candidates or speculative gaps. Computed
declarations can remain origin metadata without making their variable tokens
member occurrences.

The design retains exact decoded matching as an explicit product normalization
of 7.0.2's raw-text search, with escaped/empty/numeric spelling fixtures.
Coverage declares `lexical` or `member`, each with a defined baseline. The member
baseline also includes combined identity and the fresh-request checking schedule.
Lexical queries and their call filters remain binding/syntax-only. Proposed
differences are recorded separately in [references.md](references.md#proposed-tq3q4-differences).

## Medium-severity findings

| Finding | Disposition and change | Required proof |
| --- | --- | --- |
| **M1 — Reference relationships** | Accepted. Type operations owns `GetRootSymbols`, mapped origins and special spread links; Reference search includes inline union seed substitution before related-symbol search. Same-key JSX attributes become `jsx_typing_required` gaps; member tag names resolve normally. | Partial/Readonly/Pick/Omit, ordinary/special spreads, inline/interface union controls, props query and tag controls, each seeded at declarations and uses. |
| **M2 — Early/late query-order artifact** | Accepted as explicit combined-member normalization. Early views remain internal to permitted re-entry; public Q3 uses the resolved combined member. Warming a use first is not the oracle contract. Computed-only established uses may differ from cold LS results. | Cold/warm/reversed/cross-file seeds, raw observations plus independently justified root normalization, direct/service equality. |
| **M3 — Alias identity and ordering** | Accepted. Alias attachment, alias arguments and union origins enter type construction/identity. Compiler union order and intersection insertion order are preserved. Presenter uses `getPropertiesOfType`'s operation-specific sequence, not a blanket final `sortSymbols` or alphabetical sort. | Distinct aliases of equal structures, union reduction/origins, own/inherited members, overload and intersection order. |
| **M4 — Invalidity** | Accepted. Evaluation propagates actual recovery data dependencies. A failed final `satisfies` validation does not poison the unchanged expression type. Its contextual target can still be a required/invalid dependency. | Valid expression under a rejected relation, invalid contextual target, overload recovery, genuine any, unrelated diagnostics. |
| **M5 — Selection ownership/anchors** | Accepted. One Target selection owner maps node+role to typed question; parameter-property modifiers and actual namespace segments supply round-trip anchors. The anchor table covers enum, namespace, aliases, accessors, type syntax, calls/new/tags and explicit unsupported catalog forms. Alias normalization is recorded as a product choice. | Every declaration/alias spelling, renamed sides, both type-only forms, dual-role and merged meanings, overload deduplication and replay through `--at`. |
| **M6 — Q3 results** | Accepted. Top-level complete/partial outcome, typed Unselected not-found/ambiguous answers, catalog `unsearchedMembers`, and symbol/question collapse before ambiguity. Delivery explicitly distinguishes partial results from stderr failures. References retain their JSON result form. | Empty/partial/unselected/error documents, legacy lexical result compatibility, member-only bare names, exact exit/stdout/stderr/`-o` behavior. |
| **M7 — Q4 schema** | Accepted. Read/write types, exact key kinds, predicate/this/optional/rest/variadic signatures, type-argument/array edges, bounded summaries and wide-list counts are specified. Breadth-first minimum-depth expansion preserves sharing. Unsupported child/collection demands produce typed partial results; invalid/root/resource failures remain errors. | Primitive depth zero, object elision, wide JSON-derived types, alias/shared-depth graphs, distinct accessor writes, unsupported leaves/collections and independent semantic/display budgets. |
| **M8 — Representation/migration** | Accepted. Semantic source hides term production; compare eager terms with request-local reprojection of captured text. On-demand is the starting candidate, following existing Q2/editing. No disk reparse or second binder. Checkpoint 2 closes representation and each old binding projection's replacement. | Old-operation load/refresh and peak/retained memory, first/repeated semantic cost, source-site round-trip, removed duplicate facts and S/C/E compatibility. |
| **M9 — JSON facts/owner** | Accepted for module/property typing, with a profile clarification. JsonFacts owns kinds, arrays, exact repeated key spans and last-value links, feeding linker/member/token projections. Correct existing name is `value_property`; its narrow lookup explicitly distinguishes present/absent/deferred and failures. Q4 positions inside JSON are excluded; imported types and Q3 keys remain supported. | Strict-null states, empty/mixed arrays, repeated/escaped keys, all key origins and last-value type, old binding behavior, rejected JSON Q4 positions. |
| **M10 — Shared-service failures** | Accepted with realistic containment. Named semantic worker, checked recursion/measured stack, typed unwinding-panic failure, distinct resources, deterministic budgets and proposed 30-second cooperative deadline. Catching panics is not stack-overflow/process-abort isolation. | Stack/budget/deadline boundaries, teardown/unwinding/join cost, post-panic lexical success, lexical latency behind one semantic request, direct/service outcomes. |
| **M11 — Core owners and protocols** | Accepted. Value-symbol typing belongs to Evaluation; declared/apparent/lookup operations to Type operations. Diagram shows semantic back-edges. Member, signature, type-resolution and relation re-entry protocols are named separately. Constant evaluation and effective options have owners. | Cycles/re-entry, constant and enum forms, compiler strict-unset defaults, each override state, diagnostic-only options and captured module format. |

The JSON qualification matters: compiler module/property types widen scalar
literals, but direct JSON value positions can retain literal types, and an early
duplicate key's type can differ from the imported property's last value. The
revised profile excludes those Q4 position questions instead of asserting that
TypeScript never uses JSON scalar payloads.

## Low-severity refinements

| Review item | Resolution |
| --- | --- |
| Filters change group sets (refs-8) | Stated explicitly: filtered-only related groups can disappear; the seed's metadata and surviving non-test identities do not change. |
| `--calls` owner (refs-9/owners-8) | Extend the syntax owner with separate outer-expression/call-target operations; do not broaden existing parentheses-only callers. Exclude implicit decorators/JSX/instanceof; include explicit `@factory()` calls. |
| Vocabulary (contract-8) | Closed outcome, question, reason, semantic error and node-kind sets; unknown deferred syntax uses a named generic reason with a source explanation. |
| Conflicting CLI rules (contract-9) | Explicit format/delivery/exit table replaces the vague “existing project-query rules” reference. Partial text type results require an explicit result-versus-diagnostic distinction. |
| Profile omissions (checker-8) | Add variadic tuples and named hooks. The six intrinsic aliases include BuiltinIteratorReturn; Awaited is a separate global alias hook, not an intrinsic-keyword alias. |
| Real-project reproducibility (coherence-11) | Pin clean Command Center `92fe6ab06fbea2c4b6a305689f8cf336173eef81` and tsconfig, use observed SpecWorkflowCleanupPort/factory/props/filter targets, require a hashed manifest and successful journeys. RunHost is only illustrative. |
| Determinism (coherence-12/13, errors-9) | Fix semantic/order/presentation definitions, gap/origin/candidate ordering, cache lifetime and direct/service/reversed-request checks. Do not claim every warmed compiler history gives one answer. |

Other refinements are incorporated with their parent finding: overload/merged
sites under M5/M6; graph depth-zero/key/units under M7; semantic resource naming
under H4/M10; call effects under H3. The structures the review found sound remain:
deep Project operations, directed relationships, immutable lexical identities,
request-local state and the coverage ledger.

## Evidence and handoff limits

New evidence consists of the flow configurations, core option/overload cases,
and 13 fresh prelude sessions described in [types-design-evidence.md](types-design-evidence.md).
Existing review probes were inspected where rerunning them would add no new
information. Source-backed corrections do not turn unmeasured resource or
coverage claims into measurements. The review's reported Command Center syntax
counts were not independently recomputed.

No runtime behavior changed, so no production red-green cycle or full Rust suite
was run for this response. Document checks cover JSON examples, local links,
format/whitespace and consistency. Implementation remains gated by a reproducible
whole-query compiler oracle, measured representation and execution choices,
behavior-level tests and useful real-project journeys. Those obligations are
concrete exit criteria, not evidence already obtained.

Revision fingerprints (SHA-256; after the response edits):

| Artifact | Hash |
| --- | --- |
| `docs/types-design-review-2.md` | `aa274a8b4afc359776994b81909939cee36c994857b4275a363dbc64534495bc` |
| `docs/types-design.md` | `7f27f3c1678c217273292519d3892914a0b11605efc3d0786a4fc354fa6f9b0e` |
| `docs/types-design-evidence.md` | `95a9c09378d65cc714b655aec35b1e987d1bfb9f77d614c226d1704356baa68c` |
| `docs/references.md` | `33956b1eaa75eebac99d2162cb0e037b830e397341e306a7ec1990a0909f3d08` |
