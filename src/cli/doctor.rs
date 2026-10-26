//! The doctor (`--doctor`): a read-only report of what project queries
//! depend on, one line per check, in the design's order (design, step 5):
//! the build, the environment, and the runtime directory, and with
//! `--project` the configuration, its service, and the limits that service
//! inherited. It decides nothing itself: each check asks the owner of its
//! answer (`super::version`, `process_limits::Environment`, and the
//! service's endpoint, identity, and status) and reports a failure that
//! owner gives as a finding, with its remedy. It never loads a project,
//! starts a service, or creates a file, and a check it cannot perform is a
//! finding (design, "Failure contract").

use super::Failure;
use super::command_line;
use super::delivery::{self, Delivered, Delivery, FINDINGS, Route};
use super::registry::{FlagId, OperationId};
use super::request::Doctor;
use crate::process_limits::{self, Environment, Limits, Setting};
use crate::service::{self, Status};
use serde_json::{Value, json};
use std::io;
use std::path::{Path, PathBuf};

/// The command that stops the project's service, whose replacement reads
/// the environment of the request that starts it.
fn stop_command(config: &str) -> Vec<String> {
    vec![
        "tsparse".into(),
        FlagId::ReferencesStop.to_string(),
        FlagId::Project.to_string(),
        config.into(),
    ]
}

/// One check: what it found, or what it confirmed.
struct Check {
    name: &'static str,
    outcome: Result<String, Finding>,
}

/// What a check found: its detail, and where one exists, why the
/// constraint holds and the remedy, a command or a sentence.
struct Finding {
    detail: String,
    why: Option<&'static str>,
    next: Option<Vec<String>>,
    remedy: Option<String>,
}

impl Finding {
    fn new(detail: String) -> Self {
        Self {
            detail,
            why: None,
            next: None,
            remedy: None,
        }
    }
}

/// A failure an owner reports, with its remedy, as a finding.
impl From<&Failure> for Finding {
    fn from(failure: &Failure) -> Self {
        Self {
            detail: failure.message().to_string(),
            why: failure.why(),
            next: failure.next().map(<[String]>::to_vec),
            remedy: failure.remedy().map(str::to_string),
        }
    }
}

/// Print the report `request` asks for: text, or with `--json` one
/// document, on stdout, exiting with `FINDINGS` when a check found
/// something.
pub(crate) fn run(request: &Doctor) -> Result<(), Delivered> {
    let checks = checks(request.project.as_deref());
    let found: Vec<&str> = checks
        .iter()
        .filter(|check| check.outcome.is_err())
        .map(|check| check.name)
        .collect();
    let route = Route {
        format: request.format,
        pretty: request.pretty,
        output: None,
        new_file: false,
        operation: Some(OperationId::Doctor.operation().name),
        fields: &[],
    };
    let code = if found.is_empty() { 0 } else { FINDINGS };
    delivery::deliver(
        route,
        Ok(Delivery {
            code,
            document: Box::new(|writer, pretty| {
                delivery::write_document(
                    writer,
                    &document(&checks, request.project.as_deref(), &found),
                    pretty,
                )
            }),
            text: Some(Box::new(|| text(&checks, &found))),
            text_is_result: true,
        }),
    )
}

/// Every check, in order.
fn checks(project: Option<&str>) -> Vec<Check> {
    let mut checks = vec![
        build(),
        environment(),
        Check {
            name: "runtime directory",
            outcome: service::inspect_directory(project).map_err(|failure| (&failure).into()),
        },
    ];
    if let Some(config) = project {
        checks.push(configuration(config));
        let status = service::observe(config);
        checks.push(service_check(config, &status));
        checks.push(service_limits(config, &status));
    }
    checks
}

/// The build stamp and what this build can do on its platform: the
/// project service and applying a plan, which every build has, as the
/// crate builds only on macOS and Linux (ENVELOPE.md); and memory
/// monitoring, which every query's watchdog needs, asked of
/// `process_limits` by sampling this process.
fn build() -> Check {
    build_with(process_limits::memory_footprint())
}

