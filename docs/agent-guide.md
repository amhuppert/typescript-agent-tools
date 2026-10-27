# Agent guide

How an agent should use `tsparse` instead of reading and searching text. This
guide reaches agents that work in this repository only: whether and how it
reaches agents in other repositories is an [open
decision](agent-cli-design.md#open-decisions-for-the-maintainer). The [CLI
reference](cli-reference.md) has every operation's flags, examples, and exit
codes; `tsparse --OP --help` prints one operation's part of it.

## Choose the operation

When the question is about TypeScript declarations, symbols, or types, use an
operation; a text match cannot tell a symbol from its spelling.

| When you need | Use | Instead of text search because |
| --- | --- | --- |
| What a file declares, and where | `--outline` | it lists each declaration with its `LINE:COLUMN` and signature, indented by containment, so you need not read the file |
| A test file's suites, tests, and hooks | `--test-outline` | it lists every `describe`, `it`, and `test` call and hook with its `LINE:COLUMN` and the test's name, at every level; each position selects that test's statement for `--show --at` and `--insert-after --at` |
| One declaration's source | `--show` | it prints exactly that declaration with its original line numbers; overloads and same-named locals are separate candidates, never a guess |
| Every use of a symbol | `--references` | it follows import and export aliases and keeps shadowed bindings and same-named symbols apart |
| Every use or call of a member | `--references` with a dotted `--name`, and `--calls` for calls | member identity follows resolved receiver types, not spelling |
| A type | `--type` | it reports the type of the declaration or token, and a bounded graph with `--expand` |
| To replace a body or insert statements | `--replace-body`, `--insert-after`, or `--edit-batch`, then `--apply` | the preview is the exact diff; the rest of each file stays byte for byte, the result must parse and keep comment directives and JSDoc governing what they did, and no project file is written until you apply (`--plan` and `-o` only create new files); it is not type-checked |
| Whether a file parses | `--check` | it parses as TypeScript 7 does; it does not type-check, but rejects a namespace declared with `module` (TS1540) as TypeScript 7 does |

Keep text search for:

