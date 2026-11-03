//! The lexer: turns source text into tokens, reporting every lexical error in one pass.
//!
//! `logos` recognises the tokens; this module then validates what a regular
//! expression cannot (number syntax, string escapes, confusable characters) and
//! turns each problem into a [`Diagnostic`] with a cause and a suggested fix.
//! Lexing never stops at the first error: an erroneous piece of source becomes a
//! [`TokenKind::Error`] token, or the token it most likely meant, and lexing goes on.

use logos::{FilterResult, Logos};
use perasc_diag::{Diagnostic, Edit, ErrorCode, Fix, Label, Message, Span, codes};

use crate::token::{Token, TokenKind};

/// The result of lexing one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lexed {
    /// Every token, comments and whitespace excluded, ending with [`TokenKind::Eof`].
    pub tokens: Vec<Token>,
    /// Every lexical error, sorted by position.
    pub diagnostics: Vec<Diagnostic>,
}

/// Lexes a whole source file.
#[must_use]
pub fn lex(source: &str) -> Lexed {
    lex_bounded(source, usize::try_from(u32::MAX).unwrap_or(usize::MAX))
}

/// Decodes the value of a [`TokenKind::String`] token from its source text,
/// quotes included. Invalid escapes, already reported by the lexer, are kept as written.
#[must_use]
pub fn string_value(token_text: &str) -> String {
    let mut chars = token_text.chars();
    let quote = chars.next();
    let body = chars.as_str();
    let body = match quote {
        Some(q) => body.strip_suffix(q).unwrap_or(body),
        None => body,
    };
    scan_escapes(body, |_| {})
}

/// Lexes `source`, refusing it when longer than `max_len` bytes.
fn lex_bounded(source: &str, max_len: usize) -> Lexed {
    if source.len() > max_len {
        return Lexed {
            tokens: vec![Token {
                kind: TokenKind::Eof,
                span: Span::new(0, 0),
            }],
            diagnostics: vec![source_too_large(source.len())],
        };
    }

    // A byte order mark is allowed at the very start of a file, and only there.
    let (body, base) = match source.strip_prefix('\u{FEFF}') {
        Some(rest) => (rest, '\u{FEFF}'.len_utf8()),
        None => (source, 0),
    };

    let mut lexer = Lexer {
        source,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
        invalid: None,
    };
    lexer.report_bidi_controls();

    let mut raw = Raw::lexer(body);
    while let Some(result) = raw.next() {
        let range = raw.span();
        let span = Span::new(offset(base + range.start), offset(base + range.end));
        match result {
            Err(RawError::InvalidCharacter) => lexer.invalid_characters(span),
            Ok(kind) => {
                lexer.flush_invalid();
                lexer.token(kind, span);
            }
            Err(error) => {
                lexer.flush_invalid();
                lexer.error(error, span);
            }
        }
    }
    lexer.flush_invalid();

    let end = offset(source.len());
    lexer.tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(end, end),
    });
    lexer.fix_bidi_inside_strings();
    lexer
        .diagnostics
        .sort_by_key(|d| d.primary_span().map(|s| s.start));
    Lexed {
        tokens: lexer.tokens,
        diagnostics: lexer.diagnostics,
    }
}

/// Converts a byte offset to `u32`. Sources are at most `u32::MAX` bytes long.
fn offset(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Converts a `u32` offset back to `usize`.
fn index(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// Errors that `logos` itself detects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum RawError {
    /// Input matching no token. The default: `logos` uses it for unknown input.
    #[default]
    InvalidCharacter,
    /// A quote with no closing quote on its line.
    UnterminatedString,
    /// A `/*` with no `*/` before the end of the file.
    UnterminatedBlockComment,
}

/// The tokens as `logos` sees them, before validation.
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(error = RawError)]
#[logos(skip r"[ \t\r\n\f]+")]
// A line comment runs to the end of its line: the greedy match is the intent.
#[logos(skip(r"//[^\n]*", allow_greedy = true))]
enum Raw {
    #[token("/*", block_comment)]
    BlockComment,

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,
    // Deliberately broad: `classify_number` explains what is wrong with `0b102` or `10px`.
    #[regex(r"[0-9][0-9A-Za-z_]*")]
    #[regex(r"[0-9][0-9A-Za-z_]*\.[0-9][0-9A-Za-z_]*")]
    #[regex(r"[0-9][0-9_]*(\.[0-9][0-9_]*)?[eE][+-][0-9A-Za-z_]*")]
    Number,
    #[token("\"", |lex| string(lex, '"'))]
    #[token("'", |lex| string(lex, '\''))]
    Str,

