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

/// Only the accumulator recursion elimination, for tests about that pass.
fn tail_only(module: &Module) -> Module {
    let mut out = module.clone();
    for (index, function) in out.functions.iter_mut().enumerate() {
        let id = FnId(u32::try_from(index).unwrap_or(u32::MAX));
        crate::opt::eliminate_tail_recursion(function, id);
    }
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
    let (before, after) = same_results(source, &i32s(-3..=24));
    insta::assert_snapshot!(tail_only(&before).to_string(), @r"
    export fn fib(%0 n: i32) -> i32
      local %1 acc: i32
      %1 = 0:i32
      loop while forever
        if (i32.< %0 2:i32)
          return (i32.+ %1 %0)
        %1 = (i32.+ %1 (call #0 (i32.- %0 1:i32)))
        %0 = (i32.- %0 2:i32)
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
    // `a % b` reads `a` after `a` would get `b`: only `a` needs a temporary.
    assert!(text.contains("next0") && !text.contains("next1"), "{text}");
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
        assert_eq!(tail_only(&before), before, "{source}");
    }
}

#[test]
fn fib_is_unrolled_three_levels_with_one_real_call() {
    let source = "export fn fib(n: i32): i32 {\n  if (n < 2) return n;\n  return fib(n - 1) + fib(n - 2);\n}\n";
    let (_, after) = same_results(source, &i32s(-3..=27));
    let text = after.to_string();
    assert_eq!(text.matches("call #0").count(), 1, "{text}");
    assert_eq!(text.matches("block").count(), 3, "{text}");
    assert!(text.contains("acc_3") && !text.contains("acc_4"), "{text}");
    // Every loop was rotated: the exit test is the loop condition.
    assert_eq!(text.matches("loop while (bool.not").count(), 4, "{text}");
}

#[test]
fn unrolling_keeps_every_result() {
    let floats: Vec<Vec<Value>> = [-1.0, 0.0, 0.5, 3.0, 40.25, 300.0]
        .iter()
        .map(|&x| vec![Value::F64(x)])
        .collect();
    // Float addition: the copies keep the order of operations.
    same_results(
        "export fn f(x: f64): f64 {\n  if (x <= 0.0) return 0.0;\n  return x + f(x - 1.0);\n}\n",
        &floats,
    );
    // A return inside a loop leaves the copy from inside that loop.
    same_results(
        "export fn f(n: i32): i32 {\n  while (n > 10) { return f(n - 1); }\n  return n;\n}\n",
        &i32s(-5..=60),
    );
    // Two operators: the copies are made, the loop is not.
    same_results(
        "export fn f(n: i32): i32 {\n  if (n < 2) return 1;\n  if (n % 2 == 0) return 2 + f(n - 1);\n  return 3 * f(n - 1);\n}\n",
        &i32s(-2..=40),
    );
    // A variable the copy reads before writing, which must start at zero again.
    same_results(
        "export fn f(n: i32): i32 {\n  let s: i32 = 0;\n  let i: i32 = 0;\n  while (i < n) { s += i; i += 1; }\n  if (n < 1) return s;\n  return s + f(n - 1);\n}\n",
        &i32s(-2..=30),
    );
}

#[test]
fn calls_that_cannot_move_are_not_unrolled() {
    for source in [
        // Evaluating `g(n)` after the copy would change the order of calls.
        "export fn f(n: i32): i32 {\n  if (n < 1) return 0;\n  return f(n - 1) + g(n);\n}\nfn g(n: i32): i32 { return 10 / n; }\n",
        // A division in the other operand could trap in a different place.
        "export fn f(n: i32): i32 {\n  if (n < 1) return 0;\n  return f(n - 1) + 100 / n;\n}\n",
        // `&&` makes the call conditional.
        "export fn f(n: i32): bool {\n  if (n < 1) return true;\n  return n > 3 && f(n - 1);\n}\n",
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

/// `f(n)`: `if (n < 1) return x; x = x + 1; return x + f(n - 1)`, where `x` is
/// read before any assignment, as zero. Lowering never writes this; the passes
/// must still keep its result.
#[test]
fn variables_read_before_assignment_start_at_zero_again() {
    use crate::{BinOp, Const, Expr, ExprKind, Function, Local, LocalId, Stmt, Ty};
    let get = |l: u32| Expr {
        kind: ExprKind::Local(LocalId(l)),
        ty: Ty::I32,
    };
    let int = |v: i64| Expr {
        kind: ExprKind::Const(Const::Int(v)),
        ty: Ty::I32,
    };
    let bin = |op: BinOp, a: Expr, b: Expr, ty: Ty| Expr {
        kind: ExprKind::Binary(op, Box::new(a), Box::new(b)),
        ty,
    };
    let call = Expr {
        kind: ExprKind::Call(FnId(0), vec![bin(BinOp::Sub, get(0), int(1), Ty::I32)]),
        ty: Ty::I32,
    };
    let function = Function {
        name: "f".to_owned(),
        exported: true,
        param_count: 1,
        locals: vec![
            Local {
                name: "n".to_owned(),
                ty: Ty::I32,
            },
            Local {
                name: "x".to_owned(),
                ty: Ty::I32,
            },
        ],
        ret: Ty::I32,
        body: vec![
            Stmt::If {
                cond: bin(BinOp::Lt, get(0), int(1), Ty::Bool),
                then: vec![Stmt::Return(Some(get(1)))],
                otherwise: Vec::new(),
            },
            Stmt::Set(LocalId(1), bin(BinOp::Add, get(1), int(1), Ty::I32)),
            Stmt::Return(Some(bin(BinOp::Add, get(1), call, Ty::I32))),
        ],
    };
    // `if (n < 1) return x; x = x + 1; i = 0; while (i < 2) { y = y + f(n - 1); i = i + 1; }
    // return y + x`: the copy runs twice in the same call, and must find `x` and `y`
    // at zero both times.
    let mut twice = function.clone();
    for name in ["y", "i"] {
        twice.locals.push(Local {
            name: name.to_owned(),
            ty: Ty::I32,
        });
    }
    let call = twice.body.pop().and_then(|last| match last {
        Stmt::Return(Some(Expr {
            kind: ExprKind::Binary(_, _, call),
            ..
        })) => Some(*call),
        _ => None,
    });
    twice.body.extend([
        Stmt::Set(LocalId(3), int(0)),
        Stmt::Loop {
            cond: Some(bin(BinOp::Lt, get(3), int(2), Ty::Bool)),
            body: vec![
                Stmt::Set(
                    LocalId(2),
                    bin(BinOp::Add, get(2), call.unwrap_or_else(|| int(0)), Ty::I32),
                ),
                Stmt::Set(LocalId(3), bin(BinOp::Add, get(3), int(1), Ty::I32)),
            ],
            step: Vec::new(),
        },
        Stmt::Return(Some(bin(BinOp::Add, get(2), get(1), Ty::I32))),
    ]);
    for (function, inputs) in [(function, -2..=40), (twice, -2..=12)] {
        let before = Module {
            functions: vec![function],
        };
        assert_eq!(verify(&before), Ok(()));
        assert_ne!(optimized(&before), before, "nothing to check");
        for after in [tail_only(&before), optimized(&before)] {
            for n in inputs.clone() {
                let args = [Value::I32(n)];
                assert_eq!(
                    run(&after, FnId(0), &args),
                    run(&before, FnId(0), &args),
                    "{after}"
                );
            }
        }
    }
}

#[test]
fn integer_sign_flips_become_branchless_abs() {
    let edges = [i32::MIN, i32::MIN + 1, -7, -1, 0, 1, 7, i32::MAX];
    for cond in ["x < 0", "x <= 0", "0 > x", "0 >= x"] {
        let source = format!(
            "export fn f(x: i32): i32 {{\n  let y = x;\n  if ({}) y = -y;\n  return y;\n}}\n",
            cond.replace('x', "y")
        );
        let inputs: Vec<Vec<Value>> = edges.iter().map(|&n| vec![Value::I32(n)]).collect();
        let (_, after) = same_results(&source, &inputs);
        let text = after.to_string();
        assert!(
            text.contains("(i32.abs %1)") && !text.contains("if"),
            "{text}"
        );
    }
    let wide = "export fn f(x: i64): i64 {\n  let y = x;\n  if (y < 0) y = -y;\n  return y;\n}\n";
    let inputs: Vec<Vec<Value>> = [i64::MIN, -3, 0, 5, i64::MAX]
        .iter()
        .map(|&n| vec![Value::I64(n)])
        .collect();
    let (_, after) = same_results(wide, &inputs);
    assert!(after.to_string().contains("(i64.abs %1)"), "{after}");
}

#[test]
fn other_sign_tests_keep_their_branch() {
    for source in [
        // `-0.0` and NaN: `abs` would change them.
        "export fn f(x: f64): f64 {\n  let y = x;\n  if (y < 0.0) y = -y;\n  return y;\n}\n",
        // Another variable, another comparison, an `else`, a second statement.
        "export fn f(x: i32, z: i32): i32 {\n  let y = x;\n  if (z < 0) y = -y;\n  return y;\n}\n",
        "export fn f(x: i32): i32 {\n  let y = x;\n  if (y < 1) y = -y;\n  return y;\n}\n",
        "export fn f(x: i32): i32 {\n  let y = x;\n  if (y < 0) y = -y; else y = 1;\n  return y;\n}\n",
        "export fn f(x: i32): i32 {\n  let y = x;\n  if (y < 0) { y = -y; y = y + 1; }\n  return y;\n}\n",
    ] {
        let before = compile(source);
        assert_eq!(optimized(&before), before, "{source}");
    }
}
