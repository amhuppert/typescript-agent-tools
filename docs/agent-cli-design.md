# Agent-facing CLI surface: technical design

Status: proposed, 2026-10-18, from the audit of `tsparse` against the
`agentic-engineering-principles` skills (collaboration run `6fc1ba6b`). It
designs every change that audit recommended, in the delivery order both
reviewers agreed on, and names the one owner for each decision as
[AGENTS.md](../AGENTS.md) requires. It changes no analysis, binding, or
editing semantics: every change below is in how requests are parsed, how
outcomes are rendered and delivered, how the service explains itself, and
what the documents say. Existing contracts that the audit found correct are
listed under *Preserved behavior* and are not reopened.

## Purpose and operating envelope

An agent should be able to discover an operation from its own help, read a
bounded answer, branch on a typed failure without parsing prose, follow every
printed command as printed, and diagnose a misconfigured service without
guessing. The consumer is a program that must branch on results and a
language model that reads the text; the two have different needs, and the
design serves each where it reads.

Keep the native Rust implementation, the vendored offline build (no new
crate: every table, parser, and quoting rule below is plain Rust in this
crate), the TypeScript 7.0.2 baseline, and the one-process-per-request CLI
with its optional per-user service on macOS and Linux. Callers are local CLI
processes, the Node validation harness, Command Center's wrappers, and
in-repository Rust library consumers. Exit codes 0, 1, and 2 keep their
present meanings throughout this delivery; a numeric migration is an open
decision recorded at the end, not part of the design.

Measured motivation, from the audit's probes on synthetic inputs:

| Probe | Output |
| --- | --- |
| Default outline of 1,001 top-level declarations | 1,002 lines, 39.8 KB text; 224 KB JSON |
| References for one declaration with 1,000 uses | 126 KB JSON, no text form |
| Show of a function with 1,000 body statements | 1,003 lines, 24.9 KB |
| `tsparse --references --help` | the same 117-line, 8.7 KB text as bare `--help` |

Reproduced defects: a plan named `my plan.json` prints an apply command that
fails when followed; an automatic service start with
`TSPARSE_SERVICE_IDLE_SECS=0` reports only `startup failed` while the
foreground mode names the cause; the parser accepts `--output` that help
omits; three usage errors enumerate stale operation sets; a running service
keeps the limits it started with while the client's environment says
otherwise, and nothing reports it.

## Preserved behavior

These are the contracts the audit found correct. Each later section extends
them through their existing owners rather than adding a check callers must
remember.

- One parsed-request boundary (`cli::request::parse`): nothing starts or
  loads for an invalid request.
- One delivery policy (`cli::delivery`): a JSON outcome is never accompanied
  by stderr; a text failure never creates or replaces the `-o` file; with
  `--json`, failure documents and partial results reach `-o` by design; a
  result that cannot be written is one diagnostic, never followed by a second
  document.
- `cli::delivery::failure_code` is the one owner of analysis-failure exit
  codes, with `UNSELECTED`, `REJECTED`, `load_failure_code`, and
  `applied_code` beside it.
- Reference `--json` already delivers analysis failures as documents;
  successful reference encoding is JSON today. The text form below is added
  beside that, not instead of it.
- Preview, plan, apply as separate steps; apply checks bytes and identity,
  restores on failure, and reports `effect` independently of success; plans
  refuse another build.
- Type presentation budgets: 512-byte summaries, 2,000 graph nodes, 256 KiB
  descriptions, with exact byte-boundary tests in
  `tests/type_presentation.rs`.
- Explicit project identity: `--project` is required, no ancestor directory
  is searched, a transport failure never becomes a direct load, and a service
  is keyed by the selected configuration spelling and the compiled build
  identity. Distinct spellings owning distinct services is deliberate, because
  a spelling through a symbolic link can be retargeted.
- Status is administrative: it neither loads a project nor validates
  freshness, and nothing added below may imply that it did.
- Service and direct answers are byte-identical for every operation
  (`tests/project_service.rs`), and remain so for every rendering change
  below.
- Raw AST emission stays JSON; it is machine data.

