//! Editing commands: prepare a batch from one edit or a batch file, in
//! named files or a project, preview it, and save it as a plan; or apply a
//! saved plan. Preparation and application are the library's; this module
//! reads the request's inputs, checks that outputs alias no input or
//! target, and lays out each outcome, which `super::delivery` delivers.
//!
//! Text output for a preview is, per edit, its selected site and where it
//! changes the file, then a unified diff of every changed file, then the
//! saved plan and the command that applies it. JSON gives the same outcome,
//! files, changes, and diffs as one document, with that command as `next`.
//! Every edit document names its `effect` on the files, independently of
//! any error. A preview shows at most its cap of each changed file's diff
//! lines, and an unselected edit its cap of candidates, as show does
//! (`super::delivery::Cap`); every changed file is listed, and the cut
//! leaves the outcome and `effect` as they are.

use super::Failure;
use super::command_line;
use super::delivery::{
    self, Base, Delivered, Delivery, Display, Route, Undelivered, write_document,
};
use super::failure::Code;
use super::help;
use super::navigation;
use super::query;
use super::registry::{DIFF_LINES, FlagId, OperationId};
use super::request::{EditAction, EditCommand, EditSource};
use serde_json::{Value, json};
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use typescript_parser::editing::{
    self, Applied, Edit, EditRequest, Effect, FileState, LoadError, LoadErrorKind, Operation,
    Outcome, PreparedBatch, Rejection, Unselected,
};
use typescript_parser::navigation::{Show, ShowQuery};
use typescript_parser::project::{ProjectRequest, ProjectResponse};

mod batch;

/// How an editing command's documents name it, and the fields each
/// carries, before any file is written.
#[derive(Clone, Copy)]
pub(crate) struct Documents {
    pub(crate) operation: &'static str,
    pub(crate) fields: &'static [(&'static str, &'static str)],
}

/// A command that prepares edits: a preview, which writes no file.
const PREPARED: Documents = Documents {
    operation: "edit",
    fields: &[("effect", "none")],
};

/// Applying a plan, before it changes any file.
const UNAPPLIED: Documents = Documents {
    operation: "apply",
    fields: &[("effect", "unchanged")],
};

/// How the documents of `operation` name it before any file is written,
/// when it edits: so a usage failure's document carries the `effect` its
/// other documents do.
pub(crate) fn documents(operation: OperationId) -> Option<Documents> {
    match operation {
        OperationId::ReplaceBody | OperationId::InsertAfter | OperationId::EditBatch => {
            Some(PREPARED)
        }
        OperationId::Apply => Some(UNAPPLIED),
        OperationId::Parse
        | OperationId::Outline
        | OperationId::Show
        | OperationId::Type
        | OperationId::References
        | OperationId::ReferencesStatus
        | OperationId::ReferencesStop
        | OperationId::ReferencesServe
        | OperationId::Doctor => None,
    }
}

impl EditCommand {
    /// Where this command's outcomes go: `-o` only as a new file, and
    /// documents named as `documents` says.
    fn route(&self, documents: Documents) -> Route<'_> {
        Route {
            format: self.format,
            pretty: self.pretty,
            output: self.output.as_deref(),
            new_file: true,
            operation: Some(documents.operation),
            fields: documents.fields,
        }
    }
}

pub(crate) fn run(command: EditCommand) -> Result<(), Delivered> {
    match &command.action {
        EditAction::Prepare {
            source,
            project,
            plan,
        } => prepare(&command, source, project.as_ref(), plan.as_deref()),
        EditAction::Apply { plan } => apply(&command, plan),
    }
}

/// Read a fragment from a file, or stdin for `-`, exactly.
fn read_fragment(path: &Path) -> Result<String, Failure> {
    if path == Path::new("-") {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| Failure::new(Code::Read, format!("-: {error}")))?;
        return Ok(text);
    }
    std::fs::read_to_string(path)
        .map_err(|error| Failure::new(Code::Read, format!("{}: {error}", path.display())))
}

/// An edit whose target is known and whose fragment may still have to be
/// read.
struct Pending {
    operation: Operation,
    target: ShowQuery,
    fragment: Fragment,
}

enum Fragment {
    Inline(String),
    /// A file, or stdin for `-`.
    From(PathBuf),
}

