//! Renders a diagnostic for the terminal, in the format of the language spec:
//!
//! ```text
//! error[E0006]: `===` does not exist in Peras
//!   --> src/a.peras:2:7
//!    |
//!  2 | if (a === b) { return 1; }
//!    |       ^^^ write `==`
//!    |
//!    = why: Peras never converts types implicitly, so `==` and `!=` already compare strictly.
//!    = fix: replace `===` with `==`
//!    |
//!  2 - if (a === b) { return 1; }
//!  2 + if (a == b) { return 1; }
//!    |
//!    = details: perasc explain E0006
//! ```
//!
//! Characters that would be invisible or would reorder the terminal output
//! (bidirectional controls, zero-width characters, control codes) are shown as
//! `<U+202E>`: the excerpt always shows what the compiler sees.

use std::fmt::Write as _;

use unicode_width::UnicodeWidthChar;

use crate::i18n::Localizer;
use crate::messages::Language;
use crate::source::SourceFile;
use crate::{Diagnostic, Edit, Label};

/// Rendering options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderOptions {
    /// Use ANSI colours.
    pub color: bool,
}

/// ANSI styles.
#[derive(Clone, Copy)]
enum Style {
    Error,
    Secondary,
    Gutter,
    Removed,
    Added,
    Bold,
}

impl Style {
    const fn code(self) -> &'static str {
        match self {
            Self::Error => "1;31",
            Self::Secondary | Self::Gutter => "1;34",
            Self::Removed => "31",
            Self::Added => "32",
            Self::Bold => "1",
        }
    }
}

struct Painter {
    color: bool,
}

impl Painter {
    fn paint(&self, style: Style, text: &str) -> String {
        if self.color && !text.is_empty() {
            format!("\u{1b}[{}m{text}\u{1b}[0m", style.code())
        } else {
            text.to_owned()
        }
    }
}

/// The colon between a word and what follows: French puts a space before it.
#[must_use]
pub const fn colon(language: Language) -> &'static str {
    match language {
        Language::English => ":",
        Language::French => " :",
    }
}

/// Renders one diagnostic, ending with a newline.
#[must_use]
pub fn render(
    diagnostic: &Diagnostic,
    file: &SourceFile<'_>,
    localizer: &Localizer,
    options: RenderOptions,
) -> String {
    let painter = Painter {
        color: options.color,
    };
    let colon = colon(localizer.language());
    let primary = diagnostic.primary_span().unwrap_or_default();
    let location = file.position(primary.start);

    // Lines shown in the excerpt, and the lines the fix rewrites.
    let mut lines: Vec<usize> = diagnostic
        .labels
        .iter()
        .map(|label| file.position(label.span.start).line)
        .collect();
    lines.sort_unstable();
    lines.dedup();
    let diff = Diff::new(file, &diagnostic.fix.edits);
    let widest = lines
        .iter()
        .copied()
        .chain(diff.as_ref().map(Diff::last_line))
        .max()
        .unwrap_or(1);
    let width = widest.to_string().len();
    let blank = " ".repeat(width);
    let bar = painter.paint(Style::Gutter, "|");
    let gutter = |line: Option<usize>| match line {
        Some(n) => painter.paint(Style::Gutter, &format!(" {n:>width$} |")),
        None => format!(" {blank} {bar}"),
    };

    let mut out = String::new();
    let title = format!("{}[{}]", localizer.text("diag-error"), diagnostic.code);
    let _ = writeln!(
        out,
        "{}{colon} {}",
        painter.paint(Style::Error, &title),
        painter.paint(Style::Bold, &localizer.message(&diagnostic.message)),
    );
    let _ = writeln!(
        out,
        " {blank}{} {}:{}:{}",
        painter.paint(Style::Gutter, "-->"),
        file.name(),
        location.line,
        location.column
    );
    let _ = writeln!(out, "{}", gutter(None));

    for &line in &lines {
        excerpt_line(
            &mut out, file, line, diagnostic, localizer, &painter, &gutter,
        );
    }

    let _ = writeln!(out, "{}", gutter(None));
    let note = |word: &str, text: &str| {
        format!(
            " {blank} {} {}{colon} {text}",
            painter.paint(Style::Gutter, "="),
            painter.paint(Style::Bold, &localizer.text(word))
        )
    };
    let _ = writeln!(
        out,
        "{}",
        note("diag-why", &localizer.message(&diagnostic.cause))
    );
    let _ = writeln!(
        out,
        "{}",
        note("diag-fix", &localizer.message(&diagnostic.fix.message))
    );
    if let Some(diff) = diff {
        let _ = writeln!(out, "{}", gutter(None));
        for (n, text) in diff.removed {
            let line = format!(" {n:>width$} - {text}");
            let _ = writeln!(out, "{}", painter.paint(Style::Removed, &line));
        }
        for (n, text) in diff.added {
            let line = format!(" {n:>width$} + {text}");
            let _ = writeln!(out, "{}", painter.paint(Style::Added, &line));
        }
        let _ = writeln!(out, "{}", gutter(None));
    }
    let explain = format!("perasc explain {}", diagnostic.code);
    let _ = writeln!(out, "{}", note("diag-details", &explain));
    out
}