## Alternatives and decision

A verb layer (`tsparse references …`) would give help a natural graph, but it
replaces every documented invocation and buys nothing the registry below does
not. Decision: keep the flag-as-operation syntax, route `--help` by the
operation flag present, and record a verb layer as optional later cleanup.

A clap-style dependency would supply help derivation, but the build is
offline with vendored crates, and the present parser's exact-name rules (a
non-Unicode argument is a usage error, `--at` read from the right) are
easier to keep in a hand-written parser driven by a small table. Decision:
a typed registry in this crate, with the parser's match arms, help, flag
scopes, and the documented reference all derived from it.

Caps could live in the library results, but a bounded view of a complete
search must not become a partial answer. Decision: caps are a rendering
decision in `cli`, never a change to a library result, and JSON reports what
rendering withheld under a `display` key that is distinct from `coverage`.

A single all-user service inventory was proposed and deferred: the endpoint
key is a one-way hash, so an inventory needs either an identity field in the
status reply or an identity file beside each lock, and that is a protocol
change to justify separately. Decision: the first `--doctor` is scoped to the
selected project; the inventory is recorded under *Later work*.

## Delivery order

1. Two reproduced defects: printed commands are shell-safe, and a failed
   automatic service start reports its cause.
2. The operation registry: per-operation help, derived flag scopes and
   scope errors, structured help, a generated reference, and a contract test.
3. One typed failure boundary: usage, batch, administration, and transport
   failures carry a category and code to text and to `--json` alike.
4. Bounded query presentation: a reference text form, display caps for
   references, outlines, candidates, show, and previews, a selection echo,
   and the tests that prove the bounds.
5. Project-scoped diagnostics: `--doctor`, a richer status, the build stamp
   in `--version`, lifecycle-aware limit messages, refusal wording with
   remedies, clean verdicts, and documentation routing.
6. Later work, each item with the evidence that would justify it.

Steps 3 and 4 are independent of each other and may be scheduled by
implementation dependency; step 2 precedes both because every later change
adds flags and messages that the registry's contract test must cover.

## Step 1: shell-safe printed commands and a reported startup cause

### Command rendering owner

`cli::command_line` becomes the one owner of turning an argument list into
text a shell accepts:

```rust
/// `argv` as one POSIX shell command line: each argument that is not a safe
/// word is single-quoted, with `'` written as `'\''`.
pub(crate) fn render(argv: &[impl AsRef<str>]) -> String;
```

