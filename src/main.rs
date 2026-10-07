use std::{
    env, fs,
    io::{self, BufWriter, Read, Write},
    path::PathBuf,
    process::ExitCode,
};
use typescript_parser::references::{
    ProjectErrorKind, QueryErrorKind, ReferenceError, ReferenceProject, ReferenceQuery,
    SourcePosition,
};
use typescript_parser::{ParseOptions, parse_with_options};

#[cfg(unix)]
mod service;

const HELP: &str = "\
tsparse [--pretty] [--tsx] [--check] [-o OUTPUT] [FILE|-]
tsparse --references --project TSCONFIG (--at LINE:COLUMN FILE | --name NAME [FILE]) [--pretty] [-o OUTPUT]
tsparse (--references-status | --references-stop | --references-serve) --project TSCONFIG

Parse TypeScript into a compiler-style JSON AST. Reads stdin when FILE is omitted.
--check validates syntax without producing JSON.

On Unix, reference commands reuse a local project service, started automatically.
--references-direct loads a fresh project in this process. --references-status
reports service state; --references-stop releases it; --references-serve runs it
in the foreground. Services exit after ten idle minutes. TSPARSE_SERVICE_DIR
selects a private runtime directory; TSPARSE_SERVICE_IDLE_SECS sets idle seconds.

--references groups the declarations and occurrences of the symbol selected by
the identifier at LINE:COLUMN in FILE (one-based line, UTF-16 column), or by
every declaration, local import, or exported alias spelled exactly NAME; FILE
then narrows the candidates. Import and export aliases join their underlying
symbol; shadowed bindings form separate groups. Argument paths are relative to
the working directory; result paths are relative to the directory of the
reported project, the TSCONFIG directory's real path.
Supported: one configuration and its inherited settings; .ts, .tsx, .mts, and
.cts sources and declarations; node16, nodenext, and bundler resolution; and
installed packages with declarations. Project references are not followed:
a configuration that needs them is rejected rather than searched partially.
Standard libraries are not loaded: an explicit lib setting or lib reference is
rejected, and names declared only by them are unresolved. Properties, class
members, and enum members are not reference targets. JavaScript inputs and
JSON modules are not supported.

In reference mode, exit 1 also reports project analysis failures, unsupported
inputs, and unsupported or unresolved selected targets.
Exit codes: 0 success, 1 syntax error, 2 usage or I/O error.";

#[derive(Default)]
struct Options {
    pretty: bool,
    jsx: bool,
    check: bool,
    references: bool,
    files: Vec<String>,
    output: Option<String>,
    project: Option<String>,
    at: Option<String>,
    name: Option<String>,
    service_action: Option<String>,
    references_direct: bool,
}

fn run() -> Result<(), (u8, String)> {
    let mut options = Options::default();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .ok_or_else(|| (2, format!("Missing value for {flag}")))
        };
        match arg.as_str() {
            "-h" | "--help" => return print(&format!("{HELP}\n")),
            "-V" | "--version" => {
                return print(&format!("tsparse {}\n", env!("CARGO_PKG_VERSION")));
            }
            "--pretty" => options.pretty = true,
            "--tsx" => options.jsx = true,
            "--check" => options.check = true,
            "--references" => options.references = true,
            "--references-direct" => {
                options.references = true;
                options.references_direct = true;
            }
            "--references-serve" | "--references-status" | "--references-stop" => {
                if options.service_action.replace(arg).is_some() {
                    return Err((2, "Expected one reference service action".into()));
                }
                options.references = true;
            }
            "-o" | "--output" => options.output = Some(value("-o")?),
            "--project" => options.project = Some(value("--project")?),
            "--at" => options.at = Some(value("--at")?),
            "--name" => options.name = Some(value("--name")?),
            "-" => options.files.push(arg),
            _ if arg.starts_with('-') => return Err((2, format!("Unknown option: {arg}"))),
            _ => options.files.push(arg),
        }
    }
    if options.files.len() > 1 {
        return Err((2, "Expected one input file".into()));
    }
    if options.references {
        references(options)
    } else if options.project.is_some() || options.at.is_some() || options.name.is_some() {
        Err((2, "--project, --at, and --name require --references".into()))
    } else {
        parse(options)
    }
}

fn print(text: &str) -> Result<(), (u8, String)> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|_| stdout.flush())
        .map_err(|e| (2, e.to_string()))
}

fn parse(options: Options) -> Result<(), (u8, String)> {
    let name = options.files.first().map_or("-", String::as_str);
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
            jsx: options.jsx || file.jsx,
            ..file
        },
    )
    .map_err(|e| {
        let (line, column) = e.line_column(&source);
        (1, format!("{name}:{line}:{column}: {}", e.message))
    })?;
    if options.check {
        return Ok(());
    }
    write_output(options.output.as_deref(), |writer| {
        ast.write_json(&mut *writer, options.pretty)
    })
}

