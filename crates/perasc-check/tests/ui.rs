//! UI tests for names and types: every file under `tests/ui/check` is parsed and
//! checked, and its diagnostics are compared with an `insta` snapshot.

use std::fs;

use perasc_diag::Diagnostic;
use perasc_diag::snapshot::diagnostics;

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

// The check that every error code has a UI test lives in perasc-ir, which runs
// the whole pipeline, code generation included.