A safe word matches `[A-Za-z0-9_./:=@%+,-]+` and does not begin with `-`
unless it is one of this program's registered flags; everything else is
quoted. Every place that prints a follow-up command calls `render`: the
preview's `plan: …; apply it with:` line and the `the plan was saved to …;
apply it with:` line in `cli::editing`, and every hint added later. The
JSON forms carry the same action structurally:

```json
"next": { "argv": ["tsparse", "--apply", "my plan.json"] }
```

The exact path identity is not altered; only its display is. A path that is
not Unicode cannot be rendered into a command line and is reported as it is
today, in lossy form in the message and `null` in identity fields.

### Reported startup cause

Two changes, both in existing owners:

1. **Client preflight through the shared environment owner.** The service
   and the client ask the same question about `TSPARSE_SERVICE_IDLE_SECS`,
   `TSPARSE_SERVICE_DIR`, `TSPARSE_MAX_MEMORY_MB`, and
   `TSPARSE_QUERY_TIMEOUT_MS`, so one owner answers it:

   ```rust
   // process_limits::Environment, the one reader of TSPARSE_* variables.
   pub(crate) struct Environment { pub memory_mb: u64, pub timeout_ms: u64,
                                   pub idle_secs: u64, pub service_dir: PathBuf }
   impl Environment { pub(crate) fn read() -> Result<Self, Failure /* Usage */>; }
   ```

   `Endpoint::start` reads it before spawning, so an invalid value fails in
   the client with the same message the service would have printed, exit 2,
   and no child is spawned. `service::serve`, `process_limits::run`, and the
   doctor read the same owner. The parse rule is unchanged: a positive
   integer, a missing variable takes the default, anything else is a usage
   error naming the variable, the accepted form, and the default.

2. **A bounded startup channel for every other cause.** The child, started
   with null standard streams as today, writes the text of a startup failure
   that occurs before it binds its socket (a lease it cannot take, a session
   it cannot create, a socket it cannot bind) to `<key>.start-error` in the
   runtime directory, then exits non-zero. The client, on observing the
   non-zero exit in `Endpoint::start`, reads and removes that file and
   reports its text as the transport failure; a file that is absent is the
   only tolerated read failure, and then the present generic message is
   used with the instruction to run `--references-serve`. The file is
   created with mode 0600 in a directory already required to be private, is
   written once, and is removed by the client or overwritten by the next
   start, so nothing accumulates. No pipe is passed to the child, so the
   descriptor rule in `close_inherited_descriptors` is untouched.

Verification: a test that spawns the real binary with each invalid variable
and asserts the exact client message and that no service process exists
afterwards; a test with an unwritable runtime directory and one with a
socket path over the limit, asserting the cause reaches the client; and the
plan-hint test that prepares a plan under names with a space, a single
quote, `$`, `;`, and a non-ASCII character, then runs the printed line
through `sh -c` and asserts the apply result.

## Step 2: the operation registry

### Registry

`cli::registry` holds one static table, the single source of the CLI
vocabulary:

```rust
pub(crate) struct Flag {
    pub name: &'static str,            // "--project"
    pub aliases: &'static [&'static str], // ["-o"] for "--output"
    pub kind: Kind,                    // Boolean | Value("TSCONFIG")
    pub summary: &'static str,
    pub repeat: Repeat,                // Reject | LastValue (recorded, see below)
}
pub(crate) struct Operation {
    pub name: &'static str,            // "references"
    pub flag: Option<&'static str>,    // "--references"; None for parse mode
    pub summary: &'static str,
    pub usage: &'static [&'static str],
    pub flags: &'static [&'static str],// accepted flag names, global ones excluded
    pub examples: &'static [Example],  // at least one per operation
    pub related: &'static [&'static str],
    pub doc: &'static str,             // "docs/references.md"
}
pub(crate) static FLAGS: &[Flag];
pub(crate) static OPERATIONS: &[Operation];
pub(crate) static GLOBAL_FLAGS: &[&str]; // --json, --pretty, -o, --help, --version
```

Every surface derives from it:

| Derived surface | Replaces |
| --- | --- |
| The parser's flag match (name and aliases to a `Flag`) | hand-written arms, which today accept `--output` that help omits |
| Per-operation flag acceptance and the scope error `--json applies to: --outline, --show, --type, --references, --replace-body, --insert-after, --edit-batch, --apply` | `navigation_only` and the three stale messages in `cli::request` |
| `tsparse --help`: a terse index, one line per operation, one pointer line for global flags, the exit table, and `tsparse --OP --help` | the 117-line constant in `main.rs` |
| `tsparse --OP --help`: summary, usage, the operation's flags, examples, related operations, its document | nothing exists today |
| `tsparse --help --json` and `tsparse --OP --help --json`: the entry as a document | nothing exists today |
| `docs/cli-reference.md`: every operation, flag, example, and the exit table, generated | the hand-written exit tables in help, `README.md`, `docs/references.md`, `docs/navigation.md`, `docs/editing.md` |

The parser keeps its per-operation semantic checks (exactly one of `--at`
and `--name`, `--from` required, `--apply` takes only the plan) in
`cli::request`; those are rules about combinations, not vocabulary. Help is
resolved before any other validation: `--help` anywhere in the arguments,
with the operation flags present anywhere in the arguments, selects the
node, exits 0, reads no file, and contacts no service.

Examples teach the shapes the audit found failure-prone: `--at
FILE:LINE:COLUMN` beside `--at LINE:COLUMN FILE`; a bare `--name run` beside
a dotted `--name RunHost.run`; `--from body.ts` holding a complete `{ … }`
body; the operations that accept `--pretty` alone. `--pretty` keeps its one
rule, recorded in the registry and stated by its error: it indents JSON
output, so it is valid alone only where the output is JSON (parse mode and
references), and needs `--json` elsewhere. The two repeated-option policies
(parse and references take the last value; navigation, type, and editing
reject a repeat) are recorded per operation in `Repeat` so help and the
reference state them; unifying them is a separately tested change listed
under *Later work*.

Unknown options point back into the graph:

```text
Unknown option: --ouput; did you mean --output? See tsparse --help
```

### Generated reference and contract test

`tests/cli_registry.rs` renders `docs/cli-reference.md` from the registry in
an ignored test (`cargo test --test cli_registry -- --ignored write_reference`)
and, in a normal test, compares the checked-in file with the rendering and
fails on any difference, as `tools/generate-compiler-schema.mjs --check`
already does for syntax kinds. The five documents link to the reference and
drop their copies of the exit table. The same test asserts:

1. Every operation the parser dispatches has an entry, and every entry's
   flag is dispatched.
2. Every flag the parser accepts, aliases included, appears in `FLAGS`, and
   every `FLAGS` entry is accepted.
3. Every operation has a summary, usage, at least one example, and a
   document path that exists.
4. Every related edge names an operation in the table.
5. Every usage message in `cli::request` that names a flag or operation
   names one in the table (the messages are built from registry names, so
   this is asserted by construction and by a scan of the literal set).
6. The index lists every operation; each leaf lists exactly its flags.
7. No flag is boolean in one entry and value-taking in another.

The existing `help.contains(needle)` tests are replaced by these.

## Step 3: one typed failure boundary

### The failure type

`cli::Exit` is today `(u8, String)`. It becomes:

```rust
pub(crate) struct Failure {
    pub category: Category,     // Usage | Input | Transport | Write | Internal
    pub code: &'static str,     // stable, greppable: "usage.unknown_option",
                                // "usage.flag_scope", "input.batch", "transport.startup",
                                // "transport.directory", "transport.socket", "write.output"
    pub message: String,        // one line, what and the remedy
    pub why: Option<&'static str>,      // one sentence, only for designed constraints
    pub issues: Vec<Issue>,     // per-field detail, structural
    pub next: Option<Vec<String>>,      // argv of a follow-up command, when one exists
}
pub(crate) struct Issue { pub path: String, pub message: String } // "edits[1].name"
```

`cli::delivery` is the one owner of the exit code and the channel for a
`Failure`, as it is for analysis failures: in this delivery every category
maps to exit 2, in one function beside `failure_code`, so a later numeric
decision is one change there plus the consumer inventory recorded under
*Open decisions*. Analysis failures keep `failure_code` and their present
documents; nothing in this step changes what an `AnalysisError` renders.

### Rendering

Text mode: the message on stderr, then one `  path: message` line per issue,
then `  why: …` when present, then `  next: …` rendered by
`cli::command_line`. JSON mode: one document on stdout or `-o`:

```json
{"operation":"edit","outcome":"error","effect":"none",
 "error":{"category":"input","code":"input.batch","message":"batch.json: 2 edits are invalid",
          "issues":[{"path":"edits[0]","message":"give exactly one of name and at"},
                    {"path":"edits[1].extra","message":"unknown field"}]}}