    #[token("async")]
    Async,
    #[token("await")]
    Await,
    #[token("break")]
    Break,
    #[token("class")]
    Class,
    #[token("const")]
    Const,
    #[token("continue")]
    Continue,
    #[token("else")]
    Else,
    #[token("enum")]
    Enum,
    #[token("export")]
    Export,
    #[token("false")]
    False,
    #[token("fn")]
    Fn,
    #[token("for")]
    For,
    #[token("if")]
    If,
    #[token("impl")]
    Impl,
    #[token("import")]
    Import,
    #[token("inout")]
    Inout,
    #[token("interface")]
    Interface,
    #[token("let")]
    Let,
    #[token("match")]
    Match,
    #[token("return")]
    Return,
    #[token("sink")]
    Sink,
    #[token("struct")]
    Struct,
    #[token("trait")]
    Trait,
    #[token("true")]
    True,
    #[token("type")]
    Type,
    #[token("while")]
    While,

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,
    #[token("?")]
    Question,
    #[token("@")]
    At,
    #[token("=>")]
    FatArrow,

    #[token("=")]
    Eq,
    #[token("==")]
    EqEq,
    #[token("===")]
    EqEqEq,
    #[token("!")]
    Bang,
    #[token("!=")]
    BangEq,
    #[token("!==")]
    BangEqEq,
    #[token("<")]
    Lt,
    #[token("<=")]
    LtEq,
    #[token(">")]
    Gt,
    #[token(">=")]
    GtEq,
    #[token("+")]
    Plus,
    #[token("+=")]
    PlusEq,
    #[token("++")]
    PlusPlus,
    #[token("-")]
    Minus,
    #[token("-=")]
    MinusEq,
    #[token("--")]
    MinusMinus,
    #[token("*")]
    Star,
    #[token("*=")]
    StarEq,
    #[token("/")]
    Slash,
    #[token("/=")]
    SlashEq,
    #[token("%")]
    Percent,
    #[token("%=")]
    PercentEq,
    #[token("&")]
    Amp,
    #[token("&&")]
    AmpAmp,
    #[token("|")]
    Pipe,
    #[token("||")]
    PipePipe,
    #[token("^")]
    Caret,
    #[token("~")]
    Tilde,
}

/// Skips a block comment, or reports it unterminated.
fn block_comment(lex: &mut logos::Lexer<'_, Raw>) -> FilterResult<(), RawError> {
    let rest = lex.remainder();
    if let Some(end) = rest.find("*/") {
        lex.bump(end + 2);
        FilterResult::Skip
    } else {
        lex.bump(rest.len());
        FilterResult::Error(RawError::UnterminatedBlockComment)
    }
}

/// Consumes a string up to its closing quote. A string never spans lines:
/// without a closing quote, it stops at the end of the line, `\r` excluded.
fn string(lex: &mut logos::Lexer<'_, Raw>, quote: char) -> Result<(), RawError> {
    let rest = lex.remainder();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == quote {
            lex.bump(i + c.len_utf8());
            return Ok(());
        }
        match c {
            '\\' => match chars.next() {
                Some((_, '\n')) | None => {
                    lex.bump(i + 1);
                    return Err(RawError::UnterminatedString);
                }
                Some(_) => {}
            },
            '\n' => {
                let end = if rest.get(..i).is_some_and(|s| s.ends_with('\r')) {
                    i - 1
                } else {
                    i
                };
                lex.bump(end);
                return Err(RawError::UnterminatedString);
            }
            _ => {}
        }
    }
    lex.bump(rest.len());
    Err(RawError::UnterminatedString)
}

/// A run of adjacent invalid characters, reported as one diagnostic.
struct InvalidRun {
    span: Span,
    first: char,
    count: i64,
    fix: InvalidFix,
}

/// What to suggest for an invalid character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvalidFix {
    /// Delete it.
    Remove,
    /// Replace it with the ASCII text it imitates.
    Replace(&'static str),
    /// A letter outside ASCII, probably in a name.
    Identifier,
    /// A backtick, starting a template literal.
    Template,
}

struct Lexer<'src> {
    source: &'src str,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
    invalid: Option<InvalidRun>,
}

