# Captured-source representation spike: bounded adversarial review

Reviewed using the Ousterhout reviewing route and AGENTS.md. Read the final
spike report, shared Rust driver, summary aggregation, native primitive verifier,
and selected raw/summary evidence. No production code, additional benchmark,
or new representation choice was introduced by this review.

Verdict: no blocking claim or measurement defect found after the author's
explicit lower-bound and measurement-limit refinements. The report is a useful
partial structural replay/cost experiment and correctly leaves checkpoint 2
open.

Both alternatives use one parser/traversal/inventory/projection/consumer. Final
AST preorder replaces arena ordinals, and logical child/span operations reuse
the existing owners. Requested-live allocation accounting preserves its baseline
across drops/reallocations. The instrumented binary supplies heap observations;
the distinct tracking-disabled binary supplies elapsed medians. Aggregation
does not pool those timings. Assertions remain timed where the report says they
do. Neither process resident memory nor a whole Project load is measured.

The 662,181 terms and logical child edges explain the reported 29,135,892-byte
partial payload. The all-file request reports 16 actual reparses and 322 answers,
while its inventory counter includes two undemanded files. Zero retained request
delta applies to this cache/consumer after teardown, not to a future checker or
service. Hot-process timings without CPU isolation are observational samples,
not fresh-query guarantees.

The native verifier reads final timing rows and checks 40 selected primitive
annotation sites in two shared native sessions. The four observed kinds do not
prove all eleven prototype mappings or whole declared value types. The explicit
BOM normalization is checked against otherwise identical captured bytes. No
binding, alias, inference, overload, flow, factory, member, refresh or full
semantic replay claim follows from these leaves.

The exact retained depth300 control is native-valid and fails this parser's
resource budget. Its exclusion and reproduction are disclosed; it does not
become successful supported input or a parser limit-conformance proof.

The original broad lower-bound wording needed qualification: 40-byte partial
terms do not lower-bound every possible full representation. The final report
now restricts that statement to extending this exact prototype and notes that
smaller kind codes, node inventories and fields could alter the layout. No
additional correction is required for this bounded evidence. Keep the full
canonical binding, complete shared semantic-source contract, real native demand
consumer, and old-operation measurements before choosing the final storage
representation.