```

`operation` is present when the request got far enough to name one;
`effect` is present for editing failures, as every editing document carries
it today. Two invariants stay: a document is never accompanied by stderr,
and if the document itself cannot be written, the failure is one stderr
diagnostic and no second document is attempted.

### Knowing that JSON was wanted

A usage failure can occur before the request is valid, so `cli::request`
records whether `--json` appears anywhere among the arguments before it
validates anything, and a `Failure` from parsing carries that flag to
delivery. The present exceptions stay, narrowed and documented: a usage
error about `--json` itself, an argument that is not Unicode, and
`--references-serve`'s own startup remain stderr text.

### Batch validation reports every issue

`cli::editing::batch` decodes a batch file in two stages: `serde_json::Value`
first, then a validator that walks every edit and collects every issue
(unknown field, both or neither of `name` and `at`, both or neither of
`fragment` and `from`, a malformed `at`, an empty batch) before building the
`EditRequest`. Strictness is unchanged: an unknown field is still an error.
Only the first stage can fail with one issue (the file is not JSON), and that
issue carries the line and column serde reports.

### Administration and transport

`service::status`, `service::stop`, and the transport errors in
`service::Endpoint` return `Failure` with `transport.*` codes and, for
directory and socket problems, the remedy and the reason:

```text
transport.directory: /tmp/tsparse-501 must be a private directory owned by this user (mode 0700)
  next: chmod 700 /tmp/tsparse-501
  why: any process that can open the socket can run queries as you