impl Lexer<'_> {
    fn text(&self, span: Span) -> &str {
        self.source
            .get(index(span.start)..index(span.end))
            .unwrap_or("")
    }

    fn push(&mut self, kind: TokenKind, span: Span) {
        self.tokens.push(Token { kind, span });
    }

    /// Handles a token `logos` recognised.
    fn token(&mut self, raw: Raw, span: Span) {
        let kind = match raw {
            Raw::Number => match classify_number(self.text(span)) {
                Ok(kind) => kind,
                Err(problem) => {
                    let literal = self.text(span).to_owned();
                    self.diagnostics
                        .push(malformed_number(&literal, span, &problem));
                    TokenKind::Error
                }
            },
            Raw::Str => {
                self.check_escapes(span, true);
                TokenKind::String
            }
            Raw::EqEqEq => {
                self.diagnostics.push(strict_equality(span, "===", "=="));
                TokenKind::EqEq
            }
            Raw::BangEqEq => {
                self.diagnostics.push(strict_equality(span, "!==", "!="));
                TokenKind::BangEq
            }
            other => simple_kind(other),
        };
        self.push(kind, span);
    }

    /// Handles an error `logos` detected, other than invalid characters.
    fn error(&mut self, error: RawError, span: Span) {
        match error {
            RawError::UnterminatedString => {
                let text = self.text(span).to_owned();
                let quote = text.chars().next().unwrap_or('"');
                self.diagnostics
                    .push(unterminated_string(span, &text, quote));
                self.check_escapes(span, false);
                self.push(TokenKind::String, span);
            }
            RawError::UnterminatedBlockComment => {
                let end = offset(self.source.len());
                self.diagnostics.push(unterminated_block_comment(span, end));
            }
            RawError::InvalidCharacter => self.invalid_characters(span),
        }
    }

    /// Records invalid characters, merging adjacent ones that share the same fix.
    fn invalid_characters(&mut self, span: Span) {
        self.push(TokenKind::Error, span);
        let text = self.text(span).to_owned();
        let mut at = span.start;
        for c in text.chars() {
            let char_span = Span::new(at, at + offset(c.len_utf8()));
            at = char_span.end;
            if bidi_name(c).is_some() {
                continue; // Already reported by `report_bidi_controls`.
            }
            let fix = invalid_fix(c);
            let mergeable = matches!(fix, InvalidFix::Remove | InvalidFix::Identifier);
            if let Some(run) = &mut self.invalid
                && mergeable
                && run.fix == fix
                && run.span.end == char_span.start
            {
                run.span = run.span.to(char_span);
                run.count += 1;
                continue;
            }
            self.flush_invalid();
            self.invalid = Some(InvalidRun {
                span: char_span,
                first: c,
                count: 1,
                fix,
            });
        }
    }

    fn flush_invalid(&mut self) {
        if let Some(run) = self.invalid.take() {
            self.diagnostics.push(invalid_character(&run));
        }
    }

    /// Reports invalid escapes in a string token. `closed` tells whether it ends with its quote.
    fn check_escapes(&mut self, span: Span, closed: bool) {
        let text = self.text(span).to_owned();
        let mut chars = text.chars();
        let Some(quote) = chars.next() else { return };
        let body_start = span.start + offset(quote.len_utf8());
        let body = chars.as_str();
        let body = if closed {
            body.strip_suffix(quote).unwrap_or(body)
        } else {
            body
        };
        let mut found = Vec::new();
        scan_escapes(body, |problem| found.push(problem));
        for problem in found {
            let span = Span::new(
                body_start + offset(problem.start),
                body_start + offset(problem.end),
            );
            let escape = body.get(problem.start..problem.end).unwrap_or("\\");
            self.diagnostics
                .push(invalid_escape(span, escape, &problem.kind));
        }
    }

    /// Reports every bidirectional control character, wherever it is (Trojan Source).
    fn report_bidi_controls(&mut self) {
        for (i, c) in self.source.char_indices() {
            if let Some(name) = bidi_name(c) {
                let start = offset(i);
                let span = Span::new(start, start + offset(c.len_utf8()));
                self.diagnostics.push(bidi_control(span, c, name));
            }
        }
    }

    /// Inside a string, a bidirectional control is better written as an escape than deleted.
    fn fix_bidi_inside_strings(&mut self) {
        let strings: Vec<Span> = self
            .tokens
            .iter()
            .filter(|t| t.kind == TokenKind::String)
            .map(|t| t.span)
            .collect();
        let source = self.source;
        for diagnostic in &mut self.diagnostics {
            if diagnostic.code != codes::BIDI_CONTROL_CHARACTER {
                continue;
            }
            let Some(span) = diagnostic.primary_span() else {
                continue;
            };
            let inside = strings
                .iter()
                .any(|s| s.start < span.start && span.end <= s.end);
            let c = source
                .get(index(span.start)..index(span.end))
                .and_then(|t| t.chars().next());
            if let (true, Some(c)) = (inside, c) {
                let escape = format!("\\u{{{:X}}}", u32::from(c));
                diagnostic.fix = Fix {
                    message: Message::new("lex-bidi-control-fix-escape").text("escape", &escape),
                    edits: vec![Edit {
                        span,
                        replacement: escape,
                    }],
                };
            }
        }
    }
}