/// One source line of the excerpt, then one underline row per label starting on it.
fn excerpt_line(
    out: &mut String,
    file: &SourceFile<'_>,
    line: usize,
    diagnostic: &Diagnostic,
    localizer: &Localizer,
    painter: &Painter,
    gutter: &dyn Fn(Option<usize>) -> String,
) {
    let display = DisplayLine::new(file, line);
    let _ = writeln!(out, "{} {}", gutter(Some(line)), display.text);
    let mut on_line: Vec<&Label> = diagnostic
        .labels
        .iter()
        .filter(|label| file.position(label.span.start).line == line)
        .collect();
    on_line.sort_by_key(|label| label.span.start);
    for label in on_line {
        let (start, end) = display.columns(file, label);
        let (mark, style) = if label.primary {
            ('^', Style::Error)
        } else {
            ('-', Style::Secondary)
        };
        let marks = mark.to_string().repeat(end.saturating_sub(start).max(1));
        let text = format!("{marks} {}", localizer.message(&label.message));
        let _ = writeln!(
            out,
            "{} {}{}",
            gutter(None),
            " ".repeat(start),
            painter.paint(style, &text)
        );
    }
}

/// How a character is shown, and how many terminal columns it takes.
fn display_char(c: char) -> (String, usize) {
    if c == '\t' {
        return ("    ".to_owned(), 4);
    }
    let hidden = c.is_control()
        || (c.is_whitespace() && c != ' ')
        || matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        );
    if hidden {
        let shown = format!("<U+{:04X}>", u32::from(c));
        let width = shown.len();
        (shown, width)
    } else {
        (c.to_string(), c.width().unwrap_or(0))
    }
}

/// A source line made safe to print, with the column of each byte offset.
struct DisplayLine {
    text: String,
    /// `(byte offset in the file, display column)` of each character start.
    columns: Vec<(usize, usize)>,
    /// Display width of the whole line.
    width: usize,
    /// Byte range of the line in the file.
    range: (usize, usize),
}

impl DisplayLine {
    fn new(file: &SourceFile<'_>, line: usize) -> Self {
        let range = file.line_range(line).unwrap_or((0, 0));
        let mut text = String::new();
        let mut columns = Vec::new();
        let mut width = 0;
        for (i, c) in file.line(line).char_indices() {
            let (shown, w) = display_char(c);
            columns.push((range.0 + i, width));
            text.push_str(&shown);
            width += w;
        }
        Self {
            text,
            columns,
            width,
            range,
        }
    }

    /// Display column of a byte offset in the file, clamped to this line.
    fn column(&self, offset: usize) -> usize {
        if offset >= self.range.1 {
            return self.width;
        }
        let at = self.columns.partition_point(|&(o, _)| o <= offset);
        at.checked_sub(1)
            .and_then(|i| self.columns.get(i))
            .map_or(0, |&(_, col)| col)
    }

    /// Display columns covered by a label on this line.
    fn columns(&self, file: &SourceFile<'_>, label: &Label) -> (usize, usize) {
        let start = file.slice_start(label.span.start);
        let end = file.slice_start(label.span.end).max(start);
        (self.column(start), self.column(end))
    }
}

impl SourceFile<'_> {
    /// A span offset as a byte index into the text.
    fn slice_start(&self, offset: u32) -> usize {
        usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(self.text().len())
    }
}

/// The lines a fix rewrites, before and after.
struct Diff {
    removed: Vec<(usize, String)>,
    added: Vec<(usize, String)>,
}

impl Diff {
    fn new(file: &SourceFile<'_>, edits: &[Edit]) -> Option<Self> {
        let first = edits.iter().map(|e| e.span.start).min()?;
        let last = edits.iter().map(|e| e.span.end).max()?;
        let first_line = file.position(first).line;
        let last_line = file.position(last).line;
        let start = file.line_range(first_line)?.0;
        let end = file.line_range(last_line)?.1;
        let before = file.text().get(start..end)?;

        let mut sorted: Vec<&Edit> = edits.iter().collect();
        sorted.sort_by_key(|e| e.span.start);
        let mut after = String::new();
        let mut cursor = start;
        for edit in sorted {
            let from = file.slice_start(edit.span.start).clamp(cursor, end);
            let to = file.slice_start(edit.span.end).clamp(from, end);
            after.push_str(file.text().get(cursor..from)?);
            after.push_str(&edit.replacement);
            cursor = to;
        }
        after.push_str(file.text().get(cursor..end)?);

        let shown = |text: &str| -> String { text.chars().map(|c| display_char(c).0).collect() };
        let numbered = |text: &str| -> Vec<(usize, String)> {
            text.split('\n')
                .enumerate()
                .map(|(i, l)| (first_line + i, shown(l.strip_suffix('\r').unwrap_or(l))))
                .collect()
        };
        Some(Self {
            removed: numbered(before),
            added: numbered(&after),
        })
    }

