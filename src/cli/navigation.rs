//! Navigation output: the text and JSON forms of outline and show results,
//! for file navigation and project navigation alike, and of the test view,
//! which only file navigation gives. Renderers lay out library results;
//! they make no declaration or selection decisions, and `super::delivery`
//! decides where each form goes.
//!
//! A project result names its configuration in `project` and its files
//! relative to that configuration's directory, as the library does; JSON
//! keeps that form. Text names every file absolutely, the configuration's
//! directory joined with the relative file, as file navigation names its
//! file, so a printed `FILE:LINE:COLUMN` is an operand for `--at` from any
//! working directory.
//!
//! An outline shows at most its cap of the entries `--depth` shows, the
//! test view at most its cap of its entries, show at most its cap of a
//! declaration's lines, from its first, and of an ambiguous selection's
//! candidates (`super::delivery::Cap`).

use super::command_line;
use super::delivery::{Cap, Delivery, Display, write_document};
use super::registry::{self, CANDIDATES, ENTRIES, FlagId, LINES, TEST_ENTRIES};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};
use typescript_parser::navigation::{
    self, Declaration, Kind, Outline, Selector, ShowOutcome, Specifier, TestEntry, TestOutline,
};

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

/// An outline result, shown to `depth` and to its cap of entries.
pub(crate) fn outline<'a>(
    outline: &'a Outline,
    depth: Depth,
    display: &'a Display,
) -> Delivery<'a> {
    let cap = display.cap(&ENTRIES);
    Delivery {
        code: 0,
        text_is_result: true,
        document: Box::new(move |writer, pretty| {
            let kept = Kept::new(&outline.entries, depth, cap);
            let mut document = json!({
                "operation": "outline",
                "outcome": "ok",
                "file": outline.file,
                "entries": nested(&outline.entries, depth, &kept.entries),
            });
            project(&mut document, outline.project.as_deref());
            if let Some(display) = cap.display(kept.total, kept.returned) {
                document["display"] = display;
            }
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || {
            let kept = Kept::new(&outline.entries, depth, cap);
            let mut text = outline_text(
                &place(outline.project.as_deref(), &outline.file),
                &outline.entries,
                depth,
                &kept.entries,
            );
            cap.end(&mut text, kept.total, kept.returned, None, None);
            text
        })),
    }
}

/// The entries an outline displays: those `--depth` shows, in source order,
/// up to its cap.
struct Kept {
    /// Whether each entry is displayed.
    entries: Vec<bool>,
    /// How many `--depth` shows.
    total: usize,
    returned: usize,
}

impl Kept {
    fn new(entries: &[Declaration], depth: Depth, cap: Cap) -> Self {
        let total = entries
            .iter()
            .filter(|entry| shown(depth, entry.depth))
            .count();
        let returned = cap.shown(total);
        let mut rank = 0;
        let entries = entries
            .iter()
            .map(|entry| {
                let kept = shown(depth, entry.depth) && rank < returned;
                rank += usize::from(shown(depth, entry.depth));
                kept
            })
            .collect();
        Self {
            entries,
            total,
            returned,
        }
    }
}

/// A test view, shown to its cap of entries: the file, then per entry
/// `LINE:COLUMN call[ name]`, indented two spaces per level, and nested by
/// containment in JSON. The cap keeps a prefix in source order, so every
/// shown entry's parent is shown.
pub(crate) fn test_outline<'a>(outline: &'a TestOutline, display: &'a Display) -> Delivery<'a> {
    let cap = display.cap(&TEST_ENTRIES);
    let total = outline.entries.len();
    let returned = cap.shown(total);
    let shown = &outline.entries[..returned];
    Delivery {
        code: 0,
        text_is_result: true,
        document: Box::new(move |writer, pretty| {
            let mut document = json!({
                "operation": "test-outline",
                "outcome": "ok",
                "file": outline.file,
                "entries": test_entries(shown),
            });
            if let Some(display) = cap.display(total, returned) {
                document["display"] = display;
            }
            write_document(writer, &document, pretty)
        }),
        text: Some(Box::new(move || {
            let mut text = outline.file.clone();
            for entry in shown {
                let indent = "  ".repeat(entry.depth - 1);
                let anchor = entry.anchor;
                let _ = write!(
                    text,
                    "\n{indent}{}:{} {}",
                    anchor.line, anchor.column, entry.call
                );
                if let Some(name) = &entry.name {
                    let _ = write!(text, " {name}");
                }
            }
            cap.end(&mut text, total, returned, None, None);
            text
        })),
    }
}

