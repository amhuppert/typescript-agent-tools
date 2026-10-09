//! Editing commands: prepare a batch from one edit or a batch file, in
//! named files or a project, preview it, and save it as a plan; or apply a
//! saved plan. Preparation and application are the library's; this module
//! reads the request's inputs, checks that outputs alias no input or
//! target, and lays out each outcome, which `super::delivery` delivers.
//!
//! Text output for a preview is, per edit, its selected site and where it
//! changes the file, then a unified diff of every changed file. JSON gives
//! the same outcome, files, changes, and diffs as one document. Every edit
//! document names its `effect` on the files, independently of any error.

use super::Exit;
use super::delivery::{self, Base, Delivery, Undelivered, write_document};
use super::navigation;
use super::query;
use super::request::{EditAction, EditCommand, EditSource};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use typescript_parser::editing::{
    self, Applied, Edit, EditRequest, Effect, FileState, LoadError, Operation, Outcome,
    PreparedBatch, Rejection, Unselected,
};
use typescript_parser::navigation::{Show, ShowQuery};
use typescript_parser::project::{ProjectRequest, ProjectResponse};

pub(crate) fn run(command: EditCommand) -> Result<(), Exit> {
    match &command.action {
        EditAction::Prepare {
            source,
            project,
            plan,
        } => prepare(&command, source, project.as_ref(), plan.as_deref()),
        EditAction::Apply { plan } => apply(&command, plan),
    }
}

/// One edit of a batch file. Paths are relative to the working directory.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchEdit {
    operation: Operation,
    /// An exact or qualified declaration name, as `--name` takes.
    name: Option<String>,
    /// `LINE:COLUMN` with `file`, or `FILE:LINE:COLUMN`, as `--at` takes.
    at: Option<String>,
    file: Option<PathBuf>,
    /// The new source inline, or `from` a file.
    fragment: Option<String>,
    from: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchFile {
    edits: Vec<BatchEdit>,
}

/// Read a fragment from a file, or stdin for `-`, exactly.
fn read_fragment(path: &Path) -> Result<String, Exit> {
    if path == Path::new("-") {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| (2, format!("-: {error}")))?;
        return Ok(text);
    }
    std::fs::read_to_string(path).map_err(|error| (2, format!("{}: {error}", path.display())))
}

/// The request a command's source describes.
fn request(source: &EditSource) -> Result<EditRequest, Exit> {
    match source {
        EditSource::One {
            operation,
            target,
            from,
        } => Ok(EditRequest {
            edits: vec![Edit {
                operation: *operation,
                target: target.clone(),
                fragment: read_fragment(from)?,
            }],
        }),
        EditSource::Batch(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|error| (2, format!("{}: {error}", path.display())))?;
            let batch: BatchFile = serde_json::from_str(&text)
                .map_err(|error| (2, format!("{}: not an edit batch: {error}", path.display())))?;
            let mut edits = Vec::with_capacity(batch.edits.len());
            for (index, edit) in batch.edits.into_iter().enumerate() {
                let invalid = |message: &str| {
                    (
                        2,
                        format!("{}: edit {}: {message}", path.display(), index + 1),
                    )
                };
                let target = match (edit.name, edit.at) {
                    (Some(name), None) => ShowQuery::Name {
                        name,
                        file: edit.file,
                    },
                    (None, Some(at)) => {
                        let (file, position) = super::request::location(
                            &at,
                            edit.file.map(|file| file.to_string_lossy().into_owned()),
                            "at",
                        )
                        .map_err(|message| invalid(&message))?;
                        ShowQuery::At { file, position }
                    }
                    _ => return Err(invalid("give exactly one of name and at")),
                };
                let fragment = match (edit.fragment, edit.from) {
                    (Some(fragment), None) => fragment,
                    (None, Some(from)) => read_fragment(&from)?,
                    _ => return Err(invalid("give exactly one of fragment and from")),
                };
                edits.push(Edit {
                    operation: edit.operation,
                    target,
                    fragment,
                });
            }
            if edits.is_empty() {
                return Err((2, format!("{}: the batch has no edits", path.display())));
            }
            Ok(EditRequest { edits })
        }
    }
}

