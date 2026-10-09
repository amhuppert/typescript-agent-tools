//! Answer JSON-lines reference queries against one loaded project.
//!
//! ```sh
//! cargo run --offline --example reference_batch -- --project path/to/tsconfig.json
//! ```
//!
//! Each stdin line is `{"at":{"file":"…","line":L,"column":C}}` or
//! `{"name":"…","file":null|"…"}`; relative paths resolve against the working
//! directory. Each query prints one line: `{"result": ReferenceResult}`,
//! `{"error": {"kind": "<QueryErrorKind>", "message": "…", "path": …,
//! "position": …}}`, whose kind is an `InputErrorKind` instead for a query
//! path that names no file exactly, or `{"panic": "…"}` when the
//! implementation panics. A project that fails to load prints
//! `{"projectError": {...}}` with an `InputErrorKind` and exits 1. Kinds are
//! the variant names. Usage errors and malformed query lines exit 2.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use typescript_parser::analysis::{AnalysisError, InputErrorKind};
use typescript_parser::project::Project;
use typescript_parser::references::{ReferenceQuery, ReferenceResult};
use typescript_parser::source::SourcePosition;

#[derive(Deserialize)]
#[serde(untagged)]
enum Query {
    At { at: At },
    Name { name: String, file: Option<PathBuf> },
}

#[derive(Deserialize)]
struct At {
    file: PathBuf,
    line: usize,
    column: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum Response<'a> {
    Result(&'a ReferenceResult),
    Error {
        kind: serde_json::Value,
        message: String,
        path: Option<String>,
        position: Option<SourcePosition>,
    },
    Panic(String),
    ProjectError {
        kind: InputErrorKind,
        message: String,
        path: Option<String>,
        position: Option<SourcePosition>,
    },
}

fn display(path: Option<&Path>) -> Option<String> {
    path.map(|path| path.display().to_string())
}

/// The message and location of the most recent panic, captured quietly so a
/// failing query does not flood stderr during a large comparison.
static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match args.as_slice() {
        [flag, config] if flag == "--project" => PathBuf::from(config),
        _ => {
            eprintln!("usage: reference_batch --project path/to/tsconfig.json < queries.jsonl");
            return ExitCode::from(2);
        }
    };
    match run(&config) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("reference_batch: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(config: &std::path::Path) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let project = match Project::load(config) {
        Ok(project) => project,
        Err(error) => {
            let response = Response::ProjectError {
                kind: error.kind,
                message: error.to_string(),
                path: display(error.path.as_deref()),
                position: error.position,
            };
            write_line(&mut out, &response)?;
            return Ok(ExitCode::from(1));
        }
    };
    let directory = std::env::current_dir()?;
    std::panic::set_hook(Box::new(|info| {
        *LAST_PANIC
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(info.to_string());
    }));
    for (index, line) in std::io::stdin().lock().lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let query = match serde_json::from_str::<Query>(&line) {
            Ok(Query::At { at }) => ReferenceQuery::At {
                file: directory.join(at.file),
                position: SourcePosition {
                    line: at.line,
                    column: at.column,
                },
            },
            Ok(Query::Name { name, file }) => ReferenceQuery::Name {
                name,
                file: file.map(|file| directory.join(file)),
            },
            Err(error) => {
                eprintln!("reference_batch: query line {}: {error}", index + 1);
                return Ok(ExitCode::from(2));
            }
        };
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| project.find_references(query)));
        match outcome {
            Ok(Ok(result)) => write_line(&mut out, &Response::Result(&result))?,
            Ok(Err(error)) => {
                let (kind, path, position) = match &error {
                    AnalysisError::Query(error) => {
                        (serde_json::json!(error.kind), &error.path, error.position)
                    }
                    AnalysisError::Input(error) => {
                        (serde_json::json!(error.kind), &error.path, error.position)
                    }
                };
                write_line(
                    &mut out,
                    &Response::Error {
                        kind,
                        message: error.to_string(),
                        path: display(path.as_deref()),
                        position,
                    },
                )?
            }
            Err(_) => {
                let message = LAST_PANIC
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .take()
                    .unwrap_or_else(|| "panic without a message".into());
                write_line(&mut out, &Response::Panic(message))?;
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn write_line(out: &mut impl Write, response: &Response<'_>) -> std::io::Result<()> {
    serde_json::to_writer(&mut *out, response)?;
    out.write_all(b"\n")?;
    out.flush()
}
