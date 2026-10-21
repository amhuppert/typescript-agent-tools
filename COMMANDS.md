# `tsparse` CLI Commands Reference

`tsparse` parses TypeScript 7 and TSX into a compiler-style JSON AST, and navigates, queries, and edits declarations across files and `tsconfig.json` projects.

> For full flag rules, failure codes, and exit tables per operation, run `tsparse --OP --help` or see [docs/cli-reference.md](docs/cli-reference.md).

---

## Quick Index

| Category | Flag / Command | What it does |
| --- | --- | --- |
| **Parse & Validate** | *(no operation flag)* | Parse a TypeScript/TSX file or stdin into a TypeScript 7 JSON AST. |
| | `--check` | Validate syntax only and print a one-line verdict instead of the AST. |
| **Navigate** | `--outline` | List a file's declarations in source order, indented by containment. |
| | `--show` | Print one declaration's exact source with original line numbers. |
| **Project Queries** | `--type` | Inspect the type (and optional structural graph) of a declaration or token. |
| | `--references` | Group declarations and occurrences of a symbol or member across a project. |
| **Edit Code** | `--replace-body` | Preview replacing the body of a function, method, constructor, accessor, or arrow. |
| | `--insert-after` | Preview inserting statements after a selected declaration's statement. |
| | `--edit-batch EDITS` | Preview multiple edits from a JSON batch file against the original files. |
| | `--apply PLAN` | Write a saved edit plan's exact bytes after verifying file contents and identity. |
| **Service & Health** | `--references-status` | Report the state of the project's background service without loading or refreshing. |
| | `--references-stop` | Stop the project's background service and release its cached snapshot. |
| | `--references-serve` | Run the project's service in the foreground until idle. |
| | `--doctor` | Check build capabilities, environment variables, runtime directory, and service state. |
| **Meta** | `-h, --help` | Print the operation index, or `tsparse --OP --help` for one operation (`--json` for JSON). |
| | `-V, --version` | Print the version and build stamp (`tsparse 0.1.0 (build ...)`). |

---

## 1. Parse & Validate

### Parse (`tsparse [FILE|-]`)
Parses one TypeScript file or stdin (`-` or omitted) into a TypeScript 7 compiler-style JSON AST. `.tsx` and `.jsx` filenames enable JSX automatically; pass `--tsx` to enable JSX for stdin or other filenames.

```sh
tsparse [--tsx] [--check] [--pretty] [-o OUTPUT] [FILE|-]
```

- **`--check`**: Validate syntax only; prints `ok: FILE parses as TypeScript 7 (jsx: off|on)` on stdout instead of the AST.
- **`--tsx`**: Enable JSX parsing when the filename does not end in `.tsx` or `.jsx`.
- **`--pretty`**: Pretty-print the JSON AST.
- **`-o, --output OUTPUT`**: Write the AST to `OUTPUT` instead of stdout.

**Examples:**
```sh
tsparse --pretty src/example.ts
tsparse --check src/example.ts
printf 'const x: number = 1;' | tsparse --check
```

---

## 2. Navigate Declarations