```

When the directory is not owned by the user, `next` names
`TSPARSE_SERVICE_DIR` instead. Every `transport.*` message ends with
`run: tsparse --doctor --project CONFIG`.

Verification: one test per failure stage (unknown option, scope error,
malformed `--at`, batch with several issues, missing input, non-private
directory, over-long socket path, failed write) in text and in JSON, through
direct and service entries where the stage exists in both, asserting the
exact document, the empty stderr beside a document, and the exit code.

## Step 4: bounded query presentation

### Rules shared by every renderer

- A cap is a rendering decision in `cli::references`, `cli::navigation`,
  `cli::types`, and `cli::editing`. Library results are complete; `outcome`,
  `coverage`, `gaps`, `issues`, and the exit code are untouched by a cap.
- One flag, `--limit N|all`, recorded once in the registry, bounds the
  operation's primary list: reference occurrences, outline entries,
  ambiguity candidates, show lines, and preview diff lines per file. Each
  operation has a default; `all` removes the bound.
- Every cut states what it withheld and names the exact command that
  reveals more, in text as the last line and in JSON under `display`:

  ```json
  "display": {"occurrences": {"total": 412, "returned": 200}, "truncated": true,
              "next": {"argv": ["tsparse","--references","--project","tsconfig.json",
                                 "--name","run","--json","-o","run.json"]}}
  ```

  `display` is present only when something was withheld, so unchanged
  outputs stay byte-identical and the service-versus-direct tests keep their
  meaning.
- Output to `-o` is the spillover channel: with `-o`, no default cap applies
  and the document is complete. Without `-o`, `--json` is capped like text
  and says so; a complete document past a cap belongs in a file the agent
  can read with its own tools.
- Handles stay stable and exact: files and `LINE:COLUMN` as today, never
  positional indexes. No offset or cursor is added in this delivery;
  narrowing is by filter. If one is ever added it carries the service
  generation and is refused when the generation changed.

### References in text

`cli::references` renders a `ReferenceAnswer`:

```text
tsconfig.json: `helper` 1 group, 10 occurrences in 3 files; coverage lexical, complete
group 1: helper, declared at lib.ts:1:31
  app.ts:3:39 import
  app.ts:5:23 import (assist)
  app.ts:8:1 use
  barrel.ts:3:10 export (assist)
