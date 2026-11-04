//! UI tests for code generation: every file under `tests/ui/build` goes through the
//! whole pipeline, and its IR or its diagnostics are compared with an `insta`
//! snapshot. Also checks that every error code of the compiler has a UI test.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use perasc_diag::snapshot::diagnostics;
use perasc_diag::{Diagnostic, codes};

/// The whole pipeline up to the IR: the first stage with errors stops it.
fn compile(source: &str) -> Result<perasc_ir::Module, Vec<Diagnostic>> {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics);
    }
    let analysis = perasc_check::analyze(&parsed.ast, source);
    if !analysis.diagnostics.is_empty() {
        return Err(analysis.diagnostics);
    }
    perasc_ir::lower(&parsed.ast, &analysis)
}

#[test]
fn build_ui() {
    insta::glob!("../../../tests/ui", "build/*.peras", |path| {
        let source = fs::read_to_string(path).unwrap_or_default();
        let out = match compile(&source) {
            Ok(module) => format!("ir:\n{module}"),
            Err(errors) => {
                let mut out = String::new();
                diagnostics(&mut out, &source, &errors);
                out
            }
        };
        insta::assert_snapshot!(out);
    });
}

/// Every error code has at least one UI test, in any stage's directory.
#[test]
fn every_code_has_a_ui_test() {
    let mut seen = BTreeSet::new();
    for stage in ["lexer", "parser", "check", "build"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/ui")
            .join(stage);
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let source = fs::read_to_string(entry.path()).unwrap_or_default();
            if let Err(errors) = compile(&source) {
                seen.extend(errors.iter().map(|d| d.code));
            }
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