/// The token kind for raw tokens that need no validation.
fn simple_kind(raw: Raw) -> TokenKind {
    match raw {
        Raw::Ident => TokenKind::Ident,
        Raw::Async => TokenKind::Async,
        Raw::Await => TokenKind::Await,
        Raw::Break => TokenKind::Break,
        Raw::Class => TokenKind::Class,
        Raw::Const => TokenKind::Const,
        Raw::Continue => TokenKind::Continue,
        Raw::Else => TokenKind::Else,
        Raw::Enum => TokenKind::Enum,
        Raw::Export => TokenKind::Export,
        Raw::False => TokenKind::False,
        Raw::Fn => TokenKind::Fn,
        Raw::For => TokenKind::For,
        Raw::If => TokenKind::If,
        Raw::Impl => TokenKind::Impl,
        Raw::Import => TokenKind::Import,
        Raw::Inout => TokenKind::Inout,
        Raw::Interface => TokenKind::Interface,
        Raw::Let => TokenKind::Let,
        Raw::Match => TokenKind::Match,
        Raw::Return => TokenKind::Return,
        Raw::Sink => TokenKind::Sink,
        Raw::Struct => TokenKind::Struct,
        Raw::Trait => TokenKind::Trait,
        Raw::True => TokenKind::True,
        Raw::Type => TokenKind::Type,
        Raw::While => TokenKind::While,
        Raw::LParen => TokenKind::LParen,
        Raw::RParen => TokenKind::RParen,
        Raw::LBrace => TokenKind::LBrace,
        Raw::RBrace => TokenKind::RBrace,
        Raw::LBracket => TokenKind::LBracket,
        Raw::RBracket => TokenKind::RBracket,
        Raw::Comma => TokenKind::Comma,
        Raw::Semicolon => TokenKind::Semicolon,
        Raw::Colon => TokenKind::Colon,
        Raw::Dot => TokenKind::Dot,
        Raw::Question => TokenKind::Question,
        Raw::At => TokenKind::At,
        Raw::FatArrow => TokenKind::FatArrow,
        Raw::Eq => TokenKind::Eq,
        Raw::EqEq | Raw::EqEqEq => TokenKind::EqEq,
        Raw::Bang => TokenKind::Bang,
        Raw::BangEq | Raw::BangEqEq => TokenKind::BangEq,
        Raw::Lt => TokenKind::Lt,
        Raw::LtEq => TokenKind::LtEq,
        Raw::Gt => TokenKind::Gt,
        Raw::GtEq => TokenKind::GtEq,
        Raw::Plus => TokenKind::Plus,
        Raw::PlusEq => TokenKind::PlusEq,
        Raw::PlusPlus => TokenKind::PlusPlus,
        Raw::Minus => TokenKind::Minus,
        Raw::MinusEq => TokenKind::MinusEq,
        Raw::MinusMinus => TokenKind::MinusMinus,
        Raw::Star => TokenKind::Star,
        Raw::StarEq => TokenKind::StarEq,
        Raw::Slash => TokenKind::Slash,
        Raw::SlashEq => TokenKind::SlashEq,
        Raw::Percent => TokenKind::Percent,
        Raw::PercentEq => TokenKind::PercentEq,
        Raw::Amp => TokenKind::Amp,
        Raw::AmpAmp => TokenKind::AmpAmp,
        Raw::Pipe => TokenKind::Pipe,
        Raw::PipePipe => TokenKind::PipePipe,
        Raw::Caret => TokenKind::Caret,
        Raw::Tilde => TokenKind::Tilde,
        // Handled by `Lexer::token` before reaching here; never a valid token alone.
        Raw::BlockComment | Raw::Number | Raw::Str => TokenKind::Error,
    }
}

// ---------------------------------------------------------------------------
// Characters
// ---------------------------------------------------------------------------

/// Name of a bidirectional control character, as in the Unicode standard.
fn bidi_name(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{202A}' => "LEFT-TO-RIGHT EMBEDDING",
        '\u{202B}' => "RIGHT-TO-LEFT EMBEDDING",
        '\u{202C}' => "POP DIRECTIONAL FORMATTING",
        '\u{202D}' => "LEFT-TO-RIGHT OVERRIDE",
        '\u{202E}' => "RIGHT-TO-LEFT OVERRIDE",
        '\u{2066}' => "LEFT-TO-RIGHT ISOLATE",
        '\u{2067}' => "RIGHT-TO-LEFT ISOLATE",
        '\u{2068}' => "FIRST STRONG ISOLATE",
        '\u{2069}' => "POP DIRECTIONAL ISOLATE",
        _ => return None,
    })
}

/// The fix for an invalid character: look-alikes of ASCII operators get replaced.
fn invalid_fix(c: char) -> InvalidFix {
    let replacement = match c {
        '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{2033}' | '\u{FF02}' => "\"",
        '\u{2018}' | '\u{2019}' | '\u{2032}' | '\u{FF07}' => "'",
        '\u{2010}' | '\u{2011}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => "-",
        '\u{00D7}' => "*",
        '\u{00F7}' => "/",
        '\u{2260}' => "!=",
        '\u{2264}' => "<=",
        '\u{2265}' => ">=",
        '\u{037E}' | '\u{FF1B}' => ";",
        '\u{FF0C}' => ",",
        '\u{FF1A}' => ":",
        '\u{FF08}' => "(",
        '\u{FF09}' => ")",
        '\u{FF1D}' => "=",
        '\u{00A0}' | '\u{2007}' | '\u{2009}' | '\u{202F}' | '\u{3000}' => " ",
        '`' => return InvalidFix::Template,
        '$' => return InvalidFix::Identifier,
        c if c.is_alphabetic() => return InvalidFix::Identifier,
        _ => return InvalidFix::Remove,
    };
    InvalidFix::Replace(replacement)
}

