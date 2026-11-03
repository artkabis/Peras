//! UI tests for names and types: every file under `tests/ui/check` is parsed and
//! checked, and its diagnostics are compared with an `insta` snapshot. Also checks
//! that every error code of the compiler has at least one UI test.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use perasc_diag::snapshot::diagnostics;
use perasc_diag::{Diagnostic, codes};

/// The pipeline: syntax errors stop it, as in the driver.
fn compile(source: &str) -> Vec<Diagnostic> {
    let parsed = perasc_syntax::parse(source);
    if parsed.diagnostics.is_empty() {
        perasc_check::check(&parsed.ast, source)
    } else {
        parsed.diagnostics
    }
}

#[test]
fn check_ui() {
    insta::glob!("../../../tests/ui", "check/*.{peras,apeiron}", |path| {
        let source = fs::read_to_string(path).unwrap_or_default();
        let mut out = String::new();
        diagnostics(&mut out, &source, &compile(&source));
        insta::assert_snapshot!(out);
    });
}

/// Every error code has at least one UI test, in any stage's directory.
#[test]
fn every_code_has_a_ui_test() {
    let mut seen = BTreeSet::new();
    for stage in ["lexer", "parser", "check"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/ui")
            .join(stage);
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let source = fs::read_to_string(entry.path()).unwrap_or_default();
            seen.extend(compile(&source).iter().map(|d| d.code));
        }
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
