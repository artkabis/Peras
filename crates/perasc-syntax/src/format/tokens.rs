//! f1: the author's line breaks kept, the rest made regular.

use super::{Item, closes, opens, space, unary_after};
use crate::token::TokenKind as K;

/// Lays `items` out, the author's line breaks kept.
pub(super) fn format(items: &[Item<'_>]) -> String {
    let mut out = String::new();
    // The indentation of the line each open bracket is on.
    let mut open: Vec<usize> = Vec::new();
    let mut prev: Option<K> = None;
    let mut prefix = false;
    let mut line_indent = 0_usize;
    let mut at_line_start = true;
    // After a `//` comment, the next thing goes on a new line.
    let mut must_break = false;
    // The line began with `@`: what follows is not its continuation.
    let mut line_is_attribute = false;
    for item in items {
        for c in &item.comments {
            if !out.is_empty() && (c.breaks > 0 || must_break) {
                newlines(&mut out, c.breaks.max(1), prev, None);
                at_line_start = true;
            }
            if at_line_start {
                line_indent = open.last().map_or(0, |i| i + 1);
                indent(&mut out, line_indent);
            } else if !matches!(prev, Some(K::LParen | K::LBracket)) {
                out.push(' ');
            }
            out.push_str(c.text);
            at_line_start = false;
            must_break = c.line;
        }
        if item.kind == K::Eof {
            break;
        }
        let after_comment = item.comments.last().is_some_and(|c| !c.line) && !at_line_start;
        if !out.is_empty() && (item.breaks > 0 || must_break) {
            newlines(&mut out, item.breaks.max(1), prev, Some(item.kind));
            at_line_start = true;
        }
        must_break = false;
        if at_line_start {
            let base = if closes(item.kind) {
                open.last().copied().unwrap_or(0)
            } else {
                open.last().map_or(0, |i| i + 1)
            };
            let continues = !closes(item.kind)
                && !line_is_attribute
                && !matches!(
                    prev,
                    None | Some(
                        K::LBrace | K::RBrace | K::Semicolon | K::Comma | K::LParen | K::LBracket
                    )
                );
            line_indent = base + usize::from(continues);
            line_is_attribute = item.kind == K::At;
            indent(&mut out, line_indent);
        } else if after_comment {
            if !matches!(item.kind, K::RParen | K::RBracket | K::Comma | K::Semicolon) {
                out.push(' ');
            }
        } else if let Some(p) = prev
            && space(p, item.kind, prefix, item.spaced)
        {
            out.push(' ');
        }
        out.push_str(item.text);
        at_line_start = false;
        prefix = match item.kind {
            K::Minus | K::Plus | K::PlusPlus | K::MinusMinus => unary_after(prev),
            _ => false,
        };
        if closes(item.kind) {
            open.pop();
        }
        if opens(item.kind) {
            open.push(line_indent);
        }
        prev = Some(item.kind);
    }
    let mut text: String = out
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text
}

/// Ends the line: one blank line kept from a run of them, none after an
/// opening bracket or before a closing one.
fn newlines(out: &mut String, breaks: usize, prev: Option<K>, next: Option<K>) {
    let blank = breaks > 1
        && !prev.is_some_and(|p| matches!(p, K::LBrace | K::LParen | K::LBracket))
        && !next.is_some_and(closes);
    out.push('\n');
    if blank {
        out.push('\n');
    }
}

/// Two spaces a level.
fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}
