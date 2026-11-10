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

/// The operands of `set` and `add` run in order: here the key reads the
/// entry the loop is on, through a temporary, before the call.
#[test]
fn a_loop_adds_what_it_reads_in_insertion_order() {
    let source = "export fn f(_n: i32): i64 {\n  let table: Map<String, i32> = Map.new();\n  table.set(\"a\", 11);\n  table.set(\"b\", 5);\n  table.set(\"c\", 21);\n  let rests: Set<i64> = Set.new();\n  for (const e of table) {\n    rests.add((e.1 % 7).toI64());\n  }\n  let out: i64 = 0;\n  for (const r of rests) {\n    out = out * 10 + r;\n  }\n  return out;\n}\n";
    let module = compile(source);
    let (value, heap) =
        run_with_heap(&module, FnId(0), &[Value::I32(1)]).unwrap_or_else(|t| unreachable!("{t:?}"));
    // 11 % 7, 5 % 7, 21 % 7, in that order.
    assert_eq!(value, Value::I64(450));
    assert_eq!(heap.live, Vec::<u32>::new());
}

/// Maps held by a structure, an array, an option and another map: each
/// released with what holds it, and copied with it, deeply.
#[test]
fn maps_held_anywhere_are_released_and_copied_with_their_holder() {
    let source = "struct Index {\n  byId: Map<i64, String>;\n  seen: Set<String>;\n}\n\
        fn fill(index: inout Index, n: i32): void {\n  for (let i = 0; i < n; i++) {\n    index.byId.set(i.toI64(), `id${i}`);\n    index.seen.add(`s${i % 3}`);\n  }\n}\n\
        export fn f(n: i32): i64 {\n  let index = Index(Map.new(), Set.new());\n  fill(&index, n);\n\
          let groups: Map<String, Map<i32, i32>> = Map.new();\n  for (let i = 0; i < n; i++) {\n    let inner = groups.get(`g${i % 2}`).unwrapOr(Map.new());\n    inner.set(i, i * i);\n    groups.set(`g${i % 2}`, inner);\n  }\n\
          let shelves: Array<Map<i32, String>> = [];\n  for (let i = 0; i < 3; i++) {\n    let shelf: Map<i32, String> = Map.new();\n    shelf.set(i, `b${i}`);\n    shelves.push(shelf);\n  }\n  shelves[1].set(9, \"nine\");\n  let copies = shelves.copy();\n  shelves[1].delete(9);\n\
          let maybe: Option<Map<i32, i32>> = None;\n  if (n > 2) {\n    let m: Map<i32, i32> = Map.new();\n    m.set(1, 2);\n    maybe = Some(m);\n  }\n\
          let total: i64 = index.byId.size.toI64() * 1000 + index.seen.size.toI64() * 100;\n  for (const g of groups) {\n    for (const e of g.1) {\n      total = total * 3 + e.1.toI64() + g.0.length.toI64();\n    }\n  }\n\
          total = total * 10 + shelves[1].size.toI64() * 2 + copies[1].size.toI64() + copies[1].get(9).unwrapOr(\"\").length.toI64();\n\
          const sized = match (maybe) {\n    Some(m) => m.size,\n    None => -1,\n  };\n  const twin = index.byId.copy();\n  index.byId.clear();\n\
          return total * 100 + sized.toI64() * 10 + twin.size.toI64() - index.byId.size.toI64();\n}\n";
    let module = compile(source);
    for (n, want) in [(5_i32, f_model(5)), (1, f_model(1)), (0, f_model(0))] {
        let (value, heap) = run_with_heap(&module, function(&module, "f"), &[Value::I32(n)])
            .unwrap_or_else(|t| unreachable!("{t:?}"));
        assert_eq!(value, Value::I64(want), "f({n})");
        assert_eq!(heap.live, Vec::<u32>::new(), "f({n}) leaks");
    }
}

/// What `maps_held_anywhere…`'s program computes.
fn f_model(n: i64) -> i64 {
    let mut total = n * 1000 + n.min(3) * 100;
    for parity in 0..2 {
        for i in (0..n).filter(|i| i % 2 == parity) {
            total = total * 3 + i * i + 2;
        }
    }
    total = total * 10 + 2 + 2 + 4;
    let sized = if n > 2 { 1 } else { -1 };
    total * 100 + sized * 10 + n
}