/// How a character is shown in a message: as itself if visible, else as an escape.
fn display_char(c: char) -> String {
    let invisible = c.is_whitespace()
        || c.is_control()
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}');
    if invisible {
        c.escape_unicode().to_string()
    } else {
        c.to_string()
    }
}

/// `U+00A0` notation.
fn codepoint(c: char) -> String {
    format!("U+{:04X}", u32::from(c))
}

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------

/// Why a number literal is malformed. Offsets are relative to the literal.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NumberProblem {
    reason: NumberReason,
    start: usize,
    end: usize,
    /// An edit certain to fix it: replace `start..end` with this text.
    replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NumberReason {
    NoDigits {
        prefix: String,
    },
    InvalidDigit {
        digit: char,
        base: u32,
    },
    /// `octal` is set when every digit is octal: `0755` may mean `0o755`.
    LeadingZero {
        decimal: String,
        octal: Option<String>,
    },
    Underscore,
    MissingExponent,
    Suffix {
        suffix: String,
    },
    RadixFraction,
}

/// Checks a number literal and tells whether it is an integer or a float.
fn classify_number(text: &str) -> Result<TokenKind, NumberProblem> {
    let problem = |reason, start, end, replacement| NumberProblem {
        reason,
        start,
        end,
        replacement,
    };

    let base = match text.get(..2) {
        Some("0x" | "0X") => 16,
        Some("0o" | "0O") => 8,
        Some("0b" | "0B") => 2,
        _ => return classify_decimal(text),
    };
    let body = text.get(2..).unwrap_or("");
    if body.contains('.') {
        return Err(problem(NumberReason::RadixFraction, 0, text.len(), None));
    }
    for (i, c) in body.char_indices() {
        if c != '_' && !c.is_digit(base) {
            let at = 2 + i;
            return Err(problem(
                NumberReason::InvalidDigit { digit: c, base },
                at,
                at + c.len_utf8(),
                None,
            ));
        }
    }
    if !body.chars().any(|c| c.is_digit(base)) {
        let prefix = text.get(..2).unwrap_or("0x").to_owned();
        return Err(problem(
            NumberReason::NoDigits { prefix },
            0,
            text.len(),
            None,
        ));
    }
    if let Some(i) = misplaced_underscore(body, |c| c.is_digit(base)) {
        let at = 2 + i;
        return Err(problem(
            NumberReason::Underscore,
            at,
            at + 1,
            Some(String::new()),
        ));
    }
    Ok(TokenKind::Int)
}

/// Checks a decimal literal: integer part, optional fraction, optional exponent, then nothing.
fn classify_decimal(text: &str) -> Result<TokenKind, NumberProblem> {
    let problem = |reason, start, end, replacement| NumberProblem {
        reason,
        start,
        end,
        replacement,
    };

    let is_part = |c: char| c.is_ascii_digit() || c == '_';
    let int_end = text.find(|c: char| !is_part(c)).unwrap_or(text.len());
    let mut pos = int_end;
    let mut parts = vec![(0, int_end)];
    let mut float = false;

    if text.get(pos..).is_some_and(|r| r.starts_with('.')) {
        float = true;
        let frac_start = pos + 1;
        let frac_end = text
            .get(frac_start..)
            .and_then(|r| r.find(|c: char| !is_part(c)))
            .map_or(text.len(), |i| frac_start + i);
        parts.push((frac_start, frac_end));
        pos = frac_end;
    }

    let rest = text.get(pos..).unwrap_or("");
    if rest.starts_with(['e', 'E']) {
        let after_e = rest.get(1..).unwrap_or("");
        let signed = after_e.starts_with(['+', '-']);
        let digits_start = pos + 1 + usize::from(signed);
        let digits = text.get(digits_start..).unwrap_or("");
        let digits_len = digits.find(|c: char| !is_part(c)).unwrap_or(digits.len());
        if digits_len == 0 {
            if signed || after_e.is_empty() {
                return Err(problem(
                    NumberReason::MissingExponent,
                    pos,
                    digits_start,
                    None,
                ));
            }
            // `1em`: letters after the `e`, a suffix rather than an exponent.
        } else {
            float = true;
            parts.push((digits_start, digits_start + digits_len));
            pos = digits_start + digits_len;
        }
    }

    if let Some(suffix) = text.get(pos..).filter(|s| !s.is_empty()) {
        return Err(problem(
            NumberReason::Suffix {
                suffix: suffix.to_owned(),
            },
            pos,
            text.len(),
            Some(String::new()),
        ));
    }

    let int_part = text.get(..int_end).unwrap_or("");
    let int_digits = int_part.chars().filter(char::is_ascii_digit).count();
    if int_digits > 1 && int_part.starts_with('0') {
        let stripped = int_part.trim_start_matches(['0', '_']);
        let decimal = if stripped.is_empty() { "0" } else { stripped }.to_owned();
        // `0755` may be a decimal 755 or a JavaScript-style octal 0o755: only the author knows.
        let octal = (!float && int_part.chars().all(|c| matches!(c, '0'..='7' | '_')))
            .then(|| format!("0o{decimal}"));
        let replacement = octal.is_none().then(|| decimal.clone());
        return Err(problem(
            NumberReason::LeadingZero { decimal, octal },
            0,
            int_end,
            replacement,
        ));
    }

    for (start, end) in parts {
        let part = text.get(start..end).unwrap_or("");
        if let Some(i) = misplaced_underscore(part, |c| c.is_ascii_digit()) {
            return Err(problem(
                NumberReason::Underscore,
                start + i,
                start + i + 1,
                Some(String::new()),
            ));
        }
    }

    Ok(if float {
        TokenKind::Float
    } else {
        TokenKind::Int
    })
}

