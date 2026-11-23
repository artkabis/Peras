//! Decision 0040, prototyped: `test "name" { … }` blocks, rewritten from the
//! tokens of a source before it is parsed, so that every line keeps its
//! number. A normal build blanks them out, a test build makes each an
//! exported function giving `""` or the message of its first failure.
//!
//! - `assert(cond);` fails when `cond` is false (v1 and v2);
//! - `assertEqual(a, b);` fails when `a != b` (v1 and v2);
//! - `expect(a).toBe(b);` and `expect(a).toEqual(b);` fail when `a != b`,
//!   saying both values (v2); `expect(r).toBeOk();` and `.toBeErr();` test
//!   a result (v2).

use crate::lexer::{lex, string_value};
use crate::token::{Token, TokenKind};

/// What to make of the `test` blocks of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tests {
    /// Leave the source as it is: no option of decision 0040.
    Keep,
    /// Blank them out: a normal build.
    Strip,
    /// Make each an exported function, under option v1 or v2.
    Build(u8),
}

/// The option of decision 0040 the environment names, and whether this is a
/// test build: `PERAS_PROTO_TEST=v1|v2`, `PERAS_TEST_BUILD=1`.
#[must_use]
pub fn tests_from_env() -> Tests {
    let option = match std::env::var("PERAS_PROTO_TEST").as_deref() {
        Ok("v1") => 1,
        Ok("v2") => 2,
        _ => return Tests::Keep,
    };
    if std::env::var("PERAS_TEST_BUILD").as_deref() == Ok("1") {
        Tests::Build(option)
    } else {
        Tests::Strip
    }
}

/// The function a test becomes: `__peras_test_0`, `__peras_test_1`…
#[must_use]
pub fn test_function(index: usize) -> String {
    format!("__peras_test_{index}")
}

/// The source rewritten as `how` says, and the name of each test, in order.
#[must_use]
pub fn prepare(source: &str, file: &str, how: Tests) -> (String, Vec<String>) {
    if how == Tests::Keep {
        return (source.to_owned(), Vec::new());
    }
    let tokens: Vec<Token> = lex(source).tokens;
    let text = |t: &Token| {
        source
            .get(t.span.start as usize..t.span.end as usize)
            .unwrap_or("")
    };
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut names = Vec::new();
    let mut depth = 0_i32;
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        match t.kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => depth -= 1,
            _ => {}
        }
        let opens_test = depth == 0
            && t.kind == TokenKind::Ident
            && text(t) == "test"
            && tokens
                .get(i + 1)
                .is_some_and(|n| n.kind == TokenKind::String)
            && tokens
                .get(i + 2)
                .is_some_and(|n| n.kind == TokenKind::LBrace);
        if !opens_test {
            i += 1;
            continue;
        }
        let open = i + 2;
        let Some(close) = matching(&tokens, open, TokenKind::LBrace, TokenKind::RBrace) else {
            break;
        };
        let start = t.span.start as usize;
        let end = tokens[close].span.end as usize;
        match how {
            Tests::Strip => edits.push((start, end, blank(source.get(start..end).unwrap_or("")))),
            Tests::Build(option) => {
                let name = string_value(text(&tokens[i + 1]));
                let head = format!("export fn {}(): String {{", test_function(names.len()));
                edits.push((start, tokens[open].span.end as usize, head));
                rewrite_body(source, file, &tokens, (open, close), option, &mut edits);
                edits.push((
                    tokens[close].span.start as usize,
                    end,
                    "return \"\";\n}".to_owned(),
                ));
                names.push(name);
            }
            Tests::Keep => {}
        }
        i = close + 1;
    }
    edits.sort_by_key(|e| e.0);
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for (start, end, with) in edits {
        if start < at {
            continue;
        }
        out.push_str(source.get(at..start).unwrap_or(""));
        out.push_str(&with);
        at = end;
    }
    out.push_str(source.get(at..).unwrap_or(""));
    (out, names)
}

