//! UI tests: every `.peras` and `.apeiron` file under `tests/ui/lexer` is lexed, and
//! every file under `tests/ui/parser` is parsed; the tokens or the syntax tree, and
//! the diagnostics, are compared with an `insta` snapshot.
//!
//! Diagnostics are shown as message IDs and arguments, independent of the language.
//! Review new or changed snapshots with `cargo insta review`; in CI an unreviewed
//! snapshot fails the build.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use perasc_diag::codes;
use perasc_diag::snapshot::{diagnostics, position};
use perasc_syntax::{lex, parse};

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
    diagnostics(&mut out, source, &lexed.diagnostics);
    out
}

/// Renders the syntax tree and diagnostics of a source as stable text.
fn render_parsed(source: &str) -> String {
    let parsed = parse(source);
    let mut out = String::from("ast:\n");
    for line in parsed.ast.dump().lines() {
        let _ = writeln!(out, "  {line}");
    }
    diagnostics(&mut out, source, &parsed.diagnostics);
    out
}

#[test]
fn lexer_ui() {
    insta::glob!("../../../tests/ui", "lexer/*.{peras,apeiron}", |path| {
        let source = fs::read_to_string(path).unwrap_or_default();
        insta::assert_snapshot!(render(&source));
    });
}

#[test]
fn parser_ui() {
    insta::glob!("../../../tests/ui", "parser/*.{peras,apeiron}", |path| {
        let source = fs::read_to_string(path).unwrap_or_default();
        insta::assert_snapshot!(render_parsed(&source));
    });
}

/// Every lexer and parser error code has at least one UI test; the name and type
/// codes are covered by `perasc-check`.
#[test]
fn every_syntax_code_has_a_ui_test() {
    let mut seen = BTreeSet::new();
    for stage in ["lexer", "parser"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/ui")
            .join(stage);
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let source = fs::read_to_string(entry.path()).unwrap_or_default();
            seen.extend(parse(&source).diagnostics.iter().map(|d| d.code));
        }
    }
    for entry in codes::REGISTRY {
        // E0008 needs a 4 GiB file: covered by a unit test with a lowered limit instead.
        if entry.code == codes::SOURCE_TOO_LARGE || entry.retired || entry.code.number() >= 200 {
            continue;
        }
        assert!(
            seen.contains(&entry.code),
            "no UI test produces {}",
            entry.code
        );
    }
}
