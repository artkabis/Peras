//! UI tests for names and types: every file under `tests/ui/check` is parsed and
//! checked, and its diagnostics are compared with an `insta` snapshot.

use std::fs;

use perasc_diag::Diagnostic;
use perasc_diag::snapshot::diagnostics;

/// The owned module the managed tests import, as `./stats.peras`.
const STATS: &str = "export fn mean(xs: Array<f64>): f64 { return xs[0]; }\n\
                     export struct Point { x: f64 }\n\
                     export struct Pair<T> { first: T }\n\
                     export fn origin(): Point { return Point(0.0); }\n";

/// The pipeline: syntax errors stop it, as in the driver. A `.apeiron` file
/// is checked as a managed module, importing `./stats.peras`.
fn compile(source: &str, managed: bool) -> Vec<Diagnostic> {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return parsed.diagnostics;
    }
    let context = perasc_check::Context {
        managed,
        imports: vec![perasc_check::ImportedModule {
            from: "./stats.peras".to_owned(),
            functions: perasc_check::exports_of(STATS),
            types: perasc_check::module_exports(STATS)
                .map(|(_, types)| types)
                .unwrap_or_default(),
        }],
        path: String::new(),
    };
    perasc_check::analyze_with(&parsed.ast, source, &context).diagnostics
}

#[test]
fn check_ui() {
    insta::glob!("../../../tests/ui", "check/*.{peras,apeiron}", |path| {
        let source = fs::read_to_string(path).unwrap_or_default();
        let mut out = String::new();
        let managed = path.extension().is_some_and(|e| e == "apeiron");
        diagnostics(&mut out, &source, &compile(&source, managed));
        insta::assert_snapshot!(out);
    });
}

// The check that every error code has a UI test lives in perasc-ir, which runs
// the whole pipeline, code generation included.