/// The edits a command's source names, each with its target, before any
/// fragment is read: a batch file's are read and validated by `batch`.
fn pending(source: &EditSource) -> Result<Vec<Pending>, Failure> {
    match source {
        EditSource::One {
            operation,
            target,
            from,
        } => Ok(vec![Pending {
            operation: *operation,
            target: target.clone(),
            fragment: Fragment::From(from.clone()),
        }]),
        EditSource::Batch(path) => batch::read(path),
    }
}

/// The request `pending` describes, once every fragment is read.
fn request(pending: Vec<Pending>) -> Result<EditRequest, Failure> {
    let mut edits = Vec::with_capacity(pending.len());
    for edit in pending {
        edits.push(Edit {
            operation: edit.operation,
            target: edit.target,
            fragment: match edit.fragment {
                Fragment::Inline(fragment) => fragment,
                Fragment::From(from) => read_fragment(&from)?,
            },
        });
    }
    Ok(EditRequest { edits })
}

fn prepare(
    command: &EditCommand,
    source: &EditSource,
    project: Option<&(String, bool)>,
    plan: Option<&Path>,
) -> Result<(), Delivered> {
    let route = command.route(PREPARED);
    // Every outcome, failures included, reaches -o only once the outputs
    // are checked against the files the edits name, which a batch names in
    // its file. A batch that cannot be read or decoded leaves them unknown,
    // so, as for a plan that cannot be read, its failure is not written to
    // -o (decisions.md, step 3).
    let pending = match pending(source) {
        Ok(pending) => pending,
        Err(failure) => {
            return Err(match command.output.as_deref() {
                Some(output) => delivery::fail(
                    Route::TEXT,
                    failure
                        .reworded(|message| {
                            format!("{message}; the result is not written to {output}")
                        })
                        .with_why(
                            "the batch's file names cannot be established, so the result's path could be one of them",
                        ),
                ),
                None => delivery::fail(route, failure),
            });
        }
    };
    // A command repeating these edits could not read their stdin again,
    // so what reads it is never cut (`Display::reading_stdin`).
    let stdin = pending.iter().any(|edit| match &edit.fragment {
        Fragment::From(from) => from == Path::new("-"),
        Fragment::Inline(_) => false,
    });
    let once;
    let display = if stdin {
        once = command
            .display
            .reading_stdin()
            .map_err(|failure| delivery::fail(route, failure))?;
        &once
    } else {
        &command.display
    };
    let named: Vec<PathBuf> = pending
        .iter()
        .filter_map(|edit| match &edit.target {
            ShowQuery::Name { file, .. } => file.clone(),
            ShowQuery::At { file, .. } => Some(file.clone()),
        })
        .collect();
    let base_file = named.first().cloned();
    // Every file the command reads or edits: each target, the project's
    // configuration, each fragment file, and the batch file, so that no
    // outcome, a failure to read one included, creates one of them.
    let batch = match source {
        EditSource::One { .. } => None,
        EditSource::Batch(path) => Some(path.as_path()),
    };
    let fragments = pending.iter().filter_map(|edit| match &edit.fragment {
        Fragment::From(from) if from != Path::new("-") => Some(from.as_path()),
        Fragment::From(_) | Fragment::Inline(_) => None,
    });
    let reserved: Vec<&Path> = named
        .iter()
        .map(PathBuf::as_path)
        .chain(project.map(|(config, _)| Path::new(config)))
        .chain(fragments)
        .chain(batch)
        .collect();
    new_outputs(command.output.as_deref(), plan)
        .and_then(|()| unreserved(command.output.as_deref(), plan, &reserved))
        .map_err(|failure| delivery::fail(route, failure))?;
    let request = request(pending).map_err(|failure| delivery::fail(route, failure))?;
    let prepared = match project {
        None => editing::prepare(&request),
        Some((config, direct)) => {
            match query::project(config, ProjectRequest::Edit(request), *direct) {
                Err(failure) => return Err(delivery::fail(route, failure)),
                Ok(Err(error)) => Err(error),
                Ok(Ok(ProjectResponse::Edit(selected))) => match *selected {
                    Outcome::Ready(selection) => editing::bind(&selection),
                    Outcome::Unselected(unselected) => Ok(Outcome::Unselected(unselected)),
                    Outcome::Rejected(rejection) => Ok(Outcome::Rejected(rejection)),
                },
                Ok(Ok(other)) => unreachable!("an edit request answered {other:?}"),
            }
        }
    };
    let project_path = project.map(|(config, _)| config.as_str());
    let base = match (project_path, &base_file) {
        (Some(config), _) => Base::Project(config),
        (None, Some(file)) => Base::File(file),
        (None, None) => Base::Project(""),
    };
    let outcome = match &prepared {
        Err(error) => delivery::failure_with(PREPARED.operation, base, error, PREPARED.fields),
        Ok(Outcome::Unselected(unselected)) => {
            unselected_delivery(unselected, project_path, display)
        }
        Ok(Outcome::Rejected(rejection)) => rejected(rejection),
        Ok(Outcome::Ready(batch)) => {
            if let Some(plan) = plan
                && let Err(error) = batch.save(plan)
            {
                return Err(delivery::fail(
                    route,
                    Failure::new(
                        Code::WritePlan,
                        format!("{}: cannot save the plan: {error}", plan.display()),
                    ),
                ));
            }
            let preview = preview(batch, plan, display);
            return match (delivery::try_deliver(route, Ok(preview)), plan) {
                (Ok(()), _) => Ok(()),
                (Err(Undelivered::Outcome(delivered)), _) => Err(delivered),
                (Err(Undelivered::Write(failure)), Some(plan)) => {
                    Err(delivery::fail(route, plan_saved(failure, plan)))
                }
                (Err(Undelivered::Write(failure)), None) => Err(delivery::fail(route, failure)),
            };
        }
    };
    delivery::deliver(route, Ok(outcome))
}

