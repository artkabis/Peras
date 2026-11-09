//! The methods of arrays that take an arrow function, run in the
//! interpreter: each is a loop with the function's body inside, on numbers,
//! strings and structures, with nothing left alive.

use crate::interp::{Value, run_with_heap};
use crate::{FnId, lower, verify};

#[test]
fn map_filter_reduce_find_some_and_every_run_without_a_leak() {
    let parsed = perasc_syntax::parse(SOURCE);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, SOURCE);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    for (n, expected) in [(5, 38_592.0), (-1, 7_959.0)] {
        let (value, heap) = run_with_heap(&module, FnId(0), &[Value::I32(n)]).unwrap_or_else(|t| {
            unreachable!(
                "{t:?}
{module}"
            )
        });
        assert_eq!(
            value,
            Value::F64(expected),
            "{n}
{module}"
        );
        assert_eq!(
            heap.live,
            Vec::<u32>::new(),
            "leak on {n}
{module}"
        );
    }
}

const SOURCE: &str = r#"export fn run(n: i32): f64 {
  const xs: Array<i32> = [n, 2, 7, -3, 4];
  const doubled = xs.map((x) => x * 2);
  const big = xs.filter((x) => x > 2);
  const sum = xs.reduce((acc, x) => acc + x, 0);
  const firstBig = xs.find((x) => x > 5).unwrapOr(-1);
  const at = xs.findIndex((x) => x < 0);
  const any = xs.some((x) => x == 7);
  const all = xs.every((x) => x > -10);
  const words: Array<String> = ["a", "bcd", "ef"];
  const loud = words.map((w) => w + "!");
  const long = words.filter((w) => w.length > 1);
  const total = words.reduce((acc, w) => acc + w.length, 0);
  const found = words.find((w) => w.length == 2).unwrapOr("none");
  const points: Array<Point> = [Point(1.0, 2.0), Point(3.0, 4.0)];
  const xsOf = points.map((p) => p.x);
  const far = points.filter((p) => p.y > 3.0);
  const lim = n.clamp(0, 3) + n.min(1) + n.max(10);
  const scale = 2;
  const scaled = xs.map((x) => x * scale + n);
  let flags = 0;
  if (any) {
    flags += 1;
  }
  if (all) {
    flags += 10;
  }
  return (doubled[0] + big.length * 10 + sum * 100 + firstBig * 1000 + at * 10000 + flags + loud[1].length + long.length + total + found.length + lim + scaled[1]).toF64() + xsOf[1] + far.length.toF64();
}

struct Point {
  x: f64;
  y: f64;
}

"#;