3 catalog members named helper were not searched; search one with --name Receiver.helper
```

Occurrences are grouped by file and ordered as the result orders them; a
renamed alias shows its spelling in parentheses. Partial coverage prints each
gap as `gap FILE:LINE:COLUMN: reason: message` before the count line, so the
semantic facts are primary output and the display cut is last. Member
suggestions ride the hint tier: one line, imperative, naming the registered
flag. The default `--limit` for occurrences is 200.

A `--file FILE` **result** filter keeps occurrences in one file. It is
distinct from the FILE operand, which narrows candidate declarations, and
the registry's example and the help text say so.

### Selection echo

`ReferenceResult` gains `selection: ReferenceQuery`, and `ReferenceQuery`
is tagged `snake_case` (`{"name": {"name": "value", "file": "app.ts"}}`,
`{"at": {…}}`), which also corrects the `Name` key in today's not-found
document; this is a documented schema change with serialization tests. In
text the header says `candidates narrowed to app.ts` when a FILE operand was
given, so `groups: []` can no longer read as "no references". JSON ordering
puts `selection` after `project`.

### Outline, candidates, show, previews

- Outline: `--limit` (default 200 entries) applies after `--depth` chooses
  the levels; the cut line is `… N more entries; see all with --limit all or
  --json -o outline.json`. Import grouping and the eight-name rule stay.
- Ambiguity candidates (show, type, and editing selections): default 50,
  `… N more candidates; narrow with --name a.b or --at FILE:LINE:COLUMN`.
- Show: default 400 lines, printed from the declaration's first line; the
  cut line names `--limit all` and `-o`. The excerpt that reaches `-o` or
  `--json -o` is always complete, so no shortened preview can pass for the
  declaration.
- Preview diffs: default 400 lines per file; the cut line names the plan,
  which holds the complete proposed text, and `--json -o`. No changed file
  is ever omitted from the list of changed files, and `effect` is never
  affected.

### Verification

Tests ship with this step, modeled on `tests/type_presentation.rs`: for
each bounded list, inputs just under, at, and over the default, a wide
shallow file, long names and signatures, many groups, many candidates, text
and JSON, direct and service, with and without `-o`; the service and direct
outputs byte-identical; and, for every cut, following the printed `next`
command and asserting that every withheld item is recovered. Expected
occurrences and groups are taken from the pinned compiler where the tests
touch semantic behavior.

## Step 5: diagnostics, identity, messages, verdicts, routing

### `--doctor`

```sh
tsparse --doctor
tsparse --doctor --project tsconfig.json [--json]
```

A read-only report, one line per check, exit 0 when every check passes and
exit 1 when any finding is present. It never loads a project. Checks, in
order:

| Check | Without `--project` | With `--project` |
| --- | --- | --- |
| Build | compiled build stamp, package version, platform capabilities: service (Unix), apply (macOS, Linux), memory monitoring (macOS, Linux) | the same |
| Environment | each `TSPARSE_*` variable: value or default, and any parse failure, via the shared environment owner | the same |
| Runtime directory | path, exists, mode 0700, owned by this user, and the socket-path budget: directory length plus 22 against the platform limit | the same |
| Configuration | | the spelling as given, its absolute form, readable, and a note that another spelling of the same file would own a separate service |
| Service | | running or not; when running: pid, build (must equal the client's), generation, started time, idle timeout and remaining seconds, and the limits it inherited beside the client's current values, with a finding when they differ |

The limits finding states the mechanism and the remedy:

```text
service limits: TSPARSE_QUERY_TIMEOUT_MS 60000 inherited at start, 120000 in this environment
  next: tsparse --references-stop --project tsconfig.json
  why: a service keeps the environment it started with
```

### Status and version

The service's `Status` reply gains `build`, `limits` (memory and timeout in
effect), `idle_secs`, `idle_remaining_secs`, and `started_unix`. Clients and
services already refuse each other across builds, so the wire change needs no
compatibility shim. `--references-status` prints text by default:

```text
running: pid 59643, generation 1, build 18f3…-2a1c, idle 10m, 7m12s remaining
limits: 6144 MiB, 60000 ms
```

and the document with `--json`, which the registry now admits for status.
`--version` prints `tsparse 0.1.0 (build 18f3…-2a1c)`. Status still does
not validate freshness, and its text says `generation` without adjectives.

### Limit messages

`process_limits::Failure::into_error` receives the limits and the execution
context (direct or service) and renders:

```text
query exceeded TSPARSE_QUERY_TIMEOUT_MS (60000 ms) in the project service; that service and its cache exited, and the next request starts a replacement
  next: set a higher TSPARSE_QUERY_TIMEOUT_MS before retrying
