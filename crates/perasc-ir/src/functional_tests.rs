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

#[test]
fn the_functions_of_numbers_round_halves_up_and_compute_as_math_does() {
    let source = "export fn f(x: f64): f64 {\n  return x.round() * 1000.0 + x.pow(2.0) + x.hypot(4.0) + x.atan2(1.0) + x.exp().log();\n}\n";
    let parsed = perasc_syntax::parse(source);
    let analysis = perasc_check::analyze(&parsed.ast, source);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    for x in [2.5_f64, -2.5, 3.0, 0.499_999_999_999_999_94] {
        let (value, _) = run_with_heap(&module, FnId(0), &[Value::F64(x)])
            .unwrap_or_else(|t| unreachable!("{t:?}\n{module}"));
        let down = x.floor();
        let round = if x - down >= 0.5 { down + 1.0 } else { down };
        let expected = round * 1000.0 + x.powf(2.0) + x.hypot(4.0) + x.atan2(1.0) + x.exp().ln();
        let Value::F64(got) = value else {
            unreachable!("{value:?}")
        };
        assert!((got - expected).abs() < 1e-9, "{x}: {got} for {expected}");
    }
}

#[test]
fn integers_and_booleans_become_text_and_text_becomes_integers() {
    let parsed = perasc_syntax::parse(FORMAT);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, FORMAT);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    // The smallest `i64` round-trips, and is out of the range of `i32`.
    for (n, expected) in [(i64::MIN, 253_305), (42, 253_126), (0, 253_116)] {
        let (value, heap) = run_with_heap(&module, FnId(0), &[Value::I64(n)])
            .unwrap_or_else(|t| unreachable!("{t:?}\n{module}"));
        assert_eq!(value, Value::I64(expected), "{n}\n{module}");
        assert_eq!(heap.live, Vec::<u32>::new(), "leak on {n}\n{module}");
    }
}

const FORMAT: &str = r#"export fn run(n: i64): i64 {
  const text = n.toString();
  let score = text.parseI64().unwrapOr(7) - n;
  if (text.parseI32().isSome()) score += 1;
  score += text.length.toI64() * 10;
  if (true.toString() == "true") score += 100;
  score += false.toString().length.toI64();
  const m: i32 = -7;
  score += m.toString().length.toI64() * 100000;
  let valid = 0;
  const inputs: Array<String> = ["", "-", "+5", "12a", " 1", "2147483648", "-2147483648", "9223372036854775807", "-9223372036854775809", "007"];
  for (const s of inputs) {
    if (s.parseI32().isSome()) valid += 1;
    if (s.parseI64().isSome()) valid += 10;
  }
  return score + valid.toI64() * 1000;
}
"#;

#[test]
fn filter_on_a_temporary_keeps_its_elements_in_place_without_a_leak() {
    let parsed = perasc_syntax::parse(KEEP);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, KEEP);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    assert!(module.to_string().contains("arr_keep"), "{module}");
    // Checked by hand: 5 words in " a  bb ccc  dddd e "; "bb-ccc-dddd", 11
    // bytes; the tags past 1.0, "bbc" or "bbcdd"; 5 or 2 even numbers; 7
    // letters in the words longer than 2.
    for (n, expected) in [(0, 75_011_352), (3, 75_011_523)] {
        let (value, heap) = run_with_heap(&module, FnId(2), &[Value::I32(n)])
            .unwrap_or_else(|t| unreachable!("{t:?}\n{module}"));
        assert_eq!(value, Value::I32(expected), "{n}\n{module}");
        assert_eq!(heap.live, Vec::<u32>::new(), "leak on {n}\n{module}");
    }
}

const KEEP: &str = r#"struct Tag {
  x: f64;
  name: String;
}

fn tags(n: i32): Array<Tag> {
  return [Tag(0.5, "a"), Tag(2.0, "bb"), Tag(3.0, "c"), Tag(n.toF64(), "dd")];
}

fn long(text: String): Array<String> {
  return text.split(" ").filter((w) => w.length > 1);
}

export fn run(n: i32): i32 {
  const words = " a  bb ccc  dddd e ".split(" ").filter((w) => w.length > 0).length;
  const kept = long(" a  bb ccc  dddd e ");
  const joined = kept.join("-");
  const big = tags(n).filter((t) => t.x > 1.0);
  let names = "";
  for (const t of big) {
    names += t.name;
  }
  const doubled = [1, 2, 3, 4, 5].map((x) => x * n).filter((x) => x % 2 == 0).length;
  // A loop over a temporary array, which lives through every turn.
  let letters = 0;
  for (const w of " a  bb ccc  dddd e ".split(" ")) {
    if (w.length > 2) letters += w.length;
    if (letters > 100) return -1;
    if (w.length > 10) break;
  }
  return words * 1000000 + joined.length * 1000 + names.length * 100 + doubled * 10 + big.length + letters * 10000000;
}
"#;

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
