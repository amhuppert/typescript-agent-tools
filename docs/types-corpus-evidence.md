# Command Center member journey observations

Exploratory TypeScript 7.0.2 observations, 2026-10-10. These use the design's
fresh-checker prelude and the single private compiler hook. They establish
compiler expectations on the recorded corpus; they do not establish native Rust
admission, normalized decoded Q3 result groups, gap classification, service
performance, or completed T/Q3/Q4 delivery.

The corpus is revision `92fe6ab06fbea2c4b6a305689f8cf336173eef81` with a clean
tracked tree. All 7,639 manifest inputs, configuration and listed dependency
locks matched their recorded hashes before and after the capture. That check
does not make multiple disk reads transactional. The full input manifest hash
is `53cdf23518d299cba7b6540985cd76538147da0cd5016b05098ff577f3e42f1f`.
The recorded sources include the six generated files and installed libraries;
no project files were modified.

The per-request replay captured 7,967 compiler-read paths. Of these, 327 project
metadata files fall outside the historical source/configuration/lock verifier,
mostly installed dependency `package.json` files and `.next/package.json`.
The stored answers retain their paths, not content hashes; their bytes were not
verified unchanged across requests. These three historical rows are exploratory
per-request replay observations, not a complete fixed-input proof. Future
captures must record and compare hashes of every consumed input through the
existing read-capture owner. Current disk bytes cannot supply historical hashes.

## Observed member answers

Each row is one fresh request. Candidate counts include decoded lexical names
that the related-symbol owner may prove unrelated. Counts below are raw compiler
observations, not the proposed product coverage ledger.

| Seed | Decoded candidate demands | Raw groups / occurrences | Native related candidate observations |
| --- | --- | --- | --- |
| `SpecWorkflowCleanupPort.observe`, execution-service.ts:236:3 | 119 | 1 / 7 | 7 |
| Contextual implementation, workflow-cleanup-port.ts:24:5 | 119 | 1 / 7 | 7 |
| `CollapsibleTextProps.maxCollapsedHeight`, CollapsibleText.tsx:7:3 | 12 | 2 / 9 | 9 |

Both cleanup seeds produce the same group with the interface method origin.
Its occurrences include the interface declaration, contextual implementation,
three explicit calls in execution-service.ts:1410:42,
route-handlers.ts:472:49 and service-factory.ts:592:57, the object mock in
spine-test-fixture.ts:780:13, and a read in bundle-production.ts:253:59.
The mock's filename is deliberately not a `--no-tests` match under the exact
product path rule. These are concrete cross-file compiler expectations, not a
spelling-based approximation or proof of a native member query.

The JSX seed has four component attribute occurrences: WorkflowEventLog.tsx:891,
TasksSection.tsx:309, and ValidationRounds.tsx:172 and :362. They require the
profile's deferred JSX typing operation and must become located
`jsx_typing_required` gaps when that operation is demanded. Three storybook
object assignments form another compiler group; the remaining occurrences are
the interface declaration and the component binding-property name. Other local
reads of the same spelling are candidates with different lexical identity.
No native admission/gap count for these dependencies has been measured yet.

The storybook group repeats one exact PropertySignature origin four times.
That raw result is preserved. Sorting locations is not complete product
normalization: canonical member identities, source roles and declaration-set
projection must settle repeated synthetic origins and groups. Origin-set equality
alone is insufficient, as the parameter-property local/member controls elsewhere
share one declaration span but denote distinct symbols.

## Capture and limits

Raw results are retained in
[member-observations.json](../fixtures/types/command-center/member-observations.json),
including candidate ledgers, native meaning/validity/relatedness, roots, groups,
call-role observations and before/after-demand counters. Exact historical shared
oracle/hook code and hashes are under
[member-relations/captured-tooling](../fixtures/types/member-relations/captured-tooling/README.md),
with [provenance](../fixtures/types/member-relations/provenance.json).
The reusable data-capture wiring is
[member-probe.mjs](../fixtures/types/command-center/member-probe.mjs); it takes
`CORPUS OUTPUT` and leaves failure visible. The historical run used the preserved
scratch script and completed all three rows with all input hashes still matching.
Later oracle improvements do not retroactively change those raw observations.

Future captures use schema 2 and the hook's per-request `inputHashes`, including
configuration ancestors, package manifests and raw BOM bytes. The final capture
owner verifies the complete hash/path inventory, consistency of repeated inputs
between requests, and every disk input's current bytes before publishing
`captureComplete:true`. It requires a fresh schema-2 unfinished artifact,
observed answers and a bijection with the artifact's exact expected requests;
each answer's input identities are unique. Completed artifacts are immutable
historical records and cannot be finalized again. The member caller requires
exactly one of each of its three seeds and writes the new unfinished artifact
before the first demand. Missing/changed inputs fail and leave the unfinished
artifact. Virtual-only inputs require their own pinned replay path and currently
fail this disk-capture owner rather than being silently omitted. Compiler and
library identity remain separate obligations; before/after disk checks still do
not provide transactional capture. The artifact-level regression failed four
assertions in `vrun-250977bd-4deb-4f10-8527-94c9d9c36fcd`, then passed four tests
with no skips in `vrun-d6304deb-cc94-4501-a958-0d3ae0ffa87a`.
The capture review (archived in Git history) exposed malformed inventory,
lifecycle and caller seed gaps. Four additional artifact/caller tests reproduced
them in `vrun-cf3b0b10-60e1-4c1f-8951-8ff8e4a6dcc2`; all eight tests then passed
without skips in `vrun-6e043def-115f-487f-8610-2c4b973dce63`. Input completeness
still depends on the reviewed read-capture producer: the finalizer cannot
reconstruct a file omitted from both of an answer's inventories.

