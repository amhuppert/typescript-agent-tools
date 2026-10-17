//! The one delivery policy: how every outcome of a query reaches stdout,
//! stderr, the `-o` file, and the exit status, whether file navigation, a
//! project loaded in this process, or the project's service answered it.
//!
//! | Format | Result (complete exit 0, partial exit 1) | Selection or analysis failure (exit 1 or 2) |
//! | --- | --- | --- |
//! | text (navigation's default; references) | text, or a reference result document, to stdout or `-o` | diagnostic to stderr |
//! | JSON (navigation's `--json`) | document to stdout or `-o` | document to stdout or `-o` |
//!
//! Ambiguous and missing selections exit 1; an analysis failure exits with
//! the code of its kind (`failure_code`). Editing adds rejected edits
//! (`REJECTED`), plans that cannot be loaded (`load_failure_code`), and the
//! effects of applying (`applied_code`); its text results and diagnostics
//! and its documents follow the same table. A document is never accompanied by
//! stderr output, and a diagnostic never creates or replaces the `-o` file.
//! A failure that leaves no outcome to deliver is an exit-2 diagnostic on
//! stderr in every format: a usage error before a valid request, a service
//! transport failure, and a failure to write the result, which is never
//! followed by a second document.

use super::Exit;
use serde_json::{Value, json};
use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use typescript_parser::analysis::{
    AnalysisError, InputErrorKind, QueryErrorKind, SemanticErrorKind,
};
use typescript_parser::editing::{Applied, ApplyFailureKind, Effect, LoadErrorKind};

/// How a request asks for its outcomes, chosen at the request boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    /// Results in their own form; failures as stderr diagnostics.
    Text,
    /// Every outcome of the request as one JSON document.
    Json,
}

/// Write an outcome as one JSON document, indented when `pretty`.
pub(crate) type WriteDocument<'a> = Box<dyn FnOnce(&mut dyn Write, bool) -> io::Result<()> + 'a>;

/// One outcome of a valid request, in each form the policy may write. Only
/// the form the policy selects is rendered, so a form that cannot exist for
/// this outcome, such as JSON naming a path that is not Unicode, is never
/// attempted.
pub(crate) struct Delivery<'a> {
    /// The outcome's exit code, including 1 for a partial type result.
    pub(crate) code: u8,
    pub(crate) document: WriteDocument<'a>,
    /// The outcome as text: a navigation result, or a failure's diagnostic.
    /// `None` for a reference result, whose one form is its document.
    pub(crate) text: Option<Box<dyn FnOnce() -> String + 'a>>,
    /// A result belongs to stdout/output even when partial (exit 1). A
    /// diagnostic belongs to stderr. Exit status alone cannot decide this.
    pub(crate) text_is_result: bool,
}

/// Deliver `outcome`, the outcome of a valid request, or report the
/// transport failure that left none. `pretty` indents documents.
pub(crate) fn deliver(
    format: Format,
    pretty: bool,
    output: Option<&str>,
    outcome: Result<Delivery, &io::Error>,
) -> Result<(), Exit> {
    try_deliver(format, pretty, output, false, outcome).map_err(|undelivered| match undelivered {
        Undelivered::Outcome(exit) | Undelivered::Write(exit) => exit,
    })
}

/// Why delivery did not end with exit 0.
pub(crate) enum Undelivered {
    /// The outcome was delivered and exits non-zero: its diagnostic for
    /// stderr, or an empty message after its document was written.
    Outcome(Exit),
    /// The outcome's form could not be written to stdout or `-o`.
    Write(Exit),
}

