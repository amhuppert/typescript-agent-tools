# Persistent reference service

The service began with reference queries; it now answers every project query
(references, and outline and show with `--project`) through the same session
request boundary. Its administration keeps the `--references-*` flags.

## Intent and operating envelope

Repeated `tsparse --references` invocations, and project outline and show,
should reuse a completed project snapshot across agent processes. Callers are local processes of the same user,
including concurrent agents, over editable files on disk. Each worktree and
selected configuration must remain isolated. The initial service targets Unix
(macOS and Linux); other platforms retain direct reference loading. There is no
network listener, editor buffer protocol, or cross-user service.

The existing Command Center benchmark measures 22–25 seconds to construct an
index and about 304 MiB retained afterward. Those measurements use the documented
benchmark shims, not the unchanged Command Center configuration. Persistence
amortizes construction over unchanged queries; an edit initially requires a
complete rebuild. Neither IPC latency nor freshness checking is free.

## Alternatives and decision

Saving and restoring the frozen index on every invocation would preserve the
one-process lifecycle, but repeatedly allocates and reconstructs a large object.
A resident process can use the existing immutable `Project` directly.
Choose a resident process first; measure restart frequency before adding a disk
format. A single global multiplexing server would need project eviction,
multi-project scheduling, and routing. One service per selected configuration
provides isolation and a simpler lifecycle; idle shutdown bounds residency.

An explicit filesystem observer is preferable to a thread-local recording mode:
the observer is supplied to project loading and owns all read observations.
TypeScript configuration, file selection, and resolution rules remain in their
current owners. The observer knows filesystem operations, not TypeScript rules.

## Public behavior

Ordinary project queries connect to the service, starting it when necessary.
Each completed request performs freshness validation before answering,
whichever operation it is. Query JSON, diagnostics, output files, and exit
classes retain their CLI contracts: one CLI delivery policy renders an answer
the same way whether the service or a direct load produced it. Before
either entry answers, the client makes the request's file operand absolute
against its own working directory, through the one method that knows which
operand each operation has (`ProjectRequest::make_absolute`), and restores
the caller's spelling in diagnostics that name that path.

`--references-status --project CONFIG` reports whether a service is running and,
when it is, its PID and successful index generation. It does not start or load a
project. `--references-stop --project CONFIG` stops that project's service and
succeeds when none is running. `--references-direct` selects fresh in-process
loading for diagnostics and comparisons. `--references-serve --project CONFIG`
runs the service in the foreground for a process supervisor.

Service identity includes the selected absolute configuration spelling and the
running executable's compiled build identifier. Keeping the selected spelling matters:
the loader captures query aliases through that configuration directory, and a
symlink can later be retargeted. Distinct spellings may own separate services;
they must never accidentally reuse each other's captured lookup aliases.

## Owners

* `project::program::disk`: filesystem operations and their observations:
  exact text/bytes read, directory names, entry kinds (including absence), and
  canonicalization results (including absence). Share captured source text with
  `SourceText` rather than retaining another copy. A load asks each path's real
  path once. Validation repeats these operations and compares their results;
  source identity does not depend on size or modification time. I/O failures
  remain failures.
* `project::ProjectSession`: one project's current generation. Its one
  request operation (`execute(ProjectRequest)`) validates, loads when
  necessary, and answers any operation from a complete immutable `Project`.
  Callers do not orchestrate refresh steps. The service and the CLI's direct
  queries both answer through it. The existing `Project` API retains its
  explicitly captured-snapshot semantics and does not acquire hidden refresh
  behavior.
* Binary-private `service`: rendezvous, exclusive process ownership, private
  socket protocol, startup, idle shutdown, and administrative operations. The
  wire carries `ProjectRequest` and `Result<ProjectResponse, AnalysisError>`
  unchanged, so a new operation needs no protocol work.
* `cli`: one parsed-request boundary (`cli::request`), execution in file,
  direct, or service entry (`cli::query`), and one delivery policy for every
  outcome's channels and exit code (`cli::delivery`).

## Freshness and failure contract