fn references(options: Options) -> Result<(), (u8, String)> {
    if options.check || options.jsx {
        return Err((2, "--check and --tsx do not apply to --references".into()));
    }
    let project = options
        .project
        .ok_or((2, "--references requires --project TSCONFIG".to_string()))?;
    if let Some(action) = &options.service_action {
        if options.at.is_some()
            || options.name.is_some()
            || !options.files.is_empty()
            || options.references_direct
        {
            return Err((2, "Service actions do not take a reference query".into()));
        }
        #[cfg(unix)]
        return match action.as_str() {
            "--references-status" => {
                let status = service::status(project.as_ref()).map_err(|e| (2, e.to_string()))?;
                write_output(options.output.as_deref(), |writer| {
                    if options.pretty {
                        serde_json::to_writer_pretty(writer, &status)
                    } else {
                        serde_json::to_writer(writer, &status)
                    }
                    .map_err(io::Error::other)
                })
            }
            "--references-stop" => service::stop(project.as_ref()).map_err(|e| (2, e.to_string())),
            "--references-serve" => {
                service::serve(project.as_ref()).map_err(|e| (2, e.to_string()))
            }
            _ => unreachable!("argument parsing selects the service action"),
        };
        #[cfg(not(unix))]
        return Err((2, format!("{action} is available only on Unix")));
    }
    let file = options.files.into_iter().next();
    if file.as_deref() == Some("-") {
        return Err((
            2,
            "--references reads files from the project, not stdin".into(),
        ));
    }
    let query = match (options.at, options.name) {
        (Some(at), None) => ReferenceQuery::At {
            file: file
                .map(PathBuf::from)
                .ok_or((2, "--at requires a FILE".to_string()))?,
            position: position(&at)?,
        },
        (None, Some(name)) => ReferenceQuery::Name {
            name,
            file: file.map(PathBuf::from),
        },
        _ => {
            return Err((
                2,
                "--references requires exactly one of --at or --name".into(),
            ));
        }
    };
    #[cfg(unix)]
    let result = if !options.references_direct {
        service::query(project.as_ref(), query).map_err(|error| match error {
            service::Error::Reference(error) => reference_error(error),
            service::Error::Transport(error) => (2, error.to_string()),
        })?
    } else {
        direct_references(project.as_ref(), query)?
    };
    #[cfg(not(unix))]
    let result = direct_references(project.as_ref(), query)?;
    write_output(options.output.as_deref(), |writer| {
        if options.pretty {
            serde_json::to_writer_pretty(&mut *writer, &result)
        } else {
            serde_json::to_writer(&mut *writer, &result)
        }
        .map_err(io::Error::other)
    })
}

fn direct_references(
    project: &std::path::Path,
    query: ReferenceQuery,
) -> Result<typescript_parser::references::ReferenceResult, (u8, String)> {
    let project = ReferenceProject::load(project)
        .map_err(|error| reference_error(ReferenceError::Project(error)))?;
    project
        .find_references(query)
        .map_err(|error| reference_error(ReferenceError::Query(error)))
}

/// Exit policy is shared by direct loading and service responses. Transport
/// errors are handled separately; they never become successful empty searches.
fn reference_error(error: ReferenceError) -> (u8, String) {
    let code = match &error {
        ReferenceError::Project(error) => match error.kind {
            ProjectErrorKind::Io => 2,
            ProjectErrorKind::Config
            | ProjectErrorKind::Syntax
            | ProjectErrorKind::Unsupported
            | ProjectErrorKind::Binding
            | ProjectErrorKind::UnresolvedImport => 1,
        },
        ReferenceError::Query(error) => match error.kind {
            QueryErrorKind::OutsideProject | QueryErrorKind::InvalidPosition => 2,
            QueryErrorKind::UnsupportedTarget | QueryErrorKind::UnresolvedTarget => 1,
        },
    };
    (code, error.to_string())
}

fn position(text: &str) -> Result<SourcePosition, (u8, String)> {
    let invalid = || (2, format!("--at expects LINE:COLUMN, got {text}"));
    let (line, column) = text.split_once(':').ok_or_else(invalid)?;
    let number = |part: &str| part.parse::<usize>().ok().filter(|&n| n > 0);
    Ok(SourcePosition {
        line: number(line).ok_or_else(invalid)?,
        column: number(column).ok_or_else(invalid)?,
    })
}

fn write_output(
    output: Option<&str>,
    write: impl FnOnce(&mut BufWriter<Box<dyn Write>>) -> io::Result<()>,
) -> Result<(), (u8, String)> {
    let writer: Box<dyn Write> = match output {
        Some(path) => Box::new(fs::File::create(path).map_err(|e| (2, format!("{path}: {e}")))?),
        None => Box::new(io::stdout().lock()),
    };
    let mut writer = BufWriter::new(writer);
    write(&mut writer).map_err(|e| (2, e.to_string()))?;
    writeln!(writer)
        .and_then(|_| writer.flush())
        .map_err(|e| (2, e.to_string()))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}
