//! A program of random operations on maps and sets, and its model written
//! with the standard library's maps: the compiled program, run as
//! WebAssembly or in the IR interpreter, must give what the model gives.

use std::collections::{HashMap, HashSet};

/// Random operations on a map of integers, a map of strings to floats and a
/// set, each step folded into a checksum. `range` keeps keys few, so that
/// they come back, are deleted and come back again; `clear` now and then.
pub const PROGRAM: &str = "
fn step(s: i32): i32 {
  return s.wrappingMul(1103515245).wrappingAdd(12345);
}

fn ints(n: i32, range: i32): i32 {
  let table: Map<i64, i64> = Map.new();
  let s = 7;
  let check = 0;
  for (let i = 0; i < n; i++) {
    s = step(s);
    const k = ((s / 65536) % range).toI64();
    const op = ((s / 7) % 6).abs();
    if (op <= 1) {
      table.set(k, i.toI64() * 3);
    } else if (op == 2) {
      check = check.wrappingAdd(table.get(k).unwrapOr(-1).toI32());
    } else if (op == 3) {
      if (table.delete(k)) check = check.wrappingAdd(1000);
    } else if (op == 4) {
      if (table.has(k)) check = check.wrappingAdd(7);
    } else if (i % 97 == 5) {
      table.clear();
    }
    check = check.wrappingMul(31).wrappingAdd(table.size);
  }
  return check;
}

fn texts(n: i32, range: i32): i32 {
  let table: Map<String, f64> = Map.new();
  let s = 11;
  let check = 0;
  for (let i = 0; i < n; i++) {
    s = step(s);
    const k = (s / 65536) % range;
    const key = `k${k}é`;
    const op = ((s / 7) % 5).abs();
    if (op <= 1) {
      table.set(key.copy(), i.toF64() * 0.5);
    } else if (op == 2) {
      check = check.wrappingAdd((table.get(key).unwrapOr(-1.0) * 2.0).toI32());
    } else if (op == 3) {
      if (table.delete(key)) check = check.wrappingAdd(1000);
    } else if (table.has(key)) {
      check = check.wrappingAdd(7);
    }
    check = check.wrappingMul(31).wrappingAdd(table.size);
  }
  return check;
}

fn members(n: i32, range: i32): i32 {
  let set: Set<i32> = Set.new();
  let flags: Map<bool, f32> = Map.new();
  let s = 3;
  let check = 0;
  for (let i = 0; i < n; i++) {
    s = step(s);
    const k = (s / 65536) % range;
    const op = ((s / 7) % 3).abs();
    if (op == 0) {
      set.add(k);
    } else if (op == 1) {
      if (set.delete(k)) check = check.wrappingAdd(1000);
    } else if (set.has(k)) {
      check = check.wrappingAdd(7);
    }
    flags.set(k > 0, i.toF32() * 0.25);
    check = check.wrappingMul(31).wrappingAdd(set.size + flags.size);
  }
  const last = flags.get(true).unwrapOr(-4.0) * 4.0;
  return check.wrappingAdd(last.toI32());
}

fn walks(n: i32, range: i32): i32 {
  let table: Map<String, i32> = Map.new();
  let s = 5;
  let check = 0;
  for (let i = 0; i < n; i++) {
    s = step(s);
    const k = (s / 65536) % range;
    const op = ((s / 7) % 4).abs();
    if (op <= 1) {
      table.set(`w${k}`, i);
    } else if (op == 2) {
      table.delete(`w${k}`);
    } else if (i % 50 == 0) {
      for (const e of table) {
        check = check.wrappingMul(31).wrappingAdd(e.0.length * 1000 + e.1);
      }
    }
  }
  for (const e of table) {
    check = check.wrappingMul(31).wrappingAdd(e.0.length * 1000 + e.1);
  }
  let rests: Set<i64> = Set.new();
  for (const e of table) {
    rests.add((e.1 % 7).toI64());
  }
  for (const r of rests) {
    check = check.wrappingMul(7).wrappingAdd(r.toI32());
  }
  return check;
}

export fn intsFew(n: i32): i32 { return ints(n, 16); }
export fn walksFew(n: i32): i32 { return walks(n, 30); }
export fn walksMany(n: i32): i32 { return walks(n, 2000); }
export fn intsMany(n: i32): i32 { return ints(n, 5000); }
export fn textsFew(n: i32): i32 { return texts(n, 24); }
export fn textsMany(n: i32): i32 { return texts(n, 3000); }
export fn membersFew(n: i32): i32 { return members(n, 40); }
";

fn step(s: i32) -> i32 {
    s.wrapping_mul(1_103_515_245).wrapping_add(12345)
}