    fn last_line(&self) -> usize {
        self.removed
            .iter()
            .chain(&self.added)
            .map(|(n, _)| *n)
            .max()
            .unwrap_or(1)
    }
}

#[cfg(test)]
mod tests {
    use super::{RenderOptions, render};
    use crate::i18n::Localizer;
    use crate::messages::Language;
    use crate::source::SourceFile;
    use crate::{Diagnostic, Edit, ErrorCode, Fix, Label, Message, Span};

    fn diagnostic(labels: Vec<Label>, edits: Vec<Edit>) -> Diagnostic {
        Diagnostic {
            code: ErrorCode::new(6),
            message: Message::new("lex-strict-equality").text("operator", "==="),
            labels,
            cause: Message::new("lex-strict-equality-why"),
            fix: Fix {
                message: Message::new("lex-strict-equality-fix")
                    .text("operator", "===")
                    .text("replacement", "=="),
                edits,
            },
        }
    }

    fn label(start: u32, end: u32, primary: bool) -> Label {
        Label {
            span: Span::new(start, end),
            message: Message::new("lex-strict-equality-label").text("replacement", "=="),
            primary,
        }
    }

    fn show(text: &str, d: &Diagnostic) -> String {
        let file = SourceFile::new("a.peras", text);
        render(
            d,
            &file,
            &Localizer::new(Language::English),
            RenderOptions::default(),
        )
    }

    #[test]
    fn wide_characters_keep_the_carets_aligned() {
        // `漢` is one character but two terminal columns.
        let text = "\"漢\" === x";
        let d = diagnostic(vec![label(6, 9, true)], Vec::new());
        let out = show(text, &d);
        assert!(
            out.contains(" 1 | \"漢\" === x\n   |      ^^^ write `==`\n"),
            "{out}"
        );
        // Without edits there is no diff block.
        assert!(!out.contains(" 1 - "), "{out}");
    }

    #[test]
    fn the_gutter_widens_for_two_digit_lines() {
        let text = format!("{}a === b", "\n".repeat(11));
        let d = diagnostic(
            vec![label(13, 16, true)],
            vec![Edit {
                span: Span::new(13, 16),
                replacement: "==".to_owned(),
            }],
        );
        let out = show(&text, &d);
        assert!(
            out.contains("   --> a.peras:12:3\n    |\n 12 | a === b\n"),
            "{out}"
        );
        assert!(out.contains(" 12 - a === b\n 12 + a == b\n"), "{out}");
    }

    #[test]
    fn secondary_labels_use_dashes_and_insertions_one_mark() {
        let d = diagnostic(vec![label(0, 1, true), label(3, 3, false)], Vec::new());
        let out = show("abc", &d);
        assert!(
            out.contains("   | ^ write `==`\n   |    - write `==`\n"),
            "{out}"
        );
    }

    #[test]
    fn an_insertion_after_the_last_newline_is_shown_on_the_empty_last_line() {
        let d = diagnostic(
            vec![label(0, 2, true)],
            vec![Edit {
                span: Span::new(5, 5),
                replacement: "*/".to_owned(),
            }],
        );
        let out = show("/* a\n", &d);
        assert!(out.contains(" 2 - \n 2 + */\n"), "{out}");
    }

    #[test]
    fn a_fix_spanning_lines_shows_every_line() {
        let d = diagnostic(
            vec![label(0, 1, true)],
            vec![Edit {
                span: Span::new(1, 4),
                replacement: String::new(),
            }],
        );
        let out = show("a\nb\nc", &d);
        assert!(out.contains(" 1 - a\n 2 - b\n 3 - c\n 1 + ac\n"), "{out}");
    }

    #[test]
    fn an_empty_file_still_renders() {
        let d = diagnostic(vec![label(0, 0, true)], Vec::new());
        let out = show("", &d);
        assert!(
            out.starts_with("error[E0006]: `===` does not exist in Peras\n  --> a.peras:1:1\n"),
            "{out}"
        );
        assert!(out.contains(" 1 | \n   | ^ write `==`\n"), "{out}");
    }
}
