//! Captured source text, shared by project loading and file navigation: how
//! a file's contents become analyzed text, positions in it as one-based
//! lines and UTF-16 columns (`SourcePosition`), which every analysis result
//! and failure reports, and the conversions between them and UTF-8 byte
//! spans.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::path::Path;

/// One-based line and one-based UTF-16 column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

/// A UTF-8 byte order mark, U+FEFF.
const BYTE_ORDER_MARK: &[u8] = "\u{FEFF}".as_bytes();

/// Where the text of a file's contents begins: after a leading UTF-8 byte
/// order mark, else at 0.
fn text_start(bytes: &[u8]) -> usize {
    if bytes.starts_with(BYTE_ORDER_MARK) {
        BYTE_ORDER_MARK.len()
    } else {
        0
    }
}

/// The text TypeScript analyzes for a file's contents: port of `decodeBytes`
/// (internal/vfs/internal/internal.go), which `(*Common).ReadFile` applies to
/// every file the compiler reads, sources, configurations, and package
/// manifests alike. A leading UTF-8 byte order mark is not text, so
/// positions, ranges, excerpts, and diagnostics on the first line count from
/// the character after it; a mark anywhere else is text (whitespace in
/// TypeScript). Every analysis capture reads through this rule (`read_text`,
/// and `decode` for contents read as bytes); parse mode, which parses what it
/// is given, keeps the mark in its AST.
///
/// Not ported: `decodeBytes` also decodes UTF-16 contents after a UTF-16
/// byte order mark. Here such contents are not UTF-8, so they fail to
/// decode, like any other contents that are not UTF-8.
pub(crate) fn decode(bytes: &[u8]) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(&bytes[text_start(bytes)..])
}

/// The encoding prefix of a file's contents, which is not analyzed text:
/// a leading byte order mark, else nothing (`decode`).
pub(crate) fn prefix(bytes: &[u8]) -> &[u8] {
    &bytes[..text_start(bytes)]
}

/// The contents whose analyzed text is `text`, after a byte order mark when
/// `marked`. Spans in analyzed text map to raw bytes by that prefix alone,
/// so editing never computes the offset itself.
pub(crate) fn encode(marked: bool, text: &str) -> Vec<u8> {
    let prefix: &[u8] = if marked { BYTE_ORDER_MARK } else { b"" };
    let mut bytes = Vec::with_capacity(prefix.len() + text.len());
    bytes.extend_from_slice(prefix);
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

/// The contents of a file whose contents `original` were, after an edit
/// made their analyzed text `text`: `original`'s prefix, then `text`.
pub(crate) fn encode_like(original: &[u8], text: &str) -> Vec<u8> {
    encode(!prefix(original).is_empty(), text)
}

/// Read a file and `decode` its contents. Contents that are not UTF-8 are
/// an `InvalidData` error, as from `std::fs::read_to_string`.
pub(crate) fn read_text(path: &Path) -> io::Result<String> {
    decode_text(std::fs::read(path)?)
}

/// The text of a file read as `bytes`, as `read_text` reads it.
pub(crate) fn decode_text(mut bytes: Vec<u8>) -> io::Result<String> {
    bytes.drain(..text_start(&bytes));
    String::from_utf8(bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        )
    })
}

/// The ECMAScript line breaks in `text` as byte ranges, in order. CRLF is
/// one break; CR, LF, U+2028, and U+2029 are also breaks.
pub(crate) fn line_breaks(text: &str) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
    let mut from = 0;
    std::iter::from_fn(move || {
        let at = from + crate::lexer::first_line_break(&text[from..])?;
        let rest = &text[at..];
        let length = if rest.starts_with("\r\n") {
            2
        } else {
            rest.chars().next().map_or(0, char::len_utf8)
        };
        from = at + length;
        Some(at..from)
    })
}

