//! Preparation of every edit to one captured text, the one construction of
//! a file's proposed text: one parse of the original, selection of each
//! edit's site in it, the operation policies, composition of all changes
//! together, at most one draft parse for layout, and one parse of the final
//! text, which must keep every protected syntax and comment relationship.
//! Preparing from a site's anchor instead of its name prepares the same
//! text again, which is how a plan is loaded.

use super::compare::{self, Extension, Slot};
use super::compose::{self, Composition, Origin};
use super::policy::{self, Opening};
use super::seam::{self, Attachments, Seam};
use super::{
    Change, Operation, Outcome, PlannedEdit, PreparedText, Rejection, RejectionKind, Unselected,
};
use crate::analysis::{AnalysisError, InputError, Parser};
use crate::navigation::{self, Selector, Syntax};
use crate::source::{PositionError, SourceText};
use crate::{Ast, Diagnostic, NodeId};
use std::path::Path;
use std::sync::Arc;

/// One edit to prepare in a file.
#[derive(Debug, Clone)]
pub(super) struct Request<'a> {
    /// The edit's position in its request.
    pub(super) edit: usize,
    pub(super) operation: Operation,
    pub(super) selector: Selector,
    pub(super) fragment: &'a str,
}

/// Why a file's edits could not be prepared.
#[derive(Debug)]
pub(super) enum Failure {
    /// The original text does not parse (`InputError::unparsable`).
    Original(InputError),
    /// A selector's position addresses no position in the file.
    Position(PositionError, Selector),
    Unselected {
        edit: usize,
        query: Selector,
        outcome: Box<navigation::ShowOutcome>,
    },
    Rejected(Box<Rejection>),
}

impl Failure {
    /// The failure as an operation reports it, for the file at `path`
    /// named `display` in results.
    pub(super) fn into_outcome<T>(
        self,
        path: &Path,
        display: &str,
    ) -> Result<Outcome<T>, AnalysisError> {
        match self {
            Self::Original(error) => Err(error.into()),
            Self::Position(error, selector) => {
                Err(navigation::invalid_position(error, path, &selector).into())
            }
            Self::Unselected {
                edit,
                query,
                outcome,
            } => Ok(Outcome::Unselected(Unselected {
                edit,
                file: Some(display.to_owned()),
                query,
                outcome: *outcome,
            })),
            Self::Rejected(rejection) => Ok(Outcome::Rejected(*rejection)),
        }
    }
}

/// One planned edit while its file is prepared.
struct Planned<'r> {
    request: &'r Request<'r>,
    site: navigation::Declaration,
    opening: Opening,
    seam: Seam,
}

