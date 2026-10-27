//! The one delivery policy: how every outcome of a query reaches stdout,
//! stderr, the `-o` file, and the exit status, whether file navigation, a
//! project loaded in this process, or the project's service answered it.
//!
//! | Format | Result (complete exit 0, partial exit 1) | Selection or analysis failure (exit 1 or 2) |
//! | --- | --- | --- |
//! | text (the default) | text to stdout or `-o` | diagnostic to stderr |
//! | JSON (`--json`) | document to stdout or `-o` | document to stdout or `-o` |
//!
//! A reference answer with `--pretty` and no `--json` keeps its indented
//! document in place of its text, for a selection as for a result, while
//! an analysis failure stays a diagnostic (decisions.md, step 4).
//!
//! Ambiguous and missing selections (`UNSELECTED`), partial results
//! (`PARTIAL`), and syntax errors (`SYNTAX_ERROR`) exit 1; an analysis
//! failure exits with the code of its kind (`failure_code`), which a
//! process a query's limit stops also exits with. Editing adds rejected edits
//! (`REJECTED`), plans that cannot be loaded (`load_failure_code`), and the
//! effects of applying (`applied_code`); its text results and diagnostics
//! and its documents follow the same table. A document is never accompanied by
//! stderr output, and a diagnostic never creates or replaces the `-o` file.
//!
//! A failure that leaves no outcome to deliver is a [`Failure`], whose
//! category decides its exit code (`category_code`) and its channel
//! (`fail`): in text, `CODE: message` and its detail lines on stderr; with
//! `--json`, one document, a usage failure's on stdout and never `-o`, and
//! any other's where the request's outcome would go. A result that cannot
//! be written is one stderr diagnostic, never followed by a second document.
//!
//! What an outcome displays of the lists it bounds is decided here too, by
//! [`Display`] and the [`Cap`] each renderer asks it for: a cut is a
//! rendering decision, never a change to a library result, so it leaves
//! the outcome, its coverage, gaps, and issues, and its exit code as they
//! are (design, step 4: "Rules shared by every renderer").

use super::Failure;
use super::command_line;
use super::failure::{Category, Code};
use super::registry::{Bounded, FlagId, List, Operation};
use crate::process_limits;
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

/// Where a request's outcomes go, and what its failure documents name,
/// chosen at the request boundary.
#[derive(Clone, Copy)]
pub(crate) struct Route<'a> {
    pub(crate) format: Format,
    /// Indent documents.
    pub(crate) pretty: bool,
    /// The `-o` file, or stdout without one.
    pub(crate) output: Option<&'a str>,
    /// Create `-o` only as a new file, never replacing one, as an editing
    /// command requires.
    pub(crate) new_file: bool,
    /// The operation a failure document names.
    pub(crate) operation: Option<&'static str>,
    /// Fields every document of the operation carries, such as editing's
    /// `effect`.
    pub(crate) fields: &'static [(&'static str, &'static str)],
}

impl Route<'_> {
    /// A request whose outcomes are text and whose failures go to stderr:
    /// one that has no `--json`.
    pub(crate) const TEXT: Route<'static> = Route {
        format: Format::Text,
        pretty: false,
        output: None,
        new_file: false,
        operation: None,
        fields: &[],
    };
}

/// What `--limit` asks of the lists a request's outcome bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Limit {
    /// Not given: each list's default, unless the request names `-o`.
    Default,
    /// At most this many, a positive number, wherever the outcome goes.
    At(usize),
    /// Every item.
    All,
}

/// How a request displays the lists its outcome bounds: the one owner of
/// display caps and of the `display` key, which every renderer applies
/// through the [`Cap`] of each list it lays out.
///
/// `-o` is the spillover channel: with `-o` and no `--limit` nothing is
/// cut, and an explicit `--limit` applies wherever the outcome goes, `-o`
/// included (decisions.md, step 4). A cut withholds the end of a list and
/// says so: in text as the last line, naming what it withheld and the
/// command that shows it; in a document as `display`, present only when
/// something was withheld, so an outcome that shows every item is written
/// exactly as it always was. The command is the request as given with
/// `--limit all` (the run owner's answer to question a61fe38c), which
/// shows every item in the same form and channel.
pub(crate) struct Display {
    /// The operation requested, whose flags a cut's advice may name.
    operation: &'static Operation,
    limit: Limit,
    /// The request names `-o`.
    spill: bool,
    /// The command that shows every item, program name first.
    reveal: Vec<String>,
}

