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
| `transport.lock` | transport | A lock file in the service's runtime directory cannot be opened or locked; next or remedy says how to select a writable directory. |
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
| [test-outline](#test-outline) | `--test-outline` | List the test calls of one test file in source order, one per line, indented by containment, with each test's name. |
| [show](#show) | `--show` | Print one declaration's exact source with its original line numbers. |
| [type](#type) | `--type` | Inspect the type of the selected declaration or token in one explicit project. |
| [references](#references) | `--references` | Group the declarations and occurrences of the selected symbol in one explicit project. |
| [definition](#definition) | `--definition` | Find the declarations the selected name denotes, following import and export aliases, in one explicit project. |
| [constructs](#constructs) | `--constructs` | List the object literals that construct the selected interface, class, or type alias in one explicit project, and the properties each does not certainly provide. |
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

Reads stdin when FILE is omitted or is -. A .tsx, .jsx, .js, .mjs, or .cjs file name, in any case, enables JSX, as --tsx does for stdin, and a JavaScript file name also rejects TypeScript-only syntax, such as a type annotation, as TypeScript does in a JavaScript file. The AST has TypeScript's node kinds and positions; a syntax error is FILE:LINE:COLUMN: message on stderr, with a one-based line and UTF-16 column. --check prints no AST but one line on stdout, ok: FILE parses as TypeScript 7 (jsx: off), or jsx: on, with - for stdin; it checks syntax only, and also fails, as TypeScript 7 does, on a namespace declared with module (TS1540), whose AST is still written without --check; --pretty and --output change nothing then.

| Flag | Summary |
| --- | --- |
| `--tsx` | Parse JSX, as a .tsx or JavaScript file name already does. |
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

## test-outline

List the test calls of one test file in source order, one per line, indented by containment, with each test's name.

```text
tsparse --test-outline FILE [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

A test call is an expression statement that calls a hook (beforeAll, beforeEach, afterAll, or afterEach) or describe, it, or test with Vitest's modifiers: only, skip, todo, concurrent, sequential, shuffle, and fails, and each(...), for(...), skipIf(...), and runIf(...) called before the test. Syntax alone decides it, with no import check and no project, so Vitest globals count; a call wrapped in await, parentheses, or another operator is not one, and nothing inside a computed property name is listed. Each line is LINE:COLUMN, the call, such as it.each(), and the test's name, its first argument as written without line breaks; every level is shown. Each position is the test statement's first token, so --show --at prints the whole test and --insert-after --at inserts after it. Only FILE is read, and text and JSON name it absolutely. TypeScript and JavaScript files are accepted as by --outline; JavaScript statements can be shown, but editing them is refused. File-only navigation reads no configuration and does not apply project diagnostics such as TS1540.

| Flag | Summary |
| --- | --- |
| `--test-outline` | List one test file's describe, it, and test calls and hooks, indented by containment. |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json. Display limits: at most 200 entries, in source order; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --test-outline src/app.test.ts`: Every suite, test, and hook with its position; the default shows 200 entries.
- `tsparse --test-outline src/app.test.ts --json -o tests.json`: Every entry, nested by containment, as one document in a file.

| Exit | Reports |
| --- | --- |
| 0 | The test view, which is empty for a file without test calls. |
| 1 | A syntax error, a file past the parser's grammar budget, or unsupported input. |
| 2 | A usage error, a file that cannot be read, or a result that cannot be written. |

Related: [outline](#outline), [show](#show), [insert-after](#insert-after). Document: [docs/navigation.md](navigation.md).

## show

Print one declaration's exact source with its original line numbers.

```text
tsparse --show (--name NAME FILE | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) [--limit N|all] [--json [--pretty]] [--output OUTPUT]
tsparse --show --project TSCONFIG (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) [--direct] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

--name selects an exact declared name, and a.b.c selects c inside declarations named b and then a; an import or export specifier has a name only where it renames, and a default import never does. So --name NAME FILE misses a name FILE imports or re-exports without renaming: the miss names each such specifier and the --definition --at command that finds its declaration, complete with --project and with the TSCONFIG placeholder without it; --json lists them in specifiers, with the command as next only when it is complete. --at selects the declaration whose name or anchor is at the position; where there is none, --at on a statement's first token selects that statement, such as an it(...) test or an expect(...) line. Overloads, merged declarations, and same-named locals are separate declarations: an ambiguous or missing selection exits 1 and lists each candidate's FILE:LINE:COLUMN and selector, on stderr or, with --json, in the document. With --project, FILE must be one of the files --references searches, --name without FILE searches all of them, and a printed FILE:LINE:COLUMN selects its declaration from any directory. Text names files absolutely; project JSON names them relative to the configuration's directory.

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

Related: [outline](#outline), [definition](#definition), [type](#type), [replace-body](#replace-body). Document: [docs/navigation.md](navigation.md).

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

--at selects the identifier at the position (one-based line, UTF-16 column). A bare NAME selects every lexical declaration, local import, or exported alias spelled exactly NAME and lists the matching members it did not search; a dotted NAME such as RunHost.observe selects a member, whose identity follows resolved receiver types, not spelling. FILE narrows the candidates; --file FILE then keeps only their occurrences in FILE. Import and export aliases join their underlying symbol; shadowed bindings form separate groups; an incomplete search reports partial coverage and its gaps. Text prints a header with the selection, its counts, and the filters that bound them, each group with its declarations and its occurrences by file (a renamed alias's spelling in parentheses), each gap, and one line counting the members a bare name did not search and naming the --at that searches one; files are named absolutely, and an ambiguous or missing selection is a diagnostic on stderr. --json writes the result document instead, which also delivers analysis failures as documents and names files relative to the configuration's directory; --pretty without --json writes that document indented. Supported: one configuration and its inherited settings; .ts, .tsx, .mts, and .cts sources and declarations; under allowJs, .js, .jsx, .mjs, and .cjs sources as plain ES modules or with one top-level module.exports = name, where other CommonJS, require, and JSDoc beyond a variable's @type fail loading at the construct; node16, nodenext, and bundler resolution; installed packages with declarations; and JSON modules where resolveJsonModule allows them, though positions inside JSON are not selectable. Standard libraries follow target, lib, noLib, and lib references, from the TypeScript 7.0.2 declarations vendored with this build, or for a project whose installed compiler is TypeScript 5 from that compiler's own lib directory by TypeScript 5.6.3's rules: their declarations join groups, but their files are never searched. Project references are not followed, so a configuration that needs them is rejected rather than searched partially.

| Flag | Summary |
| --- | --- |
| `--references` | Group a symbol's declarations and occurrences in one project. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--calls` | Keep only direct call, new, and tagged-template targets. |
| `--no-tests` | Omit results in test paths: references keep the selected declarations, and constructs skips the literals there. |
| `--file FILE` | Keep only results in FILE, one of the searched files: occurrences, or construction sites; unlike the FILE operand, it does not narrow the candidates. |
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

## definition

Find the declarations the selected name denotes, following import and export aliases, in one explicit project.

```text
tsparse --definition --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--json [--pretty]] [--output OUTPUT]
```

--at selects the identifier at the position (one-based line, UTF-16 column); --name selects every declaration, local import, or exported alias spelled exactly NAME, and FILE narrows the candidates to those in FILE, such as a name FILE only imports or re-exports. Each selected symbol is one group listing its declarations: the original declarations behind every import and export alias, in location order. A call or new lists every declaration of the callee, never one signature. Text prints a header with the selection and its counts, then each group and one FILE:LINE:COLUMN per declaration, naming files absolutely; --json writes the document instead, naming files relative to the configuration's directory. A position on a member (a property access, a member's name, a literal member name or key, or a parameter property's modifier) is not followed: it exits 1 naming the --references --at search that finds the member's declarations. A position in no identifier, such as a keyword, whitespace, or a module specifier, and a name with no candidate select nothing and exit 1, never an empty result. NAME is one identifier: --show --name takes a dotted one.

| Flag | Summary |
| --- | --- |
| `--definition` | Find the declarations a name denotes, following import and export aliases, in one project. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json.

Examples:

- `tsparse --definition --project tsconfig.json --at src/app.ts:1:10`: The declaration an import resolves to, through re-exports.
- `tsparse --definition --project tsconfig.json --name formatTitle src/app.ts`: The declarations of a name src/app.ts declares, imports, or exports.

| Exit | Reports |
| --- | --- |
| 0 | One or more groups, each with its declarations. |
| 1 | A missing selection, a member position (naming the --references search that finds its declarations), an unsupported or unresolved target, a semantic failure or limit while selecting, or a project analysis failure. |
| 2 | A usage error, a file that cannot be read or another I/O failure, an invalid position, a file outside the project, an internal failure, a service that cannot be started or reached, or a result that cannot be written. |

Related: [references](#references), [show](#show). Document: [docs/references.md](references.md).

## constructs

List the object literals that construct the selected interface, class, or type alias in one explicit project, and the properties each does not certainly provide.

```text
tsparse --constructs --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--missing KEY] [--all-literals] [--no-tests] [--file FILE] [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

--name takes --show's declaration containment selectors and FILE narrows the seeds; --at selects the declaration's name. By default the object literals of the searched files examined are those with a member naming a key the target declares, or with no named member ({}, or only spreads and computed names that are not literals). --all-literals examines every literal, as a target always does when an alternative has an index signature or is an empty object type or the global Object type: also those whose named members name only other keys, which construct the target, for example, under a type assertion such as {other: 1} as Options, beside a union member those keys choose, in a union context with a member accepting any key such as View | {}, or with an excess-property error. A site is one whose contextual type, as declared and as TypeScript discriminates it, is the target or one of its union alternatives, generic instantiations and aliases of intersections included; bases, intersection parts, and mapped forms such as Partial&lt;T> are other types. Each site lists, per constructed alternative, its properties the literal lacks, quoted, marked ? when optional, absent or of unknown presence in the compiler's type of the literal (a spread of an optional property, an index signature, or any); this is not the runtime object. A literal spread into another literal belongs to that literal's site. --missing KEY keeps the sites that lack the string key KEY, compared exactly. Literals whose context cannot be established are located gaps, and the coverage is partial. A query runs under TSPARSE_MAX_MEMORY_MB (default 6144 MiB) and TSPARSE_QUERY_TIMEOUT_MS (default 60000), positive integers checked before any service starts; a service keeps the values it started with, and a limit failure exits the process that hit it.

| Flag | Summary |
| --- | --- |
| `--constructs` | List the object literals that construct a type in one project, and the properties each lacks. |
| `--project TSCONFIG` | The project's configuration file, as spelled; no ancestor directory is searched. |
| `--direct` | Load a fresh project in this process instead of asking the project's service. |
| `--at [FILE:]LINE:COLUMN` | Select by position: a one-based line and UTF-16 column, in FILE or the FILE operand. |
| `--name NAME` | Select by exact declared name; a dotted name such as RunHost.run selects by containment. |
| `--missing KEY` | Keep only the construction sites whose constructed type declares the string key KEY, compared exactly, and that do not certainly provide it. |
| `--all-literals` | Examine every object literal, also those whose named members name only keys the target does not declare; several times slower on a large project. |
| `--no-tests` | Omit results in test paths: references keep the selected declarations, and constructs skips the literals there. |
| `--file FILE` | Keep only results in FILE, one of the searched files: occurrences, or construction sites; unlike the FILE operand, it does not narrow the candidates. |
| `--limit N|all` | Show at most N items of each list the operation bounds, or all of them; without it each list has a default, which -o removes. |
| `--json` | Write one result document, also for a failure the analysis reports. |
| `--pretty` | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
| `-h, --help` | Print help and exit. |
| `-V, --version` | Print the version and exit. |

A value flag may be given once. --pretty is valid only with --json. Display limits: at most 200 construction sites, 20 properties each construction lacks, 50 gaps of a partial answer and 50 candidates of an ambiguous selection; --limit N|all sets them, -o without --limit removes them, and a cut's last line names the command that shows the rest.

Examples:

- `tsparse --constructs --project tsconfig.json --name RunView`: Every literal that builds a RunView, and the properties each lacks.
- `tsparse --constructs --project tsconfig.json --name RunView --missing lane --no-tests`: Only the sites outside test paths that do not certainly provide lane.
- `tsparse --constructs --project tsconfig.json --at src/views.ts:12:18 --file src/fixtures.ts --json`: The type declared at a position, its sites in one file, as a document.
- `tsparse --constructs --project tsconfig.json --name Options --all-literals --json -o sites.json`: Every literal examined, also those naming only keys Options does not declare; the whole document goes to a file.

| Exit | Reports |
| --- | --- |
| 0 | A complete answer, including one with no site. |
| 1 | A partial answer, an ambiguous or missing selection, an unsupported target, a --missing KEY the target does not declare, an analysis failure, or a memory, time, depth, or output limit. |
| 2 | A usage error, a file that cannot be read or another I/O failure, an invalid position, a file outside the project, an internal failure, a service that cannot be started or reached, or a result that cannot be written. |

Related: [type](#type), [references](#references). Document: [docs/constructs.md](constructs.md).

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

FRAGMENT holds complete statements, inserted after the statement the selected declaration designates (a binding's variable statement, a specifier's import or export) in its file, block, module block, or case clause. Where no declaration is named or anchored at the position, --at on a statement's first token selects that statement, so statements can be inserted after a statement in a statement list, such as an it(...) test or an expect(...) line. A selected callback is refused; when its innermost enclosing statement is in a statement list and --at on that statement's first token selects only that statement, the refusal names that position. Selection is --show's, in FILE or, with --project, in the project's searched files; an ambiguous or missing selection changes nothing. The preview prints each selected site and a unified diff of the exact result, validated for syntax and edit boundaries (preserved syntax, comment directives, and JSDoc keep their meaning), not type-checked. Nothing is written unless --plan saves the edits to PLAN for --apply; --plan and --output are written only as new files, never as a file the command names. A fragment read from stdin cannot be read again by a command that shows what a cut withholds, so such a preview is never cut and refuses --limit N.

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
- `tsparse --insert-after --at src/host.test.ts:10:3 --from new-test.ts`: Insert after the statement whose first token is at 10:3, such as an it(...) test.

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

EDITS is {"edits": [{"operation": "replace_body" or "insert_after", "name" or "at", "file", "fragment" or "from"}]}: at is LINE:COLUMN with file, or FILE:LINE:COLUMN, paths are relative to the working directory, a member that is null is absent, and unknown fields are errors. An invalid batch reports every issue by its JSON path, such as edits[1].extra, before anything is read or prepared. at selects as --show --at does: where no declaration is named or anchored, a statement's first token selects that statement, and insert_after inserts after it when it is in a statement list, such as an it(...) test or an expect(...) line. Every edit is selected in the original files and the edits are prepared together. The preview prints each selected site and a unified diff of the exact result, validated for syntax and edit boundaries (preserved syntax, comment directives, and JSDoc keep their meaning), not type-checked. Nothing is written unless --plan saves the edits to PLAN for --apply; --plan and --output are written only as new files, never as a file the command names. A fragment read from stdin cannot be read again by a command that shows what a cut withholds, so such a preview is never cut and refuses --limit N.

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
| 1 | A plan another build wrote or whose edits no longer give its text, or a file changed since preparing (effect unchanged), which preparing again resolves; or a plan that records a file editing refuses, such as a JavaScript file (Unsupported, naming the file). |
| 2 | A usage error; a plan that cannot be read or is not a plan; a file that cannot be read, a staging failure, or a failed first replacement (unchanged); a later failure after which every file was restored (rolled_back); a file that could not be restored (recovery_required); or a result that cannot be written. |

Related: [replace-body](#replace-body), [insert-after](#insert-after), [edit-batch](#edit-batch). Document: [docs/editing.md](editing.md).

## references-status

Report the state of the project's service; it loads nothing, does not check freshness, and does not extend the service's idle time.

```text
tsparse --references-status --project TSCONFIG [--json] [--pretty] [--output OUTPUT]
```

--references-status, --references-stop, and --references-serve administer the project service, which answers every project query: --references, --definition, --type, --constructs, and --outline, --show, and edits with --project. On macOS and Linux, project queries reuse one service per configuration spelling and build, started automatically, which reloads the project when its inputs change. Status prints running: with the service's pid, generation (the number of snapshots it has published), build, idle timeout, and the idle time it has left, then limits: with the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with; or no service: TSCONFIG. --json prints the document instead: running, pid, generation, build, limits (max_memory_mb, query_timeout_ms), idle_secs, idle_remaining_secs, and started_unix; without a service running is false, generation 0, and every other field null; --pretty, with or without --json, prints it indented. It reads only the private runtime directory, owned by this user with mode 0700: TSPARSE_SERVICE_DIR when it is set; otherwise TMPDIR/tsparse-UID when TMPDIR is a non-empty absolute path short enough for socket paths; otherwise /tmp/tsparse-UID. Environments that choose different runtime directories, such as a shell and a sandbox with another TMPDIR, have separate services; the same TSPARSE_SERVICE_DIR in both shares one.

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

--references-status, --references-stop, and --references-serve administer the project service, which answers every project query: --references, --definition, --type, --constructs, and --outline, --show, and edits with --project. It prints stopped: pid N when it stopped a service, or no service: TSCONFIG when none was running, on stdout. The next project query starts a new service. A service keeps the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with, so stop it after changing them; --doctor --project TSCONFIG reports a service whose limits differ from this environment's. It reads only the runtime directory, which TSPARSE_SERVICE_DIR and TMPDIR choose.

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

--references-status, --references-stop, and --references-serve administer the project service, which answers every project query: --references, --definition, --type, --constructs, and --outline, --show, and edits with --project. A service exits after TSPARSE_SERVICE_IDLE_SECS idle seconds (default 600). Its runtime directory is TSPARSE_SERVICE_DIR when it is set; otherwise TMPDIR/tsparse-UID when TMPDIR is a non-empty absolute path short enough for socket paths; otherwise /tmp/tsparse-UID: owned by this user with mode 0700, and short enough for a socket path of about 100 bytes; a project query passes the directory it chose to the service it starts. Its queries run under TSPARSE_MAX_MEMORY_MB (default 6144 MiB) and TSPARSE_QUERY_TIMEOUT_MS (default 60000); a limit failure exits the service and the next request starts a replacement, and memory sampling can overshoot. TSPARSE_SERVICE_IDLE_SECS, TSPARSE_MAX_MEMORY_MB, and TSPARSE_QUERY_TIMEOUT_MS must each be a positive integer. It prints nothing but a failure.

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

Checks, in order: build (the version, build stamp, platform, and whether the project service, --apply, and memory monitoring work here); environment (each TSPARSE_* variable, its value or default, and each invalid value); runtime directory (its path and the rule that chose it, which a finding also names before its failure unless no directory could be chosen, as for a configuration whose service cannot be named or a TSPARSE_SERVICE_DIR that cannot be made absolute: TSPARSE_SERVICE_DIR, TMPDIR/tsparse-UID, or /tmp/tsparse-UID with why TMPDIR was not used; that it is a private directory of this user's with mode 0700 or does not exist yet; and the length of its socket paths beside the longest a socket address holds). With --project: configuration (the spelling as given, its absolute form, and that it can be read; another spelling of the same file has its own service); compatibility (the TypeScript package installed nearest the configuration, by its manifest and version, or none; the dialect it selects, TypeScript 5.6 for any 5.x and TypeScript 7 otherwise, with validated for 5.6.3 only for a 5.x other than 5.6.3; TypeScript 7.0.2 as the semantic authority; and the standard library's source; a manifest that is unreadable or names no version is a finding, as loading would fail); service (no service, with any startup diagnostic an earlier start left, which the next start removes; or its pid, generation, build, idle timeout and the time it has left, and when it started); and service limits (the TSPARSE_MAX_MEMORY_MB and TSPARSE_QUERY_TIMEOUT_MS it started with beside this environment's, since a service keeps the environment it started with). Each check is one line, CHECK: detail; a finding adds its why and next lines, and a last line says ok: N checks pass or findings: and the checks with one. A check that cannot be performed is a finding. --json prints one document with an entry per check: name, status (pass or finding), detail, and a finding's why, next.argv, or remedy. Every transport failure of a project query ends with the doctor's command for its configuration. Directory creation/inspection and lock failures also name writable TMPDIR/TSPARSE_SERVICE_DIR remedies, including why TMPDIR was skipped; privacy failures instead name a permission or ownership remedy such as chmod 700. A passing runtime metadata check does not prove lock writability.

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
| `--test-outline` | stays on | List one test file's describe, it, and test calls and hooks, indented by containment. |
| `--show` | stays on | Print one declaration's exact source with its line numbers. |
| `--type` | stays on | Inspect the type of a declaration or token in one project. |
| `--references` | stays on | Group a symbol's declarations and occurrences in one project. |
| `--definition` | stays on | Find the declarations a name denotes, following import and export aliases, in one project. |
| `--constructs` | stays on | List the object literals that construct a type in one project, and the properties each lacks. |
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
| `--no-tests` | stays on | Omit results in test paths: references keep the selected declarations, and constructs skips the literals there. |
| `--file FILE` | as the operation says | Keep only results in FILE, one of the searched files: occurrences, or construction sites; unlike the FILE operand, it does not narrow the candidates. |
| `--missing KEY` | as the operation says | Keep only the construction sites whose constructed type declares the string key KEY, compared exactly, and that do not certainly provide it. |
| `--all-literals` | stays on | Examine every object literal, also those whose named members name only keys the target does not declare; several times slower on a large project. |
| `--from FRAGMENT` | as the operation says | The file holding the new body or statements; - reads stdin. |
| `--plan PLAN` | as the operation says | Save the prepared edits to PLAN, a new file, for --apply. |
| `--tsx` | stays on | Parse JSX, as a .tsx or JavaScript file name already does. |
| `--check` | stays on | Validate syntax, printing one verdict line instead of the AST. |
| `--json` | stays on | Write one result document, also for a failure the analysis reports. |
| `--pretty` | stays on | Indent JSON output: alone where the output is JSON, and with --json elsewhere. |
| `-o, --output OUTPUT` | as the operation says | Write the result to OUTPUT instead of stdout; editing writes it only as a new file. |
