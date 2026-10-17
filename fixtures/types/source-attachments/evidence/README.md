# Historical capture accounting

These files record a bounded measurement after the captured final-tree index
was introduced. They are evidence, not fresh compiler oracle inputs or a
production resource guarantee.

`captured-syntax.rs` is the exact copied private capture implementation.
`measurement-test.rs` constructs the same four generated source families as
`tests/type_flow_limits.rs`, parses before measuring, and captures once.
`allocator.rs` counts successful allocation requests and live requested bytes;
it does not measure allocator overhead, RSS or stack memory. The before/after
live delta includes the new parent vector and attachment table, while the AST
was already live. Instrumented timing is separate from production latency.

`copied-source-inputs.json` names the whole copied crate's original source
hashes. That complete crate was kept under
`.cc/temp/types-source-attachments/measurement-crate`, with its own target
directory and an inert service build string; the full historical crate is not
duplicated here. `output.txt` retains the raw terminal result, and
`measurements.json` retains its exact rows/compiler/command. Reproduction
requires those named source versions; a current checkout with different hashes
must not be represented as reproducing this historical run. The excerpt alone
does not make the entire crate reproducible.

`registered-post-index.json` separately records the successful registered
four-test Project invocation and whole wall time. Queueing and compilation are
included; this is not an isolated query microbenchmark. `provenance.json`
hashes each bounded evidence artifact.