/// The statements of a test's body that test something, rewritten.
fn rewrite_body(
    source: &str,
    file: &str,
    tokens: &[Token],
    (open, close): (usize, usize),
    option: u8,
    edits: &mut Vec<(usize, usize, String)>,
) {
    let text = |t: &Token| {
        source
            .get(t.span.start as usize..t.span.end as usize)
            .unwrap_or("")
    };
    let slice = |a: usize, b: usize| source.get(a..b).unwrap_or("").trim().to_owned();
    let mut k = open + 1;
    while k < close {
        let t = &tokens[k];
        let at_statement = matches!(
            tokens[k - 1].kind,
            TokenKind::LBrace | TokenKind::RBrace | TokenKind::Semicolon
        );
        let word = text(t);
        if !(at_statement && t.kind == TokenKind::Ident) {
            k += 1;
            continue;
        }
        let Some(paren) = tokens.get(k + 1).filter(|p| p.kind == TokenKind::LParen) else {
            k += 1;
            continue;
        };
        let Some(shut) = matching(tokens, k + 1, TokenKind::LParen, TokenKind::RParen) else {
            return;
        };
        let line = 1 + source
            .get(..t.span.start as usize)
            .unwrap_or("")
            .matches('\n')
            .count();
        let at = format!("{file}:{line}");
        let inner = (paren.span.end as usize, tokens[shut].span.start as usize);
        let args = split_args(tokens, k + 1, shut);
        let (stop, with) = match word {
            "assert" => {
                let cond = slice(inner.0, inner.1);
                let what = quoted(&format!("{at}: assert({cond}) failed"));
                (shut, format!("if (!({cond})) {{ return {what}; }}"))
            }
            "assertEqual" if args.len() == 2 => {
                let (a, b) = (slice(args[0].0, args[0].1), slice(args[1].0, args[1].1));
                let what = quoted(&format!("{at}: assertEqual({a}, {b}) failed"));
                (shut, format!("if (({a}) != ({b})) {{ return {what}; }}"))
            }
            "expect" if option == 2 => {
                let value = slice(inner.0, inner.1);
                let Some((method, call_end, wanted)) = matcher(source, tokens, shut) else {
                    k += 1;
                    continue;
                };
                let with = match method.as_str() {
                    "toBe" | "toEqual" => format!(
                        "if (({value}) != ({wanted})) {{ return `{}: expected ${{{wanted}}}, got ${{{value}}}`; }}",
                        template_safe(&at)
                    ),
                    "toBeOk" => format!(
                        "if (!(({value}).isOk())) {{ return {}; }}",
                        quoted(&format!("{at}: expected Ok, got Err"))
                    ),
                    "toBeErr" => format!(
                        "if (!(({value}).isErr())) {{ return {}; }}",
                        quoted(&format!("{at}: expected Err, got Ok"))
                    ),
                    _ => {
                        k += 1;
                        continue;
                    }
                };
                (call_end, with)
            }
            _ => {
                k += 1;
                continue;
            }
        };
        // The statement's `;` goes with it.
        let semi = tokens
            .get(stop + 1)
            .filter(|s| s.kind == TokenKind::Semicolon)
            .map_or(tokens[stop].span.end, |s| s.span.end) as usize;
        edits.push((t.span.start as usize, semi, with));
        k = stop + 1;
    }
}

/// `.toBe(b)` after `expect(a)`: the method, the last token of the call, and
/// the argument's text.
fn matcher(source: &str, tokens: &[Token], shut: usize) -> Option<(String, usize, String)> {
    let dot = tokens.get(shut + 1).filter(|t| t.kind == TokenKind::Dot)?;
    let _ = dot;
    let name = tokens
        .get(shut + 2)
        .filter(|t| t.kind == TokenKind::Ident)?;
    tokens
        .get(shut + 3)
        .filter(|t| t.kind == TokenKind::LParen)?;
    let end = matching(tokens, shut + 3, TokenKind::LParen, TokenKind::RParen)?;
    let method = source
        .get(name.span.start as usize..name.span.end as usize)?
        .to_owned();
    let arg = source
        .get(tokens[shut + 3].span.end as usize..tokens[end].span.start as usize)?
        .trim()
        .to_owned();
    Some((method, end, arg))
}

/// The token that closes the one at `open`.
fn matching(tokens: &[Token], open: usize, left: TokenKind, right: TokenKind) -> Option<usize> {
    let mut depth = 0;
    for (j, t) in tokens.iter().enumerate().skip(open) {
        if t.kind == left {
            depth += 1;
        } else if t.kind == right {
            depth -= 1;
            if depth == 0 {
                return Some(j);
            }
        }
    }
    None
}

/// The byte ranges of the arguments between the parentheses at `open` and
/// `shut`, split at their top-level commas.
fn split_args(tokens: &[Token], open: usize, shut: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut start = tokens[open].span.end as usize;
    for t in &tokens[open + 1..shut] {
        match t.kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => depth -= 1,
            TokenKind::Comma if depth == 0 => {
                out.push((start, t.span.start as usize));
                start = t.span.end as usize;
            }
            _ => {}
        }
    }
    out.push((start, tokens[shut].span.start as usize));
    out
}

/// Spaces where `text` has anything but a line break.
fn blank(text: &str) -> String {
    text.chars()
        .map(|c| if c == '\n' { '\n' } else { ' ' })
        .collect()
}

/// A string literal of `text`.
fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `text` safe inside a template literal.
fn template_safe(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('$', "\\$")
}
