//! Enumerations and `match`, run in the interpreter: tags, data, bindings,
//! literals, options, arms with blocks, and buffers inside variants, which
//! must never leak whatever the variant.

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

fn gives(source: &str, cases: &[(Vec<Value>, Value)]) {
    let module = compile(source);
    for (args, expected) in cases {
        let (value, heap) = run_with_heap(&module, FnId(0), args)
            .unwrap_or_else(|t| unreachable!("{t:?}\n{module}"));
        assert_eq!(&value, expected, "{args:?}\n{module}");
        assert_eq!(heap.live, Vec::<u32>::new(), "leak on {args:?}\n{module}");
    }
}

const SHAPE: &str = "enum Shape {\n  Circle(f64),\n  Rect(f64, f64),\n  Empty,\n}\n";

#[test]
fn variants_carry_their_data_and_match_takes_them_apart() {
    let source = format!(
        "export fn f(n: i32): f64 {{\n  return area(make(n));\n}}\n{SHAPE}\
         fn make(n: i32): Shape {{\n  if (n == 0) {{\n    return Shape.Circle(1.0);\n  }}\n  if (n == 1) {{\n    return Shape.Rect(2.0, 3.0);\n  }}\n  return Shape.Empty;\n}}\n\
         fn area(s: Shape): f64 {{\n  return match (s) {{\n    Circle(r) => 3.0 * r * r,\n    Shape.Rect(w, h) => w * h,\n    Empty => -1.0,\n  }};\n}}\n"
    );
    gives(
        &source,
        &[
            (vec![Value::I32(0)], Value::F64(3.0)),
            (vec![Value::I32(1)], Value::F64(6.0)),
            (vec![Value::I32(2)], Value::F64(-1.0)),
        ],
    );
}

#[test]
fn literals_wildcards_names_and_blocks() {
    let source = "export fn f(n: i32): i32 {\n  let total = 0;\n  match (n) {\n    0 => {\n      total = 100;\n    }\n    -1 => {\n      return -100;\n    }\n    other => {\n      total = other * 2;\n    }\n  }\n  const flag = match (n > 5) {\n    true => 1,\n    false => 0,\n  };\n  return total + flag;\n}\n";
    gives(
        source,
        &[
            (vec![Value::I32(0)], Value::I32(100)),
            (vec![Value::I32(-1)], Value::I32(-100)),
            (vec![Value::I32(7)], Value::I32(15)),
        ],
    );
}

#[test]
fn options_match_and_nested_patterns_test_their_parts() {
    let source = "export fn f(n: i32): i32 {\n  const o = half(n);\n  const a = match (o) {\n    Some(0) => 1000,\n    Some(x) => x,\n    None => -1,\n  };\n  return a;\n}\n\
         fn half(n: i32): Option<i32> {\n  if (n % 2 != 0) {\n    return None;\n  }\n  return Some(n / 2);\n}\n";
    gives(
        source,
        &[
            (vec![Value::I32(0)], Value::I32(1000)),
            (vec![Value::I32(8)], Value::I32(4)),
            (vec![Value::I32(3)], Value::I32(-1)),
        ],
    );
}

#[test]
fn variants_holding_strings_and_arrays_never_leak() {
    let source = "export fn f(n: i32): i32 {\n  let total = 0;\n  for (let i = 0; i < n; i++) {\n    const e = make(i);\n    total += match (e) {\n      Text(s) => s.length,\n      Numbers(xs) => xs.length * 10,\n      Nothing => 100,\n    };\n  }\n  let names: Array<String> = [\"a\", \"bb\"];\n  const last = names.pop().unwrapOr(\"none\");\n  const gone = names.copy().pop();\n  const none: Option<String> = None;\n  return total * 1000 + last.length * 10 + none.unwrapOr(\"xyz\").length + match (gone) {\n    Some(s) => s.length * 100,\n    None => 0,\n  };\n}\n\
         enum Event {\n  Text(String),\n  Numbers(Array<i32>),\n  Nothing,\n}\n\
         fn make(i: i32): Event {\n  if (i % 3 == 0) {\n    return Event.Text(\"hello\");\n  }\n  if (i % 3 == 1) {\n    return Event.Numbers([1, 2, i]);\n  }\n  return Event.Nothing;\n}\n";
    // n = 3: 5 + 30 + 100 = 135; last "bb"; "xyz"; the copy pops "a".
    gives(source, &[(vec![Value::I32(3)], Value::I32(135_123))]);
}

#[test]
fn results_carry_a_value_or_an_error_and_the_question_mark_returns_it() {
    let source = "export fn f(n: i32): i32 {\n  return match (run(n)) {\n    Ok(v) => v,\n    Err(Negative) => -1,\n    Err(Odd(k)) => -100 - k,\n  };\n}\n\
         enum Problem {\n  Negative,\n  Odd(i32),\n}\n\
         fn check(n: i32): Result<i32, Problem> {\n  if (n < 0) {\n    return Err(Problem.Negative);\n  }\n  if (n % 2 != 0) {\n    return Err(Problem.Odd(n));\n  }\n  return Ok(n / 2);\n}\n\
         fn run(n: i32): Result<i32, Problem> {\n  let names: Array<String> = [\"kept\"];\n  const half = check(n)?;\n  const quarter = check(half)?;\n  names.push(\"x\");\n  return Ok(quarter + names.length);\n}\n";
    gives(
        source,
        &[
            (vec![Value::I32(8)], Value::I32(4)),
            (vec![Value::I32(-4)], Value::I32(-1)),
            (vec![Value::I32(3)], Value::I32(-103)),
            (vec![Value::I32(6)], Value::I32(-103)),
        ],
    );
}

#[test]
fn errors_holding_strings_move_out_through_the_question_mark() {
    let source = "export fn f(n: i32): i32 {\n  const r = outer(n);\n  if (r.isOk()) {\n    return r.unwrapOr(0);\n  }\n  return match (r) {\n    Ok(_) => 0,\n    Err(message) => -message.length,\n  };\n}\n\
         fn inner(n: i32): Result<i32, String> {\n  if (n > 10) {\n    return Err(\"too large: \" + \"n\");\n  }\n  return Ok(n);\n}\n\
         fn outer(n: i32): Result<i32, String> {\n  const v = inner(n)?;\n  return Ok(v * 2);\n}\n";
    gives(
        source,
        &[
            (vec![Value::I32(4)], Value::I32(8)),
            (vec![Value::I32(11)], Value::I32(-12)),
        ],
    );
}