impl Display {
    /// The display of a request for `operation`: what `--limit` asked,
    /// whether it names `-o`, and `reveal`, the request as given with
    /// `--limit all`, which `cli::request` builds.
    pub(crate) fn new(
        operation: &'static Operation,
        limit: Limit,
        spill: bool,
        reveal: Vec<String>,
    ) -> Self {
        Self {
            operation,
            limit,
            spill,
            reveal,
        }
    }

    /// The display of this request once it is known to read stdin, as an
    /// edit whose fragment is `-` does: no command can repeat it to show
    /// what a cut withholds, so nothing is cut, and a `--limit N` that
    /// would cut is refused before stdin is read.
    pub(crate) fn reading_stdin(&self) -> Result<Display, Failure> {
        match self.limit {
            Limit::At(bound) => Err(Failure::new(
                Code::FlagScope,
                format!(
                    "{} {bound} needs every fragment in a file: a command repeating this edit to show what the limit withholds cannot read stdin again",
                    FlagId::Limit
                ),
            )),
            Limit::Default | Limit::All => Ok(Display {
                operation: self.operation,
                limit: Limit::All,
                spill: self.spill,
                reveal: self.reveal.clone(),
            }),
        }
    }

    /// The cap of `bounded`, one of the registry's bounded lists.
    pub(crate) fn cap(&self, bounded: &Bounded) -> Cap<'_> {
        let bound = match self.limit {
            Limit::At(bound) => Some(bound),
            Limit::All => None,
            Limit::Default if self.spill => None,
            Limit::Default => Some(bounded.default),
        };
        Cap {
            list: bounded.list,
            bound,
            display: self,
        }
    }
}

/// What a renderer shows of one bounded list, and how it says what it
/// withheld. A renderer asks only these questions of a cap, so no renderer
/// truncates on its own.
#[derive(Clone, Copy)]
pub(crate) struct Cap<'a> {
    list: List,
    /// At most this many items; `None` for every one.
    bound: Option<usize>,
    display: &'a Display,
}

