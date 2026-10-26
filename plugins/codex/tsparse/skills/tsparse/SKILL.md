---
name: tsparse
description: Navigate, query, and edit TypeScript and TSX code with the
  `tsparse` CLI. Use when working in a TypeScript project (.ts, .tsx, .mts,
  .cts, tsconfig.json) to list declarations in a file (`--outline`), read a
  declaration's exact source (`--show`), find symbol or member references and
  call sites across a project (`--references`), inspect types (`--type`),
  replace callable bodies or insert statements (`--replace-body`,
  `--insert-after`, `--edit-batch`, `--apply`), or check TypeScript 7 syntax
  (`--check`).
---

# Working on TypeScript Projects with `tsparse`

Use `tsparse` instead of text search or whole-file reads whenever the question or edit targets TypeScript declarations, symbols, members, or types. A text search cannot distinguish a symbol from its spelling, follow import/export aliases, separate shadowed bindings, or resolve receiver types on member accesses; reading an entire file wastes context when you only need its declaration catalog or one function's implementation.

Assume `tsparse` is on `PATH`; it builds and runs on macOS and Linux only. For any flag scope or failure code not covered below, run `tsparse --OP --help` (for example, `tsparse --references --help` or `tsparse --edit-batch EDITS --help`), which prints the operation's exact contract in a few milliseconds without reading files or starting a service.

## Choose the Operation

| Goal | Command | Why `tsparse` instead of text search / raw file reads |
| --- | --- | --- |
| Map what a file declares | `tsparse --outline src/host.ts` | Lists every declaration's `LINE:COLUMN`, kind, and one-line signature indented by containment without reading the full file. |
| Read one declaration's source | `tsparse --show --name RunHost.run src/host.ts` | Prints only that declaration with original line numbers; overloads and shadowed locals stay separate candidates instead of guessing. |
| Find all uses of a symbol | `tsparse --references --project tsconfig.json --name start` | Follows import/export renames and barrel re-exports while keeping shadowed bindings in separate groups. |
| Find uses or callers of a member | `tsparse --references --project tsconfig.json --name RunHost.observe --calls --no-tests` | Matches members by resolved receiver type rather than property spelling, optionally restricted to call sites outside tests. |
| Inspect a declaration or token type | `tsparse --type --project tsconfig.json --at src/host.ts:42:10` | Reports the resolved type summary (and a bounded structural graph with `--expand [--depth 0..32]`). |
| Replace a callable body | `tsparse --replace-body --name RunHost.run src/host.ts --from body.ts --plan run.plan.json` | Replaces only the body, preserves all surrounding bytes, and validates syntax, AST boundaries, `@ts-ignore`/`@ts-expect-error` reach, and JSDoc attachment before writing. |
| Insert statements after a declaration | `tsparse --insert-after --name start src/host.ts --from stmts.ts --plan add.plan.json` | Inserts at the statement seam with matching indentation, line breaks, and automatic semicolon safety. |
| Apply a prepared edit plan | `tsparse --apply run.plan.json` | Verifies every target file's original bytes and inode identity before replacing, rolling back all files if any replacement fails. Replaced files keep only their permission mode. |
| Check if a file or snippet parses | `tsparse --check src/host.ts` | Validates TypeScript 7 syntax in milliseconds and prints a one-line verdict (`ok: ...`). Does not type-check. |
| Diagnose a project query or service failure | `tsparse --doctor --project tsconfig.json` | Checks build capabilities, `TSPARSE_*` variables, runtime directory permissions, config readability, and service state without loading the project. |

**When to use other tools instead:**
- **Text search (`rg` / `grep`):** Searching comments, string literals, markdown/config files, JavaScript files (`.js`/`.jsx` are not analyzed in project mode), files outside the `tsconfig.json` program, or finding an exact identifier spelling when you only know a substring.
- **Standard file-edit tools:** Editing interface/type/enum member lists, changing function parameters or return type annotations, modifying import specifiers in place, or making sub-statement expression edits.

## Select Declarations and Symbols (`--name` and `--at`)

Every selector is exact so a typo or overloaded name never silently targets the wrong code:

- **Exact names (`--name NAME`):** Case-sensitive, decoded identifier match with no fuzzy matching.
  - **Containment (`--name Parent.child`):** In `--show`, `--type`, `--replace-body`, `--insert-after`, and `--references`, dotted names walk declaration containment (e.g., `createRunHost.startEligible` selects `startEligible` inside `createRunHost`).
  - **Imports and exports:** An import or export specifier has a `--name` key only when it renames (`import { a as b }` has key `b`; `import { a }` has no key). Thus `--show --project tsconfig.json --name createThing` selects the original declaration of `createThing`, not every file that imports it.
- **Lexical vs. semantic member references (`--references`):**
  - A **bare name** (`--references --project tsconfig.json --name run`) performs a fast lexical search across declarations, imports, and export aliases, and ends with a hint listing any matching class/interface/object members it did not search (`N catalog members named run were not searched; search SELECTOR with --at FILE:LINE:COLUMN`).
  - A **dotted name** (`--references --project tsconfig.json --name RunHost.observe`) or **`--at` on a member token** performs a semantic member search across resolved receiver types.
