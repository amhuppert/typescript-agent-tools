//! Trivia and insertion seams: where an inserted fragment goes relative to
//! the comments around its anchor, which statement terminators separate it
//! from its neighbors, how its lines are laid out in the surrounding
//! indentation and line-ending style, and whether the comments TypeScript
//! gives meaning to (comment directives and JSDoc) keep governing what they
//! governed. Policies ask for a seam; preparation lays it out and verifies
//! its attachments on the final text. Nothing else scans comments, guesses
//! semicolons, or decides attachment.
//!
//! Layout and placement are product rules: same-line trailing comments stay
//! with the statement before a seam; later comments stay with what follows.
//! Attachment follows TypeScript's rules through their owners: directive
//! reach is `pragmas::DirectiveCoverage`, and JSDoc collection is
//! `lexer::jsdoc_comments`.

use super::compare::Extension;
use super::compose::{Change, Composition, Origin};
use crate::lexer::{self, Lexer};
use crate::pragmas::{self, DirectiveCoverage};
use crate::source::SourceText;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span};

/// The changes of one seam, and the protected statement an inserted
/// terminator now ends.
#[derive(Debug, Clone)]
pub(super) struct Seam {
    /// The seam's changes, the one carrying the fragment last.
    changes: Vec<Placed>,
    /// The statement, and its original end, where a `;` was inserted.
    terminated: Option<(NodeId, usize)>,
}

/// The statement terminator a seam inserts.
const TERMINATOR: &str = ";";

impl Seam {
    /// The seam's changes, as placed.
    pub(super) fn changes(&self) -> impl Iterator<Item = &Change> {
        self.changes.iter().map(|placed| &placed.change)
    }

    /// Where the change carrying the fragment, with what separates it, lies
    /// in `composition`: the region that whatever fills the edit's slot
    /// must lie within.
    pub(super) fn fill_region(&self, composition: &Composition) -> Span {
        let fragment = self.changes.last().expect("a seam carries its fragment");
        composition.region_of(&fragment.change).composed
    }

    /// The protected statement a `;` inserted at its end now ends, and
    /// where it ends in `composition`.
    pub(super) fn extension(&self, composition: &Composition) -> Option<Extension> {
        let (node, end) = self.terminated?;
        let before = composition
            .map(end, super::compose::Side::Before)
            .expect("a statement's end is preserved");
        Some(Extension {
            node,
            end: before + TERMINATOR.len(),
        })
    }
}

/// One change, with the layout of the fragment it carries, if any.
#[derive(Debug, Clone)]
pub(super) struct Placed {
    pub(super) change: Change,
    layout: Option<Layout>,
    /// Where the change's text holds a probe: a line break, indentation,
    /// and `;` after the fragment, whose parse in the draft shows whether
    /// the fragment's last statement needs a terminator (`settle`).
    probe: Option<Span>,
}

/// How to lay out a fragment inside a change's text.
#[derive(Debug, Clone)]
struct Layout {
    /// Where the fragment lies in the change's text.
    fragment: Span,
    /// The indentation of each of the fragment's lines.
    indent: String,
    /// The fragment's first line: at the start of a line, which the change
    /// itself indents, or inline after preserved text.
    first_line: FirstLine,
    eol: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FirstLine {
    AtLineStart,
    Inline,
}

/// A fragment without the whitespace at its edges that layout replaces:
/// all whitespace after it, and before it either all whitespace (an inline
/// first line) or the blank lines before its first line, keeping that
/// line's indentation, to which its other lines are relative.
fn trim(fragment: &str, first_line: FirstLine) -> &str {
    let fragment = fragment.trim_end_matches(lexer::whitespace);
    match first_line {
        FirstLine::Inline => fragment.trim_start_matches(lexer::whitespace),
        FirstLine::AtLineStart => {
            let content = fragment.len() - fragment.trim_start_matches(lexer::whitespace).len();
            let line_start = crate::source::line_breaks(&fragment[..content])
                .last()
                .map_or(0, |line_break| line_break.end);
            let fragment = &fragment[line_start..];
            // One line's common indentation is its own, which layout would
            // remove from a fragment of several lines.
            if crate::source::line_breaks(fragment).next().is_none() {
                fragment.trim_start_matches([' ', '\t'])
            } else {
                fragment
            }
        }
    }
}

/// The line-ending style at `at`: the line break that ends its line, else
/// the text's first line break, else LF.
fn eol(text: &str, at: usize) -> &'static str {
    let style = |found: &str| match found {
        "\r\n" => "\r\n",
        "\r" => "\r",
        "\u{2028}" => "\u{2028}",
        "\u{2029}" => "\u{2029}",
        _ => "\n",
    };
    crate::source::line_breaks(&text[at..])
        .next()
        .map(|range| style(&text[at + range.start..at + range.end]))
        .or_else(|| {
            crate::source::line_breaks(text)
                .next()
                .map(|range| style(&text[range]))
        })
        .unwrap_or("\n")
}

