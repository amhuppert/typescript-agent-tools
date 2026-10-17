//! Answering a query: file navigation in this process, or a project request
//! through the project's service or a project loaded here. Both project
//! entries answer through `ProjectSession::execute`, the one boundary that
//! refreshes a generation before any operation; the service keeps that
//! session between requests. Every answer, and a transport failure that
//! leaves none, goes to the one delivery policy.

use super::Exit;
use super::delivery::{self, Base, Delivery};
use super::navigation;
use super::request::{FileOperation, Query, Target};
use std::io;
use std::path::Path;
use typescript_parser::analysis::AnalysisError;
use typescript_parser::navigation::{outline_file, show_file};
use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};

/// An answer, or the transport failure that left none.
pub(crate) type Answer = Result<Result<ProjectResponse, AnalysisError>, io::Error>;

pub(crate) fn run(query: Query) -> Result<(), Exit> {
    // The service protects its own process. Direct/file reads protect this one;
    // edit application has a separate entry and must never be killed mid-write.
    if cfg!(unix) && matches!(&query.target, Target::Project { direct: false, .. }) {
        return run_here(&query);
    }
    let answer = crate::process_limits::run(
        |failure| {
            let answer = Err(failure.into_error());
            let outcome = delivery::deliver(
                query.format,
                query.pretty,
                query.output.as_deref(),
                Ok(delivery(&query, &answer)),
            );
            match outcome {
                Ok(()) => 1,
                Err((code, message)) => {
                    if !message.is_empty() {
                        eprintln!("{message}");
                    }
                    code.into()
                }
            }
        },
        || answer(&query.target),
    )
    .map_err(|error| (2, error.to_string()))?;
    deliver_answer(&query, &answer)
}

fn run_here(query: &Query) -> Result<(), Exit> {
    let answer = answer(&query.target);
    deliver_answer(query, &answer)
}

fn deliver_answer(query: &Query, answer: &Answer) -> Result<(), Exit> {
    let outcome = match answer {
        Ok(answer) => Ok(delivery(query, answer)),
        Err(transport) => Err(transport),
    };
    delivery::deliver(query.format, query.pretty, query.output.as_deref(), outcome)
}

fn answer(target: &Target) -> Answer {
    match target {
        Target::File { file, operation } => Ok(match operation {
            FileOperation::Outline => outline_file(file).map(ProjectResponse::Outline),
            FileOperation::Show(selector) => {
                show_file(file, selector).map(|show| ProjectResponse::Show(Box::new(show)))
            }
        }),
        Target::Project {
            config,
            request,
            direct,
        } => project(Path::new(config), request.clone(), *direct),
    }
}

/// A project request through the service, unless `direct`; platforms
/// without the service always answer directly. Either way, the request's
/// file operand is made absolute here, and diagnostics that name it get
/// the caller's spelling back (`ProjectRequest::make_absolute`), so the
/// two entries answer alike from any working directory.
pub(crate) fn project(config: &Path, mut request: ProjectRequest, direct: bool) -> Answer {
    let spelled = match request.make_absolute() {
        Ok(spelled) => spelled,
        Err(error) => return Ok(Err(error)),
    };
    let answer = if direct {
        // A fresh session's first request loads the project, as the
        // service's does, through the same refresh and validation.
        Ok(one_shot(config, request))
    } else {
        service(config, request)
    };
    answer.map(|result| {
        result.map_err(|mut error| {
            spelled.restore(&mut error);
            error
        })
    })
}

/// Answer this process's only request. The process exits after delivering
/// it, and the session holds no external resource, so its memory returns
/// with the process instead of being freed piece by piece, which takes
/// seconds for a large project.
fn one_shot(config: &Path, request: ProjectRequest) -> Result<ProjectResponse, AnalysisError> {
    let session = ProjectSession::for_one_request(config).map_err(AnalysisError::from)?;
    std::mem::ManuallyDrop::new(session).execute(request)
}