- **Positions (`--at`):** Uses 1-based line and 1-based UTF-16 column.
  - Pass either `--at FILE:LINE:COLUMN` (the exact handle printed by `--show`, `--references`, and candidate lists—paste it directly with no `FILE` operand) or `--at LINE:COLUMN FILE` (the `LINE:COLUMN` printed under a file header in `--outline`). Giving a `FILE` operand alongside `--at FILE:LINE:COLUMN` is a usage error.
  - For constructor parameter properties (`constructor(public readonly id: string)`), the identifier `id` selects the lexical parameter; the first modifier (`public`) anchors the semantic class property.
- **Candidate narrowing (`FILE`) vs. occurrence filtering (`--file FILE`):**
  - The positional `FILE` operand narrows which file's candidate declarations `--name` selects.
  - In `--references`, `--file FILE` filters the reported occurrences to `FILE` without narrowing where the symbol is declared. Combine both when needed:
    ```sh
    tsparse --references --project tsconfig.json --name run src/app.ts --file src/api.ts
    ```
- **Ambiguity (`exit 1`):** Overloads, merged interfaces/namespaces, and same-named locals in separate blocks are distinct syntax sites. When `--name` matches more than one, `tsparse` exits `1` and lists every candidate's `FILE:LINE:COLUMN` and qualified selector. Re-run with the desired candidate's `--at FILE:LINE:COLUMN` or narrower `--name`.

## Work with the Project Service and Cost Model