/// A preview that could not be written after its plan was saved: the
/// failure names the plan and, where its path can be named exactly, the
/// command that applies it as `next`.
fn plan_saved(failure: Failure, plan: &Path) -> Failure {
    let failure =
        failure.reworded(|message| format!("{message}; the plan was saved to {}", plan.display()));
    match apply_command(plan) {
        Some(argv) => failure.with_next(argv.map(String::from).to_vec()),
        None => failure,
    }
}

/// The result (`--output`) and plan paths a command writes, each by the
/// flag that names it, which messages name as the registry spells it,
/// whichever alias was given.
fn outputs<'a>(output: Option<&'a str>, plan: Option<&'a Path>) -> Vec<(FlagId, &'a Path)> {
    output
        .map(|output| (FlagId::Output, Path::new(output)))
        .into_iter()
        .chain(plan.map(|plan| (FlagId::Plan, plan)))
        .collect()
}

/// Why an editing command refuses an output that exists or names a file it
/// reads or edits.
const NEW_FILES: &str = "no outcome may replace a file the command read or edits";

/// Refuse a result (`--output`) or plan path that exists, even as a
/// symbolic link to nothing: an editing command writes both only as new
/// files, so no outcome replaces a file it read, selected, or edits. The
/// refusal names the remedy, another path, and why (design, step 5,
/// "Refusal wording").
fn new_outputs(output: Option<&str>, plan: Option<&Path>) -> Result<(), Failure> {
    for (flag, path) in outputs(output, plan) {
        match std::fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(Failure::new(
                    Code::Output,
                    format!(
                        "{flag} {} already exists; an edit command writes its result and plan only as new files; choose another path",
                        path.display()
                    ),
                )
                .with_why(NEW_FILES));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(Failure::new(
                    Code::Output,
                    format!("{flag} {}: cannot be inspected: {error}", path.display()),
                ));
            }
        }
    }
    Ok(())
}

/// Refuse new result and plan paths that name one new file, or that would
/// create a file the command names, `reserved`, which may have been
/// removed since: so no outcome recreates a file it read or edits. Each
/// refusal names the remedy, another path, and why, as `new_outputs` does.
fn unreserved<'a>(
    output: Option<&'a str>,
    plan: Option<&'a Path>,
    reserved: &[&'a Path],
) -> Result<(), Failure> {
    let created = |flag: FlagId, path: &Path, names: &[&'a Path]| {
        editing::would_create(path, names).map_err(|error| {
            Failure::new(Code::Output, format!("{flag} {}: {error}", path.display()))
        })
    };
    let outputs = outputs(output, plan);
    if let [(flag, output), (_, plan)] = outputs[..]
        && created(flag, output, &[plan])?.is_some()
    {
        return Err(Failure::new(
            Code::Output,
            format!(
                "{} and {} name one file; choose another path",
                FlagId::Output,
                FlagId::Plan
            ),
        )
        .with_why("one file cannot hold both the result and the plan"));
    }
    for (flag, path) in outputs {
        if let Some(name) = created(flag, path, reserved)? {
            return Err(Failure::new(
                Code::Output,
                format!(
                    "{flag} {} names a file the command reads or edits, {}; choose another path",
                    path.display(),
                    name.display()
                ),
            )
            .with_why(NEW_FILES));
        }
    }
    Ok(())
}

