//! Navigation output: the text and JSON forms of outline and show results,
//! for file navigation and project navigation alike. Renderers lay out
//! library results; they make no declaration or selection decisions, and
//! `super::delivery` decides where each form goes.
//!
//! A project result names its configuration in `project` and its files
//! relative to that configuration's directory, as the library does; JSON
//! keeps that form. Text names every file absolutely, the configuration's
//! directory joined with the relative file, as file navigation names its
//! file, so a printed `FILE:LINE:COLUMN` is an operand for `--at` from any
//! working directory.

use super::delivery::{Delivery, write_document};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};
use typescript_parser::navigation::{self, Declaration, Kind, Outline, Selector, ShowOutcome};

/// How many outline levels to display; parsing and the catalog never
/// depend on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Depth {
    Levels(usize),
    All,
}

impl Default for Depth {
    fn default() -> Self {
        Self::Levels(2)
    }
}

/// An outline result, shown to `depth`.
pub(crate) fn outline(outline: &Outline, depth: Depth) -> Delivery<'_> {
    Delivery {
        code: 0,
        document: Box::new(move |writer, pretty| {
            let mut document = json!({
                "operation": "outline",
                "outcome": "ok",
                "file": outline.file,
                "entries": nested(&outline.entries, depth),
            });
            project(&mut document, outline.project.as_deref());
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || {
            outline_text(
                &place(outline.project.as_deref(), &outline.file),
                &outline.entries,
                depth,
            )
        })),
    }
}

/// A show result: a found declaration exits 0, and an ambiguous or missing
/// selection exits 1.
pub(crate) fn show(show: &navigation::Show) -> Delivery<'_> {
    Delivery {
        code: match show.outcome {
            ShowOutcome::Found(_) => 0,
            ShowOutcome::Ambiguous(_) | ShowOutcome::NotFound => super::delivery::UNSELECTED,
        },
        document: Box::new(move |writer, pretty| write_document(writer, &show_json(show), pretty)),
        text: Some(Box::new(move || show_text(show))),
    }
}

/// A project result's configuration, the base of its relative files.
fn project(document: &mut Value, project: Option<&Path>) {
    if let Some(project) = project {
        document["project"] = json!(project.to_str());
    }
}

/// A result file as text names it: absolute, the project's directory joined
/// with a project result's relative file, whose `..` segments are resolved
/// lexically, which is exact because that directory is canonical.
pub(crate) fn place(project: Option<&Path>, file: &str) -> String {
    let Some(directory) = project.and_then(Path::parent) else {
        return file.to_owned();
    };
    let mut path = PathBuf::new();
    for component in directory.join(file).components() {
        match component {
            Component::ParentDir
                if matches!(path.components().next_back(), Some(Component::Normal(_))) =>
            {
                path.pop();
            }
            Component::CurDir => {}
            component => path.push(component),
        }
    }
    path.display().to_string()
}

pub(crate) fn show_json(show: &navigation::Show) -> Value {
    let mut document = json!({ "operation": "show" });
    project(&mut document, show.project.as_deref());
    // The searched file; absent when a name searched every file.
    let scope = |document: &mut Value| {
        if let Some(file) = &show.file {
            document["file"] = json!(file);
        }
    };
    match &show.outcome {
        ShowOutcome::Found(found) => {
            document["outcome"] = json!("found");
            document["file"] = json!(found.file);
            document["declaration"] = declaration(&found.declaration);
            document["source"] = json!(found.source);
        }
        ShowOutcome::Ambiguous(candidates) => {
            document["outcome"] = json!("ambiguous");
            scope(&mut document);
            document["query"] = json!(show.query);
            let list: Vec<Value> = candidates
                .iter()
                .map(|candidate| {
                    let mut value = json!({
                        "file": candidate.file,
                        "label": candidate.label,
                        "kind": candidate.kind,
                    });
                    optional(&mut value, "key", &candidate.key);
                    optional(&mut value, "selector", &candidate.selector);
                    value["anchor"] = json!(candidate.anchor);
                    value
                })
                .collect();
            document["candidates"] = list.into();
        }
        ShowOutcome::NotFound => {
            document["outcome"] = json!("not_found");
            scope(&mut document);
            document["query"] = json!(show.query);
        }
    }
    document
}

pub(crate) fn show_text(show: &navigation::Show) -> String {
    let place = |file: &str| place(show.project.as_deref(), file);
    match &show.outcome {
        ShowOutcome::Found(found) => {
            let width = found.lines().last().map_or(1, |(line, _)| digits(line));
            let anchor = found.declaration.anchor;
            let mut text = format!(
                "{}:{}:{} {} {}",
                place(&found.file),
                anchor.line,
                anchor.column,
                found.declaration.kind,
                found.declaration.label
            );
            for (line, source) in found.lines() {
                let _ = write!(text, "\n{line:>width$} |");
                if !source.is_empty() {
                    let _ = write!(text, " {source}");
                }
            }
            text
        }
        ShowOutcome::Ambiguous(candidates) => {
            let mut text = match &show.query {
                Selector::Name(name) => format!(
                    "{}: `{name}` matches {} declarations; select one with --at FILE:LINE:COLUMN",
                    scope(show),
                    candidates.len()
                ),
                Selector::At(at) => format!(
                    "{}:{}:{}: {} declarations are named or anchored at this position",
                    scope(show),
                    at.line,
                    at.column,
                    candidates.len()
                ),
            };
            // Each candidate's location is an `--at` operand; its selector,
            // when it has one, shows its containment.
            for candidate in candidates {
                let _ = write!(
                    text,
                    "\n{}:{}:{} {} {}",
                    place(&candidate.file),
                    candidate.anchor.line,
                    candidate.anchor.column,
                    candidate.kind,
                    candidate.label
                );
                if let Some(selector) = &candidate.selector {
                    let _ = write!(text, " ({selector})");
                }
            }
            text
        }
        ShowOutcome::NotFound => match &show.query {
            Selector::Name(name) => {
                format!("{}: no declaration matches `{name}`", scope(show))
            }
            Selector::At(at) => format!(
                "{}:{}:{}: no declaration is named or anchored at this position",
                scope(show),
                at.line,
                at.column
            ),
        },
    }
}