```

In direct mode the consequence is `this query process exited`. A restart
instruction appears only where a service is still running with inherited
settings, which only the doctor and status can know, so the limit message
never tells a caller to stop a service that has already exited. The memory
message follows the same shape with the observed and configured values.

### Refusal wording

| Today | Change | Owner |
| --- | --- | --- |
| `Unknown option: --foo` | add the nearest registered name and `See tsparse --help` | `cli::request` from the registry |
| `--json applies to --outline and --show` and the two other stale scope messages | derived from the registry's per-operation flag sets | `cli::request` |
| `Reference service directory must be a private directory owned by this user: P` | `transport.directory` with `next` and `why` as in step 3 | `service::Endpoint::check_directory` |
| `--plan P already exists; an edit command writes its result and plan only as new files` | append `; choose another path` and `why: no outcome may replace a file the command read or edits` | `cli::editing::new_outputs` |
| `X cannot have its body replaced: this interface has no body to replace` | keep; add `member-list editing is not supported; select a declaration with a body` and never name `--insert-after`, which inserts after the whole interface | `editing::policy` message, rendered by `cli::editing` |
| `not one of the project's searched files` | append the configuration whose `files`, `include`, and `exclude` decide membership; a listing verb is later work | `cli::delivery::failure` for `OutsideProject` |
| `Reference service …` in every message, help, and document | `project service`; help states that `--references-status`, `--references-stop`, and `--references-serve` administer the service that answers every project query | `service`, `main.rs` help via the registry |

Every advertised remedy is executed in a scratch fixture by a test before it
ships, as the stale-apply message's `prepare it again` already is.

### Clean verdicts

| Command | Today | Change |
| --- | --- | --- |
| `tsparse --check FILE` | exit 0, no output | `ok: FILE parses as TypeScript 7 (jsx: off)` on stdout |
| `tsparse --references-stop --project C` | exit 0, no output | `stopped: pid N` or `no service: C` |
| `scripts/validate/format-full.sh`, `lint-full.sh` | silent when clean | `format: ok`, `lint: ok`, one line each, from `run_quiet` given a label |

Verdicts are one line, say what was checked, and never imply type checking.

### Documentation routing

- `AGENTS.md` keeps its five rules and existing pointers and gains a short
  *Read on demand* list with triggers, one line each: before changing a
  flag, message, or help text, `docs/agent-cli-design.md` and the registry
  contract test; before changing an exit code or output channel,
  `docs/navigation.md#exit-codes-and-channels`; before running an editing
  flow, that `--apply` and `tools/editing-flows.mjs` write files; before
  measuring the service, that it is one background process per user,
  configuration spelling, and build.
- `docs/agent-guide.md`, one owning consumer guide: when to use each
  operation instead of text search, the first command per task, the generated
  exit table by reference, the cost model qualified as measured on Command
  Center at a stated revision (warm queries about 0.15 s, a rebuild about
  9 s, a member search about 10 s, idle exit after 10 minutes), and the
  friction taxonomy. Designed friction to absorb: exact names, no partial
  batches, no ancestor `tsconfig.json` search, no fallback from the service
  to a direct load, project references rejected, exit 1 on partial results,
  new-file-only outputs, one writer during apply. Incidental friction to
  report: silent truncation, a message that misleads, a missing verb, a
  read that answers from the wrong state. If the guide is to reach agents
  in other repositories, its distribution path is defined with it; a file
  under this repository's `.agents/skills` does not provide one.
- `docs/validation.md` becomes a current-state index whose tables hold the
  latest result per check and link to `docs/validation-history.md`, where
  the dated re-run narrative moves unchanged.
- `README.md` keeps its sections but replaces its exit table and flag
  synopsis with links to `docs/cli-reference.md`.

## Ownership

| Decision | Owner |
| --- | --- |
| The CLI vocabulary: operations, flags, aliases, kinds, scopes, examples, related operations, documents | `cli::registry` |
| Which combinations of accepted flags form a valid request | `cli::request` |
| Rendering an argument list as a shell command line | `cli::command_line` |
| Reading and validating every `TSPARSE_*` variable | `process_limits::Environment` |
| The exit code and channel of every outcome, analysis failures and `Failure` alike | `cli::delivery` |
| Decoding and validating a batch file | `cli::editing::batch` |
| Display caps and the `display` key | each renderer in `cli`, through one `Cap` helper in `cli::delivery` |
| The text form of a reference answer | `cli::references` |
| Service identity, status fields, startup error file, transport failures | `service::Endpoint` and `service::handle` |
| The doctor report | `cli::doctor`, composed from the owners above; it decides nothing itself |
| The generated CLI reference and its drift check | `tests/cli_registry.rs` |