/// The command that applies the plan saved at `plan`, or `None` when its
/// path is not Unicode and so cannot be named exactly.
fn apply_command(plan: &Path) -> Option<[&str; 3]> {
    plan.to_str()
        .map(|plan| ["tsparse", FlagId::Apply.flag().name, plan])
}

/// A saved plan's path and, where it can be named exactly, the command
/// that applies it: `PLAN; apply it with: tsparse --apply PLAN`.
fn saved_plan(plan: &Path) -> String {
    match apply_command(plan) {
        Some(argv) => format!(
            "{}; apply it with: {}",
            plan.display(),
            command_line::render(&argv)
        ),
        None => plan.display().to_string(),
    }
}

/// Each file's diff, and how many of its lines its cap shows.
struct Diffs {
    diffs: Vec<(String, usize)>,
    total: usize,
    returned: usize,
}

impl Diffs {
    fn new(batch: &PreparedBatch, cap: delivery::Cap) -> Self {
        let mut total = 0;
        let mut returned = 0;
        let diffs = batch
            .files()
            .iter()
            .map(|file| {
                let diff = file.diff();
                let lines = diff.split_inclusive('\n').count();
                let shown = cap.shown(lines);
                total += lines;
                returned += shown;
                (diff, shown)
            })
            .collect();
        Self {
            diffs,
            total,
            returned,
        }
    }

    /// The lines of the `index`th file's diff its cap shows, each with its
    /// newline, and whether that is all of them.
    fn shown(&self, index: usize) -> (&str, bool) {
        let (diff, shown) = &self.diffs[index];
        let end = diff.split_inclusive('\n').take(*shown).map(str::len).sum();
        (&diff[..end], end == diff.len())
    }
}

/// The forms of a prepared batch: its edits and the diffs its cap shows.
fn preview<'a>(
    batch: &'a PreparedBatch,
    plan: Option<&'a Path>,
    display: &'a Display,
) -> Delivery<'a> {
    let cap = display.cap(&DIFF_LINES);
    Delivery {
        code: 0,
        text_is_result: true,
        document: Box::new(move |writer, pretty| {
            let diffs = Diffs::new(batch, cap);
            let files: Vec<Value> = batch
                .files()
                .iter()
                .enumerate()
                .map(|(index, file)| {
                    json!({
                        "file": file.display(),
                        "path": file.path().to_str(),
                        "changed": file.changes(),
                        "edits": file.edits(),
                        "diff": diffs.shown(index).0,
                    })
                })
                .collect();
            let mut document = json!({
                "operation": "edit",
                "outcome": if batch.is_empty() { "no_change" } else { "preview" },
                "effect": "none",
                "validation": "syntax_and_edit_boundaries",
                "files": files,
                "plan": plan.and_then(Path::to_str),
            });
            if let Some(project) = batch.project() {
                document["project"] = json!(project.to_str());
            }
            if let Some(argv) = plan.and_then(apply_command) {
                document["next"] = json!({ "argv": argv });
            }
            if let Some(display) = cap.display(diffs.total, diffs.returned) {
                document["display"] = display;
            }
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || {
            let diffs = Diffs::new(batch, cap);
            let mut text = String::new();
            for file in batch.files() {
                let place = navigation::place(batch.project(), file.display());
                for edit in file.edits() {
                    let anchor = edit.site.anchor;
                    text.push_str(&format!(
                        "{place}:{}:{}: {} {} {}",
                        anchor.line,
                        anchor.column,
                        edit.operation.as_str(),
                        edit.site.kind,
                        edit.site.label
                    ));
                    if let Some(selector) = &edit.site.selector {
                        text.push_str(&format!(" ({selector})"));
                    }
                    let range = edit.site.range;
                    match edit.operation {
                        Operation::ReplaceBody => {
                            if let Some(change) = edit.changes.first() {
                                text.push_str(&format!(
                                    ": body {}:{}-{}:{}",
                                    change.start.line,
                                    change.start.column,
                                    change.end.line,
                                    change.end.column
                                ));
                            }
                        }
                        Operation::InsertAfter => {
                            text.push_str(&format!(
                                ": after the statement at {}:{}-{}:{}",
                                range.start.line,
                                range.start.column,
                                range.end.line,
                                range.end.column
                            ));
                        }
                    }
                    text.push('\n');
                }
            }
            // The files whose diffs the cap cuts, as text names them.
            let mut cut = Vec::new();
            if batch.is_empty() {
                text.push_str("no change: the edits leave every file as it is\n");
            } else {
                text.push_str("validated: syntax and edit boundaries; not type-checked\n");
                for (index, file) in batch.files().iter().enumerate() {
                    let (diff, whole) = diffs.shown(index);
                    text.push_str(diff);
                    if !whole {
                        cut.push(navigation::place(batch.project(), file.display()));
                    }
                }
            }
            match plan {
                Some(plan) => text.push_str(&format!("plan: {}", saved_plan(plan))),
                None => text.push_str(&format!(
                    "preview only: save a plan with {} PLAN to apply it",
                    FlagId::Plan
                )),
            }
            let scope = format!("in {}", cut.join(", "));
            let note = plan.map(|plan| {
                format!(
                    "the plan {} holds the complete proposed text",
                    plan.display()
                )
            });
            cap.end(
                &mut text,
                diffs.total,
                diffs.returned,
                Some(&scope),
                note.as_deref(),
            );
            text
        })),
    }
}