/// Source text with an index of ECMAScript line starts (`line_breaks`).
#[derive(Debug)]
pub(crate) struct SourceText {
    text: std::sync::Arc<str>,
    line_starts: Vec<usize>,
    /// For non-ASCII text, the UTF-16 length of the text before each multiple
    /// of `CHECKPOINT` bytes, rounded down to a character boundary, so a
    /// column needs no scan longer than `CHECKPOINT`. Empty for ASCII text.
    utf16: Vec<usize>,
}

const CHECKPOINT: usize = 256;

/// Why a one-based line and UTF-16 column address no position in a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PositionError {
    LineOutOfRange,
    ColumnOutOfRange,
    InsideSurrogatePair,
}

impl fmt::Display for PositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LineOutOfRange => "line is outside the file",
            Self::ColumnOutOfRange => "column is outside the line",
            Self::InsideSurrogatePair => "column is inside a surrogate pair",
        })
    }
}

impl SourceText {
    pub(crate) fn new(text: String) -> Self {
        Self::shared(text.into())
    }

    pub(crate) fn shared(text: std::sync::Arc<str>) -> Self {
        let line_starts = std::iter::once(0)
            .chain(line_breaks(&text).map(|line_break| line_break.end))
            .collect();
        let mut utf16 = Vec::new();
        if !text.is_ascii() {
            let mut units = 0;
            for (index, c) in text.char_indices() {
                while utf16.len() * CHECKPOINT < index + c.len_utf8() {
                    utf16.push(units);
                }
                units += c.len_utf16();
            }
            while utf16.len() * CHECKPOINT <= text.len() {
                utf16.push(units);
            }
        }
        Self {
            text,
            line_starts,
            utf16,
        }
    }

    /// The UTF-16 length of the text before a UTF-8 boundary.
    fn utf16_offset(&self, byte: usize) -> usize {
        if self.utf16.is_empty() {
            return byte;
        }
        let checkpoint = byte / CHECKPOINT;
        let mut base = checkpoint * CHECKPOINT;
        while !self.text.is_char_boundary(base) {
            base -= 1;
        }
        self.utf16[checkpoint]
            + self.text[base..byte]
                .chars()
                .map(char::len_utf16)
                .sum::<usize>()
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    /// The zero-based line containing `byte`.
    pub(crate) fn line_index(&self, byte: usize) -> usize {
        self.line_starts.partition_point(|start| *start <= byte) - 1
    }

    pub(crate) fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// The position of a UTF-8 boundary in this text.
    pub(crate) fn position(&self, byte: usize) -> SourcePosition {
        debug_assert!(self.text.is_char_boundary(byte));
        let line = self.line_index(byte);
        let column = self.utf16_offset(byte) - self.utf16_offset(self.line_starts[line]);
        SourcePosition {
            line: line + 1,
            column: column + 1,
        }
    }

    /// The byte offset of a one-based line and UTF-16 column. A column may
    /// address the end of its line, but not a line break or a character's
    /// second UTF-16 unit.
    pub(crate) fn offset(&self, position: SourcePosition) -> Result<usize, PositionError> {
        if position.line == 0 || position.line > self.line_starts.len() {
            return Err(PositionError::LineOutOfRange);
        }
        if position.column == 0 {
            return Err(PositionError::ColumnOutOfRange);
        }
        let start = self.line_starts[position.line - 1];
        let content = &self.text[start..self.line_end(position.line - 1)];
        let mut remaining = position.column - 1;
        for (index, c) in content.char_indices() {
            if remaining == 0 {
                return Ok(start + index);
            }
            let width = c.len_utf16();
            if remaining < width {
                return Err(PositionError::InsideSurrogatePair);
            }
            remaining -= width;
        }
        if remaining == 0 {
            Ok(start + content.len())
        } else {
            Err(PositionError::ColumnOutOfRange)
        }
    }

    /// Where the zero-based line `index` begins.
    pub(crate) fn line_start(&self, index: usize) -> usize {
        self.line_starts[index]
    }

    fn line_end(&self, line: usize) -> usize {
        let Some(&next) = self.line_starts.get(line + 1) else {
            return self.text.len();
        };
        let before = &self.text[self.line_starts[line]..next];
        let terminator = if before.ends_with("\r\n") {
            2
        } else {
            before.chars().next_back().map_or(0, char::len_utf8)
        };
        next - terminator
    }
}

#[cfg(test)]
mod tests {
    /// Line breaks found by bytes are the characters' line breaks, CRLF as
    /// one, for text of every class the rule separates: CR, LF, CRLF, LF CR,
    /// U+2028, U+2029, other characters encoded from 0xE2, and none at all.
    #[test]
    fn line_breaks_by_bytes_are_the_characters_line_breaks() {
        const PIECES: &[&str] = &[
            "a",
            " ",
            "\n",
            "\r",
            "\r\n",
            "\n\r",
            "\u{2028}",
            "\u{2029}",
            "\u{2026}",
            "\u{2030}",
            "\u{20ac}",
            "\u{feff}",
            "\u{e9}",
            "\u{1f600}",
            "",
        ];
        let expected = |text: &str| {
            let mut chars = text.char_indices().peekable();
            let mut breaks = Vec::new();
            while let Some((index, c)) = chars.next() {
                if c == '\r' && chars.next_if(|(_, next)| *next == '\n').is_some() {
                    breaks.push(index..index + 2);
                } else if crate::lexer::line_break(c) {
                    breaks.push(index..index + c.len_utf8());
                }
            }
            breaks
        };
        let mut state = 0x9e37_79b9_u32;
        for case in 0..2_000 {
            let mut text = String::new();
            for _ in 0..case % 23 {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                text.push_str(PIECES[state as usize % PIECES.len()]);
            }
            assert_eq!(
                super::line_breaks(&text).collect::<Vec<_>>(),
                expected(&text),
                "{text:?}"
            );
        }
    }

