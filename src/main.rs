use std::{
    env, fs,
    io::{self, Read, Write},
    process::ExitCode,
};
use typescript_parser::{ParseOptions, parse_with_options};

mod cli;
mod process_limits;
#[cfg(unix)]
mod service;

use cli::Exit;
use cli::delivery::write_output;
use cli::request::{self, Request};

const HELP: &str = "\
tsparse [--pretty] [--tsx] [--check] [-o OUTPUT] [FILE|-]
tsparse --references --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--calls] [--no-tests] [--json] [--pretty] [-o OUTPUT]
tsparse --type --project TSCONFIG (--at LINE:COLUMN FILE | --at FILE:LINE:COLUMN | --name NAME [FILE]) [--direct] [--expand [--depth 0..32]] [--json [--pretty]] [-o OUTPUT]
tsparse (--references-status | --references-stop | --references-serve) --project TSCONFIG
tsparse --outline FILE [--depth N|all] [--json [--pretty]] [-o OUTPUT]
tsparse --show (--name NAME FILE | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) [--json [--pretty]] [-o OUTPUT]
tsparse --outline --project TSCONFIG FILE [--direct] [--depth N|all] [--json [--pretty]] [-o OUTPUT]
tsparse --show --project TSCONFIG (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) [--direct] [--json [--pretty]] [-o OUTPUT]
tsparse (--replace-body | --insert-after) (--name NAME [FILE] | --at LINE:COLUMN FILE | --at FILE:LINE:COLUMN) --from FRAGMENT [--project TSCONFIG [--direct]] [--plan PLAN] [--json [--pretty]] [-o OUTPUT]
tsparse --edit-batch EDITS [--project TSCONFIG [--direct]] [--plan PLAN] [--json [--pretty]] [-o OUTPUT]
tsparse --apply PLAN [--json [--pretty]] [-o OUTPUT]

Parse TypeScript into a compiler-style JSON AST. Reads stdin when FILE is omitted.
--check validates syntax without producing JSON.

On macOS and Linux, project queries (--references, --type, and --outline and --show with
--project) reuse one local service per TSCONFIG, started automatically, which
reloads the project when its inputs change before answering any of them.
--direct (for references also --references-direct) loads a fresh project in
this process instead. --references-status reports service state;
--references-stop releases it; --references-serve runs it in the foreground.
Services exit after ten idle minutes. TSPARSE_SERVICE_DIR selects a private
runtime directory, short enough for a socket path of about 100 bytes;
TSPARSE_SERVICE_IDLE_SECS sets idle seconds.
Read-only queries use a process memory threshold and elapsed-time limit:
TSPARSE_MAX_MEMORY_MB (default 6144 MiB) and TSPARSE_QUERY_TIMEOUT_MS (default
60000). Both must be positive integers. Restart an existing service after
changing them. A limit failure exits that process; the next request restarts
its service. Memory sampling can overshoot. File application is outside this
watchdog; Rust library callers retain control of their own process.

--references groups the declarations and occurrences of the symbol selected by
the identifier at LINE:COLUMN in FILE (one-based line, UTF-16 column), or by
every lexical declaration, local import, or exported alias spelled exactly
NAME. A dotted containment selector such as RunHost.observe selects a member;
FILE narrows the candidates. Member identity follows resolved receiver types,
not spelling alone. Incomplete searches report partial coverage and gaps. --at FILE:LINE:COLUMN, the form
results print, names the file without a FILE operand. Import and export aliases
join their underlying symbol; shadowed bindings form separate groups. Argument
paths are relative to the working directory; result paths are relative to the
directory of the reported project, the TSCONFIG directory's real path.
--calls keeps direct call, new and tagged-template targets; --no-tests omits
occurrences in test paths, preserving selected declarations. --json also
delivers analysis errors as documents; usage and transport errors use stderr.
Supported: one configuration and its inherited settings; .ts, .tsx, .mts, and
.cts sources and declarations; node16, nodenext, and bundler resolution; and
installed packages with declarations. Project references are not followed:
a configuration that needs them is rejected rather than searched partially.
Standard libraries follow target, lib, noLib, and lib references, from the
TypeScript 7.0.2 declarations vendored with this build: their declarations
join groups, but their files are never searched. JSON modules are program
inputs where resolveJsonModule allows them. Select semantic members at their
TypeScript declarations/uses or with a dotted containment selector. Positions
inside JSON are not selectable. JavaScript inputs are not supported.

--type inspects the selected declaration or token's type in one explicit
project. --name uses declaration containment selectors; FILE narrows seeds.
--expand adds a bounded structural graph; --depth counts type edges, defaults
to 3 and accepts 0 through 32. --json delivers a structured type or error;
--pretty indents it. Semantic dependencies that cannot be established report located failures;
partial expansion is explicit and does not claim full TypeScript checking.