/// An edit's selection that matched no site or several, as `show` gives
/// it.
fn unselected_delivery<'a>(
    unselected: &'a Unselected,
    project: Option<&'a str>,
    display: &Display,
) -> Delivery<'a> {
    let show = Show {
        project: project.map(PathBuf::from),
        file: unselected.file.clone(),
        query: unselected.query.clone(),
        outcome: unselected.outcome.clone(),
    };
    let edit = unselected.edit;
    let document = {
        let mut document = navigation::show_json(&show, display);
        document["operation"] = json!("edit");
        document["effect"] = json!("none");
        document["edit"] = json!(edit);
        document
    };
    let text = format!(
        "edit {}: {}",
        edit + 1,
        navigation::show_text(&show, display)
    );
    Delivery {
        code: delivery::UNSELECTED,
        text_is_result: false,
        document: Box::new(move |writer, pretty| write_document(writer, &document, pretty)),
        text: Some(Box::new(move || text)),
    }
}

/// A rejected batch.
fn rejected(rejection: &Rejection) -> Delivery<'_> {
    Delivery {
        text_is_result: false,
        code: delivery::REJECTED,
        document: Box::new(move |writer, pretty| {
            let document = json!({
                "operation": "edit",
                "outcome": "rejected",
                "effect": "none",
                "error": rejection,
            });
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || {
            let mut text = String::new();
            if let Some(file) = &rejection.file {
                text.push_str(file);
                if let Some(position) = rejection.position {
                    text.push_str(&format!(":{}:{}", position.line, position.column));
                }
                text.push_str(": ");
            }
            if let Some(edit) = rejection.edit {
                text.push_str(&format!("edit {}: ", edit + 1));
            }
            text.push_str(&rejection.message);
            if let Some((position, line)) = &rejection.proposed {
                text.push_str(&format!(
                    "\n  proposed line {}, column {}: {line}",
                    position.line, position.column
                ));
            }
            text
        })),
    }
}

