//! Arrays, lowered to buffers and run in the interpreter: results, bounds
//! checks, and above all releases. After every run, on every path (`return`
//! in a loop, `break` after a move, a move on one side of an `if` or of `&&`),
//! no buffer may be left alive.

use crate::interp::{Trap, Value, run_with_heap};
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

/// Function 0 on each input: the expected result, and nothing left alive.
fn gives(source: &str, cases: &[(Vec<Value>, Result<Value, Trap>)]) -> Module {
    let module = compile(source);
    for (args, expected) in cases {
        let outcome = run_with_heap(&module, FnId(0), args);
        match (outcome, expected) {
            (Ok((value, heap)), Ok(want)) => {
                assert_eq!(&value, want, "{args:?}\n{module}");
                assert_eq!(heap.live, Vec::<u32>::new(), "leak on {args:?}\n{module}");
            }
            (outcome, want) => assert_eq!(outcome.map(|(v, _)| v), *want, "{args:?}\n{module}"),
        }
    }
    module
}

fn i(v: i32) -> Value {
    Value::I32(v)
}

#[test]
fn push_index_length_and_for_of() {
    let source = "export fn f(n: i32): i32 {\n  let xs: Array<i32> = [];\n  for (let i = 0; i < n; i++) {\n    xs.push(i * i);\n  }\n  let total = 0;\n  for (const x of xs) {\n    total += x;\n  }\n  return total * 1000 + xs.length + xs[n - 1];\n}\n";
    gives(
        source,
        &[
            (vec![i(1)], Ok(i(1))),
            (vec![i(4)], Ok(i(14_013))),
            (vec![i(100)], Ok(i(328_359_901))),
        ],
    );
}

#[test]
fn every_index_is_checked() {
    let source =
        "export fn f(n: i32): i32 {\n  let xs = [10, 20, 30];\n  xs[n] = 5;\n  return xs[n];\n}\n";
    gives(
        source,
        &[
            (vec![i(0)], Ok(i(5))),
            (vec![i(2)], Ok(i(5))),
            (vec![i(3)], Err(Trap::Bounds)),
            (vec![i(-1)], Err(Trap::Bounds)),
        ],
    );
}

#[test]
fn pop_gives_an_option() {
    let source = "export fn f(n: i32): i32 {\n  let xs = [1, 2, 3];\n  let total = 0;\n  for (let i = 0; i < n; i++) {\n    total = total * 10 + xs.pop().unwrapOr(9);\n  }\n  return total * 10 + xs.length;\n}\n";
    gives(
        source,
        &[(vec![i(2)], Ok(i(321))), (vec![i(5)], Ok(i(321_990)))],
    );
}

#[test]
fn copies_are_independent_and_assignment_moves() {
    let source = "export fn f(): i32 {\n  let a = [1, 2];\n  let b = a.copy();\n  b.push(3);\n  let c = b;\n  c[0] = 7;\n  return a.length * 100 + c.length * 10 + c[0] - a[0];\n}\n";
    gives(source, &[(vec![], Ok(i(236)))]);
}

#[test]
fn reassignment_releases_the_old_buffer() {
    // Each turn makes a new array and drops the previous one.
    let source = "export fn f(n: i32): i32 {\n  let xs: Array<i32> = [];\n  for (let i = 0; i < n; i++) {\n    xs = [i, i + 1];\n    xs = grow(xs);\n  }\n  return xs.length;\n}\n\
         fn grow(xs: sink Array<i32>): Array<i32> {\n  let ys = xs;\n  ys.push(0);\n  return ys;\n}\n";
    gives(source, &[(vec![i(0)], Ok(i(0))), (vec![i(50)], Ok(i(3)))]);
}

#[test]
fn moves_on_one_path_are_released_on_the_others() {
    let source = "export fn f(n: i32): i32 {\n  let a = [1, 2, 3];\n  let b = [4];\n  let total = 0;\n  if (n > 5) {\n    total = take(a);\n  } else if (n > 2) {\n    total = take(b);\n  }\n  if (n == 1 && take([7, 7]) == 2) {\n    total += 100;\n  }\n  return total;\n}\n\
         fn take(xs: sink Array<i32>): i32 {\n  return xs.length;\n}\n";
    gives(
        source,
        &[
            (vec![i(9)], Ok(i(3))),
            (vec![i(3)], Ok(i(1))),
            (vec![i(1)], Ok(i(100))),
            (vec![i(0)], Ok(i(0))),
        ],
    );
}

#[test]
fn return_break_and_continue_release_what_they_leave() {
    let source = "export fn f(n: i32): i32 {\n  let outer = [1];\n  let total = 0;\n  for (let i = 0; i < 10; i++) {\n    let inner = [i, i];\n    if (i == n) {\n      return total + take(outer) + inner.length;\n    }\n    if (i % 3 == 0) {\n      continue;\n    }\n    if (i == 7) {\n      total += take(outer);\n      break;\n    }\n    total += inner[0];\n  }\n  return total;\n}\n\
         fn take(xs: sink Array<i32>): i32 {\n  return xs.length;\n}\n";
    // n = 4: turns 1 and 2 add 1 + 2, then 1 + 2 at return.
    gives(
        source,
        &[
            (vec![i(4)], Ok(i(6))),
            (vec![i(0)], Ok(i(3))),
            // No return: 1 + 2 + 4 + 5, then turn 7 moves `outer` and breaks.
            (vec![i(99)], Ok(i(13))),
        ],
    );
}

