//! The forms of a reference answer: its JSON document, and its text (design,
//! step 4: "References in text"), the one owner of that text. The renderer
//! lays out a library answer, whose fields `typescript_parser::references`
//! defines; it makes no selection, grouping, or coverage decision, and
//! `super::delivery` decides where each form goes.
//!
//! Text names files absolutely, the configuration's directory joined with
//! a result's relative file (`super::navigation::place`), as every text form
//! does, so each printed `FILE:LINE:COLUMN` in a searched file is an `--at`
//! operand from any working directory. A selected answer is a result, on
//! stdout or `-o`; an ambiguous or missing selection is a diagnostic on
//! stderr.
//!
//! A result shows at most its cap of occurrences, counted across groups in
//! the result's order (`super::delivery::Cap`); every group, declaration,
//! gap, and the member hint are always shown, and the header counts every
//! occurrence.

use super::command_line;
use super::delivery::{self, Cap, Delivery, Display, write_displayed, write_document};
use super::navigation::place;
use super::registry::{FlagId, OCCURRENCES};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use typescript_parser::references::{
    DefinitionAnswer, DefinitionResult, DefinitionSelection, DefinitionSelectionFailure, Location,
    MemberSuggestion, Occurrence, ReferenceAnswer, ReferenceFilters, ReferenceGap, ReferenceGroup,
    ReferenceOutcome, ReferenceQuery, ReferenceResult, ReferenceSelection,
    ReferenceSelectionFailure,
};
use typescript_parser::source::SourcePosition;

/// A reference answer. Without `--json` its form is its text, unless
/// `pretty`: `--pretty` alone keeps the indented document references have
/// always written, analysis failures staying diagnostics (decisions.md,
/// step 4).
pub(crate) fn answer<'a>(
    answer: &'a ReferenceAnswer,
    pretty: bool,
    display: &'a Display,
) -> Delivery<'a> {
    let cap = display.cap(&OCCURRENCES);
    let text: Box<dyn FnOnce() -> String + '_> = Box::new(move || match answer {
        ReferenceAnswer::Selected(result) => result_text(result, cap),
        ReferenceAnswer::Unselected(selection) => unselected_text(selection),
    });
    Delivery {
        code: match answer {
            ReferenceAnswer::Selected(result) => code(result),
            ReferenceAnswer::Unselected(_) => delivery::UNSELECTED,
        },
        text_is_result: matches!(answer, ReferenceAnswer::Selected(_)),
        document: Box::new(move |writer, pretty| match answer {
            ReferenceAnswer::Selected(result) => write_result(writer, result, cap, pretty),
            ReferenceAnswer::Unselected(_) => write_document(writer, answer, pretty),
        }),
        text: (!pretty).then_some(text),
    }
}

/// A result of `Project::find_references`, which always selects, in the
/// forms of [`answer`].
pub(crate) fn result<'a>(
    result: &'a ReferenceResult,
    pretty: bool,
    display: &'a Display,
) -> Delivery<'a> {
    let cap = display.cap(&OCCURRENCES);
    let text: Box<dyn FnOnce() -> String + '_> = Box::new(move || result_text(result, cap));
    Delivery {
        code: code(result),
        text_is_result: true,
        document: Box::new(move |writer, pretty| write_result(writer, result, cap, pretty)),
        text: (!pretty).then_some(text),
    }
}

/// How many of each group's occurrences a result shows: the first ones of
/// the result's order, up to its cap, and the total and number shown.
fn kept(result: &ReferenceResult, cap: Cap) -> (Vec<usize>, usize, usize) {
    let total = result
        .groups
        .iter()
        .map(|group| group.occurrences.len())
        .sum();
    let returned = cap.shown(total);
    let mut remaining = returned;
    let groups = result
        .groups
        .iter()
        .map(|group| {
            let shown = group.occurrences.len().min(remaining);
            remaining -= shown;
            shown
        })
        .collect();
    (groups, total, returned)
}

/// A result's document: exactly the result when it shows every occurrence,
/// and otherwise the result with each group's occurrences cut, and
/// `display` after its fields.
fn write_result(
    writer: &mut dyn std::io::Write,
    result: &ReferenceResult,
    cap: Cap,
    pretty: bool,
) -> std::io::Result<()> {
    let (groups, total, returned) = kept(result, cap);
    let Some(display) = cap.display(total, returned) else {
        return write_document(writer, result, pretty);
    };
    let mut shown = result.clone();
    for (group, kept) in shown.groups.iter_mut().zip(groups) {
        group.occurrences.truncate(kept);
    }
    write_displayed(writer, &shown, Some(display), pretty)
}