fn apply(command: &EditCommand, plan: &Path) -> Result<(), Delivered> {
    let route = command.route(UNAPPLIED);
    let output = command.output.as_deref();
    let loaded = PreparedBatch::load(plan);
    let recorded: Option<Vec<&Path>> = match &loaded {
        Ok(batch) => Some(batch.names()),
        Err(error) => {
            (error.names.as_ref()).map(|names| names.iter().map(PathBuf::as_path).collect())
        }
    };
    new_outputs(output, None).map_err(|failure| delivery::fail(route, failure))?;
    let Some(recorded) = recorded else {
        // Which files a plan that cannot be read or decoded names is
        // unknown, so no output may be created for it.
        let error = loaded.expect_err("only a plan that failed to load has unknown names");
        return match output {
            Some(output) => Err(delivery::diagnostic(
                delivery::load_failure_code(error.kind),
                &format!(
                    "{error}\nthe result is not written to {output}: the plan's file names cannot be read, so it could be one of them{}",
                    plan_help_line(&error)
                ),
            )),
            None => delivery::deliver(route, Ok(load_failure(&error))),
        };
    };
    unreserved(output, None, &[&[plan][..], &recorded].concat())
        .map_err(|failure| delivery::fail(route, failure))?;
    let batch = match loaded {
        Ok(batch) => batch,
        Err(error) => {
            return match delivery::try_deliver(route, Ok(load_failure(&error))) {
                Ok(()) => Ok(()),
                Err(Undelivered::Outcome(delivered)) => Err(delivered),
                Err(Undelivered::Write(failure)) => Err(delivery::fail(
                    route,
                    unwritten(failure).with_undelivered(error.to_string()),
                )),
            };
        }
    };
    let applied = batch.apply();
    match delivery::try_deliver(route, Ok(applied_delivery(&applied))) {
        Ok(()) => Ok(()),
        Err(Undelivered::Outcome(delivered)) => Err(delivered),
        Err(Undelivered::Write(failure)) => {
            Err(delivery::fail(route, undelivered(&applied, failure)))
        }
    }
}

/// A write failure beside an outcome it must not hide.
fn unwritten(failure: Failure) -> Failure {
    failure.reworded(|message| format!("the result could not be written: {message}"))
}

/// The failure of an apply whose result could not be written: the write
/// failure, then the outcome as the text result gives it (its failure,
/// files, retained originals, and leftovers), so none is lost.
fn undelivered(applied: &Applied, failure: Failure) -> Failure {
    match applied.effect {
        Effect::NoChange => failure,
        _ => unwritten(failure).with_undelivered(applied_text(applied)),
    }
}

/// The command that prints `--apply`'s help, which says a plan is what
/// `--plan` saved, for a plan whose text does not decode as one; `None` for
/// any other load failure, which that help could not fix.
fn plan_help(error: &LoadError) -> Option<Vec<String>> {
    (error.kind == LoadErrorKind::Malformed && error.names.is_none())
        .then(|| help::help_argv(OperationId::Apply.operation()))
}

/// `plan_help` as the text's last line, or nothing.
fn plan_help_line(error: &LoadError) -> String {
    match plan_help(error) {
        Some(argv) => format!("\n  next: {}", command_line::render(&argv)),
        None => String::new(),
    }
}

fn load_failure(error: &LoadError) -> Delivery<'_> {
    Delivery {
        text_is_result: false,
        code: delivery::load_failure_code(error.kind),
        document: Box::new(move |writer, pretty| {
            let mut document = json!({
                "operation": "apply",
                "outcome": "error",
                "effect": "unchanged",
                "error": error,
            });
            if let Some(argv) = plan_help(error) {
                document["error"]["next"] = json!({ "argv": argv });
            }
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || {
            format!("{error}{}", plan_help_line(error))
        })),
    }
}

fn applied_delivery(applied: &Applied) -> Delivery<'_> {
    Delivery {
        code: delivery::applied_code(applied),
        text_is_result: delivery::applied_code(applied) == 0,
        document: Box::new(move |writer, pretty| {
            let outcome = match applied.effect {
                Effect::Applied => "applied",
                Effect::NoChange => "no_change",
                _ => "error",
            };
            let mut document = json!({
                "operation": "apply",
                "outcome": outcome,
                "effect": applied.effect,
                "files": applied.files,
                "leftovers": applied.leftovers,
            });
            if let Some(failure) = &applied.failure {
                document["error"] = json!(failure);
            }
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || applied_text(applied))),
    }
}