/// Offset of the first `_` that is not between two digits.
fn misplaced_underscore(part: &str, is_digit: impl Fn(char) -> bool) -> Option<usize> {
    let chars: Vec<(usize, char)> = part.char_indices().collect();
    chars.iter().enumerate().find_map(|(n, &(i, c))| {
        if c != '_' {
            return None;
        }
        let before = n
            .checked_sub(1)
            .and_then(|p| chars.get(p))
            .is_some_and(|&(_, b)| is_digit(b));
        let after = chars.get(n + 1).is_some_and(|&(_, a)| is_digit(a));
        (!(before && after)).then_some(i)
    })
}

// ---------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------

/// An invalid escape, at `start..end` of a string body.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EscapeProblem {
    kind: EscapeKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EscapeKind {
    /// `\d`: not an escape. Probably meant a literal backslash.
    Unknown { character: char },
    /// `A` or `\x41`: the JavaScript forms, written `\u{41}` in Peras.
    Braces { replacement: String },
    /// `\u{110000}`, `\u{D800}`, `\u{}`: not a Unicode scalar value.
    Unicode,
}

/// Decodes the escapes of a string body, reporting invalid ones to `report`.
fn scan_escapes(body: &str, mut report: impl FnMut(EscapeProblem)) -> String {
    let mut value = String::with_capacity(body.len());
    let mut chars = body.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '\\' {
            value.push(c);
            continue;
        }
        let Some((j, e)) = chars.next() else {
            value.push('\\'); // Trailing backslash of an unterminated string.
            break;
        };
        let simple = match e {
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            '0' => Some('\0'),
            '\\' | '\'' | '"' => Some(e),
            _ => None,
        };
        if let Some(decoded) = simple {
            value.push(decoded);
            continue;
        }
        let after = j + e.len_utf8();
        let rest = body.get(after..).unwrap_or("");
        match e {
            'u' if rest.starts_with('{') => {
                let close = rest.find('}');
                let inner = close.and_then(|end| rest.get(1..end));
                let end = close.map_or(after + 1, |c| after + c + 1);
                let decoded = inner
                    .filter(|hex| (1..=6).contains(&hex.len()))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .and_then(char::from_u32);
                if let Some(d) = decoded {
                    value.push(d);
                } else {
                    report(EscapeProblem {
                        kind: EscapeKind::Unicode,
                        start: i,
                        end,
                    });
                    value.push_str(body.get(i..end).unwrap_or(""));
                }
                skip_to(&mut chars, end);
            }
            'u' | 'x' => {
                let width = if e == 'u' { 4 } else { 2 };
                let hex = rest
                    .get(..width)
                    .filter(|h| h.len() == width && h.chars().all(|c| c.is_ascii_hexdigit()));
                let (kind, end) = match hex {
                    Some(h) => (
                        EscapeKind::Braces {
                            replacement: format!("\\u{{{h}}}"),
                        },
                        after + width,
                    ),
                    None => (EscapeKind::Unicode, after),
                };
                report(EscapeProblem {
                    kind,
                    start: i,
                    end,
                });
                value.push_str(body.get(i..end).unwrap_or(""));
                skip_to(&mut chars, end);
            }
            _ => {
                report(EscapeProblem {
                    kind: EscapeKind::Unknown { character: e },
                    start: i,
                    end: after,
                });
                value.push('\\');
                value.push(e);
            }
        }
    }
    value
}