--outline lists the declarations of one TypeScript FILE in source order, one
per line as LINE:COLUMN, kind, and signature (or label), indented by
containment; an import declaration takes one line, at its first name, listing
the names it imports. --depth shows N levels (default 2; all shows every level)
and counts what it hides. --show prints one declaration's exact source with its
original line numbers. --name selects an exact declared name; a.b.c selects c
inside declarations named b and then a. An import or export specifier names a
declaration only when it renames, and a default import never does (as
TypeScript's workspace symbols count them), so a name finds what was imported,
not each import. --at selects the declaration whose
name or anchor is at LINE:COLUMN. Overloads, merged declarations, and
same-named locals are separate declarations: when a selection is ambiguous or
missing, show exits 1 and lists the candidates' locations and selectors on
stderr. --json writes one result document instead, also for ambiguous or
missing selections and errors; -o receives it. Without --project, navigation
reads only FILE. With --project, FILE must be one of the files references
search; --show --name without FILE searches all of them; and a project that
fails to load answers nothing, never falling back to FILE alone. Project JSON
names the configuration as project and files relative to its directory; text
names files absolutely. A printed FILE:LINE:COLUMN, or a JSON candidate's file
joined to the configuration's directory with its anchor, selects that
declaration with --show --project TSCONFIG --at FILE:LINE:COLUMN from any
directory.

--replace-body replaces the complete body of the selected function, method,
constructor, accessor, or arrow function, or of a variable or property whose
initializer is directly one: FRAGMENT is the new body, a block { ... }, or an
expression for an arrow function. --insert-after inserts FRAGMENT, complete
statements, after the statement the selected declaration designates (a
binding's variable statement, a specifier's import or export) in its file,
block, module block, or case clause. Selection is --show's, in FILE or, with
--project, in the project's searched files; a selection that is ambiguous or
missing changes nothing. --from - reads FRAGMENT from stdin. --edit-batch reads
several edits from a JSON file, {\"edits\": [{\"operation\": \"replace_body\" or
\"insert_after\", \"name\" or \"at\", \"file\", \"fragment\" or \"from\"}]}, all selected in
the original files. Paths are relative to the working directory.
An edit prints its selected sites and a unified diff of the exact result,
validated for syntax and edit boundaries (preserved syntax, comment directives,
and JSDoc keep their meaning), not type-checked. Nothing is written unless
--plan saves the batch to PLAN, a new file; --apply PLAN then writes exactly
those bytes, after checking that every file still has its original bytes and
identity, and restores every file if a later one cannot be replaced. Keep
other writers away from the files while applying. Editing writes -o and --plan
only as new files, never as a file the command or its plan names.

In reference mode, exit 1 also reports project analysis failures, unsupported
inputs, and unsupported or unresolved selected targets. In navigation, exit 1
also reports an ambiguous or missing selection, project analysis failures, and
unsupported input, and an invalid position or a file outside the project exits
2. A service that cannot be reached exits 2 with a diagnostic, also with --json.
In editing, exit 1 also reports an ambiguous or missing selection, a rejected
edit, a plan from another build or whose edits no longer give its text, and
files changed since preparing; exit 2 also reports a failed write, a rollback,
and files that need recovery. JSON documents name the effect on the files.
Exit codes: 0 success, 1 syntax error, 2 usage or I/O error.";

fn run() -> Result<(), Exit> {
    // Requests name files exactly, so an argument that is not Unicode is a
    // usage error; only the message shows its lossy form.
    let args = env::args_os()
        .skip(1)
        .map(|arg| {
            arg.into_string().map_err(|arg| {
                (
                    2,
                    format!("Arguments must be Unicode: {}", arg.to_string_lossy()),
                )
            })
        })
        .collect::<Result<Vec<String>, Exit>>()?;
    match request::parse(args.into_iter()).map_err(|message| (2, message))? {
        Request::Help => print(&format!("{HELP}\n")),
        Request::Version => print(&format!("tsparse {}\n", env!("CARGO_PKG_VERSION"))),
        Request::Parse(request) => parse(request),
        Request::Service(request) => service_action(request),
        Request::Query(query) => cli::query::run(query),
        Request::Edit(command) => cli::editing::run(command),
    }
}

fn print(text: &str) -> Result<(), Exit> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|_| stdout.flush())
        .map_err(|e| (2, e.to_string()))
}

fn parse(request: request::Parse) -> Result<(), Exit> {
    let name = request.file.as_deref().unwrap_or("-");
    let file = ParseOptions::for_file(if name == "-" { "source.ts" } else { name });
    let source = if name == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| (2, e.to_string()))?;
        source
    } else {
        fs::read_to_string(name).map_err(|e| (2, format!("{name}: {e}")))?
    };
    let ast = parse_with_options(
        &source,
        ParseOptions {
            jsx: request.jsx || file.jsx,
            ..file
        },
    )
    .map_err(|e| {
        let (line, column) = e.line_column(&source);
        (1, format!("{name}:{line}:{column}: {}", e.message))
    })?;
    if request.check {
        return Ok(());
    }
    write_output(request.output.as_deref(), |writer| {
        ast.write_json(writer, request.pretty)
    })
}

#[cfg_attr(not(unix), allow(unused_variables))]
fn service_action(request: request::Service) -> Result<(), Exit> {
    #[cfg(unix)]
    return match request.action {
        request::ServiceAction::Status => {
            let status =
                service::status(request.project.as_ref()).map_err(|e| (2, e.to_string()))?;
            write_output(request.output.as_deref(), |writer| {
                if request.pretty {
                    serde_json::to_writer_pretty(writer, &status)
                } else {
                    serde_json::to_writer(writer, &status)
                }
                .map_err(io::Error::other)
            })
        }
        request::ServiceAction::Stop => {
            service::stop(request.project.as_ref()).map_err(|e| (2, e.to_string()))
        }
        request::ServiceAction::Serve => {
            service::serve(request.project.as_ref()).map_err(|e| (2, e.to_string()))
        }
    };
    #[cfg(not(unix))]
    return Err((2, format!("{} is available only on Unix", request.flag)));
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            if !message.is_empty() {
                eprintln!("{message}");
            }
            ExitCode::from(code)
        }
    }
}
