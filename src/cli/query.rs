//! Answering a query: file navigation in this process, or a project request
//! through the project's service or a project loaded here. Both project
//! entries answer through `ProjectSession::execute`, the one boundary that
//! refreshes a generation before any operation; the service keeps that
//! session between requests. Every answer, and a failure that leaves none,
//! goes to the one delivery policy.

use super::Failure;
use super::delivery::{self, Base, Delivered, Delivery, Route};
use super::failure::Code;
use super::navigation;
use super::request::{FileOperation, Query, Target};
use crate::process_limits::{Context, Environment};
use std::path::Path;
use typescript_parser::analysis::AnalysisError;
use typescript_parser::navigation::{outline_file, show_file};
use typescript_parser::project::{ProjectRequest, ProjectResponse, ProjectSession};

/// An answer, or the failure that left none.
pub(crate) type Answer = Result<Result<ProjectResponse, AnalysisError>, Failure>;

pub(crate) fn run(query: Query) -> Result<(), Delivered> {
    // Every query reads the environment before anything else, whichever
    // entry answers it, so an invalid value fails alike in both.
    let environment = match Environment::read() {
        Ok(environment) => environment,
        Err(failure) => return Err(delivery::fail(query.route(), failure)),
    };
    // The service protects its own process. Direct/file reads protect this one;
    // edit application has a separate entry and must never be killed mid-write.
    if matches!(&query.target, Target::Project { direct: false, .. }) {
        return deliver_answer(&query, answer(&query.target));
    }
    let answer = crate::process_limits::run(
        &environment,
        |failure| {
            let error = failure.into_error(&environment.limits(), Context::Direct);
            // The process exits as its query's failure does.
            let code = delivery::failure_code(&error);
            let answer = Err(error);
            match delivery::deliver(query.route(), Ok(delivery(&query, &answer))) {
                Ok(()) => code.into(),
                Err(delivered) => delivered.code().into(),
            }
        },
        || answer(&query.target),
    );
    match answer {
        Ok(answer) => deliver_answer(&query, answer),
        Err(error) => Err(delivery::fail(
            query.route(),
            Failure::new(Code::Watchdog, error.to_string()),
        )),
    }
}

fn deliver_answer(query: &Query, answer: Answer) -> Result<(), Delivered> {
    match answer {
        Ok(answer) => delivery::deliver(query.route(), Ok(delivery(query, &answer))),
        Err(failure) => delivery::deliver(query.route(), Err(failure)),
    }
}

impl Query {
    /// Where this query's outcomes go: its documents name its operation.
    fn route(&self) -> Route<'_> {
        Route {
            format: self.format,
            pretty: self.pretty,
            output: self.output.as_deref(),
            new_file: false,
            operation: Some(self.target.operation()),
            fields: &[],
        }
    }
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
        } => project(config, request.clone(), *direct),
    }
}

/// A project request through the service, unless `direct`. Either way,
/// the request's file operand is made absolute here, and diagnostics that
/// name it get the caller's spelling back (`ProjectRequest::make_absolute`),
/// so the two entries answer alike from any working directory.
pub(crate) fn project(config: &str, mut request: ProjectRequest, direct: bool) -> Answer {
    let spelled = match request.make_absolute() {
        Ok(spelled) => spelled,
        Err(error) => return Ok(Err(error)),
    };
    let answer = if direct {
        here(config, request)
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

/// A project request answered in this process. It reads the environment the
/// service would have read (`Endpoint::start`), so a value the service path
/// refuses fails here too. A fresh session's first request loads the
/// project, as the service's does, through the same refresh and validation.
fn here(config: &str, request: ProjectRequest) -> Answer {
    Environment::read()?;
    Ok(one_shot(Path::new(config), request))
}

/// Answer this process's only request. The process exits after delivering
/// it, and the session holds no external resource, so its memory returns
/// with the process instead of being freed piece by piece, which takes
/// seconds for a large project.
fn one_shot(config: &Path, request: ProjectRequest) -> Result<ProjectResponse, AnalysisError> {
    let session = ProjectSession::for_one_request(config).map_err(AnalysisError::from)?;
    std::mem::ManuallyDrop::new(session).execute(request)
}

fn service(config: &str, request: ProjectRequest) -> Answer {
    match crate::service::execute(config, request) {
        Ok(response) => Ok(Ok(response)),
        Err(crate::service::Error::Analysis(error)) => Ok(Err(error)),
        Err(crate::service::Error::Unanswered(failure)) => Err(failure),
    }
}

/// The forms of one answer.
fn delivery<'a>(
    query: &'a Query,
    answer: &'a Result<ProjectResponse, AnalysisError>,
) -> Delivery<'a> {
    match answer {
        Ok(ProjectResponse::References(result)) => {
            super::references::result(result, query.pretty, &query.display)
        }
        Ok(ProjectResponse::ReferenceSearch(answer)) => {
            super::references::answer(answer, query.pretty, &query.display)
        }
        Ok(ProjectResponse::Outline(outline)) => {
            navigation::outline(outline, query.depth, &query.display)
        }
        Ok(ProjectResponse::Show(show)) => navigation::show(show, &query.display),
        Ok(ProjectResponse::Type(answer)) => super::types::answer(answer, &query.display),
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
    use crate::cli::delivery::{Display, Format, Limit};
    use crate::cli::navigation::Depth;
    use crate::cli::registry::OperationId;
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
            display: Display::new(
                OperationId::Show.operation(),
                Limit::Default,
                output.is_some(),
                vec!["tsparse".into()],
            ),
            output,
        }
    }

    #[test]
    fn a_path_that_is_not_unicode_is_a_typed_failure_in_both_formats() {
        let outline = query(FileOperation::Outline, Format::Text, None);
        {
            // The diagnostic text mode writes to stderr.
            let Ok(answer) = answer(&outline.target) else {
                panic!("file navigation always answers");
            };
            let failure = delivery(&outline, &answer);
            assert!(!failure.text_is_result);
            assert_eq!(
                failure.text.unwrap()(),
                "/navigation-\u{FFFD}/x.ts: the file name is not Unicode, so it cannot be named exactly"
            );
        }
        let result = run(outline);
        assert_eq!(result.map_err(|delivered| delivered.code()), Err(1));

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
        assert_eq!(result.map_err(|delivered| delivered.code()), Err(1));
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