fn prepare(
    command: &EditCommand,
    source: &EditSource,
    project: Option<&(String, bool)>,
    plan: Option<&Path>,
) -> Result<(), Exit> {
    let request = request(source)?;
    let named: Vec<PathBuf> = request
        .edits
        .iter()
        .filter_map(|edit| match &edit.target {
            ShowQuery::Name { file, .. } => file.clone(),
            ShowQuery::At { file, .. } => Some(file.clone()),
        })
        .collect();
    let base_file = named.first().cloned();
    let reserved: Vec<&Path> = named
        .iter()
        .map(PathBuf::as_path)
        .chain(project.map(|(config, _)| Path::new(config)))
        .collect();
    new_outputs(command.output.as_deref(), plan)?;
    unreserved(command.output.as_deref(), plan, &reserved)?;
    let prepared = match project {
        None => editing::prepare(&request),
        Some((config, direct)) => {
            match query::project(Path::new(config), ProjectRequest::Edit(request), *direct) {
                Err(transport) => {
                    return deliver(command, Err(&transport));
                }
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
        Err(error) => delivery::failure_with("edit", base, error, &[("effect", "none")]),
        Ok(Outcome::Unselected(unselected)) => unselected_delivery(unselected, project_path),
        Ok(Outcome::Rejected(rejection)) => rejected(rejection),
        Ok(Outcome::Ready(batch)) => {
            if let Some(plan) = plan {
                batch.save(plan).map_err(|error| {
                    (
                        2,
                        format!("{}: cannot save the plan: {error}", plan.display()),
                    )
                })?;
            }
            let delivered = delivery::try_deliver(
                command.format,
                command.pretty,
                command.output.as_deref(),
                true,
                Ok(preview(batch, plan)),
            );
            return match (delivered, plan) {
                (Ok(()), _) => Ok(()),
                (Err(Undelivered::Write((code, error))), Some(plan)) => Err((
                    code,
                    format!(
                        "{error}\nthe plan was saved to {}; apply it with: tsparse --apply {}",
                        plan.display(),
                        plan.display()
                    ),
                )),
                (Err(Undelivered::Outcome(exit) | Undelivered::Write(exit)), _) => Err(exit),
            };
        }
    };
    deliver(command, Ok(outcome))
}

/// Deliver an editing outcome, `-o` only as a new file.
fn deliver(command: &EditCommand, outcome: Result<Delivery, &io::Error>) -> Result<(), Exit> {
    delivery::try_deliver(
        command.format,
        command.pretty,
        command.output.as_deref(),
        true,
        outcome,
    )
    .map_err(|undelivered| match undelivered {
        Undelivered::Outcome(exit) | Undelivered::Write(exit) => exit,
    })
}

/// The result (`-o`) and plan paths a command writes, by flag.
fn outputs<'a>(output: Option<&'a str>, plan: Option<&'a Path>) -> Vec<(&'static str, &'a Path)> {
    output
        .map(|output| ("-o", Path::new(output)))
        .into_iter()
        .chain(plan.map(|plan| ("--plan", plan)))
        .collect()
}

/// Refuse a result (`-o`) or plan path that exists, even as a symbolic
/// link to nothing: an editing command writes both only as new files, so
/// no outcome replaces a file it read, selected, or edits.
fn new_outputs(output: Option<&str>, plan: Option<&Path>) -> Result<(), Exit> {
    for (flag, path) in outputs(output, plan) {
        match std::fs::symlink_metadata(path) {
            Ok(_) => {
                return Err((
                    2,
                    format!(
                        "{flag} {} already exists; an edit command writes its result and plan only as new files",
                        path.display()
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err((
                    2,
                    format!("{flag} {}: cannot be inspected: {error}", path.display()),
                ));
            }
        }
    }
    Ok(())
}

/// Refuse new result and plan paths that name one new file, or that would
/// create a file the command names, `reserved`, which may have been
/// removed since: so no outcome recreates a file it read or edits.
fn unreserved<'a>(
    output: Option<&'a str>,
    plan: Option<&'a Path>,
    reserved: &[&'a Path],
) -> Result<(), Exit> {
    let created = |flag: &str, path: &Path, names: &[&'a Path]| {
        editing::would_create(path, names)
            .map_err(|error| (2, format!("{flag} {}: {error}", path.display())))
    };
    let outputs = outputs(output, plan);
    if let [(flag, output), (_, plan)] = outputs[..]
        && created(flag, output, &[plan])?.is_some()
    {
        return Err((2, "-o and --plan name one file; choose another path".into()));
    }
    for (flag, path) in outputs {
        if let Some(name) = created(flag, path, reserved)? {
            return Err((
                2,
                format!(
                    "{flag} {} names a file the command reads or edits, {}; choose another path",
                    path.display(),
                    name.display()
                ),
            ));
        }
    }
    Ok(())
}

/// The forms of a prepared batch: its edits and diffs.
fn preview<'a>(batch: &'a PreparedBatch, plan: Option<&'a Path>) -> Delivery<'a> {
    Delivery {
        code: 0,
        document: Box::new(move |writer, pretty| {
            let files: Vec<Value> = batch
                .files()
                .iter()
                .map(|file| {
                    json!({
                        "file": file.display(),
                        "path": file.path().to_str(),
                        "changed": file.changes(),
                        "edits": file.edits(),
                        "diff": file.diff(),
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
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || {
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
            if batch.is_empty() {
                text.push_str("no change: the edits leave every file as it is\n");
            } else {
                text.push_str("validated: syntax and edit boundaries; not type-checked\n");
                for file in batch.files() {
                    text.push_str(&file.diff());
                }
            }
            match plan {
                Some(plan) => text.push_str(&format!(
                    "plan: {}; apply it with: tsparse --apply {}",
                    plan.display(),
                    plan.display()
                )),
                None => text.push_str("preview only: save a plan with --plan PLAN to apply it"),
            }
            text
        })),
    }
}

/// An edit's selection that matched no site or several, as `show` gives
/// it.
fn unselected_delivery<'a>(unselected: &'a Unselected, project: Option<&'a str>) -> Delivery<'a> {
    let show = Show {
        project: project.map(PathBuf::from),
        file: unselected.file.clone(),
        query: unselected.query.clone(),
        outcome: unselected.outcome.clone(),
    };
    let edit = unselected.edit;
    let document = {
        let mut document = navigation::show_json(&show);
        document["operation"] = json!("edit");
        document["effect"] = json!("none");
        document["edit"] = json!(edit);
        document
    };
    let text = format!("edit {}: {}", edit + 1, navigation::show_text(&show));
    Delivery {
        code: delivery::UNSELECTED,
        document: Box::new(move |writer, pretty| write_document(writer, &document, pretty)),
        text: Some(Box::new(move || text)),
    }
}

/// A rejected batch.
fn rejected(rejection: &Rejection) -> Delivery<'_> {
    Delivery {
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

fn apply(command: &EditCommand, plan: &Path) -> Result<(), Exit> {
    let output = command.output.as_deref();
    let loaded = PreparedBatch::load(plan);
    let recorded: Option<Vec<&Path>> = match &loaded {
        Ok(batch) => Some(batch.names()),
        Err(error) => {
            (error.names.as_ref()).map(|names| names.iter().map(PathBuf::as_path).collect())
        }
    };
    new_outputs(output, None)?;
    let Some(recorded) = recorded else {
        // Which files a plan that cannot be read or decoded names is
        // unknown, so no output may be created for it.
        let error = loaded.expect_err("only a plan that failed to load has unknown names");
        return match output {
            Some(output) => Err((
                delivery::load_failure_code(error.kind),
                format!(
                    "{error}\nthe result is not written to {output}: the plan's file names cannot be read, so it could be one of them"
                ),
            )),
            None => deliver(command, Ok(load_failure(&error))),
        };
    };
    unreserved(output, None, &[&[plan][..], &recorded].concat())?;
    let batch = match loaded {
        Ok(batch) => batch,
        Err(error) => {
            let delivered = delivery::try_deliver(
                command.format,
                command.pretty,
                output,
                true,
                Ok(load_failure(&error)),
            );
            return match delivered {
                Ok(()) => Ok(()),
                Err(Undelivered::Outcome(exit)) => Err(exit),
                Err(Undelivered::Write((code, write))) => Err((
                    code,
                    format!("the result could not be written: {write}\n{error}"),
                )),
            };
        }
    };
    let applied = batch.apply();
    let delivered = delivery::try_deliver(
        command.format,
        command.pretty,
        command.output.as_deref(),
        true,
        Ok(applied_delivery(&applied)),
    );
    match delivered {
        Ok(()) => Ok(()),
        Err(Undelivered::Outcome(exit)) => Err(exit),
        Err(Undelivered::Write(exit)) => Err(undelivered(&applied, exit)),
    }
}

/// The diagnostic for an apply whose result could not be written: the
/// write failure, then the outcome as the text result gives it (its
/// failure, files, retained originals, and leftovers), so none is lost.
fn undelivered(applied: &Applied, (code, error): Exit) -> Exit {
    match applied.effect {
        Effect::NoChange => (code, error),
        _ => (
            2,
            format!(
                "the result could not be written: {error}\n{}",
                applied_text(applied)
            ),
        ),
    }
}

fn load_failure(error: &LoadError) -> Delivery<'_> {
    Delivery {
        code: delivery::load_failure_code(error.kind),
        document: Box::new(move |writer, pretty| {
            let document = json!({
                "operation": "apply",
                "outcome": "error",
                "effect": "unchanged",
                "error": error,
            });
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || error.to_string())),
    }
}

fn applied_delivery(applied: &Applied) -> Delivery<'_> {
    Delivery {
        code: delivery::applied_code(applied),
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
            let (exit, message) = undelivered(applied, (2, "out: Is a directory".into()));
            assert_eq!(exit, 2);
            assert_eq!(
                message,
                format!("the result could not be written: out: Is a directory\n{text}")
            );
        }
    }
}