Validation checks observations made by the existing loader, including negative
probes and directory listings. Adding a higher-priority module candidate,
changing inherited configuration or package metadata, retargeting a symlink,
adding a wildcard root, or replacing a dependency must invalidate the generation.
An unchanged metadata timestamp does not establish unchanged contents.

Validation repeats the observations on several threads; the first observation in
key order that differs or fails decides, exactly as one pass in key order would.
A read that could wait for another process, such as a pipe without a writer, is
never made speculatively: a path that is not a regular file, and a read that
fails other than by absence, are read only where that pass would read them, and
the observations after such a read are repeated after it ends. Loading follows
the same rule for its captures (`Reading::InOrder`). A warm request is answered
from the current generation while it is validated, and the answer is delivered
only once validation confirms the generation; otherwise a semantic answer stops
at its next check, any other answer finishes, the answer is discarded, and the
session reloads as before.

If an old observation is now unreadable, discard the cached generation and do a
full load. The current configuration/import graph decides whether that old path
is still required. A failure on a required current input propagates normally;
an obsolete input must not prevent loading a valid new project. Final validation
of a newly built generation propagates every I/O failure directly.

A request uses one complete generation. On detected change, invalidate the old
generation and construct another; if construction fails, return that failure.
Never answer from the previous generation after a failed refresh. Revalidate a
new generation before publishing it to detect changes during construction; if
the inputs keep changing, fail with a located I/O-class diagnostic rather than
retry indefinitely. As with ordinary filesystem loading, there is no atomic
filesystem snapshot and no guarantee against a write after its final check.

Serialize requests within a project service. Concurrent cold requests therefore
share one build, and no reader observes a partially replaced index. This is
proportionate to measured query costs and avoids concurrent rebuild memory peaks.
A slow or disconnected client must not terminate the service or monopolize it
indefinitely; bound protocol input and socket waits. Analysis failures and
transport failures remain distinct until CLI exit handling.

## Lifecycle

Use a private runtime directory outside the project source tree, with an
optional `TSPARSE_SERVICE_DIR` override for managed environments and tests.
Hold an operating-system advisory lock for the lifetime of a server. Only the
lock holder may remove a stale socket and bind its replacement. Lock files
remain in place so concurrent processes always lock the same inode; a crash
releases the lock automatically. Simultaneous launch attempts are harmless.

The client starts the current executable with null standard streams; the service
owns its own lifetime and exits after an idle period (default ten minutes).
An inherited shell pipe must not keep CLI output collection open, so the
service also closes every descriptor it inherited beyond the standard streams
before opening its own: a client can inherit a pipe its parent did not mean to
pass (std on macOS creates a pipe and marks it close-on-exec in two steps, so
a concurrent spawn can inherit it), and a service holding it would keep that
parent's output collection open until the service exits. Foreground
mode supports external supervisors. Administrative stop and test teardown must
wait for process ownership to be released. Runtime permission failures and
unexpected startup failures are reported, not silently converted to direct
loading. Windows uses direct loading until a native transport is implemented.

## Delivery and adversarial checkpoints

1. Pin public session behavior with failing integration tests: reuse, edits,
   failed refresh/recovery, negative probes, config/package changes, added roots,
   exact text despite identical size/time, and symlinks. Implement observation
   ownership and session refresh; review the actual filesystem call graph.
2. Pin independent CLI process reuse, concurrency, working-directory handling,
   startup/stop/restart, stale sockets, and failure propagation. Implement the
   transport and CLI; review process and socket races adversarially.
3. Reassess module boundaries against actual callers; remove leaked lifecycle
   steps and duplicated decisions. Run required formatting, lint, Rust, harness,
   and pinned semantic checks. Measure cold and repeated full CLI calls,
   including validation and serialization, and record limitations.

Incremental refresh can later reuse owned `FileFacts`, with configuration and
parse settings in the reuse contract, while initially relinking the whole
project. Disk snapshots and watchers are not required for this first complete
service. Neither should be introduced until measurements justify their lifecycle
and invalidation costs.

## Implementation review checkpoints

The first review retained the explicit filesystem owner: configuration,
resolution, root discovery, and source capture now supply one `Disk` instance.
Reviewing every filesystem call found no input reads outside it; the pre-existing
platform case detection remains in its owner. Source text is shared with the
observations using `Arc<str>`. Obsolete generations are released before rebuilding,
so serialized requests do not retain two large indexes simultaneously.

