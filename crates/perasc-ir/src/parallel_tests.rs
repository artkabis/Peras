//! `parallel for`: the body moved into a job, called over every turn, gives
//! what the same loop written `for` gives, bit for bit; a loop whose turns
//! could see each other is refused with its reason.

use crate::interp::{Value, run_with_heap};
use crate::{FnId, Module, lower, optimize, verify};

fn lowered(source: &str) -> Result<Module, Vec<String>> {
    let parsed = perasc_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, source);
    assert_eq!(analysis.diagnostics, [], "check errors");
    lower(&parsed.ast, &analysis).map_err(|errors| {
        errors
            .iter()
            .map(|d| {
                let reason = d.message.args.iter().find(|(k, _)| *k == "reason");
                format!("{}:{reason:?}", d.code)
            })
            .collect()
    })
}

/// Each kernel, as `parallel for` and as `for`, on the same inputs.
const KERNELS: &str = "
export fn shade(n: i32, gain: i32): i64 {
  let pixels: Array<i32> = [];
  for (let i = 0; i < n; i++) {
    pixels.push(i.wrappingMul(-1640531535));
  }
  PAR (let i = 0; i < pixels.length; i++) {
    const p = pixels[i];
    let y = (((p >> 16) & 255) * 77 + ((p >> 8) & 255) * 150 + (p & 255) * 29) >> 8;
    if (y > 200) {
      continue;
    }
    pixels[i] = y * gain;
  }
  let sum: i64 = 0;
  for (const p of pixels) {
    sum = sum * 31 + p.toI64();
  }
  return sum;
}

export fn particles(n: i32, steps: i32): f64 {
  let xs: Array<f64> = [];
  let vs: Array<f64> = [];
  for (let i = 0; i < n; i++) {
    xs.push(i.toF64() * 0.25);
    vs.push((i % 7).toF64() - 3.0);
  }
  for (let s = 0; s < steps; s++) {
    PAR (let i = 1; i < n - 1; i++) {
      let v = vs[i];
      const x = xs[i] + v * 0.01;
      if (x < 0.0 || x > 10.0) {
        v = -v;
      }
      vs[i] = v;
      xs[i] = x.sqrt() + x * 0.5;
    }
  }
  let total = 0.0;
  for (const x of xs) {
    total = total * 0.5 + x;
  }
  return total;
}
";

#[test]
fn a_parallel_loop_gives_what_the_same_loop_gives() {
    let parallel =
        lowered(&KERNELS.replace("PAR", "parallel for")).unwrap_or_else(|e| unreachable!("{e:?}"));
    let plain = lowered(&KERNELS.replace("PAR", "for")).unwrap_or_else(|e| unreachable!("{e:?}"));
    // The jobs come after the other functions.
    assert_eq!(parallel.functions.len(), plain.functions.len() + 2);
    assert_eq!(parallel.functions.iter().filter(|f| f.job).count(), 2);
    for module in [&parallel, &plain] {
        assert_eq!(verify(module), Ok(()), "{module}");
    }
    let mut optimized = parallel.clone();
    optimize(&mut optimized);
    assert_eq!(verify(&optimized), Ok(()), "{optimized}");
    let cases: [(u32, Vec<Value>); 4] = [
        (0, vec![Value::I32(0), Value::I32(3)]),
        (0, vec![Value::I32(1000), Value::I32(3)]),
        (1, vec![Value::I32(2), Value::I32(5)]),
        (1, vec![Value::I32(300), Value::I32(20)]),
    ];
    for (f, args) in cases {
        let want = run_with_heap(&plain, FnId(f), &args).unwrap_or_else(|t| unreachable!("{t:?}"));
        for module in [&parallel, &optimized] {
            let (got, heap) =
                run_with_heap(module, FnId(f), &args).unwrap_or_else(|t| unreachable!("{t:?}"));
            assert_eq!(got, want.0, "{f} {args:?}");
            assert_eq!(heap.live, Vec::<u32>::new());
        }
    }
}

