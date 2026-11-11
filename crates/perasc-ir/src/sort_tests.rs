//! `sortBy`, lowered and run in the interpreter: the program the WebAssembly
//! test runs too, against the same model, and no buffer left alive.

#[path = "../../perasc-driver/tests/fixtures/sort.rs"]
mod sort;

use crate::interp::{Value, run_with_heap};
use crate::{FnId, lower, verify};

#[test]
fn the_interpreter_sorts_as_a_stable_sort_does() {
    let parsed = perasc_syntax::parse(sort::PROGRAM);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, sort::PROGRAM);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    for (name, model) in sort::CASES {
        let index = module.functions.iter().position(|f| f.name == name);
        let id = FnId(
            index
                .and_then(|i| u32::try_from(i).ok())
                .unwrap_or(u32::MAX),
        );
        for n in sort::SIZES {
            let (value, heap) = run_with_heap(&module, id, &[Value::I32(n)])
                .unwrap_or_else(|t| unreachable!("{name}({n}): {t:?}"));
            assert_eq!(value, Value::I32(model(n)), "{name}({n})");
            assert_eq!(heap.live, Vec::<u32>::new(), "{name}({n}) leaks");
        }
    }
}
