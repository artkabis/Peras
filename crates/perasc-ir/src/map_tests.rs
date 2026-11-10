//! Maps and sets, lowered and run in the interpreter: the program of random
//! operations the WebAssembly test runs too, against the same model, and no
//! buffer left alive.

#[path = "../../perasc-driver/tests/fixtures/maps.rs"]
mod maps;

use crate::interp::{Value, run_with_heap};
use crate::{FnId, Module, lower, verify};

fn compile(source: &str) -> Module {
    let parsed = perasc_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, source);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    module
}

fn function(module: &Module, name: &str) -> FnId {
    let index = module.functions.iter().position(|f| f.name == name);
    FnId(
        index
            .and_then(|i| u32::try_from(i).ok())
            .unwrap_or(u32::MAX),
    )
}

#[test]
fn the_interpreter_gives_what_the_standard_library_gives() {
    let module = compile(maps::PROGRAM);
    for (name, model) in maps::CASES {
        for n in [0, 7, 400] {
            let (value, heap) = run_with_heap(&module, function(&module, name), &[Value::I32(n)])
                .unwrap_or_else(|t| unreachable!("{name}({n}): {t:?}"));
            assert_eq!(value, Value::I32(model(n)), "{name}({n})");
            assert_eq!(heap.live, Vec::<u32>::new(), "{name}({n}) leaks");
        }
    }
}

#[test]
fn a_map_s_size_get_and_delete() {
    let source = "export fn f(n: i32): i32 {\n  let m: Map<String, i32> = Map.new();\n  for (const w of \"the cat the dog a the cat\".split(\" \")) {\n    const c = m.get(w).unwrapOr(0);\n    m.set(w.copy(), c + n);\n  }\n  const the = m.get(\"the\").unwrapOr(-1);\n  if (m.has(\"zzz\")) {\n    return -5;\n  }\n  m.delete(\"the\");\n  return m.size * 100 + the;\n}\n";
    let module = compile(source);
    let (value, heap) =
        run_with_heap(&module, FnId(0), &[Value::I32(1)]).unwrap_or_else(|t| unreachable!("{t:?}"));
    assert_eq!(value, Value::I32(303));
    assert_eq!(heap.live, Vec::<u32>::new());
}