/// An apply's outcome as text: what happened to each file, the failure,
/// retained originals, and leftovers.
fn applied_text(applied: &Applied) -> String {
    let mut lines: Vec<String> = Vec::new();
    let path = |path: &Path| path.display().to_string();
    match applied.effect {
        Effect::NoChange => lines.push("no change: the plan leaves every file as it is".into()),
        Effect::Applied => {
            let changed: Vec<String> = applied
                .files
                .iter()
                .filter(|file| file.state == FileState::Replaced)
                .map(|file| path(&file.path))
                .collect();
            lines.push(format!("applied: {} changed", changed.join(", ")));
        }
        Effect::Unchanged | Effect::RolledBack | Effect::RecoveryRequired => {
            if let Some(failure) = &applied.failure {
                lines.push(format!("{}: {}", path(&failure.path), failure.message));
            }
            lines.push(match applied.effect {
                Effect::Unchanged => "no file was changed".into(),
                Effect::RolledBack => "every changed file was restored".into(),
                _ => "recovery required: some files could not be restored".into(),
            });
            for file in &applied.files {
                if let FileState::RecoveryRequired { observed, original } = &file.state {
                    lines.push(format!(
                        "{} holds {observed}; its original is kept at {}",
                        path(&file.path),
                        path(original)
                    ));
                }
            }
        }
    }
    for leftover in &applied.leftovers {
        lines.push(format!(
            "warning: could not remove {}: {}",
            path(&leftover.path),
            leftover.message
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use typescript_parser::editing::{ApplyFailure, ApplyFailureKind, FileOutcome, Leftover};

    fn outcome(effect: Effect, states: [FileState; 2], leftovers: Vec<Leftover>) -> Applied {
        let [a, b] = states;
        Applied {
            effect,
            failure: (effect != Effect::Applied).then(|| ApplyFailure {
                kind: ApplyFailureKind::Io,
                message: "injected rename failure".into(),
                path: "/p/b.ts".into(),
            }),
            files: vec![
                FileOutcome {
                    path: "/p/a.ts".into(),
                    display: "a.ts".into(),
                    state: a,
                },
                FileOutcome {
                    path: "/p/b.ts".into(),
                    display: "b.ts".into(),
                    state: b,
                },
            ],
            leftovers,
        }
    }

    #[test]
    fn every_effect_is_reported_as_itself_even_when_the_result_cannot_be_written() {
        let recovery = outcome(
            Effect::RecoveryRequired,
            [
                FileState::RecoveryRequired {
                    observed: "the edited file".into(),
                    original: "/p/.a.ts.tsparse-1-1.orig".into(),
                },
                FileState::Unchanged,
            ],
            vec![],
        );
        let rolled_back = outcome(
            Effect::RolledBack,
            [FileState::Restored, FileState::Unchanged],
            vec![],
        );
        let applied = outcome(
            Effect::Applied,
            [FileState::Replaced, FileState::Replaced],
            vec![Leftover {
                path: "/p/.a.ts.tsparse-1-2.orig".into(),
                message: "busy".into(),
            }],
        );
        let unchanged = outcome(
            Effect::Unchanged,
            [FileState::Unchanged, FileState::Unchanged],
            vec![Leftover {
                path: "/p/.b.ts.tsparse-1-3.new".into(),
                message: "busy".into(),
            }],
        );
        for (applied, code, says, never) in [
            (
                &recovery,
                2,
                "recovery required: some files could not be restored\n/p/a.ts holds the edited file; its original is kept at /p/.a.ts.tsparse-1-1.orig",
                "applied:",
            ),
            (
                &rolled_back,
                2,
                "/p/b.ts: injected rename failure\nevery changed file was restored",
                "applied:",
            ),
            (
                &applied,
                0,
                "applied: /p/a.ts, /p/b.ts changed\nwarning: could not remove /p/.a.ts.tsparse-1-2.orig: busy",
                "restored",
            ),
            (
                &unchanged,
                2,
                "no file was changed\nwarning: could not remove /p/.b.ts.tsparse-1-3.new: busy",
                "applied:",
            ),
        ] {
            let text = applied_text(applied);
            assert!(text.contains(says) && !text.contains(never), "{text}");
            assert_eq!(delivery::applied_code(applied), code, "{text}");
            let failure = undelivered(
                applied,
                Failure::new(Code::WriteOutput, "out: Is a directory"),
            );
            assert_eq!(delivery::category_code(failure.category()), 2);
            assert_eq!(
                failure.text(),
                format!(
                    "write.output: the result could not be written: out: Is a directory\n{text}"
                )
            );
        }
    }

    /// A directory holding a file whose `start` body an edit replaces, and
    /// the new body as `body.ts`.
    #[cfg(unix)]
    fn edited_file(name: &str) -> (PathBuf, PathBuf) {
        let directory = std::env::temp_dir().join(format!(
            "tsparse-plan-command-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("host.ts");
        std::fs::write(&file, "function start() {\n  return 1;\n}\n").unwrap();
        std::fs::write(directory.join("body.ts"), "{ return 2; }").unwrap();
        (directory, file)
    }

    /// A plan path that is not Unicode cannot be named exactly: the preview
    /// shows it lossily, holds `null` where it would name it, and prints no
    /// command for it.
    #[cfg(unix)]
    #[test]
    fn a_plan_path_that_is_not_unicode_is_never_rendered_into_a_command() {
        use std::os::unix::ffi::OsStrExt;
        let (directory, file) = edited_file("preview");
        let prepared = editing::prepare(&EditRequest {
            edits: vec![Edit {
                operation: Operation::ReplaceBody,
                target: ShowQuery::Name {
                    name: "start".into(),
                    file: Some(file),
                },
                fragment: "{ return 2; }".into(),
            }],
        });
        std::fs::remove_dir_all(&directory).unwrap();
        let Ok(Outcome::Ready(batch)) = prepared else {
            panic!("the edit does not prepare");
        };
        let plan = Path::new(std::ffi::OsStr::from_bytes(b"plan-\xff.json"));
        let display = Display::new(
            OperationId::ReplaceBody.operation(),
            delivery::Limit::Default,
            false,
            vec!["tsparse".into()],
        );
        let text = (preview(&batch, Some(plan), &display).text.unwrap())();
        assert!(text.ends_with("\nplan: plan-\u{FFFD}.json"), "{text}");
        let mut document = Vec::new();
        (preview(&batch, Some(plan), &display).document)(&mut document, false).unwrap();
        let document: Value = serde_json::from_slice(&document).unwrap();
        assert_eq!(document["plan"], Value::Null);
        assert_eq!(document.get("next"), None, "{document}");
    }

    /// The same holds for the failure of a preview that cannot be written
    /// after its plan was saved: it names the plan lossily, with no `next`,
    /// and the command saves such a plan where the file system can.
    #[cfg(unix)]
    #[test]
    fn a_saved_plan_whose_path_is_not_unicode_is_named_lossily_without_a_command() {
        use std::os::unix::ffi::OsStrExt;
        let unwritten = Failure::new(Code::WriteOutput, "out.json: Is a directory");
        let failure = plan_saved(
            unwritten,
            Path::new(std::ffi::OsStr::from_bytes(b"plan-\xff.json")),
        );
        assert_eq!(
            failure.text(),
            "write.output: out.json: Is a directory; the plan was saved to plan-\u{FFFD}.json"
        );
        assert_eq!(failure.error().get("next"), None);
        let named = plan_saved(
            Failure::new(Code::WriteOutput, "out.json: Is a directory"),
            Path::new("my plan.json"),
        );
        assert_eq!(
            named.text(),
            "write.output: out.json: Is a directory; the plan was saved to my plan.json\n  next: tsparse --apply 'my plan.json'"
        );
        let (directory, file) = edited_file("unwritten");
        let plan = directory.join(std::ffi::OsStr::from_bytes(b"plan-\xff.json"));
        if let Err(error) = std::fs::write(&plan, "") {
            std::fs::remove_dir_all(&directory).unwrap();
            // Apple file systems refuse names that are not UTF-8 with EILSEQ.
            if !(cfg!(target_vendor = "apple") && error.raw_os_error() == Some(92)) {
                panic!("{error}");
            }
            eprintln!(
                "SKIP a_saved_plan_whose_path_is_not_unicode_is_named_lossily_without_a_command: the file system refuses names that are not UTF-8 ({error})"
            );
            return;
        }
        std::fs::remove_file(&plan).unwrap();
        let output = directory.join("missing/out.json");
        let result = run(EditCommand {
            action: EditAction::Prepare {
                source: EditSource::One {
                    operation: Operation::ReplaceBody,
                    target: ShowQuery::Name {
                        name: "start".into(),
                        file: Some(file),
                    },
                    from: directory.join("body.ts"),
                },
                project: None,
                plan: Some(plan.clone()),
            },
            format: delivery::Format::Json,
            pretty: false,
            output: Some(output.to_str().unwrap().into()),
            display: Display::new(
                OperationId::ReplaceBody.operation(),
                delivery::Limit::Default,
                true,
                vec!["tsparse".into()],
            ),
        });
        let saved = plan.exists();
        std::fs::remove_dir_all(&directory).unwrap();
        assert_eq!(result.map_err(|delivered| delivered.code()), Err(2));
        assert!(saved);
    }
}