/// [`PROGRAM`]'s `ints`, with a `HashMap`.
fn ints(n: i32, range: i32) -> i32 {
    let mut table: HashMap<i64, i64> = HashMap::new();
    let (mut s, mut check) = (7_i32, 0_i32);
    for i in 0..n {
        s = step(s);
        let k = i64::from((s / 65536) % range);
        let op = ((s / 7) % 6).abs();
        match op {
            0 | 1 => {
                table.insert(k, i64::from(i) * 3);
            }
            2 => {
                let v = table.get(&k).copied().unwrap_or(-1);
                check = check.wrapping_add(i32::try_from(v).unwrap_or(0));
            }
            3 => {
                if table.remove(&k).is_some() {
                    check = check.wrapping_add(1000);
                }
            }
            4 => {
                if table.contains_key(&k) {
                    check = check.wrapping_add(7);
                }
            }
            _ => {
                if i % 97 == 5 {
                    table.clear();
                }
            }
        }
        let size = i32::try_from(table.len()).unwrap_or(0);
        check = check.wrapping_mul(31).wrapping_add(size);
    }
    check
}

/// [`PROGRAM`]'s `texts`, with a `HashMap`.
#[allow(clippy::cast_possible_truncation)]
fn texts(n: i32, range: i32) -> i32 {
    let mut table: HashMap<String, f64> = HashMap::new();
    let (mut s, mut check) = (11_i32, 0_i32);
    for i in 0..n {
        s = step(s);
        let key = format!("k{}é", (s / 65536) % range);
        let op = ((s / 7) % 5).abs();
        match op {
            0 | 1 => {
                table.insert(key, f64::from(i) * 0.5);
            }
            2 => {
                let v = table.get(&key).copied().unwrap_or(-1.0) * 2.0;
                check = check.wrapping_add(v as i32);
            }
            3 => {
                if table.remove(&key).is_some() {
                    check = check.wrapping_add(1000);
                }
            }
            _ => {
                if table.contains_key(&key) {
                    check = check.wrapping_add(7);
                }
            }
        }
        let size = i32::try_from(table.len()).unwrap_or(0);
        check = check.wrapping_mul(31).wrapping_add(size);
    }
    check
}

/// [`PROGRAM`]'s `members`, with a `HashSet` and a `HashMap`.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn members(n: i32, range: i32) -> i32 {
    let mut set: HashSet<i32> = HashSet::new();
    let mut flags: HashMap<bool, f32> = HashMap::new();
    let (mut s, mut check) = (3_i32, 0_i32);
    for i in 0..n {
        s = step(s);
        let k = (s / 65536) % range;
        match ((s / 7) % 3).abs() {
            0 => {
                set.insert(k);
            }
            1 => {
                if set.remove(&k) {
                    check = check.wrapping_add(1000);
                }
            }
            _ => {
                if set.contains(&k) {
                    check = check.wrapping_add(7);
                }
            }
        }
        flags.insert(k > 0, i as f32 * 0.25);
        let size = i32::try_from(set.len() + flags.len()).unwrap_or(0);
        check = check.wrapping_mul(31).wrapping_add(size);
    }
    let last = flags.get(&true).copied().unwrap_or(-4.0) * 4.0;
    check.wrapping_add(last as i32)
}

/// [`PROGRAM`]'s `walks`, with vectors in insertion order: a key set again
/// keeps its place, a key deleted then set again goes last.
fn walks(n: i32, range: i32) -> i32 {
    fn fold(check: i32, table: &[(String, i32)]) -> i32 {
        table.iter().fold(check, |c, (k, v)| {
            let len = i32::try_from(k.len()).unwrap_or(0);
            c.wrapping_mul(31).wrapping_add(len * 1000 + v)
        })
    }
    let mut table: Vec<(String, i32)> = Vec::new();
    let (mut s, mut check) = (5_i32, 0_i32);
    for i in 0..n {
        s = step(s);
        let key = format!("w{}", (s / 65536) % range);
        let place = table.iter().position(|(k, _)| *k == key);
        match ((s / 7) % 4).abs() {
            0 | 1 => match place {
                Some(p) => {
                    if let Some(slot) = table.get_mut(p) {
                        slot.1 = i;
                    }
                }
                None => table.push((key, i)),
            },
            2 => {
                if let Some(p) = place {
                    table.remove(p);
                }
            }
            _ => {
                if i % 50 == 0 {
                    check = fold(check, &table);
                }
            }
        }
    }
    check = fold(check, &table);
    let mut rests: Vec<i64> = Vec::new();
    for (_, v) in &table {
        let r = i64::from(v % 7);
        if !rests.contains(&r) {
            rests.push(r);
        }
    }
    for r in rests {
        check = check
            .wrapping_mul(7)
            .wrapping_add(i32::try_from(r).unwrap_or(0));
    }
    check
}

pub type Model = fn(i32) -> i32;

pub const CASES: [(&str, Model); 7] = [
    ("intsFew", |n| ints(n, 16)),
    ("intsMany", |n| ints(n, 5000)),
    ("textsFew", |n| texts(n, 24)),
    ("textsMany", |n| texts(n, 3000)),
    ("membersFew", |n| members(n, 40)),
    ("walksFew", |n| walks(n, 30)),
    ("walksMany", |n| walks(n, 2000)),
];