/// A complete result exits 0, and a partial one 1.
fn code(result: &ReferenceResult) -> u8 {
    match result.outcome {
        ReferenceOutcome::Complete => 0,
        ReferenceOutcome::Partial => delivery::PARTIAL,
    }
}

/// The header, `CONFIG: SELECTION N groups, M occurrences in K files`, with
/// the FILE operand that narrowed the candidates, the filters that bounded
/// the occurrences, and the coverage; then each group, with the
/// occurrences its cap keeps; each gap; the members a bare name did not
/// search; and the cut, last.
fn result_text(result: &ReferenceResult, cap: Cap) -> String {
    let project = &result.project;
    let occurrences = || result.groups.iter().flat_map(|group| &group.occurrences);
    let mut files: Vec<&str> = occurrences()
        .map(|occurrence| occurrence.location.file.as_str())
        .collect();
    files.sort_unstable();
    files.dedup();
    let mut text = format!(
        "{}: {}{}, {} in {}{}{}; coverage {}, {}",
        project.display(),
        subject(project, &result.selection),
        count(result.groups.len(), "group", "groups"),
        count(occurrences().count(), "occurrence", "occurrences"),
        count(files.len(), "file", "files"),
        narrowed(project, &result.selection),
        filtered(project, &result.filters),
        spelled(result.coverage.baseline),
        spelled(result.coverage.status),
    );
    let (kept, total, returned) = kept(result, cap);
    for (index, (group, kept)) in result.groups.iter().zip(kept).enumerate() {
        group_text(&mut text, project, index + 1, group, kept);
    }
    for gap in &result.coverage.gaps {
        gap_text(&mut text, project, gap);
    }
    members_text(&mut text, result);
    cap.end(&mut text, total, returned, None, None);
    text
}

/// How a result's header names its selection: `` `NAME` ``, or
/// `at FILE:LINE:COLUMN:`, each followed by a space.
fn subject(project: &Path, selection: &ReferenceQuery) -> String {
    match selection {
        ReferenceQuery::Name { name, .. } => format!("`{name}` "),
        ReferenceQuery::At { file, position } => {
            format!("at {}: ", position_text(project, file, *position))
        }
    }
}

/// `group N: NAME, declared at FILE:LINE:COLUMN, …`, then the first `kept`
/// of its occurrences. A group is named by its first declaration, or
/// without one by its first occurrence.
fn group_text(
    text: &mut String,
    project: &Path,
    number: usize,
    group: &ReferenceGroup,
    kept: usize,
) {
    let name = match (group.declarations.first(), group.occurrences.first()) {
        (Some(declaration), _) => declaration.name.as_str(),
        (None, Some(occurrence)) => occurrence.name.as_str(),
        (None, None) => "",
    };
    let _ = write!(text, "\ngroup {number}: {name}");
    for (index, declaration) in group.declarations.iter().enumerate() {
        text.push_str(if index == 0 { ", declared at " } else { ", " });
        text.push_str(&location_text(project, &declaration.location));
    }
    occurrences_text(text, project, name, &group.occurrences[..kept]);
}

/// `  FILE:LINE:COLUMN ROLES`, with a renamed alias's spelling in
/// parentheses: one line per occurrence, those of each file together in the
/// order the result first names the file, and each file's in the result's
/// order. This is the only place occurrence lines are written; the
/// occurrences it is given are those the cap keeps.
fn occurrences_text(text: &mut String, project: &Path, name: &str, occurrences: &[Occurrence]) {
    let mut files: Vec<Vec<&Occurrence>> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for occurrence in occurrences {
        let at = *index
            .entry(occurrence.location.file.as_str())
            .or_insert_with(|| {
                files.push(Vec::new());
                files.len() - 1
            });
        files[at].push(occurrence);
    }
    for occurrence in files.into_iter().flatten() {
        let _ = write!(text, "\n  {}", location_text(project, &occurrence.location));
        for (index, role) in occurrence.roles.iter().enumerate() {
            text.push_str(if index == 0 { " " } else { ", " });
            text.push_str(&spelled(role));
        }
        if occurrence.name != name {
            let _ = write!(text, " ({})", occurrence.name);
        }
    }
}