    use super::*;

    fn at(line: usize, column: usize) -> SourcePosition {
        SourcePosition { line, column }
    }

    #[test]
    fn positions_count_utf16_units_across_every_line_break() {
        let text = SourceText::new("a\r\nb\rc\nd\u{2028}e\u{2029}😀x".into());
        assert_eq!(text.position(0), at(1, 1));
        assert_eq!(text.position(3), at(2, 1));
        assert_eq!(text.position(5), at(3, 1));
        assert_eq!(text.position(7), at(4, 1));
        assert_eq!(text.position(11), at(5, 1));
        let x = text.as_str().find('x').unwrap();
        assert_eq!(text.position(x), at(6, 3));
        assert_eq!(text.offset(at(6, 3)), Ok(x));
        assert_eq!(
            text.offset(at(6, 2)),
            Err(PositionError::InsideSurrogatePair)
        );
        assert_eq!(text.offset(at(1, 2)), Ok(1));
        assert_eq!(text.offset(at(1, 3)), Err(PositionError::ColumnOutOfRange));
        assert_eq!(text.offset(at(7, 1)), Err(PositionError::LineOutOfRange));
    }

    #[test]
    fn long_non_ascii_lines_match_a_full_scan() {
        // Characters of every UTF-8 width straddle the checkpoints.
        let mut source = String::new();
        for index in 0..2_000 {
            source.push(['a', 'é', '€', '😀'][index % 4]);
            if index % 700 == 699 {
                source.push('\n');
            }
        }
        let text = SourceText::new(source.clone());
        for (byte, _) in source.char_indices().chain([(source.len(), ' ')]) {
            let line_start = source[..byte].rfind('\n').map_or(0, |at| at + 1);
            let line = source[..byte].matches('\n').count() + 1;
            let column = source[line_start..byte].encode_utf16().count() + 1;
            assert_eq!(text.position(byte), at(line, column), "byte {byte}");
        }
    }
}