The transport checkpoint found and corrected two observable failures. On macOS,
accepted sockets can inherit nonblocking mode, so a client connecting before
writing was closed prematurely. Accepted streams are now explicitly blocking,
with socket timeouts; a delayed-write regression covers that gap. Moving query
paths across the process boundary also changed caller-spelled error paths.
The client restores the operand spelling for invalid-position/outside-project
errors while retaining canonical source paths for semantic target errors.

Another adversarial case changed configuration to exclude an input while that
old file became unreadable. Reporting its validation error rejected a project
that loaded successfully from scratch. A failing public-session test established
the issue; the session now delegates current input selection and error reporting
to a full load when it cannot validate the previous generation.

The lifecycle review replaced runtime executable hashing with a compiled build
identifier. Reopening an executable path can read a newly installed image rather
than the image executing the request, and adds file I/O to every CLI call. The
Cargo build script emits a unique identifier and watches source, vendored
dependencies, manifests, Cargo configuration, and compiler flags. This is build
wiring, so a separate test-first cycle was skipped; cross-process reuse tests
cover its integration. The protocol checks the complete identity even though
socket names use a compact hash, so a rendezvous collision cannot serve another
project's results.

The service deliberately serializes requests: tests prove concurrent cold callers
share one build, crash recovery does not trust a stale socket, stop releases the
OS lease, and idle shutdown frees the process. The request boundary is checked
just below, at, and above 1 MiB. In-range requests reach protocol validation;
oversized requests fail while subsequent queries still use the same generation.
No transport failure becomes an empty semantic result or a silent direct load.

The final [CLI measurement](reference-service-performance.md) found a 12.338 s
direct median and a 0.651 s warm median on 4,000 generated files (18.96×). This
supports retaining the current design. Freshness remains proportional to the
observed inputs, and post-edit queries still pay for a complete rebuild; these
are measured limits, not hidden incremental-update promises.

A follow-up [Command Center measurement](reference-service-command-center.md)
used all 5,316 native-selected roots with the existing benchmark compatibility
adjustments. Warm CLI medians were 0.709–0.945 s versus direct calls of
10.011–11.733 s, including a 14.9 MB response. Concurrent requests shared one
generation; editing a benchmark-only source rebuilt it and matched a fresh load.
The service retained about 662 MiB RSS. This keeps the lifecycle and observation
boundaries justified on the real corpus, while exposing the remaining costs of
full freshness validation and full post-edit rebuilds. The unchanged Command
Center config still hits the known library, JSON, and TipTap compatibility limits.

## Navigation checkpoint

Project outline and show joined references on the same wire request and
response enums and the same session boundary; no navigation-specific refresh,
cache, or daemon exists. Tests run outline, show, and references through
direct and service entry on identical inputs and require identical bytes and
exit codes, including failures; clients in other working directories, with
relative and absolute operands, share one generation; concurrent cold clients
of all three operations share one build; and source, configuration,
dependency, and library-replacement edits each publish one new generation to
whichever operation asks first, while a failed reload answers no operation
until the input is repaired.

A concurrency test that had failed intermittently under load
(`concurrent_cold_clients_share_one_build` reporting generation 0) had a
cause other than timing: a client spawned concurrently with another inherited
that client's output pipe, passed it to the service it started, and the test
waited for end of file until the service's idle exit, after which status found
no service. A regression test now passes the service an inherited pipe and
requires end of file once the client exits; the service closes inherited
descriptors at startup.

On unchanged Command Center (revision `7706073d`, its own `tsconfig.json`,
release build, a machine shared with another build), the first project query,
an outline through a newly started service, answered in 13.9 seconds; warm
outline, qualified show, ambiguous show (460 candidates), a candidate's
`--at` round trip from another working directory, and a reference name query
each answered in 0.46–0.70 seconds, including validation of every captured
input. Direct loads took 11.6–11.9 seconds each, and their outline, ambiguous
show, and reference outputs were byte-identical to the service's. The service
retained about 680 MiB RSS at generation 1.