/// `gap FILE:LINE:COLUMN: REASON: MESSAGE`, then each located cause and a
/// count of the causes the library omitted.
pub(super) fn gap_text(text: &mut String, project: &Path, gap: &ReferenceGap) {
    let _ = write!(
        text,
        "\ngap {}: {}: {}",
        location_text(project, &gap.location),
        spelled(gap.reason),
        gap.message
    );
    for cause in &gap.causes {
        text.push_str("\n  cause");
        if let Some(location) = &cause.location {
            let _ = write!(text, " {}", location_text(project, location));
        }
        let _ = write!(text, ": {}: {}", spelled(cause.reason), cause.message);
    }
    if let Some(omitted) = gap.omitted_causes {
        let _ = write!(
            text,
            "\n  … {}",
            count(omitted, "more cause", "more causes")
        );
    }
}

/// One line, when a bare name left catalog members unsearched: their count
/// and, after the same `--references --project CONFIG`, the `--at` that
/// searches one of them, at the position the library names for that
/// (`MemberSuggestion::search`), with the request's filters, so that it asks
/// the same question of that member. The first member with a selector is
/// preferred, as the line can name it. A member without that position, as in
/// a result of `Project::find_references`, is not offered, and with none the
/// line names no search.
fn members_text(text: &mut String, result: &ReferenceResult) {
    let members = &result.unsearched_members;
    if members.selectors.is_empty() {
        return;
    }
    let project = &result.project;
    let chosen = members
        .selectors
        .iter()
        .filter(|member| member.selector.is_some())
        .chain(
            members
                .selectors
                .iter()
                .filter(|member| member.selector.is_none()),
        )
        .find_map(|member: &MemberSuggestion| Some((member, member.search.as_ref()?)));
    let named = match &result.selection {
        ReferenceQuery::Name { name, .. } => format!(" named {name}"),
        ReferenceQuery::At { .. } => String::new(),
    };
    let _ = match members.count {
        1 => write!(text, "\n1 catalog member{named} was not searched"),
        count => write!(text, "\n{count} catalog members{named} were not searched"),
    };
    let Some((member, search)) = chosen else {
        return;
    };
    let mut argv = vec![
        FlagId::At.to_string(),
        format!(
            "{}:{}:{}",
            place(Some(project), &search.file),
            search.position.line,
            search.position.column
        ),
    ];
    for (flag, value) in filter_flags(project, &result.filters) {
        argv.push(flag.to_string());
        argv.extend(value);
    }
    let which = match (&member.selector, members.count) {
        (Some(selector), _) => selector.as_str(),
        (None, 1) => "it",
        (None, _) => "one",
    };
    let _ = write!(
        text,
        "; search {which} with {}",
        command_line::render(&argv)
    );
}

/// A selection that selected no symbol or several, naming the selection;
/// several are listed by the anchor that selects each.
fn unselected_text(selection: &ReferenceSelection) -> String {
    let project = &selection.project;
    let narrowed = narrowed(project, &selection.selection);
    let subject = match &selection.selection {
        ReferenceQuery::Name { name, .. } => format!("`{name}`"),
        ReferenceQuery::At { file, position } => format!(
            "at {}:{}:{}",
            file_text(project, file),
            position.line,
            position.column
        ),
    };
    match &selection.outcome {
        ReferenceSelectionFailure::NotFound { .. } => match &selection.selection {
            ReferenceQuery::Name { .. } => format!(
                "{}: no declaration matches {subject}{narrowed}",
                project.display()
            ),
            ReferenceQuery::At { .. } => {
                format!("{}: no declaration is {subject}", project.display())
            }
        },
        ReferenceSelectionFailure::Ambiguous { candidates } => {
            let mut text = format!(
                "{}: {subject} matches {}{narrowed}; select one with {} FILE:LINE:COLUMN",
                project.display(),
                count(candidates.len(), "symbol", "symbols"),
                FlagId::At
            );
            for candidate in candidates {
                let selector = match &candidate.selector {
                    Some(selector) => selector,
                    None => "(position only)",
                };
                let _ = write!(
                    text,
                    "\n  {}:{}:{} {selector}",
                    place(Some(project), &candidate.anchor.file),
                    candidate.anchor.position.line,
                    candidate.anchor.position.column,
                );
            }
            text
        }
    }
}