No semantic diagnostics pass warmed these requests. No observed instantiation,
conditional-tail, union-size or subtype guard failure occurred in these three
histories. Historical instrumentation did not observe relation depth/allowance
overflow or circular recovery, so this is not general recovery causality proof.
The raw compiler search still searches its own program scope;
this capture does not prove the proposed product's complete eligible-site
accounting. Timings in the raw file are single exploratory samples under other
workload, including API startup, capture, Go compilation and replay. They are
not native request latencies or resource calibration.

The earlier five fresh root type observations remain in
[api-observations.json](../fixtures/types/command-center/api-observations.json).
They include the inferred factory and predicate/callback anchor. Native useful
journeys, first/repeated/reversed orders, type graphs, exact refusals,
load/refresh/heap costs and queued-lexical guarantees remain required.


## Current input drift (2026-10-12)

Read-only verification against all 7,639 recorded file hashes found eight
changed inputs in the live Command Center checkout, now revision
`aa484f422f00dfb64967d2008fac3c4f9a92538e`. Six are project files (including
the generated model catalog and generated build-info source); two are generated
Next route/validator files. All recorded dependency and library file hashes
still match. Four tracked source changes separate this revision from the pinned
revision; the live checkout also has an unrelated modified generated model
catalog. It was neither changed nor reverted by this implementation work.

The exact drift inventory is `.cc/temp/stage65-corpus-drift.json`. Five changed
tracked files are recoverable with matching hashes from the old revision; the
historical representation spike also retains the old generated validator bytes.
The old routes declaration and build-info bytes are not recovered by that
inventory. Consequently, future captures must either reconstruct and verify
every historical input, or record an explicit new isolated revision/generated
input baseline and regenerate the compiler answers as the design permits.
Current bytes must not be presented as historical evidence. The earlier
exploratory artifacts and their hashes remain unchanged.


## Isolated controlled baseline and complete native captures (2026-10-12)

The current evidence baseline is
[`aa484f4-2026-10-12/single-threaded-native`](../fixtures/types/command-center/aa484f4-2026-10-12/single-threaded-native/README.md).
It uses the independent clean snapshot with copied/hash-verified dependencies
and generated inputs. Its explicit config extends the pristine original config
with `singleThreaded:true`. Parallel native parsing varied default-library
external provenance on unchanged inputs; this profile preserves actual native
flags and order instead of folding them. Original parallel artifacts remain
unfinished historical evidence. The capture review (archived in Git history)
records source evidence, RED/GREEN runs and limitations.

Registered freeze `vrun-85e920b5-4e96-43e0-a8f8-989faf646f33` passed two fresh
discoveries of all 7,639 sources. Fresh Q3 member capture
`vrun-cf1abce4-3435-458b-af90-8a03dd47c9e6` and all five Q4 roots
`vrun-0745e72c-9f3c-4c4b-90a2-b24f142cfe08` passed, with zero skips.
Both raw query artifacts have legitimate `captureComplete:true` markers after
checking exact requests, actual native replay order/provenance/decoded hashes,
independent API inventory/options and every consumed raw input hash. Each query
contains 7,639 native program sources and 7,968 raw input hashes. Actual native
program evidence is retained separately from API input, and raw BOM bytes remain
separate from decoded compiler text hashes. Exact toolkits and artifact hashes
are preserved; temporary external-corpus test prerequisites were removed.

Both cleanup member seeds establish the same seven-occurrence group and three
actual cross-file calls. The native JSX seed establishes two groups with nine
occurrences, including four JSX attributes. The annotated member, contextual
implementation, inferred factory, JSX property and predicate expression all have
non-error native root types. Every Q4 observation already has two TS2300
diagnostics before summary, while `demandFailures` is empty. This is useful
selected-type evidence despite unrelated diagnostics, not a clean-project claim.
The predicate anchor selects the filter method expression with its actual
checking prelude; its native overloaded summary is truncated.

The [raw observation summary](../fixtures/types/command-center/aa484f4-2026-10-12/single-threaded-native/observation-summary.json)
separates inventory/demand counts and timings. Those timings include research
transport/capture/replay costs. Complete product normalized groups, eligible
call/test filter schedules, repeated/reversed requests, Q4 graph expansion,
JSX/predicate gap/refusal behavior, direct/service/load-refresh equality,
resource calibration and queued-lexical deadline acceptance remain open. Native
capture completion does not close those delivery gates.