impl Cap<'_> {
    /// How many of `total` items are shown: the first ones, up to the
    /// bound. Lists cut per part, such as each file's diff, ask it of each.
    pub(crate) fn shown(&self, total: usize) -> usize {
        self.bound.map_or(total, |bound| bound.min(total))
    }

    /// The `display` member of a document that shows `returned` of
    /// `total` items: `None` when it shows every one.
    pub(crate) fn display(&self, total: usize, returned: usize) -> Option<Value> {
        (returned < total).then(|| {
            let mut display = json!({
                "truncated": true,
                "next": { "argv": self.display.reveal },
            });
            display[self.list.name()] = json!({ "total": total, "returned": returned });
            display
        })
    }

    /// End `text`, which shows `returned` of `total` items, with the line
    /// of its cut, unless it shows every one: `… N more ITEMS[ SCOPE];
    /// [NOTE; ]` and the command that shows every one. Candidates, a
    /// diagnostic's list, which never reaches `-o`, are narrowed rather than
    /// written to a file: with `--name` and `--at` where the operation takes
    /// them, and otherwise, for a batch, with the edit's `name` and `at` in
    /// its file. Any other list names `-o` as the alternative when the
    /// request does not.
    pub(crate) fn end(
        &self,
        text: &mut String,
        total: usize,
        returned: usize,
        scope: Option<&str>,
        note: Option<&str>,
    ) {
        if returned >= total {
            return;
        }
        let withheld = total - returned;
        let (one, many) = match self.list {
            List::Occurrences => ("occurrence", "occurrences"),
            List::Entries => ("entry", "entries"),
            List::Candidates => ("candidate", "candidates"),
            List::Lines => ("line", "lines"),
            List::DiffLines => ("diff line", "diff lines"),
            List::Sites => ("site", "sites"),
            List::Lacks => ("lacking property", "lacking properties"),
            List::Gaps => ("gap", "gaps"),
        };
        let mut line = format!(
            "\n… {withheld} more {}",
            if withheld == 1 { one } else { many }
        );
        if let Some(scope) = scope {
            line.push(' ');
            line.push_str(scope);
        }
        if let Some(note) = note {
            line.push_str("; ");
            line.push_str(note);
        }
        let command = command_line::render(&self.display.reveal);
        let operation = self.display.operation;
        match self.list {
            List::Candidates if operation.accepts(FlagId::Name) && operation.accepts(FlagId::At) => {
                line.push_str(&format!(
                    "; narrow with {} a.b or {} FILE:LINE:COLUMN, or see all with: {command}",
                    FlagId::Name,
                    FlagId::At
                ))
            }
            List::Candidates => line.push_str(&format!(
                "; narrow the edit's name to a.b or give its at as FILE:LINE:COLUMN, or see all with: {command}"
            )),
            List::Occurrences
            | List::Entries
            | List::Lines
            | List::DiffLines
            | List::Sites
            | List::Lacks
            | List::Gaps => {
                line.push_str(&format!("; see all with: {command}"));
                if !self.display.spill {
                    line.push_str(&format!(
                        ", or write them to a file with {} FILE",
                        FlagId::Output.flag().aliases[0]
                    ));
                }
            }
        }
        text.push_str(&line);
    }
}

/// A request that did not succeed, once its outcome, or the failure that
/// left none, was delivered: the code it exits with. Only this module
/// decides it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Delivered(u8);

impl Delivered {
    pub(crate) fn code(&self) -> u8 {
        self.0
    }
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
    /// The outcome as text: a result, or a diagnostic. `None` where a text
    /// request is answered by the document: a reference answer with
    /// `--pretty` (`super::references`).
    pub(crate) text: Option<Box<dyn FnOnce() -> String + 'a>>,
    /// A result belongs to stdout/output even when partial (exit 1). A
    /// diagnostic belongs to stderr. Exit status alone cannot decide this.
    pub(crate) text_is_result: bool,
}

/// Deliver `outcome`, the outcome of a valid request, or the failure that
/// left none, as `route` asks.
pub(crate) fn deliver(route: Route, outcome: Result<Delivery, Failure>) -> Result<(), Delivered> {
    try_deliver(route, outcome).map_err(|undelivered| match undelivered {
        Undelivered::Outcome(delivered) => delivered,
        Undelivered::Write(failure) => fail(route, failure),
    })
}

/// Why delivery did not end with exit 0.
pub(crate) enum Undelivered {
    /// The outcome, or the failure that left none, was delivered and exits
    /// non-zero.
    Outcome(Delivered),
    /// The outcome's form could not be written to stdout or `-o`: a write
    /// failure, not yet delivered, beside which the caller may still have
    /// an effect to report.
    Write(Failure),
}

/// `deliver`, telling a delivered non-zero outcome from a failed write, for
/// an outcome whose effect the caller must still report when its form
/// cannot be written.
pub(crate) fn try_deliver(
    route: Route,
    outcome: Result<Delivery, Failure>,
) -> Result<(), Undelivered> {
    let delivery = match outcome {
        Ok(delivery) => delivery,
        Err(failure) => return Err(Undelivered::Outcome(fail(route, failure))),
    };
    let text = match route.format {
        Format::Text => delivery.text,
        Format::Json => None,
    };
    match (text, delivery.text_is_result) {
        (Some(text), true) => {
            let text = text();
            write_to(route.output, route.new_file, |writer| {
                writer.write_all(text.as_bytes())
            })
            .map_err(Undelivered::Write)?;
            exited(delivery.code)
        }
        (Some(text), false) => Err(Undelivered::Outcome(diagnostic(delivery.code, &text()))),
        (None, _) => {
            write_to(route.output, route.new_file, |writer| {
                (delivery.document)(writer, route.pretty)
            })
            .map_err(Undelivered::Write)?;
            exited(delivery.code)
        }
    }
}

