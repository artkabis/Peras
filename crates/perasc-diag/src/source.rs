//! A source file with its line index, to turn byte offsets into lines and columns.

use crate::Span;

/// A named source file, indexed by line.
#[derive(Debug, Clone)]
pub struct SourceFile<'src> {
    name: &'src str,
    text: &'src str,
    /// Byte offset of the start of each line.
    line_starts: Vec<usize>,
}

/// A position for people: line and column both start at 1, columns count characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Line, from 1.
    pub line: usize,
    /// Column in characters, from 1.
    pub column: usize,
}

impl<'src> SourceFile<'src> {
    /// Indexes `text`, shown as `name` in diagnostics.
    #[must_use]
    pub fn new(name: &'src str, text: &'src str) -> Self {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        Self {
            name,
            text,
            line_starts,
        }
    }

    /// The name shown in diagnostics, usually a path.
    #[must_use]
    pub const fn name(&self) -> &'src str {
        self.name
    }

    /// The whole text.
    #[must_use]
    pub const fn text(&self) -> &'src str {
        self.text
    }

    /// Number of lines; a file ending with a newline has an empty last line.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Clamps an offset into the text, on a character boundary.
    fn clamp(&self, offset: u32) -> usize {
        let mut at = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(self.text.len());
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    /// The position of a byte offset.
    #[must_use]
    pub fn position(&self, offset: u32) -> Position {
        let at = self.clamp(offset);
        let line_index = self
            .line_starts
            .partition_point(|&start| start <= at)
            .saturating_sub(1);
        let start = self.line_starts.get(line_index).copied().unwrap_or(0);
        let column = self.text.get(start..at).map_or(0, |s| s.chars().count()) + 1;
        Position {
            line: line_index + 1,
            column,
        }
    }

    /// Byte range of a line (from 1), without its line ending.
    #[must_use]
    pub fn line_range(&self, line: usize) -> Option<(usize, usize)> {
        let start = *self.line_starts.get(line.checked_sub(1)?)?;
        let next = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.text.len());
        let full = self.text.get(start..next)?;
        let trimmed = full.strip_suffix('\n').unwrap_or(full);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        Some((start, start + trimmed.len()))
    }

    /// The text of a line (from 1), without its line ending.
    #[must_use]
    pub fn line(&self, line: usize) -> &'src str {
        self.line_range(line)
            .and_then(|(start, end)| self.text.get(start..end))
            .unwrap_or("")
    }

    /// The source covered by a span.
    #[must_use]
    pub fn slice(&self, span: Span) -> &'src str {
        self.text
            .get(self.clamp(span.start)..self.clamp(span.end))
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::{Position, SourceFile};

    #[test]
    fn positions_count_lines_and_characters() {
        let file = SourceFile::new("a.peras", "ab\r\ncé\nx");
        assert_eq!(file.line_count(), 3);
        assert_eq!(file.position(0), Position { line: 1, column: 1 });
        assert_eq!(file.position(4), Position { line: 2, column: 1 });
        // `é` is two bytes but one column.
        assert_eq!(file.position(7), Position { line: 2, column: 3 });
        assert_eq!(file.position(8), Position { line: 3, column: 1 });
        assert_eq!(file.line(1), "ab");
        assert_eq!(file.line(2), "cé");
        assert_eq!(file.line(4), "");
    }

    #[test]
    fn offsets_past_the_end_or_inside_a_character_are_clamped() {
        let file = SourceFile::new("a.peras", "é");
        assert_eq!(file.position(1), Position { line: 1, column: 1 });
        assert_eq!(file.position(99), Position { line: 1, column: 2 });
    }
}
