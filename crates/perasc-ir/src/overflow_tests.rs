//! Integer overflow in a debug build (decision 0011, step 5): `+`, `-` and
//! `*` stop the program when the result does not fit; `wrappingAdd` and the
//! others still wrap; a loop counter under `i < bound` is not checked.

use crate::interp::{Trap, Value, run_with_heap};
use crate::{BufStmt, FnId, Module, Stmt, lower_with, verify};

const SOURCE: &str = "
export fn add(a: i32, b: i32): i32 {
  return a + b;
}

export fn wrapped(a: i32, b: i32): i32 {
  return a.wrappingAdd(b).wrappingMul(2).wrappingSub(b);
}

export fn big(a: i64): i64 {
  return a * 3 - 1;
}

export fn total(n: i32): i32 {
  let s = 0;
  for (let i = 0; i < n; i++) {
    s += i;
  }
  return s;
}

export fn cells(n: i32): i32 {
  let xs: Array<i32> = [2147483600, 0];
  for (let i = 0; i < n; i++) {
    xs[0] += 1;
  }
  return xs[0];
}
";

fn module(checks: bool) -> Module {
    let parsed = perasc_syntax::parse(SOURCE);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, SOURCE);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module =
        lower_with(&parsed.ast, &analysis, checks).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    module
}

fn run(module: &Module, name: &str, args: &[Value]) -> Result<Value, Trap> {
    let index = module.functions.iter().position(|f| f.name == name);
    let id = FnId(
        index
            .and_then(|i| u32::try_from(i).ok())
            .unwrap_or(u32::MAX),
    );
    run_with_heap(module, id, args).map(|(value, heap)| {
        assert_eq!(heap.live, Vec::<u32>::new(), "{name} leaks");
        value
    })
}

#[test]
fn a_debug_build_stops_on_an_overflow_and_production_wraps() {
    let debug = module(true);
    let production = module(false);
    let max = Value::I32(i32::MAX);
    assert_eq!(
        run(&debug, "add", &[max, Value::I32(1)]),
        Err(Trap::Overflow)
    );
    assert_eq!(
        run(&production, "add", &[max, Value::I32(1)]),
        Ok(Value::I32(i32::MIN))
    );
    assert_eq!(
        run(&debug, "add", &[Value::I32(2), Value::I32(3)]),
        Ok(Value::I32(5))
    );
    // Wrapping on purpose wraps in both.
    for m in [&debug, &production] {
        assert_eq!(
            run(m, "wrapped", &[max, Value::I32(1)]),
            Ok(Value::I32(i32::MIN.wrapping_mul(2).wrapping_sub(1)))
        );
    }
    assert_eq!(
        run(&debug, "big", &[Value::I64(i64::MAX / 2)]),
        Err(Trap::Overflow)
    );
    assert_eq!(run(&debug, "big", &[Value::I64(-5)]), Ok(Value::I64(-16)));
    // `s += i` overflows past 65,536 turns; the counter never does.
    assert_eq!(
        run(&debug, "total", &[Value::I32(1000)]),
        Ok(Value::I32(499_500))
    );
    assert_eq!(
        run(&debug, "total", &[Value::I32(100_000)]),
        Err(Trap::Overflow)
    );
    assert_eq!(
        run(&debug, "cells", &[Value::I32(47)]),
        Ok(Value::I32(i32::MAX))
    );
    assert_eq!(run(&debug, "cells", &[Value::I32(48)]), Err(Trap::Overflow));
}

/// Whether a check is among `stmts`, at any depth.
fn checks(stmts: &[Stmt]) -> usize {
    stmts
        .iter()
        .map(|s| match s {
            Stmt::Buf(BufStmt::CheckOverflow, _) => 1,
            Stmt::If {
                then, otherwise, ..
            } => checks(then) + checks(otherwise),
            Stmt::Loop { body, step, .. } => checks(body) + checks(step),
            Stmt::Block(body) => checks(body),
            _ => 0,
        })
        .sum()
}

/// The steps of the loops among `stmts`.
fn steps(stmts: &[Stmt], out: &mut Vec<Vec<Stmt>>) {
    for s in stmts {
        match s {
            Stmt::Loop { body, step, .. } => {
                out.push(step.clone());
                steps(body, out);
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                steps(then, out);
                steps(otherwise, out);
            }
            Stmt::Block(body) => steps(body, out),
            _ => {}
        }
    }
}

#[test]
fn a_counted_loop_step_is_not_checked() {
    let debug = module(true);
    let total = debug.functions.iter().find(|f| f.name == "total");
    let body = total.map(|f| f.body.clone()).unwrap_or_default();
    let mut found = Vec::new();
    steps(&body, &mut found);
    assert_eq!(found.len(), 1, "{debug}");
    assert_eq!(
        found.iter().map(|s| checks(s)).sum::<usize>(),
        0,
        "the step `i++` is checked:\n{debug}"
    );
    // `s += i` is.
    assert!(checks(&body) >= 1, "{debug}");
    // Nothing is checked in production.
    let production = module(false);
    assert!(production.functions.iter().all(|f| checks(&f.body) == 0));
}
