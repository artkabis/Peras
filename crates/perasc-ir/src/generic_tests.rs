//! Generics, run in the interpreter: one copy of a generic function per set
//! of type arguments, generic structures and enumerations, constraints on
//! numbers, and buffers moved through type parameters without a leak.

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
        let (value, heap) = run_with_heap(&module, FnId(0), args).unwrap_or_else(|t| {
            unreachable!(
                "{t:?}
{module}"
            )
        });
        assert_eq!(
            &value, expected,
            "{args:?}
{module}"
        );
        assert_eq!(
            heap.live,
            Vec::<u32>::new(),
            "leak on {args:?}
{module}"
        );
    }
}

#[test]
fn numbers_structures_and_enumerations_get_one_copy_per_type() {
    let source = A;
    gives(
        source,
        &[
            (vec![Value::I64(5)], Value::F64(28.0)),
            (vec![Value::I64(-3)], Value::F64(25.0)),
        ],
    );
    let module = compile(source);
    let names: Vec<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
    for name in [
        "larger__i32",
        "larger__i64",
        "larger__f64",
        "swap__i32_f64",
        "sum__f64",
    ] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }
    // `first` is never called: it has no copy.
    assert!(!names.iter().any(|n| n.starts_with("first")), "{names:?}");
}

#[test]
fn strings_and_arrays_move_through_type_parameters_without_a_leak() {
    gives(H, &[(vec![Value::I32(5)], Value::I32(8_524_235))]);
}

#[test]
fn generic_functions_call_each_other_and_recurse() {
    let source = r#"export fn f(n: i32): i32 {
  const xs: Array<i32> = [n, 2, 3];
  const words: Array<String> = ["a", "bcd"];
  return count(xs, 0) * 100 + longest(words) + total<i64>([1, 2]).toI32();
}

fn count<T>(xs: Array<T>, from: i32): i32 {
  if (from >= xs.length) {
    return 0;
  }
  return 1 + count(xs, from + 1);
}

fn longest(words: Array<String>): i32 {
  let best = 0;
  for (const w of words) {
    best = larger(best, w.length);
  }
  return best + count(words, 0) * 10;
}

fn larger<T: Ord>(a: T, b: T): T {
  if (a > b) {
    return a;
  }
  return b;
}

fn total<T: Numeric>(xs: Array<T>): T {
  let sum = xs[0];
  for (let i = 1; i < xs.length; i++) {
    sum += larger(xs[i], xs[i]);
  }
  return sum;
}
"#;
    gives(source, &[(vec![Value::I32(7)], Value::I32(326))]);
}

const A: &str = "export fn run(n: i64): f64 {\n  const a = larger(3, 4);\n  const b = larger(n, 7);\n  const c = larger(1.5, 2.0);\n  const p = swap(Pair(1, 2.5));\n  const xs: Array<i64> = [1, 2, n];\n  const big = biggest(xs, 0);\n  const t = Tree.Leaf(5);\n  const e: Tree<i32> = Tree.Empty;\n  const total = a + leafOr(t, 0) + leafOr(e, 1);\n  return c + p.first + mean([1.0, 2.0], 0.0) + (b + big).toF64() + total.toF64();\n}\n\nstruct Pair<A, B> {\n  first: A;\n  second: B;\n}\n\nenum Tree<T> {\n  Leaf(T),\n  Empty,\n}\n\nfn larger<T: Ord>(a: T, b: T): T {\n  if (a > b) {\n    return a;\n  }\n  return b;\n}\n\nfn swap<A: Copy, B: Copy>(p: Pair<A, B>): Pair<B, A> {\n  return Pair(p.second, p.first);\n}\n\nfn biggest<T: Ord>(xs: Array<T>, start: T): T {\n  let best = start;\n  for (const x of xs) {\n    best = larger(best, x);\n  }\n  return best;\n}\n\nfn sum<T: Numeric>(xs: Array<T>, zero: T): T {\n  let total = zero;\n  for (const x of xs) {\n    total += x;\n  }\n  return total;\n}\n\nfn mean<T: Numeric>(xs: Array<T>, zero: T): f64 {\n  return sum(xs, zero).toF64() / xs.length.toF64();\n}\n\nfn leafOr<T: Copy>(t: Tree<T>, fallback: T): T {\n  return match (t) {\n    Tree.Leaf(v) => v,\n    Tree.Empty => fallback,\n  };\n}\n\nfn first<T>(xs: inout Array<T>): Option<T> {\n  return xs.pop();\n}\n\n";

const H: &str = "export fn run(n: i32): i32 {\n  let words: Array<String> = [\"a\", \"bb\"];\n  pushTwice(&words, \"ccc\", \"dddd\");\n  const w = last(&words).unwrapOr(\"none\");\n  let lists: Array<Array<i32>> = [[1], [2, 3]];\n  const l = last(&lists).unwrapOr([]);\n  const named = label(\"count\", [n, n, n]);\n  const t = Tree.Pair(\"x\", \"yy\");\n  const u: Tree<Array<i32>> = Tree.Empty;\n  const s = orElse(wrap(\"hello\"), \"bye\");\n  const e: Option<String> = None;\n  const f = orElse(e, \"fallback\");\n  return w.length * 1000 + l.length * 100 + named.value.length * 10 + named.name.length + count(t) * 10000 + count(u) + s.length * 100000 + f.length * 1000000;\n}\n\nstruct Named<T> {\n  name: String;\n  value: T;\n}\n\nenum Tree<T> {\n  Leaf(T),\n  Pair(T, T),\n  Empty,\n}\n\nfn last<T>(xs: inout Array<T>): Option<T> {\n  return xs.pop();\n}\n\nfn pushTwice<T>(xs: inout Array<T>, a: sink T, b: sink T) {\n  xs.push(a);\n  xs.push(b);\n}\n\nfn label<T>(name: String, value: sink T): Named<T> {\n  return Named(name.copy(), value);\n}\n\nfn count<T>(t: Tree<T>): i32 {\n  return match (t) {\n    Tree.Leaf(_) => 1,\n    Tree.Pair(_, _) => 2,\n    Tree.Empty => 0,\n  };\n}\n\nfn wrap<T>(value: sink T): Option<T> {\n  return Some(value);\n}\n\nfn orElse<T>(o: sink Option<T>, fallback: sink T): T {\n  return o.unwrapOr(fallback);\n}\n\n";
