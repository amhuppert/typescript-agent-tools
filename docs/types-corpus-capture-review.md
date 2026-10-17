# Bounded corpus capture completion review

This read-only Ousterhout review examines `tools/type-corpus-capture.mjs`, its
four artifact tests, and `fixtures/types/command-center/member-probe.mjs`.
The envelope is local fixed-corpus compiler research: fresh native requests,
trusted checker instrumentation and disk artifacts that may be incomplete or
malformed. Completion verifies capture bookkeeping and bytes, not compiler
admission, transactional filesystem snapshots or production semantic behavior.
No historical corpus artifact or shared JavaScript was edited by this review.

## Findings

1. **Medium: malformed artifact inventories can acquire a success marker.**
   `completeCapture` checks nonempty observations and aligned file/hash rows but
   accepts any schema and answer status, and permits duplicate files within a
   request. Temporary artifact probes showed `schema:1`, `status:"failed"`, and
   duplicate `[file,file]` rows each persisted `captureComplete:true` with valid
   current disk hashes. Duplicate rows silently collapse into one aggregate
   input. Thus byte verification alone does not establish the promised schema-2
   completed-observation boundary. Validate the schema, unfinished lifecycle,
   successful observed answers and unique exact inventory before publishing.
   Keep those decisions inside the completion owner rather than having every
   capture caller repeat them.

2. **Medium: the member caller can finish a different or incomplete seed run.**
   `member-probe.mjs` filters for three IDs without requiring exactly one of
   each. It writes the new unfinished artifact only inside that loop. With a
   malformed manifest containing no matching query and an existing valid OUTPUT,
   the loop writes nothing; finalization then reads the old output and can print
   success for it. A missing or duplicated selected ID can likewise produce an
   incomplete intended three-seed capture. The current checked-in manifest has
   all three IDs, so this is not a defect in the historical recorded rows.
   Validate the exact requested seed set before checker work, write the new
   unfinished artifact before its first demand, and preserve the expected request
   inventory so completion can detect missing/duplicate observations. Selection
   policy belongs to this fixed-seed caller; generic finalization should validate
   the artifact's explicit inventory.

3. **Low, accepted lifecycle clarification:** finalizing an already completed
   artifact after an input changes throws but leaves its existing true marker.
   Historical completion is evidence about the captured run, so mutating that
   record to describe today's disk would be misleading. Explicitly prohibit
   re-finalization of immutable completed artifacts; a new capture should have
   its own unfinished record. This also makes the helper's unchanged-on-failure
   behavior understandable instead of depending on an undocumented fresh-state
   convention.

The parent accepted these findings and will add registered behavior-level RED
cases at the artifact boundary, then fix the same owners after the active shared
harness gate settles. The review probes used disposable temporary artifacts and
are not registered RED evidence. No new compiler-admission obligation is inferred.

## Supported guarantees and limits

The existing four tests are meaningful: they compare the completed disk
artifact, reject missing hashes, preserve a false artifact when metadata changes,
and reject incomplete or inconsistent per-request hash rows. This reviewer read
the durable GREEN status of
`vrun-d6304deb-cc94-4501-a958-0d3ae0ffa87a`: four executed/passed, zero skipped.
The parent's earlier precise four-assertion RED is preserved separately. That
GREEN does not cover the additional malformed/lifecycle/seed variants above.

Current file reading errors, mismatched content hashes and cross-request hash
inconsistency throw rather than becoming missing inputs. Raw BOM bytes are
hashed as bytes. Non-absolute virtual inputs fail this disk verifier; they are
not silently omitted. Compiler-source and library identity remain separate
provenance obligations. The corpus before/after verifier checks tracked revision,
listed source/configuration/lock hashes, and the finalizer checks every reported
consumed disk input. It cannot independently reconstruct an input that an
upstream producer omitted from both reported arrays. Completeness therefore
depends on the separately reviewed native read-capture owner as well as the
artifact's inventory validation; align prose with that boundary.

Before/after verification remains nontransactional. Multiple native requests
and final file reads are not an atomic filesystem snapshot, and their elapsed
times include research capture/build/replay work. These facts do not prove type
admission, complete eligible-site accounting, native semantic performance or
production refresh/cache correctness. The historical corpus's 327 metadata
paths without content hashes remain a declared exploratory provenance gap;
future capture fixes cannot retroactively supply those bytes.

## Follow-up disposition

The parent addressed all three findings after registered desired RED
`vrun-cf3b0b10-60e1-4c1f-8951-8ff8e4a6dcc2`. Independent re-reading found
no new blocking defect within this capture envelope. Completion now requires
schema 2, an explicitly unfinished artifact, unique expected request IDs, exact
deep equality between each expected/observed demand, observed answers and unique
per-request input identities. Equal counts and unique known IDs establish that
every expected request has exactly one observation before any success write.
Completed historical records explicitly reject re-finalization.