## Failure contract

Failures stay failures. A startup error file that is absent is the only
tolerated read failure in `Endpoint::start`; any other read error is
reported as the transport failure it is. The doctor reports a check it
cannot perform as a finding, never as a pass. Caps never drop a semantic
gap, issue, or candidate count; they withhold rendering and say so. A
`Failure` keeps its category and code until `cli::delivery` decides the
channel and code; no layer maps one category into another. A path that is
not Unicode stays a usage error and is never rendered into a `next` command.

## Verification plan

Each step is done when a test that fails without it passes for every
variant the rule distinguishes, asserting final CLI output:

1. Printed commands followed through `sh -c` for five path classes;
   startup causes for each invalid variable, an unwritable directory, and an
   over-long socket, with no service left running.
2. The registry contract test; `--help` and `--OP --help` for every
   operation, offline, exit 0, and byte-identical with and without a running
   service; the generated reference matching the checked-in file.
3. Every failure stage in text and JSON, direct and service, asserting the
   document, the empty stderr beside it, and the exit code; a batch with
   three issues reporting all three.
4. Every bounded list under, at, and over its default, wide and long
   inputs, text and JSON, with and without `-o`, direct and service
   byte-identical, and recovery of every withheld item by following `next`.
5. Doctor findings for each check, including the inherited-limits finding
   against a real running service; status fields; the version stamp; limit
   messages in both contexts; each refusal's remedy executed in a scratch
   fixture; each verdict line.

Existing suites that pin bytes (`tests/project_service.rs`,
`tests/navigation_cli.rs`, `tests/references_cli.rs`, `tests/editing_cli.rs`)
are updated in the same change as the rendering they pin, and the Node
harness, which treats only exit 1 as a syntax rejection, is unaffected
because exit codes do not change.

## Later work

Each item waits for the evidence named with it and is not part of this
delivery:

- **All-user service inventory and sibling-spelling warning.** Needs the
  status reply, or an identity file beside each lock, to carry the exact
  configuration spelling; then `--doctor` can list this user's services and
  warn when two spellings name one canonical file. Evidence: a second
  service observed on a real project.
- **`--files --project TSCONFIG`**, a bounded listing of the searched files
  with a count and a filter, as the complete answer to `OutsideProject`.
- **Uniform repeated-option rule and a text-mode `--pretty` no-op**, each as
  a tested compatibility change once the registry records today's policies.
- **Service flag rename** (`--service-status`, `--service-stop`,
  `--service-serve`) with the old spellings as aliases, or a verb layer.
- **Opt-in diagnostic events** in `<key>.log` beside the socket: service
  start, refresh, request completion with operation, duration, validation
  time, and generation, limit termination, idle exit; and a `timing` field in
  results. Evidence: a service failure that the client's one line could not
  explain.
- **`--check-plan PLAN`**: the plan's replay and current-file identity
  checks without writing; it cannot promise a future write and apply still
  revalidates.
- **Checked-in performance budgets** evaluated by the journeys wrapper,
  advisory first.

## Open decisions for the maintainer

1. **Numeric exit codes.** The plugin's taxonomy separates caller-fixable
   (2) from environment (3) failures and would give apply's rollback and
   recovery outcomes their own code. After step 3 every failure carries a
   category and code, so scripts can branch without a renumbering. A
   migration is a contract break: 17 sites outside `cli::delivery` construct
   exit 2 directly today, 45 tests assert it, and the consumers are the Rust
   tests, the Node tools, Command Center's wrappers, and any shell caller.
   The design centralizes the mapping so the migration is one change in
   `cli::delivery` plus that inventory, and leaves the decision here.
2. **Default caps.** 200 occurrences, 200 outline entries, 50 candidates,
   400 show lines, 400 diff lines per file are starting values; they should
   be calibrated against Command Center's real files before the tests pin
   them.
3. **Where the consumer guide lives and how it is distributed** to agents
   in other repositories, if that is wanted.
