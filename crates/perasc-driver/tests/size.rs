//! What a module carries: only the runtime functions it calls, imported by
//! its WebAssembly and kept in the runtime its loader embeds, and only the
//! loader helpers its functions use. The public API, `__perasHeap` and
//! `PerasArray`, stays in every loader of a module that uses the heap.

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_driver::{Allocator, DirectOptions, DirectOutput, build_direct};

fn build(source: &str) -> DirectOutput {
    build_direct(
        source,
        "m.wasm",
        DirectOptions {
            optimize: true,
            allocator: Allocator::Tlsf,
            simd: true,
            threads: false,
        },
        &Localizer::new(Language::English),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
}

/// The names a module imports from the runtime.
fn runtime_imports(wasm: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::ImportSection(section)) = payload {
            for import in section.into_imports().flatten() {
                if import.module == "peras_rt" {
                    names.push(import.name.to_owned());
                }
            }
        }
    }
    names
}

const SPLIT: &str = "export fn words(text: String): i32 {\n  return text.split(\" \").filter((w) => w.length > 0).length;\n}\n";
const SUM: &str = "export fn total(xs: Array<f64>): f64 {\n  let t = 0.0;\n  for (const x of xs) t += x;\n  return t;\n}\n";

#[test]
fn a_module_imports_only_the_runtime_functions_it_calls() {
    let words = runtime_imports(&build(SPLIT).wasm);
    assert!(words.iter().any(|n| n == "str_split_views"), "{words:?}");
    for unused in ["arr_sort", "str_replace", "fmt_float", "str_upper"] {
        assert!(!words.iter().any(|n| n == unused), "{unused} in {words:?}");
    }
    let sum = runtime_imports(&build(SUM).wasm);
    assert!(
        !sum.iter()
            .any(|n| n.starts_with("str_") || n.starts_with("arr_")),
        "{sum:?}"
    );
}

#[test]
fn a_loader_carries_only_the_helpers_its_functions_use() {
    let words = build(SPLIT).js;
    let sum = build(SUM).js;
    // Text crosses in `words`, an array in `total`.
    assert!(words.contains("const $toStr ="));
    assert!(!sum.contains("const $toStr ="));
    assert!(!sum.contains("const $encoder ="));
    assert!(!words.contains("const $readRows ="));
    // The runtime embedded is pruned too: the sum's is the smaller.
    assert!(sum.len() < words.len(), "{} >= {}", sum.len(), words.len());
    // The public API stays in both.
    for js in [&words, &sum] {
        assert!(js.contains("export const __perasHeap"));
        assert!(js.contains("class PerasArray"));
        assert!(js.contains("const $array ="));
    }
}