/// Advances `chars` until the next char starts at or after `end`.
fn skip_to(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, end: usize) {
    while chars.next_if(|&(k, _)| k < end).is_some() {}
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

fn diagnostic(
    code: ErrorCode,
    message: Message,
    label: Label,
    cause: &'static str,
    fix: Fix,
) -> Diagnostic {
    Diagnostic {
        code,
        message,
        labels: vec![label],
        cause: Message::new(cause),
        fix,
    }
}

fn primary(span: Span, message: Message) -> Label {
    Label {
        span,
        message,
        primary: true,
    }
}

fn edit(span: Span, replacement: impl Into<String>) -> Vec<Edit> {
    vec![Edit {
        span,
        replacement: replacement.into(),
    }]
}

fn invalid_character(run: &InvalidRun) -> Diagnostic {
    let fix = match run.fix {
        InvalidFix::Remove => Fix {
            message: Message::new("lex-invalid-character-fix-remove").number("count", run.count),
            edits: edit(run.span, ""),
        },
        InvalidFix::Replace(replacement) => Fix {
            message: Message::new("lex-invalid-character-fix-replace")
                .text("character", display_char(run.first))
                .text("replacement", replacement),
            edits: edit(run.span, replacement),
        },
        InvalidFix::Identifier => Fix {
            message: Message::new("lex-invalid-character-fix-identifier"),
            edits: Vec::new(),
        },
        InvalidFix::Template => Fix {
            message: Message::new("lex-invalid-character-fix-template"),
            edits: Vec::new(),
        },
    };
    diagnostic(
        codes::INVALID_CHARACTER,
        Message::new("lex-invalid-character")
            .text("character", display_char(run.first))
            .text("codepoint", codepoint(run.first))
            .number("count", run.count),
        primary(
            run.span,
            Message::new("lex-invalid-character-label").number("count", run.count),
        ),
        "lex-invalid-character-why",
        fix,
    )
}

fn unterminated_string(span: Span, text: &str, quote: char) -> Diagnostic {
    let quote_text = quote.to_string();
    let opening = Span::new(span.start, span.start + 1);
    let end = Span::new(span.end, span.end);
    let insert = Span::new(closing_point(text, span.end), closing_point(text, span.end));
    let quote = quote_text;
    Diagnostic {
        code: codes::UNTERMINATED_STRING,
        message: Message::new("lex-unterminated-string"),
        labels: vec![
            primary(opening, Message::new("lex-unterminated-string-label")),
            Label {
                span: end,
                message: Message::new("lex-unterminated-string-end-label").text("quote", &quote),
                primary: false,
            },
        ],
        cause: Message::new("lex-unterminated-string-why"),
        fix: Fix {
            message: Message::new("lex-unterminated-string-fix").text("quote", &quote),
            edits: edit(insert, quote),
        },
    }
}

/// Where to close an unterminated string: before a trailing `;` (and the `)`, `]`
/// or `}` just before it), so that `f("abc);` becomes `f("abc");`; else at the end.
fn closing_point(text: &str, end: u32) -> u32 {
    let body = text.trim_end();
    if !body.ends_with(';') {
        return end;
    }
    let kept = body.trim_end_matches([';', ')', ']', '}']);
    if kept.len() <= 1 {
        return end; // Nothing but the quote would stay inside.
    }
    end - offset(text.len() - kept.len())
}

fn unterminated_block_comment(span: Span, end_of_file: u32) -> Diagnostic {
    diagnostic(
        codes::UNTERMINATED_BLOCK_COMMENT,
        Message::new("lex-unterminated-block-comment"),
        primary(
            Span::new(span.start, span.start + 2),
            Message::new("lex-unterminated-block-comment-label"),
        ),
        "lex-unterminated-block-comment-why",
        Fix {
            message: Message::new("lex-unterminated-block-comment-fix"),
            edits: edit(Span::new(end_of_file, end_of_file), "*/"),
        },
    )
}

fn invalid_escape(span: Span, escape: &str, kind: &EscapeKind) -> Diagnostic {
    let (label, fix) = match kind {
        EscapeKind::Unknown { character } => (
            Message::new("lex-invalid-escape-unknown-label")
                .text("character", character.to_string()),
            Fix {
                message: Message::new("lex-invalid-escape-fix-backslash"),
                edits: edit(Span::new(span.start, span.start + 1), "\\\\"),
            },
        ),
        EscapeKind::Braces { replacement } => (
            Message::new("lex-invalid-escape-braces-label"),
            Fix {
                message: Message::new("lex-invalid-escape-fix-braces")
                    .text("replacement", replacement),
                edits: edit(span, replacement.clone()),
            },
        ),
        EscapeKind::Unicode => (
            Message::new("lex-invalid-escape-unicode-label"),
            Fix {
                message: Message::new("lex-invalid-escape-fix-unicode"),
                edits: Vec::new(),
            },
        ),
    };
    diagnostic(
        codes::INVALID_ESCAPE,
        Message::new("lex-invalid-escape").text("escape", escape),
        primary(span, label),
        "lex-invalid-escape-why",
        fix,
    )
}

fn malformed_number(literal: &str, span: Span, problem: &NumberProblem) -> Diagnostic {
    let at = Span::new(
        span.start + offset(problem.start),
        span.start + offset(problem.end),
    );
    let (id, label) = match &problem.reason {
        NumberReason::NoDigits { prefix } => (
            "lex-number-no-digits",
            Message::new("lex-number-no-digits-label").text("prefix", prefix),
        ),
        NumberReason::InvalidDigit { digit, base } => (
            "lex-number-invalid-digit",
            Message::new("lex-number-invalid-digit-label")
                .text("digit", digit.to_string())
                .number("base", i64::from(*base)),
        ),
        NumberReason::LeadingZero { .. } => (
            "lex-number-leading-zero",
            Message::new("lex-number-leading-zero-label"),
        ),
        NumberReason::Underscore => (
            "lex-number-underscore",
            Message::new("lex-number-underscore-label"),
        ),
        NumberReason::MissingExponent => (
            "lex-number-exponent",
            Message::new("lex-number-exponent-label"),
        ),
        NumberReason::Suffix { suffix } => (
            "lex-number-suffix",
            Message::new("lex-number-suffix-label").text("suffix", suffix),
        ),
        NumberReason::RadixFraction => (
            "lex-number-radix-fraction",
            Message::new("lex-number-radix-fraction-label"),
        ),
    };
    let (cause, fix_id) = number_messages(id);
    let fix_message = match &problem.reason {
        NumberReason::Suffix { suffix } => Message::new(fix_id).text("suffix", suffix),
        NumberReason::LeadingZero {
            decimal,
            octal: Some(octal),
        } => Message::new("lex-number-leading-zero-fix-ambiguous")
            .text("decimal", decimal)
            .text("octal", octal),
        _ => Message::new(fix_id),
    };
    diagnostic(
        codes::MALFORMED_NUMBER,
        Message::new("lex-malformed-number").text("literal", literal),
        primary(at, label),
        cause,
        Fix {
            message: fix_message,
            edits: problem
                .replacement
                .as_ref()
                .map(|r| edit(at, r.clone()))
                .unwrap_or_default(),
        },
    )
}

/// The cause and fix message IDs of a number problem.
fn number_messages(id: &str) -> (&'static str, &'static str) {
    match id {
        "lex-number-no-digits" => ("lex-number-no-digits-why", "lex-number-no-digits-fix"),
        "lex-number-invalid-digit" => (
            "lex-number-invalid-digit-why",
            "lex-number-invalid-digit-fix",
        ),
        "lex-number-leading-zero" => ("lex-number-leading-zero-why", "lex-number-leading-zero-fix"),
        "lex-number-underscore" => ("lex-number-underscore-why", "lex-number-underscore-fix"),
        "lex-number-exponent" => ("lex-number-exponent-why", "lex-number-exponent-fix"),
        "lex-number-suffix" => ("lex-number-suffix-why", "lex-number-suffix-fix"),
        _ => (
            "lex-number-radix-fraction-why",
            "lex-number-radix-fraction-fix",
        ),
    }
}

fn strict_equality(span: Span, operator: &str, replacement: &str) -> Diagnostic {
    diagnostic(
        codes::STRICT_EQUALITY,
        Message::new("lex-strict-equality").text("operator", operator),
        primary(
            span,
            Message::new("lex-strict-equality-label").text("replacement", replacement),
        ),
        "lex-strict-equality-why",
        Fix {
            message: Message::new("lex-strict-equality-fix")
                .text("operator", operator)
                .text("replacement", replacement),
            edits: edit(span, replacement),
        },
    )
}

fn bidi_control(span: Span, c: char, name: &str) -> Diagnostic {
    diagnostic(
        codes::BIDI_CONTROL_CHARACTER,
        Message::new("lex-bidi-control")
            .text("codepoint", codepoint(c))
            .text("name", name),
        primary(span, Message::new("lex-bidi-control-label")),
        "lex-bidi-control-why",
        Fix {
            message: Message::new("lex-bidi-control-fix-remove"),
            edits: edit(span, ""),
        },
    )
}

fn source_too_large(len: usize) -> Diagnostic {
    let size = i64::try_from(len).unwrap_or(i64::MAX);
    diagnostic(
        codes::SOURCE_TOO_LARGE,
        Message::new("lex-source-too-large"),
        primary(
            Span::new(0, 0),
            Message::new("lex-source-too-large-label").number("size", size),
        ),
        "lex-source-too-large-why",
        Fix {
            message: Message::new("lex-source-too-large-fix"),
            edits: Vec::new(),
        },
    )
}

#[cfg(test)]
mod tests;