/// The end of a delivered outcome that exits with `code`.
fn exited(code: u8) -> Result<(), Undelivered> {
    match code {
        0 => Ok(()),
        code => Err(Undelivered::Outcome(Delivered(code))),
    }
}

/// Deliver an outcome whose one form here is a diagnostic, such as a syntax
/// error, on stderr, exiting with `code`.
pub(crate) fn diagnostic(code: u8, text: &str) -> Delivered {
    eprintln!("{text}");
    Delivered(code)
}

/// The exit code of a failure of each category, the one owner of that
/// decision: 2 for every category, as for a usage error, so that a numeric
/// migration is one change here (design, step 3).
pub(crate) fn category_code(category: Category) -> u8 {
    match category {
        Category::Usage
        | Category::Input
        | Category::Transport
        | Category::Write
        | Category::Internal => 2,
    }
}

/// Deliver `failure`, of a request `route` describes, and return its exit
/// code. In text it is stderr's. With `--json` it is one document: a usage
/// failure's on stdout, never `-o`, because the request, its environment,
/// or the path it named for `-o` is what is wrong; any other's where the
/// request's outcome would go (decisions.md, step 3). A result that cannot
/// be written (`write.output`), like a failure document that cannot be, is
/// one stderr diagnostic, never followed by a second document.
pub(crate) fn fail(route: Route, failure: Failure) -> Delivered {
    let code = category_code(failure.category());
    if route.format == Format::Text || failure.code() == Code::WriteOutput {
        eprintln!("{}", failure.text());
        return Delivered(code);
    }
    let output = match failure.category() {
        Category::Usage => None,
        Category::Input | Category::Transport | Category::Write | Category::Internal => {
            route.output
        }
    };
    let mut document = json!({ "outcome": "error" });
    if let Some(operation) = route.operation {
        document["operation"] = json!(operation);
    }
    for (field, value) in route.fields {
        document[*field] = json!(value);
    }
    document["error"] = failure.error();
    match write_to(output, route.new_file, |writer| {
        write_document(writer, &document, route.pretty)
    }) {
        Ok(()) => Delivered(code),
        Err(unwritten) => fail(route, unwritten),
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
        AnalysisError::Input(error) => input_failure_code(error.kind),
        AnalysisError::Query(error) => match error.kind {
            QueryErrorKind::OutsideProject
            | QueryErrorKind::InvalidPosition
            | QueryErrorKind::InvalidRequest => 2,
            QueryErrorKind::UnsupportedTarget
            | QueryErrorKind::UnresolvedTarget
            | QueryErrorKind::UnknownProperty => 1,
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

/// The exit code of a doctor report with a finding; one whose every check
/// passes exits 0 (decisions.md, cross-cutting).
pub(crate) const FINDINGS: u8 = 1;

/// The exit code of a selection that matched no declaration or several,
/// in navigation, types, references, and editing alike.
pub(crate) const UNSELECTED: u8 = 1;

/// The exit code of a result that is not complete, a partial reference
/// search or type description, which is still delivered as a result.
pub(crate) const PARTIAL: u8 = 1;

/// The exit code of a source that does not parse, in parse mode as in an
/// analysis (`failure_code`).
pub(crate) const SYNTAX_ERROR: u8 = 1;

/// The exit code of edits that cannot be made as requested: a conflict,
/// an unsupported site, an invalid fragment, a boundary or attachment
/// violation, a stale file, or recorded edits that no longer prepare.
pub(crate) const REJECTED: u8 = 1;

/// The exit code of a plan that cannot be loaded: 2 when it could not be
/// read or is not a plan, as for other unreadable input; 1 when another
/// build wrote it or its edits no longer give its text, which preparing
/// again resolves, and, as for any unsupported input, when it records a
/// file editing refuses.
pub(crate) fn load_failure_code(kind: LoadErrorKind) -> u8 {
    match kind {
        LoadErrorKind::Io | LoadErrorKind::Malformed => 2,
        LoadErrorKind::Incompatible | LoadErrorKind::Replay => REJECTED,
        LoadErrorKind::Unsupported => input_failure_code(InputErrorKind::Unsupported),
    }
}

/// The exit code of an input failure, by its kind: 2 for one that could
/// not be read, 1 otherwise.
fn input_failure_code(kind: InputErrorKind) -> u8 {
    match kind {
        InputErrorKind::Io => 2,
        InputErrorKind::Syntax => SYNTAX_ERROR,
        InputErrorKind::Config
        | InputErrorKind::ResourceLimit
        | InputErrorKind::Unsupported
        | InputErrorKind::Binding
        | InputErrorKind::UnresolvedImport => 1,
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
/// a lossy form. A limit's failure carries its remedy, a sentence, as
/// `error.remedy` and as the diagnostic's `  next:` line
/// (`process_limits::remedy`). A file outside the project names, in both
/// forms' message, the configuration that decides membership
/// (`membership`).
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
            let message = format!("{message}{}", membership(error, base));
            let message = match named.filter(|path| path.to_str().is_none()) {
                Some(path) => format!("{}: {message}", path.display()),
                None => message,
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
            if let Some(remedy) = process_limits::remedy(error) {
                document["error"]["remedy"] = json!(remedy);
            }
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || {
            let membership = membership(error, base);
            match process_limits::remedy(error) {
                Some(remedy) => format!("{error}{membership}\n  next: {remedy}"),
                None => format!("{error}{membership}"),
            }
        })),
    }
}

/// What a failure for a file outside the project adds to its message: the
/// configuration, as the request spelled it, whose `files`, `include`, and
/// `exclude`, or those of a configuration it extends, decide which files
/// are searched (design, step 5, "Refusal wording"). Nothing for any other
/// failure, or without a project.
fn membership(error: &AnalysisError, base: Base) -> String {
    match (error, base) {
        (AnalysisError::Query(error), Base::Project(config))
            if error.kind == QueryErrorKind::OutsideProject =>
        {
            format!(
                "; the files, include, and exclude of {config}, or of a configuration it extends, decide which files are searched"
            )
        }
        (AnalysisError::Input(_) | AnalysisError::Query(_) | AnalysisError::Semantic(_), _) => {
            String::new()
        }
    }
}

/// Write `document`, a library result serialized from its struct, with the
/// `display` its cap gives it after its own fields, or exactly as it is
/// without one. A document built as a JSON value takes `display` as one of
/// its keys instead, which sorts it among them.
pub(crate) fn write_displayed(
    writer: &mut dyn Write,
    document: &impl serde::Serialize,
    display: Option<Value>,
    pretty: bool,
) -> io::Result<()> {
    #[derive(serde::Serialize)]
    struct Displayed<'a, T> {
        #[serde(flatten)]
        document: &'a T,
        display: Value,
    }
    match display {
        Some(display) => write_document(writer, &Displayed { document, display }, pretty),
        None => write_document(writer, document, pretty),
    }
}

/// Write a document, indented when `pretty`.
pub(crate) fn write_document(
    writer: &mut dyn Write,
    document: &impl serde::Serialize,
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
) -> Result<(), Delivered> {
    write_to(output, false, write).map_err(|failure| fail(Route::TEXT, failure))
}

/// `write_output`, creating `output` only as a new file when `new_file`,
/// or the write failure that left it unwritten.
fn write_to(
    output: Option<&str>,
    new_file: bool,
    write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), Failure> {
    let unwritten = |message: String| Failure::new(Code::WriteOutput, message);
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
            Box::new(file.map_err(|e| unwritten(format!("{path}: {e}")))?)
        }
        None => Box::new(io::stdout().lock()),
    };
    let mut writer = BufWriter::new(writer);
    write(&mut writer).map_err(|e| unwritten(e.to_string()))?;
    writeln!(writer)
        .and_then(|_| writer.flush())
        .map_err(|e| unwritten(e.to_string()))
}
