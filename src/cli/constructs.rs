//! Text and JSON layout of a construction-site result. The library's result
//! is complete; this owner shows at most the caps of its sites and of each
//! construction's lacking properties (`super::delivery::Cap`), and says so.

use super::delivery::{self, Cap, Delivery, Display, write_displayed, write_document};
use super::references::{count, file_text, gap_text, location_text, spelled};
use super::registry::{CANDIDATES, FlagId, GAPS, LACKS, SITES};
use serde_json::{Value, json};
use std::fmt::Write;
use typescript_parser::constructs::{
    ConstructionSite, ConstructsAnswer, ConstructsOutcome, ConstructsResult, Lack, Presence,
};
use typescript_parser::types::{SelectionFailure, TypeKey, TypeQuery};

pub(crate) fn answer<'a>(answer: &'a ConstructsAnswer, display: &'a Display) -> Delivery<'a> {
    let code = match answer {
        ConstructsAnswer::Selected(result) => match result.outcome {
            ConstructsOutcome::Complete => 0,
            ConstructsOutcome::Partial => delivery::PARTIAL,
        },
        ConstructsAnswer::Unselected(_) => delivery::UNSELECTED,
    };
    let sites = display.cap(&SITES);
    let lacks = display.cap(&LACKS);
    let gaps = display.cap(&GAPS);
    let candidates = display.cap(&CANDIDATES);
    Delivery {
        code,
        text_is_result: matches!(answer, ConstructsAnswer::Selected(_)),
        document: Box::new(move |writer, pretty| match answer {
            ConstructsAnswer::Selected(result) => {
                let shown = Shown::new(result, sites, lacks, gaps);
                if !shown.cut() {
                    return write_document(writer, answer, pretty);
                }
                let mut document = serde_json::to_value(result).map_err(std::io::Error::other)?;
                let documents = document["sites"]
                    .as_array_mut()
                    .expect("sites serialize as a list");
                documents.truncate(shown.sites);
                for (site, kept) in documents.iter_mut().zip(&shown.kept) {
                    for (construction, (kept, total)) in site["constructs"]
                        .as_array_mut()
                        .expect("constructs serialize as a list")
                        .iter_mut()
                        .zip(kept)
                    {
                        if kept < total {
                            construction["lacks"]
                                .as_array_mut()
                                .expect("lacks serialize as a list")
                                .truncate(*kept);
                            construction["omittedLacks"] = json!(total - kept);
                        }
                    }
                }
                document["coverage"]["gaps"]
                    .as_array_mut()
                    .expect("gaps serialize as a list")
                    .truncate(shown.gaps);
                write_displayed(writer, &document, shown.display(), pretty)
            }
            ConstructsAnswer::Unselected(selection) => match &selection.outcome {
                SelectionFailure::Ambiguous { candidates: all } => {
                    let returned = candidates.shown(all.len());
                    let Some(display) = candidates.display(all.len(), returned) else {
                        return write_document(writer, answer, pretty);
                    };
                    let mut shown = selection.clone();
                    if let SelectionFailure::Ambiguous { candidates } = &mut shown.outcome {
                        candidates.truncate(returned);
                    }
                    write_displayed(writer, &shown, Some(display), pretty)
                }
                SelectionFailure::NotFound { .. } => write_document(writer, answer, pretty),
            },
        }),
        text: Some(Box::new(move || match answer {
            ConstructsAnswer::Selected(result) => text(result, sites, lacks, gaps),
            ConstructsAnswer::Unselected(selection) => {
                let project = &selection.project;
                let subject = subject(project, &selection.selection);
                match &selection.outcome {
                    SelectionFailure::NotFound { .. } => format!(
                        "{}: no class, interface or type alias matches {subject}",
                        project.display()
                    ),
                    SelectionFailure::Ambiguous { candidates: all } => {
                        let returned = candidates.shown(all.len());
                        let mut text = format!(
                            "{}: {subject} matches {} declarations; select one with {} FILE:LINE:COLUMN",
                            project.display(),
                            all.len(),
                            FlagId::At
                        );
                        for candidate in &all[..returned] {
                            let anchor = &candidate.anchor;
                            write!(
                                text,
                                "\n  {}:{}:{} {}",
                                super::navigation::place(Some(project), &anchor.file),
                                anchor.line,
                                anchor.column,
                                candidate.selector.as_deref().unwrap_or("(position only)")
                            )
                            .unwrap();
                        }
                        candidates.end(&mut text, all.len(), returned, None, None);
                        text
                    }
                }
            }
        })),
    }
}

