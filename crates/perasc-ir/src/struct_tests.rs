//! Structures, lowered to their scalars and run in the interpreter, unoptimized
//! and optimized: values, copies, fields, calls with several results, and the
//! order of evaluation, seen through which trap a program stops on.

use crate::interp::{Trap, Value, run};
use crate::{FnId, Module, lower, optimize, verify};

fn compile(source: &str) -> Module {
    let parsed = perasc_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, source);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    module
}

/// Runs function 0 on each input, unoptimized and optimized: both must give
/// `expected`.
fn gives(source: &str, cases: &[(Vec<Value>, Result<Value, Trap>)]) -> Module {
    let module = compile(source);
    let mut optimized = module.clone();
    optimize(&mut optimized);
    assert_eq!(verify(&optimized), Ok(()), "{optimized}");
    for (args, expected) in cases {
        for m in [&module, &optimized] {
            assert_eq!(&run(m, FnId(0), args), expected, "{args:?}\n{m}");
        }
    }
    module
}

const VEC: &str = "struct V { x: i32; y: i32; }\n";

#[test]
fn a_structure_is_its_scalars() {
    let source = format!(
        "{VEC}export fn f(a: i32, b: i32): i32 {{\n  let p = V(a, b);\n  let q = p;\n  q.x = 100;\n  q.y += 1;\n  q.y++;\n  return p.x * 1000 + q.x + q.y;\n}}\n"
    );
    let module = gives(
        &source,
        &[(vec![Value::I32(3), Value::I32(4)], Ok(Value::I32(3106)))],
    );
    // Two locals for `p`, two for `q`: no heap, no call.
    let text = module.to_string();
    assert!(
        text.contains("local %2 p_x: i32") && text.contains("local %5 q_y: i32"),
        "{text}"
    );
    assert!(!text.contains("alloc") && !text.contains("call"), "{text}");
}

#[test]
fn nested_structures_flatten_and_copy() {
    let source = format!(
        "{VEC}struct Box {{ low: V; high: V; ok: bool; }}\n\
         export fn f(n: i32): i32 {{\n  let b = Box(V(n, n + 1), V(n * 2, n * 3), true);\n  let low = b.low;\n  b.low = b.high;\n  b.high = low;\n  b.high.y = 7;\n  if (!b.ok) return -1;\n  return b.low.x * 100 + b.low.y * 10 + b.high.y;\n}}\n"
    );
    // n = 2: low = (2, 3), high = (4, 6); swapped, then high.y = 7.
    gives(&source, &[(vec![Value::I32(2)], Ok(Value::I32(467)))]);
}

#[test]
fn assigning_a_structure_to_itself_swaps_its_fields() {
    let source = format!(
        "{VEC}export fn f(a: i32, b: i32): i32 {{\n  let p = V(a, b);\n  p = V(p.y, p.x);\n  return p.x * 10 + p.y;\n}}\n"
    );
    gives(
        &source,
        &[(vec![Value::I32(1), Value::I32(2)], Ok(Value::I32(21)))],
    );
}

#[test]
fn functions_take_and_return_structures() {
    let source = format!(
        "{VEC}export fn f(n: i32): i32 {{\n  let v = add(add(V(n, 1), V(10, 20)), double(V(n, n)));\n  return v.x * 1000 + v.y + one(V(n, 5)).x;\n}}\n\
         fn add(a: V, b: V): V {{ return V(a.x + b.x, a.y + b.y); }}\n\
         fn double(v: V): V {{ return add(v, v); }}\n\
         struct One {{ x: i32; }}\n\
         fn one(v: V): One {{ return One(v.y); }}\n"
    );
    // n = 3: (3 + 10 + 6, 1 + 20 + 6) = (19, 27); one(..).x = 5.
    let module = gives(&source, &[(vec![Value::I32(3)], Ok(Value::I32(19_032)))]);
    let text = module.to_string();
    // Two results for `add`, one for `one`: a single scalar needs no `call many`.
    assert!(
        text.contains("fn add(%0 a_x: i32, %1 a_y: i32, %2 b_x: i32, %3 b_y: i32) -> (i32, i32)"),
        "{text}"
    );
    assert!(
        text.contains("fn one(%0 v_x: i32, %1 v_y: i32) -> i32"),
        "{text}"
    );
}

/// `boom` divides by zero (`Arithmetic`); `spin` calls itself until the stack
/// runs out (`Depth`).
const TRAPS: &str = "fn boom(n: i32): i32 { return 1 / (n - n); }\n\
                     fn spin(n: i32): V { return spin(n + 1); }\n";

#[test]
fn hoisting_a_call_keeps_the_order_of_evaluation() {
    for (body, expected) in [
        // `boom` is left of `spin`: it must run, and trap, first.
        ("return boom(n) + spin(n).x;", Err(Trap::Arithmetic)),
        ("return spin(n).x + boom(n);", Err(Trap::Depth)),
        // In arguments too, flattened or not.
        ("return pair(boom(n), spin(n));", Err(Trap::Arithmetic)),
        (
            "let v = V(boom(n), spin(n).y); return v.x;",
            Err(Trap::Arithmetic),
        ),
        // `spin` must not run when `&&` or `||` has decided.
        (
            "if (n > 5 && spin(n).x > 0) return 1; return 2;",
            Ok(Value::I32(2)),
        ),
        (
            "if (n < 5 || spin(n).x > 0) return 1; return 2;",
            Ok(Value::I32(1)),
        ),
    ] {
        let source = format!(
            "{VEC}export fn f(n: i32): i32 {{ {body} }}\n{TRAPS}fn pair(a: i32, v: V): i32 {{ return a + v.x; }}\n"
        );
        gives(&source, &[(vec![Value::I32(1)], expected)]);
    }
}

#[test]
fn a_loop_condition_calls_again_at_every_turn() {
    let source = format!(
        "{VEC}export fn f(n: i32): i32 {{\n  let p = V(0, 0);\n  let turns = 0;\n  while (step(p).x <= n) {{\n    p = step(p);\n    turns++;\n    if (turns > 100) break;\n  }}\n  for (let i = 0; step(V(i, 0)).x < 3; i++) {{\n    turns += 10;\n    if (i == 1) continue;\n  }}\n  return turns * 1000 + p.y;\n}}\n\
         fn step(v: V): V {{ return V(v.x + 1, v.y + 2); }}\n"
    );
    // n = 4: four turns, p = (4, 8); then two turns of the `for`.
    gives(&source, &[(vec![Value::I32(4)], Ok(Value::I32(24_008)))]);
}
