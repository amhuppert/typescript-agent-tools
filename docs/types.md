# Type and member queries

Use one explicit TypeScript project for either query:

```sh
tsparse --type --project tsconfig.json --name createRunHost --expand --json
tsparse --type --project tsconfig.json --at src/runs/host.ts:42:10 --expand --depth 3
tsparse --references --project tsconfig.json --name RunHost.observe --calls --no-tests
tsparse --references --project tsconfig.json --at src/runs/host.ts:42:10 --json
```

`--direct` loads a fresh project in the CLI process. Otherwise macOS and Linux
use the existing project service. Each request checks for changed inputs before
answering; semantic checker state is fresh per request. A warm service retains
the project snapshot, not a history-dependent checker session.

## Selection

Positions use one-based lines and UTF-16 columns. `--at LINE:COLUMN FILE` and
`--at FILE:LINE:COLUMN` are equivalent. `--name` uses exact declaration names;
a dotted selector follows declaration containment. An optional FILE narrows
candidates. Ambiguous selectors return candidates rather than choosing one;
`--type` lists at most 50 of them unless `--limit N|all` or `-o` says
otherwise, counts every one, and ends a cut with the command that lists the
rest, as show does ([display limits](navigation.md#display-limits)).
Positions inside JSON modules are not selectable.

For references, a bare name keeps lexical selection across matching declarations
and aliases. Its `unsearchedMembers` suggestions identify matching catalog members
outside that question. Use a dotted member selector or a member declaration/use
position to ask the semantic question. Receiver types and native symbol
relationships establish member identity; matching spelling alone is insufficient.
A parameter property's name selects the lexical parameter, while its first
property-producing modifier anchors the property.

## Results and limits

Type output is text by default; `--json` gives a structured result. `--expand`
adds a graph with default depth 3; `--depth` accepts 0 through 32 only with
expansion. Presentation also limits the graph to 2,000 nodes and the type
description to 256 KiB. Omitted detail is explicit. These presentation limits
do not alter type identity or reference matching.

References are text, and with `--json` their result document, which
`--pretty` alone writes indented. `--calls` retains direct call, constructor,
and tagged-template targets; `--no-tests` removes occurrences in recognized
test paths; `--file FILE` keeps the occurrences in one searched file. Each
preserves selected declarations, and results disclose their effective filters. The precise path
and call rules are in [references.md](references.md).

A successful complete answer exits 0. Partial semantic answers, missing or ambiguous type/dotted-reference
selections, and analysis failures exit 1; usage, transport, and delivery
failures exit 2. A partial reference result includes located coverage gaps; a
partial type expansion includes issues. An unavailable selected root type is a
failure, not a successful `any` fallback. With `--json`, analysis failures are
result documents, and so are usage and transport failures, on the
[channels](navigation.md#exit-codes-and-channels) navigation gives them: a
usage failure's document on stdout, never `-o`, and a transport failure's
(`transport.*`, such as a runtime directory that is not private) on stdout or
`-o`, never beside stderr. Without `--json`, each is `CODE: message` on stderr.

TypeScript 7.0.2 defines the native semantics. This tool is not a whole-program
diagnostics replacement: each query follows its own documented demand schedule,
and unsupported dependencies remain explicit. Supported families and deliberate
normalizations are detailed in the [technical design](types-design.md) and
[semantic differences](references.md#semantic-type-and-member-query-differences).

## Process protection

Read-only CLI queries use a 6144 MiB process-memory threshold and a 60-second
elapsed-time limit. Override them with positive integer environment variables:

```sh
TSPARSE_MAX_MEMORY_MB=8192 TSPARSE_QUERY_TIMEOUT_MS=60000 tsparse --type --direct --project tsconfig.json --name createRunHost
```

A service inherits limits when it starts. Stop it with `--references-stop
--project tsconfig.json` before changing them; `tsparse --doctor --project
tsconfig.json` reports a running service whose limits differ from the current
environment's, with that command. A memory or time failure terminates
that query process and discards its cache, and its message names the variable,
its value (and for memory the footprint observed), and that cost:
`query exceeded TSPARSE_QUERY_TIMEOUT_MS (60000 ms) in the project service;
that service and its cache exited, and the next request starts a replacement`,
or `…; this query process exited` with `--direct`. Its remedy, `set a higher
TSPARSE_QUERY_TIMEOUT_MS before retrying` (or `TSPARSE_MAX_MEMORY_MB`), is the
`  next:` line in text and `error.remedy` in JSON. The next service request
starts a new process with its own environment; the failed query is not
automatically retried. Other clients queued on the old process may receive a
transport failure.

Memory is sampled every 25 ms using macOS physical footprint or Linux resident
plus swapped memory. This is a threshold that can overshoot, not a hard OS cap;
reporting a structured failure is best effort if the allocator or OS fails first.
Monitoring currently supports macOS and Linux. Normal query completion cancels
and joins its watchdog before response delivery. File application is outside
this scope. The Rust library does not terminate its embedding process.

Native cycle rules and a separate recursion guard remain active. A depth failure
stays a whole-query failure. The implementation has no per-allocation or per-work
fee ledger.