/// `deliver`, telling a delivered non-zero outcome from a failed write, for
/// an outcome whose effect the caller must still report when its form
/// cannot be written. With `new_file`, `-o` is created and never replaces
/// a file, as an editing command requires.
pub(crate) fn try_deliver(
    format: Format,
    pretty: bool,
    output: Option<&str>,
    new_file: bool,
    outcome: Result<Delivery, &io::Error>,
) -> Result<(), Undelivered> {
    let delivery = outcome.map_err(|transport| Undelivered::Outcome((2, transport.to_string())))?;
    let text = match format {
        Format::Text => delivery.text,
        Format::Json => None,
    };
    match (text, delivery.text_is_result) {
        (Some(text), true) => {
            let text = text();
            write_to(output, new_file, |writer| writer.write_all(text.as_bytes()))
                .map_err(Undelivered::Write)?;
            match delivery.code {
                0 => Ok(()),
                code => Err(Undelivered::Outcome((code, String::new()))),
            }
        }
        (Some(text), false) => Err(Undelivered::Outcome((delivery.code, text()))),
        (None, _) => {
            write_to(output, new_file, |writer| {
                (delivery.document)(writer, pretty)
            })
            .map_err(Undelivered::Write)?;
            match delivery.code {
                0 => Ok(()),
                code => Err(Undelivered::Outcome((code, String::new()))),
            }
        }
    }
}

/// The exit code of each analysis failure kind, the one owner of that
/// decision for every operation (references, outline, show) and entry
/// point (file, direct, and service): 1 when analysis found an input or
/// target invalid or unsupported, and 2, as for usage errors, when an input
/// could not be read or the request names a file or position the analysis
/// does not cover.
pub(crate) fn failure_code(error: &AnalysisError) -> u8 {
    match error {
        AnalysisError::Input(error) => match error.kind {
            InputErrorKind::Io => 2,
            InputErrorKind::Config
            | InputErrorKind::Syntax
            | InputErrorKind::ResourceLimit
            | InputErrorKind::Unsupported
            | InputErrorKind::Binding
            | InputErrorKind::UnresolvedImport => 1,
        },
        AnalysisError::Query(error) => match error.kind {
            QueryErrorKind::OutsideProject
            | QueryErrorKind::InvalidPosition
            | QueryErrorKind::InvalidRequest => 2,
            QueryErrorKind::UnsupportedTarget | QueryErrorKind::UnresolvedTarget => 1,
        },
        AnalysisError::Semantic(error) => match error.kind {
            SemanticErrorKind::Internal => 2,
            SemanticErrorKind::UnsupportedSemantic
            | SemanticErrorKind::InvalidSemantic
            | SemanticErrorKind::SemanticMemoryLimit
            | SemanticErrorKind::SemanticDepthLimit
            | SemanticErrorKind::SemanticDeadline
            | SemanticErrorKind::OutputLimit => 1,
        },
    }
}

/// The exit code of a selection that matched no declaration or several,
/// in navigation and editing alike.
pub(crate) const UNSELECTED: u8 = 1;

/// The exit code of edits that cannot be made as requested: a conflict,
/// an unsupported site, an invalid fragment, a boundary or attachment
/// violation, a stale file, or recorded edits that no longer prepare.
pub(crate) const REJECTED: u8 = 1;

/// The exit code of a plan that cannot be loaded: 2 when it could not be
/// read or is not a plan, as for other unreadable input; 1 when another
/// build wrote it or its edits no longer give its text, which preparing
/// again resolves.
pub(crate) fn load_failure_code(kind: LoadErrorKind) -> u8 {
    match kind {
        LoadErrorKind::Io | LoadErrorKind::Malformed => 2,
        LoadErrorKind::Incompatible | LoadErrorKind::Replay => REJECTED,
    }
}

/// The exit code of applying a plan: 0 when the files hold the batch or it
/// changes nothing; 1 when a file changed since preparing and nothing was
/// written; 2 for an I/O failure, a rollback after one, or recovery.
pub(crate) fn applied_code(applied: &Applied) -> u8 {
    match (
        applied.effect,
        applied.failure.as_ref().map(|failure| failure.kind),
    ) {
        (Effect::Applied | Effect::NoChange, _) => 0,
        (Effect::Unchanged, Some(ApplyFailureKind::Stale)) => REJECTED,
        _ => 2,
    }
}

