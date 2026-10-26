# tsparse command-line reference

Generated from `tsparse --help --json` by
`cargo test --test cli_registry -- --ignored write_reference`; change the
registry in `src/cli/registry.rs`, not this file. `tsparse --help` prints
the index of these operations and `tsparse --OP --help` one operation's
section.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success: a complete result, a preview, or a plan applied or changing nothing. |
| 1 | No complete answer: a syntax error, an analysis failure, an ambiguous or missing selection, a partial result, a rejected edit, or a plan that no longer applies to the files. |
| 2 | A usage error, an input that cannot be read, an invalid position or a file outside the project, a service that cannot be started or reached, a failed write, or a rollback or recovery while applying. |

Each operation's section says what its codes report.

## Failure codes

A failure that leaves no result begins its stderr diagnostic with its code,
`CODE: message`, followed by `  path: message` for each issue, `  why: …`,
and `  next: …`, a command to run or a remedy to apply. With `--json` it is
one document,
`{"outcome": "error", "error": {"category", "code", "message", …}}`, whose
`error` has the same parts: `issues`, each `{"path", "message"}` with a JSON
path whose indexes are zero-based (`edits[1].extra`), `why`, and either
`next`, a command as `{"argv": […]}`, or `remedy`, a sentence. It has
`operation` when the request names one and `effect` for editing: a usage
failure's on stdout, never `-o`, and any other's where the result would go.
Three are always text: a result that cannot be written (`write.output`),
which is one diagnostic and never a second document; a usage failure about
`--json` itself, given to an operation that does not take it; and an
argument that is not Unicode. Every failure exits 2. Analysis
failures, ambiguous selections, and rejected edits are results with their
own documents, described in each operation's document.

| Code | Category | Meaning |
| --- | --- | --- |
| `usage.not_unicode` | usage | An argument is not Unicode; requests name files exactly. Always text. |
| `usage.unknown_option` | usage | An argument written as an option names no flag. |
| `usage.missing_value` | usage | A flag that takes a value is the last argument. |
| `usage.invalid_value` | usage | A flag's value is not in the form it takes, such as --at or --depth. |
| `usage.operation` | usage | The flags name operations that cannot run together. |
| `usage.flag_scope` | usage | The operation does not take a flag, or takes it only beside another. |
| `usage.repeated_flag` | usage | A flag the operation takes once is given again. |
| `usage.missing_flag` | usage | A flag the operation requires is absent, such as --project or --from. |
| `usage.selection` | usage | The request does not select exactly once: both or neither of --at and --name, or a selection where a plan or batch names its own. |
| `usage.operands` | usage | The FILE operands do not fit the operation: more than one, stdin where it reads files by name, none where it needs one, or one beside --at FILE:LINE:COLUMN. |
| `usage.output` | usage | An editing command cannot write its result or plan at the path given: it exists, names the other output or a file the command reads or edits, or cannot be inspected. |
| `usage.environment` | usage | A TSPARSE_* variable holds a value it does not accept. |
| `input.read` | input | An input the request names cannot be read: the file to parse, stdin, a fragment, or a batch file. |
| `input.batch` | input | A batch file is not JSON, or not a valid edit batch: each issue is named by its JSON path, and next prints the batch format. |
| `transport.directory` | transport | The service's runtime directory cannot be created or inspected, or is not a private directory owned by this user with mode 0700; next or remedy says how to fix it. |
| `transport.socket` | transport | The service's socket cannot be bound, connected, or used: its path is too long for a socket address or names another kind of file, or the connection failed or closed before an answer. |
| `transport.lock` | transport | A lock file in the service's runtime directory cannot be opened or locked. |
| `transport.startup` | transport | A service could not be started: it could not be spawned or set up, it exited without recording why, or the diagnostic it recorded cannot be read or removed. |
| `transport.timeout` | transport | The service did not become ready, take a request, answer, or stop in time. |
| `transport.protocol` | transport | A request or answer the other side cannot use: a request over 1 MiB, a request the service could not read, or an answer that cannot be decoded or does not answer the request. |
| `transport.identity` | transport | The service's identity, its configuration's absolute path and the build, cannot be established, or the service at its socket serves another. |
| `write.output` | write | The result could not be written to stdout or the output file, so neither can its document be: always text. |
| `write.plan` | write | The plan could not be saved. |
| `internal.watchdog` | internal | The query's time and memory watchdog could not run. |

