## TypeScript Navigation, Queries, and Editing (`tsparse`)

Use `tsparse` (and load the `tsparse` skill) as your primary workflow for reading, searching, analyzing, and editing TypeScript/TSX code—before reaching for `rg`/`grep` or reading whole files. `tsparse` understands TypeScript 7 syntax, scopes, import/export aliases, and receiver types, and reuses a warm background project service (~150 ms per query). Using it instead of raw file reads and text searches saves time, context tokens, and cost while avoiding false-positive text matches.

- **Skill:** Load the `tsparse` skill whenever navigating unfamiliar TypeScript modules, tracing symbol/member references or types, or performing structured code edits.
- **Help:** Run `tsparse --help` for the operation index, or `tsparse --<op> --help` (e.g., `tsparse --references --help`, `tsparse --edit-batch EDITS --help`) for an operation's flags, examples, and exit codes.
- **Platform and inputs:** `tsparse` builds and runs on macOS and Linux only. Project loading reads only regular files: a source, `tsconfig.json`, or `package.json` that is a pipe, device, or socket is refused with `PATH: not a regular file` (exit 2).

### Operations
- `tsparse [FILE|-]` (`--check`, `--tsx`): Parse a file or stdin into a TypeScript 7 JSON AST, or pass `--check` to validate syntax only.
- `tsparse --outline FILE [--project TSCONFIG]`: List a file's declarations in source order with `LINE:COLUMN`, kind, and one-line signature.
- `tsparse --show (--name NAME | --at [FILE:]LINE:COLUMN) [FILE] [--project TSCONFIG]`: Print one declaration's exact source with original line numbers.
- `tsparse --type --project TSCONFIG (--name NAME | --at [FILE:]LINE:COLUMN) [--expand]`: Inspect the resolved type (and optional structural graph) of a declaration or token.
- `tsparse --references --project TSCONFIG (--name NAME | --at [FILE:]LINE:COLUMN) [--calls] [--no-tests] [--file FILE]`: Group all declarations and uses of a symbol or `Receiver.member` across a project.
- `tsparse --constructs --project TSCONFIG (--name NAME | --at [FILE:]LINE:COLUMN) [--missing KEY] [--all-literals] [--no-tests] [--file FILE]`: List the object literals that construct an interface, class, or type alias, and the properties (optional ones too) each does not certainly provide; by default it examines literals naming one of its keys or none, and `--all-literals` (much slower) examines every literal.
- `tsparse --replace-body (--name NAME | --at [FILE:]LINE:COLUMN) --from FRAGMENT [--plan PLAN]`: Preview replacing the body of a function, method, constructor, accessor, or arrow function.
- `tsparse --insert-after (--name NAME | --at [FILE:]LINE:COLUMN) --from FRAGMENT [--plan PLAN]`: Preview inserting complete statements after a selected declaration's statement.
- `tsparse --edit-batch EDITS [--project TSCONFIG] [--plan PLAN]`: Preview and validate multiple `replace_body` and `insert_after` edits together from a JSON file.
- `tsparse --apply PLAN`: Atomically write a saved `--plan` to disk after verifying every target file's original bytes and identity; replaced files keep only their permission mode (no extended attributes, ACLs, owner, or group).
- `tsparse --references-status --project TSCONFIG`: Report the state, PID, generation, idle time, and limits of the project's background service.
- `tsparse --references-stop --project TSCONFIG`: Stop the project's background service and release its cached snapshot.
- `tsparse --references-serve --project TSCONFIG`: Run the project's background service in the foreground until idle.
- `tsparse --doctor [--project TSCONFIG]`: Check build capabilities, `TSPARSE_*` environment variables, runtime directory permissions, and service health.

### Report Issues, Friction, and Enhancements
Do not work around `tsparse` problems silently. In your final response, surface any bugs, workflow friction, misleading diagnostics or remedies, stale/incorrect results, or missing capabilities/enhancements that forced you to fall back to text search or raw file reads—include the exact command run, its output, and what you needed instead.
