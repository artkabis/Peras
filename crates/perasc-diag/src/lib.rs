//! Diagnostics, stable error codes and English/French messages for the Peras compiler.
//!
//! This crate defines the data every error carries: a stable code, labelled spans,
//! a cause and a suggested fix. Texts are never stored here: a diagnostic only holds
//! Fluent message IDs and their arguments, resolved in `messages/en.ftl` and
//! `messages/fr.ftl`. Terminal rendering (`ariadne`) and `--json` output build on it.

use std::fmt;

pub mod codes;
pub mod messages;

/// A stable error code such as `E0204`. Codes are never reused or renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErrorCode(u16);

impl ErrorCode {
    /// Creates an error code from its number, e.g. `ErrorCode::new(204)` for `E0204`.
    #[must_use]
    pub const fn new(number: u16) -> Self {
        Self(number)
    }

    /// The numeric part of the code.
    #[must_use]
    pub const fn number(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E{:04}", self.0)
    }
}

/// A half-open byte range `start..end` in a source file.
///
/// Offsets are `u32`: source files are limited to 4 GiB (see `E0008`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    /// Offset of the first byte.
    pub start: u32,
    /// Offset one past the last byte.
    pub end: u32,
}

impl Span {
    /// Creates a span. `start` must not be greater than `end`.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Length of the span in bytes.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the span covers no byte.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    /// The smallest span covering both `self` and `other`.
    #[must_use]
    pub fn to(self, other: Self) -> Self {
        Self::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// A value substituted into a Fluent message, e.g. `{ $character }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// Text, inserted verbatim.
    Text(String),
    /// Integer, formatted by Fluent for the target language.
    Number(i64),
}

/// A reference to a Fluent message and the arguments it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Message ID in `messages/*.ftl`, e.g. `lex-invalid-character`.
    pub id: &'static str,
    /// Named arguments, in the order they were added.
    pub args: Vec<(&'static str, Arg)>,
}

impl Message {
    /// A message without arguments.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self {
            id,
            args: Vec::new(),
        }
    }

    /// Adds a text argument.
    #[must_use]
    pub fn text(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.args.push((name, Arg::Text(value.into())));
        self
    }

    /// Adds an integer argument.
    #[must_use]
    pub fn number(mut self, name: &'static str, value: i64) -> Self {
        self.args.push((name, Arg::Number(value)));
        self
    }
}

/// A span of source underlined in the excerpt, with its explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// The underlined source.
    pub span: Span,
    /// What this span shows.
    pub message: Message,
    /// The primary label (`^^^`) marks where the error is; others (`---`) give context.
    pub primary: bool,
}

/// A replacement of one span of source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// The source replaced; empty to insert.
    pub span: Span,
    /// The new text; empty to delete.
    pub replacement: String,
}

/// A suggested fix: an explanation, and the edits that apply it when they are certain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    /// What the fix does.
    pub message: Message,
    /// The edits, shown as a diff. Empty when the right fix depends on intent.
    pub edits: Vec<Edit>,
}

/// A compile error. Every diagnostic blocks the build: there are no warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable code, documented by `perasc explain`.
    pub code: ErrorCode,
    /// One-line title.
    pub message: Message,
    /// Underlined spans; exactly one is primary.
    pub labels: Vec<Label>,
    /// Why this is an error, in one sentence.
    pub cause: Message,
    /// The suggested fix.
    pub fix: Fix,
}

impl Diagnostic {
    /// Span of the primary label, where the error is reported.
    #[must_use]
    pub fn primary_span(&self) -> Option<Span> {
        self.labels
            .iter()
            .find(|label| label.primary)
            .map(|l| l.span)
    }

    /// Every message ID this diagnostic references, to check they all exist.
    pub fn message_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        [self.message.id, self.cause.id, self.fix.message.id]
            .into_iter()
            .chain(self.labels.iter().map(|label| label.message.id))
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorCode, Span};

    #[test]
    fn codes_are_printed_with_four_digits() {
        assert_eq!(ErrorCode::new(204).to_string(), "E0204");
        assert_eq!(ErrorCode::new(1).to_string(), "E0001");
    }

    #[test]
    fn spans_join_and_measure() {
        let a = Span::new(2, 5);
        let b = Span::new(8, 9);
        assert_eq!(a.to(b), Span::new(2, 9));
        assert_eq!(a.len(), 3);
        assert!(Span::new(4, 4).is_empty());
    }
}