/// What a request's documents are relative to: the absolute file of file
/// navigation, or a project's configuration.
#[derive(Clone, Copy)]
pub(crate) enum Base<'a> {
    /// The navigated file as the request named it; navigation makes it
    /// absolute, and its failures name the absolute file.
    File(&'a Path),
    /// The configuration as the request spelled it. A result names the
    /// loaded configuration's canonical path instead, which a failure may
    /// not have.
    Project(&'a str),
}

/// The forms of an analysis failure of `operation`, with the exit code of
/// its kind: the diagnostic `path:line:column: message`, and an `error`
/// document. Identity fields (`file`, `error.path`) hold exact paths only:
/// a path that is not Unicode is `null` there, and the message names it in
/// a lossy form.
pub(crate) fn failure<'a>(
    operation: &'static str,
    base: Base<'a>,
    error: &'a AnalysisError,
) -> Delivery<'a> {
    failure_with(operation, base, error, &[])
}

/// `failure`, with fields an operation adds to its documents, such as
/// editing's `effect`.
pub(crate) fn failure_with<'a>(
    operation: &'static str,
    base: Base<'a>,
    error: &'a AnalysisError,
    fields: &'a [(&'static str, &'static str)],
) -> Delivery<'a> {
    Delivery {
        code: failure_code(error),
        text_is_result: false,
        document: Box::new(move |writer, pretty| {
            let (kind, message, error_path, position) = match error {
                AnalysisError::Input(error) => (
                    json!(error.kind),
                    &error.message,
                    &error.path,
                    error.position,
                ),
                AnalysisError::Query(error) => (
                    json!(error.kind),
                    &error.message,
                    &error.path,
                    error.position,
                ),
                AnalysisError::Semantic(error) => (
                    json!(error.kind),
                    &error.message,
                    &error.path,
                    error.position,
                ),
            };
            // The file the failure concerns: its own path, or else the
            // navigated file.
            let named = match base {
                Base::File(file) => Some(error_path.as_deref().unwrap_or(file)),
                Base::Project(_) => error_path.as_deref(),
            };
            let message = match named.filter(|path| path.to_str().is_none()) {
                Some(path) => format!("{}: {message}", path.display()),
                None => message.clone(),
            };
            let mut document = json!({ "operation": operation, "outcome": "error" });
            for (field, value) in fields {
                document[*field] = json!(value);
            }
            match base {
                Base::File(_) => document["file"] = json!(named.and_then(Path::to_str)),
                Base::Project(config) => document["project"] = json!(config),
            }
            document["error"] = json!({
                "kind": kind,
                "message": message,
                "path": error_path.as_deref().and_then(Path::to_str),
                "position": position,
            });
            if let AnalysisError::Semantic(error) = error {
                document["error"]["reason"] = json!(error.reason);
                if !error.causes.is_empty() {
                    document["error"]["causes"] = json!(error.causes);
                }
                if let Some(omitted) = error.omitted_causes {
                    document["error"]["omittedCauses"] = json!(omitted);
                }
            }
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(|| error.to_string())),
    }
}

/// Write a navigation document.
pub(crate) fn write_document(
    writer: &mut dyn Write,
    document: &Value,
    pretty: bool,
) -> io::Result<()> {
    if pretty {
        serde_json::to_writer_pretty(writer, document)
    } else {
        serde_json::to_writer(writer, document)
    }
    .map_err(io::Error::other)
}

/// Write one result to `output`, or stdout without one, ending with a
/// newline. The output file is created only here, after the result exists.
pub(crate) fn write_output(
    output: Option<&str>,
    write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), Exit> {
    write_to(output, false, write)
}

/// `write_output`, creating `output` only as a new file when `new_file`.
fn write_to(
    output: Option<&str>,
    new_file: bool,
    write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), Exit> {
    let writer: Box<dyn Write> = match output {
        Some(path) => {
            let file = if new_file {
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
            } else {
                fs::File::create(path)
            };
            Box::new(file.map_err(|e| (2, format!("{path}: {e}")))?)
        }
        None => Box::new(io::stdout().lock()),
    };
    let mut writer = BufWriter::new(writer);
    write(&mut writer).map_err(|e| (2, e.to_string()))?;
    writeln!(writer)
        .and_then(|_| writer.flush())
        .map_err(|e| (2, e.to_string()))
}
