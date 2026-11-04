//! The optimizer, checked against the unoptimized program on many inputs.

use crate::interp::{Trap, Value, run};
use crate::{FnId, Module, lower, optimize, verify};

fn compile(source: &str) -> Module {
    let parsed = perasc_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, source);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(verify(&module), Ok(()));
    module
}

fn optimized(module: &Module) -> Module {
    let mut out = module.clone();
    optimize(&mut out);
    assert_eq!(verify(&out), Ok(()), "{out}");
    out
}

/// Runs both versions on every input: same result, or same trap. The optimized
/// version may succeed where the original runs out of stack (`Depth`).
fn same_results(source: &str, inputs: &[Vec<Value>]) -> (Module, Module) {
    let before = compile(source);
    let after = optimized(&before);
    for args in inputs {
        let expected = run(&before, FnId(0), args);
        let actual = run(&after, FnId(0), args);
        if expected == Err(Trap::Depth) {
            assert_ne!(actual, Err(Trap::Fuel), "{args:?}");
            continue;
        }
        assert_eq!(actual, expected, "{args:?}\n{after}");
    }
    (before, after)
}

fn i32s(range: std::ops::RangeInclusive<i32>) -> Vec<Vec<Value>> {
    range.map(|n| vec![Value::I32(n)]).collect()
}

#[test]
fn fib_keeps_one_call_and_loops_on_the_other() {
    let source = "export fn fib(n: i32): i32 {\n  if (n < 2) return n;\n  return fib(n - 1) + fib(n - 2);\n}\n";
    let (_, after) = same_results(source, &i32s(-3..=24));
    insta::assert_snapshot!(after.to_string(), @r"
    export fn fib(%0 n: i32) -> i32
      local %1 acc: i32
      local %2 next0: i32
      %1 = 0:i32
      loop while forever
        if (i32.< %0 2:i32)
          return (i32.+ %1 %0)
        %1 = (i32.+ %1 (call #0 (i32.- %0 1:i32)))
        %2 = (i32.- %0 2:i32)
        %0 = %2
        continue
    ");
    assert_eq!(
        run(&after, FnId(0), &[Value::I32(24)]),
        Ok(Value::I32(46_368))
    );
}

#[test]
fn accumulating_sums_and_products_become_loops() {
    let sum =
        "export fn sum(n: i64): i64 {\n  if (n <= 0) return 0;\n  return n + sum(n - 1);\n}\n";
    let inputs: Vec<Vec<Value>> = [0, 1, 2, 10, 1000, 4000, 1_000_000]
        .iter()
        .map(|&n| vec![Value::I64(n)])
        .collect();
    let (before, after) = same_results(sum, &inputs);
    // The original runs out of stack on a million; the loop does not.
    assert_eq!(
        run(&before, FnId(0), &[Value::I64(1_000_000)]),
        Err(Trap::Depth)
    );
    assert_eq!(
        run(&after, FnId(0), &[Value::I64(1_000_000)]),
        Ok(Value::I64(500_000_500_000))
    );

    let fact =
        "export fn fact(n: i32): i32 {\n  if (n <= 1) return 1;\n  return n * fact(n - 1);\n}\n";
    same_results(fact, &i32s(-2..=40));

    // The call on the left, with a simple right operand.
    let count =
        "export fn count(n: i32): i32 {\n  if (n == 0) return 0;\n  return count(n - 1) + 1;\n}\n";
    let mut inputs = i32s(0..=50);
    inputs.extend([500, 3000].map(|n| vec![Value::I32(n)]));
    let (_, after) = same_results(count, &inputs);
    assert!(after.to_string().contains("continue"), "{after}");
}

#[test]
fn tail_calls_swap_their_arguments_safely() {
    let gcd = "export fn gcd(a: i32, b: i32): i32 {\n  if (b == 0) return a;\n  return gcd(b, a % b);\n}\n";
    let mut inputs = Vec::new();
    for a in [0, 1, 12, 18, 35, 1071, -48, i32::MAX, i32::MIN] {
        for b in [0, 1, 6, 462, -18, 7, i32::MAX] {
            inputs.push(vec![Value::I32(a), Value::I32(b)]);
        }
    }
    let (_, after) = same_results(gcd, &inputs);
    let text = after.to_string();
    assert!(
        !text.contains("acc"),
        "a plain tail call needs no accumulator:\n{text}"
    );
}

#[test]
fn what_could_change_a_result_is_left_alone() {
    for source in [
        // Float addition is not associative.
        "export fn f(x: f64): f64 {\n  if (x <= 0.0) return 0.0;\n  return x + f(x - 1.0);\n}\n",
        // A `continue` inside a loop would target that loop.
        "export fn f(n: i32): i32 {\n  while (n > 10) { return f(n - 1); }\n  return n;\n}\n",
        // Two different accumulating operators.
        "export fn f(n: i32): i32 {\n  if (n < 2) return 1;\n  if (n % 2 == 0) return 2 + f(n - 1);\n  return 3 * f(n - 1);\n}\n",
        // Evaluating `g(n)` before the call would change the order of calls.
        "export fn f(n: i32): i32 {\n  if (n < 1) return 0;\n  return f(n - 1) + g(n);\n}\nfn g(n: i32): i32 { return 10 / n; }\n",
        // A division in the right operand could trap earlier.
        "export fn f(n: i32): i32 {\n  if (n < 1) return 0;\n  return f(n - 1) + 100 / n;\n}\n",
    ] {
        let before = compile(source);
        assert_eq!(optimized(&before), before, "{source}");
    }
}

#[test]
fn the_verifier_rejects_ill_typed_ir() {
    let mut module = compile("export fn f(x: i32): i32 { return x; }");
    if let Some(function) = module.functions.first_mut()
        && let Some(local) = function.locals.first_mut()
    {
        local.ty = crate::Ty::F64;
    }
    let problems = verify(&module).err().unwrap_or_default();
    assert!(
        problems.iter().any(|p| p.contains("%0 read as i32")),
        "{problems:?}"
    );
}