/// The build check, given what sampling this process's memory gave.
fn build_with(memory: io::Result<u64>) -> Check {
    let detail = format!(
        "{} on {} {}: project service yes, {} yes",
        super::version(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        FlagId::Apply,
    );
    Check {
        name: "build",
        outcome: match memory {
            Ok(_) => Ok(format!("{detail}, memory monitoring yes")),
            Err(error) => Err(Finding {
                why: Some("every read-only query samples its memory and fails without it"),
                ..Finding::new(format!("{detail}, memory monitoring no: {error}"))
            }),
        },
    }
}

/// Each variable as the environment's owner reads it: every invalid
/// value, then the value or default of each valid one.
fn environment() -> Check {
    let mut invalid = Vec::new();
    let mut valid = Vec::new();
    for setting in Environment::settings() {
        match setting {
            Setting::Valid {
                name,
                value,
                default: true,
            } => valid.push(format!("{name}={value} (default)")),
            Setting::Valid {
                name,
                value,
                default: false,
            } => valid.push(format!("{name}={value}")),
            Setting::Invalid(message) => invalid.push(message),
        }
    }
    let valid = valid.join(", ");
    Check {
        name: "environment",
        outcome: if invalid.is_empty() {
            Ok(valid)
        } else {
            invalid.push(valid);
            Err(Finding::new(invalid.join("; ")))
        },
    }
}

/// The configuration as given, its absolute form, which keys its service,
/// and whether it can be read, which reading it without waiting answers;
/// nothing parses it.
fn configuration(config: &str) -> Check {
    let absolute = match absolute(config) {
        Ok(absolute) => absolute,
        Err(failure) => {
            return Check {
                name: "configuration",
                outcome: Err((&failure).into()),
            };
        }
    };
    Check {
        name: "configuration",
        outcome: match readable(Path::new(config)) {
            Ok(()) => Ok(format!(
                "{config} is {}, a readable file; another spelling of the same file would own a separate service",
                absolute.display()
            )),
            Err(error) => Err(Finding::new(format!(
                "{config} ({}) cannot be read: {error}",
                absolute.display()
            ))),
        },
    }
}

/// The configuration's absolute spelling, as its service is keyed.
fn absolute(config: &str) -> Result<PathBuf, Failure> {
    service::absolute(config)
}

/// Read `path` to its end when it names a regular file, following symbolic
/// links as loading does. Another kind of file, such as a pipe an open would
/// wait on, is refused before it is opened (ENVELOPE.md, "Accepted inputs");
/// a path replaced by a pipe between the check and the open can block, which
/// ENVELOPE.md accepts.
fn readable(path: &Path) -> io::Result<()> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    io::copy(&mut std::fs::File::open(path)?, &mut io::sink())?;
    Ok(())
}

/// Whether the project's service runs, and if so what it reports of
/// itself, whose build must be this one; and if not, what an earlier start
/// left that the next start removes (`service::leftovers`).
fn service_check(config: &str, status: &Result<Status, Failure>) -> Check {
    let outcome = match status {
        Err(failure) => Err(failure.into()),
        Ok(Status::Absent) => match service::leftovers(config) {
            Ok(left) => Ok(std::iter::once(format!(
                "no service is running for {config}; the next project query starts one"
            ))
            .chain(left)
            .collect::<Vec<_>>()
            .join("; ")),
            Err(failure) => Err((&failure).into()),
        },
        Ok(Status::Running(running)) => {
            let now = service::unix_seconds(std::time::SystemTime::now());
            // A clock set back since the service started reads as no time.
            let started = (now - running.started_unix).max(0) as u64;
            let detail = format!(
                "running: {}; started {} ago",
                running.summary(),
                service::seconds(started)
            );
            let build = env!("TSPARSE_SERVICE_BUILD");
            if running.build == build {
                Ok(detail)
            } else {
                Err(Finding {
                    next: Some(stop_command(config)),
                    ..Finding::new(format!("{detail}; this build is {build}"))
                })
            }
        }
    };
    Check {
        name: "service",
        outcome,
    }
}

/// `limits` as a report names them: each variable and its value.
fn named(limits: &Limits) -> String {
    let [(memory, megabytes), (timeout, milliseconds)] = limits.named();
    format!("{memory} {megabytes} and {timeout} {milliseconds}")
}

/// The limits the running service inherited beside this environment's: a
/// service keeps the environment it started with, so a difference is a
/// finding whose remedy is to stop it.
fn service_limits(config: &str, status: &Result<Status, Failure>) -> Check {
    let finding = |detail: String| Check {
        name: "service limits",
        outcome: Err(Finding::new(detail)),
    };
    let current = match Environment::read() {
        Ok(environment) => environment.limits(),
        Err(failure) => {
            return finding(format!(
                "this environment's limits cannot be read: {}",
                failure.message()
            ));
        }
    };
    let running = match status {
        Err(_) => {
            return finding("the service's limits cannot be read without its status".into());
        }
        Ok(Status::Absent) => {
            return Check {
                name: "service limits",
                outcome: Ok(format!(
                    "no service is running; one started from this environment takes {}",
                    named(&current)
                )),
            };
        }
        Ok(Status::Running(running)) => running,
    };
    let differences: Vec<String> = running
        .limits
        .named()
        .into_iter()
        .zip(current.named())
        .filter(|((_, inherited), (_, now))| inherited != now)
        .map(|((name, inherited), (_, now))| {
            format!("{name} {inherited} inherited at start, {now} in this environment")
        })
        .collect();
    Check {
        name: "service limits",
        outcome: if differences.is_empty() {
            Ok(format!("{}, as in this environment", named(&current)))
        } else {
            Err(Finding {
                why: Some("a service keeps the environment it started with"),
                next: Some(stop_command(config)),
                ..Finding::new(differences.join("; "))
            })
        },
    }
}

