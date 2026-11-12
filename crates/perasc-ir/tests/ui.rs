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

/// The owned module the managed tests import, as `./stats.peras`.
const STATS: &str = "export fn mean(xs: Array<f64>): f64 { return xs[0]; }\n\
                     export struct Point { x: f64 }\n\
                     export struct Pair<T> { first: T }\n\
                     export fn origin(): Point { return Point(0.0); }\n";

/// A managed module's checks, importing `./stats.peras`: it is not lowered,
/// since it compiles to JavaScript.
fn check_managed(source: &str) -> Result<(), Vec<Diagnostic>> {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics);
    }
    let context = perasc_check::Context {
        managed: true,
        imports: vec![perasc_check::ImportedModule {
            from: "./stats.peras".to_owned(),
            functions: perasc_check::exports_of(STATS),
            types: perasc_check::module_exports(STATS)
                .map(|(_, types)| types)
                .unwrap_or_default(),
        }],
        path: String::new(),
    };
    let analysis = perasc_check::analyze_with(&parsed.ast, source, &context);
    if analysis.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(analysis.diagnostics)
    }
}

/// A test file's errors, in the mode its extension gives.
fn errors_of(path: &Path, source: &str) -> Vec<Diagnostic> {
    if path.extension().is_some_and(|e| e == "apeiron") {
        check_managed(source).err().unwrap_or_default()
    } else {
        compile(source).err().unwrap_or_default()
    }
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
            seen.extend(errors_of(&entry.path(), &source).iter().map(|d| d.code));
        }
    }
    for entry in codes::REGISTRY {
        // E0008 needs a 4 GiB file: covered by a unit test with a lowered limit instead.
        // E0250 needs several files: covered by `perasc-driver/tests/modules.rs`.
        if entry.code == codes::SOURCE_TOO_LARGE
            || entry.code == codes::IMPORT_CYCLE
            || entry.retired
        {
            continue;
        }
        assert!(
            seen.contains(&entry.code),
            "no UI test produces {}",
            entry.code
        );
    }
}

/// The exit criterion of the kickoff plan: every diagnostic of every UI test,
/// rendered as in a terminal, shows its location, an excerpt with a marker, a
/// cause and a fix, in English and in French, and the French is a translation.
#[test]
fn every_diagnostic_shows_excerpt_cause_and_fix_in_both_languages() {
    use perasc_diag::i18n::Localizer;
    use perasc_diag::messages::Language;
    use perasc_diag::render::{RenderOptions, render};
    use perasc_diag::source::SourceFile;

    let mut checked = 0;
    for stage in ["lexer", "parser", "check", "build"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/ui")
            .join(stage);
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path().display().to_string();
            let source = fs::read_to_string(entry.path()).unwrap_or_default();
            let errors = errors_of(&entry.path(), &source);
            if errors.is_empty() {
                continue;
            }
            let file = SourceFile::new(&path, &source);
            for d in &errors {
                let mut causes = Vec::new();
                for (language, why, fix, details) in [
                    (
                        Language::English,
                        "= why: ",
                        "= fix: ",
                        "= details: perasc explain ",
                    ),
                    (
                        Language::French,
                        "= pourquoi : ",
                        "= correction : ",
                        "= détails : perasc explain ",
                    ),
                ] {
                    let localizer = Localizer::new(language);
                    let text = render(d, &file, &localizer, RenderOptions { color: false });
                    let context = format!("{path}, {language:?}:\n{text}");
                    assert!(text.contains(&format!("--> {path}:")), "{context}");
                    assert!(
                        text.lines()
                            .any(|l| l.trim_start().starts_with('|') && l.contains('^')),
                        "no marked excerpt: {context}"
                    );
                    let line = |prefix: &str| {
                        text.lines()
                            .find_map(|l| l.trim_start().strip_prefix(prefix))
                            .map(str::to_owned)
                    };
                    let cause = line(why).unwrap_or_default();
                    assert!(cause.len() > 10, "no cause: {context}");
                    assert!(
                        line(fix).is_some_and(|f| !f.is_empty()),
                        "no fix: {context}"
                    );
                    assert_eq!(
                        line(details),
                        Some(d.code.to_string()),
                        "no explain: {context}"
                    );
                    causes.push(cause);
                }
                assert_ne!(causes[0], causes[1], "{path}: the cause is not translated");
                checked += 1;
            }
        }
    }
    // Guards against a test that checks nothing.
    assert!(checked >= 46, "only {checked} diagnostics checked");
}