/// A definition answer (design, "Result" and "Member positions"): a result
/// is text on stdout or `-o`, exiting 0; a missing selection or a member
/// position is a diagnostic on stderr, or its document, exiting 1, so that a
/// selection of nothing never reads as no declaration. `config` is the request's
/// `--project` spelling, which the command a member position names repeats.
pub(crate) fn definition<'a>(answer: &'a DefinitionAnswer, config: &'a str) -> Delivery<'a> {
    Delivery {
        code: match answer {
            DefinitionAnswer::Selected(_) => 0,
            DefinitionAnswer::Unselected(_) => delivery::UNSELECTED,
        },
        text_is_result: matches!(answer, DefinitionAnswer::Selected(_)),
        document: Box::new(move |writer, pretty| match answer {
            DefinitionAnswer::Selected(result) => write_document(writer, result, pretty),
            DefinitionAnswer::Unselected(selection) => match &selection.outcome {
                DefinitionSelectionFailure::Member { .. } => {
                    #[derive(Serialize)]
                    struct Next<'a> {
                        #[serde(flatten)]
                        selection: &'a DefinitionSelection,
                        next: serde_json::Value,
                    }
                    let next = Next {
                        selection,
                        next: serde_json::json!({ "argv": member_search(selection, config) }),
                    };
                    write_document(writer, &next, pretty)
                }
                DefinitionSelectionFailure::NotFound => write_document(writer, selection, pretty),
            },
        }),
        text: Some(Box::new(move || match answer {
            DefinitionAnswer::Selected(result) => definition_text(result),
            DefinitionAnswer::Unselected(selection) => {
                unselected_definition_text(selection, config)
            }
        })),
    }
}

/// The header, `CONFIG: SELECTION N groups, M declarations` with the FILE
/// operand that narrowed the candidates, then each group, named by its first
/// declaration, with one `FILE:LINE:COLUMN` per declaration and, where a
/// declaration is spelled otherwise, that spelling in parentheses. Every
/// group and declaration is shown.
fn definition_text(result: &DefinitionResult) -> String {
    let project = &result.project;
    let declarations = result
        .groups
        .iter()
        .map(|group| group.declarations.len())
        .sum();
    let mut text = format!(
        "{}: {}{}, {}{}",
        project.display(),
        subject(project, &result.selection),
        count(result.groups.len(), "group", "groups"),
        count(declarations, "declaration", "declarations"),
        narrowed(project, &result.selection),
    );
    for (index, group) in result.groups.iter().enumerate() {
        let name = group
            .declarations
            .first()
            .map_or("", |declaration| declaration.name.as_str());
        let _ = write!(text, "\ngroup {}: {name}", index + 1);
        for declaration in &group.declarations {
            let _ = write!(
                text,
                "\n  {}",
                location_text(project, &declaration.location)
            );
            if declaration.name != name {
                let _ = write!(text, " ({})", declaration.name);
            }
        }
    }
    text
}

/// The diagnostic of a definition that selected no declaration: at a
/// member position, the member and the search that finds its declarations;
/// at a position in no identifier, what a definition follows; and for a name,
/// what it searched.
fn unselected_definition_text(selection: &DefinitionSelection, config: &str) -> String {
    let project = &selection.project;
    let at = match &selection.selection {
        ReferenceQuery::At { file, position } => position_text(project, file, *position),
        ReferenceQuery::Name { .. } => project.display().to_string(),
    };
    match (&selection.outcome, &selection.selection) {
        (DefinitionSelectionFailure::Member { name }, _) => format!(
            "{at}: `{name}` is a member, which {} does not follow; find its declarations with: {}",
            FlagId::Definition,
            command_line::render(&member_search(selection, config))
        ),
        (DefinitionSelectionFailure::NotFound, ReferenceQuery::At { .. }) => format!(
            "{at}: no identifier is at this position; {} follows identifiers, not keywords or string literals",
            FlagId::Definition
        ),
        (DefinitionSelectionFailure::NotFound, ReferenceQuery::Name { name, .. }) => format!(
            "{at}: no declaration, import, or export matches `{name}`{}",
            narrowed(project, &selection.selection)
        ),
    }
}

/// The reference search that a selection a definition did not follow asks
/// of the same selection: `tsparse --references --project CONFIG` with the
/// selection, its file absolute, as the request spelled `--project` and
/// naming no transport, so the service's and a direct answer print the
/// same command.
fn member_search(selection: &DefinitionSelection, config: &str) -> Vec<String> {
    let project = &selection.project;
    let mut argv = vec![
        "tsparse".to_string(),
        FlagId::References.to_string(),
        FlagId::Project.to_string(),
        config.to_string(),
    ];
    match &selection.selection {
        ReferenceQuery::At { file, position } => argv.extend([
            FlagId::At.to_string(),
            position_text(project, file, *position),
        ]),
        ReferenceQuery::Name { name, file } => {
            argv.extend([FlagId::Name.to_string(), name.clone()]);
            argv.extend(file.as_deref().map(|file| file_text(project, file)));
        }
    }
    argv
}

