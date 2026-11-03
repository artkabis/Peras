//! A stable, language-independent text form of diagnostics, for snapshot tests.
//!
//! Messages appear as Fluent IDs with their arguments, and positions as
//! `line:column`, so a snapshot changes only when a diagnostic really changes.

use std::fmt::Write as _;

use crate::{Arg, Diagnostic, Message, Span};

/// `line:column`, both starting at 1, columns counted in characters.
#[must_use]
pub fn position(source: &str, offset: u32) -> String {
    let offset = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(source.len());
    let before = source.get(..offset).unwrap_or("");
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    format!("{line}:{column}")
}

fn range(source: &str, span: Span) -> String {
    format!(
        "{}-{}",
        position(source, span.start),
        position(source, span.end)
    )
}

fn message(m: &Message) -> String {
    if m.args.is_empty() {
        return m.id.to_owned();
    }
    let args: Vec<String> = m
        .args
        .iter()
        .map(|(name, value)| match value {
            Arg::Text(text) => format!("{name}: {text:?}"),
            Arg::Number(n) => format!("{name}: {n}"),
        })
        .collect();
    format!("{} {{ {} }}", m.id, args.join(", "))
}

/// Appends `diagnostics:` and one block per diagnostic, or `none`.
pub fn diagnostics(out: &mut String, source: &str, diagnostics: &[Diagnostic]) {
    out.push_str("diagnostics:\n");
    if diagnostics.is_empty() {
        out.push_str("  none\n");
    }
    for d in diagnostics {
        let _ = writeln!(out, "  error[{}] {}", d.code, message(&d.message));
        for label in &d.labels {
            let kind = if label.primary {
                "primary"
            } else {
                "secondary"
            };
            let _ = writeln!(
                out,
                "    {kind} {}: {}",
                range(source, label.span),
                message(&label.message)
            );
        }
        let _ = writeln!(out, "    cause: {}", message(&d.cause));
        let _ = writeln!(out, "    fix: {}", message(&d.fix.message));
        for edit in &d.fix.edits {
            let _ = writeln!(
                out,
                "      {} -> {:?}",
                range(source, edit.span),
                edit.replacement
            );
        }
    }
}