The fixed member caller requires exactly one of all three selected IDs and
writes the new unfinished artifact before its first native demand. Added
subprocess tests pin malformed/empty seed lists with an existing old output and
a first-demand failure with a freshly written false artifact. Native semantics
are deliberately stubbed only in those bookkeeping tests; no compiler behavior
expectation is taken from the stub.

Final registered single-file gate
`vrun-6e043def-115f-487f-8610-2c4b973dce63` executed/passed eight tests with
zero skips. This reviewer independently confirmed its durable terminal status.
Trusted upstream input completeness, nontransactional filesystem verification,
compiler/library identity and all historical provenance limitations above remain
unchanged. No shared source or historical corpus artifact was edited by this
reviewer.


## Native program inventory follow-up (2026-10-12)

The new isolated `aa484f4` baseline exposed two additional producer obligations.
Resuming its unfinished two-row member artifact failed the full program join in
`vrun-45839cad-2619-4d58-8198-fa72ba79dfa2`: 35 default-library files had different
`externalLibrary` flags between native-OS API discovery and callback-filesystem
API discovery, while source identities/order, options and byte hashes agreed.
No resumed native demand occurred. The earlier manifest and unfinished artifact
are preserved under `aa484f4-2026-10-12/superseded-api-baseline`.

A five-file native fixture plus one external package with a lib reference
reproduced the mismatch under the corpus `dom/dom.iterable/esnext` profile in
`vrun-6283eb40-412c-45a6-b811-38e3b57f4794`. The final selected type was healthy
`() => void`; the failure was the provenance comparison. The es5-only control
had passed. Moving the existing filesystem capture unchanged into
`tools/type-oracle-filesystem.mjs`, then using that same owner in both freeze and
query capture, passed the exact reproduction in
`vrun-368d90f6-4bb5-41c6-b13b-e6cdd04ad61e`. No provenance flag was normalized.

Independent source review also found that `compilerHookOracle.programFiles`
echoed its API input; the Go replay checked only a sorted non-default source
set. This did not establish native replay order, default-library identity or
native provenance. Two import-order fixtures with and without BOM failed only
on missing actual replay evidence in
`vrun-9e2fa3f6-6c9f-4985-ac53-4820a151d506`, then passed in
`vrun-d86a2100-e54c-458e-8878-25ce60412240` after recording the actual native
`GetSourceFiles`, `IsSourceFileDefaultLibrary` and
`IsSourceFileFromExternalLibrary` results with decoded source SHA-256 hashes.
Raw input hashes remain independent and preserve BOM bytes.

The shared oracle now retains raw `nativeProgramFiles` and independent
`apiProgramFiles`. Its returned `programFiles` comes from validated native
replay metadata. Only a unique default library with identical pinned source
text may map a native bundled URI to its API disk identity; all flags and order
must then match exactly. Compatibility and the new full corpus captures remain
pending. These fixes cannot retroactively add actual native inventory evidence
to the two earlier incomplete observations.


### Controlled-profile disposition

The shared-setup repair is retained, but matching setup alone cannot make pinned
parallel parser provenance deterministic. A subsequent unchanged tiny run failed
freeze's two-discovery equality (`vrun-cf180a45-adfc-4bf0-bb12-39bbdfb533a4`);
the stronger actual native replay join also failed the lib-overlap case in
`vrun-9cada597-46d6-4ccd-ab20-8dab16158b97` (19 other compatibility tests passed).
The two-profile diagnostic and source are preserved under `.cc/temp`; on another
run both profiles agreed, which is further evidence of scheduling variation,
not a stable parallel contract.

Pinned filesparser.go lowers shared task depth when it is reached through a
shallower path, but starts a previously loaded task's children only when its
`startedSubTasks` flag is false. This source explains how library children can
retain an external depth from an earlier dependency path. No pinned compiler
code was changed. `singleThreaded` is an accepted native config Boolean and an
already classified Rust tooling/policy Boolean; it does not override product
workers. The explicit new native profile extends the original config with true.
Its exact child/base hashes, purpose and original parallel limitations are
recorded rather than silently changing options or normalizing provenance.

The explicit-config final-artifact test failed in
`vrun-44181236-c553-4a11-80f2-5e5311624351`, then the controlled-profile checkpoint
passed seven tests with zero skips in
`vrun-482d1679-46e3-46c6-88b4-e84084028f56`. This includes the controlled tiny
lib-overlap native/API comparison and both actual replay order/BOM witnesses.
Full controlled corpus freeze, three fresh member seeds and five fresh Q4 roots
then passed in `vrun-85e920b5-4e96-43e0-a8f8-989faf646f33`,
`vrun-cf1abce4-3435-458b-af90-8a03dd47c9e6` and
`vrun-0745e72c-9f3c-4c4b-90a2-b24f142cfe08`. Every query retains actual native
source metadata/order and all consumed raw hashes. Original parallel observations
cannot be joined to this changed execution profile and were not reused.

No external corpus prerequisite remains in the normal harness suite: temporary
research test drivers were archived with exact hashes and removed after terminal
collection. Product filter schedules, normalization, TypeScript-profile gap
classification and service/resource/corpus acceptance remain independent work.