Project queries (`--references`, `--type`, and `--outline`/`--show`/edits with `--project`) require an explicit `--project TSCONFIG`. `tsparse` never searches ancestor directories for `tsconfig.json`, and it rejects configs with project `references` rather than searching partially (pass the specific subproject's `tsconfig.json` instead).

`--project` queries automatically start and reuse a per-project background service (keyed by the exact `--project` path spelling and build, expiring after 10 idle minutes). Before each request, the service verifies that no captured source, config, or package manifest changed, rebuilding the snapshot when any input changed.

Typical costs on a ~4,000-file TypeScript project:
- **Single-file `--outline`, `--show`, `--check`, or edit without `--project`:** 5–20 ms (no project loaded).
- **Warm project query (`--references` lexical, `--type`, `--outline`, `--show`):** 120–160 ms.
- **Cold start (first project query):** ~6 s (~1.6 GB service RSS).
- **First project query after any file edit:** ~4.5 s (snapshot rebuild).
- **Warm semantic member search (`--references --name Receiver.member`):** ~9–10 s.

Follow these sequencing rules to keep queries fast:
1. **Omit `--project` when one file suffices:** Use `tsparse --outline FILE` and `tsparse --show ... FILE` without `--project` when you already know the file path.
2. **Batch project reads before editing:** Complete your `--references`, `--type`, and project-wide `--show` queries before modifying files so every query reuses the warm snapshot instead of triggering a rebuild.
3. **Parallelize independent warm queries:** Once the service is running, issue independent read queries concurrently; if the service is cold, let the first query start it so concurrent callers do not wait on the startup lock.
4. **Keep `--direct` for debugging only:** `--direct` (as in `--references --direct`) loads the entire project in the CLI process (~6 s and ~2 GiB RSS every run). Use it only to rule out the service when diagnosing an issue.

## Edit Declarations (`--replace-body`, `--insert-after`, `--edit-batch`, `--apply`)

`tsparse` edits operate in two steps: **prepare** (which validates syntax, AST boundaries, `@ts-ignore`/`@ts-expect-error` governance, and JSDoc attachment, prints a unified diff, and optionally saves a `--plan`) and **apply** (which atomically replaces the target files if their bytes and inode identities are unchanged).

### Supported Sites and Fragment Rules
- **`--replace-body`:** Replaces the complete body of a function, method, constructor, accessor, arrow function, or a variable/property initialized directly with a function or arrow.
  - The fragment (`--from FRAGMENT` or batch `"fragment"`) must be the complete new `{ ... }` block, or an expression for an arrow function.
  - An arrow function returning an object literal must wrap it in parentheses—`({ ok: true })`—because `{ ok: true }` parses as a block with a labeled statement.
  - Interfaces, type aliases, enums, classes, and overload signatures have no callable body (`UnsupportedSite`); to edit a class method, select the method (`--name MyClass.myMethod`), not the class.
- **`--insert-after`:** Inserts one or more complete statements after the statement designated by the selected declaration (a variable binding designates its whole variable statement; an import/export specifier designates its whole import/export declaration). The target statement must live in a file, block, namespace body, or `case`/`default` clause.

### Two-Step Workflow

1. **Prepare and save a plan (`--plan PLAN`):**
   - For a single edit, write the replacement body or statements to a temporary file (or pass `--from -` for stdin when `--limit` is not used) and save a plan:
     ```sh
     tsparse --replace-body --project tsconfig.json --name RunHost.start \
       --from /tmp/new-body.ts --plan /tmp/start.plan.json
     ```
   - For multiple edits across one or more files, use `--edit-batch EDITS` so all selections happen against the original files and are validated together:
     ```json
     {
       "edits": [
         {
           "operation": "replace_body",
           "name": "RunHost.start",
           "file": "src/host.ts",
           "fragment": "{\n  return this.launch();\n}"
         },
         {
           "operation": "insert_after",
           "at": "src/host.ts:1:1",
           "fragment": "import { Metrics } from \"./metrics\";"
         }
       ]
     }
     ```
     ```sh
     tsparse --edit-batch /tmp/edits.json --project tsconfig.json --plan /tmp/batch.plan.json
     ```
   - **New-file invariant:** `--plan PLAN` and `-o OUTPUT` on edit commands **must name paths that do not exist yet** (`usage.output` if the file already exists). Delete old plan files or pick a fresh path before re-running.

2. **Apply the plan (`--apply PLAN`):**
   ```sh
   tsparse --apply /tmp/batch.plan.json
   ```
   - Do not modify target files between `--plan` and `--apply`; if any file changed, `--apply` exits `1` (`effect: unchanged`) and you must prepare the plan again.
   - Each file is replaced by a new file that keeps only the original's permission mode: extended attributes, ACLs, owner, and group are not carried over. Files with another hard link or macOS flags (`chflags`) are refused.
   - `tsparse` does **not** type-check edited code. After applying edits, run the project's type checker (e.g., `tsc --noEmit`) or relevant tests to verify types and behavior.

## Read Outcomes, Display Caps, and Failures

### Exit Codes
- **`0` (Success):** Complete query result, valid syntax (`--check`), edit preview/plan saved, or plan applied (`applied` or `no_change`).
- **`1` (No complete answer):** Ambiguous or missing selection, partial semantic coverage (`gaps` or `issues` present), syntax/analysis error, rejected edit (`Boundary`, `Attachment`, `Conflict`, `UnsupportedSite`), stale plan (`unchanged`), or `--doctor` finding.
  - **Always read stdout/stderr on exit `1` before retrying:** an ambiguous selection lists the exact `--at FILE:LINE:COLUMN` candidates to pick from, and a partial reference or type query still delivers the occurrences/graph it resolved alongside the specific gaps.
- **`2` (Usage, I/O, or transport failure):** Begins with a machine-greppable `CODE: message` on stderr (or in `error.code` with `--json`), followed by per-field `issues`, `why:`, and `next:` remedies.

### Display Caps (`--limit` and `-o`)
To protect context windows, stdout responses are capped by default (both in text and `--json`):
- `--references`: `200` occurrences across groups
- `--outline`: `200` entries (after `--depth` filtering)
- `--show`: `400` declaration lines
- Edit previews: `400` diff lines per changed file
- Ambiguity diagnostics: `50` candidates

When output is truncated, the last line of text (or the `display` object in `--json`) states how many items were withheld and gives the exact shell-safe command to view all items (`--limit all` or `-o FILE`, where `-o` without `--limit` writes the uncapped output to a file).

### Troubleshooting and Designed Constraints
- **File outside project (`OutsideProject`, exit `2`):** The queried file is in `node_modules`, a declaration library, or excluded by `tsconfig.json`'s `files`/`include`/`exclude`. Use single-file `--outline FILE` or `--show ... FILE` (without `--project`) to inspect files outside the project's searched set.
- **Query memory or timeout limits (`exit 1`):** Read-only queries default to `TSPARSE_MAX_MEMORY_MB=6144` and `TSPARSE_QUERY_TIMEOUT_MS=60000`. A limit failure terminates that service process and its cache. If you change `TSPARSE_*` variables while a service is already running, run `tsparse --references-stop --project tsconfig.json` first so the next request starts a service with the new environment.
- **Input that is not a regular file (`Io`, exit `2`):** Project loading (`--project`) refuses a source, `tsconfig.json`, or `package.json` that is a pipe, device, or socket with `PATH: not a regular file` and never reads it; the service reloads and reports the same once an input becomes one. Replace it with a regular file. Single-file parse and navigation without `--project` do not make this check.
- **Service transport errors (`transport.*`, exit `2`):** `tsparse` never falls back silently from a broken service to `--direct`. Run the `tsparse --doctor --project CONFIG` command printed at the end of the error, and follow its `next:` remedy (such as `chmod 700` on the runtime directory or setting a shorter `TSPARSE_SERVICE_DIR` if the Unix socket path exceeds ~100 bytes).
- **Surface friction and enhancements:** Do not work around `tsparse` defects or gaps silently. In your final response, report any bugs, misleading messages/remedies, stale results after edits, silent truncation, or missing operations/enhancements that forced you to fall back to text search or raw file reads, along with the command run, its output, and what you expected.