Navigation commands work on a single file (no project loaded) or with `--project TSCONFIG` (queries the project's searched files via the background project service, or in-process with `--direct`).

### `--outline`
Lists one file's declarations in source order (`LINE:COLUMN kind signature`), indented by containment. Imports are grouped one line per import declaration.

```sh
tsparse --outline FILE [--project TSCONFIG [--direct]] [--depth N|all] [--limit N|all] [--json [--pretty]] [-o OUTPUT]
```

- **`--depth N|all`**: Nesting levels to display (default `2`; `all` shows every level). Hidden child counts are noted inline.
- **`--limit N|all`**: Cap displayed entries (default `200`; removed when writing to `-o` without `--limit`).

**Examples:**
```sh
tsparse --outline src/host.ts
tsparse --outline src/host.ts --depth all --limit all
tsparse --outline --project tsconfig.json src/host.ts --json -o outline.json
```

### `--show`
Prints a single declaration's exact source with original 1-based line numbers. Overloads, merged declarations, and shadowed locals remain distinct; ambiguous matches exit `1` and list each candidate's `FILE:LINE:COLUMN` and qualified selector.

```sh
tsparse --show (--name NAME [FILE] | --at [FILE:]LINE:COLUMN [FILE]) [--project TSCONFIG [--direct]] [--limit N|all] [--json [--pretty]] [-o OUTPUT]
```

- **`--name NAME`**: Select by exact declared name or dotted containment path (e.g., `RunHost.run`). With `--project`, `FILE` is optional and searches all project files when omitted.
- **`--at [FILE:]LINE:COLUMN`**: Select the declaration whose name or anchor is at the 1-based line and UTF-16 column.
- **`--limit N|all`**: Cap displayed declaration lines (default `400`) and ambiguity candidates (default `50`).

**Examples:**
```sh
tsparse --show --name createRunHost src/host.ts
tsparse --show --project tsconfig.json --name RunHost.run
tsparse --show --at src/host.ts:42:10
```

---

## 3. Semantic Project Queries

Project queries require an explicit `--project TSCONFIG` (no ancestor directory search). On macOS and Linux, they automatically start and reuse a per-project background service that rebuilds when inputs change; pass `--direct` to run in the CLI process instead.

### `--type`
Inspects the type of a selected declaration (`--name`) or token (`--at`) in a project.

```sh
tsparse --type --project TSCONFIG (--name NAME [FILE] | --at [FILE:]LINE:COLUMN [FILE]) [--direct] [--expand [--depth 0..32]] [--limit N|all] [--json [--pretty]] [-o OUTPUT]
```

- **`--expand`**: Include a bounded structural graph of the type.
- **`--depth 0..32`**: Number of type edges to expand when `--expand` is set (default `3`).
- **`--limit N|all`**: Cap ambiguity candidates (default `50`).

**Examples:**
```sh
tsparse --type --project tsconfig.json --at src/host.ts:42:10
tsparse --type --project tsconfig.json --name createRunHost --expand --depth 2 --json
```

### `--references` (alias: `--references-direct`)
Finds and groups all declarations and occurrences of a symbol across a project, following import/export aliases and keeping shadowed bindings in separate groups.

```sh
tsparse --references --project TSCONFIG (--name NAME [FILE] | --at [FILE:]LINE:COLUMN [FILE]) [--direct] [--calls] [--no-tests] [--file FILE] [--limit N|all] [--json] [--pretty] [-o OUTPUT]
```

- **`--name NAME`**: A bare name (e.g., `run`) searches lexical declarations, imports, and export aliases and notes any unsearched members; a dotted name (e.g., `RunHost.observe`) searches a member by resolved receiver type. Optional `FILE` operand narrows candidate declarations to `FILE`.
- **`--at [FILE:]LINE:COLUMN`**: Select the identifier at the given position.
- **`--calls`**: Keep only direct call, `new`, and tagged-template occurrences.
- **`--no-tests`**: Omit occurrences in test files while keeping the selected declarations.
- **`--file FILE`**: Filter occurrences to `FILE` without narrowing candidate declarations.
- **`--limit N|all`**: Cap displayed occurrences across groups (default `200`).
- **`--pretty`**: Alone or with `--json`, emits the indented JSON result document.

**Examples:**
```sh
tsparse --references --project tsconfig.json --name run
tsparse --references --project tsconfig.json --name RunHost.observe --calls --no-tests
tsparse --references --project tsconfig.json --at src/app.ts:12:8 --pretty
```

---

## 4. Edit Declarations

Editing is two-phase: preview commands (`--replace-body`, `--insert-after`, `--edit-batch`) validate syntax, comment directives (`@ts-ignore`/`@ts-expect-error`), and JSDoc attachment, print a unified diff, and never modify source files. Passing `--plan PLAN` saves the prepared edit batch to a new file, which `--apply PLAN` then writes atomically.

### `--replace-body`
Previews replacing the complete body (`{ ... }` block, or expression for an arrow function) of a selected function, method, constructor, accessor, or function-initialized variable/property.

```sh
tsparse --replace-body (--name NAME [FILE] | --at [FILE:]LINE:COLUMN [FILE]) --from FRAGMENT [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [-o OUTPUT]
```

- **`--from FRAGMENT`**: File containing the replacement body (`-` for stdin).
- **`--plan PLAN`**: Save the prepared edit plan to a new file `PLAN` for `--apply`.
- **`--limit N|all`**: Cap ambiguity candidates (default `50`) and diff lines per file (default `400`).

**Examples:**
```sh
tsparse --replace-body --name start src/host.ts --from body.ts
tsparse --replace-body --project tsconfig.json --name RunHost.start --from body.ts --plan start.plan.json
```

### `--insert-after`
Previews inserting complete statements after the statement designated by the selected declaration (in its file, block, namespace, or case clause).

```sh
tsparse --insert-after (--name NAME [FILE] | --at [FILE:]LINE:COLUMN [FILE]) --from FRAGMENT [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [-o OUTPUT]
```

- **`--from FRAGMENT`**: File containing the statements to insert (`-` for stdin).
- **`--plan PLAN`**: Save the prepared edit plan to a new file `PLAN` for `--apply`.

**Examples:**
```sh
tsparse --insert-after --at src/host.ts:3:7 --from helpers.ts
tsparse --insert-after --project tsconfig.json --name projectRunHalts --from helpers.ts --plan insert.plan.json
```

### `--edit-batch`
Previews multiple `replace_body` and `insert_after` edits from a JSON file, selecting and preparing all edits together against the original files.

```sh
tsparse --edit-batch EDITS [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [-o OUTPUT]
```

- **`EDITS` format**: `{"edits": [{"operation": "replace_body"|"insert_after", "name"|"at": "...", "file": "...", "fragment"|"from": "..."}]}`.

**Example:**
```sh
tsparse --edit-batch edits.json --project tsconfig.json --plan batch.plan.json
```

### `--apply`
Writes a saved plan's exact bytes to disk after verifying that every target file still matches its original bytes and identity. Rolls back all modified files if a later replacement fails.

```sh
tsparse --apply PLAN [--json [--pretty]] [-o OUTPUT]
```

**Example:**
```sh
tsparse --apply batch.plan.json
```

---

## 5. Service Administration & Diagnostics

`--references-status`, `--references-stop`, and `--references-serve` manage the background project service that answers all `--project` queries (`--references`, `--type`, `--outline`, `--show`, and project edits).

### `--references-status`
Reports whether the project's service is running, its PID, snapshot generation, build stamp, remaining idle time, and active memory/timeout limits. Does not load the project, check freshness, or reset the idle timer.

```sh
tsparse --references-status --project TSCONFIG [--json] [--pretty] [-o OUTPUT]
```

### `--references-stop`
Stops the project's background service and frees its cached snapshot (`stopped: pid N` or `no service: TSCONFIG`). Stop the service after changing `TSPARSE_MAX_MEMORY_MB` or `TSPARSE_QUERY_TIMEOUT_MS` so the next query starts with the new limits.

```sh
tsparse --references-stop --project TSCONFIG
```

### `--references-serve`
Runs the project's service in the foreground (for process supervisors) until its idle timeout expires.

```sh
tsparse --references-serve --project TSCONFIG
```

### `--doctor`
Runs read-only health checks on what project queries depend on (loads nothing, starts nothing). Without `--project`, checks build capabilities, `TSPARSE_*` environment variables, and the runtime directory (`0700` permissions and socket path length). With `--project`, also checks `tsconfig.json` readability, service status, and whether a running service's inherited limits differ from the current environment.

```sh
tsparse --doctor [--project TSCONFIG] [--json [--pretty]]
```

---

## Shared Flags, Environment Variables & Exit Codes

### Shared Output & Meta Flags

| Flag | Description |
| --- | --- |
| `--json` | Emit one structured JSON result or error document on stdout or `-o` (stderr remains empty). |
| `--pretty` | Indent JSON output. Valid alone on `parse`, `--references`, and `--references-status`; requires `--json` elsewhere. |
| `-o, --output FILE` | Write output to `FILE` instead of stdout. Removes default display `--limit` caps when `--limit` is omitted. |
| `--limit N\|all` | Override default display caps (`200` occurrences/outline entries, `400` show/diff lines, `50` candidates). |
| `-h, --help` | Print general help, or combine with an operation flag (`tsparse --references --help`) for operation help. |
| `-V, --version` | Print `tsparse` version and build stamp. |

### Environment Variables

| Variable | Default | Description |
| --- | --- | --- |
| `TSPARSE_MAX_MEMORY_MB` | `6144` | Positive integer MiB memory ceiling for read-only queries (macOS/Linux). |
| `TSPARSE_QUERY_TIMEOUT_MS` | `60000` | Positive integer millisecond deadline for read-only queries. |
| `TSPARSE_SERVICE_IDLE_SECS` | `600` | Positive integer idle timeout in seconds before a background service exits. |
| `TSPARSE_SERVICE_DIR` | `/tmp/tsparse-<UID>` | Private runtime directory (must be owned by current user with mode `0700`). |

### Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | **Success:** Complete answer, valid syntax (`--check`), edit preview generated, or plan applied. |
| `1` | **No complete answer:** Syntax/analysis error, ambiguous or missing selection, partial result, rejected edit, stale plan, or doctor finding. |
| `2` | **Usage / I/O / transport error:** Invalid CLI flags, unreadable file, position outside file, file outside project, service unreachable, or failed write/rollback. |