/// What the caps show of a result: its first `sites`, of each shown
/// construction `(kept, total)` lacking properties, and its first `gaps`.
struct Shown<'c> {
    sites: usize,
    kept: Vec<Vec<(usize, usize)>>,
    gaps: usize,
    site_cap: Cap<'c>,
    lack_cap: Cap<'c>,
    gap_cap: Cap<'c>,
    total_sites: usize,
    total_gaps: usize,
}

impl<'c> Shown<'c> {
    fn new(
        result: &ConstructsResult,
        site_cap: Cap<'c>,
        lack_cap: Cap<'c>,
        gap_cap: Cap<'c>,
    ) -> Self {
        let sites = site_cap.shown(result.sites.len());
        let kept = result.sites[..sites]
            .iter()
            .map(|site| {
                site.constructs
                    .iter()
                    .map(|construction| {
                        let total = construction.lacks.len();
                        (lack_cap.shown(total), total)
                    })
                    .collect()
            })
            .collect();
        let total_gaps = result.coverage.gaps.len();
        Self {
            sites,
            kept,
            gaps: gap_cap.shown(total_gaps),
            site_cap,
            lack_cap,
            gap_cap,
            total_sites: result.sites.len(),
            total_gaps,
        }
    }

    /// Lacking properties shown and listed, across the shown sites.
    fn lacks(&self) -> (usize, usize) {
        self.kept
            .iter()
            .flatten()
            .fold((0, 0), |(kept, total), (k, t)| (kept + k, total + t))
    }

    fn cut(&self) -> bool {
        let (kept, total) = self.lacks();
        self.sites < self.total_sites || kept < total || self.gaps < self.total_gaps
    }

    /// One `display` member naming each list the caps cut.
    fn display(&self) -> Option<Value> {
        let (kept, total) = self.lacks();
        let mut display: Option<Value> = None;
        for (cut, name) in [
            (self.site_cap.display(self.total_sites, self.sites), "sites"),
            (self.lack_cap.display(total, kept), "lacks"),
            (self.gap_cap.display(self.total_gaps, self.gaps), "gaps"),
        ] {
            match (&mut display, cut) {
                (Some(display), Some(cut)) => display[name] = cut[name].clone(),
                (None, Some(cut)) => display = Some(cut),
                (_, None) => {}
            }
        }
        display
    }
}

