//! One-line signatures from written syntax. Projection chooses which source
//! parts summarize a declaration and which nested bodies to elide; this
//! module only renders them. Between tokens, each run of whitespace and
//! comments becomes one space; literal tokens (strings, templates, regular
//! expressions, JSX text) are copied exactly, or replaced whole by `…` when
//! they span lines; elided bodies become `{ … }`; and text past `LIMIT`
//! characters ends with `…` at a token boundary, never before the name.

use crate::Span;
use crate::lexer;

/// Characters a signature keeps before abbreviating its tail.
pub(super) const LIMIT: usize = 160;

const ELLIPSIS: char = '…';

/// What one signature shows: source `parts`, rendered in order with one
/// space between them, and the end of the declared name, which abbreviation
/// never cuts before.
pub(super) struct Spec {
    pub(super) parts: smallvec::SmallVec<[Span; 2]>,
    pub(super) name_end: usize,
}

/// Render `spec` from `source`, whose literal token spans (sorted) are
/// `literals`. `elided(at)` gives the nested body that starts at `at`, if
/// the signature elides one there; it is asked at increasing positions,
/// and only at those the signature reaches.
pub(super) fn render(
    source: &str,
    literals: &[Span],
    spec: &Spec,
    mut elided: impl FnMut(usize) -> Option<Span>,
) -> String {
    let mut out = Output::default();
    let mut literal = 0;
    for (index, part) in spec.parts.iter().enumerate() {
        if index > 0 {
            out.space = true;
        }
        let mut at = part.start;
        while at < part.end {
            if at >= spec.name_end && out.name_len.is_none() {
                out.name_len = Some(out.boundary());
            }
            literal += literals[literal..].partition_point(|span| span.start < at);
            if let Some(span) = elided(at) {
                out.token("{ … }", true);
                at = span.end.min(part.end);
            } else if let Some(span) = literals.get(literal).filter(|span| span.start == at) {
                let text = &source[span.start..span.end.min(part.end)];
                if text.contains(lexer::line_break) {
                    out.token("…", true);
                } else {
                    out.token(text, true);
                }
                at = span.end.min(part.end);
            } else {
                let c = source[at..].chars().next().expect("a character boundary");
                if lexer::whitespace(c) {
                    out.space = true;
                    at += c.len_utf8();
                } else if let Some(end) = lexer::comment_end(source, at) {
                    out.space = true;
                    at = end.min(part.end);
                } else {
                    let mut buffer = [0; 4];
                    out.token(c.encode_utf8(&mut buffer), false);
                    at += c.len_utf8();
                }
            }
            if out.chars > LIMIT && at >= spec.name_end {
                return out.abbreviated();
            }
        }
    }
    out.text
}

#[derive(Default)]
struct Output {
    text: String,
    chars: usize,
    /// A pending space from trivia, written before the next token.
    space: bool,
    /// The text length after the name, the earliest place to abbreviate.
    name_len: Option<usize>,
    /// The text length at the last token boundary.
    last_boundary: usize,
}

impl Output {
    /// Append token text. `whole` marks text that is a complete token, so a
    /// boundary follows it as well as preceding it.
    fn token(&mut self, text: &str, whole: bool) {
        if self.space && !self.text.is_empty() {
            self.last_boundary = self.text.len();
            self.text.push(' ');
            self.chars += 1;
        } else if whole {
            self.last_boundary = self.text.len();
        }
        self.space = false;
        self.text.push_str(text);
        self.chars += text.chars().count();
        if whole {
            self.last_boundary = self.text.len();
        }
    }

    /// The current length, recorded as a boundary.
    fn boundary(&mut self) -> usize {
        self.last_boundary = self.text.len();
        self.text.len()
    }

    /// The text up to its last token boundary, never before the name, then
    /// `…`. Every boundary follows token text, never a generated space (a
    /// pending space is written only before the next token, after the
    /// boundary that precedes it), so nothing is trimmed: whitespace at the
    /// cut belongs to a retained literal, such as JSX text, and stays.
    fn abbreviated(mut self) -> String {
        let floor = self.name_len.unwrap_or(self.text.len());
        let cut = self.last_boundary.max(floor);
        self.text.truncate(cut);
        self.text.push(ELLIPSIS);
        self.text
    }
}