/// A show result: a found declaration exits 0, and an ambiguous or missing
/// selection exits 1. `config` is the request's `--project` spelling, which
/// the commands a missed name's specifiers name repeat, or `None` in file
/// mode.
pub(crate) fn show<'a>(
    show: &'a navigation::Show,
    display: &'a Display,
    config: Option<&'a str>,
) -> Delivery<'a> {
    Delivery {
        text_is_result: matches!(show.outcome, ShowOutcome::Found(_)),
        code: match show.outcome {
            ShowOutcome::Found(_) => 0,
            ShowOutcome::Ambiguous(_) | ShowOutcome::NotFound { .. } => super::delivery::UNSELECTED,
        },
        document: Box::new(move |writer, pretty| {
            write_document(writer, &show_json(show, display, config), pretty)
        }),
        text: Some(Box::new(move || show_text(show, display, config))),
    }
}

/// The first `lines` lines of `found`'s source, as the library splits them
/// (`Found::lines`), without the line break after the last.
fn excerpt(found: &navigation::Found, lines: usize) -> &str {
    let source = found.source.as_str();
    match found.lines().nth(lines.saturating_sub(1)) {
        // Each line is a slice of the source, so its end is an offset in it.
        Some((_, line)) => {
            let end = line.as_ptr() as usize - source.as_ptr() as usize + line.len();
            &source[..end]
        }
        None => source,
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

/// A show result's document, with at most its cap of the declaration's
/// lines or of the candidates, and a missed name's `specifiers`, each with
/// its command as `next` only where `config` completes it.
pub(crate) fn show_json(show: &navigation::Show, display: &Display, config: Option<&str>) -> Value {
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
            let cap = display.cap(&LINES);
            let total = found.lines().count();
            let returned = cap.shown(total);
            document["outcome"] = json!("found");
            document["file"] = json!(found.file);
            document["declaration"] = declaration(&found.declaration);
            document["source"] = json!(excerpt(found, returned));
            if let Some(display) = cap.display(total, returned) {
                document["display"] = display;
            }
        }
        ShowOutcome::Ambiguous(candidates) => {
            let cap = display.cap(&CANDIDATES);
            let returned = cap.shown(candidates.len());
            document["outcome"] = json!("ambiguous");
            scope(&mut document);
            document["query"] = json!(show.query);
            let list: Vec<Value> = candidates[..returned]
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
            if let Some(display) = cap.display(candidates.len(), returned) {
                document["display"] = display;
            }
        }
        ShowOutcome::NotFound { specifiers } => {
            document["outcome"] = json!("not_found");
            scope(&mut document);
            document["query"] = json!(show.query);
            if !specifiers.is_empty() {
                let list: Vec<Value> = specifiers
                    .iter()
                    .map(|specifier| {
                        let mut value = json!({
                            "anchor": specifier.anchor,
                            "kind": specifier.kind,
                        });
                        if let Some(config) = config {
                            value["next"] = json!({
                                "argv": definition_at(show, specifier, config),
                            });
                        }
                        value
                    })
                    .collect();
                document["specifiers"] = list.into();
            }
        }
    }
    document
}

/// The definition that a specifier a name missed denotes: `tsparse
/// --definition --project CONFIG --at FILE:LINE:COLUMN` at its name, its
/// file absolute, naming no transport, so the service's and a direct answer
/// print the same command.
fn definition_at(show: &navigation::Show, specifier: &Specifier, config: &str) -> Vec<String> {
    vec![
        "tsparse".to_owned(),
        FlagId::Definition.to_string(),
        FlagId::Project.to_string(),
        config.to_owned(),
        FlagId::At.to_string(),
        specifier_place(show, specifier),
    ]
}

/// A specifier's anchor as an `--at` operand, in the show's file, absolute.
fn specifier_place(show: &navigation::Show, specifier: &Specifier) -> String {
    format!(
        "{}:{}:{}",
        scope(show),
        specifier.anchor.line,
        specifier.anchor.column
    )
}

