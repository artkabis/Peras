//! Decision 0041: the prototypes of a formatter, behind `PERAS_PROTO_FMT`.
//!
//! - f1 ([`Style::Tokens`]): the author's line breaks kept, indentation by the
//!   brackets open, the spaces between two tokens by a table of their kinds;
//! - f2 ([`Style::Width`]): a printer at a width of 100, the layout its own
//!   but for blank lines between statements.
//!
//! Both read the same items: each token with the comments and the line breaks
//! before it, read back from the source between two spans, since the lexer
//! drops them. A file with a lexical or syntax error is refused, never half
//! formatted.

mod tokens;
mod width;

use perasc_diag::Diagnostic;

use crate::lexer::lex;
use crate::testing::{Tests, prepare};
use crate::token::{Token, TokenKind};

/// How to lay a file out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// f1: the author's line breaks, indentation and spaces made regular.
    Tokens,
    /// f2: a printer at a width of 100.
    Width,
}

/// The width f2 prints at, Prettier's for the repository (`.prettierrc`).
pub const WIDTH: usize = 100;

/// The source laid out as `style` says, or the errors that refuse it.
///
/// # Errors
///
/// The lexical and syntax errors of the source, or of its `test` blocks.
pub fn format(source: &str, style: Style) -> Result<String, Vec<Diagnostic>> {
    let parsed = crate::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics);
    }
    // Every parse blanks the `test` blocks out: their bodies are read here.
    let (built, names) = prepare(source, "", Tests::Build);
    if !names.is_empty() {
        let tests = crate::parse(&built);
        if !tests.diagnostics.is_empty() {
            return Err(tests.diagnostics);
        }
    }
    let lexed = lex(source);
    let items = items(source, &lexed.tokens);
    Ok(match style {
        Style::Tokens => tokens::format(&items),
        Style::Width => width::format(&items),
    })
}

/// A comment, as the source wrote it.
#[derive(Debug, Clone, Copy)]
struct Comment<'a> {
    /// Its text, `//` or `/*` included, trailing blanks dropped.
    text: &'a str,
    /// A `//` comment, which a line break must end.
    line: bool,
    /// The line breaks between what precedes it and it.
    breaks: usize,
}

/// A token, with what the source holds before it.
#[derive(Debug, Clone)]
struct Item<'a> {
    kind: TokenKind,
    text: &'a str,
    /// The comments between the previous token and this one.
    comments: Vec<Comment<'a>>,
    /// The line breaks between the last comment (or the previous token) and it.
    breaks: usize,
    /// Whether a blank separated it from what precedes it.
    spaced: bool,
}

/// Every token of `source` with its comments, ending with the end of file.
fn items<'a>(source: &'a str, tokens: &[Token]) -> Vec<Item<'a>> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut at = 0_usize;
    for t in tokens {
        let start = t.span.start as usize;
        let end = t.span.end as usize;
        let gap = source.get(at..start).unwrap_or("");
        let (comments, breaks) = gap_of(gap);
        out.push(Item {
            kind: t.kind,
            text: source.get(start..end).unwrap_or(""),
            comments,
            breaks,
            spaced: !gap.is_empty(),
        });
        at = end.max(at);
    }
    out
}

/// The comments of the text between two tokens, and the line breaks after
/// the last of them.
fn gap_of(gap: &str) -> (Vec<Comment<'_>>, usize) {
    let mut comments = Vec::new();
    let mut breaks = 0;
    let mut i = 0;
    let bytes = gap.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            breaks += 1;
            i += 1;
        } else if gap[i..].starts_with("//") {
            let end = gap[i..].find('\n').map_or(gap.len(), |n| i + n);
            comments.push(Comment {
                text: gap[i..end].trim_end(),
                line: true,
                breaks,
            });
            breaks = 0;
            i = end;
        } else if gap[i..].starts_with("/*") {
            let end = gap[i + 2..].find("*/").map_or(gap.len(), |n| i + 2 + n + 2);
            comments.push(Comment {
                text: &gap[i..end],
                line: false,
                breaks,
            });
            breaks = 0;
            i = end;
        } else {
            i += gap[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    (comments, breaks)
}

/// Whether a `+` or `-` after `prev` is a sign rather than an operator.
fn unary_after(prev: Option<TokenKind>) -> bool {
    use TokenKind as K;
    !matches!(
        prev,
        Some(
            K::Ident
                | K::Int
                | K::Float
                | K::String
                | K::TemplateTail
                | K::RParen
                | K::RBracket
                | K::True
                | K::False
                | K::PlusPlus
                | K::MinusMinus
                | K::Question
        )
    )
}

/// Whether one space goes between `a` and `b`, on one line. `prefix` says
/// that `a` is a sign or a prefix step; `spaced`, that the author put a blank
/// between them, which decides around `<` and `>`, a comparison or a type.
fn space(a: TokenKind, b: TokenKind, prefix: bool, spaced: bool) -> bool {
    use TokenKind as K;
    if matches!(
        b,
        K::RParen
            | K::RBracket
            | K::Comma
            | K::Semicolon
            | K::Dot
            | K::Colon
            | K::Question
            | K::TemplateMiddle
            | K::TemplateTail
    ) {
        return false;
    }
    if matches!(
        a,
        K::LParen
            | K::LBracket
            | K::Dot
            | K::Bang
            | K::Tilde
            | K::At
            | K::TemplateHead
            | K::TemplateMiddle
    ) {
        return false;
    }
    if prefix {
        return false;
    }
    if b == K::RBrace {
        return a != K::LBrace;
    }
    if matches!(b, K::PlusPlus | K::MinusMinus) && !unary_after(Some(a)) {
        return false;
    }
    if matches!(a, K::Lt | K::Gt) || matches!(b, K::Lt | K::Gt) {
        return spaced;
    }
    if b == K::LParen || b == K::LBracket {
        return !matches!(a, K::Ident | K::RParen | K::RBracket);
    }
    true
}

/// Whether a token opens a bracket, a `${` included.
const fn opens(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::LBrace
            | TokenKind::TemplateHead
            | TokenKind::TemplateMiddle
    )
}

/// Whether a token closes a bracket, a `}` of `${` included.
const fn closes(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::RBrace
            | TokenKind::TemplateMiddle
            | TokenKind::TemplateTail
    )
}

#[cfg(test)]
mod tests;