/// The indentation of the line holding `at`: its leading spaces and tabs.
fn indentation(text: &str, at: usize) -> &str {
    let start = crate::source::line_breaks(&text[..at])
        .last()
        .map_or(0, |line_break| line_break.end);
    let line = &text[start..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// Whether a statement starting with the token at `at` could continue an
/// expression on the line before it, so that a statement ended only by a
/// line break would absorb it: `(`, `[`, a template, `+`, `-`, `/` (a
/// regular expression), or `<` (type arguments or a comparison).
/// TypeScript ends a statement at a line break (`canParseSemicolon`) only
/// once the expression before it stops, and these tokens continue one; no
/// other statement start does, including `++`, `--`, `!`, and `.5`, nor
/// does any start continue a type. The pinned syntax API confirms the set.
fn continues_expression(text: &str, at: usize) -> bool {
    let token = Lexer { src: text, pos: at }.next();
    token.end > token.start
        && !text[token.start..].starts_with("++")
        && !text[token.start..].starts_with("--")
        && matches!(
            text[token.start..].chars().next(),
            Some('(' | '[' | '`' | '+' | '-' | '/' | '<')
        )
}

/// Whether automatic semicolon insertion may end a statement: it has no
/// final `;` and is not closed by its own body or member list.
fn ends_open(ast: &Ast<'_>, statement: NodeId) -> bool {
    let node = ast.node(statement);
    if ast.source()[..node.span.end].ends_with(';') {
        return false;
    }
    let body = crate::syntax::id(node, "body").is_some();
    !match node.syntax_kind {
        SyntaxKind::ClassDeclaration
        | SyntaxKind::InterfaceDeclaration
        | SyntaxKind::EnumDeclaration => true,
        SyntaxKind::FunctionDeclaration | SyntaxKind::ModuleDeclaration => body,
        _ => false,
    }
}

/// The body of a callable, replaced by `fragment`, laid out after the
/// header that precedes it on its line.
pub(super) fn body(ast: &Ast<'_>, body: NodeId, fragment: &str, edit: usize) -> Seam {
    let text = ast.source();
    let span = ast.token_span(body);
    let fragment = trim(fragment, FirstLine::Inline);
    Seam {
        changes: vec![Placed {
            change: Change {
                span,
                text: fragment.to_owned(),
                edit,
            },
            layout: Some(Layout {
                fragment: Span {
                    start: 0,
                    end: fragment.len(),
                },
                indent: indentation(text, span.start).to_owned(),
                first_line: FirstLine::Inline,
                eol: eol(text, span.start),
            }),
            probe: None,
        }],
        terminated: None,
    }
}

/// Statements inserted after `statement`, a member of a statement list:
/// after the comments that trail it on its line, which stay with it, and
/// before the comments, JSDoc, and directives on later lines, which stay
/// with what follows. The fragment starts a line of its own at the
/// statement's indentation, and anything after the seam on that line moves
/// to a line of its own after it. A `;` ends the statement, or the
/// fragment, where the other side could otherwise continue it.
pub(super) fn after(ast: &Ast<'_>, statement: NodeId, fragment: &str, edit: usize) -> Seam {
    let text = ast.source();
    let end = ast.node(statement).span.end;
    let mut seam = end;
    for comment in lexer::comments(text, end) {
        if comment.space_before.contains(lexer::line_break) {
            break;
        }
        seam = comment.start + comment.text.len();
    }
    let fragment = trim(fragment, FirstLine::AtLineStart);
    let indent = indentation(text, ast.token_span(statement).start).to_owned();
    let eol = eol(text, seam);
    let rest = &text[seam..];
    let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let follower = !rest[spaces..].is_empty() && !rest[spaces..].starts_with(lexer::line_break);
    let terminate =
        has_token(fragment) && ends_open(ast, statement) && continues_expression(fragment, 0);
    let mut changes = Vec::new();
    let mut inserted = String::new();
    if terminate {
        if seam == end {
            inserted.push_str(TERMINATOR);
        } else {
            changes.push(Placed {
                change: Change {
                    span: Span { start: end, end },
                    text: TERMINATOR.into(),
                    edit,
                },
                layout: None,
                probe: None,
            });
        }
    }
    // A blank line after the anchor, as between declarations, separates the
    // insertion from it too; the one already there separates it from what
    // follows.
    let blank = !follower
        && crate::source::line_breaks(
            &rest[..rest.len() - rest.trim_start_matches(lexer::whitespace).len()],
        )
        .nth(1)
        .is_some();
    inserted.push_str(eol);
    if blank {
        inserted.push_str(eol);
    }
    inserted.push_str(&indent);
    let start = inserted.len();
    inserted.push_str(fragment);
    let layout = Layout {
        fragment: Span {
            start,
            end: inserted.len(),
        },
        indent: indent.clone(),
        first_line: FirstLine::AtLineStart,
        eol,
    };
    // What follows could continue the fragment's last statement: probe
    // whether it needs a terminator, on a line of its own, where no comment
    // in the fragment can hide it.
    let probe = (has_token(fragment) && continues_expression(text, seam + spaces)).then(|| {
        let start = inserted.len();
        inserted.push_str(eol);
        inserted.push_str(&indent);
        inserted.push_str(TERMINATOR);
        Span {
            start,
            end: inserted.len(),
        }
    });
    if follower {
        inserted.push_str(eol);
        inserted.push_str(&indent);
    }
    changes.push(Placed {
        change: Change {
            span: Span {
                start: seam,
                end: if follower { seam + spaces } else { seam },
            },
            text: inserted,
            edit,
        },
        layout: Some(layout),
        probe,
    });
    Seam {
        changes,
        terminated: terminate.then_some((statement, end)),
    }
}

/// Whether `text` holds a token, rather than only whitespace and comments.
fn has_token(text: &str) -> bool {
    let token = Lexer { src: text, pos: 0 }.next();
    token.end > token.start
}

/// Whether settling the seams needs a parsed draft: for the literal and
/// comment boundaries of a fragment that spans lines, or for a probe.
pub(super) fn needs_draft(seams: &[Seam]) -> bool {
    seams.iter().flat_map(|seam| &seam.changes).any(|placed| {
        placed.probe.is_some()
            || placed.layout.as_ref().is_some_and(|layout| {
                crate::source::line_breaks(
                    &placed.change.text[layout.fragment.start..layout.fragment.end],
                )
                .next()
                .is_some()
            })
    })
}

/// The final changes of `seams`, given `draft`, the composition of the
/// changes as placed, and `tree`, its final syntax tree.
///
/// A probe is resolved and removed: when it terminated the fragment's last
/// statement, a `;` goes right after that statement's last token, before any
/// comment after it; when it was a statement of its own, the fragment needs
/// none; otherwise final validation judges the fragment as supplied.
///
/// Then each line of a fragment after its first, and an at-line-start first
/// line, moves from the fragment's common indentation to the seam's; each
/// line break becomes the seam's style; blank lines lose their spaces. A
/// line that begins inside a literal token (string, template, regular
/// expression, or JSX text) or a comment keeps its bytes and the line break
/// before it: a line prefix inside a literal is data, not indentation.
pub(super) fn settle(draft: &Composition, tree: &Ast<'_>, seams: &[Seam]) -> Vec<Change> {
    let literals = tree.literal_token_spans();
    let comments: Vec<Span> = lexer::comment_spans(draft.text(), literals.clone()).collect();
    let mut protected: Vec<Span> = literals
        .into_iter()
        .chain(comments.iter().copied())
        .collect();
    protected.sort_by_key(|span| span.start);
    // Literal tokens and comments are disjoint, so only the last one
    // starting at or before an offset can hold it. JSX text may begin with
    // a line break, which is then its first byte.
    let inside = |offset: usize| {
        let index = protected.partition_point(|span| span.start <= offset);
        index > 0 && offset < protected[index - 1].end
    };
    seams
        .iter()
        .flat_map(|seam| &seam.changes)
        .map(|placed| {
            let Some(layout) = &placed.layout else {
                return placed.change.clone();
            };
            let region = draft.region_of(&placed.change);
            let base = region.composed.start + layout.fragment.start;
            let mut fragment =
                placed.change.text[layout.fragment.start..layout.fragment.end].to_owned();
            // Where a terminator goes, as an offset in the fragment.
            let terminator = placed.probe.and_then(|probe| {
                let at = region.composed.start + probe.end - TERMINATOR.len();
                terminator(tree, at, base, &comments).map(|end| end - base)
            });
            if let Some(at) = terminator {
                fragment.insert_str(at, TERMINATOR);
            }
            let laid_out = lay_out_fragment(&fragment, layout, |offset| match terminator {
                Some(at) if offset >= at + TERMINATOR.len() => {
                    inside(base + offset - TERMINATOR.len())
                }
                Some(at) if offset >= at => false,
                _ => inside(base + offset),
            });
            let mut text = placed.change.text[..layout.fragment.start].to_owned();
            text.push_str(&laid_out);
            let after = match placed.probe {
                Some(probe) => &placed.change.text[probe.end..],
                None => &placed.change.text[layout.fragment.end..],
            };
            text.push_str(after);
            Change {
                text,
                ..placed.change.clone()
            }
        })
        .collect()
}

/// Where a terminator goes for the probe `;` at `probe` of `tree`'s text,
/// whose fragment begins at `fragment`: right after the last token before
/// the probe, skipping `comments`, when that token is in the fragment and
/// the probe terminated a statement, as an optional `;` of whatever
/// statement ends there (a `do` loop's or an unbraced body's included). A
/// probe that forms an empty statement needs none: of its own when the
/// fragment was already terminated, or as the body of an incomplete one
/// such as `if (x)`, which final validation then rejects.
fn terminator(tree: &Ast<'_>, probe: usize, fragment: usize, comments: &[Span]) -> Option<usize> {
    let mut current = Some(tree.root_id());
    while let Some(id) = current.take() {
        let node = tree.node(id);
        if node.syntax_kind == SyntaxKind::EmptyStatement
            && tree.token_span_of(node)
                == (Span {
                    start: probe,
                    end: probe + 1,
                })
        {
            return None;
        }
        current = tree.children(id).find(|&child| {
            let span = tree.node(child).span;
            span.start <= probe && probe < span.end
        });
    }
    let text = tree.source();
    let mut at = probe;
    loop {
        at = text[..at].trim_end_matches(lexer::whitespace).len();
        let ending = comments.partition_point(|comment| comment.end < at);
        match comments.get(ending) {
            Some(comment) if comment.end == at => at = comment.start,
            _ => break,
        }
    }
    (at > fragment).then_some(at)
}

/// `lay_out` for one fragment, where `protected` says whether an offset in
/// the fragment lies inside a literal or comment.
fn lay_out_fragment(fragment: &str, layout: &Layout, protected: impl Fn(usize) -> bool) -> String {
    struct Line<'f> {
        /// The line break before the line, if it is laid out.
        before: Option<&'f str>,
        text: &'f str,
        laid_out: bool,
    }
    let mut lines = Vec::new();
    let mut start = 0;
    let mut before: Option<&str> = None;
    let mut laid_out = layout.first_line == FirstLine::AtLineStart;
    for line_break in crate::source::line_breaks(fragment) {
        lines.push(Line {
            before,
            text: &fragment[start..line_break.start],
            laid_out,
        });
        laid_out = !protected(line_break.start);
        before = Some(&fragment[line_break.clone()]);
        start = line_break.end;
    }
    lines.push(Line {
        before,
        text: &fragment[start..],
        laid_out,
    });
    let indentation = |line: &str| line.len() - line.trim_start_matches([' ', '\t']).len();
    let blank = |line: &str| line.trim_matches(lexer::whitespace).is_empty();
    // The longest common leading run of spaces and tabs of the laid-out,
    // non-blank lines (only those after the first for an inline one).
    let common = lines
        .iter()
        .enumerate()
        .filter(|(index, line)| {
            line.laid_out
                && !blank(line.text)
                && (*index > 0 || layout.first_line == FirstLine::AtLineStart)
        })
        .map(|(_, line)| &line.text[..indentation(line.text)])
        .reduce(|common, indent| {
            let shared = common
                .bytes()
                .zip(indent.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            &common[..shared]
        })
        .unwrap_or("")
        .len();
    let mut text = String::with_capacity(fragment.len());
    for (index, line) in lines.iter().enumerate() {
        match line.before {
            Some(_) if line.laid_out => text.push_str(layout.eol),
            Some(line_break) => text.push_str(line_break),
            None => {}
        }
        if !line.laid_out || index == 0 && layout.first_line == FirstLine::Inline {
            text.push_str(line.text);
        } else if blank(line.text) {
        } else {
            if index > 0 {
                text.push_str(&layout.indent);
            }
            text.push_str(&line.text[common.min(indentation(line.text))..]);
        }
    }
    text
}

/// What the comments TypeScript gives meaning to govern in the original
/// text, recorded before an edit and verified on its final text.
#[derive(Debug)]
pub(super) struct Attachments {
    coverage: DirectiveCoverage,
}

/// An edit that would change what a comment governs, at an original offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Violation {
    pub(super) original: usize,
    /// The edit whose change causes it.
    pub(super) edit: Option<usize>,
    pub(super) message: String,
}

/// The lines comment directives cover in `tree`'s text, `source`.
fn coverage(tree: &Ast<'_>, source: &SourceText) -> DirectiveCoverage {
    DirectiveCoverage::new(
        source,
        pragmas::comment_directives(tree.source(), tree.literal_token_spans()),
    )
}

/// The JSDoc comments a node starting at the token at `token` collects
/// (`lexer::jsdoc_comments`), or `None` when no node starts there.
fn jsdoc(tree: &Ast<'_>, token: usize) -> Option<Vec<Span>> {
    let mut full_start = None;
    let mut trailing = false;
    let mut current = Some(tree.root_id());
    while let Some(id) = current.take() {
        let node = tree.node(id);
        if tree.token_span_of(node).start == token && node.syntax_kind != SyntaxKind::SourceFile {
            full_start = Some(node.span.start);
            trailing |= crate::syntax::trailing_jsdoc(node.syntax_kind);
        }
        current = tree.children(id).find(|&child| {
            let span = tree.node(child).span;
            span.start <= token && token < span.end.max(span.start + 1)
        });
    }
    let full_start = full_start?;
    Some(
        lexer::jsdoc_comments(tree.source(), full_start, token, trailing)
            .map(|comment| Span {
                start: comment.start,
                end: comment.start + comment.text.len(),
            })
            .collect(),
    )
}

impl Attachments {
    /// Record what the comments of `tree`, parsed from `source`, govern.
    pub(super) fn record(tree: &Ast<'_>, source: &SourceText) -> Self {
        Self {
            coverage: coverage(tree, source),
        }
    }

    /// Verify, on `composed`, the final tree of `composition`'s text, that
    /// comment directives cover exactly the preserved lines they covered,
    /// and that the first preserved node after each change collects exactly
    /// the JSDoc comments it collected. A new directive or JSDoc comment may
    /// govern only new syntax.
    pub(super) fn verify(
        &self,
        original: &Ast<'_>,
        source: &SourceText,
        composition: &Composition,
        composed: &Ast<'_>,
    ) -> Result<(), Violation> {
        let composed_source = SourceText::new(composition.text().to_owned());
        let composed_coverage = coverage(composed, &composed_source);
        let text = source.as_str();
        // Each preserved stretch of the original, between changes, and its
        // offset in the composed text.
        let mut start = 0;
        let mut stretches = Vec::new();
        for region in composition.regions() {
            stretches.push((start, region.change.span.start));
            start = region.change.span.end;
        }
        stretches.push((start, text.len()));
        for (start, end) in stretches {
            let Some(shift) = composition
                .map(start, super::compose::Side::After)
                .map(|mapped| mapped as isize - start as isize)
            else {
                continue;
            };
            if start >= end {
                continue;
            }
            for line in source.line_index(start)..source.line_count() {
                let line_start = source.line_start(line);
                if line_start >= end {
                    break;
                }
                let from = line_start.max(start);
                let to = if line + 1 < source.line_count() {
                    source.line_start(line + 1).min(end)
                } else {
                    end
                };
                if !text[from..to].contains(|c| !lexer::whitespace(c)) {
                    continue;
                }
                let composed_line = composed_source.line_index((from as isize + shift) as usize);
                let before = self.coverage.governing(line);
                let after =
                    composed_coverage.governing(composed_line).map(|start| {
                        match composition.origin(start) {
                            Origin::Preserved(start) => Some(start),
                            Origin::Changed { .. } => None,
                        }
                    });
                let message = match (before, after) {
                    (Some(before), Some(Some(after))) if before == after => continue,
                    (None, None) => continue,
                    (Some(_), None) => {
                        "would no longer be governed by the comment directive (@ts-ignore or @ts-expect-error) that governed it"
                    }
                    (None, Some(_)) => {
                        "would become governed by a comment directive (@ts-ignore or @ts-expect-error)"
                    }
                    (Some(_), Some(_)) => {
                        "would be governed by another comment directive (@ts-ignore or @ts-expect-error) than the one that governed it"
                    }
                };
                // The change nearest before the line, or else the first.
                let regions = composition.regions();
                let nearest = regions
                    .iter()
                    .rev()
                    .find(|region| region.change.span.start <= from)
                    .or(regions.first());
                return Err(Violation {
                    original: from,
                    edit: nearest.map(|region| region.change.edit),
                    message: format!("preserved line {} {message}", line + 1),
                });
            }
        }
        // The first token after each change, when it is preserved: the
        // trivia before it, and so the JSDoc it collects, may have changed.
        let mut checked = Vec::new();
        for region in composition.regions() {
            let composed_next = Lexer {
                src: composition.text(),
                pos: region.composed.end,
            }
            .next()
            .start;
            let Origin::Preserved(next) = composition.origin(composed_next) else {
                continue;
            };
            if checked.contains(&next) {
                continue;
            }
            checked.push(next);
            let (Some(before), Some(after)) =
                (jsdoc(original, next), jsdoc(composed, composed_next))
            else {
                continue;
            };
            let after: Vec<Option<Span>> = after
                .into_iter()
                .map(|span| match composition.origin(span.start) {
                    Origin::Preserved(start) => Some(Span {
                        start,
                        end: start + (span.end - span.start),
                    }),
                    Origin::Changed { .. } => None,
                })
                .collect();
            let message = if after.contains(&None) {
                "a JSDoc comment in the fragment would document the preserved syntax that follows it"
            } else if after.into_iter().flatten().collect::<Vec<_>>() != before {
                "the preserved syntax here would no longer be documented by the JSDoc comments it had"
            } else {
                continue;
            };
            return Err(Violation {
                original: next,
                edit: Some(region.change.edit),
                message: message.into(),
            });
        }
        Ok(())
    }
}