#[test]
fn structures_hold_arrays_and_pass_them_every_way() {
    let source = "export fn f(n: i32): f64 {\n  let b = Buffer([], 1.5);\n  for (let i = 0; i < n; i++) {\n    b.samples.push(i.toF64());\n  }\n  gain(&b.samples, b.factor);\n  const peak = largest(b);\n  return peak + consume(b).toF64();\n}\n\
         struct Buffer {\n  samples: Array<f64>;\n  factor: f64;\n}\n\
         fn gain(xs: inout Array<f64>, k: f64): void {\n  for (let i = 0; i < xs.length; i++) {\n    xs[i] = xs[i] * k;\n  }\n}\n\
         fn largest(b: Buffer): f64 {\n  let best = 0.0;\n  for (const s of b.samples) {\n    if (s > best) {\n      best = s;\n    }\n  }\n  return best;\n}\n\
         fn consume(b: sink Buffer): i32 {\n  return b.samples.length;\n}\n";
    gives(
        source,
        &[
            (vec![i(4)], Ok(Value::F64(8.5))),
            (vec![i(0)], Ok(Value::F64(0.0))),
        ],
    );
}

#[test]
fn functions_return_arrays_and_structures_holding_them() {
    let source = "export fn f(n: i32): i32 {\n  const r = make(n);\n  return r.items.length * 10 + count(r.items) + evens(n).length;\n}\n\
         struct Result {\n  items: Array<i32>;\n  ok: bool;\n}\n\
         fn make(n: i32): Result {\n  let items: Array<i32> = [];\n  for (let i = 0; i < n; i++) {\n    items.push(i);\n  }\n  return Result(items, true);\n}\n\
         fn count(xs: Array<i32>): i32 {\n  return xs.length;\n}\n\
         fn evens(n: i32): Array<i32> {\n  let found: Array<i32> = [];\n  for (let i = 0; i < n; i += 2) {\n    found.push(i);\n  }\n  return found;\n}\n";
    gives(source, &[(vec![i(5)], Ok(i(58))), (vec![i(0)], Ok(i(0)))]);
}

#[test]
fn every_element_type() {
    let source = "export fn f(): f64 {\n  let flags = [true, false, true];\n  let wide: Array<i64> = [9000000000, 1];\n  let small: Array<f32> = [0.5, 0.25];\n  let n = 0;\n  for (const b of flags) {\n    if (b) {\n      n++;\n    }\n  }\n  return n.toF64() + (wide[0] / 1000000000).toF64() + (small[0] + small[1]).toF64();\n}\n";
    gives(source, &[(vec![], Ok(Value::F64(11.75)))]);
}

#[test]
fn inout_numbers_come_back_to_the_caller() {
    let source = "export fn f(n: i32): i32 {\n  let c = Counter(0, 0.5);\n  for (let i = 0; i < n; i++) {\n    bump(&c);\n  }\n  let a = 1;\n  let b = 2;\n  swap(&a, &b);\n  return c.value * 100 + (c.ratio * 10.0).toI32() + a * 10 + b;\n}\n\
         struct Counter {\n  value: i32;\n  ratio: f64;\n}\n\
         fn bump(c: inout Counter): void {\n  c.value += 1;\n  c.ratio *= 2.0;\n}\n\
         fn swap(a: inout i32, b: inout i32): void {\n  const t = a;\n  a = b;\n  b = t;\n}\n";
    // n = 3: value 3, ratio 4.0.
    gives(source, &[(vec![i(3)], Ok(i(361))), (vec![i(0)], Ok(i(26)))]);
}

#[test]
fn the_question_mark_releases_what_the_function_holds() {
    // `?` leaves early, with `xs` and the temporary of `make()` alive.
    let source = "export fn f(n: i32): i32 {\n  return g(n).unwrapOr(-1);\n}\n\
         fn g(n: i32): Option<i32> {\n  let xs = [1, 2, 3];\n  let count = 0;\n  let seen = 0;\n  for (let i = 0; i < n; i++) {\n    count += make(i).length + check(i, &seen)?;\n  }\n  return Some(count + seen + xs.length);\n}\n\
         fn make(n: i32): Array<i32> {\n  let out: Array<i32> = [];\n  for (let i = 0; i < n; i++) {\n    out.push(i);\n  }\n  return out;\n}\n\
         fn check(i: i32, seen: inout i32): Option<i32> {\n  seen += 100;\n  if (i == 4) {\n    return None;\n  }\n  return Some(0);\n}\n";
    gives(source, &[(vec![i(3)], Ok(i(306))), (vec![i(9)], Ok(i(-1)))]);
}