/// Prepare `requests`, every edit to the file analysis names `name`, whose
/// captured text is `source`; results name the file `display`.
pub(super) fn prepare(
    parser: &Parser,
    name: &str,
    display: &str,
    source: &Arc<SourceText>,
    requests: &[Request<'_>],
) -> Result<PreparedText, Failure> {
    let ast = parser.parse(source.as_str(), name).map_err(|error| {
        Failure::Original(InputError::unparsable(error, source, Path::new(name)))
    })?;
    let syntax = Syntax::new(&ast, Arc::clone(source));
    let reject = |kind, message: String, edit: Option<usize>, at: Option<usize>| {
        Failure::Rejected(Box::new(Rejection {
            kind,
            message,
            edit,
            file: Some(display.to_owned()),
            position: at.map(|offset| source.position(offset)),
            proposed: None,
        }))
    };
    let mut planned = Vec::with_capacity(requests.len());
    for request in requests {
        let catalog = syntax.catalog();
        let selection = catalog
            .select(&request.selector)
            .map_err(|error| Failure::Position(error, request.selector.clone()))?;
        let Some(selected) = selection.single() else {
            return Err(Failure::Unselected {
                edit: request.edit,
                query: request.selector.clone(),
                outcome: Box::new(navigation::outcome(display, catalog, &selection)),
            });
        };
        let site = catalog.selected(selected, display).declaration;
        // A batch is prepared again from each site's anchor, which must
        // select the site alone, whether a record or a statement site.
        if catalog
            .select(&Selector::At(site.anchor))
            .map(|selection| selection.single())
            != Ok(Some(selected))
        {
            return Err(reject(
                RejectionKind::UnsupportedSite,
                "this site's anchor also selects another declaration, so the edit could not be prepared again from it".into(),
                Some(request.edit),
                source.offset(site.anchor).ok(),
            ));
        }
        let (opening, seam) = policy::plan(
            request.operation,
            syntax.ast(),
            syntax.site(selected),
            request.fragment,
            request.edit,
        )
        .map_err(|unsupported| {
            reject(
                RejectionKind::UnsupportedSite,
                format!(
                    "{} cannot {}: {}{}",
                    site.label,
                    match request.operation {
                        Operation::ReplaceBody => "have its body replaced",
                        Operation::InsertAfter => "have statements inserted after it",
                    },
                    unsupported.reason,
                    remedy(&syntax, source, unsupported.enclosing)
                ),
                Some(request.edit),
                source.offset(site.anchor).ok(),
            )
        })?;
        planned.push(Planned {
            request,
            site,
            opening,
            seam,
        });
    }
    let attachments = Attachments::record(&ast, source);
    let seams: Vec<Seam> = planned.iter().map(|planned| planned.seam.clone()).collect();
    let changes: Vec<compose::Change> = seams
        .iter()
        .flat_map(|seam| seam.changes().cloned())
        .collect();
    let draft = Composition::new(source.as_str(), changes.clone()).map_err(|conflict| {
        reject(
            RejectionKind::Conflict,
            format!(
                "edits {} and {} conflict: {}; combine them into one fragment",
                conflict.first + 1,
                conflict.second + 1,
                conflict.reason
            ),
            Some(conflict.second),
            Some(conflict.at.start),
        )
    })?;
    let check = |composition: &Composition, tree: &Ast<'_>| -> Result<PreparedText, Failure> {
        validate(&ast, source, &attachments, &planned, composition, tree)
            .map_err(|(kind, message, edit, at)| reject(kind, message, edit, Some(at)))?;
        Ok(proposal(name, display, source, &planned, composition))
    };
    fn parse<'c>(
        parser: &Parser,
        name: &str,
        composition: &'c Composition,
        source: &SourceText,
        display: &str,
    ) -> Result<Ast<'c>, Failure> {
        parser
            .parse(composition.text(), name)
            .map_err(|error| candidate_failure(error, composition, source, display))
    }
    let parse = |composition| parse(parser, name, composition, source, display);
    if !seam::needs_draft(&seams) {
        return check(&draft, &parse(&draft)?);
    }
    let draft_tree = parse(&draft)?;
    let settled = seam::settle(&draft, &draft_tree, &seams);
    if settled == changes {
        return check(&draft, &draft_tree);
    }
    drop(draft_tree);
    let composition =
        Composition::new(source.as_str(), settled).expect("settling keeps every change's span");
    check(&composition, &parse(&composition)?)
}

/// The remedy a refusal names for `enclosing`, the innermost statement in a
/// statement list around a refused function or arrow function
/// (`policy::Unsupported::enclosing`): the position of its first token,
/// when `--at` there selects one site, which designates that statement;
/// otherwise nothing.
fn remedy(syntax: &Syntax<'_, '_>, source: &SourceText, enclosing: Option<NodeId>) -> String {
    let Some(statement) = enclosing else {
        return String::new();
    };
    let at = source.position(syntax.ast().first_token_span(statement).start);
    match syntax
        .catalog()
        .select(&Selector::At(at))
        .expect("a token's start is a position in its file")
        .single()
    {
        Some(selected) if syntax.site(selected).designation == statement => format!(
            "; to insert after the enclosing statement, select it with --at {}:{}",
            at.line, at.column
        ),
        Some(_) | None => String::new(),
    }
}