/// The report as text, without the final newline delivery adds: one line
/// per check, `CHECK: detail`, a finding's `why` and `next` lines in a
/// failure's order, and the verdict.
fn text(checks: &[Check], found: &[&str]) -> String {
    let mut lines = Vec::new();
    for check in checks {
        match &check.outcome {
            Ok(detail) => lines.push(format!("{}: {detail}", check.name)),
            Err(finding) => {
                lines.push(format!("{}: {}", check.name, finding.detail));
                if let Some(why) = finding.why {
                    lines.push(format!("  why: {why}"));
                }
                if let Some(next) = &finding.next {
                    lines.push(format!("  next: {}", command_line::render(next)));
                }
                if let Some(remedy) = &finding.remedy {
                    lines.push(format!("  next: {remedy}"));
                }
            }
        }
    }
    lines.push(if found.is_empty() {
        format!("ok: {} checks pass", checks.len())
    } else {
        format!("findings: {}", found.join(", "))
    });
    lines.join("\n")
}

/// The report as one document, with an entry per check: its name, status,
/// detail, and a finding's `why`, `next.argv`, and `remedy`.
fn document(checks: &[Check], project: Option<&str>, found: &[&str]) -> Value {
    let entries: Vec<Value> = checks
        .iter()
        .map(|check| match &check.outcome {
            Ok(detail) => json!({"name": check.name, "status": "pass", "detail": detail}),
            Err(finding) => {
                let mut entry =
                    json!({"name": check.name, "status": "finding", "detail": finding.detail});
                if let Some(why) = finding.why {
                    entry["why"] = json!(why);
                }
                if let Some(next) = &finding.next {
                    entry["next"] = json!({ "argv": next });
                }
                if let Some(remedy) = &finding.remedy {
                    entry["remedy"] = json!(remedy);
                }
                entry
            }
        })
        .collect();
    let mut document = json!({
        "operation": OperationId::Doctor.operation().name,
        "outcome": if found.is_empty() { "pass" } else { "findings" },
        "checks": entries,
    });
    if let Some(config) = project {
        document["project"] = json!(config);
    }
    document
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::service::Running;

    fn running(build: &str) -> Status {
        Status::Running(Running {
            pid: 7,
            generation: 2,
            build: build.into(),
            limits: Environment::read().unwrap().limits(),
            idle_secs: 600,
            idle_remaining_secs: 432,
            started_unix: service::unix_seconds(std::time::SystemTime::now()),
        })
    }

    /// A service reached through its endpoint is this build's, since its
    /// key and its identity check include the build; a report of another
    /// is a finding whose remedy stops it.
    #[test]
    fn a_service_of_another_build_is_a_finding() {
        let build = env!("TSPARSE_SERVICE_BUILD");
        let same = service_check("tsconfig.json", &Ok(running(build)));
        assert_eq!(
            same.outcome.ok(),
            Some(format!(
                "running: pid 7, generation 2, build {build}, idle 10m, 7m12s remaining; started 0s ago"
            ))
        );
        let Err(other) = service_check("tsconfig.json", &Ok(running("other"))).outcome else {
            panic!("another build passes");
        };
        assert_eq!(
            other.detail,
            format!(
                "running: pid 7, generation 2, build other, idle 10m, 7m12s remaining; started 0s ago; this build is {build}"
            )
        );
        assert_eq!(
            other.next.as_deref().map(command_line::render),
            Some("tsparse --references-stop --project tsconfig.json".into())
        );
    }

    /// Every query samples its memory, so a sampler that fails is a finding.
    #[test]
    fn memory_monitoring_that_fails_is_a_finding() {
        assert!(build_with(Ok(1)).outcome.is_ok());
        let Err(finding) = build_with(Err(io::Error::other("no sampler"))).outcome else {
            panic!("a failed sampler passes");
        };
        assert!(
            finding
                .detail
                .ends_with(", memory monitoring no: no sampler"),
            "{}",
            finding.detail
        );
        assert_eq!(
            finding.why,
            Some("every read-only query samples its memory and fails without it")
        );
    }
}