- Comments, string contents, configuration, and documentation.
- Files the project does not search, and JavaScript beyond plain ES modules
  and one `module.exports = name`, which a project refuses where it is
  written (other CommonJS, `require`, JSDoc beyond a variable's `@type`).
- A name you know only in part: every name is exact.

## Start a task

| Task | First command |
| --- | --- |
| Learn what a file declares | `tsparse --outline src/host.ts` |
| Learn a test file's tests, or where to add one | `tsparse --test-outline src/host.test.ts` |
| Read one declaration | `tsparse --show --project tsconfig.json --name RunHost.run` |
| Find every use of a function, variable, class, or import | `tsparse --references --project tsconfig.json --name start` |
| Find a method's calls outside tests | `tsparse --references --project tsconfig.json --name RunHost.observe --calls --no-tests` |
| Learn a type | `tsparse --type --project tsconfig.json --at src/host.ts:8:17` |
| Replace a body | `tsparse --replace-body --project tsconfig.json --name RunHost.run --from body.ts --plan run.plan.json` |
| Insert statements after a declaration | `tsparse --insert-after --project tsconfig.json --name start --from helpers.ts --plan start.plan.json` |
| Make several edits together | `tsparse --edit-batch edits.json --project tsconfig.json --plan change.plan.json` |
| Write a previewed edit | `tsparse --apply run.plan.json` |
| Check syntax | `tsparse --check src/host.ts` |
| A project query fails to start or answer | `tsparse --doctor --project tsconfig.json` |
| Learn which compiler's rules read a project (TypeScript 5.x or 7) | `tsparse --doctor --project tsconfig.json` (its `compatibility` line) |

Then:

- Select by position with `--at FILE:LINE:COLUMN`, as `--show`, reference
  occurrences, and candidate lists print it, or with `--at LINE:COLUMN FILE`,
  as an outline or a test view prints `LINE:COLUMN` under its file.
- `--name` is exact. A dotted `--name RunHost.run` selects `run` inside
  `RunHost`.
- With `--references`, a bare name searches lexical declarations, local
  imports, and exported aliases, not members: one line of its answer names
  the members it did not search and the `--at` that searches one.
- An ambiguous selection exits 1 and lists each candidate's
  `FILE:LINE:COLUMN`; pass the one you mean to `--at`.
- `--from` names a file holding the complete new body (a block with its
  braces, or an arrow function's expression) or the statements to insert.
- A long answer is cut for display. Cut text ends with a line that counts
  what was withheld and names the command that shows it; a cut JSON document
  has no such line but a `display` entry with the counts, `"truncated": true`,
  and `next.argv`. Run that command as printed: it repeats the request
  without the cut. `-o FILE` without `--limit` writes the complete result, but an
  explicit `--limit N` applies to `-o` as well; `--limit all` removes it.

## Read the outcome

- Branch on the exit code: the [CLI reference](cli-reference.md#exit-codes)
  gives what 0, 1, and 2 mean, and each operation's section what its codes
  report.
- Exit 1 means no complete answer: a partial result with its gaps, an
  ambiguous selection with its candidates, a rejected edit, or an analysis
  failure. Read what it delivered before you retry.
- A failure that leaves no result starts with its code, `CODE: message`,
  which the [failure codes](cli-reference.md#failure-codes) list; branch on
  the code, not the prose.
- With `--json`, an outcome is one document on stdout or `-o`, and stderr
  stays empty; the failure codes name the few failures that stay text.

## Cost model

Measured on Command Center, a Next.js application whose configuration
searches 4,160 files, with this repository at `2a59e7c` (release build,
2026-10-17, on a shared host at a load average of about 4;
[single-thread work](performance-single-thread-opportunities.md#implementation-status),
which records this repository's revision but not Command Center's):

| Request | Time | Memory |
| --- | ---: | ---: |
| Warm project query: references, type, or outline | 124–155 ms, about 110 ms of it checking that no input changed | |
| First project query, which starts the service | 5.9–6.4 s | service holds 1,563–1,591 MB |
| First project query after a one-file edit | 4.2–4.5 s | service holds 1,310–1,435 MB |
| Warm member search (`--name AgentTaskRunner.run`) | 9.5–9.6 s | |
| `--direct` project load, every time | 6.05–6.3 s | peak 1.94–1.95 GiB |

Commands that load no project take milliseconds: `--check` of a small file
5 ms, an outline of the largest file 20 ms, and `--references-status` 4.5–4.6 ms
(2026-10-17: the release build of the performance work's final working tree,
binary `53cc1fe9d6fc2e0b…`, measured beside baseline `76e6b61`, whose
file-only times were the same;
[Command Center performance](performance-command-center.md#current-state-2026-10-17-after-the-performance-work)).
A service exits after 10 idle minutes, the default of
`TSPARSE_SERVICE_IDLE_SECS` ([references-serve](cli-reference.md#references-serve)),
and the next project query starts it again.

So:

- Leave project queries on the service, the default. Add `--direct` only to
  rule the service out: every direct command loads the project.
- Ask related questions in a row: warm queries are cheap.
- Group reads before edits: the first project query after any file change
  reloads the project.
- Expect a member search to take seconds.
- When one file answers the question, use `--outline FILE` or
  `--show … FILE` without `--project`: no project loads.
- `--references-status` and `--doctor` load nothing and start nothing.

## Friction

### Designed friction: absorb it

These constraints are the design. Work with them; do not report them as
defects.

| Friction | Why | Do instead |
| --- | --- | --- |
| Exact names | `--name` selects declarations spelled exactly NAME, never a near match, so a typo cannot select another symbol. | Find the spelling with `--outline` or text search. |
| No partial batches | A batch is selected in the original files and validated together; it is prepared completely or rejected, and no subset is offered. | Fix what the failure names (an invalid batch lists every issue) and run the whole batch again. |
| No ancestor `tsconfig.json` search | `--project` names the configuration, so an answer is never about a project you did not name. | Pass the configuration's path. |
| No fallback from the service to a direct load | A transport failure exits 2 and never becomes a direct load, so a broken service is reported rather than hidden. | Run the `--doctor` command the failure prints, or add `--direct` yourself. |
| Project references rejected | A configuration with non-empty `references` is rejected rather than searched partially. | Pass the configuration of the referenced project you mean. |
| Exit 1 on partial results | A partial answer must not pass for a complete one; it is delivered with its gaps or issues. | Use what it covers, and read the gaps. |
| New-file-only outputs | An editing command writes `--plan` and `--output` only as new files: no outcome may replace a file the command read or edits. | Choose another path. |
| One writer during apply | `--apply` checks every file's bytes and identity, then replaces them; another writer makes the plan stale (exit 1, nothing changed) or can leave files to recover (exit 2). | Keep other writers away while applying; prepare a stale plan again. |

### Incidental friction: report it

These are defects. Report each in your result with the command, its output,
and what you expected; do not work around it silently.

- **Silent truncation:** output that withholds items without saying so: text
  without a last line that counts them and names the command that shows them,
  or a JSON document without a `display` entry that counts them and gives
  `next.argv`.
- **A message that misleads:** a failure, remedy, or printed command that
  points the wrong way, or a printed command that fails when run as printed.
- **A missing verb:** a question about declarations, symbols, or types that
  you had to answer by text search or by reading files because no operation
  asks it.
- **A read that answers from the wrong state:** an answer that does not
  match the files as they are now, such as a project query after an edit that
  still shows the old text.
