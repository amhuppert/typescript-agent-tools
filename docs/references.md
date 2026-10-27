# Find references

`tsparse --references` groups the declarations and occurrences of a symbol in
one configured TypeScript project. The Rust library exposes the same operation
as `typescript_parser::project::Project::find_references`, with query and result
types in `typescript_parser::references`. The design is recorded in
[find-references-design.md](find-references-design.md); this document describes
the implemented behavior and where it deliberately differs from TypeScript.
`tsparse --definition` prints the declarations the same selection denotes
([go to definition](#go-to-definition)).

Unless a passage names the TypeScript 5.6 dialect, this document describes
the TypeScript 7 dialect, which reads every project whose installed compiler
is not TypeScript 5. A project whose installed compiler is TypeScript 5 is
read with the TypeScript 5.6 dialect, which differs only where a passage
names it: in [Compiler selection](#compiler-selection), which describes the
whole dialect, and in [Package identity](#package-identity), [JavaScript
inputs](#javascript-inputs), and the library and JSX-runtime entries in
[Compatibility limits](#compatibility-limits).

## Command

```sh
tsparse --references --project ./tsconfig.json --at 12:8 src/app.ts
tsparse --references --project ./tsconfig.json --at src/app.ts:12:8
tsparse --references --project ./tsconfig.json --name run
tsparse --references --project ./tsconfig.json --name run src/app.ts --pretty
tsparse --references --project ./tsconfig.json --name run --calls --no-tests
tsparse --references --project ./tsconfig.json --name run src/app.ts --file src/api.ts
tsparse --references --project ./tsconfig.json --name run --json
tsparse --references --project ./tsconfig.json --name run --limit all
```

`--project` names a configuration file; no ancestor directory is searched.
Exactly one of `--at LINE:COLUMN` (one-based line, one-based UTF-16 column, with
a required FILE) or `--name NAME` (with an optional FILE that narrows the
candidates) selects the symbols. `--at FILE:LINE:COLUMN`, the form navigation
text prints, names the file without a FILE operand; it is read from the right,
so FILE may contain `:`, and giving both is a usage error. A bare NAME selects
lexical declarations and aliases. A dotted NAME such as `RunHost.observe`
selects by declaration containment, with explicit not-found/ambiguous results;
`--at` also selects supported semantic member tokens. Argument paths are relative to
the working directory. Without `--json` an answer is [text](#result); `--json`
writes its document instead, and `--pretty` indents that document, alone
writing it in place of the text, as references always have. `-o OUTPUT`
writes the result to a file. An answer shows at most 200 occurrences unless
`--limit N|all` or `-o` says otherwise ([display limit](#display-limit)). Stdin, `--check`, `--tsx`, and `--depth` are
rejected. `--direct` answers in the CLI process instead of the service. A result goes to stdout or `-o`; by default a
selection that selects nothing or several, and a failure, are diagnostics on
stderr and never create or replace the `-o` file. With `--json`, such a
selection and an analysis failure are delivered as documents to stdout or
`-o`; with `--pretty` alone the selection is its indented document there, and
a failure stays a diagnostic. A usage or transport failure is
`CODE: message` on stderr, and with `--json` a document (a usage failure's on
stdout, never `-o`); a result that cannot be written is one stderr
diagnostic. Navigation shares this delivery policy
([exit codes and channels](navigation.md#exit-codes-and-channels)), and the
[CLI reference](cli-reference.md#failure-codes) lists the codes.

`--calls` keeps only direct call, optional call, constructor and tagged-template
targets. Parentheses, non-null assertions, type assertions, `as`, `satisfies`
and instantiation wrappers are peeled. Namespace accesses count their member,
not their receiver. Taking a function as a value, passing it as an argument,
or calling its `.call`, `.apply` or `.bind` member does not count as a call to
that function. Implicit decorator calls and JSX tags are excluded; explicit
`@factory()` calls are included. Declaration metadata survives an empty filter.

`--no-tests` removes occurrences in exact case-sensitive directory segments
`test`, `tests`, `__tests__`, or basenames ending `.test` or `.spec` followed by
`.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs` or `.cjs`, on normalized
project-relative `/` paths.
Selection, loading, dependencies and declaration metadata still include those
files. `fixtures`, `spec`, `Tests`, and `.TEST.ts` are retained.

`--file FILE` keeps only occurrences in FILE. FILE is named as the FILE
operand is, relative to the working directory and never `-`, and compared as
every query path is ([Library](#library)): lexically, with `\` read as `/`, and with the
file system's case policy, so on a case-insensitive file system `APP.TS`
names `app.ts`. A FILE that is not one of the searched files fails the query
as `OutsideProject` (exit 2), never giving an empty answer; its message names
the configuration whose `files`, `include`, and `exclude` decide membership. The FILE operand
narrows the candidate declarations; `--file` instead filters the occurrences
of whatever was selected, and the two combine. Like the other filters it
keeps declaration metadata and the selected group, and a member search does
not establish candidates outside FILE. The filters are echoed in the result
(`filters.file` names FILE as locations name files, and is present only when
given) and rejected on service-administration commands.
Results identify their coverage as `lexical` or `member`. Bare-name lexical
results list matching catalog members in `unsearchedMembers`. Member searches
report `complete` or `partial` coverage and located gaps; they do not merge
unrelated properties by spelling. See [types.md](types.md) for semantic selection,
partial results, and process limits. Final delivery validation is tracked in
[the completion checkpoint](types-completion-correctness-2026-10-13.md).

The [CLI reference](cli-reference.md#references) lists what each exit code
reports, as `tsparse --references --help` does.

Diagnostics on stderr have the form `path:line:column: message` when a location
is known. Sources, configurations, and package manifests are read as TypeScript
reads them (`decodeBytes`): a leading UTF-8 byte order mark is not part of the
text, so first-line columns in queries, results, and diagnostics are the ones
`tsc` reports. Loading reads only regular files: a pipe, device, or socket it
selects there, which TypeScript would read, is refused before it is opened, as
an `Io` failure `PATH: not a regular file` (exit 2), so no query waits on a
pipe's writer.

## Persistent CLI service

Ordinary CLI project queries use a local service: reference, definition,
construction, and type queries, and [outline and show](navigation.md#project-mode)
with `--project`.
One service per absolute configuration spelling answers these through one
session request boundary, which checks the captured filesystem inputs before
every request and loads a new generation after any change; there is no second
service, cache, or refresh path per operation. It reuses a complete snapshot
while its inputs match. The check rereads and compares every captured input,
on several threads, so it dominates a warm request on a large project: on
Command Center (4,154 searched files) the check takes about 0.1 s of a
0.12–0.16 s warm query, and a rebuild after an edit about 9.5 s
([measurements](performance-command-center.md#current-state-2026-10-17-after-the-performance-work)).
`--references-status --project CONFIG` reports a running service in two
lines, `running: pid 59643, generation 1, build STAMP, idle 10m, 7m12s remaining`
and `limits: 6144 MiB, 60000 ms`, or `no service: CONFIG`; `--json` (or
`--pretty`, alone or with it) prints the document instead: `running`, `pid`,
`generation` (the number of successfully published snapshots), `build`,
`limits` (`max_memory_mb`, `query_timeout_ms`), `idle_secs`,
`idle_remaining_secs`, and `started_unix`. Without a service `running` is
`false`, `generation` is `0`, and every other field is `null`.
Status does not check freshness and does not extend the service's idle time;
between two calls to one running service only `idle_remaining_secs` changes.
A runtime directory that does not exist, or whose path runs through a file,
holds no service: status reports none there and stop has none to stop.
`--references-stop --project CONFIG` stops it and prints `stopped: pid N`, or
`no service: CONFIG` when none was running. These three flags administer the
project service, which answers every project query (`--references`,
`--definition`, `--type`, `--constructs`, and `--outline`, `--show`, and edits
with `--project`), whatever their names say. `--direct` performs a fresh load
in the CLI process through the
same request boundary, giving the same output and exit code.
`--references-serve --project CONFIG` runs the service in the foreground for a
supervisor. Background services use null standard streams and a separate process
group, close every other descriptor they inherit, and expire after ten idle
minutes.

A service reads `TSPARSE_MAX_MEMORY_MB`, `TSPARSE_QUERY_TIMEOUT_MS`, and
`TSPARSE_SERVICE_IDLE_SECS` once, from the environment of the client that
starts it, and keeps them until it exits: changing them in a later client's
environment does not change a running service. A query that exceeds a limit
ends the process that ran it, and its failure says which: for example
`query exceeded TSPARSE_QUERY_TIMEOUT_MS (60000 ms) in the project service;
that service and its cache exited, and the next request starts a replacement`,
or in a direct query `…; this query process exited`, and for memory the
footprint observed beside the threshold. Its remedy, `set a higher
TSPARSE_QUERY_TIMEOUT_MS before retrying`, is the text's `  next:` line and
the document's `error.remedy`; it never says to stop a service, which has
already exited, and the replacement the next request starts reads that
request's environment.

`tsparse --doctor` checks what project queries depend on, one line per check,
and loads nothing, starts nothing, and creates nothing: `build` (the version,
build stamp, platform, and whether the service, `--apply`, and memory
monitoring work there), `environment` (each `TSPARSE_*` variable with its value
or default, and every invalid value), and `runtime directory` (its path, that
it is a private directory of this user's with mode 0700 or does not exist yet,
and its socket paths' length beside the longest a socket address holds, 103
bytes on macOS and 107 on Linux). `tsparse --doctor --project CONFIG` adds
`configuration` (the spelling, its absolute form, and that it can be read),
`service` (none, with the failure an earlier start recorded and a partial
diagnostic it left, which the next start removes and the doctor only reads;
or its pid, generation, build, idle time, and start), and
`service limits`, the limits the service inherited beside this environment's.
A difference is a finding:

```text
service limits: TSPARSE_QUERY_TIMEOUT_MS 60000 inherited at start, 120000 in this environment
  why: a service keeps the environment it started with
  next: tsparse --references-stop --project tsconfig.json
```

Each finding prints its `why` and `next` lines, as a failure does, and the
last line is `ok: N checks pass` or `findings:` and the checks that found
something; a check the doctor cannot perform is a finding. It exits 0 when
every check passes and 1 when any finding is present. `--json` prints one
document with an entry per check: `name`, `status` (`pass` or `finding`),
`detail`, and a finding's `why`, `next.argv`, or `remedy`.

The private runtime directory is chosen from the environment alone, without
looking at the file system, by the first of these rules that applies:

1. `TSPARSE_SERVICE_DIR`, when it is set: any path, a relative one taken in the
   working directory. A directory it names that cannot be used fails; nothing
   else is tried.
2. `TMPDIR/tsparse-UID`, when `TMPDIR` is a non-empty absolute path and the
   socket paths in that directory fit a socket address. `TMPDIR` is taken by
   its components, so a trailing slash, as macOS writes it, names the same
   directory as none.
3. `/tmp/tsparse-UID` otherwise: `TMPDIR` is not set, empty, relative, or so
   long that the socket paths would not fit.

On macOS every login session sets `TMPDIR`, so the second rule is the usual
one there. `--doctor` names the directory, the rule that chose it, and, for
the third, why `TMPDIR` was not used:

```text
runtime directory: /var/folders/zz/zyxvpxvq6csfxvn_n0000000000000/T/tsparse-501 (TMPDIR/tsparse-UID, as TSPARSE_SERVICE_DIR is not set) is a private directory owned by this user with mode 0700; its socket paths take 82 of at most 103 bytes
runtime directory: /tmp/tsparse-501 (/tmp/tsparse-UID, as TSPARSE_SERVICE_DIR is not set and TMPDIR is not an absolute path) does not exist yet; a project query creates it with mode 0700; its socket paths take 38 of at most 103 bytes
```

A finding names the directory and the rule the same way, then the failure a
query there would report, with its remedy (`detail` and `remedy` in a
document). A failure before any directory is chosen, for a configuration whose
service cannot be named or a `TSPARSE_SERVICE_DIR` that cannot be made
absolute, is reported alone:

```text
runtime directory: /tmp/tsparse-501 (/tmp/tsparse-UID, as TSPARSE_SERVICE_DIR is not set and TMPDIR is not set): Project service directory /tmp/tsparse-501 cannot be created: Operation not permitted (os error 1)
  next: set TMPDIR to an absolute directory this user can write, or TSPARSE_SERVICE_DIR to a path in one; TMPDIR is not set
```

The directory must be owned by the current user with mode 0700, and a client
creates it, but not its parent. A client passes the absolute directory it chose
to the service it starts. Environments that choose different directories,
such as an unsandboxed shell and a sandboxed agent with another `TMPDIR`, start
separate services for one configuration and build, and status and stop in one
do not see the other's; set the same `TSPARSE_SERVICE_DIR` in both to share
one service. Keep paths short enough for Unix sockets: a socket
address holds about 100 bytes (104 on macOS, 108 on Linux), and the socket's
name adds 22 to the directory. A longer `TSPARSE_SERVICE_DIR` fails every
service request with `transport.socket`, exit 2, naming the socket, with the
remedy `next: set a shorter TSPARSE_SERVICE_DIR` (`remedy` in a document). Set
`TSPARSE_SERVICE_IDLE_SECS` to a positive integer to change the idle timeout.
Configuration spellings and compiled build identities have separate services.
Old-build services expire normally. Lock files remain after shutdown so concurrent
launchers always lock the same inode; do not remove them while services run.

A client reports every startup cause it can see before it starts a service,
with exit 2 and `CODE: message`, then any remedy, on stderr, or with `--json` one
document: an invalid `TSPARSE_SERVICE_IDLE_SECS`,
`TSPARSE_MAX_MEMORY_MB`, or `TSPARSE_QUERY_TIMEOUT_MS`, named with its accepted
form, default, and value (`usage.environment: TSPARSE_SERVICE_IDLE_SECS must be
a positive integer number of seconds (default 600), got 0`), which direct
queries, direct edits, and `--references-serve` report alike; as
`transport.directory`, a runtime directory that cannot be created or
inspected, named with the cause, and one that is not a directory owned by the
current user with mode 0700, since a service could not report why it failed in
a directory it cannot write, which `--references-status` and
`--references-stop` also report; and,
as `transport.socket`, a socket path that is too long. A directory of this
user's with any group or other permission prints
`  why: any process that can open the socket can run queries as you` and
`  next: chmod 700 DIR` (`why` and `next.argv` in a document), or, where DIR is
not Unicode and so cannot be named in a command,
`  next: give the runtime directory mode 0700` (`remedy`); one whose owner
lacks read, write, or search says a service could not create its socket or
record why it failed, with the same command; and a directory of another user,
or a link or another kind of file at its path, prints
`  next: set TSPARSE_SERVICE_DIR to a new path or to a private directory this user owns`
(`remedy` in a document). A directory that cannot be created or inspected
names another directory by the rule that chose it, such as
`  next: set TMPDIR to an absolute directory this user can write, or TSPARSE_SERVICE_DIR to a path in one; TMPDIR is not set`
for a sandbox that denies writes to `/tmp`; for a directory `TMPDIR` chose,
`  next: set TMPDIR to a directory this user can write, or TSPARSE_SERVICE_DIR to a path in one`;
and for one `TSPARSE_SERVICE_DIR` names,
`  next: set TSPARSE_SERVICE_DIR to a path in a directory this user can write, or unset it and set TMPDIR to such a directory`.
Status and stop read only the runtime directory `TSPARSE_SERVICE_DIR` and
`TMPDIR` choose. Clients that find no service start one at a time, holding
`KEY.start.lock`, which, like the lock file, is never removed; a client that
waits 10 seconds for another's start without a service answering says so, as
`transport.timeout`. A
service that fails before it listens, for example on a lock file it cannot
open, writes the failure's code and message to `KEY.start-error` beside its
lock file: it writes
`KEY.start-error.staged`, a new file with mode 0600 that it locks while
writing, and renames it into place once complete. The client that started it
reports that failure with its code, as `--references-serve` prints it (for
example `transport.lock: Project service lock …`), and removes the file. A staged copy whose writer died, for
example when it was killed while writing, is removed by the next start or the
next failing service and never read. A start waits up to 5 seconds for the
writer of a copy whose lock is held, since a writer killed while writing holds
it until any core dump completes, and leaves a copy still held then to it. A staged path naming anything but a regular file, such as a pipe or a
symbolic link, is reported without opening or removing it, and so is a
published diagnostic of another kind when the client reads it after a failed
start. A client removes an earlier published diagnostic, of any kind but a
directory, before starting a service; a foreground service's failure stays
recorded until then. When a service exits without recording a cause, including
when the diagnostic cannot be written, the client prints, as its `next`
command, `tsparse --references-serve --project` with the configuration's
absolute path: the service it started, run in the foreground, whose stderr
shows the cause and why it was not recorded. A service that was killed while
starting, rather than failing, records nothing either, and its cause may not
recur: then that command starts a healthy service, which serves until it is
idle (`TSPARSE_SERVICE_IDLE_SECS`) or `--references-stop` stops it.

Requests are limited to 1 MiB including their terminating newline. Socket reads
and writes at the server allow five seconds without progress; clients wait up to
five minutes for a result. A transport failure is exit 2, `CODE: message` and
its `why` and `next` lines on stderr or, with `--json`, a document with the same
parts, and never a silent direct load. Its message ends with the command that
diagnoses it, `run: tsparse --doctor --project CONFIG`. Its code names the cause:
`transport.directory`, `transport.socket` (also a connection that fails or
closes before an answer), `transport.lock`, `transport.startup` (a service that
could not be started, exited without recording why, or whose diagnostic cannot
be read or removed), `transport.timeout` (a service that does not become ready,
take a request, answer, or stop in time), `transport.protocol` (a request over
1 MiB, one the service could not read, or an answer that does not decode), and
`transport.identity` (a service at the endpoint for another configuration or
build); the [CLI reference](cli-reference.md#failure-codes) lists them. Before either a direct or a service answer, the client makes a query's
file absolute against its own working directory
(`ProjectRequest::make_absolute`, for every operation), so both answer alike
from any directory; diagnostics about that file (`OutsideProject`,
`InvalidPosition`) keep the caller's spelling, and a working directory that
cannot be read is an `Io` failure naming the file as spelled.
`tsparse` builds and runs only on macOS and Linux (ENVELOPE.md).
See [the design](reference-service-design.md) for ownership and consistency limits.

## Library

`ProjectSession::new(config)` creates a lazy refreshable session.
`session.execute(ProjectRequest::References(query))` validates or rebuilds and
returns either `ProjectResponse::References` with the usual result or an
`analysis::AnalysisError` (`AnalysisError::Input` / `AnalysisError::Query`).
The session has this one
request method for every operation (`References`, `Outline`, `Show`), so each
one refreshes the same way; the CLI's direct queries and the service both
answer through it. Configuration
paths are fixed at construction; relative query paths use the caller's current
directory. `session.generation()` counts successful publications. Failed refreshes
discard the old generation. An unreadable *previous* input forces a full load:
current selection decides whether that input is still required. Required input
failures are reported by the loader, never answered from stale state.

`Project::references(ReferenceRequest { query, filters })` and
`ProjectRequest::ReferenceSearch(request)` apply the reporting filters at the
shared lexical/semantic selection owner. `find_references` keeps its existing name/position
selection with default filters; both return the additive coverage and advisory
fields below. A legacy library name containing `.` keeps its exact lexical
question and does not gain containment selection.

`Project` retains its immutable, explicitly captured snapshot semantics:

```rust
use std::path::Path;
use typescript_parser::project::Project;
use typescript_parser::references::ReferenceQuery;
use typescript_parser::source::SourcePosition;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::load(Path::new("tsconfig.json"))?;
    let result = project.find_references(ReferenceQuery::At {
        file: "src/app.ts".into(),
        position: SourcePosition { line: 12, column: 8 },
    })?;
    for group in &result.groups {
        println!("{} occurrences", group.occurrences.len());
    }
    Ok(())
}
```

`load` reads each program input once and returns either a complete immutable
index or an `analysis::InputError` (`Io`, `Config`, `Syntax`, `ResourceLimit`
for a source nesting deeper than the parser's grammar budget, `Unsupported`,
`Binding`, or `UnresolvedImport`) with a path and position where available. A
program input longer than 4,294,967,295 bytes (`u32::MAX`), or whose control
flow has more nodes or containers than that, is `Unsupported`: that is this
tool's limit, not a TypeScript rule. Only a path
that does not exist, or that runs through a file, is absent. Any other failure
to inspect or read a file or directory, such as a package manifest, a type
root, or a directory entry, is an `Io` error. Queries read
no files: a query path is made absolute against the working directory and
matched lexically against each searched file's canonical path, the paths
through which loading reached it, and, for a file under the configuration
directory, its path under that directory as `--project` spelled it (such as a
symlink). A query path names a file only by its exact text: one that is not
Unicode, which a lossy conversion could turn into another file's name, is
`AnalysisError::Input` with kind `Unsupported`, never a file, for every
operation. Results reflect the captured contents even if files change
afterwards. Queries fail with `AnalysisError::Query` (`OutsideProject`,
including an empty path, `InvalidPosition`, `UnsupportedTarget`, or
`UnresolvedTarget`), or with `AnalysisError::Input` for an unrepresentable
path or a working directory that cannot be read (`Io`). A `QueryError`'s
`path` is the file as the query spelled it for `OutsideProject` and
`InvalidPosition`, and otherwise the selected token's file as a canonical
absolute path, whichever entry point selected it. Both error kinds serialize
as their variant names. Loading and querying share
normalization and the captured filesystem case policy; parent traversal is
clamped at an absolute path's root. Results own their data and
can outlive the project.

## Result

Without `--json`, an answer is text:

```text
/abs/path/tsconfig.json: `run` 1 group, 4 occurrences in 2 files; coverage lexical, complete
group 1: run, declared at /abs/path/src/api.ts:1:17
  /abs/path/src/api.ts:1:17 declaration, export
  /abs/path/src/app.ts:1:10 import
  /abs/path/src/app.ts:1:17 import (start)
  /abs/path/src/app.ts:2:1 use (start)
```

The header is `CONFIG: SELECTION N groups, M occurrences in K files; coverage
BASELINE, STATUS`, the selection being `` `NAME` `` or `at FILE:LINE:COLUMN:`.
When a FILE operand narrowed a name's candidates it adds `; candidates
narrowed to FILE`, so no groups there does not read as no references, and the
filters given add `; filtered by --calls, --no-tests, --file FILE`, so a count
they bounded does not read as every occurrence. Each
group follows as `group N: NAME, declared at FILE:LINE:COLUMN, …`, named by
its first declaration, then its occurrences as `  FILE:LINE:COLUMN ROLES`,
those of one file together, files in the order the result first names them
and each file's in the result's order. An occurrence spelled other than its
group, such as a renamed import or export alias, adds its spelling in
parentheses. A partial result then prints each gap as `gap FILE:LINE:COLUMN:
REASON: MESSAGE`, each located cause under it as `  cause FILE:LINE:COLUMN:
REASON: MESSAGE`, and `  … N more causes` for causes it omits. When a bare
name left catalog members unsearched, the last line counts them, `N catalog
members named NAME were not searched`, and names the search of one of them,
`; search SELECTOR with --at FILE:LINE:COLUMN`, followed by the request's
`--calls`, `--no-tests`, and `--file FILE`. The member is the first
suggestion with a selector (`search one with`, or `search it with`, when none
has one), and the position is its `search` (below), which selects that member
alone: a selector may name a static and an instance member alike, and a
parameter property's anchor is its lexical parameter. Followed as printed
after the same `--references --project CONFIG`, it searches that member with
the same filters. A suggestion without a `search` is not offered, and when
none has one the line names no search. Text names files absolutely, the
configuration's directory joined with the relative file, so every
`FILE:LINE:COLUMN` in a searched file is an `--at` operand from any working
directory; a declaration in an installed package or a standard library is
printed where it is, though no query selects a position there.

A dotted name that selects no declaration, or several, exits 1 with a
diagnostic on stderr: ``CONFIG: no declaration matches `NAME` ``, or ``CONFIG:
`NAME` matches N symbols; select one with --at FILE:LINE:COLUMN`` followed by
each candidate's anchor and selector, each anchor selecting its symbol. Each
adds `; candidates narrowed to FILE` after the name when a FILE operand
narrowed the candidates.

With `--json`, or indented with `--pretty` alone, the answer is its document:

```json
{
  "operation": "references",
  "outcome": "complete",
  "project": "/abs/path/tsconfig.json",
  "selection": { "name": { "name": "run", "file": null } },
  "groups": [{
    "declarations": [{ "name": "run", "location": { "file": "src/api.ts", "start": { "line": 1, "column": 17 }, "end": { "line": 1, "column": 20 } } }],
    "occurrences": [{ "name": "run", "location": { "...": "..." }, "roles": ["declaration", "export"] }]
  }],
  "filters": { "calls": false, "excludeTests": false },
  "coverage": { "baseline": "lexical", "status": "complete", "gaps": [] },
  "unsearchedMembers": { "count": 0, "selectors": [] }
}
```

`selection` echoes the request's selection, tagged `name` (`name`, and the
FILE operand or `null`) or `at` (`{"at": {"file": "src/app.ts", "position":
{"line": 12, "column": 8}}}`). Its file is named as every location's `file`
is, relative to the configuration's directory, so the echo is the same from
any working directory and through the service. A dotted name that selects
nothing or several answers `{"operation", "project", "selection", "outcome":
"not_found" | "ambiguous", "candidates"}`, whose `selection` is the same
echo; before the text form, it was tagged `Name` or `At` and named the file
as the request spelled it, made absolute.

Each group is one underlying symbol. `declarations` lists all of its
declarations, including merged declarations and declarations in installed
packages and standard libraries. `occurrences` lists the symbol's tokens in the searched files: its
declarations (`declaration`), references (`use`), and names in import and
export clauses (`import`, `export`). A token can carry several roles, such as
`export function run` (declaration and export). A token that denotes more than
one symbol, such as `export { Shape }` naming both an interface and a constant,
appears in each group.

File paths use `/` and are relative to the configuration's canonical
directory, with `../` where needed; `project` names the configuration in that
directory, so joining the two locates the file even when `--project` is
spelled through a symlink. Ranges are exact identifier tokens without trivia; `end` is
exclusive. Occurrences are ordered by file and position, groups by their
earliest declaration.

`complete` describes the existing lexical question and the effective filters.
It does not establish receiver/member identity, infer types, or certify all
TypeScript semantic references. Lexical queries produce no semantic coverage gaps and do not demand receiver or
member types. The shared request boundary still performs checker initialization.

For a bare name, `unsearchedMembers.selectors` lists matching method, accessor,
property and enum-member declarations from the captured Q2 catalog. FILE narrows
these declarations; `--calls` and `--no-tests` do not hide them. A position query
has an empty advisory. Each suggestion has `file`, `selector`,
`anchor: {file,line,column}`, and `search: {file,line,column}`, the position
whose position query searches that member, as a dotted selector naming it
alone would: the anchor, or for a parameter property its first
property-producing modifier, since its name selects the lexical parameter.
`search` comes from the selection owner (`reference_selector_anchor`), the
position ambiguity candidates' `anchor`s also name. Answers of
`Project::references`, and so of the CLI and its service, carry it;
`Project::find_references`, which makes no semantic request, leaves it out, as
it does for a member whose name has no reference token. It is the one field
these documents gained beside `selection`, by the run owner's decision
(question 37f47364) as an exception to keeping step 4's documents unchanged.
The key match is exact decoded text, including
empty and normalized numeric keys; it is distinct from selector containment.
Computed/unkeyed sites and inferred members absent from Q2 are not enumerated.

Suggestions are ordered by exact file, selector (null before strings), then
anchor. Entries with one file/selector share the earliest Q2 anchor, including
overload, accessor, merged and static/instance display duplicates. An
unspellable key or ancestor keeps `selector:null` and is deduplicated by
file/anchor instead; distinct anchors remain visible. `count` equals the emitted
suggestions. These are catalog suggestions, not a semantic identity merge.
Their anchors replay `--show --at`. In particular, a parameter property's Q2
name anchor still selects its lexical parameter for reference queries;
its first property-producing modifier is a semantic member anchor.

### Display limit

An answer shows at most 200 occurrences, the default that
`tsparse --references --help --json` publishes under `limits`, counted across
groups in the result's order: the first ones are shown, and the rest are
withheld. `--limit N` (a positive number) sets the bound in every channel,
and `--limit all` removes it; with `-o` and no `--limit` nothing is withheld,
so the file holds the complete result. The result itself is complete, and a
cut changes no outcome, coverage, gap, filter, member suggestion, or exit
code: the header still counts every group, occurrence, and file, every group
and its declarations are printed (a group whose occurrences the cut withholds
prints its header alone), each gap and the member hint follow, and a partial
result still exits 1. A cut text ends with one line, after the gaps and the
member hint:

```text
… 212 more occurrences; see all with: tsparse --references --project tsconfig.json --name run --limit all, or write them to a file with -o FILE
```

The command is the request as given, with `--limit all` in place of any
`--limit` value, rendered so a shell reads it back exactly; followed from the
same working directory it prints every occurrence, and it keeps a request's
`-o`, so it then writes the complete result to that file. With `--json`, or
with `--pretty` alone, a cut document is the result with each group's
`occurrences` cut, and after its fields a `display` that names the cut and the
same command:

```json
"display": {"next": {"argv": ["tsparse", "--references", "--project", "tsconfig.json", "--name", "run", "--json", "--limit", "all"]},
            "occurrences": {"returned": 200, "total": 412}, "truncated": true}
```

`display` is present only when something was withheld, so every answer that
shows all its occurrences is the document it always was. The command names
no transport: it leaves out `--direct`, since the service and a direct
load answer alike, so a cut answer is the same bytes from both. Ambiguous and missing dotted names list
their candidates in full.

Declarations that have no identifier use an anchor: an anonymous default export
(`export default function () {}`, `export default 42`) is named `default` at the
`default` keyword, `export = expression` is named `export=` at its `=`, a module file used as a namespace (`import * as ns`, `import x =
require()`, `export * as ns`) is named by its path at line 1, column 1, and an
ambient module (`declare module "x"`) is named `x` at its string literal.

## Go to definition

Definitions use the same configured program as references: compiler selection,
`baseUrl` and `paths`, JavaScript admission, and package-id redirects all apply.
An imported name in an admitted JavaScript file can lead to its original
declaration; a request explicitly naming a duplicate package file is refused
and names the primary. A project loading failure, including a JavaScript
capability refusal or the selected dialect's TS1540 error, prevents an answer.
The import hint on a missed `--show --name` prints a definition command at
the existing specifier; it does not generate a new module name.

`tsparse --definition` prints the declarations that the identifier at a
position, or a name, denotes, following import and export aliases to the
original declarations. It reads the declarations of the symbols the reference
selection selects, so its groups' declarations are exactly the ones a
reference search's groups list; the library operation is
`Project::definition(ReferenceQuery)`, also
`ProjectRequest::Definition(query)`, answering a `DefinitionAnswer` from
`typescript_parser::references`. The design is recorded in
[definition-and-import-hints-design.md](definition-and-import-hints-design.md).

```sh
tsparse --definition --project ./tsconfig.json --at src/app.ts:1:10
tsparse --definition --project ./tsconfig.json --at 1:10 src/app.ts
tsparse --definition --project ./tsconfig.json --name formatTitle src/app.ts
tsparse --definition --project ./tsconfig.json --name formatTitle --json
```

It runs only in project mode. It takes `--project`, `--direct`, exactly one of
`--at` and `--name`, `--json` (with `--pretty` to indent the document), and
`-o OUTPUT`; a value flag given twice, `--pretty` without `--json`, and any
other flag (`--limit`, `--calls`, `--file`, `--depth`, …) are usage errors,
as are stdin (FILE `-`, whether the operand or the file of `--at
FILE:LINE:COLUMN`) and two FILE operands. NAME is one identifier: a dotted NAME
selects a member by containment, which a definition does not follow, so it
is refused (`usage.invalid_value`, exit 2) with the command that selects it,
`next: tsparse --show --project TSCONFIG --name NAME`, `--project` as the
request spelled it, without FILE (which `--show` reads as narrowing the
declarations) or `--direct`. Through the library a dotted name is
`InvalidRequest` (exit 2).

### Definition selection

The reference selection owner decides what is selected, in the per-request
semantic worker `--references` uses:

1. `--at FILE:LINE:COLUMN`: FILE must be a searched file (`OutsideProject`,
   exit 2) and the position inside it (`InvalidPosition`, exit 2). A position
   in a member site whose identifier, if any, denotes no symbol is a **member
   position** (a property access such as `page.title` or `this.n`, a member's
   name, a literal member name or key such as `"q"` or `obj["q"]`, a parameter
   property's modifier such as `public`, a JSX attribute): the member is
   selected as `--references --at` selects it, and the answer is the `member`
   outcome, or the failure of selecting it.
2. Otherwise the identifier at the position gives its symbols, as
   `--references --at` does ([lexical selection](#lexical-selection)), or is a
   refused token (`UnsupportedTarget`) or an unresolved one
   (`UnresolvedTarget`). A name with a lexical target ends here even where its
   syntax is member access, as `formatTitle` in `lib.formatTitle` does.
3. Otherwise nothing is selected: `not_found`. That is a position in
   whitespace, a comment, punctuation, a keyword, a module specifier, a string
   specifier name, or the argument of a type's `import("./m")`.

`--name NAME [FILE]` selects every candidate spelled exactly NAME (decoded,
case-sensitive): declaration names, local import names, and exported alias
names, only those in FILE when it is given. So `--name useProjectFilesQuery
RegisterDocumentDialog.tsx` answers for a file that only imports the name:
its import specifier is a candidate. A name FILE only uses is no candidate;
`--at` selects such a use. A refused or unresolved candidate fails the request
with its error at its location. The selected symbols are deduplicated, so two
aliases of one symbol give one group and two shadowed declarations matched by
one name give two; nothing is ever ambiguous.

### Definition result

One group per selected symbol, listing its declarations (`Index::sites`) in
location order: the original declarations behind every import and export
alias, including declarations merged into the symbol and those in dependencies
and standard libraries. Groups are ordered by their first declaration. Every
group and declaration is printed; nothing is cut, so the operation takes no
`--limit`. Text, on stdout or `-o`, exit 0:

```text
/work/tsconfig.json: at /work/src/app.ts:1:10: 1 group, 1 declaration
group 1: formatTitle
  /work/src/lib.ts:1:17
```

With `--name` the header names the selection as `` `NAME` `` and ends
`; candidates narrowed to FILE` when FILE narrowed it. A group is named by its
first declaration; a declaration spelled otherwise shows its spelling in
parentheses: a namespace import of `src/lib.ts`, which `src/augment.ts`
augments with `declare module "./lib"`, lists `group 1: ./lib`, the
augmentation, then `/work/src/lib.ts:1:1 (src/lib.ts)`. Files are
absolute, so each line is an `--at` operand from any directory; a declaration
at a name token is also a `--show --at` operand, one at a synthetic anchor
(`default`, `export=`, a module's `1:1`) is not. `--json` writes the document
instead, files relative to the configuration's directory:

```json
{"operation":"definition","outcome":"complete","project":"/work/tsconfig.json","selection":{"at":{"file":"src/app.ts","position":{"line":1,"column":10}}},"groups":[{"declarations":[{"name":"formatTitle","location":{"file":"src/lib.ts","start":{"line":1,"column":17},"end":{"line":1,"column":28}}}]}]}
```

A **missing selection** exits 1, never an empty success, unlike
`--references`, which answers no groups: an agent must not read "no groups"
as "no declaration exists". The text is a diagnostic on stderr,

```text
/work/src/app.ts:1:1: no identifier is at this position; --definition follows identifiers, not keywords or string literals
/work/tsconfig.json: no declaration, import, or export matches `nothing`[; candidates narrowed to FILE]
```

and the document, on stdout or `-o`, is
`{"operation":"definition","project":…,"selection":…,"outcome":"not_found"}`.

A **member position** exits 1 with the search that finds the member's
declarations, `--references` at the same position, whose header names them.
It is rendered so a shell reads it back, with `--project` as the request
spelled it and no `--direct`, so the service and a direct answer print the
same bytes:

```text
/work/src/app.ts:4:29: `title` is a member, which --definition does not follow; find its declarations with: tsparse --references --project tsconfig.json --at /work/src/app.ts:4:29
```

```json
{"operation":"definition","project":"/work/tsconfig.json","selection":{"at":{"file":"src/app.ts","position":{"line":4,"column":29}}},"outcome":"member","name":"title","next":{"argv":["tsparse","--references","--project","tsconfig.json","--at","/work/src/app.ts:4:29"]}}
```

`name` is the member's decoded name; the library's `member` outcome carries
it, and the CLI adds `next`. The search repeats the position through the same
selection owner, so it selects the same member; it can still stop at a
semantic limit or report partial coverage, as any member search can. When
selecting the member fails, that failure is the answer and no search is
named: a member with no symbol, such as a property of `any`, is
``UnresolvedTarget`` (`` `foo` has no reference symbol at this site ``,
exit 1); a JSX attribute is `UnsupportedSemantic` with reason
`jsx_typing_required` (exit 1); and any character of a JSX namespaced
attribute such as `xlink:href` fails today with `Internal` (`canonical
semantic declaration association is not retained`, exit 2), as
`--references --at` does there.

Every other failure is delivered as a reference query's is: a refused token
(a label, `import.meta`, `new.target`, `arguments`, `undefined`,
`globalThis`, an intrinsic JSX tag, a tuple label, an import attribute, a
qualified member in a type position) is `UnsupportedTarget` and a name with
no declaration among the program inputs `UnresolvedTarget`, each
`FILE:LINE:COLUMN: message` on stderr or an `error` document, exit 1; a
semantic failure or limit while selecting, and every load failure, exit 1;
`OutsideProject`, `InvalidPosition`, `Io`, and `Internal` exit 2. The
[CLI reference](cli-reference.md#definition) lists what each exit code
reports.

The worker, its checker initialization, and the selection are those of
`--references` at the same selection; the answer then reads the index's
declarations and builds no occurrence list, so a definition costs at most
what the reference search costs, and on a warm service both are dominated by
the input check.

### Definition differences from TypeScript 7.0.2

A definition returns, for an identifier, the declarations of the symbols it
denotes after following import and export aliases. TypeScript 7.0.2 answers
`textDocument/definition` (`internal/ls/definition.go`) more broadly:

- **K1. Calls and `new`.** TypeScript adds the called signature: at a local
  or namespace-qualified call it answers the resolved overload alone, and at
  such a `new` the class declarations and the constructor without a merged
  interface; through an import alias it keeps every declaration and appends
  the called one; at `super(…)` it answers the base class and its
  constructor. Here no signature is selected: every call and `new` lists the
  symbol's declarations in location order, never a constructor, and `super`
  is a missing selection.
- **K2. Locations of declarations without a name.** An anonymous default
  export, `export =` of an expression other than a name, and a module used as
  a namespace keep the reference model's anchors (`default` at the `default`
  keyword, `export=` at the `=`, the module's path at `1:1`), where
  TypeScript starts at the declaration's first token, and at a file's first
  token after leading trivia. The declaration is the same.
- **K3. Unresolved modules.** TypeScript answers the import specifier itself.
  Here a module that does not resolve in a file TypeScript checks fails the
  load (`UnresolvedImport`, exit 1), and in a file it skips, such as one under
  `// @ts-nocheck`, the names it imports are `UnresolvedTarget` (exit 1).
- **K4. Members.** TypeScript follows a property of a value (`obj.method`,
  `this.n`), a member declaration's name, an enum member, a private name, a
  JSX attribute, a binding element's property name, an object literal key, a
  string or numeric literal member name or key, a parameter property's
  modifier, and an index signature. Here every position the reference
  selection sends to its member path answers `member`, or the failure of
  selecting the member, and a qualified member in a type position
  (`let x: E.A`) is `UnsupportedTarget`, as `--references --at` reports it.
- **K5. Keywords and labels.** TypeScript answers `this`, `super`, `return`,
  `await`, `yield`, `case`, `default`, `override`, and labels, maps a keyword
  before a declaration's name (`function`, `class`, an `export`, the `import`
  of `import x = require()`) to that name, and a class's `constructor` keyword
  to its constructor. Here a keyword is a missing selection, unless it is a
  parameter property's modifier (K4), and a label is `UnsupportedTarget`
  (`label`). Neither compiler answers the `import` of an import declaration,
  nor does TypeScript 7.0.2 answer the `static` of a class static block.
- **K6. File references and string names.** TypeScript answers a module
  specifier, the argument of a type's `import("./m")`, and a
  `/// <reference>` with their files, and a string-literal specifier name
  (`export { "f-g" } from`) with its target. Here each is a missing
  selection.
- **K7. The lexical model's differences.** Every difference of the lexical
  model applies unchanged ([loading and lexical
  differences](#loading-and-lexical-differences-from-typescript-702)), such as
  a name that two `export *` declarations supply with different targets being
  unresolved.
- **K8. Shorthand names and binding elements.** At the name of an object
  binding element without a property name that is not a rest element
  (`const { a } = source`), TypeScript answers the matching property of the
  pattern's type; here the name declares the binding and answers it. At a
  shorthand property name TypeScript answers the name's own declarations
  without resolving an alias, then the contextual type's property, so `{ b }`
  typed `Shape` adds `Shape.b`, and `{ formatTitle }` of an imported name
  answers the import specifier. Here a shorthand name is a use of its value:
  it answers the value's declarations, and an imported one follows its alias
  to the original declaration, as every other use of the import does. A use
  of such a binding, and a rest element's name, answer the same in both.

## Lexical selection

This section describes the lexical projection. Member declarations and uses
are handled by the semantic selection owner described in [types.md](types.md),
including dotted selectors and parameter-property modifier anchors.


A position must fall inside an identifier token, start inclusive and end
exclusive. Whitespace, punctuation, and keywords give an empty result. Member,
property, enum member, label, JSX attribute, intrinsic element, tuple label,
import attribute, `import.meta`, `new.target`, `arguments`, `undefined`,
`globalThis`, and `this` in a type query are `UnsupportedTarget`. A name with no declaration among the
program inputs is `UnresolvedTarget`.

A name query is exact and case-sensitive on decoded identifier text, so
`\u0061bc` and `abc` are the same name. Candidates are declaration names, local
import names, and exported alias names in the searched files; FILE restricts
candidates, not results. Every matching candidate's symbols are grouped, so two
shadowed declarations give two groups and two aliases of one symbol give one.
A candidate that is not a supported target, such as an import of a member or an
export of `globalThis`, makes the query fail with `UnsupportedTarget`, and an
unresolved candidate with `UnresolvedTarget`, exactly as a position query on
that token would, rather than silently omitting it.

## Binding model

Each scope maps a name to at most one symbol per meaning (value, type, and
namespace) plus an optional import or export alias. A reference asks for the
meaning its syntax requires and searches outward, so a type alias and a constant
with the same spelling remain separate groups and a type does not shadow a
value. Declarations that TypeScript merges become one symbol: function
overloads, interfaces, class and interface, namespace with namespace, class,
function, or enum, enums, and redeclared `var` and parameters. A namespace is
also a value only when TypeScript counts it as instantiated: when its body
holds anything other than interfaces, type aliases, uninstantiated namespaces,
non-exported imports, and `export { ... }` of such declarations. A re-exported
name declared by an `import =`, or by no enclosing block, counts as a value. So
a namespace of types shares its name with a value instead of conflicting with
it. A module object, which a namespace import, `import = require`, or `export
as namespace` names, is a value and a namespace; an `import =` alias of an
entity name takes every meaning its target has.
Conflicting declarations, such as two `let x`, fail with a `Binding` error
where the error is reported. One decision says whether a diagnostic at a
place is reported, for every binding error, every error TypeScript's checker
reports from a file's syntax alone (a namespace declared with `module`,
TS1540, a `Syntax` error at its name), and every missing input (an
unresolved module, reference directive target, or augmented module): it is
reported only in a project file, and only where TypeScript reports it there.
TypeScript reports nothing in a file it skips (`SkipTypeChecking`): every
file under `noCheck`, a declaration file under `skipLibCheck`, a standard
library under `skipDefaultLibCheck`, a JSON file, and a file whose last
`// @ts-check` or `// @ts-nocheck` leading comment is `@ts-nocheck`; nor on a
line that a `@ts-ignore` or `@ts-expect-error` comment suppresses (below). A
dependency or standard library file is never reported here, whatever
TypeScript would report: its declarations bind as they allow, so conflicting
declarations in one scope merge, and a name that depends on a missing input
is unresolved. Where TypeScript reports one diagnostic at several
declarations, such as a duplicate declaration at each, any reported one fails
the load; where none is reported, a conflict in one scope merges, and across
files TypeScript's choice stands (below). Libraries come first, so a project
declaration that conflicts with a library global, such as `let Array` in a
script, fails where the project declares it. Explicitly selected roots are
project files even inside `node_modules`. This decision is separate from
whether a file's occurrences appear in results.

Scopes follow TypeScript: function-scoped `var` and block-scoped `let`, `const`,
`using`, classes, and strict-mode functions; separate parameter and body scopes,
so a parameter default cannot see body declarations; class and interface type
parameters shared by merged declarations; mapped type parameters; and `infer`
declarations visible only in their conditional type's true branch. Namespace
blocks see each other's exports but not each other's locals. Script files share
the global scope with `declare global` blocks and UMD globals. Decorators
resolve where their class is declared.

The global table is built as TypeScript's `initializeChecker` builds it, in
program order. Each script's top-level declarations, bound together by the
binder's rules, merge into it file by file as the checker merges symbols
(`mergeGlobalSymbol`); after each file, its UMD globals (`export as
namespace`) join only where no global has their name yet, so a script
namespace declared before a UMD file keeps the name and the UMD global is
dropped. A UMD global is declared only at the top level of a module that is a
declaration file; elsewhere TypeScript declares nothing and reports TS1316
(not at the top level), TS1314 (in a script), or TS1315 (not a declaration
file), a `Binding` error where reported. Then each file's `declare global`
blocks merge into it by the same merge as module augmentations (below): a
declaration meeting an alias, such as a UMD global or an `export import`
inside a namespace, merges with what the alias names. Last, as TypeScript
defers them, ambient modules declared in more than one script merge: each
later file's declarations of the name, bound together, merge into the module
of that name in program order by the same merge, so an `export import` alias
in one file and an interface in a later one merge into what the alias names,
while the alias after the interface conflicts with it (TS2440), as within one
file. The module's first declaration in that order decides whether it is
shorthand: after `declare module "x";`, every name imported from `"x"`
denotes the module, even a later file's export. Pattern ambient modules
declared in several files merge the same way. Excluded pairs across files,
such as `let x` in two scripts, are a `Binding` error where TypeScript
reports TS2451 or TS2300 at a reported declaration of either side; where
neither side is reported, TypeScript's choice stands and the earlier
declaration keeps the name.

Aliases are directed edges. An import, re-export, `export =`, `import =`
alias, `export as namespace` global, or `export * as` name resolves to the
underlying symbol, following star re-exports (local exports shadow them, and
different targets for one name are ambiguous) and `export =` targets. A
type-only alias does not supply a value outside type positions, so
`import type { C }` followed by `new C()` leaves `C` unresolved. Restrictions
belong to each resolved meaning: a same-named local value beside an imported
type remains usable through ordinary re-exports, while an explicit type-only
re-export restricts that value too. `ns.name` and
`N.Type` resolve through module and namespace exports; other property accesses
are not reference targets. Ordinary value flow, such as `const copy = run`,
creates no alias.

A `declare module "m"` block is an augmentation where TypeScript's
`IsModuleAugmentationExternal` says so: at the top level of a module, or, with
a non-relative name, directly inside an ambient module, or a `declare global`
block, at the top level of a script. At the top level of a script it is an
ambient module, and a relative name there is a `Binding` error where
reported (TS2436). Anywhere else it is a `Binding` error where reported
(TS2435) and merges nowhere, so in a dependency it does not shadow a package;
a relative name directly inside a script's ambient module or `declare global`
block merges nowhere either, without an error. As TypeScript's parser lists
modules (`collectModuleReferences`), a string-named module counts only in an
ambient context: declared, inside a declared module, or in a declaration
file. Otherwise it is a `Binding` error where reported (TS1035) and is no
augmentation, though at the top level of a script it is still an ambient
module. The same placement rule applies to `declare global`: it merges into
the global table only at the top level of a module or directly inside a
script's top-level ambient module; elsewhere it is a `Binding` error where
reported (TS2669), and otherwise its names stay local to it.

A file's augmentations of one module name form one symbol whose declarations
bind into its own table, as in TypeScript's binder. Once every other
declaration is bound, each merges into what it augments, file by file in
program order, as TypeScript's `mergeModuleAugmentation` and `mergeSymbol` do:
one merge, applied recursively to the namespaces it declares and to their
exports. Each declaration merges with the module's export of its name: a
declared export; the target of an export alias (`export { X }`,
`export type { X }`, `export { Y as X }`, `export { X } from`, or an exported
import, also inside a namespace), which then becomes the module's own export
without the alias's type-only restriction; or the symbol a non-ambiguous
`export *` supplies. The original declaration, every augmentation, the export,
and its consumers form one group; merged interfaces and classes share their
type parameters, and a meaning the merge adds, such as a class's value, is
visible wherever the original declaration is. An export of a shorthand ambient
module names the module itself, so a declaration merged through it gives the
module that meaning: every import of the module and its module object then
have it. An export that names a JSON module's value or property merges as the
property TypeScript resolves it to: a declaration that excludes a value, such
as a `const` or an instantiated namespace, conflicts with it (TS2451, TS2300),
while an interface or a namespace of types merges beside it, so the name keeps
denoting the value in an expression. Excluded pairs, such as a type alias
augmenting an interface, are a `Binding` error where TypeScript reports them
at a reported declaration of either side; where neither is reported,
TypeScript's choice stands: the augmentation replaces an export alias it
conflicts with, and the original declaration stays where the augmentation
conflicts with the declaration itself. A module whose `export =` names a
namespace, module, or enum is augmented through that target; any other target
(TS2671), or, when the file's augmentations of that module declare values, a
target that also declares a variable (TS2649) or is a const enum (TS2567), is
a `Binding` error where reported, and otherwise binds nothing. As in
TypeScript, an augmentation's name adds no file to the program: its target is
a program input only when another route includes it. Augmenting a module that
is not in the program (TS2664) or a script (TS2306) is an `UnresolvedImport`
error, decided once for both: not where the augmentation's parent is ambient,
in a declaration file or inside a script's ambient module, nor where the error
is not reported, as for a missing import. There the augmentation binds
nothing.

An augmentation of a specifier that only a pattern ambient module (`"*.css"`)
matches extends that specifier alone, one way: it has the pattern's and the
augmentation's declarations, while other matching specifiers, and the
pattern's own declarations, keep only the pattern's. The specifier gets a
module of its own at that augmentation's turn, as TypeScript's
`mergeModuleAugmentation` makes it: a merge before it, such as one through
another module's `export * as X from "x.css"`, merged into the pattern itself,
so every matching specifier has what it added. And once any merge has merged
into the pattern module, including a pattern declared in several files, an
augmentation of a matching specifier merges into the pattern both ways, as
into any module, because TypeScript's resolved module is then no longer the
pattern's own. The specifier's module is shorthand when the pattern is and
the augmentation declares no values. While augmentations merge, the
specifier's module already exports everything the pattern does, so an alias
resolved meanwhile, such as another module's `export { X } from "x.css"`,
finds the pattern's declaration. An alias the pattern declares
beside the augmentation's declaration stays an alias declaration of the
merged name, as in TypeScript: its group lists what the alias names, but no
compatibility check compares that declaration, and its type parameters stay
its own. The pattern's own type parameters stay its own too, so the merged
interface has the augmentation's parameters followed by the pattern's, and
each declaration must list a prefix of them that includes every parameter
without a default (TS2428): `interface X<T>` in both is a `Binding` error.
That check, as every merge check, reads the declarations of the symbol a
checked declaration denotes, so a project pattern that a dependency's
augmentation merges into one way is not checked against it.

Aliases and qualified references resolve during loading in dependency order,
with an explicit task stack. As in TypeScript, a name with local declarations
resolves to them without following a same-named import for meanings they lack,
so an alias cycle that returns through such a name resolves to its locals;
the import then conflicts with the local declaration (TS2440), a `Binding`
error where reported. Every alias in a cycle of aliases alone (TS2303) is
unresolved rather than an error. Either way the result does not depend on which file
resolution reaches first.

## Project configuration

`tsconfig.json` and JSON modules are read as TypeScript reads both, with one
JSON reader (`parseJSONText`): its scanner's comments, ending at any ECMAScript
line terminator, trailing commas, a leading `#!` line, its numbers (`0x10`,
`1_000`, `.5`, and a leading `-`, but not `01`, `08`, or `1n`), and
JavaScript string escapes such as `\x41` and `\u{41}` (but not `\01` or
`\8`); keys and string values must be double-quoted, and any other value or
member, such as `+1`, `NaN`, or a method, is a syntax error. A string with an
unpaired UTF-16 surrogate keeps it, so it equals no other name; one that must
name a file, module, option value, or `paths` pattern is `Unsupported`. A
configuration's text may be empty or only comments, which is an empty
configuration; any other root must be an object, or an array holding one,
which TypeScript reads in its place. A `?` after a key is an error in a
configuration (TS8009), though not in a JSON module. A repeated key keeps its
first position and its last value, and a property with an empty name is left
out, so `"paths": { "": ... }` declares no pattern; a JSON module keeps such a
key. A `null` entry of `files`, `include`, `exclude`, or an `extends` list is
left out, and such a list of only `null`s is no list at all: `"files": [null]`
or `"include": [null]` selects as if the list were absent, so the default
`include` applies, a base's `[null]` is not inherited, and `"extends":
[null]` names nothing. TypeScript 7.0.2 drops `null` entries from every
array alike, which this tool does not yet do for compiler options: there,
`"lib": [null]` selects no library, where 7.0.2 selects the default ones, and
a `null` `paths` substitution is a `Config` error, where 7.0.2 ignores it. Every occurrence of an option this tool reads is validated, as TypeScript
validates each one it converts: a repeated key, each `compilerOptions` object
of a file, and each file of the `extends` chain, so an invalid value that a
later one overrides is still a `Config` error. Within a file, an option takes
the last occurrence that sets it, across all its `compilerOptions` objects: a
`null`, or an empty string for an enumeration such as `module`, sets nothing
and leaves an earlier value, while a list's `null` clears it. A file with
`extends`, even an empty list, takes each base's options in order and then
its own; a setting that leaves the option unset, such as a list's `null` or an
empty `jsxImportSource`, does not replace an inherited value, but a `null` as
the last value in the file's last `compilerOptions` object unsets the option,
as a base's does over the bases before it. `extends` itself may not be
`null`. `package.json` is read as strict
JSON, as TypeScript 7 reads it: a manifest with a comment, trailing comma,
single quote, other escape, raw control character, non-ASCII whitespace, or
unpaired surrogate is unparsable, so its fields are ignored while it still
marks a package scope. A configuration, manifest, or JSON module nested more
than 128 arrays and objects deep is `Unsupported`: that is this tool's limit,
not an unparsable file. `extends` accepts a
string or an ordered array of relative, rooted, or package configurations
(through `node_modules`, a manifest `tsconfig` field, `exports`, or a package's
`tsconfig.json`), with inheritance cycles reported as configuration errors.
Path settings stay relative to the configuration that declared them, unless a
path-valued option (`rootDir`, `outDir`, `declarationDir`, `rootDirs`,
`typeRoots`, a `paths` substitution) or a `files`, `include`, or `exclude`
entry starts with `${configDir}`, matched without case as Go's
`strings.ToLower` lowers it: it then names a path under the selected
configuration's directory once its first exact `${configDir}` becomes `./`, so
`${configDir}src` is `src` there and `${CONFIGDIR}/src` is
`${CONFIGDIR}/src` there. Other options, such as `jsxImportSource`, `types`,
or a `paths` pattern, keep the text. A `paths` substitution is checked as
substituted, as TypeScript 7.0.2 checks it after normalizing it: its `*`
count (TS5062, which names the substituted path) and whether it is relative
are the path's, so `${configDir}/x*/../src/*` has one `*`, and a `*` in the
configuration's directory counts. `files`, `include`, and `exclude` replace inherited values: a
configuration that has the key at all, even as `null`, inherits none, and a
`null` then selects as if the key were absent; otherwise the last base whose
own value, after its own inheritance, is a list supplies it.

Roots follow TypeScript's selection: `files`, `include` and `exclude` globs,
`foo.ts` hiding `foo.d.ts`, and default excludes of `outDir` and
`declarationDir`. Program inputs add every resolved import, re-export,
`import = require`, import type, dynamic `import("literal")`, module
augmentation, `/// <reference path>`, `/// <reference types>`, and `types`
entry, JSON files included, plus the automatic JSX runtime of each `.tsx` or `.jsx` file under `jsx`
`react-jsx` or `react-jsxdev`, from `jsxImportSource` or a block-comment
`@jsxImportSource` pragma (`@jsxRuntime classic` turns it off). Searched files are the roots and their local dependencies; files reached
through packages or type packages are program inputs whose declarations
resolve names but whose own occurrences are not reported.

Program order, which orders the global table's merges, pattern ambient
modules, and module augmentations, follows TypeScript 7's (`collectFiles`, as
`tsc --listFilesOnly` prints it): starting from the roots, the root
libraries, and the `types` packages, each file follows the files its
subtasks reach, depth first, and libraries come first. A file's subtasks are,
in order, its `/// <reference path>`s, its `types` and `lib` references, the
automatic JSX runtime, its imports and re-exports, then its dynamic imports
and import types; within each group they follow source order, wherever they
appear (an `else` branch, a call's arguments, a type annotation), as
TypeScript finds them in the text. The TypeScript 5.6 dialect retains the
JSX-runtime admission difference described under
[Standard libraries and program order](#standard-libraries-and-program-order).
A module augmentation's name adds no file.
On unchanged Command Center, the 7,639 inputs this selects, their order, and
their provenance (library, dependency, or searched) are the pinned compiler's
(`tools/program-inputs.mjs`, see [validation](validation.md#command-center-checkpoint)).

Standard libraries follow TypeScript 7.0.2. A project with root files and
without `noLib` loads the default library of its `target`, or else each `lib`
entry; `es6` and `es2015` select `lib.es6.d.ts`, and an empty
`lib` list selects no library. `/// <reference lib>` directives in any file,
libraries included, add the libraries they name, so selection is a transitive
closure; `noLib` ignores every such reference. `lib` entries are library
names in any case, not file names. An unknown or empty library reference is
an `UnresolvedImport` at its value, tolerated where other missing references
are: where the reporting decision (Binding model) does not report it, as in a
library or dependency, a declaration file under `skipLibCheck`, or after a
`@ts-ignore` or `@ts-expect-error` line.
`no-default-lib="true"` only makes TypeScript ignore its whole directive; it
does not affect library selection.

An unset, null, or empty `target` is ES2025 (`lib.es2025.full.d.ts`), and a reference also accepts the file a name
selects, such as `lib.es2015.promise.d.ts`. With `libReplacement`, each
library file except `lib.d.ts` looks up its `@typescript/lib-*` package as a
`require` from the configuration's directory (`lib.dom.iterable.d.ts` is
`@typescript/lib-dom/iterable`), and an installed package replaces the file.
The declarations are the pinned compiler's own, vendored in
`vendor/typescript-7.0.2-lib` and located through the checkout the tool was
built from: no compiler is searched for or run, nothing is downloaded, a
missing or unreadable library file is an `Io` failure, and moving the checkout
requires rebuilding.

Library paths in results are real, readable files. Libraries come first in
program order, ordered as the compiler orders them, and are read as CommonJS scripts without a `package.json`
lookup. They are never searched and never external, even when a replacement
package supplies them: their declarations join groups, so `interface Array`
merges with a project's `declare global { interface Array<T> { ... } }`,
while their own occurrences are not reported and their bindings are checked
leniently, like dependencies. Library reads and replacement lookups are
captured inputs, so a session refreshes when an installed replacement
changes.

Module resolution implements TypeScript 7's `bundler` (the default), `node16`,
and `nodenext` modes: importer format, per-request resolution mode, extension
substitution, directory and extensionless rules, `paths`, `rootDirs`,
`moduleSuffixes`, `node_modules` and `@types` lookup, package `exports`,
`imports`, `typesVersions`, `types`/`typings`/`main`, self-name imports,
`customConditions`, and `resolution-mode` on type-only imports and import
types. Relative specifiers, paths, and directives accept `\` as a separator.
A cyclic package `imports` chain is unresolved, and one longer than 1,024
redirects fails loading as `Unsupported` wherever it appears, since that is
this tool's limit rather than an input TypeScript rejects. A name with a NUL
character names no file, so it is not looked up on disk. A file's identity is its real path unless `preserveSymlinks` is set, so
a file reached through a symlinked directory is one input where TypeScript can
create two. Exact ambient modules (`declare module "x"`) win over files for
non-relative names, and pattern ambient modules (`"*.css"`) satisfy any
otherwise unresolved name: the one with the longest text before its `*`, the
first declared on a tie, in program order, as in TypeScript's
`findBestPatternMatch`. A name with more than one `*` is no pattern.
An `export *` route whose export of a name fails to resolve still exports it:
alone, its failure is the result; beside another route, the name is
ambiguous, as TypeScript reports TS2308.

A file's triple-slash `<reference ...>` directives, `@ts-check` and
`@ts-nocheck` pragmas, and JSX pragmas follow TypeScript 7: only spaces and
tabs separate their parts, a `<reference` tag needs no closing `/>`, the last
of a repeated attribute wins, and a present `types`, `lib`, or `path`
attribute, even an empty one, decides the directive in that order. A
`@ts-check` or `@ts-nocheck` pragma is a leading `//` or `///` comment whose
text after spaces or tabs is `@` and that name, matched without case and
ending before any character other than a letter or `-`; the last one decides,
and a block comment is none. A `#!` line may follow a byte order mark.

An enumeration option (`module`, `moduleResolution`, `moduleDetection`, `jsx`,
`target`, and each `lib` entry) is matched in any case, after Go's
`strings.ToLower`, and an empty string leaves it unset; an unknown value is a
`Config` error, as are `lib` with `noLib` (TS5053) and a `lib` file name in
place of a library name.

`types` defaults to `[]`, as in TypeScript 7.0.2, though this tool's design
first assumed automatic discovery when `types` is absent: list packages, or
use `"types": ["*"]` for every type root. A type name is looked up in each
type root (`resolveTypeReferenceDirective`): under an explicit
`typeRoots`, an empty list included, first as a declaration file
(`types/name.d.ts`) and then as a package directory, and under the default
`node_modules/@types` roots as a package only. A name the configuration
infers, a `types` entry or an automatic name, is looked up nowhere else when
`typeRoots` is explicit (TS2688), while a `/// <reference types>`
directive's name, and any name under the default roots, then falls back to
`node_modules` above the file that names it. The two origins are separate
lookups, as in TypeScript 7.0.2's cache. `moduleDetection` defaults to `force` for Node.js `module` settings
and `auto` otherwise. Options that do not affect the program graph or binding
are ignored.

A file that resolution finds for a module request is a program input, and
what the request names, only where TypeScript admits it for the importer
(`GetResolutionDiagnostic`): a `.tsx` file needs `jsx`, a JSON file
`resolveJsonModule`, and a declaration file found for an arbitrary
extension, such as `styles.d.css.ts` for `./styles.css` or `data.d.json.ts`
for `./data.json`, `allowArbitraryExtensions` or a declaration-file importer.
Otherwise the request is unresolved (TS6142, TS7042, TS6263), even where
another route makes the file an input, and a pattern ambient module such as
`"*.css"` may still satisfy it; the only exception is a `.tsx` file that is an
input by another route, such as a root, which the request then names. The
same file named with its own extension, as `./styles.d.css` or a package's
`types`, is an ordinary declaration file. An empty module specifier is never
resolved.

JSON modules are program inputs under `resolveJsonModule`, whose default
follows TypeScript 7.0.2: on for `module` `node20` and `nodenext`, off for
`node16` and `node18`, and otherwise on with bundler resolution, the default.
Then a `.json` specifier resolves to `<name>.d.json.ts`, then `<name>.json`,
including through `paths`, package `exports`, `main`, and subpaths; an
extensionless specifier never adds `.json`. A `<name>.d.json.ts` found first
is admitted as above, so without `allowArbitraryExtensions` a source file's
request is unresolved rather than naming `<name>.json`. A `files` entry or
`/// <reference path>` naming a `.json` file must then exist; without the
option, its extension is unsupported, as for `.css`, and a `.json` specifier
is unresolved, so a pattern ambient module such as `"*.json"` may satisfy it.
An `include` spec selects JSON files only when it ends in `.json`, after the
other matches. A JSON file is always a module and is parsed as JSON by its
extension in any case; it declares no names, its own `/// <reference>`
directives are followed, and, since TypeScript checks no JSON file, a missing
target there is not an error. Its only export is `export=`, its value: a
default, namespace, `import =`, or `{ default as x }` import, even beside a
`"default"` key, names that value. Any other name selects a property of the
value's type, as a named import, a re-export, or a qualified name does
through a module whose `export =` is the value or a property of it
(`getExternalModuleMember`): a key of an object, compared exactly, including
an empty key, or, with standard libraries, a member that the global `String`,
`Number`, `Boolean`, or `Array` interface declares for a string, number,
boolean, or array, without `Object`'s members. The value, its properties, and
such members are values only, not reference targets, so queries at those
names are `UnsupportedTarget`; a name the type lacks, a name of `null` or of
an empty file, a JSON value used as a type or namespace, and an import type's
`default` (`import("./data.json").default`) are unresolved. Two `export *`
routes to one property or member are one export; to different ones, the name
is ambiguous. `export *` from a JSON module re-exports nothing, and an import
of a JSON value beside a same-named local value is a `Binding` error
(TS2440). A JSON file is searched, but its keys are string literals, so no
occurrence lies in it, and explicitly navigating one is unsupported. A JSON
file that does not parse fails loading as a `Syntax` error at the first error
this tool's parser meets, which is TypeScript's first diagnostic except where
noted below.

Rejected configurations: `baseUrl` (removed in TypeScript 7), `moduleResolution` `node`, `node10`, or `classic`, `module` `amd`, `umd`, or
`system`, `target` `es5` (removed in TypeScript 7, TS5108), `noResolve`,
`noDtsResolution`, a non-empty `references`, and, without `allowJs`, JavaScript
roots or reference targets, whether or not the file exists (TS6504). A message that reports a setting the
TypeScript 7 profile removed also names an installed compiler of another major
version, since the profile's rule is not that compiler's: `Option 'baseUrl' was
removed in TypeScript 7, whose rules read this project (installed compiler:
TypeScript 6.0.3); ...`. As in TypeScript, whether an input has a
supported extension follows the file system's case policy, so `A.JS` is
JavaScript on a case-insensitive file system; other name rules use the exact
spelling, so `G.D.TS` is an ordinary module rather than a declaration file, and
JSX parses in a `.tsx` or JavaScript file in any case (see
[JavaScript inputs](#javascript-inputs)).

### Package identity

Both compilers give a file found in a package a package id: its manifest's
`name` and `version`, the file's path in the package, and `+name@version` for
each of the package's `peerDependencies` whose manifest is in the
`node_modules` beside the package's real directory (`withPackageId`,
`getPackageId`, `readPackageJsonPeerDependencies`). Files with one id are one
program input. Collection walks the program depth first (`collectFiles`) and
keeps the first file it meets with an id; a later file with the same id
redirects to it. A request that resolves to the redirected file names the
kept file, and what only the redirected file reaches leaves the program, with
its failures: a syntax error, an unreadable file, a lookup that cannot read
a `package.json`, or a missing reference there fails no load, as neither
compiler reports anything of it. A failure is the failure of the file it
belongs to, whether of the file's own text or of a lookup the file makes
(another file making the same lookup meets it too), and loading reports the
first such file in load order that stays in the program; a failure of the
roots, the standard libraries, or the type packages themselves comes first.
Both copies are still read and parsed. TypeScript 7.0.2 deduplicates unless
`deduplicatePackages` is false; TypeScript 5.6.3 always does, and has no such
option (TS5023). So a class with a protected member, or a unique symbol, that
two installed copies of one package declare is one declaration, as both
compilers check it, and references and types through either copy reach the
kept file. Which copy is kept depends on the order imports are written, not
on where the copy is installed: with `import "a"` before `import "dup"`, the
copy of `dup` inside `a` is the one kept.

Which routes give a file an id, and what the id holds, follow each dialect's
compiler:

- The `name` and `version` are TypeScript 7.0.2's `Expected[string]`
  decodes: a repeated field keeps its last string through a later value of
  another type (`"version": "1.0.0", "version": 42` is `1.0.0`) and loses it
  to a later `null`, and an id with an empty `name` is no id. TypeScript 5.6
  reads the last value of each, which must be a string, an empty `name`
  included. Resolution itself reads a repeated field's last value in both
  dialects.

- In both, a package lookup's file (`node_modules`, `@types`, `exports`,
  `imports`, a self-name, `typesVersions`, a nested `package.json`) has the
  id of the manifest the lookup read. So does a file that a relative name, a
  `paths` substitution, a `baseUrl` name, `rootDirs`, or a type root finds
  inside `node_modules`: the manifest is that of the package directory its
  path is in. A `paths` or `typesVersions` substitution written with its
  extension gives no id (`tryLoadModuleUsingPaths` returns its file as
  found).
- Under TypeScript 5.6 only, a file found through a directory's manifest
  entry or index also takes that directory's id (`loadNodeModuleFromDirectory`):
  a relative or mapped name of a package directory, and a `/// <reference
  types>` or `types` name resolved in `node_modules/@types`, give one.
  TypeScript 7.0.2 gives none there, so such a copy is not deduplicated.
- TypeScript 7.0.2 keeps the first id any route to the file gives;
  TypeScript 5.6.3 decides when it first reaches the file, depth first, by
  that route's id, so a file first reached by a `/// <reference path>` is
  never deduplicated under TypeScript 5.6.
- Peers: TypeScript 7.0.2 reads `peerDependencies` only as a map of strings,
  in sorted order; TypeScript 5.6.3 reads any object in the order the
  manifest lists it (array indices first, as JavaScript orders keys), so two
  copies listing the same peers in different orders differ under
  TypeScript 5.6 only. A peer's version is its last decoded string under
  TypeScript 7 (`Version.Value`, so `"18.0.0"` followed by `true` stays
  `18.0.0`, and none adds `name@`) and the last value's JavaScript text under
  TypeScript 5.6 (`name@undefined` without one, `null` and booleans as their
  text; another value is `Unsupported`, naming the manifest, since its
  JavaScript text would merge distinct versions). The
  `node_modules` is the last `/node_modules` of the package's real path
  (TypeScript 7; none means no peers) or its last `node_modules` (TypeScript
  5.6; none means the path's first eleven UTF-16 units, as 5.6.3's
  `lastIndexOf` gives). The real path is taken under `preserveSymlinks` too.
- Ids are equal when their parts are (TypeScript 7.0.2's `PackageId`) or
  when their text is (TypeScript 5.6.3's `packageIdToString`).

Whether a kept file is a project file, found without a package lookup, follows
the routes each compiler counts. TypeScript 7.0.2 gives every file it loads
the lowest depth of any route to it before collection deduplicates
(`lowestDepth`), so a redirected file's own references count: where a root's
`/// <reference path>` reaches a redirected copy that references
`node_modules/shared/index.d.ts`, `shared` is a project file. TypeScript 5.6.3
counts only the routes it processes (`sourceFilesFoundSearchingNodeModules`),
and never processes a redirected file's own references and imports, so there
`shared` stays a dependency, which project queries do not answer for. When
such a route, or a root, reaches a redirected file that a package lookup met
first, TypeScript 5.6.3 processes it again, with its target's references and
imports, resolved from the redirected file's location and in the target's
module format (the redirect shares its target's parsed file): a target's
`/// <reference path="./note.d.ts" />` then adds the redirected copy's own
`note.d.ts` as a project file, a type reference finds `@types` packages
beside the redirected copy, and an import of `./note` makes that copy's
`note.d.ts` another redirect, to the target's. This tool follows them where
5.6.3's collection meets the redirected file, and reports a missing reference
there at the redirected file, as 5.6.3 does unless `skipLibCheck` skips it; a
`package.json` such a lookup cannot read fails the load at once, before the
failures of other files in load order. What a module augmentation names, a
file it never adds, is decided once collection adds no more files, so an
augmentation of the redirected copy's `note.d.ts` augments it under 5.6.
TypeScript 7.0.2 never processes a redirected file's subtasks. A request on
such a route whose JavaScript is elided is not followed again later, as
5.6.3 would within `maxNodeModuleJsDepth`.

The program lists each input once, as TypeScript 7.0.2's `--listFiles` does;
TypeScript 5.6.3 also lists each redirected file, which
`tools/program-inputs.mjs --oracle=5.6` drops before comparing. A module
named in generated text, such as `import("dup").Box`, is named through the
redirected files too (`GetEachFileNameOfModule`), so a kept copy nested in
another package can be named `dup` from the project.

Every request that names a redirected file, by its path or a spelling it was
reached by, is `Unsupported` (exit 1), whichever operation it is: its message
names the package id and the file the program reads in its place. That file
has the other copy's text, so answering for it, or editing it, would act on a
file the request did not name.

A session reloads when an id's inputs change: the package's manifest, each
peer's manifest, present or absent, the package directory's real path,
links included, and either copy's text.

## Compiler selection

The compiler installed for a configuration selects the rules that read its
project. Selection is this tool's policy, not a TypeScript rule: from the
configuration's canonical directory up to the root, the first
`node_modules/typescript/package.json` that exists and is not a directory
names the installed compiler, as Node's `require` would find the package,
passing over a directory named `node_modules` itself, a candidate without a
manifest, and a dangling link. Links on the way are followed. Any TypeScript 5 release selects the TypeScript 5.6 dialect, whose
configuration rules and tables are 5.6.3's; no package, or any other major
version (4, 6, 7, 8), selects TypeScript 7, whose rules the rest of this
document describes. Only 5.6.3 is validated: another 5.x release is read with
5.6.3's rules, and `--doctor` says `validated for 5.6.3 only`. TypeScript 7.0.2
stays the semantic authority in both dialects: checking, types, and every
query follow it. Selection reads metadata only and never runs a compiler.

A manifest that cannot name a version fails loading, naming the manifest,
and never selects another compiler: one that cannot be read, or is not valid
UTF-8, is `Io`; one that is not JSON, not an object, has no `version`, or a
`version` that is not a full SemVer version (`5.6`, `v5.6.3`, `latest`) is
`Unsupported`. Each candidate on the way, the winner's real path, and its text
are inputs a session observes, so editing the version, removing the package,
installing a nearer one, or retargeting a link answers as a fresh load does.

Selection follows each configuration, not a workspace: in a monorepo whose
TypeScript is installed only at the root, every package's configuration is
read with the root's compiler, and a package with its own
`node_modules/typescript` is read with that one, whatever the root has. The
compiler is selected from the selected configuration's directory before any
`extends` is followed, so a package named in `extends` is resolved by the
selected compiler's rules and version, which its `typesVersions` may consult.

`--doctor --project` reports the selection on its `compatibility` line: the
installed compiler's manifest and version, the dialect, the semantic
authority, and the standard library's source.

The subsections below describe where the TypeScript 5.6 dialect reads a
project as TypeScript 5.6.3 does rather than as TypeScript 7 does, and the
5.6.3 behaviors it does not reproduce. Its package ids are in [Package
identity](#package-identity), and its JavaScript inputs, TypeScript-only
syntax errors, and refusals in [JavaScript inputs](#javascript-inputs).

### Configuration

The TypeScript 5.6 dialect reads configuration as TypeScript 5.6.3 does. Its
options are 5.6.3's `optionDeclarations`, pinned as data extracted from its
`typescript.js` with that file's SHA-256 (`tools/ts56-option-catalog.mjs`;
[validation](validation.md#typescript-563-oracle)). An option 5.6.3 does not
declare, such as `stableTypeOrdering`, `deduplicatePackages`, or
`libReplacement`, is a `Config` error even when `null`: TS5093 when its name
lowers to a `--build` option's, such as `Verbose` or `Incremental`; otherwise
`Unknown compiler option`, with the suggestion 5.6.3's `getSpellingSuggestion`
makes, if any (TS5025), which measures names in UTF-16 code units and lowers
them as JavaScript does, so `stric😀` and `lİb` get none. A value of the wrong
type names a list as an `Array` and an entry by its element's name (`suffix`
for `moduleSuffixes`, `condition`, `plugin`); an enumeration's TS6046 names
the option as `--name` and lists 5.6.3's keys. Enumeration values are lowered
with JavaScript's `toLowerCase`, so `react-natİve` (U+0130) is TS6046 where
TypeScript 7 accepts it, and an empty string is TS6046 too. The declarations
also decide how each value converts: a path (`isFilePath`, of the option or
of its list's entries) is relative to the configuration that declares it,
`${configDir}` applies to the options 5.6.3 substitutes it in
(`configDirTemplateSubstitutionOptions`), and a list keeps its empty entries
only where `listPreserveFalsyValues` says so (`moduleSuffixes`).

Options TypeScript 5.5 removed (`charset`, `importsNotUsedAsValues`,
`keyofStringsOnly`, `noImplicitUseStrict`, `noStrictGenericChecks`, `out`,
`preserveValueImports`, `suppressExcessPropertyErrors`,
`suppressImplicitAnyIndexErrors`, and `target: es3`) are `Config` errors,
TS5102 or TS5108, even with `"ignoreDeprecations": "5.0"`; a setting
JavaScript reads as false, such as `"importsNotUsedAsValues": "remove"`,
passes. A non-empty `ignoreDeprecations` other than `5.0` is TS5103. The
`verifyCompilerOptions` checks that the TypeScript 7 profile ports have
5.6.3's conditions, words, and order (TS5052, the removed options, TS5061 to
TS5066 and TS5090 naming `baseUrl`, TS5053, then TS5091 for
`preserveConstEnums: false` beside `isolatedModules` or
`verbatimModuleSyntax`, TS5095 for `bundler` without ES modules or
`preserve`, TS5109, and TS5110); of several, the first is reported. Its other
checks, such as `checkJs` without `allowJs`, are not ported in either
dialect. Each error points at the value that applies, as under TypeScript 7,
where 5.6.3 points TS5102, TS5052, and TS5091 at an option's name; the
message is 5.6.3's, its "Use ... instead" detail on the same line.

Defaults are 5.6.3's `computedOptions`: `strict` is false unless set, and
each strict-family flag follows it; an unset `target` is ES5 unless `module`
is `node16` (ES2022) or `nodenext` (ESNext); `module` follows `target`;
`moduleResolution` follows `module`, `preserve` resolving as `bundler`;
`esModuleInterop` is on for `node16`, `nodenext`, and `preserve`;
`resolveJsonModule` is on for bundler resolution alone; and
`noUncheckedSideEffectImports` is off. Refused as `Unsupported`, naming the
option and whether TypeScript 5's default set it, are an effective `target`
ES5, `module` `none`, `amd`, `umd`, or `system`, `moduleResolution` `classic`
or `node10`, `esModuleInterop` or `allowSyntheticDefaultImports` false, and
`alwaysStrict` false, which TypeScript 5's default makes it when `strict` is
unset, so a project must set `strict` or `alwaysStrict`; also `outFile`,
`noResolve`, and a non-empty `references`. `downlevelIteration` is accepted.
A refused default is reported at the configuration, an explicit setting at
its value.

Each file's own options are 5.6.3's (`onPropertySet`): the last occurrence
decides, so a `null` after a value, in the same object, a later
`compilerOptions` object, or over an inherited value, unsets the option, and
an empty string replaces an inherited one (an empty `jsxFactory`,
`jsxFragmentFactory`, `reactNamespace`, or `jsxImportSource` then selects
the default, as both compilers test them for truthiness). A list leaves out
its `null` and empty entries, except that an empty path names the declaring
configuration's directory; an own `paths: {}` replaces inherited patterns;
and a `paths` pattern may be empty. Refused as `Unsupported` where they
apply, as JavaScript artifacts that would change names, are a `paths` pattern
`__proto__`, whose assignment sets the mapping's prototype in 5.6.3; a `null`
`moduleSuffixes` entry, which 5.6.3 keeps and spells `undefined` in file
names; and a root-level `__proto__` whose value is an object, in the
configuration or any base, which sets the raw configuration's prototype,
where 5.6.3's inheritance then sees its `files`, `include`, or `exclude` and
its own lookups do not. Another `__proto__` value sets no such prototype and
is read as an ordinary key. `compilerOptions` may be an array, as 5.6.3 reads
any object: it sets no option and none of its entries is converted, so
earlier and inherited options stand (TypeScript 7 reports it as TS5024).

Inheritance and root lists are 5.6.3's (`applyExtendedConfig`,
`getConfigFileSpecs`): an own `null` `files`, `include`, or `exclude`
inherits the base's list; `[]` is an explicit empty list (`include: []` is
TS18003); an own `[null]` lists nothing, so `include: [null]` is TS18003; an
empty `include` or `exclude` spec matches nothing, so a leaf's
`include: [""]` is TS18003, while an empty `files` entry names the
configuration's directory; and an inherited `null` or empty entry is rebased
first, so it names the base's directory when the base is in another one.
TS18003 lists each entry as written in the configuration that declared it,
`null` included, where 5.6.3 lists inherited entries rebased. An `extends`
entry that is not a string, `null` included, is TS5024. `${configDir}` is
matched as JavaScript's `toUpperCase` folds the value's first 12 UTF-16 code
units, so `${confıgDir}` (U+0131) matches where TypeScript 7 does not, and
`${confİgDir}` (U+0130) does not where it does.

`baseUrl` is a path option, relative to the configuration that declares it,
`${configDir}`-capable, and inherited; `paths` substitutions are relative to
it when it is set (`getPathsBasePath`), including the leaf's own and a
base's, and may then be non-relative, while without it a non-relative
substitution is TS5090. An unset `types` includes every package in the type
roots, the nearest root first, a name in two roots resolving to the nearer
one; `[]` includes none; and `*` is a package's name, TS2688 when no
`@types/*` exists.

### Resolution and naming

Module resolution follows 5.6.3's
`tryLoadModuleUsingOptionalResolutionSettings`: a name that does not start
with `./` or `../` is first matched against `paths`. A matched pattern
decides the optional settings: its first existing substitution, or, when
every substitution is missing, no file under `baseUrl` or `rootDirs`, after
which package `imports`, a self name, and packages are still searched. A bare
name no pattern matches is looked up as `baseUrl/name`, as a relative name
is, a directory's `package.json` included, before package `imports` (so
`baseUrl/#name.ts` wins), a self name, and packages; a backslash in the name
is a separator there. A relative or rooted name never uses `baseUrl`, and
goes to `rootDirs`. TypeScript 7.0.2 instead continues past a matched
pattern whose substitutions are missing, so a rooted name it matches still
tries `rootDirs` there. The same 5.6.3 rule holds for a package's
`typesVersions`: a matched pattern whose substitutions are missing ends that
directory's manifest entry or that package's lookup in one `node_modules`,
where TypeScript 7.0.2 goes on to the subpath's own file or the `types`
entry. In both places 5.6.3 replaces a substitution's `*` only with a
non-empty capture: `ab` matched by `"ab*": ["./first/*"]` tries `first/*` as
written, a file named `*`, where 7.0.2 tries the directory `first/`.

`typesVersions` ranges and `types@` export conditions are evaluated against
the installed compiler's version, as each compiler evaluates them against
its own, and 7.0.2 under TypeScript 7: in module and type-reference
resolution, in `extends` package lookups, in the standard library
replacement lookup, and in module names the checker spells. A version change
in the installed manifest selects again.

Module names the checker spells (`getLocalModuleSpecifier`) take `baseUrl`
as their base, else the `paths` base: with no package `imports` or `paths`
name, the path from `baseUrl` is the non-relative candidate (an empty `paths`
name, from an empty key or a `*` that matched nothing, still answers, and
leaves the relative name), which the checker's project-relative preference
takes for a target across the project's boundary or in another package
scope; a path that climbs out of `baseUrl` stays relative. 5.6.3 processes
that candidate's ending without asking whether an `index` file collides with
a sibling. Type displays (`--type`) name a module by its file's absolute
path, as both compilers' quick info does, so these names show in the
checker's own diagnostics and choices rather than in output.

5.6.3 differences in module naming, resolution, and manifests that the
TypeScript 5.6 dialect does not reproduce, each outside what a supported
project depends on: 5.6.3 asks `processEnding`'s index-collision question
only when `paths` validates a minimal ending and for a package's file, where
this tool, as 7.0.2, also asks it for relative, `rootDirs`, and `paths`
candidate names (the question resolves the name against the working
directory, so the two differ only when that directory is the name's base and
an `index` file's directory has a same-named sibling); 5.6.3 enumerates
integer-like `paths` and `typesVersions` keys first, as JavaScript orders
object keys; 5.6.3 compares an absolute `paths` substitution, such as one
from `${configDir}`, with a relative name and so never spells a module
through it; 5.6.3 takes an empty `typesVersions` name, from a `*` key whose
star matches nothing, as the package's root, where this tool, as 7.0.2,
blocks the package name; 5.6.3 reads a `package.json` with its configuration
JSON reader, which accepts comments and trailing commas, where this tool
reads one as strict JSON in both dialects; and 5.6.3's per-directory
type-reference cache can replay a directive's result for an inferred name,
where this tool keeps the two origins separate lookups in both dialects, as
7.0.2 does.

One compatibility exception is scoped to the text a wildcard captures.
5.6.3 inserts that text with JavaScript's `String.prototype.replace`, whose
replacement string expands `$$` to `$`, `$&` to the `*` it replaces, and
`` $` `` and `$'` to the text before and after that `*`: in the first `*` of a
`paths` or `typesVersions` substitution and of a `paths` or `exports` key it
spells a module name with (`replaceFirstStar`), and in every `*` of a package
`exports` or `imports` target (`replace(/\*/g, ...)`). TypeScript 7.0.2 inserts
the text as written (`strings.Replace`, `strings.ReplaceAll`), and so does
this tool in both dialects: with `"ab*": ["./first/*"]`, `ab$$y` names
`first/$$y` where 5.6.3 names `first/$y`. A capture without those four
sequences is inserted alike by both compilers.

### Standard libraries and program order

Standard libraries are the installed compiler's own declarations, in the
real path of its package directory's `lib`, where the compiler runs from
(`getDefaultLibLocation`), through a linked package directory or a linked
`lib` and whatever file a linked `package.json` names; a session notices any
of them relinked. Answers follow those declarations: `new
TextEncoder().encode("x")` is 5.6.3's non-generic `Uint8Array`, where 7.0.2's
is `Uint8Array<ArrayBuffer>`. The libraries are selected by 5.6.3's catalog (`libs`, `libMap`,
and `getDefaultLibFileName`, extracted from the pinned `typescript.js`):
`lib` names are 5.6.3's (an unknown one, such as `es2024` or `esnext.date`,
is TS6046), and without `lib` ES2015 selects `lib.es6.d.ts`, ES2016 to
ES2023 their `.full` files, and ESNext `lib.esnext.full.d.ts`. Libraries
come first in program order by the first position of their name in `libs`,
where `esnext.array` and `esnext.string` appear twice, and a name declared
twice selects its last file. A file the catalog names that the installed
`lib` lacks or cannot read is an `Io` failure naming it; the vendored
TypeScript 7.0.2 copy never stands in for it. A `/// <reference lib>` names
a library only (`getLibFileNameFromLibReference`), lowered as 5.6.3's
`toFileNameLowerCase` lowers it, so `lib.es2015.promise.d.ts` is TS2727 with
5.6.3's suggestion (`Cannot find lib definition for
'lib.es2015.promise.d.ts'. Did you mean 'es2018.promise'?`) and an unknown
name without a suggestion TS2726.

A `@typescript/lib-*` package replaces every library `pathForLibFile`
locates, without `libReplacement`, which 5.6.3 does not know: each `lib`
entry and each library a `/// <reference lib>` names, but not the target's
default, which 5.6.3 reads from its directory directly, though the libraries
the default references are replaced. The package is looked up from the
configuration's directory with 5.6.3's own options (`resolveLibrary`,
`getOptionsForLibraryResolution`): `moduleResolution: node10`,
`node_modules` and `@types`, declarations before JavaScript, without package
`exports` and without the project's `paths`, `baseUrl`, `rootDirs`,
`typeRoots`, `moduleSuffixes`, or conditions; it ignores `preserveSymlinks`,
so a symbolic link resolves to its target. Library selection ignores
`/// <reference no-default-lib="true" />` as under TypeScript 7, where 5.6.3
skips the default libraries when such a file is among the roots, the type
packages, or the files they reach, and lets `skipDefaultLibCheck` cover any
file that has one.

Library ordering follows 5.6.3. `createProgram` processes the type packages
before the libraries, depth first, and orders a file with the libraries by
the route that first processes it: a `lib` entry, a `/// <reference lib>`,
or a `/// <reference path>` from a file ordered with the libraries
(`processReferencedFiles(file, isDefaultLib)`), never an import or a
`/// <reference types>`. So a type package precedes the files a replacement
imports; a replacement that a type package imports or references by path is
processed there, before its `lib` entry, and is ordered as an ordinary file
after the type package's earlier files; a file a library first references by
path is ordered with the libraries though it is not itself one, even when
another library imports it later; and a replacement a type package's
`/// <reference lib>` names precedes, among libraries of equal priority, one
a `lib` entry names. The libraries are then ordered by each file's own path
(`getDefaultLibFilePriority`): a file anywhere beneath the compiler's `lib`
directory, a replacement that resolves there included, by its file name, and
any other file last, keeping discovery order among equals as 5.6.3's stable
sort does. A file is placed by the
name its first route gives it, as 5.6.3 names it (`SourceFile.fileName`): a
located library in the real `lib` directory, a replacement at the real path
it resolves to, and a file a reference path names at the path it writes from
the referencing file's own name, through any link, normalized before the
extension it was found with is added, so `lib/es5/child/..` and `lib/es5/.`
name `lib/es5.d.ts` as `lib/es5` does. So a file a replacement references
through a linked `node_modules/typescript` or a linked `lib` lies outside
that directory and is last, while a replacement whose `types` names a file
there lies inside it. The order decides answers: overload resolution tries a
later file's declarations of a merged function first, so where a replacement
imports `declare function pick(value: string): "lib"` and a type package
declares the same signature returning `"types"`, `pick("x")` is `"lib"`
here, as under 5.6.3, and `"types"` under TypeScript 7; and where a type
package imports both a file declaring `"types"` and the replacement
declaring `"lib"`, it is `"lib"` here and under 5.6.3, which order the
replacement after that file.

Synthetic JSX-runtime imports remain a compatibility limit. TypeScript
5.6.3's `collectExternalModuleReferences` considers every JavaScript file,
and every non-declaration file when effective `isolatedModules` is true or
the file is an external module; `getJSXImplicitImportBase` and
`getJSXRuntimeImport` then decide whether a runtime is selected. This tool
retains 7.0.2's JSX/TSX-only predicate in both dialects. On the unchanged
Artemis program, four moved files account for all 858 unequal order rows:
React global declarations, csstype, React index and jsx-runtime. Input sets,
provenance and all 18 redirects agree. An exhaustive comparison of the
declarations whose relative order changes found no contributed member,
base-type or overload sequence change; the four crossing React interfaces
have neither members nor bases. This proves unchanged answers for that
program only. Elsewhere the predicate can change which runtime inputs are
loaded, and thus answers; a full port must cover input admission, module
status, diagnostics and refresh under both dialects. See the
[dated acceptance](validation.md#artemis-acceptance-2026-10-27).

Replacements are libraries, never searched, and a package that imports
`typescript` reaches `typescript/lib/typescript.d.ts` as a dependency like
any other. A file reached both as a library and through another route takes
its provenance from the same breadth-first route as under TypeScript 7
([Loading and lexical differences](#standard-library-files)), while its
program-order place follows the route that first processes it depth first,
as 5.6.3 decides it (above). 5.6.3 decides provenance by content and name
instead (`isSourceFileDefaultLibrary`): a file with `no-default-lib`, as
every installed library has, is a library however it is reached, and any
other file, a replacement without that directive included, only when it is
the target's default or the file a `lib` entry resolves to. So a replacement
without `no-default-lib` of a library that only a reference names, such as
one the target's default references, is an ordinary input to 5.6.3, searched
and reported, and an installed library a project file reaches first by
reference path or import is still a library there. This tool gives both the
first route's provenance: keeping every replacement a library, never
searched, is its product policy in both dialects, not either compiler's
rule.

### Diagnostics

A namespace declared with the `module` keyword is a suggestion, not TS1540,
as TypeScript 5.6 reports it, so a project read with the TypeScript 5.6
dialect loads it; file-local `--check`, which reads no configuration, still
reports TypeScript 7's TS1540 ([Loading and lexical
differences](#diagnostics-and-failures)). Unknown options and library
references get 5.6.3's spelling suggestions ([Configuration](#configuration),
[Standard libraries and program order](#standard-libraries-and-program-order)),
and a JavaScript file's TypeScript-only syntax error and parameter
decorators follow 5.6.3's rules ([JavaScript inputs](#javascript-inputs)).

## JavaScript inputs

With `allowJs` (or `checkJs: true` and `allowJs` unset), `.js`, `.jsx`, `.mjs`,
and `.cjs` files are program inputs as TypeScript makes them: roots that
`include` or `files` selects, files a project import or `/// <reference path>`
reaches, and JavaScript a package lookup finds in `node_modules` within
`maxNodeModuleJsDepth` (default 0) package lookups of a root. A file first
reached beyond that depth and later by a shallower route is loaded, where the
deeper route met it (`collectFiles`). A loaded file that a later route reaches
at a lower depth follows its subtasks again from there, so JavaScript it elided
before is an input once within the limit. Under TypeScript 7 every such file
does, and the lower depth reaches its loaded subtasks in turn
(`filesParser.start`). The native 7.0.2 loader starts a file's subtasks only
once, so its listing varies from run to run where a deeper route loads the
file first; this tool follows the rule. Under the TypeScript 5.6 dialect only
a file with elided imports reached below the limit, or one first found in
`node_modules` reached at depth 0, does, as 5.6.3's `findSourceFile` decides,
so a package without elided imports of its own does not pass a lower depth on
to its subtasks. 5.6.3 loads depth first and this tool breadth first, so their
routes can reach a file in a different order: the listings agree where the
deeper route loads the file first under both. Loading also lowers the depth
of what a package-id duplicate's own subtasks reach (see [Package
identity](#package-identity)) before collection knows the duplicate is
redirected, which 5.6.3 never processes, so with `maxNodeModuleJsDepth` of 1
or more a `node_modules` JavaScript file reached only through a duplicate
reached at depth 0 may be an input here and not under 5.6.3; not yet
followed up. Without `allowJs`, a JavaScript
root or reference target is TS6504, and an import that finds only JavaScript
is TypeScript's untyped module: TS7016 at the import under `noImplicitAny`,
TS2665 for an augmentation, and otherwise names of the error type.
`jsconfig.json`'s defaults (`allowJs`, `maxNodeModuleJsDepth: 2`) are not
applied, and TS5055 (`allowJs` without `noEmit` overwriting an input) is not
reported.

A JavaScript file parses with the JSX language variant. TypeScript-only syntax
in one (a type annotation, `interface`, `enum`, `import =`, an access modifier,
a non-null assertion, a type assertion, and the rest of `checkJSSyntax`) fails
loading at TypeScript's first error, whatever checks the file. A parameter
decorator without `experimentalDecorators` is TS1206: under TypeScript 7, the
checker's at the first decorator that no `@ts-ignore` covers in a file it
checks, and otherwise syntactic, at the first decorator's full start, leading
comments included (`getAdditionalJSSyntacticDiagnostics`); under the
TypeScript 5.6 dialect always syntactic, at the first decorator, however the
file is checked. The TypeScript-only syntax error is each dialect's own.
Under TypeScript 7 it is the parser's first (`checkJSSyntax`), which does not
check an ambient module (`declare global`, `declare module "x"`). Under the
TypeScript 5.6 dialect it is the first of 5.6.3's walk of the whole tree
(`getJSSyntacticDiagnosticsForFile`), which checks an ambient module too and
locates each error as 5.6.3 does (`getErrorSpanForNode`): at a declaration's
name, so a bodiless function's at its name; at a list's first element; and at
an index signature's key type, whose annotation is the error (TS8010).
File-local `--check`, `--outline`, and `--show` accept JavaScript files. They
read no configuration and so have no dialect: `--check` reports TypeScript
7's error (`checkJSSyntax`), while `--outline` and `--show` answer from the
whole tree despite one.

Binding diagnostics follow `checkJs` (unset, `false`, or `true`) and the file's
last `// @ts-check` or `// @ts-nocheck` pragma (`IsCheckJSEnabledForFile`). A
file `checkJs: false` or `// @ts-nocheck` leaves unchecked reports none. A
plain JavaScript file, without a pragma while `checkJs` is unset, reports only
the codes of `plainJSErrors` (TS2451 and TS2528, for example, but not TS2300
or TS2307), ignores `@ts-ignore` and `@ts-expect-error`, and reports no merge
conflict at its own declarations (`reportMergeSymbolError`). A checked file
reports as a TypeScript file does.

Answers for plain ES module JavaScript follow TypeScript's JavaScript rules:
an object literal without a contextual type is a JavaScript literal, whose
missing members are `any` unless `noImplicitAny`; a `null` or `undefined`
initializer widens to `any` without `strictNullChecks`, and an empty array
initializer to `any[]`; an untyped function needs no arguments; a call infers `any` for a type
parameter nothing infers; and an object literal method's `this` is the literal.
A default import of a JavaScript script without ES module syntax or
`module.exports` (which has TypeScript's synthetic default), a JavaScript
class that extends a generic class (whose type arguments TypeScript fills
with `any`), and JSX element types are located query refusals
(`javascript_semantics_required`, `jsx_typing_required`). JavaScript files
are never edited ([editing.md](editing.md)).

A JSDoc `@type` in the last comment of a variable statement or declaration
is its declared type, as TypeScript 7's parser reparses it
(`reparseTags`, `reparseHosted`; `src/parser/jsdoc.rs`): the statement's
first declaration without a type takes the first such tag, its next one the
next, and a declaration's own comment comes first. The type is TypeScript
type syntax, whose names resolve where the declaration stands, and an
`import("...")` type in it is a module request the loader follows, as from
any type. References find the names the type spells, as 5.6.3's
`findReferences` does. A reference in it to a generic class or interface
is fitted to its type parameters as TypeScript fits one in JavaScript
(`fillMissingTypeArguments`), never the error type: an omitted argument
takes its default, but `any` where that default is identical to `unknown`
or `{}` or absent, and arguments past the parameters are dropped, so
`UBox` for `UBox<T = unknown>` is `UBox<any>` in JavaScript and
`UBox<unknown>` in TypeScript. A JSDoc function type's type parameters are
fitted the same way wherever TypeScript instantiates its signature with
written type arguments, whatever file writes them, since TypeScript asks of
the signature's declaration (`checkTypeArguments`,
`getSignatureInstantiation`): `f<string>("x")` from TypeScript is `any` for
`<A, B = unknown>(x: A) => B`, while a signature declared in TypeScript keeps
TypeScript's defaults though a JavaScript `@type` names it. A short list for
a parameter without a default, `p<string>("x")` for `<A, B>(x: A) => B`, is
TS2558, from which TypeScript recovers with the call's own file
(`pickLongestCandidateSignature`); this tool refuses that overload recovery
at the call, as in every file kind. The public AST (`tsparse FILE`),
`--check`, and `--outline` leave the reparsed type out, as TypeScript's
checks of written syntax do.

A JavaScript file without ES module syntax whose one `module.exports`
assignment is a top-level `module.exports = name;` is a CommonJS module, as
TypeScript 7's binder makes it (`setCommonJSModuleIndicator`,
`bindModuleExportsAssignment`; `src/project/binding/javascript/commonjs.rs`):
its `export=` is an alias of `name`, whose declaration is the assignment,
and it declares its own `module` and `exports` (`declareCommonJSVariable`),
which references find, declared at the file's start, in place of a global
of that name. A TypeScript default import of it is its synthetic default,
`name`, unless `name`'s type has an `__esModule` member
(`canHaveSyntheticDefault`; see [Compatibility limits](#compatibility-limits)
for one spelled as an identifier). References to `name` reach that import,
its uses, and the assignment's operand, as a use. `import("...")` types follow
the `export=`, and a type qualifier that `name` does not export is looked
up among the module's own exports (`getTypeFromImportTypeNode`), which have
no other. The types of `module` and `exports` themselves, which TypeScript
makes of the module, are located query refusals.

### Refused JavaScript

A JavaScript construct to which TypeScript 7.0.2 gives JavaScript-only meaning
this tool does not model is a capability limit, never a diagnostic. Binding
records each where TypeScript decides it (`src/project/binding/javascript.rs`):

- a CommonJS `module.exports =`, `exports.name =`, `module.exports.name =`,
  or `Object.defineProperty(exports, ...)` in a file without ES module
  syntax (`setCommonJSModuleIndicator`, `bindModuleExportsAssignment`,
  `bindExportsOrObjectDefineProperty`), but the one top-level
  `module.exports = name;` above: another operand (`module.exports = {}`,
  `= o.a`, `= class {}`), a second assignment, or one in a function or
  another expression is refused;
- an assignment or `Object.defineProperty` call that declares a member of a
  function, class, or object (`bindDeferredExpandoAssignment`, judged by the
  binder's `lookupEntity`, `getInitializerSymbol`, and existing-member rules:
  `f.x = 1` for a function `f` declared in the same container, a class, an
  arrow, function, or class expression, or an empty object literal);
- a `this.name =` in a class member, a property initializer, or an object
  literal method (`bindThisPropertyAssignment`);
- every `require` call, which makes a script a CommonJS module
  (`bindCallExpression`), names a module TypeScript's loader follows, and
  resolves to it in the checker;
- a JSDoc tag the parser reparses into syntax: at its host, in the host's last
  comment (`@type`, `@satisfies`, `@template`, `@param`, `@this`, `@returns`,
  `@public`, `@private`, `@protected`, `@readonly`, `@override`, `@implements`,
  and `@extends` where they change the host, `reparseHosted`), or as a
  declaration of its own (a `@typedef` with a type, `@callback`, an `@import`
  with a clause, and an `@overload` of a function, method, or constructor
  outside an object literal, `reparseUnhosted`); a variable's `@type`, above,
  is refused only for a `for`, `for...in`, `for...of`, or `catch` variable,
  and, as a `JSDoc type with JavaScript-only meaning` at the type, where
  the type is JSDoc type syntax (`*`, `?T`, `!T`, `T=`, `...T`), names
  `String`, `Number`, `BigInt`, `Boolean`, `Void`, `Undefined`, `Null`,
  `Function`, `function`, `array`, `promise`, or `Object`, which
  `getIntendedTypeFromJSDocTypeReference` reads as other types, or needed
  recovery from a syntax error, such as Closure's `function(number): string`;
- a JSDoc import type with a literal argument, whose module TypeScript's
  loader follows from a JavaScript file, but in a variable's reparsed
  `@type`.

Loading fails at the first one, as `Unsupported` with `unsupported JavaScript:
CONSTRUCT (tsparse reads JavaScript as plain ES modules or one CommonJS
module.exports, and JSDoc only as a variable's @type)`,
for every project JavaScript file that has one, whether a root, an import, a
re-export, or a reference directive reaches it, and whatever `checkJs`,
`noCheck`, a pragma, a comment directive, or `skipLibCheck` says. A dependency's
JavaScript that has one is no program input: a project file's import,
re-export, dynamic import, or import type of it fails at the specifier
(`module "NAME": FILE:LINE:COLUMN: unsupported JavaScript: ...`), even where an
ambient module of its name would stand in for it, and a dependency's request
for it names nothing, or that ambient module, so none of its declarations is
bound; a CommonJS dependency script declares no globals, as in TypeScript. Such
a file is missing from the program's inputs, which TypeScript lists.
Constructs to which TypeScript 7 gives no such meaning load under its dialect
and keep its answers: `config.resolve.alias = value` for a parameter `config`, an
assignment to a member of an object literal with members, of an existing
static member, or of a function declared in another container, `this.x =` in a
function or at the top level, `exports` and `module.exports` in an ES module,
and JSDoc without such a tag. In a CommonJS module, `config.resolve.alias =
value` for a callback's parameter `config`, spreads, an empty array in a
conditional, and reads such as `process.env.NAME` declare nothing either and
keep TypeScript 7's answers.

Deliberately broader than TypeScript's own decisions: every `require` call is
refused, not only a `require` declaration; a JSDoc import type is refused where
its tag has no meaning; a CommonJS form is refused in a file with a JSX
element, which under `jsx: react-jsx` makes the file a module where
TypeScript declares nothing; a `module.exports = name` inside a function or
block is refused, though TypeScript binds it as the top-level one
(`bindModuleExportsAssignment` reads no container); a variable's `@type`
naming `array`, `promise`, or `Object` is refused whatever its type arguments
and `noImplicitAny`, though `getIntendedTypeFromJSDocTypeReference` reads
`array` and `promise` only without type arguments and without
`noImplicitAny`, and `Object` only with two type arguments or without
`noImplicitAny`; and a project file's import of a refused dependency fails
even where TypeScript would select an ambient module of that name.

The TypeScript 5.6 dialect refuses everything above too, since both dialects
answer through one binder and checker, which port 7.0.2 for the supported slice
alone and follow it beyond the slice only in part (a CommonJS export sets the
module indicator by 7.0.2's classification, but binds nothing): a construct to
which 5.6.3 gives no meaning is not therefore one this tool can answer as 5.6.3
does. So it refuses, deliberately more than 5.6.3 gives meaning, whatever 7.0.2
alone gives meaning, for example: an assignment whose rightmost value is
`void 0`, which 5.6.3 declares nothing for (`getAssignmentDeclarationKind`),
such as `module.exports = void 0` or `exports.a = exports.b = void 0`, after which
5.6.3 imports a file with no other CommonJS export as a script (TS2306),
`f.x = void 0`, or `this.x = void 0` in a class; an expando assignment ahead
of its variable's declaration, which 5.6.3 binds as it walks; and
`@satisfies` at an export assignment or a `return`, which 7.0.2's reparser
reads. The dialect also refuses what 5.6.3 gives JavaScript-only meaning, each
where it is written, by 5.6.3's own decisions. A `require` call and a JSDoc
import type mean the same to both. 5.6.3's binder binds as it walks the file,
a statement list's function declarations first and a `for` loop's body before
its incrementor, so an assignment sees only the declarations bound before it
(`bindWorker`, `bindChildren`). Apart from an assignment of `void 0`, it
declares:

- a CommonJS export, as 7.0.2 does, but no `export=` for `module.exports =
  name` where `name` is `exports` or a variable initialized to it
  (`isExportsOrModuleExportsOrAlias`), which the dialect therefore refuses;
- for a `this.name =`, or one through a variable initialized to `this`
  (`isThisInitializedDeclaration`), a member of the class or object literal,
  or a member of the function that holds it, which becomes a constructor
  function, or of `E` for a function assigned to `E.prototype.m`; at a file's
  top level it declares a variable, unless the name is dynamic
  (`bindThisPropertyAssignment`);
- for an assignment to, or `Object.defineProperty` call on, `E.prototype.m`,
  or an object literal assigned to `E.prototype`, a member of `E`, which
  becomes a class (`bindPrototypePropertyAssignment`,
  `bindObjectDefinePrototypeProperty`, `bindPrototypeAssignment`);
- for a top-level assignment to, or `Object.defineProperty` call on, a member
  of an entity whose leftmost name is not an import, a namespace for each name
  of the entity, a global one where no declaration of the name is bound yet
  (`bindPotentiallyMissingNamespaces`), and the member;
- elsewhere, a member of a function, class, or namespace, or of a variable
  whose initializer, after a `||` or `??` default, is a function, a class, an
  immediately invoked function, or an empty object literal (`isExpandoSymbol`,
  `getExpandoInitializer`);
- for an access expression statement with a `@type` tag, such a member
  (`bindSpecialPropertyDeclaration`).

5.6.3 reads JSDoc by ownership rather than by position. A node owns the tags of
the last comment of each node its comment chain reaches, but for a
parenthesized expression's `@type` and `@satisfies`, which only it owns, and
an earlier comment's `@overload` (`getJSDocCommentsAndTags`,
`filterOwnedJSDocTags`). The
chain runs up from a variable to its statement, and from an expression to the
variable, property, assignment, `return`, or defaulted assignment that holds
it (`getNextJSDocCommentLocation`), so a function in
`ns.f = ns.f || function (a) {}` reads the statement's `@param`. A tag is refused
where an owner reads it:

- `@type` at a variable, parameter, property, binding element, accessor,
  export assignment, function, or declared assignment, or at the
  parenthesized expression it is attached to, but a variable's first owned
  `@type`, its declared type (`getJSDocType`), where the parser reparsed it
  onto that variable: the dialect refuses a statement's `@type` the reparse
  gives a later declaration, which 5.6.3 leaves untyped, and one whose
  statement initializes its first declaration with a function, which 5.6.3
  also reads as that function's signature (`getSignatureOfTypeTag`);
- `@satisfies` at the parenthesized expression it is attached to; at a
  variable, parameter, class property, or binding element with an
  initializer, whose initializer it types contextually and checks
  (`getContextualTypeForVariableLikeDeclaration`, `checkDeclarationInitializer`);
  and at an object literal property with an initializer, whose initializer it
  only types contextually (`getContextualTypeForObjectLiteralElement`). 7.0.2
  reads it at an export assignment and a `return` instead of a parameter;
- `@param` with a type or brackets at the parameter it names, or at a binding
  pattern's position;
- `@returns` with a type, `@this`, and `@template` at a function, `@overload`
  at one with comments of its own, and `@template` and `@implements` at a
  class;
- `@extends` or `@augments` at a class with an `extends` clause, whatever
  class the tag names;
- `@class` and `@constructor` at a function;
- a modifier tag at a class member or a declared assignment, and `@readonly`
  at an object literal member, whose accessibility 5.6.3 drops
  (`getDeclarationModifierFlagsFromSymbol`);
- `@typedef`, `@callback`, `@enum`, and `@import` with a clause, in any
  comment, which its binder declares.

5.6.3's broader synthetic JSX-runtime predicate is retained as a
[compatibility limit](#standard-libraries-and-program-order), including for
ordinary files under `isolatedModules`. It also lists a `node_modules` file
first loaded by a shallower route in its processing order.

### TypeScript's JavaScript-only branches

The 58 static `IsInJSFile` sites of 7.0.2's checker and binder, and what this
tool does at each. *Ported* sites behave as TypeScript's; *refused* sites are
reached only by a construct refused at loading or at a query, in both
dialects: the TypeScript 5.6 dialect refuses every construct 7.0.2 gives
meaning at such a site, even where 5.6.3 gives it none, as for
`module.exports = void 0`, which would otherwise reach `bindContainer`'s
CommonJS variables with no export (R `REFUSED_7`); *not an
operation* sites serve diagnostics, declaration emit, or hover, which this tool
does not produce; *unreachable* sites need syntax TypeScript itself rejects in
JavaScript (TS8010, TS8011). Tests are in `tests/javascript_refusals.rs` (R),
`tests/javascript_semantics.rs` (S), `tests/javascript_hosted_types.rs` (H),
`tests/javascript_commonjs.rs` (C), and the classifier's unit tests in
`src/project/binding/javascript.rs` (U).

| Site | Function | Disposition | Test |
| --- | --- | --- | --- |
| binder.go:694 | `bind` | refused at loading: `require` call | R `REFUSED` |
| binder.go:1104 | `getInitializerSymbol` | refused at loading: expando member of a class | R `REFUSED` |
| binder.go:1107 | `getInitializerSymbol` | refused at loading: expando member through a `let` or `var` | R `REFUSED`, `REFUSED_7` |
| binder.go:1112 | `getInitializerSymbol` | refused at loading: expando member through an earlier expando, which is itself refused, so loading fails at that earlier expando | R `REFUSED` (`o.a = {}; o.a.b = 1`, which fails at `o.a`), U `expandos_follow_the_binders_lookup_and_initializer_rules` (the classifier also records the later member) |
| binder.go:1122 | `bindThisPropertyAssignment` | refused at loading: `this` member | R `REFUSED`, `REFUSED_7`, U `this_assignments_follow_the_binders_this_container` |
| binder.go:1606 | `bindContainer` | ported: a CommonJS module's `module` and `exports` (`declareCommonJSVariable`), reached in both dialects only by the one `module.exports = name` bound; refused at loading: every other CommonJS indicator, `@typedef`, `@callback` | C `the_commonjs_module_variable_is_the_files_own`, R `REFUSED`, `REFUSED_7` |
| nameresolver.go:323 | `Resolve` | refused at loading: `require` call | R `REFUSED` |
| checker.go:2573 | `checkJSDocTypeIsInJsFile` | not an operation: diagnostic | |
| checker.go:2639 | `shouldCheckErasableSyntax` | not an operation: diagnostic | |
| checker.go:4713 | `checkMemberForOverrideModifier` | not an operation: diagnostic | |
| checker.go:5252 | `checkImportDeclaration` | not an operation: diagnostic | |
| checker.go:5445 | `checkImportEqualsDeclaration` | not an operation: diagnostic | |
| checker.go:5487 | `checkExportDeclaration` | not an operation: diagnostic | |
| checker.go:6730 | `checkAliasSymbol` | not an operation: diagnostic | |
| checker.go:6818 | `checkAliasSymbol` | not an operation: diagnostic | |
| checker.go:8320 | `checkCallExpression` | refused at loading: `require` call | R `REFUSED` |
| checker.go:9031 | `chooseOverload` | ported: `any` for an uninferred type parameter | S `plain_javascript_object_literals_initializers_and_calls_match_the_compilers` |
| checker.go:9041 | `chooseOverload` | ported: a JSDoc signature's explicit type arguments are fitted as JavaScript's (`fillMissingTypeArguments`), and a complete inferred list is unchanged | S `plain_javascript_object_literals_initializers_and_calls_match_the_compilers`, H `a_javascript_signature_fills_type_arguments_as_typescript_does` |
| checker.go:9063 | `chooseOverload` | ported: as 9041 | S `plain_javascript_object_literals_initializers_and_calls_match_the_compilers`, H `a_javascript_signature_fills_type_arguments_as_typescript_does` |
| checker.go:9199 | `checkTypeArguments` | ported: a TypeScript call's explicit type arguments to a JSDoc signature, fitted as JavaScript's | H `a_javascript_signature_fills_type_arguments_as_typescript_does` |
| checker.go:9359 | `getEffectiveCheckNode` | refused at loading: JSDoc type assertion | R `REFUSED` |
| checker.go:9552 | `inferSignatureInstantiationForOverloadFailure` | ported; reached only by overload recovery, a query refusal in every file kind | S `a_javascript_call_needing_overload_recovery_is_refused_at_the_call` |
| checker.go:9720 | `getArgumentArityError` | not an operation: diagnostic | |
| checker.go:10652 | `getInstantiationExpressionType` | ported: a TypeScript instantiation expression of a JSDoc signature, fitted as JavaScript's | H `a_javascript_signature_fills_type_arguments_as_typescript_does` |
| checker.go:11049 | `checkIdentifier` | not an operation: diagnostic | |
| checker.go:12003 | `getContextualThisParameterType` | ported | S `plain_javascript_object_literals_initializers_and_calls_match_the_compilers` |
| checker.go:12444 | `checkBinaryLikeExpression` | not an operation: diagnostic | |
| checker.go:13098 | `checkObjectLiteral` | ported | S `plain_javascript_object_literals_initializers_and_calls_match_the_compilers` |
| checker.go:13154 | `checkObjectLiteral` | ported | S `plain_javascript_object_literals_initializers_and_calls_match_the_compilers` |
| checker.go:14795 | `canHaveSyntheticDefault` | ported for an ES module and a `module.exports` CommonJS module; refused at query otherwise | S `a_default_import_of_javascript_follows_can_have_synthetic_default`, C `a_default_import_of_module_exports_has_the_exported_constants_type` |
| checker.go:14870 | `reportInvalidImportEqualsExportMember` | not an operation: diagnostic | |
| checker.go:15750 | `resolveQualifiedName` | refused at loading: `require` declaration | R `REFUSED` |
| checker.go:16797 | `widenTypeInferredFromInitializer` | ported | S `plain_javascript_object_literals_initializers_and_calls_match_the_compilers` |
| checker.go:18021 | `getWidenedTypeForAssignmentDeclaration` | refused at loading: assignment declaration | R `REFUSED`, `REFUSED_7` |
| checker.go:18189 | `reportImplicitAny` | not an operation: diagnostic | |
| checker.go:19185 | `getInstantiatedConstructorsForTypeArguments` | refused at query: generic base class | S `a_javascript_class_extending_a_generic_class_is_refused_at_its_heritage` |
| checker.go:19340 | `createCanonicalSignature` | ported: a JSDoc signature's type parameters, through the owner 9199 uses | H `a_javascript_signature_fills_type_arguments_as_typescript_does` |
| checker.go:19400 | `instantiateSignatureInContextOf` | ported: by the contextual signature's file, with no effect on the complete list it infers | S `untyped_javascript_signatures_need_no_arguments` |
| checker.go:19753 | `getSignatureFromDeclaration` | ported: untyped signature | S `untyped_javascript_signatures_need_no_arguments` |
| checker.go:19978 | `getSignatureOfFullSignatureType` | refused at loading: `@type` on a function | R `REFUSED` |
| checker.go:20772 | `getDefaultConstructSignatures` | refused at query: generic base class | S `a_javascript_class_extending_a_generic_class_is_refused_at_its_heritage` |
| checker.go:21839 | `getEffectiveTypeArguments` | ported: a JSDoc reference's deferred type arguments, fitted as below | H `javascript_type_references_fill_type_arguments_as_typescript_does` |
| checker.go:23084 | `getTypeFromClassOrInterfaceReference` | ported: a JSDoc reference's type arguments, fitted to the parameters with `any` for an omitted one whose default is `unknown`, `{}`, or none (`fillMissingTypeArguments`); refused at query: generic base class | H `javascript_type_references_fill_type_arguments_as_typescript_does`, S `a_javascript_class_extending_a_generic_class_is_refused_at_its_heritage` |
| checker.go:23566 | `getTypeAliasInstantiation` | refused at loading: a JavaScript type alias comes only from JSDoc | R `REFUSED` |
| emitresolver.go:427 | `hasVisibleDeclarations` | not an operation: declaration emit | |
| flow.go:261 | `getTypeAtFlowAssignment` | refused at loading: matters only for an expando member | R `REFUSED`, `REFUSED_7` |
| inference.go:86 | `inferFromTypes` | refused at loading: alias type arguments of a JSDoc type alias | R `REFUSED` |
| jsdoc.go:25 | `checkUnmatchedJSDocParameters` | not an operation: diagnostic | |
| jsx.go:979 | `getJsxPropsTypeFromClassType` | refused at query: JSX typing | S `javascript_jsx_element_types_are_refused_at_the_element` |
| jsx.go:1026 | `getJsxManagedAttributesFromLocatedAttributes` | refused at query: JSX typing | S `javascript_jsx_element_types_are_refused_at_the_element` |
| jsx.go:1290 | `getJsxElementTypeTypeAt` | refused at query: JSX typing | S `javascript_jsx_element_types_are_refused_at_the_element` |
| nodebuilder_hover.go:319 | `serializeConstructors` | not an operation: hover | |
| nodebuilderimpl.go:1998 | `tryGetThisParameterDeclaration` | refused at loading: `@this` | R `REFUSED` |
| nodecopy.go:324 | `getExistingNodeTreeVisitor` | not an operation: declaration emit and hover reuse a JSDoc type's nodes, while type display reuses only an instantiation expression, which JavaScript cannot write | |
| nodecopy.go:626 | `getExistingNodeTreeVisitor` | ported: an empty branch | H `a_hosted_type_is_its_variables_declared_type` |
| relater.go:3400 | `structuredTypeRelatedToWorker` | refused at loading: alias variance of a JSDoc type alias | R `REFUSED` |
| symbolaccessibility.go:861 | `isSymbolAccessibleWorker` | not an operation: declaration emit | |
| utilities.go:995 | `isClassInstanceProperty` | refused at loading: expando member | R `REFUSED`, `REFUSED_7` |

## Compatibility limits

- One configuration. Project references are not followed; a configuration with
  non-empty `references` is rejected.
- Inputs are `.ts`, `.tsx`, `.mts`, `.cts`, their declaration files, JSON
  modules, and, under `allowJs`, JavaScript files as plain ES modules or as
  a CommonJS module whose one export is a top-level `module.exports = name;`,
  with a variable's JSDoc `@type` as its declared type (see [JavaScript
  inputs](#javascript-inputs)). Asset modules are unsupported when
  resolution needs them, and other JavaScript-only meaning, such as other
  CommonJS, `require`, or other JSDoc TypeScript reparses, is refused where
  it is written ([Refused JavaScript](#refused-javascript)). In a dependency
  file, an import or `/// <reference path>` that reaches such an input is
  skipped instead.
- Under the TypeScript 7 dialect, standard library declarations come from
  the checkout the tool was built from; there is no installation override or
  relocation. Under the TypeScript 5.6 dialect they are the installed
  compiler's own.
- Synthetic JSX-runtime admission keeps TypeScript 7's JSX/TSX-only rule
  in both dialects, while 5.6.3 considers more files. The verified Artemis
  difference is ordering only; other projects can lose runtime inputs and
  answers. See [the precise predicate and scope of proof](#standard-libraries-and-program-order).
- `with` statements and other syntax without a binding rule are rejected with
  an `Unsupported` error naming the construct.
- Modules and exports are keyed by their exact names, which must be valid
  Unicode. A module specifier, ambient module name, or string-literal import or
  export name with an unpaired UTF-16 surrogate, such as `"\uD800"`, is
  rejected with an `Unsupported` error at its string literal rather than merged
  with the other names that a replacement character would make equal.
  TypeScript keeps such names distinct.
- File and directory names must be Unicode. On a file system that allows
  other names, such a name that file selection or `"types": ["*"]` would use
  is `Unsupported` rather than converted to a different name.
- A member declared by an identifier that begins with `__`, such as `__x:`
  or `__esModule:`, is keyed by TypeScript 6's escaped spelling (`___x`), in
  every file kind, while a string-literal member or access uses the name
  itself: such a type displays the escaped spelling, and `o["__x"]`, `o.__y`
  for a member `"__y"`, and `canHaveSyntheticDefault`'s `__esModule` marker
  written as an identifier are not found, where TypeScript 7 finds them.
- Lexical searches do not infer receiver/member identity. Semantic member
  searches are a separate coverage baseline; their failures and deliberate
  differences are described below and in [types.md](types.md).

## Loading and lexical differences from TypeScript 7.0.2

The semantic oracle is TypeScript 7.0.2's language service (see
[validation](validation.md#reference-semantics)). Its results are normalized
to this model before comparison. Normalization keeps every program file's
locations, so it cannot hide a missing or extra occurrence; it names files by
canonical path, as this tool does, unless `preserveSymlinks` is set.

### Symbols and names

- TypeScript gives every same-named declaration in one table a single symbol.
  This tool keeps separate symbols per meaning unless the declarations merge,
  so `interface X` and `declare var X` are separate groups.
- TypeScript reports several groups for aliases of one symbol; this tool
  reports one group for the underlying symbol. TypeScript also creates a
  temporary symbol for each `import * as ns`; groups are identified by their
  declarations instead.
- A value use of a type-only alias is unresolved here; TypeScript reports an
  error but still resolves it. When star re-exports reach one declaration by
  both a type-only and an ordinary route, the ordinary route makes it a value
  here; TypeScript keeps the first route and may report that error.
- A name that two `export *` sources supply with different targets is
  ambiguous and unresolved here; TypeScript reports TS2308 and resolves it to
  the first source. An augmentation that declares such a name becomes the
  module's own export here; TypeScript merges it into the first source's
  symbol.
- Every module and global augmentation of one declaration merges into one
  identity here, and every alias resolves against the merged declarations.
  TypeScript 7.0.2 keeps the alias targets it resolved while merging: an
  `import`, `export { X } from` specifier, or UMD global (`export as
  namespace`) resolved during a merge can keep the unmerged declaration, an
  import through a pattern ambient module keeps the pattern's own unmerged
  export, and augmentations that reach one declaration through different
  modules' exports can each clone it, so its declarations split into several
  groups depending on program order. Likewise, when the export an
  augmentation names is an alias in a cycle (TS2303), the augmentation's
  declaration replaces it, and the cycle's aliases and their importers then
  resolve to that declaration here; TypeScript leaves them unresolved.
  Projects in `fixtures/reference-exceptions` record the compiler's answers
  beside this tool's for each of these and for the ambiguous star export
  above; `tests/reference_exceptions.rs` checks this tool's, and a real-oracle
  test in `tools/semantic-oracle.test.mjs` the compiler's.
- When a later augmentation of a pattern-matched specifier declares a name
  that only the pattern declared, TypeScript merges it into the pattern's own
  symbol, so the pattern's declaration joins that augmentation's group; here
  every augmentation of the specifier extends that specifier only, and the
  pattern's declaration stays with the other specifiers' references
  (`pattern-later-augmentation` in `fixtures/reference-exceptions`).
- In a dependency file that declares a name both exported and local (TS2395
  or TS2383, tolerated there), the declarations merge into one symbol, so its
  group also lists the local declarations; TypeScript's export symbol has only
  the exported ones. The same holds for an exported enum, class, or function
  merged with a local namespace in spaces that do not conflict, such as an
  exported enum and a local namespace of types: TypeScript gives the export its
  own symbol, here they form one group.
- `undefined`, `arguments`, and `globalThis` are built-in non-reference
  targets. A missing export of a namespace, module, or `globalThis` qualifier is
  unresolved; any other unresolved `a.b` name is a property. Receiver types are
  not inferred, so `value.member` is a property even where TypeScript resolves
  it through a variable's `typeof namespace` type.
- A named import from a shorthand ambient module (`declare module "x";`)
  denotes the module, as in TypeScript; a member reached through one is
  unresolved. Such an import resolves in a type position too, where
  TypeScript reports TS2709. A default import of a declaration file or ambient module with
  neither a default export nor an `__esModule` export denotes the module
  object, as TypeScript's synthetic defaults do; an ordinary module without
  one has no default.
- TypeScript stores an import and a same-named local declaration of another
  meaning in one table symbol; uses select the meaning they need, and the
  import clause name denotes only the imported declaration.
- Non-reference syntax is unsupported even where TypeScript resolves it: a
  binding-pattern key such as `default` in `const { default: x } = await
  import("m")`, `const` in `as const`, `global` in `declare global`, a `this`
  parameter, `this` in a type query such as `typeof this.x` (TypeScript
  resolves it to the enclosing class), and a tuple member label.
- A constructor parameter property's name belongs to the parameter's group;
  TypeScript resolves the name itself to the property.

### JSON modules

- A JSON module's value and properties are not reference targets here;
  TypeScript gives an import of them a group of the import's own
  occurrences, and a property access such as `data.a` a group that includes
  the key in the JSON file. The oracle classifies both as members. An
  interface that an augmentation merges beside such a value is a separate
  symbol of the type meaning here, as for other declarations of several
  meanings, so a type use of the name denotes the interface; TypeScript's one
  symbol is a property, which the oracle classifies as a member
  (`fixtures/json-probes/json-augmentations.json`).
- The members a JSON string, number, boolean, or array has are those that the
  declarations of its global library interface declare. Where one of those
  declarations extends other types, a name none declares is unsupported here;
  TypeScript also finds the inherited members and leaves other names
  unresolved. A member is one target per interface and name, so two arrays'
  `length` through two `export *` routes is one export here; TypeScript
  instantiates an array's members per element type, so arrays of different
  element types make the name ambiguous there (TS2308). A library member's
  own type is not inferred, so a name selected from it, as through
  `export = length`, is unsupported; TypeScript resolves it from the member's
  type (`fixtures/json-probes/linking.json`).
- A JSON file with a syntax error fails loading, as other inputs do;
  TypeScript reports the diagnostic and still binds what it parsed.

### Standard library files

- Standard library files are never searched. TypeScript's own
  find-all-references also reports occurrences inside them; this tool, like the
  semantic oracle's model, reports their declarations in groups but no
  occurrences from them.
- Libraries of equal program-order priority, which only replacement
  packages and a `.full` default can be, keep TypeScript's depth-first
  discovery order here; TypeScript 7.0.2 sorts more than twelve libraries
  with Go's unstable sort, which may reorder them. Only file order differs.
- A file reached both as a library and through another route, such as a project file naming a vendored or
  replacement library by `/// <reference path>` or import, takes the
  provenance and the program-order place of the route that reaches it
  first, as TypeScript's loader takes both from the task it starts first:
  a root file, then a root library, then breadth first, each file's routes
  in its subtask order. Where two routes are equally deep, TypeScript's
  parallel loader starts either first, so its choice varies between runs;
  this tool takes the earlier one in program order
  (`tests/reference_libraries.rs`).
- Replacement packages are resolved from the configuration's real directory;
  TypeScript uses the directory as the configuration path spells it, so
  through a symbolic link it can find a package beside the link that this
  tool does not (`tests/reference_libraries.rs` and, for the compiler,
  `tools/semantic-oracle.test.mjs`).
- An unknown library reference is reported without TypeScript's spelling
  suggestion (TS2727).

### Diagnostics and failures

- An unknown compiler option whose name differs from a known one only by
  case, such as `Strict`, is TS5025 in TypeScript 7.0.2, which names the
  option it found lowered (`CommandLineOptionNameMap.Get`); 7.0.2 makes no
  other suggestion. This tool reports both as `Unknown compiler option`
  without the suggestion; the error is the same `Config` failure.
- Checker diagnostics that do not change what a name denotes are not reported,
  as for other checker rules: the import attribute and named-import rules for
  JSON under `node18` and later (TS1543, TS1544), `export *` from a JSON module
  (TS2498), an `import =` under ECMAScript module output (TS1202), and TS6142
  for an import of a `.tsx` file without `jsx` that another route makes a
  program input, which the import still names.
- A syntax error inside a token, such as an invalid escape, digit, or numeric
  separator, or an unterminated string or comment, is reported at the
  token's start; TypeScript reports it at the offending character, as
  `"\00"` at its `\`. Where TypeScript's error recovery parses on and its
  JSON validation then reports an earlier node, such as a JSON member that is
  not a property assignment (`{get "a": 1}`, `{default "a": 1}`) or a value
  that is not JSON (`{"a": 1 ? }`), this tool reports where its parser stops,
  later in the file (`fixtures/json-probes/parsing.json`).
- Invalid declaration arrangements that TypeScript binds but its checker
  rejects are `Binding` errors here where reported (Binding model): duplicate
  implementations (TS2393), a function merged with a non-ambient class
  (TS2813/TS2814), a repeated type parameter (TS2300), a hoisted `var` past a
  block's `let` (TS2481), a name declared both exported and local in one
  container in overlapping declaration spaces (TS2395, or TS2383 at the
  overloads whose export differs from the implementation's or the first's,
  so an enum and a namespace of types may differ),
  an instantiated namespace before or in another file from the class or
  function it merges with (TS2434/TS2433; a namespace of const enums only
  counts under `preserveConstEnums`, `isolatedModules`, or
  `verbatimModuleSyntax`), and merged classes or interfaces whose type parameter
  names disagree by position or omit a parameter that no declaration gives a
  default (TS2428), and an import whose target shares a meaning with a
  same-named local declaration, even when it resolves back to that local
  through its own module (TS2440). Constraints and default types are not
  compared, including between a declaration and its module augmentations.
- An unresolved side-effect import fails the project, as TypeScript reports
  TS2882, unless `noUncheckedSideEffectImports` is false.
- Binding errors and missing inputs in dependency and standard library files
  are not reported (Binding model), so a name that depends on such a missing
  input is unresolved, and conflicting declarations there merge; TypeScript
  reports them unless `skipLibCheck` or `skipDefaultLibCheck` covers the
  file (`fixtures/json-probes/reporting.json`).
- A namespace declared with the `module` keyword (`declare module X {}`)
  binds as a namespace in a dependency declaration file whatever
  `skipLibCheck` says, as other dependency diagnostics are not reported;
  TypeScript 7 reports TS1540 there unless `skipLibCheck` covers the file
  (`fixtures/references/module-keyword-namespaces`). In a project file, or a
  project declaration file without `skipLibCheck`, it fails the load at its
  name, as TypeScript reports it, unless `// @ts-nocheck` or a comment
  directive covers it. File-local `--check` always reports it, applying
  neither `skipLibCheck` nor directives: it reads no configuration, so it has
  no dialect and reports TypeScript 7's errors.
- An unresolved module or reference directive leaves the names that depend
  on it unresolved instead of failing the project where TypeScript reports no
  error: in a file it skips, such as a declaration file under `skipLibCheck`
  or a file under `// @ts-nocheck`, or after a `@ts-expect-error` or
  `@ts-ignore` comment directive (as for binding errors). The directive
  rules follow TypeScript 7.0.2 literally (`crate::pragmas`, from
  `processCommentDirective` and `isCommentOrBlankLine`). A directive is a
  `//` comment, after any further slashes and then only spaces and tabs, or a
  block comment's last line, after spaces and tabs and then any run of `/`
  and `*` and again spaces and tabs, that continues with `@ts-ignore` or
  `@ts-expect-error`, case-sensitively and as a prefix (`@ts-ignored`
  counts); any other whitespace, such as a no-break space, vertical tab, or
  form feed, ends it. It covers the next line after the line it starts on,
  and the lines after that while every line between is blank or a comment
  line in TypeScript's narrow sense: after spaces and tabs only, the line is
  empty up to CR, LF, or the end of the file, or begins with `//`. This is
  not the ECMAScript notion of a blank line: a line holding a no-break space,
  vertical tab, form feed, or byte order mark, or an empty line that U+2028
  or U+2029 ends, stops the directive, and the error fails the load as
  `tsc` reports it (`fixtures/json-probes/suppression-lines.json` and
  `suppression-directives.json`). Such a comment counts anywhere between
  tokens, including inside an import's braces or after code on its line, but
  not inside a string, template, regular expression, or JSX text. A missing automatic JSX runtime is reported once per
  file, at its first JSX tag in source order; TypeScript uses the first tag it
  checks, which can be later in source when a function expression's body is
  checked late.

## Semantic type and member-query differences

The semantic owners implement the contract in [types-design.md](types-design.md),
including its deliberate differences from TypeScript. This supersedes lexical-only
member exclusions for semantic queries. The [completion checkpoint](types-completion-correctness-2026-10-13.md)
records the current validation status; this section is not a delivery-complete claim.

- Coverage names its operation: existing lexical search versus semantic member
  search. A complete lexical result does not promise all receiver/property
  occurrences in the compiler's broader reference operation. Bare-name results
  disclose matching catalog member selectors that were not searched.
- Member candidates use exact decoded keys instead of 7.0.2's raw-text bounded
  substring optimization, covering escaped and equivalent numeric spellings.
  Compiler syntax/meaning eligibility still applies; arbitrary computed-key
  variable tokens are not wildcard property candidates.
- Member identity uses the resolved combined early/late member, independent of
  cold language-server query order. This also permits established literal-name
  uses of computed-only members that the cold server omits. Declaration origins
  and eligible occurrence tokens remain different questions.
- A parameter property's name preserves lexical parameter selection; its first
  property-producing modifier is a member anchor. Dotted namespace candidates
  anchor the actual selected segment. Ambiguity anchors must round-trip through
  that operation's selection owner.
- Q4 normalizes declaration/alias meanings for useful inspection. Both type-only
  import/export spellings follow one table; type-reference name tokens and a
  one-name `import("...")` qualifier select their type syntax, while the names
  of a qualified import-type qualifier are unsupported targets. Explicit closing delimiters can select call/access/tagged
  results. These product anchors use node-level type operations rather than
  claiming equality to every raw compiler position query. Unsupported catalog
  forms and positions inside JSON remain explicit target failures.
- Checking starts fresh per semantic request, resolves containing calls
  outermost-first, and uses a defined candidate/expansion schedule. It does not
  inherit earlier queries' caches or promise the same limit/context history as
  a complete `tsc` diagnostics pass. Oracle fixtures must record raw observations
  and the exact normalized sequence; a diagnostics request alone does not settle
  the API checker. Compiler constants, reset points and operation outcomes remain
  unchanged inside that schedule.
- Compiler-process numeric IDs do not affect semantic identity. Cache keys
  distinguish opaque symbol/type/node identities from exact user-name bytes;
  adjacent name bytes still concatenate as in the native operation. Generated
  private/unique keys retain identity and exact textual components with the
  request-local ordering defined in the
  [evaluation baseline](types-design.md#evaluation-order-baseline). This
  deliberately removes native collisions between encoded IDs and property
  names, and decimal-ID ordering in generated names. The
  [compiler identity evidence](#compiler-identity-evidence) shows that unrelated
  process allocations can otherwise change a final type. The
  implementation and final-query proof remain separate completion obligations.

The design also defines product call/test filters, bounded presentation,
unsupported-flow/JSX gaps and typed failures. These limitations are not a claim
that TypeScript itself cannot analyze those sites.

## Compiler identity evidence

These pinned compiler observations explain the deliberate identity differences
above. They were retained from the historical identity review; the final
[completion checkpoint](types-completion-correctness-2026-10-13.md) owns delivery
status. The maintained oracle tests and fixture paths below preserve the
reproduction inputs.

### A fresh checker does not define one native answer

Pinned TypeScript Go 7.0.2 stores `nextSymbolId` and `nextNodeId` in process-global atomics (`internal/ast/utilities.go`, `GetSymbolId` / `GetNodeId`). A new checker does not reset them. `GetSymbolId` assigns on first use, not on symbol construction. Concurrent compare-and-swap can also consume unused numbers.

The maintained control is `tools/type-symbol-id-collision-oracle.test.mjs`, with source in `fixtures/types/native-symbol-id-collision/cases.ts`. Registered run **vrun-969941b9-9e68-43e7-99f3-05c206668c19** passed all three tests with zero skips. The earlier inequality probe passed as **vrun-ca455c2c-e6e3-4d55-a95d-e8768ee8618f**.

The fixture assigns a number to a literal property and a string to a stable dynamic index, both before and inside a loop. Its selected result reads the literal property first and the dynamic index second:

```ts
obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"] = 0;
obj[key] = "value";
// The same two assignments occur in the loop.
const answer = {
    warm: obj["@\u0002\u0000\u0000\u0000\u0000\u0000\u0000\u0000"],
    result: obj[key]
};
```

The test-only oracle allocates zero, one, or two unrelated native symbols before constructing the same fresh program/checker. It uses the same serial binding setting and call prelude each time. The maintained assertions compare every captured input hash and the compiler options across the three executions. The hook observes the ID inside the existing `keyBuilder.writeSymbol` call; it does not make an extra ID request.

| Prior unrelated IDs | Actual `key` ID | Final native summary |
| --- | --- | --- |
| 0 | 2 | `{ warm: number; result: number; }` |
| 1 | 3 | `{ warm: number; result: string; }` |
| 2 | 4 | `{ warm: number; result: string; }` |

Every result has `errorType == false` and no acquired demand failure. The only permitted diagnostics are the expected missing-global-library diagnostics from `noLib`. Replacing the literal key with ordinary decimal text `"@2"` returns `result: string` under both tested histories.

This is a final type difference, not merely a changed private cache key. It demonstrates that the current fresh-checker wording alone leaves an observable process-history dimension unspecified. It does not show that every native ID assignment is semantically observable.

### Mechanism and concurrent binding evidence

`checker.writeFlowCacheKey` (`internal/checker/flow.go`) writes a literal access as its object key followed by `'.'` and raw property-name bytes. A stable identifier index writes the same object key followed by `".@"` and `keyBuilder.writeSymbol`. The latter writes **eight little-endian binary bytes**, through `hashWrite64`; it does not write decimal digits. When the symbol ID is 2, the two byte sequences in the fixture are identical and reuse the same loop-flow cache entry.

Native type IDs use four little-endian bytes. Node IDs use the same eight-byte numeric representation as symbol IDs. The decision therefore concerns all identity-bearing key components, not only this one dynamic-index branch.

Binding also affects the process epoch. `binder.GetSymbolNameForPrivateIdentifier` embeds `GetSymbolId(containingClassSymbol)` in `FE#<decimal ID>@<description>`. `Program.BindSourceFiles` queues each source file through `core.NewWorkGroup`. The default work group starts goroutines. The single-thread work group pops from the end of its queue: it binds files in **reverse** program order.

The third maintained test creates 12 independent private-field classes, reads only their already-bound private-name suffixes, and queries the same unrelated scalar result. Reading the suffixes requests no new ID or type. Two serial fresh runs assigned classes C0 through C11 the IDs `[12,11,10,9,8,7,6,5,4,3,2,1]`. Four parallel fresh runs recorded these different permutations:

```text
[2,3,4,6,8,1,5,7,10,11,9,12]
[2,3,4,5,7,1,6,8,11,9,10,12]
[2,3,4,6,7,1,5,8,10,9,11,12]
[2,3,4,5,1,6,11,12,8,7,9,10]
```

The test asserts serial ordering and that every parallel observation is a permutation of the actual allocated IDs. It deliberately does not require a particular parallel permutation, or require two runs to differ. The captured artifact is `.cc/temp/type-symbol-id-bind-observations.json`. All six scalar results were `0`; this concurrency control demonstrates variable allocation order, not a second final-answer difference.


### Chosen policy and generated-name length

The alternative of reproducing one native process epoch would require a fresh
process, explicit binding order, and the precise intrinsic initialization and
ID-acquisition schedule. This tool instead retains opaque symbol, type, and
node identities, separate from exact user-name bytes, with one deterministic
request-local lazy ordering owner. Native named-dot byte concatenation remains
unchanged; the exception concerns identity serialization and generated names.

Native nodebuilder's `addPropertyToElementList` charges
`len(ast.SymbolName(propertySymbol)) + 1` to approximate display length.
`getESSymbolLikeTypeForNode` embeds the decimal `GetSymbolId` in unique names,
so native truncation and subsequent dependencies can vary with unrelated
process allocations. This tool counts each opaque identity atom as one logical
unit, retaining every fixed prefix, separator and exact textual byte, recursively
for generated underlying names. Length reads do not acquire ordering identities.
The native `SymbolName` special case for a real private-name declaration still
precedes this calculation. Fixed names retain native lengths and cutoffs.

The historical typed-key final-query regression is `tests/type_flow_identity.rs`;
the matching strict/loose compiler controls are
`tools/type-flow-identity-oracle.test.mjs`. Actual equal-name/declaration symbol
ties are covered by `tools/type-symbol-order-identity-oracle.test.mjs`. Each
control retains its own scope; the binding-order observations above establish
variable allocation order rather than a second final-answer difference.