#[test]
fn a_loop_whose_turns_could_see_each_other_is_refused_with_its_reason() {
    let cases = [
        ("xs[i] = xs[i - 1] + 1.0;", "reads"),
        ("total += xs[i];", "outer"),
        ("xs[i + 1] = 0.0;", "index"),
        ("xs.push(1.0);", "heap"),
        // Decision 0031: a call to a function that changes something, a
        // call through a function value, a part taken elsewhere than at the
        // counter or of a size the turn computes, a part and the counter's
        // index writing one array.
        ("xs[i] = bump(xs[i]);", "impure"),
        // Writes a module variable without reading it.
        ("xs[i] = mark(xs[i]);", "impure"),
        ("xs[i] = grow(xs[i]);", "impure"),
        ("xs[i] = twiceOf(xs[i]);", "impure"),
        ("let p = xs.part(i + 1, 2);\n    p[0] = 1.0;", "part"),
        ("let p = xs.part(i, i);\n    p[0] = 1.0;", "part"),
        (
            "let p = xs.part(i, 2);\n    p[0] = 1.0;\n    xs[i] = 0.0;",
            "index",
        ),
        ("if (xs[i] > 3.0) { break; }", "exits"),
        ("if (xs[i] > 3.0) { return total; }", "exits"),
    ];
    for (body, reason) in cases {
        let source = format!(
            "fn twice(x: f64): f64 {{\n  return x * 2.0;\n}}\n\
             let count = 0;\n\
             fn bump(x: f64): f64 {{\n  count += 1;\n  return x;\n}}\n\
             fn mark(x: f64): f64 {{\n  count = 1;\n  return x;\n}}\n\
             fn grow(x: f64): f64 {{\n  const s = [x, x];\n  return s[1];\n}}\n\
             fn twiceOf(x: f64): f64 {{\n  const g = twice;\n  return g(x);\n}}\n\
             export fn f(xs: inout Array<f64>): f64 {{\n  let total = 0.0;\n  parallel for (let i = 1; i < xs.length - 1; i++) {{\n    {body}\n  }}\n  return total + twice(1.0) + bump(1.0) + mark(1.0) + grow(1.0) + twiceOf(1.0);\n}}\n"
        );
        let errors = lowered(&source).err().unwrap_or_default();
        assert_eq!(errors.len(), 1, "{body}: {errors:?}");
        assert!(
            errors
                .first()
                .is_some_and(|e| e.starts_with("E0241") && e.contains(reason)),
            "{body}: {errors:?}"
        );
    }
    for head in [
        "let i = 0; i <= 9; i++",
        "let i = 0; i < 9; i += 2",
        "let i = 0; 9 > i; i++",
        "let i: i64 = 0; i < 9; i++",
    ] {
        let source = format!(
            "export fn f(xs: inout Array<f64>): void {{\n  parallel for ({head}) {{\n    xs[0] = 1.0;\n  }}\n}}\n"
        );
        let errors = lowered(&source).err().unwrap_or_default();
        assert!(
            errors
                .iter()
                .any(|e| e.starts_with("E0241") && e.contains("shape")),
            "{head}: {errors:?}"
        );
    }
    // A part taken outside a turn.
    let outside = "export fn f(xs: inout Array<f64>): f64 {\n  let p = xs.part(0, 2);\n  p[0] = 1.0;\n  return xs[0];\n}\n";
    let errors = lowered(outside).err().unwrap_or_default();
    assert!(
        errors
            .iter()
            .any(|e| e.starts_with("E0241") && e.contains("part")),
        "{errors:?}"
    );
    // A loop nested in a turn may break out of itself.
    let nested = "export fn f(xs: inout Array<f64>): void {\n  parallel for (let i = 0; i < xs.length; i++) {\n    let x = xs[i];\n    for (let k = 0; k < 10; k++) {\n      if (x > 100.0) {\n        break;\n      }\n      x = x * 2.0;\n    }\n    xs[i] = x;\n  }\n}\n";
    assert!(lowered(nested).is_ok());
}

/// Decision 0031: a turn that calls a pure function, one returning a
/// structure, and writes through parts of two arrays, reading where it
/// writes, gives what the same loop written by hand on one thread gives.
#[test]
fn pure_calls_and_parts_give_what_one_thread_gives() {
    let source = "struct Pair {\n  a: f64;\n  b: f64;\n}\n\
         fn curve(x: f64): f64 {\n  return x * x * 0.001 + 1.0;\n}\n\
         fn split(x: f64): Pair {\n  return Pair(x * 0.5, x + 2.0);\n}\n\
         export fn bands(n: i32, rows: i32): f64 {\n  let xs: Array<f64> = [];\n  let ys: Array<f64> = [];\n  for (let i = 0; i < n * rows; i++) {\n    xs.push(i.toF64() * 0.5);\n    ys.push(0.0);\n  }\n  parallel for (let b = 0; b < n; b++) {\n    let xb = xs.part(b, rows);\n    let yb = ys.part(b, rows);\n    for (let r = 0; r < rows; r++) {\n      xb[r] = curve(xb[r]) + xb[0];\n      const p = split(xb[r]);\n      yb[r] = p.a * p.b;\n    }\n  }\n  let total = 0.0;\n  for (let i = 0; i < n * rows; i++) {\n    total = total * 0.5 + xs[i] + ys[i];\n  }\n  return total;\n}\n\
         export fn plain(n: i32, rows: i32): f64 {\n  let xs: Array<f64> = [];\n  let ys: Array<f64> = [];\n  for (let i = 0; i < n * rows; i++) {\n    xs.push(i.toF64() * 0.5);\n    ys.push(0.0);\n  }\n  for (let b = 0; b < n; b++) {\n    for (let r = 0; r < rows; r++) {\n      xs[b * rows + r] = curve(xs[b * rows + r]) + xs[b * rows];\n      const p = split(xs[b * rows + r]);\n      ys[b * rows + r] = p.a * p.b;\n    }\n  }\n  let total = 0.0;\n  for (let i = 0; i < n * rows; i++) {\n    total = total * 0.5 + xs[i] + ys[i];\n  }\n  return total;\n}\n";
    let module = lowered(source).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    assert_eq!(module.functions.iter().filter(|f| f.job).count(), 1);
    let mut optimized = module.clone();
    optimize(&mut optimized);
    assert_eq!(verify(&optimized), Ok(()), "{optimized}");
    let names: Vec<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
    let at = |name: &str| {
        FnId(u32::try_from(names.iter().position(|n| *n == name).unwrap_or(0)).unwrap_or(0))
    };
    for args in [
        vec![Value::I32(1), Value::I32(1)],
        vec![Value::I32(4), Value::I32(3)],
        vec![Value::I32(7), Value::I32(16)],
    ] {
        for m in [&module, &optimized] {
            let (want, _) =
                run_with_heap(m, at("plain"), &args).unwrap_or_else(|t| unreachable!("{t:?}"));
            let (got, heap) =
                run_with_heap(m, at("bands"), &args).unwrap_or_else(|t| unreachable!("{t:?}"));
            assert_eq!(got, want, "{args:?}");
            assert_eq!(heap.live, Vec::<u32>::new());
        }
    }
}
