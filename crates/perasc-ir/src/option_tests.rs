//! Options, lowered to a `bool` and their value's scalars, run in the
//! interpreter unoptimized and optimized: `Some`, `None`, `?`, `unwrapOr`, and
//! checked arithmetic against Rust's own.

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

/// Function 0 of the module, unoptimized and optimized: both must agree with
/// `expected` on each input.
fn agrees(source: &str, inputs: &[Vec<Value>], expected: impl Fn(&[Value]) -> Result<Value, Trap>) {
    let module = compile(source);
    let mut optimized = module.clone();
    optimize(&mut optimized);
    assert_eq!(verify(&optimized), Ok(()), "{optimized}");
    for args in inputs {
        for m in [&module, &optimized] {
            assert_eq!(run(m, FnId(0), args), expected(args), "{args:?}\n{m}");
        }
    }
}

const NONE: i64 = -12_345;

#[test]
fn some_none_and_the_question_mark() {
    let source = "export fn f(n: i32): i32 {\n  return quarter(n).unwrapOr(-1);\n}\n\
         fn half(n: i32): Option<i32> {\n  if (n % 2 != 0) {\n    return None;\n  }\n  return Some(n / 2);\n}\n\
         fn quarter(n: i32): Option<i32> {\n  const h = half(n)?;\n  return half(h);\n}\n";
    let inputs: Vec<Vec<Value>> = (-9..=9).map(|n| vec![Value::I32(n)]).collect();
    agrees(source, &inputs, |args| {
        let Value::I32(n) = args[0] else {
            unreachable!()
        };
        Ok(Value::I32(if n % 4 == 0 { n / 4 } else { -1 }))
    });
}

#[test]
fn an_option_of_a_structure_is_a_bool_and_its_fields() {
    let source = "export fn f(flag: bool, n: i32): i32 {\n  let found: Option<P> = None;\n  if (flag) {\n    found = Some(P(n, n + 1));\n  }\n  const p = found.unwrapOr(P(-1, -2));\n  if (found.isNone()) {\n    return p.x * 10 + p.y;\n  }\n  return p.x * 100 + p.y;\n}\n\
         struct P {\n  x: i32;\n  y: i32;\n}\n";
    let inputs = [
        vec![Value::Bool(true), Value::I32(3)],
        vec![Value::Bool(false), Value::I32(3)],
    ];
    agrees(source, &inputs, |args| {
        Ok(Value::I32(if args[0] == Value::Bool(true) {
            304
        } else {
            -12
        }))
    });
}

#[test]
fn unwrap_or_evaluates_the_receiver_then_the_fallback() {
    // `g(0)` traps on arithmetic, `forever(_)` on depth: whichever comes
    // first shows the order. The fallback is evaluated even when it is not used.
    let source = "export fn f(a: i32, b: i32): i32 {\n  return g(a).unwrapOr(forever(b));\n}\n\
         fn g(a: i32): Option<i32> {\n  return Some(10 / a);\n}\n\
         fn forever(b: i32): i32 {\n  return forever(b + 1) + 1;\n}\n";
    // The optimizer turns `forever` into a loop: it then runs out of fuel.
    let module = compile(source);
    let mut optimized = module.clone();
    optimize(&mut optimized);
    for m in [&module, &optimized] {
        let first = run(m, FnId(0), &[Value::I32(0), Value::I32(1)]);
        assert_eq!(first, Err(Trap::Arithmetic), "{m}");
        let second = run(m, FnId(0), &[Value::I32(1), Value::I32(1)]);
        assert!(
            matches!(second, Err(Trap::Depth | Trap::Fuel)),
            "{second:?}\n{m}"
        );
    }
}

/// Values where overflow starts and stops.
fn edges64() -> Vec<i64> {
    let mut v = vec![
        0,
        1,
        -1,
        2,
        -2,
        3,
        7,
        -7,
        1 << 31,
        -(1 << 31),
        1 << 32,
        3_037_000_499,
        -3_037_000_499,
        3_037_000_500,
    ];
    v.extend([
        i64::MAX,
        i64::MAX - 1,
        i64::MIN,
        i64::MIN + 1,
        i64::MAX / 2,
        i64::MIN / 2,
    ]);
    v
}

fn edges32() -> Vec<i32> {
    vec![
        0,
        1,
        -1,
        2,
        -2,
        46_340,
        46_341,
        -46_341,
        65_536,
        i32::MAX,
        i32::MAX - 1,
        i32::MIN,
        i32::MIN + 1,
        i32::MAX / 2,
        i32::MIN / 2,
    ]
}

#[test]
fn checked_arithmetic_matches_rust_on_i64() {
    for (method, op) in [
        (
            "checkedAdd",
            i64::checked_add as fn(i64, i64) -> Option<i64>,
        ),
        ("checkedSub", i64::checked_sub),
        ("checkedMul", i64::checked_mul),
    ] {
        let source = format!(
            "export fn f(a: i64, b: i64): i64 {{\n  return a.{method}(b).unwrapOr({NONE});\n}}\n"
        );
        let mut inputs = Vec::new();
        for a in edges64() {
            for b in edges64() {
                inputs.push(vec![Value::I64(a), Value::I64(b)]);
            }
        }
        agrees(&source, &inputs, |args| {
            let (Value::I64(a), Value::I64(b)) = (args[0], args[1]) else {
                unreachable!()
            };
            Ok(Value::I64(op(a, b).unwrap_or(NONE)))
        });
    }
}

#[test]
fn checked_arithmetic_matches_rust_on_i32() {
    let none = i32::try_from(NONE).unwrap_or(0);
    for (method, op) in [
        (
            "checkedAdd",
            i32::checked_add as fn(i32, i32) -> Option<i32>,
        ),
        ("checkedSub", i32::checked_sub),
        ("checkedMul", i32::checked_mul),
    ] {
        let source = format!(
            "export fn f(a: i32, b: i32): i32 {{\n  return a.{method}(b).unwrapOr({none});\n}}\n"
        );
        let mut inputs = Vec::new();
        for a in edges32() {
            for b in edges32() {
                inputs.push(vec![Value::I32(a), Value::I32(b)]);
            }
        }
        agrees(&source, &inputs, |args| {
            let (Value::I32(a), Value::I32(b)) = (args[0], args[1]) else {
                unreachable!()
            };
            Ok(Value::I32(op(a, b).unwrap_or(none)))
        });
    }
}

#[test]
fn the_question_mark_needs_a_function_returning_an_option() {
    let source = "export fn f(n: i32): i32 {\n  return half(n)? + 1;\n}\nfn half(n: i32): Option<i32> {\n  return Some(n / 2);\n}\n";
    let parsed = perasc_syntax::parse(source);
    let analysis = perasc_check::analyze(&parsed.ast, source);
    let lowered = lower(&parsed.ast, &analysis);
    assert!(
        lowered.is_err() || !analysis.diagnostics.is_empty(),
        "accepted: {lowered:?}"
    );
}
