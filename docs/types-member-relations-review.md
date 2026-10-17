# Independent member relationship and corpus evidence review

2026-10-10. Ousterhout reviewing route, local test-only compiler observations
and repository evidence; no native implementation or completed proof gate is
assessed. No production or shared artifact edits by this reviewer.

## Finding

**Medium, corpus capture provenance boundary:** each of the three stored
Command Center member answers lists 7,967 compiler-read input paths. There are
327 project paths outside the source/configuration/lock set checked by the
historical before/after verifier: primarily dependency `package.json` files and
`.next/package.json`. The 7,639 source-manifest hashes, configuration and lock
checks do not establish that these resolver metadata inputs were unchanged
across the three requests. A locked dependency graph does not prove installed
metadata bytes. The per-request Go replay captures those inputs, but the stored
answer retains paths rather than their content hashes.

The document's literal statement about checked manifest inputs is correct.
Calling the journeys frozen without naming this additional boundary could lead
later implementers to treat the observations as a complete fixed-input proof.
For these historical rows, document the limitation; do not manufacture old
hashes. Future captures should record all consumed read-file inputs through the
existing capture owner and compare their hashes. This reduces hidden caller
knowledge (P10/P13), without creating another configuration/resolution parser
or a second capture policy.

Disposition: the implementation owner accepted the finding and will explicitly
name the 327 unhashed resolver metadata paths and exploratory boundary in the
corpus evidence. The historical observations remain unchanged; no retrospective
input-freeze proof is asserted. The two low wording refinements below are
recommendations, not new behavior requirements.

Two low wording refinements improve the same evidence contract: clarify that
the pinned source tree and compiler limit constants remain unchanged while a
test-only overlay adds instrumentation/scheduling; and name which guard
families the historical counters observe. No observed recorded guard failure
does not establish that every possible compiler recovery owner was observed.

## Confirmed evidence

- All thirteen tooling/fixture hashes in
  `fixtures/types/member-relations/provenance.json` match the retained bytes.
  The old probe scripts retain their historical paths and are explicitly
  historical, not a second executable production owner.
- The relation capture has exactly 43 rows. Every row records baseline history;
  candidate demand counts match candidate ledgers. All recorded demand-failure
  lists are empty, within the historical instrumentation's limits.
- The durable registered result `vrun-b010ac19-16c0-4207-bb79-9541e99540a3`
  confirms one matched file, six executed tests, six passed, zero skipped.
  This reviewer read the registered verdict; no additional benchmark or
  relationship test run is claimed.
- Static and instance declarations on one line use exact kind/start/end
  origins and occurrence spans. Equal-spelling private names in different
  classes retain separate exact groups. Getter and setter origins on one line
  retain both distinct exact declarations in one group. These assertions would
  fail a line-only or spelling-only collapse. Simpler one-member-per-line
  families use line projections; the document does not claim all identity
  normalization is complete.
- Generic/homomorphic/remapped roots are asserted separately from directed
  reference groups. Remapping keeps the Box origin but searches the remapped
  key. Contextual declaration/use direction and inline/interface union
  differences are checked without treating assignability as identity or
  relatedness as a union-find merge.
- Corpus rows have actual decoded demands `119,119,12`, raw group counts
  `1,1,2`, occurrence counts `7,7,9`, and related candidate counts `7,7,9`.
  Each has one relationship search-set construction and equal candidate ledger,
  demand and observation lengths. The two cleanup seeds retain the same exact
  interface-method origin.
- The JSX row contains four actual attribute origins, each with property flags
  `4`, valid native positions and related `CollapsibleTextProps` origins. This
  supports the native relationship fact. Located `jsx_typing_required` gaps
  remain future product-profile obligations when JSX typing is demanded; no
  measured native admission or gap rate is claimed. Three storybook assignments
  retain a separate raw group with a repeated exact PropertySignature origin.
  The document preserves that raw duplication and explicitly leaves product
  canonicalization open.

## Scope verdict

No unsupported general member normalization or semantic performance claim was
found. The documents distinguish raw and sorted groups, canonical roots,
selection roles, directional relationships, and type equality. They keep
multiple-distinct spread origins, missing union members, recovery receivers,
full contextual forms, reversed/cross-file schedules, product coverage,
dependency admission and service/resource guarantees open. The exploration's
elapsed times explicitly include startup/capture/Go compilation and cannot
calibrate native request budgets.

The root accepted the provenance finding and made the historical boundary explicit in
`docs/types-corpus-evidence.md`; it also accepted both wording refinements.
With that corpus provenance boundary explicit, the evidence is suitable as
bounded compiler expectations for later implementation. It does not prove the
native checker, a canonical member graph, full Q3 groups/gaps, or gate readiness.
