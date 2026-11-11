//! A program that sorts random records with `sortBy` and `sortByDescending`,
//! and its model, a stable sort of the standard library: the compiled
//! program, run as WebAssembly, in the IR interpreter or as a managed module
//! in JavaScript, must give what the model gives.
//!
//! The keys have duplicates, `-0` and `+0`, NaN, and strings whose order in
//! UTF-16 differs from that of their code points: `\u{e000}` against `😀`.

/// Builds `n` records from a random sequence, sorts them by one key, and folds
/// the order of their ids into a checksum.
pub const PROGRAM: &str = r#"
struct Rec {
  id: i32;
  amount: f64;
  count: i32;
  big: i64;
  ratio: f32;
  name: String;
}

fn step(s: i32): i32 {
  return s.wrappingMul(1103515245).wrappingAdd(12345);
}

fn records(n: i32): Array<Rec> {
  const letters = ["a", "b", "é", "Z", "\u{e000}", "😀"];
  let xs: Array<Rec> = [];
  let s = 5;
  for (let i = 0; i < n; i++) {
    s = step(s);
    const r = ((s / 65536) % 1000).abs();
    let amount = (r % 50).toF64() - 25.0;
    if (r % 11 == 0) {
      amount = 0.0 / 0.0;
    } else if (r % 11 == 1) {
      amount = -0.0;
    } else if (r % 11 == 2) {
      amount = 0.0;
    }
    let name = "";
    for (let j = 0; j < 1 + r % 3; j++) {
      name = `${name}${letters[(r / (j + 1)) % 6]}`;
    }
    xs.push(Rec(i, amount, r % 13 - 6, (r % 17).toI64() - 8, (r % 9).toF32() * 0.5, name));
  }
  return xs;
}

fn checksum(xs: Array<Rec>): i32 {
  let check = 0;
  for (const x of xs) {
    check = check.wrappingMul(31).wrappingAdd(x.id);
  }
  return check;
}

export fn byAmount(n: i32): i32 {
  let xs = records(n);
  xs.sortBy((x) => x.amount);
  return checksum(xs);
}

export fn byAmountDown(n: i32): i32 {
  let xs = records(n);
  xs.sortByDescending((x) => x.amount);
  return checksum(xs);
}

export fn byCount(n: i32): i32 {
  let xs = records(n);
  xs.sortBy((x) => x.count);
  return checksum(xs);
}

export fn byBigDown(n: i32): i32 {
  let xs = records(n);
  xs.sortByDescending((x) => x.big);
  return checksum(xs);
}

export fn byRatio(n: i32): i32 {
  let xs = records(n);
  xs.sortBy((x) => x.ratio);
  return checksum(xs);
}

export fn byName(n: i32): i32 {
  let xs = records(n);
  xs.sortBy((x) => x.name);
  return checksum(xs);
}

export fn byNameDown(n: i32): i32 {
  let xs = records(n);
  xs.sortByDescending((x) => x.name);
  return checksum(xs);
}

export fn byComputed(n: i32): i32 {
  let xs = records(n);
  xs.sortBy((x) => x.count * 3 - x.id % 5);
  return checksum(xs);
}
"#;

struct Rec {
    id: i32,
    amount: f64,
    count: i32,
    big: i64,
    ratio: f32,
    name: String,
}

fn step(s: i32) -> i32 {
    s.wrapping_mul(1_103_515_245).wrapping_add(12345)
}

fn records(n: i32) -> Vec<Rec> {
    let letters = ["a", "b", "é", "Z", "\u{e000}", "😀"];
    let mut xs = Vec::new();
    let mut s = 5_i32;
    for i in 0..n {
        s = step(s);
        let r = ((s / 65536) % 1000).abs();
        let amount = match r % 11 {
            0 => f64::NAN,
            1 => -0.0,
            2 => 0.0,
            _ => f64::from(r % 50) - 25.0,
        };
        let mut name = String::new();
        for j in 0..=(r % 3) {
            let at = usize::try_from((r / (j + 1)) % 6).unwrap_or(0);
            name.push_str(letters.get(at).copied().unwrap_or(""));
        }
        let ratio = f32::from(u8::try_from(r % 9).unwrap_or(0)) * 0.5;
        xs.push(Rec {
            id: i,
            amount,
            count: r % 13 - 6,
            big: i64::from(r % 17) - 8,
            ratio,
            name,
        });
    }
    xs
}

fn checksum(xs: &[Rec]) -> i32 {
    xs.iter()
        .fold(0_i32, |c, x| c.wrapping_mul(31).wrapping_add(x.id))
}

/// Every NaN is the same, and last: the order of `sort()`.
fn canonical(x: f64) -> f64 {
    if x.is_nan() { f64::NAN } else { x }
}

fn sorted(n: i32, desc: bool, order: fn(&Rec, &Rec) -> std::cmp::Ordering) -> i32 {
    let mut xs = records(n);
    // A stable sort: equal keys keep their order, descending too.
    xs.sort_by(|a, b| if desc { order(b, a) } else { order(a, b) });
    checksum(&xs)
}

/// A function of the program, and its model.
pub type Model = fn(i32) -> i32;

pub const CASES: [(&str, Model); 8] = [
    ("byAmount", |n| {
        sorted(n, false, |a, b| {
            canonical(a.amount).total_cmp(&canonical(b.amount))
        })
    }),
    ("byAmountDown", |n| {
        sorted(n, true, |a, b| {
            canonical(a.amount).total_cmp(&canonical(b.amount))
        })
    }),
    ("byCount", |n| {
        sorted(n, false, |a, b| a.count.cmp(&b.count))
    }),
    ("byBigDown", |n| sorted(n, true, |a, b| a.big.cmp(&b.big))),
    ("byRatio", |n| {
        sorted(n, false, |a, b| a.ratio.total_cmp(&b.ratio))
    }),
    ("byName", |n| sorted(n, false, |a, b| a.name.cmp(&b.name))),
    ("byNameDown", |n| {
        sorted(n, true, |a, b| a.name.cmp(&b.name))
    }),
    ("byComputed", |n| {
        sorted(n, false, |a, b| {
            (a.count * 3 - a.id % 5).cmp(&(b.count * 3 - b.id % 5))
        })
    }),
];

/// The sizes tried: empty, one, under and over the thresholds of 64 and 256.
pub const SIZES: [i32; 6] = [0, 1, 7, 63, 300, 1000];