/// The header, `CONFIG: SUBJECT N sites in K files (FILTERS); [E of] M
/// literals examined; coverage STATUS (G gaps: N REASON, …)` with the
/// target's declaration; each shown site; each shown gap; the literals the
/// universe left unexamined and the flag that examines them; and the cuts,
/// last.
fn text(result: &ConstructsResult, sites: Cap<'_>, lacks: Cap<'_>, gaps: Cap<'_>) -> String {
    let project = &result.project;
    let shown = Shown::new(result, sites, lacks, gaps);
    let mut files: Vec<&str> = result
        .sites
        .iter()
        .map(|site| site.location.file.as_str())
        .collect();
    files.sort_unstable();
    files.dedup();
    let mut filters = Vec::new();
    if let Some(key) = &result.filters.missing {
        let key = TypeKey::String { name: key.clone() };
        filters.push(format!("missing {}", super::types::key_text(&key)));
    }
    if result.filters.exclude_tests {
        filters.push("outside test paths".to_owned());
    }
    if let Some(file) = &result.filters.file {
        filters.push(format!("in {}", file_text(project, file)));
    }
    let coverage = &result.coverage;
    let examined = if coverage.examined == coverage.literals {
        count(coverage.literals, "literal", "literals")
    } else {
        format!(
            "{} of {}",
            coverage.examined,
            count(coverage.literals, "literal", "literals")
        )
    };
    let mut text = format!(
        "{}: {} {}{} in {}; {examined} examined; coverage {}",
        project.display(),
        subject(project, &result.selection),
        count(
            result.sites.len(),
            "construction site",
            "construction sites"
        ),
        if filters.is_empty() {
            String::new()
        } else {
            format!(" ({})", filters.join(", "))
        },
        count(files.len(), "file", "files"),
        spelled(coverage.status),
    );
    let mut reasons: Vec<(String, usize)> = Vec::new();
    for gap in &result.coverage.gaps {
        let reason = spelled(gap.reason);
        match reasons.iter_mut().find(|(name, _)| *name == reason) {
            Some((_, count)) => *count += 1,
            None => reasons.push((reason, 1)),
        }
    }
    if !reasons.is_empty() {
        reasons.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let reasons: Vec<String> = reasons
            .iter()
            .map(|(reason, count)| format!("{count} {reason}"))
            .collect();
        write!(
            text,
            " ({}: {})",
            count(result.coverage.gaps.len(), "gap", "gaps"),
            reasons.join(", ")
        )
        .unwrap();
    }
    write!(
        text,
        "\ntarget {}, declared at {}",
        result.target.summary.summary,
        location_text(project, &result.target.location)
    )
    .unwrap();
    for (site, kept) in result.sites[..shown.sites].iter().zip(&shown.kept) {
        site_text(&mut text, result, site, kept);
    }
    for gap in &result.coverage.gaps[..shown.gaps] {
        gap_text(&mut text, project, gap);
    }
    match coverage.literals - coverage.examined {
        0 => {}
        1 => write!(
            text,
            "\n1 literal naming only keys the target does not declare was not examined; examine it with {}",
            FlagId::AllLiterals
        )
        .unwrap(),
        skipped => write!(
            text,
            "\n{skipped} literals naming only keys the target does not declare were not examined; examine them with {}",
            FlagId::AllLiterals
        )
        .unwrap(),
    }
    let (kept, total) = shown.lacks();
    lacks.end(&mut text, total, kept, Some("across the sites shown"), None);
    gaps.end(&mut text, shown.total_gaps, shown.gaps, None, None);
    sites.end(&mut text, result.sites.len(), shown.sites, None, None);
    text
}

/// `FILE:LINE:COLUMN lacks "a"?, "b" (unknown)`: keys are quoted, and one
/// marked `?` is optional for the constructed type. The constructed
/// alternative is named
/// when it is not the target as written, and the contextual type when it has
/// other alternatives.
fn site_text(
    text: &mut String,
    result: &ConstructsResult,
    site: &ConstructionSite,
    kept: &[(usize, usize)],
) {
    write!(text, "\n{}", location_text(&result.project, &site.location)).unwrap();
    let only = site.constructs.len() == 1
        && site.constructs[0].summary.summary == site.contextual_type.summary;
    if !only {
        write!(text, " in {}", site.contextual_type.summary).unwrap();
    }
    for (index, (construction, &(shown, total))) in site.constructs.iter().zip(kept).enumerate() {
        if index > 0 {
            text.push(';');
        }
        if site.constructs.len() > 1
            || construction.summary.summary != result.target.summary.summary
        {
            write!(text, " as {}", construction.summary.summary).unwrap();
        }
        if total == 0 {
            text.push_str(" lacks nothing");
            continue;
        }
        text.push_str(" lacks ");
        let names: Vec<String> = construction.lacks[..shown].iter().map(lack_text).collect();
        text.push_str(&names.join(", "));
        if shown < total {
            write!(text, ", … {} more", total - shown).unwrap();
        }
    }
}

fn lack_text(lack: &Lack) -> String {
    format!(
        "{}{}{}",
        super::types::key_text(&lack.key),
        if lack.optional { "?" } else { "" },
        match lack.presence {
            Presence::Absent => "",
            Presence::Unknown => " (unknown)",
        }
    )
}

fn subject(project: &std::path::Path, selection: &TypeQuery) -> String {
    match selection {
        TypeQuery::Name { name, file: None } => format!("`{name}`"),
        TypeQuery::Name {
            name,
            file: Some(file),
        } => format!("`{name}` in {}", file_text(project, file)),
        TypeQuery::At { file, position } => format!(
            "at {}:{}:{}",
            file_text(project, file),
            position.line,
            position.column
        ),
    }
}