/// A show result's text, with at most its cap of the declaration's lines,
/// numbered as the whole declaration is, or of the candidates; a missed
/// name adds one line per specifier that spells it, naming the command that
/// finds its declaration, with `--project`'s placeholder where `config` is
/// `None`.
pub(crate) fn show_text(
    show: &navigation::Show,
    display: &Display,
    config: Option<&str>,
) -> String {
    let place = |file: &str| place(show.project.as_deref(), file);
    match &show.outcome {
        ShowOutcome::Found(found) => {
            let cap = display.cap(&LINES);
            let total = found.lines().count();
            let returned = cap.shown(total);
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
            for (line, source) in found.lines().take(returned) {
                let _ = write!(text, "\n{line:>width$} |");
                if !source.is_empty() {
                    let _ = write!(text, " {source}");
                }
            }
            cap.end(&mut text, total, returned, None, None);
            text
        }
        ShowOutcome::Ambiguous(candidates) => {
            let cap = display.cap(&CANDIDATES);
            let returned = cap.shown(candidates.len());
            let mut text = match &show.query {
                Selector::Name(name) => format!(
                    "{}: `{name}` matches {} declarations; select one with {} FILE:LINE:COLUMN",
                    scope(show),
                    candidates.len(),
                    FlagId::At
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
            for candidate in &candidates[..returned] {
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
            cap.end(&mut text, candidates.len(), returned, None, None);
            text
        }
        ShowOutcome::NotFound { specifiers } => match &show.query {
            Selector::Name(name) => {
                let mut text = format!("{}: no declaration matches `{name}`", scope(show));
                let config = config.unwrap_or(match FlagId::Project.flag().kind {
                    registry::Kind::Value(placeholder) => placeholder,
                    registry::Kind::Boolean => unreachable!("--project takes a value"),
                });
                for specifier in specifiers {
                    let how = match specifier.kind {
                        Kind::Export => "re-exported",
                        // The catalog lists only import and export entries.
                        _ => "imported",
                    };
                    let _ = write!(
                        text,
                        "\n`{name}` is {how} at {}; find its declaration with: {}",
                        specifier_place(show, specifier),
                        command_line::render(&definition_at(show, specifier, config))
                    );
                }
                text
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
fn structure<T>(entries: &[T], depth: impl Fn(&T) -> usize) -> (Vec<Option<usize>>, Vec<usize>) {
    let mut parents = Vec::with_capacity(entries.len());
    let mut children = vec![0; entries.len()];
    let mut open: Vec<usize> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        while open
            .last()
            .is_some_and(|&last| depth(&entries[last]) >= depth(entry))
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

/// The `kept` entries nested under `children`, built bottom-up without
/// recursion. An entry at the display limit reports its `omitted_children`;
/// those its cap withholds are counted in `display`. Every kept entry's
/// parent is kept, as the cap keeps a prefix of the entries in source order.
fn nested(entries: &[Declaration], depth: Depth, kept: &[bool]) -> Value {
    let (parents, counts) = structure(entries, |entry| entry.depth);
    let mut lists: Vec<Vec<Value>> = vec![Vec::new(); entries.len()];
    let mut roots = Vec::new();
    for index in (0..entries.len()).rev() {
        let entry = &entries[index];
        if !kept[index] {
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

/// The test view's `entries`, each nested under its parent's `children`,
/// built bottom-up without recursion, as `nested` builds an outline's.
fn test_entries(entries: &[TestEntry]) -> Value {
    let (parents, _) = structure(entries, |entry| entry.depth);
    let mut lists: Vec<Vec<Value>> = vec![Vec::new(); entries.len()];
    let mut roots = Vec::new();
    for (index, entry) in entries.iter().enumerate().rev() {
        let mut children = std::mem::take(&mut lists[index]);
        children.reverse();
        let mut value = json!({
            "anchor": entry.anchor,
            "call": entry.call,
            "children": children,
        });
        optional(&mut value, "name", &entry.name);
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
/// is presentation only; the entries and JSON keep one per name. Only the
/// `kept` entries are shown: where the cap ends inside an import
/// declaration, its line lists the names before the cut and counts the
/// declaration's others, as it counts those past the eighth, so it never
/// reads as the whole declaration.
fn outline_text(file: &str, entries: &[Declaration], depth: Depth, kept: &[bool]) -> String {
    let (_, counts) = structure(entries, |entry| entry.depth);
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
        let at = index;
        let group = &entries[index..index + declaration];
        // The cap keeps a prefix of the entries, so a prefix of the group.
        let displayed = kept[index..index + declaration]
            .iter()
            .take_while(|&&kept| kept)
            .count();
        index += declaration;
        if displayed == 0 {
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
            let listed = displayed.min(IMPORT_NAMES);
            for (position, name) in group[..listed].iter().enumerate() {
                if position > 0 {
                    text.push_str(", ");
                }
                text.push_str(&name.label);
            }
            if group.len() > listed {
                let _ = write!(text, ", … {} more", group.len() - listed);
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
