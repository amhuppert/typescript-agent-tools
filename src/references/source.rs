//! Captured source text, and the reference subsystem's conversions between
//! UTF-8 byte spans and one-based line/UTF-16 column positions.

use super::SourcePosition;
use std::path::PathBuf;

/// A project-local file identity. It indexes `Sources` and never escapes the
/// project; only `Sources` creates one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct FileId(u32);

impl FileId {
    pub(super) fn index(self) -> usize {
        self.0 as usize
    }
}

/// One captured program input.
#[derive(Debug)]
pub(super) struct SourceFile {
    /// Canonical absolute path, used for identity and query matching.
    pub(super) path: PathBuf,
    /// `/`-separated path relative to the configuration directory.
    pub(super) display: String,
    pub(super) text: SourceText,
}

#[derive(Debug, Default)]
pub(super) struct Sources {
    files: Vec<SourceFile>,
    reserved: u32,
}

impl Sources {
    /// The id of an input discovered before it is captured.
    pub(super) fn reserve(&mut self) -> FileId {
        let id = FileId(self.reserved);
        self.reserved = self
            .reserved
            .checked_add(1)
            .expect("file count fits in u32");
        id
    }

    /// Capture a reserved input. Inputs are captured in reservation order,
    /// so every table indexed by `FileId` lines up with `Sources`.
    pub(super) fn capture(&mut self, id: FileId, file: SourceFile) {
        assert_eq!(
            id.index(),
            self.files.len(),
            "inputs are captured in reservation order"
        );
        self.files.push(file);
    }

    pub(super) fn get(&self, id: FileId) -> &SourceFile {
        &self.files[id.index()]
    }
    pub(super) fn ids(&self) -> impl Iterator<Item = FileId> + '_ {
        (0..self.files.len()).map(|index| FileId(index as u32))
    }
}

/// Source text with an index of ECMAScript line starts. CRLF is one break;
/// CR, LF, U+2028, and U+2029 are also breaks.
#[derive(Debug)]
pub(super) struct SourceText {
    text: String,
    line_starts: Vec<usize>,
    /// For non-ASCII text, the UTF-16 length of the text before each multiple
    /// of `CHECKPOINT` bytes, rounded down to a character boundary, so a
    /// column needs no scan longer than `CHECKPOINT`. Empty for ASCII text.
    utf16: Vec<usize>,
}

const CHECKPOINT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PositionError {
    LineOutOfRange,
    ColumnOutOfRange,
    InsideSurrogatePair,
}

impl SourceText {
    pub(super) fn new(text: String) -> Self {
        let mut line_starts = vec![0];
        let mut chars = text.char_indices().peekable();
        while let Some((index, c)) = chars.next() {
            match c {
                '\r' => {
                    if matches!(chars.peek(), Some((_, '\n'))) {
                        chars.next();
                        line_starts.push(index + 2);
                    } else {
                        line_starts.push(index + 1);
                    }
                }
                c if crate::lexer::line_break(c) => line_starts.push(index + c.len_utf8()),
                _ => {}
            }
        }
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

    pub(super) fn as_str(&self) -> &str {
        &self.text
    }

    /// The zero-based line containing `byte`.
    pub(super) fn line_index(&self, byte: usize) -> usize {
        self.line_starts.partition_point(|start| *start <= byte) - 1
    }

    pub(super) fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// The position of a UTF-8 boundary in this text.
    pub(super) fn position(&self, byte: usize) -> SourcePosition {
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
    pub(super) fn offset(&self, position: SourcePosition) -> Result<usize, PositionError> {
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

    /// The text of a one-based line, without its line break.
    pub(super) fn line(&self, line: usize) -> Option<&str> {
        let index = line
            .checked_sub(1)
            .filter(|&index| index < self.line_starts.len())?;
        Some(&self.text[self.line_starts[index]..self.line_end(index)])
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