#[cfg(unix)]
fn service(config: &Path, request: ProjectRequest) -> Answer {
    match crate::service::execute(config, request) {
        Ok(response) => Ok(Ok(response)),
        Err(crate::service::Error::Analysis(error)) => Ok(Err(error)),
        Err(crate::service::Error::Transport(error)) => Err(error),
    }
}

/// Platforms without the service answer directly.
#[cfg(not(unix))]
fn service(config: &Path, request: ProjectRequest) -> Answer {
    Ok(one_shot(config, request))
}

/// The forms of one answer.
fn delivery<'a>(
    query: &'a Query,
    answer: &'a Result<ProjectResponse, AnalysisError>,
) -> Delivery<'a> {
    match answer {
        Ok(ProjectResponse::References(result)) => Delivery {
            code: 0,
            text_is_result: true,
            document: Box::new(move |writer, pretty| {
                if pretty {
                    serde_json::to_writer_pretty(writer, result)
                } else {
                    serde_json::to_writer(writer, result)
                }
                .map_err(io::Error::other)
            }),
            text: None,
        },
        Ok(ProjectResponse::ReferenceSearch(answer)) => Delivery {
            code: match answer.as_ref() {
                typescript_parser::references::ReferenceAnswer::Selected(result)
                    if result.outcome
                        == typescript_parser::references::ReferenceOutcome::Complete =>
                {
                    0
                }
                _ => 1,
            },
            text_is_result: true,
            document: Box::new(move |writer, pretty| {
                if pretty {
                    serde_json::to_writer_pretty(writer, answer)
                } else {
                    serde_json::to_writer(writer, answer)
                }
                .map_err(io::Error::other)
            }),
            text: None,
        },
        Ok(ProjectResponse::Outline(outline)) => navigation::outline(outline, query.depth),
        Ok(ProjectResponse::Show(show)) => navigation::show(show),
        Ok(ProjectResponse::Type(answer)) => super::types::answer(answer),
        Ok(ProjectResponse::Edit(_)) => unreachable!("edit requests are delivered by cli::editing"),
        Err(error) => {
            let base = match &query.target {
                Target::File { file, .. } => Base::File(file),
                Target::Project { config, .. } => Base::Project(config),
            };
            delivery::failure(query.target.operation(), base, error)
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::cli::delivery::Format;
    use crate::cli::navigation::Depth;
    use serde_json::{Value, json};
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    use typescript_parser::navigation::Selector;

    /// A path that is not Unicode: navigation reports it as unsupported
    /// input without reading it.
    fn not_unicode() -> PathBuf {
        PathBuf::from(std::ffi::OsString::from_vec(
            b"/navigation-\xff/x.ts".to_vec(),
        ))
    }

    fn query(operation: FileOperation, format: Format, output: Option<String>) -> Query {
        Query {
            target: Target::File {
                file: not_unicode(),
                operation,
            },
            format,
            pretty: false,
            depth: Depth::default(),
            output,
        }
    }

    #[test]
    fn a_path_that_is_not_unicode_is_a_typed_failure_in_both_formats() {
        let result = run(query(FileOperation::Outline, Format::Text, None));
        assert_eq!(
            result,
            Err((
                1,
                "/navigation-\u{FFFD}/x.ts: the file name is not Unicode, so it cannot be named exactly"
                    .into()
            ))
        );

        let directory = std::env::temp_dir().join(format!(
            "tsparse-navigation-delivery-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let output = directory.join("error.json");
        let result = run(query(
            FileOperation::Show(Selector::Name("x".into())),
            Format::Json,
            Some(output.to_str().unwrap().into()),
        ));
        assert_eq!(result, Err((1, String::new())));
        let document: Value =
            serde_json::from_str(&std::fs::read_to_string(&output).unwrap()).unwrap();
        std::fs::remove_dir_all(&directory).unwrap();
        // Identity fields hold exact paths only; the lossy form is a message.
        assert_eq!(
            document,
            json!({
                "operation": "show",
                "outcome": "error",
                "file": null,
                "error": {
                    "kind": "Unsupported",
                    "message": "/navigation-\u{FFFD}/x.ts: the file name is not Unicode, so it cannot be named exactly",
                    "path": null,
                    "position": null,
                },
            })
        );
    }
}