/// Run every check of the final text: protected syntax corresponds, each
/// slot's fill is what its operation allows, and comments keep governing
/// what they governed. A failure names its kind, message, edit, and
/// original offset.
fn validate(
    ast: &Ast<'_>,
    source: &SourceText,
    attachments: &Attachments,
    planned: &[Planned<'_>],
    composition: &Composition,
    tree: &Ast<'_>,
) -> Result<(), (RejectionKind, String, Option<usize>, usize)> {
    let slots: Vec<Slot> = planned
        .iter()
        .map(|planned| planned.opening.slot(planned.seam.fill_region(composition)))
        .collect();
    let extensions: Vec<Extension> = planned
        .iter()
        .filter_map(|planned| planned.seam.extension(composition))
        .collect();
    let fills =
        compare::compare(ast, tree, composition, &slots, &extensions).map_err(|mismatch| {
            // Preserved syntax right after a change usually means the
            // change's last statement continues into it.
            let follows = composition.regions().iter().find(|region| {
                crate::lexer::Lexer::new(composition.text(), region.composed.end)
                    .next()
                    .start
                    == mismatch.composed
            });
            let edit = match (composition.origin(mismatch.composed), follows) {
                (Origin::Changed { region }, _) | (_, Some(region)) => Some(region.change.edit),
                (Origin::Preserved(_), None) => None,
            };
            let hint = if follows.is_some() {
                "; the new code before it may be incomplete or run into it"
            } else {
                ""
            };
            (
                RejectionKind::Boundary,
                format!(
                    "the edit would change syntax it must preserve: {}{hint}",
                    mismatch.message
                ),
                edit,
                mismatch.original,
            )
        })?;
    for (planned, fill) in planned.iter().zip(&fills) {
        planned.opening.accepts(tree, fill).map_err(|message| {
            (
                RejectionKind::Boundary,
                message,
                Some(planned.request.edit),
                source
                    .offset(planned.site.anchor)
                    .expect("an anchor is a position in its file"),
            )
        })?;
    }
    attachments
        .verify(ast, source, composition, tree)
        .map_err(|violation| {
            (
                RejectionKind::Attachment,
                violation.message,
                violation.edit,
                violation.original,
            )
        })
}

/// The rejection of a final text that does not parse: a resource limit or
/// a syntax error, located in the proposed text and, where the error lies
/// in preserved text, in the original.
fn candidate_failure(
    error: Diagnostic,
    composition: &Composition,
    source: &SourceText,
    display: &str,
) -> Failure {
    let proposed = SourceText::new(composition.text().to_owned());
    let at = error.span.start.min(composition.text().len());
    let position = proposed.position(at);
    let line = proposed.line_index(at);
    let line_end = if line + 1 < proposed.line_count() {
        proposed.line_start(line + 1)
    } else {
        composition.text().len()
    };
    let line_text = composition.text()[proposed.line_start(line)..line_end]
        .trim_end_matches(crate::lexer::line_break)
        .to_owned();
    let (edit, original) = match composition.origin(at) {
        Origin::Changed { region, .. } => (Some(region.change.edit), None),
        Origin::Preserved(offset) => (None, Some(source.position(offset))),
    };
    let (kind, message) = if crate::parser::exhaustion(&error) {
        (
            RejectionKind::ResourceLimit,
            format!(
                "the proposed file exceeds the parser's nesting budget: {}",
                error.message
            ),
        )
    } else {
        (
            RejectionKind::Syntax,
            format!("the proposed file does not parse: {}", error.message),
        )
    };
    Failure::Rejected(Box::new(Rejection {
        kind,
        message,
        edit,
        file: Some(display.to_owned()),
        position: original,
        proposed: Some((position, line_text)),
    }))
}

/// The text proposal of a validated composition.
fn proposal(
    name: &str,
    display: &str,
    source: &SourceText,
    planned: &[Planned<'_>],
    composition: &Composition,
) -> PreparedText {
    PreparedText {
        path: name.into(),
        display: display.to_owned(),
        original: source.as_str().to_owned(),
        proposed: composition.text().to_owned(),
        edits: planned
            .iter()
            .map(|planned| PlannedEdit {
                index: planned.request.edit,
                operation: planned.request.operation,
                site: planned.site.clone(),
                fragment: planned.request.fragment.to_owned(),
                changes: composition
                    .regions()
                    .iter()
                    .filter(|region| region.change.edit == planned.request.edit)
                    .map(|region| Change {
                        span: region.change.span,
                        start: source.position(region.change.span.start),
                        end: source.position(region.change.span.end),
                        text: region.change.text.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}
