# Native project navigation: response to round-one review

Date: 2026-10-07. Scope: proposed S, C, Q1, Q2 deliverable on
`typescript-agent-tools#1`.

Source: [round-one review](../memory-bank/collaboration/473774f6-0d7e-4bbc-8bc7-997824927fe6/round-1/agent_one/final_answer/answer.md).
Revised artifacts: [technical design](navigation-design.md) and
[implementation plan](navigation-plan.md).

The findings are accepted as improvements to the proposal. The core architecture
remains one immutable project snapshot and refreshable session, a syntactic
catalog separate from references, shared source/position ownership, and typed
failures. The revisions strengthen internal decision ownership and observable
contracts without prescribing a type or file for every responsibility. No
production implementation or compatibility fix is claimed by this response.

## Finding-by-finding response

| # | Response and change | Evidence still required during implementation |
| --- | --- | --- |
| 1 | **Accepted.** The [sequence](navigation-plan.md#sequence-and-dependencies) repeats the unchanged Command Center load after each of 3a, 3b, and 3c. C completes only with a full load and representative ordinary, library, JSON, and augmentation reference checks. Step 6 adds integrated navigation, freshness, and performance evidence. | Turn each newly exposed blocker into a maintained compiler-backed reproduction; do not assume the original three exhaust the graph. |
| 2 | **Accepted.** [Show outcomes and delivery](navigation-design.md#show-outcomes-and-delivery-channels) now define found, ambiguous, and missing results, candidate fields and anchors, exits, text/JSON channels, and output-file behavior. References retain their empty-success contract. Payload placement in an outcome or typed error remains an implementation choice. | Consume ambiguity JSON and successfully select show using a returned file/anchor; cover the delivery matrix through the CLI. |
| 3 | **Accepted.** [Selection](navigation-design.md#declaration-selection) separates optional exact lookup keys from labels. Unrepresentable names retain their sites, spans, and position selection without adding a project-load failure. Labels never become substitute identities. | Distinct lone surrogates, a valid pair, and literal U+FFFD remain distinguishable; preserve the existing successful reference-load regression. |
| 4 | **Accepted.** [Complete operations](navigation-design.md#complete-operations-and-snapshot-ownership) own the deep parsing budget and large-stack thread for all navigation parsing, including any query-time reparse. Display depth is independent; existing parse-mode defaults stay intact. | The budget-distinguishing input works through both navigation modes; limit tests include boundary cases and valid shallow final trees. |
| 5 | **Accepted; simplified.** [Library provisioning](navigation-design.md#standard-libraries) compares plain vendored files, an installation scheme, and embedding. Choose vendored checkout data with real, readable paths. There is no installed/relocatable distribution or caller-selected data root in this delivery. Existing build identity and observed reads suffice. | Verify bundled and replacement-library selection, readable returned locations, missing/unreadable failures, and freshness. No relocation test is required for a promise the proposal no longer makes. |
| 6 | **Accepted with an explicit baseline.** [Outline policy](navigation-design.md#outline-baseline-and-deliberate-differences) compares against native 7.0.2 document symbols/navigation rules while defining a syntax-site catalog. It records callback, shorthand, variable-statement, overload, depth, and position-only-name policies and their reasons. | Add a bounded test-only native protocol probe; confirm the baseline and record each actual departure. The existing sync semantic oracle does not cover navigation. |
| 7 | **Accepted; decision deferred to evidence.** [Snapshot ownership](navigation-design.md#complete-operations-and-snapshot-ownership) and [step 4](navigation-plan.md#4-integrate-project-navigation-with-the-existing-snapshot) compare eager records with a compact selection index plus projection from retained text. Both use the same projection and parsing policy, with no cross-generation reads. | Measure retained bytes, build time, small/large-file latency, peak query memory, and repeated service requests before choosing. Keep one strategy; prefer simplicity if the measured tradeoff is small. |
| 8 | **Accepted, with native-source confirmation pending.** [JSON ownership](navigation-design.md#json-modules) extends the shared reader losslessly: full spans, exact UTF-16 keys, and repeated properties precede config conversion or module projection. Manifest syntax stays separate. The proposed independent source-module dialect was removed. | Confirm native 7.0.2 entry points and narrower rules before implementation. JavaScript compiler source and the limited 7.0.2 module probes support the direction but do not establish full native grammar parity. |
| 9 | **Accepted.** [Session ownership](navigation-design.md#complete-operations-and-snapshot-ownership) and [step 5](navigation-plan.md#5-finish-command-and-service-integration) require one execution boundary that enforces refresh, not three copied refresh calls. Immutable library snapshots remain captured values. One CLI policy owns channels and exits. | Exercise outline, show, and references via direct and service entry, with changed input, failed refresh, and repair; no old generation may answer after failure. |
| 10 | **Accepted.** [Request validity](navigation-design.md#request-validity) has an operation/option matrix enforced once when producing an executable request. Navigation uses `--direct` for project queries; the existing reference spelling remains supported. | Test representative valid/invalid combinations, including file/project requirements, selectors, depth, JSON/pretty, and direct mode. |
| 11 | **Accepted.** [Step 1](navigation-plan.md#1-establish-evidence-and-settle-the-narrow-contracts) adds property initializers, class arrows, anonymous containment, owned kind mappings, multi-binding depth, overloads, exact-name variants, parsing-budget parity, and structured ambiguity followed by show. | Public-operation examples plus compiler comparisons establish these policies before the implementation spreads them across callers. |
| 12 | **Accepted.** [Augmentation](navigation-design.md#module-augmentation) names `mergeModuleAugmentation` and `mergeSymbol`. The suspected alias conflict is explicitly a hypothesis; the touched owner must resolve canonical identity and preserve table/cache invariants, not suppress a diagnostic. | Run the minimal Rust reproduction and pinned semantic expectations before deciding the fix; cover original, aliased, augmented, and consumer selections and real conflicts. |
| 13 | **Accepted; rename at integration.** [Complete operations](navigation-design.md#complete-operations-and-snapshot-ownership) and step 4 choose `Project` and `ProjectSession`, updating repository callers without aliases. This fits the unpublished crate and lack of known external API consumers. Existing CLI names and reference JSON stay stable. | Update all in-repository callers/docs and verify unchanged reference behavior. Exact module placement remains an implementation choice. |

## Consequential choices

Overloads remain separate syntax sites. This adds a position-selection step for
an overloaded name, but keeps exact excerpts and selection rules coherent rather
than introducing a second grouping model. The structured ambiguity contract
makes that step usable by an agent.

Library data stays on disk in the build checkout. The local pinned package is
small (about 3.8 MB); preserving inspectable declaration locations and reusing
ordinary capture/observation matter more here than eliminating those reads.
Installation and embedding mechanisms have no established consumer in scope.

The catalog uses compiler navigation as a comparison baseline, not as a promise
of exact editor-tree parity. Its public purpose is syntax-site selection. Native
protocol evidence must distinguish compiler-derived rules from deliberate
product policies before fixtures are finalized.

The storage strategy remains deliberately undecided until the bounded comparison
in step 4. Neither eager records nor selected-file projection has earned a claim
of lower total complexity and cost on this project's workloads. This is an
implementation decision with criteria, not a requirement to ship two mechanisms.

## Validation and estimate

This revision changes documentation only. It cross-checks the review against the
current loader, session, CLI, JSON reader, binder, regression tests, and existing
compiler probes; it does not report a new Rust fix, full application load, or
performance result. Native 7.0.2 JSON source confirmation and the document-symbol
probe remain explicit step-1 obligations. No Rust tests were run for these
documentation-only changes. Local links/anchors, whitespace, final newlines,
and code fences were checked across all three documents.

Keep the original **20–39 engineer-day** estimate as preliminary. Removing
speculative provisioning work and adding native navigation evidence change the
work mix; they do not support a precise revised number without the early probes.
Re-estimate from those probes and repeated unchanged-project checks, and revisit
it if the storage comparison reveals material work. All other wishlist items
remain proposals outside this selected deliverable.