/// `; candidates narrowed to FILE` when the FILE operand narrowed a name's
/// candidates, so no group cannot read as no reference.
fn narrowed(project: &Path, selection: &ReferenceQuery) -> String {
    match selection {
        ReferenceQuery::Name {
            file: Some(file), ..
        } => format!("; candidates narrowed to {}", file_text(project, file)),
        ReferenceQuery::Name { file: None, .. } | ReferenceQuery::At { .. } => String::new(),
    }
}

/// `; filtered by FLAGS`, naming each reporting filter the result echoes,
/// so that an occurrence count it bounded cannot read as every occurrence.
fn filtered(project: &Path, filters: &ReferenceFilters) -> String {
    let flags: Vec<String> = filter_flags(project, filters)
        .into_iter()
        .map(|(flag, value)| match value {
            Some(value) => format!("{flag} {value}"),
            None => flag.to_string(),
        })
        .collect();
    if flags.is_empty() {
        String::new()
    } else {
        format!("; filtered by {}", flags.join(", "))
    }
}

/// The flags, each with its value, that ask for `filters`, in the order
/// help lists them.
fn filter_flags(project: &Path, filters: &ReferenceFilters) -> Vec<(FlagId, Option<String>)> {
    let mut flags = Vec::new();
    if filters.calls {
        flags.push((FlagId::Calls, None));
    }
    if filters.exclude_tests {
        flags.push((FlagId::NoTests, None));
    }
    if let Some(file) = &filters.file {
        flags.push((FlagId::File, Some(file_text(project, file))));
    }
    flags
}

/// A selection's or filter's file, which the library echoes as `Location::file` names
/// files: text, so this conversion is exact.
pub(super) fn file_text(project: &Path, file: &Path) -> String {
    place(Some(project), &file.to_string_lossy())
}

/// A selection's `FILE:LINE:COLUMN`, FILE absolute.
fn position_text(project: &Path, file: &Path, position: SourcePosition) -> String {
    format!(
        "{}:{}:{}",
        file_text(project, file),
        position.line,
        position.column
    )
}

pub(super) fn location_text(project: &Path, location: &Location) -> String {
    format!(
        "{}:{}:{}",
        place(Some(project), &location.file),
        location.start.line,
        location.start.column
    )
}

/// `N noun`, singular for one.
pub(super) fn count(number: usize, one: &str, many: &str) -> String {
    format!("{number} {}", if number == 1 { one } else { many })
}

/// The name the document gives `value`, a unit variant, so that text and
/// JSON spell it alike.
pub(super) fn spelled(value: impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        other => unreachable!("a unit variant serializes as its name: {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::gap_text;
    use std::path::Path;
    use typescript_parser::references::{
        Location, ReferenceCause, ReferenceGap, ReferenceGapReason,
    };
    use typescript_parser::source::SourcePosition;

    fn location(line: usize, column: usize) -> Location {
        let position = SourcePosition { line, column };
        Location {
            file: "src/use.ts".into(),
            start: position,
            end: position,
        }
    }

    /// A cause without a declaration site, such as a compiler global, has no
    /// location to print, and causes the library omitted are counted, never
    /// dropped silently.
    #[test]
    fn a_gap_prints_every_cause_and_counts_those_omitted() {
        let gap = |omitted_causes| ReferenceGap {
            reason: ReferenceGapReason::FlowRequired,
            location: location(8, 6),
            message: "Receiver typing requires an unsupported refinement".into(),
            causes: vec![
                ReferenceCause {
                    reason: ReferenceGapReason::ErrorType,
                    location: Some(location(2, 1)),
                    message: "located".into(),
                },
                ReferenceCause {
                    reason: ReferenceGapReason::UnresolvedSymbol,
                    location: None,
                    message: "global".into(),
                },
            ],
            omitted_causes,
        };
        let lines = "
gap /work/src/use.ts:8:6: flow_required: Receiver typing requires an unsupported refinement
  cause /work/src/use.ts:2:1: error_type: located
  cause: unresolved_symbol: global";
        for (omitted, last) in [
            (None, ""),
            (Some(1), "\n  … 1 more cause"),
            (Some(3), "\n  … 3 more causes"),
        ] {
            let mut text = String::new();
            gap_text(&mut text, Path::new("/work/tsconfig.json"), &gap(omitted));
            assert_eq!(text, format!("{lines}{last}"));
        }
    }
}
