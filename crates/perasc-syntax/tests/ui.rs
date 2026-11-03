//! UI tests: every `.peras` and `.apeiron` file under `tests/ui/lexer` is lexed, and
//! its tokens and diagnostics are compared with an `insta` snapshot.
//!
//! Diagnostics are shown as message IDs and arguments, independent of the language.
//! Review new or changed snapshots with `cargo insta review`; in CI an unreviewed
//! snapshot fails the build.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use perasc_diag::{Arg, Message, Span, codes};
use perasc_syntax::lex;

/// `line:column`, both starting at 1, columns counted in characters.
fn position(source: &str, offset: u32) -> String {
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

/// Renders the tokens and diagnostics of a source as stable text.
fn render(source: &str) -> String {
    let lexed = lex(source);
    let mut out = String::from("tokens:\n");
    for token in &lexed.tokens {
        let _ = writeln!(
            out,
            "  {:<7} {:?} {:?}",
            position(source, token.span.start),
            token.kind,
            token.text(source)
        );
    }
    out.push_str("diagnostics:\n");
    if lexed.diagnostics.is_empty() {
        out.push_str("  none\n");
    }
    for d in &lexed.diagnostics {
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
    out
}

#[test]
fn lexer_ui() {
    insta::glob!("../../../tests/ui", "lexer/*.{peras,apeiron}", |path| {
        let source = fs::read_to_string(path).unwrap_or_default();
        insta::assert_snapshot!(render(&source));
    });
}

/// Every lexical error code has at least one UI test.
#[test]
fn every_lexer_code_has_a_ui_test() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ui/lexer");
    let mut seen = BTreeSet::new();
    for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let source = fs::read_to_string(entry.path()).unwrap_or_default();
        seen.extend(lex(&source).diagnostics.iter().map(|d| d.code));
    }
    for entry in codes::REGISTRY {
        // E0008 needs a 4 GiB file: covered by a unit test with a lowered limit instead.
        if entry.code == codes::SOURCE_TOO_LARGE || entry.retired {
            continue;
        }
        assert!(
            seen.contains(&entry.code),
            "no UI test produces {}",
            entry.code
        );
    }
}