/// The code block under a heading of a page, if any. Pages write invisible
/// characters as `<U+202E>`, never as themselves: they are decoded here.
fn block_after(page: &str, heading: &str) -> Option<(String, bool)> {
    let rest = page.split_once(&format!("\n## {heading}\n"))?.1;
    let rest = rest.split("\n## ").next()?;
    let (fence, code) = ["```peras\n", "```apeiron\n", "```text\n"]
        .iter()
        .filter_map(|fence| Some((*fence, rest.split_once(fence)?)))
        .min_by_key(|(_, (before, _))| before.len())
        .map(|(fence, (_, after))| (fence, after))?;
    let managed = fence == "```apeiron\n";
    let code = code.split_once("\n```").map(|(code, _)| code)?;
    let mut out = String::new();
    let mut rest = code;
    while let Some((before, after)) = rest.split_once("<U+") {
        out.push_str(before);
        let decoded = after.split_once('>').and_then(|(hex, tail)| {
            let c = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)?;
            Some((c, tail))
        });
        if let Some((c, tail)) = decoded {
            out.push(c);
            rest = tail;
        } else {
            out.push_str("<U+");
            rest = after;
        }
    }
    out.push_str(rest);
    Some((out, managed))
}

/// The documentation of every code is true: its example produces the code, and
/// its fix no longer does, in English and in French.
#[test]
fn every_documented_example_and_fix_is_true() {
    use perasc_diag::messages::Language;

    let mut checked = 0;
    let mut problems = Vec::new();
    for entry in codes::REGISTRY {
        for (language, example, fix) in [
            (Language::English, "Example", "Fix"),
            (Language::French, "Exemple", "Correction"),
        ] {
            let page = entry.docs(language);
            let context = format!("{} ({language:?})", entry.code);
            // Pages show fragments: `…` stands for anything, and statements that
            // are not a declaration (E0110 at the top level) go inside a function.
            let produces = |(code, managed): &(String, bool)| {
                let code = code.replace('…', "");
                let codes_of = |source: &str| -> Vec<String> {
                    let errors = if *managed {
                        check_managed(source).err()
                    } else {
                        compile(source).err()
                    };
                    errors
                        .unwrap_or_default()
                        .iter()
                        .map(|d| d.code.to_string())
                        .collect()
                };
                let mut found = codes_of(&code);
                if entry.code.to_string() != "E0110" && found.iter().any(|c| c == "E0110") {
                    found = codes_of(&format!("fn example() {{\n{code}\n}}\n"));
                }
                found.contains(&entry.code.to_string())
            };
            // E0111 needs more nesting than a page can show: its example is a sketch.
            // E0250 needs several files, which a page shows one by one: its
            // example is checked by `perasc-driver/tests/modules.rs`.
            if entry.code.to_string() == "E0111" || entry.code == codes::IMPORT_CYCLE {
                continue;
            }
            if let Some(code) = block_after(page, example).as_ref() {
                if !produces(code) {
                    problems.push(format!("{context}: the example does not produce it"));
                }
                checked += 1;
            }
            if let Some(code) = block_after(page, fix).as_ref()
                && produces(code)
            {
                problems.push(format!("{context}: the fix still produces it"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // E0008 has no example (it needs a 4 GiB file), E0111 is a sketch, and
    // E0250 needs several files.
    assert!(
        checked >= 2 * (codes::REGISTRY.len() - 3),
        "only {checked} examples checked"
    );
}
