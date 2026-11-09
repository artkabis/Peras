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

#[test]
fn text_methods_give_what_the_runtime_gives() {
    let parsed = perasc_syntax::parse(TEXT);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, TEXT);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    // The same value as the WebAssembly runtime gives, checked by hand.
    for (n, expected) in [(3, 3_271_180), (0, 3_271_174)] {
        let (value, heap) = run_with_heap(&module, FnId(0), &[Value::I32(n)])
            .unwrap_or_else(|t| unreachable!("{t:?}\n{module}"));
        assert_eq!(value, Value::I32(expected), "{n}\n{module}");
        assert_eq!(heap.live, Vec::<u32>::new(), "leak on {n}\n{module}");
    }
}

#[test]
fn slice_concat_reverse_sort_and_search_give_what_the_runtime_gives() {
    let parsed = perasc_syntax::parse(ARRAYS);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, ARRAYS);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    // The values the WebAssembly runtime gives, checked by hand.
    for (n, expected) in [(4, 2_493_165), (20, 2_503_165)] {
        let (value, heap) = run_with_heap(&module, FnId(0), &[Value::I32(n)])
            .unwrap_or_else(|t| unreachable!("{t:?}\n{module}"));
        assert_eq!(value, Value::I32(expected), "{n}\n{module}");
        assert_eq!(heap.live, Vec::<u32>::new(), "leak on {n}\n{module}");
    }
}

const ARRAYS: &str = r#"struct Point {
  x: f64;
  name: String;
}

export fn run(n: i32): i32 {
  let xs: Array<i32> = [5, n, 3, 9, 1, 7];
  xs.sort();
  let floats: Array<f64> = [2.5, -1.0, 0.5];
  floats.sort();
  let words: Array<String> = ["c", "a", "b"];
  words.reverse();
  const middle = xs.slice(1, -1);
  const both = middle.concat(xs.slice(0, 1));
  const tail = words.slice(-2, 10);
  const points: Array<Point> = [Point(1.0, "p"), Point(2.0, "q")];
  let more = points.concat(points.slice(1, 2));
  more.reverse();
  let score = 0;
  if (xs.includes(9)) {
    score += 1;
  }
  if (!xs.includes(100)) {
    score += 10;
  }
  if (words.includes("a")) {
    score += 100;
  }
  return score + xs[0] * 1000 + xs[5] * 10000 + xs.indexOf(7) * 100000 + words.indexOf("c") * 1000000
    + both.length * 10 + tail.length * 1000 + more.length + more[0].name.length + floats[0].toI32() * 5 + (floats[2] * 2.0).toI32();
}
"#;

const TEXT: &str = r#"export fn run(n: i32): i32 {
  const s = "  Hello, wörld!  ";
  const t = s.trim();
  const words = t.split(", ");
  const chars = "hé!".split("");
  const joined = words.join(" | ");
  const up = t.toUpperCase();
  const low = t.toLowerCase();
  const part = t.slice(0, 5);
  const tail = t.slice(-6, 100);
  const cut = "aé".slice(0, 2);
  const rep = "ab".repeat(n);
  const one = "a-b-c".replace("-", "+");
  const all = "a-b-c".replaceAll("-", "");
  let score = 0;
  if (t.startsWith("Hello")) {
    score += 1;
  }
  if (t.endsWith("!")) {
    score += 10;
  }
  if (t.includes("wör")) {
    score += 100;
  }
  if (!t.includes("xyz")) {
    score += 1000;
  }
  return score + t.indexOf("w") * 10000 + words.length * 100000 + chars.length * 1000000
    + joined.length + up.length + low.length + part.length + tail.length + cut.length + rep.length + one.length + all.length;
}

export fn shout(s: String): String {
  return s.trim().toUpperCase().replaceAll(" ", "_");
}

export fn csv(line: String): Array<String> {
  return line.split(",").map((x) => x.trim());
}
"#;

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