## Operations

| Operation | Flag | Summary |
| --- | --- | --- |
| [parse](#parse) | none | With no operation flag: parse one TypeScript file, or stdin, into a compiler-style JSON AST, or with --check only validate it. |
| [outline](#outline) | `--outline` | List the declarations of one file in source order, one per line, indented by containment. |
| [show](#show) | `--show` | Print one declaration's exact source with its original line numbers. |
| [type](#type) | `--type` | Inspect the type of the selected declaration or token in one explicit project. |
| [references](#references) | `--references` | Group the declarations and occurrences of the selected symbol in one explicit project. |
| [replace-body](#replace-body) | `--replace-body` | Preview replacing the complete body of the selected function, method, constructor, accessor, or arrow function. |
| [insert-after](#insert-after) | `--insert-after` | Preview inserting complete statements after the statement the selected declaration designates. |
| [edit-batch](#edit-batch) | `--edit-batch` | Preview several edits from a JSON file, all selected in the original files. |
| [apply](#apply) | `--apply` | Write a saved plan's exact bytes, after checking that every file still has its original bytes and identity. |
| [references-status](#references-status) | `--references-status` | Report the state of the project's service; it loads nothing, does not check freshness, and does not extend the service's idle time. |
| [references-stop](#references-stop) | `--references-stop` | Stop the project's service and release its cache. |
| [references-serve](#references-serve) | `--references-serve` | Run the project's service in the foreground, for a supervisor, until it is idle. |
| [doctor](#doctor) | `--doctor` | Check what project queries depend on, one line per check; it loads nothing and starts nothing. |

The output flags `--json`, `--pretty`, `--output` apply where an operation lists them; `--help` and `--version` apply to every operation.

## parse

With no operation flag: parse one TypeScript file, or stdin, into a compiler-style JSON AST, or with --check only validate it.

```text
tsparse [--tsx] [--check] [--pretty] [--output OUTPUT] [FILE|-]
```

Reads stdin when FILE is omitted or is -. A .tsx or .jsx file name, in any case, enables JSX, as --tsx does for stdin. The AST has TypeScript's node kinds and positions; a syntax error is FILE:LINE:COLUMN: message on stderr, with a one-based line and UTF-16 column. --check prints no AST but one line on stdout, ok: FILE parses as TypeScript 7 (jsx: off), or jsx: on, with - for stdin; it checks syntax only, and --pretty and --output change nothing then.

| Flag | Summary |
| --- | --- |
| `--tsx` | Parse JSX, as a .tsx or .jsx file name already does. |
| `--check` | Validate syntax, printing one verdict line instead of the AST. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag given more than once takes its last value. --pretty is valid without --json.

Examples:

- `tsparse --pretty example.ts`: Print the indented AST of example.ts.
- `tsparse --check --tsx`: Validate stdin as TSX, printing ok: - parses as TypeScript 7 (jsx: on) instead of the AST.
- `tsparse -o view.ast.json view.tsx`: Write the AST to a file; the .tsx name enables JSX.

| Exit | Reports |
| --- | --- |
| 0 | The AST was written, or with --check the input parses and its verdict line was printed. |
| 1 | A syntax error, as FILE:LINE:COLUMN: message on stderr (- for stdin). |
| 2 | A usage error, an input that cannot be read as UTF-8, or a result that cannot be written. |

Related: [outline](#outline), [show](#show). Document: [docs/ast-json.md](ast-json.md).

## outline

List the declarations of one file in source order, one per line, indented by containment.

```text
tsparse --outline FILE [--depth N|all] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
tsparse --outline --project TSCONFIG FILE [--direct] [--depth N|all] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

Each line is LINE:COLUMN, kind, and signature or label; an import declaration takes one line, at its first name, listing the names it imports. --depth shows N levels (default 2; all shows every level) and counts what it hides. Without --project only FILE is read. With --project, FILE must be one of the files --references searches, the project's service answers (this process with --direct), and a project that fails to load answers nothing, never falling back to FILE alone. Text names files absolutely; project JSON names them relative to the configuration's directory.

| Flag | Summary |
| --- | --- |
| `--outline` | List one file's declarations, indented by containment. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--depth DEPTH` | Outline levels to show (N or all, default 2), or type edges to expand (0 through 32, default 3). |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json. Display limits: at most 200 entries of the levels --depth shows; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --outline src/app.ts`: Two levels of declarations, counting what is hidden.
- `tsparse --outline --project tsconfig.json src/app.ts --depth all --json --output outline.json`: Every level of a project file, as one document in a file.
- `tsparse --outline src/app.ts --limit all`: Every entry of the two levels, however many; the default shows 200.

| Exit | Reports |
| --- | --- |
| 0 | The outline, which is empty for a file without declarations. |
| 1 | A syntax error, unsupported input, or a project analysis failure. |
| 2 | A usage error, a file that cannot be read, an invalid position, a file outside the project, a service that cannot be started or reached, or a result that cannot be written. |

Related: [show](#show), [references](#references). Document: [docs/navigation.md](navigation.md).

## show

Print one declaration's exact source with its original line numbers.

```text
tsparse --show (--name NAME FILE | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) [--limit N|all] [--json [--pretty]] [--output OUTPUT]
tsparse --show --project TSCONFIG (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) [--direct] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

--name selects an exact declared name, and a.b.c selects c inside declarations named b and then a; an import or export specifier has a name only where it renames, and a default import never does. --at selects the declaration whose name or anchor is at the position. Overloads, merged declarations, and same-named locals are separate declarations: an ambiguous or missing selection exits 1 and lists each candidate's FILE:LINE:COLUMN and selector, on stderr or, with --json, in the document. With --project, FILE must be one of the files --references searches, --name without FILE searches all of them, and a printed FILE:LINE:COLUMN selects its declaration from any directory. Text names files absolutely; project JSON names them relative to the configuration's directory.

| Flag | Summary |
| --- | --- |
| `--show` | Print one declaration's exact source with its line numbers. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json. Display limits: at most 400 lines of the declaration from its first line and 50 candidates of an ambiguous selection; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --show --name run src/host.ts`: A bare name: an ambiguous one lists its candidates.
- `tsparse --show --project tsconfig.json --name RunHost.run`: A dotted name: run inside RunHost, in any searched file.
- `tsparse --show --project tsconfig.json --at src/host.ts:12:3`: The FILE:LINE:COLUMN form results print.
- `tsparse --show --at 12:3 src/host.ts`: The same position, with the file as the operand.

| Exit | Reports |
| --- | --- |
| 0 | One declaration found. |
| 1 | An ambiguous or missing selection, a syntax error, unsupported input, or a project analysis failure. |
| 2 | A usage error, a file that cannot be read, an invalid position, a file outside the project, a service that cannot be started or reached, or a result that cannot be written. |

Related: [outline](#outline), [type](#type), [replace-body](#replace-body). Document: [docs/navigation.md](navigation.md).

## type

Inspect the type of the selected declaration or token in one explicit project.

```text
tsparse --type --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--expand [--depth 0..32]] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

--name takes --show's declaration containment selectors, and FILE narrows the seeds. --expand adds a bounded structural graph, and --depth counts its type edges (default 3, 0 through 32). Semantic dependencies that cannot be established are located failures and partial expansion is explicit; this is not full TypeScript checking. A query runs under TSPARSE_MAX_MEMORY_MB (default 6144 MiB) and TSPARSE_QUERY_TIMEOUT_MS (default 60000), positive integers checked before any service starts; a service keeps the values it started with, and a limit failure exits the process that hit it.

| Flag | Summary |
| --- | --- |
| `--type` | Inspect the type of a declaration or token in one project. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--expand` | Add a bounded structural graph of the type. |
| `--depth DEPTH` | Outline levels to show (N or all, default 2), or type edges to expand (0 through 32, default 3). |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json. Display limits: at most 50 candidates of an ambiguous selection; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --type --project tsconfig.json --at src/host.ts:42:10`: The type at a position the results print.
- `tsparse --type --project tsconfig.json --name createRunHost --expand --depth 2 --json --direct`: Two type edges of structure, as a document, loaded in this process.

| Exit | Reports |
| --- | --- |
| 0 | A complete answer. |
| 1 | A partial answer, an ambiguous or missing selection, an unsupported or unresolved target, an analysis failure, or a memory, time, depth, or output limit. |
| 2 | A usage error, a file that cannot be read or another I/O failure, an invalid position, a file outside the project, an internal failure, a service that cannot be started or reached, or a result that cannot be written. |

Related: [show](#show), [references](#references). Document: [docs/types.md](types.md).

## references

Group the declarations and occurrences of the selected symbol in one explicit project.

```text
tsparse --references --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--calls] [--no-tests] [--file FILE] [--limit N|all] [--json] [--pretty] [--output OUTPUT]
```

--at selects the identifier at the position (one-based line, UTF-16 column). A bare NAME selects every lexical declaration, local import, or exported alias spelled exactly NAME and lists the matching members it did not search; a dotted NAME such as RunHost.observe selects a member, whose identity follows resolved receiver types, not spelling. FILE narrows the candidates; --file FILE then keeps only their occurrences in FILE. Import and export aliases join their underlying symbol; shadowed bindings form separate groups; an incomplete search reports partial coverage and its gaps. Text prints a header with the selection, its counts, and the filters that bound them, each group with its declarations and its occurrences by file (a renamed alias's spelling in parentheses), each gap, and one line counting the members a bare name did not search and naming the --at that searches one; files are named absolutely, and an ambiguous or missing selection is a diagnostic on stderr. --json writes the result document instead, which also delivers analysis failures as documents and names files relative to the configuration's directory; --pretty without --json writes that document indented. Supported: one configuration and its inherited settings; .ts, .tsx, .mts, and .cts sources and declarations; node16, nodenext, and bundler resolution; installed packages with declarations; and JSON modules where resolveJsonModule allows them, though positions inside JSON are not selectable. Standard libraries follow target, lib, noLib, and lib references, from the TypeScript 7.0.2 declarations vendored with this build: their declarations join groups, but their files are never searched. Project references are not followed, so a configuration that needs them is rejected rather than searched partially, and JavaScript inputs are not supported.

| Flag | Summary |
| --- | --- |
| `--references` | Group a symbol's declarations and occurrences in one project. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--calls` | Keep only direct call, new, and tagged-template targets. |
| `--no-tests` | Omit occurrences in test paths, keeping the selected declarations. |
| `--file FILE` | Keep only occurrences in FILE, one of the searched files; unlike the FILE operand, it does not narrow the candidates. |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag given more than once takes its last value. --pretty is valid without --json. Display limits: at most 200 occurrences counted across groups; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --references --project tsconfig.json --name run`: Every lexical declaration, local import, or exported alias spelled run.
- `tsparse --references --project tsconfig.json --name run src/app.ts --file src/api.ts`: Only the candidates declared in src/app.ts (the FILE operand), and only their occurrences in src/api.ts (--file).
- `tsparse --references --project tsconfig.json --name RunHost.observe --calls --no-tests`: A member, by containment: its calls outside test paths.
- `tsparse --references --project tsconfig.json --at 12:8 src/app.ts --pretty`: The symbol at a position, as the indented document --pretty writes without --json.
- `tsparse --references --direct --project tsconfig.json --at src/app.ts:12:8`: Answered in this process instead of by the project's service.

| Exit | Reports |
| --- | --- |
| 0 | A complete result, including one with no groups for a token-free position or an unmatched name. |
| 1 | A partial member search, an ambiguous or missing dotted name, an unsupported or unresolved selected target, invalid configuration content or inheritance, a syntax error, unsupported required input, invalid binding, a memory or time limit, or a name resolution looks up and does not find (an import, a types entry, or a /// &lt;reference> target). |
| 2 | A usage error or invalid position; a file the request or configuration names (the --project file, an extends target, a files entry) that does not exist, or a file or directory that cannot be read; a query file that is not one of the project's searched files; a service that cannot be started or reached; or a result that cannot be written. |

Related: [show](#show), [type](#type), [references-status](#references-status). Document: [docs/references.md](references.md).

## replace-body

Preview replacing the complete body of the selected function, method, constructor, accessor, or arrow function.

```text
tsparse --replace-body (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) --from FRAGMENT [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

FRAGMENT is the complete new body: a block { ... }, or an expression for an arrow function; a variable or property whose initializer is directly a function or arrow function has that function's body replaced. Selection is --show's, in FILE or, with --project, in the project's searched files; an ambiguous or missing selection changes nothing. The preview prints each selected site and a unified diff of the exact result, validated for syntax and edit boundaries (preserved syntax, comment directives, and JSDoc keep their meaning), not type-checked. Nothing is written unless --plan saves the edits to PLAN for --apply; --plan and --output are written only as new files, never as a file the command names. A fragment read from stdin cannot be read again by a command that shows what a cut withholds, so such a preview is never cut and refuses --limit N.

| Flag | Summary |
| --- | --- |
| `--replace-body` | Preview replacing the complete body of the selected declaration. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--from FRAGMENT` | The file holding the new body or statements; - reads stdin. |
| `--plan PLAN` | Save the prepared edits to PLAN, a new file, for --apply. |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json. Display limits: at most 50 candidates of an ambiguous selection and 400 diff lines per changed file; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --replace-body --name start src/host.ts --from body.ts`: body.ts holds the complete new body, { ... }; prints the diff.
- `tsparse --replace-body --project tsconfig.json --name RunHost.start --from body.ts --plan start.plan.json`: Save the previewed edit as a plan for --apply.

| Exit | Reports |
| --- | --- |
| 0 | A preview, or no_change when the edits change nothing; with --plan, the plan is saved. |
| 1 | An ambiguous or missing selection, a rejected edit, or an analysis failure such as a syntax error. |
| 2 | A usage error, a fragment or file that cannot be read, an invalid position, a file outside the project, a service that cannot be started or reached, or a result or plan that cannot be written. |

Related: [insert-after](#insert-after), [edit-batch](#edit-batch), [apply](#apply). Document: [docs/editing.md](editing.md).

## insert-after

Preview inserting complete statements after the statement the selected declaration designates.

```text
tsparse --insert-after (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) --from FRAGMENT [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

FRAGMENT holds complete statements, inserted after the statement the selected declaration designates (a binding's variable statement, a specifier's import or export) in its file, block, module block, or case clause. Selection is --show's, in FILE or, with --project, in the project's searched files; an ambiguous or missing selection changes nothing. The preview prints each selected site and a unified diff of the exact result, validated for syntax and edit boundaries (preserved syntax, comment directives, and JSDoc keep their meaning), not type-checked. Nothing is written unless --plan saves the edits to PLAN for --apply; --plan and --output are written only as new files, never as a file the command names. A fragment read from stdin cannot be read again by a command that shows what a cut withholds, so such a preview is never cut and refuses --limit N.

| Flag | Summary |
| --- | --- |
| `--insert-after` | Preview inserting statements after the selected declaration's statement. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--from FRAGMENT` | The file holding the new body or statements; - reads stdin. |
| `--plan PLAN` | Save the prepared edits to PLAN, a new file, for --apply. |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json. Display limits: at most 50 candidates of an ambiguous selection and 400 diff lines per changed file; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --insert-after --at src/host.ts:3:7 --from statements.ts --json`: Insert the statements in statements.ts after the statement declaring the name at 3:7, previewed as a document.

| Exit | Reports |
| --- | --- |
| 0 | A preview, or no_change when the edits change nothing; with --plan, the plan is saved. |
| 1 | An ambiguous or missing selection, a rejected edit, or an analysis failure such as a syntax error. |
| 2 | A usage error, a fragment or file that cannot be read, an invalid position, a file outside the project, a service that cannot be started or reached, or a result or plan that cannot be written. |

Related: [replace-body](#replace-body), [edit-batch](#edit-batch), [apply](#apply). Document: [docs/editing.md](editing.md).

## edit-batch

Preview several edits from a JSON file, all selected in the original files.

```text
tsparse --edit-batch EDITS [--project TSCONFIG [--direct]] [--plan PLAN] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

EDITS is {"edits": [{"operation": "replace_body" or "insert_after", "name" or "at", "file", "fragment" or "from"}]}: at is LINE:COLUMN with file, or FILE:LINE:COLUMN, paths are relative to the working directory, a member that is null is absent, and unknown fields are errors. An invalid batch reports every issue by its JSON path, such as edits[1].extra, before anything is read or prepared. Every edit is selected in the original files and the edits are prepared together. The preview prints each selected site and a unified diff of the exact result, validated for syntax and edit boundaries (preserved syntax, comment directives, and JSDoc keep their meaning), not type-checked. Nothing is written unless --plan saves the edits to PLAN for --apply; --plan and --output are written only as new files, never as a file the command names. A fragment read from stdin cannot be read again by a command that shows what a cut withholds, so such a preview is never cut and refuses --limit N.

| Flag | Summary |
| --- | --- |
| `--edit-batch EDITS` | Preview the edits a JSON batch file names, selected together. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--plan PLAN` | Save the prepared edits to PLAN, a new file, for --apply. |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json. Display limits: at most 50 candidates of an ambiguous selection and 400 diff lines per changed file; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --edit-batch edits.json --project tsconfig.json --plan edits.plan.json`: Prepare every edit in edits.json together and save the plan.

| Exit | Reports |
| --- | --- |
| 0 | A preview, or no_change when the edits change nothing; with --plan, the plan is saved. |
| 1 | An ambiguous or missing selection, a rejected edit, or an analysis failure such as a syntax error. |
| 2 | A usage error, a batch file that cannot be read or is not an edit batch, a fragment or file that cannot be read, an invalid position, a file outside the project, a service that cannot be started or reached, or a result or plan that cannot be written. |

Related: [replace-body](#replace-body), [insert-after](#insert-after), [apply](#apply). Document: [docs/editing.md](editing.md).

## apply

Write a saved plan's exact bytes, after checking that every file still has its original bytes and identity.

```text
tsparse --apply PLAN [--json [--pretty]] [--output OUTPUT]
```

Writes exactly the bytes --plan saved, restoring every file if a later one cannot be replaced, and reports the effect on the files. Keep other writers away from the files while applying. A plan records absolute paths, so it applies from any directory; it reads no TSPARSE_* variable and selects nothing again. --output is written only as a new file, never as a file the plan names.

| Flag | Summary |
| --- | --- |
| `--apply PLAN` | Write a saved plan's exact bytes after checking every file it names. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json.

Examples:

- `tsparse --apply start.plan.json`: Apply a plan without selecting anything again.
- `tsparse --apply start.plan.json --json -o applied.json`: Report the outcome and the effect on the files as a document in a new file.

| Exit | Reports |
| --- | --- |
| 0 | Every file replaced (effect applied), or a plan that changes nothing (no_change). |
| 1 | A plan another build wrote or whose edits no longer give its text, or a file changed since preparing (effect unchanged); preparing again resolves it. |
| 2 | A usage error; a plan that cannot be read or is not a plan; a file that cannot be read, a staging failure, or a failed first replacement (unchanged); a later failure after which every file was restored (rolled_back); a file that could not be restored (recovery_required); or a result that cannot be written. |

Related: [replace-body](#replace-body), [insert-after](#insert-after), [edit-batch](#edit-batch). Document: [docs/editing.md](editing.md).

## references-status

Report the state of the project's service; it loads nothing, does not check freshness, and does not extend the service's idle time.

```text
tsparse --references-status --project TSCONFIG [--json] [--pretty] [--output OUTPUT]
```

--references-status, --references-stop, and --references-serve administer the project service, which answers every project query: --references, --type, and --outline, --show, and edits with --project. On macOS and Linux, project queries reuse one service per configuration spelling and build, started automatically, which reloads the project when its inputs change. Status prints running: with the service's pid, generation (the number of snapshots it has published), build, idle timeout, and the idle time it has left, then limits: with the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with; or no service: TSCONFIG. --json prints the document instead: running, pid, generation, build, limits (max_memory_mb, query_timeout_ms), idle_secs, idle_remaining_secs, and started_unix; without a service running is false, generation 0, and every other field null; --pretty, with or without --json, prints it indented. It reads only TSPARSE_SERVICE_DIR, the private runtime directory (default /tmp/tsparse-UID, owned by this user with mode 0700).

| Flag | Summary |
| --- | --- |
| `--references-status` | Report whether the project's service is running. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag given more than once takes its last value. --pretty is valid without --json.

Examples:

- `tsparse --references-status --project tsconfig.json`: Whether the service runs, and its pid, generation, build, idle time, and limits.
- `tsparse --references-status --project tsconfig.json --pretty`: The status document, indented.

| Exit | Reports |
| --- | --- |
| 0 | The status, also when no service is running. |
| 2 | A usage error, a runtime directory that is not private, a service that cannot be reached, or a result that cannot be written. |

Related: [references-stop](#references-stop), [references-serve](#references-serve). Document: [docs/references.md](references.md).

## references-stop

Stop the project's service and release its cache.

```text
tsparse --references-stop --project TSCONFIG
```

--references-status, --references-stop, and --references-serve administer the project service, which answers every project query: --references, --type, and --outline, --show, and edits with --project. It prints stopped: pid N when it stopped a service, or no service: TSCONFIG when none was running, on stdout. The next project query starts a new service. A service keeps the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with, so stop it after changing them; --doctor --project TSCONFIG reports a service whose limits differ from this environment's. It reads only TSPARSE_SERVICE_DIR.

| Flag | Summary |
| --- | --- |
| `--references-stop` | Stop the project's service. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag given more than once takes its last value.

Examples:

- `tsparse --references-stop --project tsconfig.json`: Release the service and its cache, printing stopped: pid N or no service: tsconfig.json.

| Exit | Reports |
| --- | --- |
| 0 | The service stopped, or none was running. |
| 2 | A usage error, a runtime directory that is not private, or a service that cannot be reached or stopped. |

Related: [references-status](#references-status), [references-serve](#references-serve). Document: [docs/references.md](references.md).

## references-serve

Run the project's service in the foreground, for a supervisor, until it is idle.

```text
tsparse --references-serve --project TSCONFIG
```

--references-status, --references-stop, and --references-serve administer the project service, which answers every project query: --references, --type, and --outline, --show, and edits with --project. A service exits after TSPARSE_SERVICE_IDLE_SECS idle seconds (default 600). Its runtime directory is TSPARSE_SERVICE_DIR (default /tmp/tsparse-UID): owned by this user with mode 0700, and short enough for a socket path of about 100 bytes. Its queries run under TSPARSE_MAX_MEMORY_MB (default 6144 MiB) and TSPARSE_QUERY_TIMEOUT_MS (default 60000); a limit failure exits the service and the next request starts a replacement, and memory sampling can overshoot. TSPARSE_SERVICE_IDLE_SECS, TSPARSE_MAX_MEMORY_MB, and TSPARSE_QUERY_TIMEOUT_MS must each be a positive integer. It prints nothing but a failure.

| Flag | Summary |
| --- | --- |
| `--references-serve` | Run the project's service in the foreground. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag given more than once takes its last value.

Examples:

- `tsparse --references-serve --project tsconfig.json`: Serve in this terminal, printing any startup failure.

| Exit | Reports |
| --- | --- |
| 0 | The service exited when idle or stopped, or another process already serves the project. |
| 1 | A query exceeded TSPARSE_MAX_MEMORY_MB or TSPARSE_QUERY_TIMEOUT_MS: the service answered it with that failure and exited, and the next request starts a replacement. |
| 2 | A usage error, an invalid TSPARSE_* variable, a runtime directory that cannot be created or is not private, or a failure before the service listens, such as a project that fails to load. |

Related: [references-status](#references-status), [references-stop](#references-stop). Document: [docs/references.md](references.md).

## doctor

Check what project queries depend on, one line per check; it loads nothing and starts nothing.

```text
tsparse --doctor [--project TSCONFIG] [--json [--pretty]]
```

Checks, in order: build (the version, build stamp, platform, and whether the project service, --apply, and memory monitoring work here); environment (each TSPARSE_* variable, its value or default, and each invalid value); runtime directory (its path, that it is a private directory of this user's with mode 0700 or does not exist yet, and the length of its socket paths beside the longest a socket address holds). With --project: configuration (the spelling as given, its absolute form, and that it can be read; another spelling of the same file has its own service); service (no service, with any startup diagnostic an earlier start left, which the next start removes; or its pid, generation, build, idle timeout and the time it has left, and when it started); and service limits (the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with beside this environment's, since a service keeps the environment it started with). Each check is one line, CHECK: detail; a finding adds its why and next lines, and a last line says ok: N checks pass or findings: and the checks with one. A check that cannot be performed is a finding. --json prints one document with an entry per check: name, status (pass or finding), detail, and a finding's why, next.argv, or remedy. Every transport failure of a project query ends with the doctor's command for its configuration.

| Flag | Summary |
| --- | --- |
| `--doctor` | Check what project queries depend on, one line per check. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json.

Examples:

- `tsparse --doctor`: The build, the environment, and the runtime directory.
- `tsparse --doctor --project tsconfig.json --json`: Every check for one project, its service included, as one document.

| Exit | Reports |
| --- | --- |
| 0 | Every check passes. |
| 1 | A check found something, or could not be performed. |
| 2 | A usage error, or a report that cannot be written. |

Related: [references-status](#references-status), [references-stop](#references-stop). Document: [docs/references.md](references.md).

## Flags

| Flag | Repeated | Summary |
| --- | --- | --- |
| `-h, --help` | stays on | Print help and exit. |
| `-V, --version` | stays on | Print the version and exit. |
| `--outline` | stays on | List one file's declarations, indented by containment. |
| `--show` | stays on | Print one declaration's exact source with its line numbers. |
| `--type` | stays on | Inspect the type of a declaration or token in one project. |
| `--references` | stays on | Group a symbol's declarations and occurrences in one project. |
| `--replace-body` | stays on | Preview replacing the complete body of the selected declaration. |
| `--insert-after` | stays on | Preview inserting statements after the selected declaration's statement. |
| `--edit-batch EDITS` | as the operation says | Preview the edits a JSON batch file names, selected together. |
| `--apply PLAN` | as the operation says | Write a saved plan's exact bytes after checking every file it names. |
| `--references-status` | refused | Report whether the project's service is running. |
| `--references-stop` | refused | Stop the project's service. |
| `--references-serve` | refused | Run the project's service in the foreground. |
| `--doctor` | refused | Check what project queries depend on, one line per check. |
| `--project TSCONFIG` | as the operation says | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | stays on | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | as the operation says | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | as the operation says | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--depth DEPTH` | as the operation says | Outline levels to show (N or all, default 2), or type edges to expand (0 through 32, default 3). |
| `--limit N|all` | as the operation says | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--expand` | stays on | Add a bounded structural graph of the type. |
| `--calls` | stays on | Keep only direct call, new, and tagged-template targets. |
| `--no-tests` | stays on | Omit occurrences in test paths, keeping the selected declarations. |
| `--file FILE` | as the operation says | Keep only occurrences in FILE, one of the searched files; unlike the FILE operand, it does not narrow the candidates. |
| `--from FRAGMENT` | as the operation says | The file holding the new body or statements; - reads stdin. |
| `--plan PLAN` | as the operation says | Save the prepared edits to PLAN, a new file, for --apply. |
| `--tsx` | stays on | Parse JSX, as a .tsx or .jsx file name already does. |
| `--check` | stays on | Validate syntax, printing one verdict line instead of the AST. |
| `--json` | stays on | Write one result document, also for a failure the analysis reports. |
| `--pretty` | stays on | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | as the operation says | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
