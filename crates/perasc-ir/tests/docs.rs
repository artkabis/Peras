//! The documentation is true. Every Peras block of the guide and the reference,
//! in English and French, is compiled, and must do what its label says:
//!
//! - `peras build`: checked and compiled to IR, verified;
//! - `peras`: checked, and not compiled yet (only `E0900`);
//! - `peras error E0xxx`: refused with that code;
//! - `peras planned`: refused by the parser or the checker, so that the page is
//!   updated when the feature lands.
//!
//! The method reference also lists exactly the compiler's built-in methods.

use std::fs;
use std::path::{Path, PathBuf};

use perasc_check::builtins;
use perasc_check::ty::{FloatTy, IntTy, Ty};
use perasc_diag::Diagnostic;

fn docs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs")
}

/// Pages whose code blocks are checked: the guide, the reference, the home page.
fn pages() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for lang in ["en", "fr"] {
        out.push(docs().join(lang).join("index.md"));
        for dir in ["guide", "reference"] {
            let mut files: Vec<PathBuf> = fs::read_dir(docs().join(lang).join(dir))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "md"))
                .collect();
            files.sort();
            out.extend(files);
        }
    }
    out
}

/// The Peras blocks of a page: their label and their code.
fn blocks(page: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = page;
    while let Some(start) = rest.find("```peras") {
        let after = &rest[start + 3..];
        let Some((info, body)) = after.split_once('\n') else {
            break;
        };
        let Some((code, tail)) = body.split_once("\n```") else {
            break;
        };
        out.push((info.trim().to_owned(), code.to_owned()));
        rest = tail;
    }
    out
}

/// The errors of each stage: syntax and checks, then lowering.
fn compile(source: &str) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return (parsed.diagnostics, Vec::new());
    }
    let analysis = perasc_check::analyze(&parsed.ast, source);
    if !analysis.diagnostics.is_empty() {
        return (analysis.diagnostics, Vec::new());
    }
    match perasc_ir::lower(&parsed.ast, &analysis) {
        Ok(module) => {
            assert_eq!(perasc_ir::verify(&module), Ok(()), "{module}");
            (Vec::new(), Vec::new())
        }
        Err(errors) => (Vec::new(), errors),
    }
}

fn codes(errors: &[Diagnostic]) -> Vec<String> {
    errors.iter().map(|d| d.code.to_string()).collect()
}

#[test]
fn every_code_block_does_what_its_label_says() {
    let mut problems = Vec::new();
    let mut counted = 0;
    for path in pages() {
        let page = fs::read_to_string(&path).unwrap_or_default();
        let name = path
            .strip_prefix(docs())
            .unwrap_or(&path)
            .display()
            .to_string();
        for (info, code) in blocks(&page) {
            counted += 1;
            let words: Vec<&str> = info.split_whitespace().collect();
            let (check, lower) = compile(&code);
            let first = code
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim();
            let at = format!("{name}, block starting `{first}`");
            match words.as_slice() {
                ["peras", "build"] => {
                    if !check.is_empty() || !lower.is_empty() {
                        problems.push(format!(
                            "{at}: does not compile: {:?}",
                            codes(&check)
                                .into_iter()
                                .chain(codes(&lower))
                                .collect::<Vec<_>>()
                        ));
                    }
                }
                ["peras"] => {
                    if !check.is_empty() {
                        problems.push(format!("{at}: refused: {:?}", codes(&check)));
                    } else if lower.is_empty() {
                        problems.push(format!("{at}: compiles: label it `peras build`"));
                    } else if codes(&lower).iter().any(|c| c != "E0900") {
                        problems.push(format!("{at}: lowering fails: {:?}", codes(&lower)));
                    }
                }
                ["peras", "error", code] => {
                    let all: Vec<String> = codes(&check).into_iter().chain(codes(&lower)).collect();
                    if !all.iter().any(|c| c == code) {
                        problems.push(format!("{at}: expected {code}, got {all:?}"));
                    }
                }
                ["peras", "planned"] => {
                    if check.is_empty() {
                        problems.push(format!(
                            "{at}: is accepted now: the feature landed, update the page"
                        ));
                    }
                }
                _ => problems.push(format!("{at}: unknown label `{info}`")),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(counted >= 60, "only {counted} blocks checked");
}

#[test]
fn the_method_reference_lists_exactly_the_built_in_methods() {
    let receivers = [
        Ty::Int(IntTy::I32),
        Ty::Float(FloatTy::F64),
        Ty::Array(Box::new(Ty::Int(IntTy::I32))),
        Ty::String,
        Ty::Option(Box::new(Ty::Int(IntTy::I32))),
    ];
    let known: Vec<&str> = receivers.iter().flat_map(builtins::members).collect();
    for lang in ["en", "fr"] {
        let page =
            fs::read_to_string(docs().join(lang).join("reference/methods.md")).unwrap_or_default();
        for name in &known {
            assert!(
                page.contains(&format!("`{name}(")) || page.contains(&format!("`{name}`")),
                "{lang}/reference/methods.md does not document `{name}`"
            );
        }
        // Every `name(` written in a table is a method the compiler knows.
        for line in page.lines().filter(|l| l.starts_with('|')) {
            let first_cell = line.split('|').nth(1).unwrap_or("");
            for part in first_cell.split('`').skip(1).step_by(2) {
                let Some((name, _)) = part.split_once('(') else {
                    continue;
                };
                assert!(
                    known.contains(&name),
                    "{lang}/reference/methods.md documents `{name}`, which the compiler does not know"
                );
            }
        }
    }
}
