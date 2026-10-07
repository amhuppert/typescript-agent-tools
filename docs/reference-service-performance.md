# Reference service verification and timing

Measured 2026-10-07, release build, on the local macOS development host. This
compares direct loading and persistent-service calls in the same implementation;
it is not a comparison against an earlier parser revision. Other sessions share
the host, so absolute latency and the small cold/refresh differences are not
isolated-machine measurements.

Host: Intel Core i9-9880H (2.30 GHz), 16 GiB RAM, macOS 15.7.7. A thermal-status
read after the probe reported a CPU speed limit of 39%; timings therefore reflect
a throttled, shared development machine. The build uses the repository's release
profile (thin LTO and one codegen unit).

The follow-up [Command Center measurement](reference-service-command-center.md)
exercises the real 5,316-root codebase, including large results, concurrent
clients, and refresh. Its original configuration still requires the previously
documented compatibility adjustments.

## End-to-end CLI measurement

The generated project has 4,000 TypeScript files, 15,428,221 source bytes, and
40 exported functions per file. Each file after the first imports and calls one
function from its predecessor. A name query selects one function in the middle
of that chain and returns 707 bytes of JSON. The configuration uses `noLib`,
bundler resolution, and ESNext modules; no benchmark shims are needed.

Each sample invokes a separate CLI process. Timings include process startup,
filesystem I/O, freshness validation, IPC where applicable, result construction,
JSON output, and process exit. Three direct samples precede one cold service
call and seven warm calls. Complete output bytes must agree in every unchanged
sample, and administrative status must report generation 1 throughout reuse.

| Operation | Time |
| --- | ---: |
| Direct CLI, median of 3 | 12.338 s |
| Direct CLI, range | 12.274–12.590 s |
| Cold service, one sample | 12.443 s |
| Warm service, median of 7 | 0.651 s |
| Warm service, range | 0.576–0.777 s |
| Full refresh after a source edit, one sample | 14.051 s |
| Direct load after the same edit, one sample | 13.204 s |
| Warm call after the refresh, one sample | 0.887 s |

The repeated-call median improves by **18.96×**. The service's resident set after
the warm samples is 665,536 KiB (approximately 650 MiB). RSS includes allocator
retention and process overhead and must not be compared directly with the
requested-heap measurements in earlier reports.

The edit inserts a newline into the selected function's file. The refreshed
output differs from the old output, exactly matches a new direct load, and
status advances to generation 2. Shutdown waits for the service to release its
process lease. The probe left no benchmark service running.

Warm calls still read and compare captured source/config/manifest contents and
repeat directory, entry, and canonicalization observations. This provides a
conservative freshness contract without trusting modification times or watcher
delivery. The measured improvement justifies keeping the resident-process
design. It does not establish sub-100-ms latency on large projects or fast
incremental updates. See the follow-up report for Command Center measurements
and the distinction between its unchanged configuration and benchmark shims.

## Correctness evidence

The full Rust suite passes, with one existing platform skip: this filesystem
refuses the non-Unicode filename used by that test. The strict Node harness
passes all 74 tests with no skips. The registered TypeScript 7.0.2 semantic
oracle, formatting, Clippy, and release benchmark build also pass.

Public-session tests compare full query results and typed failures across every
marked semantic fixture. Additional scenarios cover reuse, same-size/same-time
edits, failed refresh/recovery, configuration inheritance, new and removed roots,
previously missing or non-directory include roots, negative resolution probes,
package metadata and declaration contents, automatic type packages, symlink
retargeting, required unreadable inputs, and old inputs excluded by new config.

Process tests cover independent CLI invocations, concurrent cold callers, shared
runtime-directory isolation, caller-relative paths and UTF-16 positions, exact
direct/service output and diagnostics, output files, stop/restart, crashes,
stale sockets, idle exit, private-directory errors, malformed/disconnected and
delayed clients, and valid requests just below and at the 1 MiB boundary plus
rejection above it. Reviews and the resulting design changes are recorded in
[the technical design](reference-service-design.md#implementation-review-checkpoints).

## Reproduction

Build with `cctl validate run benchmark-build --queue-if-busy --scope full`.
For an existing supported project, measure `--references-direct` and
`--references` with the same `--project`, query, and output options. Stop the
service before a cold sample, keep it running for warm samples, and compare
complete outputs. After a source edit, compare the refreshed service output
against a new direct invocation. Always stop the service after measurement.

The generated-project probe and raw sample arrays from this run are session-local
artifacts at `.cc/temp/reference-service-benchmark.py` and
`.cc/temp/reference-service-benchmark.json`. The probe is a throwaway measurement
script; no test-first cycle was used for it. It asserts complete output equality,
generation reuse/advancement, successful process exits, and cleanup directly.