/// What a selection searched, for its diagnostics: the file it named, or
/// the project whose searched files a name searched.
fn scope(show: &navigation::Show) -> String {
    match (&show.file, &show.project) {
        (Some(file), project) => place(project.as_deref(), file),
        (None, Some(project)) => project.display().to_string(),
        (None, None) => String::new(),
    }
}

/// One declaration's public fields, without its depth, which nesting shows.
fn declaration(declaration: &Declaration) -> Value {
    let mut value = json!({ "label": declaration.label });
    optional(&mut value, "key", &declaration.key);
    optional(&mut value, "selector", &declaration.selector);
    value["kind"] = json!(declaration.kind);
    value["anchor"] = json!(declaration.anchor);
    value["range"] = json!(declaration.range);
    optional(&mut value, "signature", &declaration.signature);
    value
}

fn optional(value: &mut Value, field: &str, text: &Option<String>) {
    if let Some(text) = text {
        value[field] = json!(text);
    }
}

/// The parent of each entry and its number of children, from the
/// pre-order depths.
fn structure(entries: &[Declaration]) -> (Vec<Option<usize>>, Vec<usize>) {
    let mut parents = Vec::with_capacity(entries.len());
    let mut children = vec![0; entries.len()];
    let mut open: Vec<usize> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        while open
            .last()
            .is_some_and(|&last| entries[last].depth >= entry.depth)
        {
            open.pop();
        }
        let parent = open.last().copied();
        if let Some(parent) = parent {
            children[parent] += 1;
        }
        parents.push(parent);
        open.push(index);
    }
    (parents, children)
}

fn shown(depth: Depth, level: usize) -> bool {
    match depth {
        Depth::All => true,
        Depth::Levels(levels) => level <= levels,
    }
}

/// Entries nested under `children`, built bottom-up without recursion. An
/// entry at the display limit reports its `omitted_children`.
fn nested(entries: &[Declaration], depth: Depth) -> Value {
    let (parents, counts) = structure(entries);
    let mut lists: Vec<Vec<Value>> = vec![Vec::new(); entries.len()];
    let mut roots = Vec::new();
    for index in (0..entries.len()).rev() {
        let entry = &entries[index];
        if !shown(depth, entry.depth) {
            continue;
        }
        let mut value = declaration(entry);
        let mut children = std::mem::take(&mut lists[index]);
        children.reverse();
        let omitted = if shown(depth, entry.depth + 1) {
            0
        } else {
            counts[index]
        };
        value["children"] = Value::Array(children);
        if omitted > 0 {
            value["omitted_children"] = json!(omitted);
        }
        match parents[index] {
            Some(parent) => lists[parent].push(value),
            None => roots.push(value),
        }
    }
    roots.reverse();
    Value::Array(roots)
}

/// The most imported names one outline line lists; a long import
/// declaration would otherwise fill a line with names an agent rarely needs.
const IMPORT_NAMES: usize = 8;

/// The file, then `LINE:COLUMN kind signature-or-label` per shown entry,
/// indented two spaces per level, with a count line where depth cuts. An
/// import declaration's entries, one per imported name, share its range
/// and take one line, at the first name's anchor, which `--at` selects,
/// listing their labels: `1:8 import def, alias, Only`. A declaration with
/// more than `IMPORT_NAMES` names lists that many and counts the rest. This
/// is presentation only; the entries and JSON keep one per name.
fn outline_text(file: &str, entries: &[Declaration], depth: Depth) -> String {
    let (_, counts) = structure(entries);
    let mut text = file.to_owned();
    let mut index = 0;
    while index < entries.len() {
        let entry = &entries[index];
        let declaration = if entry.kind == Kind::Import {
            entries[index..]
                .iter()
                .take_while(|next| next.kind == Kind::Import && next.range == entry.range)
                .count()
        } else {
            1
        };
        let group = &entries[index..index + declaration];
        let at = index;
        index += declaration;
        if !shown(depth, entry.depth) {
            continue;
        }
        let indent = "  ".repeat(entry.depth - 1);
        let _ = write!(
            text,
            "\n{indent}{}:{} {} ",
            entry.anchor.line, entry.anchor.column, entry.kind,
        );
        if let [entry] = group {
            text.push_str(entry.signature.as_deref().unwrap_or(&entry.label));
        } else {
            for (position, name) in group.iter().take(IMPORT_NAMES).enumerate() {
                if position > 0 {
                    text.push_str(", ");
                }
                text.push_str(&name.label);
            }
            if group.len() > IMPORT_NAMES {
                let _ = write!(text, ", … {} more", group.len() - IMPORT_NAMES);
            }
        }
        let omitted = counts[at];
        if omitted > 0 && !shown(depth, entry.depth + 1) {
            let noun = if omitted == 1 { "child" } else { "children" };
            let _ = write!(text, "\n{indent}  … {omitted} more {noun}");
        }
    }
    text
}

fn digits(number: usize) -> usize {
    number.to_string().len()
}
