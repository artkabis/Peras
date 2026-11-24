//! Differential testing: random programs, made from a seed, run four ways that
//! must agree, the interpreter of the IR as written being the oracle:
//! - the IR as written, interpreted;
//! - the IR optimized, interpreted;
//! - WebAssembly built without the IR's optimizations, on the debug allocator;
//! - WebAssembly built with them, on the debug allocator and on TLSF.
//!
//! An optimization that changes what a program computes, or leaves a block
//! alive, makes them disagree, whatever the program it was written for
//! (decision 0016). The programs use maps keyed by strings, splits, characters,
//! options, loops and branches: what the optimizations of decision 0016 touch.

#[path = "fixtures/link.rs"]
mod link;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_driver::DirectOptions;
use perasc_ir::interp::{self, Value};

/// A small deterministic generator, so that a failure can be replayed.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).unwrap_or(1)).unwrap_or(0)
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items.get(self.below(items.len())).copied().unwrap_or("")
    }
}

/// A key looked up, lent.
fn key(r: &mut XorShift) -> String {
    r.pick(&["\"a\"", "\"b\"", "\"ab\"", "k", "`${i % 3}`"])
        .to_owned()
}

/// The same key, owned, for `set`.
fn owned_key(key: &str) -> String {
    if key == "k" {
        "k.copy()".to_owned()
    } else {
        key.to_owned()
    }
}

fn value(r: &mut XorShift) -> &'static str {
    r.pick(&["i", "acc % 7", "3"])
}

/// One statement of the loop's body, at most `depth` branches deep.
fn statement(r: &mut XorShift, depth: u32, out: &mut String, indent: &str) {
    // Decision 0044, prototypes: one statement more, a pool, when one is set.
    let pool = std::env::var("PERAS_PROTO_POOL").is_ok_and(|v| !v.is_empty());
    let choices = if depth == 0 { 49 } else { 50 } + usize::from(pool);
    let k = key(r);
    let v = value(r);
    let line = match r.below(choices) {
        0 => format!("if (!m.has({k})) {{ m.set({}, {v}); }}", owned_key(&k)),
        1 => format!("acc += m.get({k}).unwrapOr({v});"),
        2 => format!("if (m.has({k})) {{ acc += {v}; }}"),
        3 => format!("m.set({}, {v});", owned_key(&k)),
        4 => format!("if (m.has({k})) {{ m.delete({k}); }}"),
        5 => "k += \"b\";".to_owned(),
        6 => "if (k.length > 4) { k = \"a\"; }".to_owned(),
        7 => "for (const w of \"x,yy,zzz\".split(\",\")) { acc += w.length; }".to_owned(),
        8 => "const parts = s.split(\",\"); acc += parts.length;".to_owned(),
        9 => format!("s += {k}; s += \",\"; if (s.length > 24) {{ s = \"\"; }}"),
        10 => {
            "for (const c of k.split(\"\")) { if (c.toUpperCase() != c) { acc += 1; } }".to_owned()
        }
        11 => format!(
            "if (!m.has({k})) {{ m.set({}, {v}); }} acc += m.get({k}).unwrapOr(0);",
            owned_key(&k)
        ),
        12 => format!("acc += {k}.length; acc += m.size;"),
        13 => format!("names.set(i % 4, {}.copy());", r.pick(&["k", "s"])),
        // A fallback that copies: made only when the key is missing.
        14 => format!(
            "acc += names.get(i % 3).unwrapOr({}.copy()).length;",
            r.pick(&["k", "s"])
        ),
        // A fallback that moves a variable: the variable is released
        // either way.
        15 => "const t = k.copy(); acc += names.get(i % 5).unwrapOr(t).length;".to_owned(),
        16 => "if (names.has(i % 4)) { names.delete(i % 4); }".to_owned(),
        // A value taken out of a map, changed, and put back: one lookup.
        17 => format!(
            "let p = pairs.get({k}).unwrapOr(Pair({}.copy(), 0)); p.count += 1; pairs.set({}, p);",
            r.pick(&["k", "s"]),
            owned_key(&k)
        ),
        18 => format!(
            "let p = pairs.get({k}).unwrapOr(Pair(\"\", i)); p.count += i; if (p.count > 50) {{ pairs.delete({k}); }} else {{ pairs.set({}, p); }}",
            owned_key(&k)
        ),
        19 => "nums.set(i % 7, nums.get(i % 7).unwrapOr(0) + acc % 5);".to_owned(),
        20 => format!("acc += pairs.get({k}).unwrapOr(Pair(\"z\", 1)).count + pairs.size;"),
        // A fallback that is not quiet: it runs first.
        21 => format!("acc += pairs.get({k}).unwrapOr(made(i)).count;"),
        // The key changed between the lookup and the set: looked up again.
        22 => "let p = pairs.get(k).unwrapOr(Pair(\"q\", 0)); k += \"b\"; p.count += 2; pairs.set(k.copy(), p);".to_owned(),
        // Templates whose integers are written into their string: negative,
        // zero, wide.
        24 => "const t = `${i - 3}:${acc - 99}|${k}`; acc += t.length; if (t.startsWith(\"-\")) { acc += 1; }".to_owned(),
        25 => "const t = `${(i - 5).toI64() * 1000000000000}${true}${i}`; acc += t.length;".to_owned(),
        // A set of `i32` that grows, keys negative among them, kept in its
        // index (decision 0018).
        26 => "ids.add((i * 37) % 1000 - 300); ids.add(i - 2147483647);".to_owned(),
        27 => "if (ids.has(i - 20)) { ids.delete(i - 20); acc += 1; } if (nums.has(i % 5)) { acc += 2; }".to_owned(),
        // Keys of 64 bits that share their low 32 bits: never kept in an
        // index of 32 bits.
        28 => "wide.set(i.toI64() * 4294967296 + 5, i); acc += wide.get((i % 3).toI64() * 4294967296 + 5).unwrapOr(7) + wide.size;".to_owned(),
        // Integers read at and past the bounds of 32 and 64 bits.
        29 => "const w = `${i - 20}`; acc += w.parseI32().unwrapOr(-1) + \"2147483647\".parseI32().unwrapOr(1) % 7 + \"2147483650\".parseI32().unwrapOr(3) + \"-2147483648\".parseI32().unwrapOr(4) % 5; if (\"-9223372036854775809\".parseI64().isSome() || \"9223372036854775807\".parseI64().isNone()) { acc += 11; }".to_owned(),
        // `%` on floats, the runtime's in WebAssembly, Rust's in the oracle.
        30 => "acc += ((i.toF64() * 1.5 - 7.25) % 4.0 * 100.0).toI32() + (i.toF32() % -2.5).toI32();".to_owned(),
        // `==` on arrays of strings and of arrays, deep in the runtime.
        31 => "const a = s.split(\",\"); const b = `${k},${s}`.split(\",\"); if (a == b) { acc += 5; } if (a != \"x,yy\".split(\",\")) { acc += 1; } if ([[i % 3, 1], [2]] == [[1, 1], [2]]) { acc += 3; } if ([[0.0 / 0.0]] == [[0.0 / 0.0]] || [[-0.0]] != [[0.0]]) { acc += 7; }".to_owned(),
        // `reduce` keeping a string or an array: the previous one released
        // at each turn, the accumulator read in the body.
        32 => "const j = `${k},${s},${i}`.split(\",\").reduce((a, w) => a.slice(0, 6) + w + \"|\", k.copy()); const d = [i, acc % 9].reduce((a, x) => a.concat([x, a.length]), [1]); acc += j.length + d.length + d[d.length - 1];".to_owned(),
        // `copy()` of structures: one a loop over a map lends, one read
        // out of it, changed without changing the map.
        33 => format!("for (const e of pairs) {{ let c = e.1.copy(); c.name += \"q\"; acc += c.name.length + c.count; }} let d = pairs.get({k}).unwrapOr(Pair(\"d\", 2)).copy(); d.count += 1; acc += d.count;"),
        // A module variable, which a call changes: a value read before the
        // call is not the value after it (decision 0023).
        // `spun` uses no memory: the optimizations run on its loop.
        34 => "for (let j = 0; j < 3; j++) { const before = tally; acc += bumped(j + i) - before % 50; } if (tally % 2 == 0) { tally += 1; } acc += tally % 7 + spun(i);".to_owned(),
        // The integer types of decision 0024: unsigned division, remainder,
        // shifts and comparisons, each kept in its canonical form.
        35 => "b8 = b8.wrappingMul(31).wrappingAdd(i.toU8()); h16 = h16.wrappingAdd(b8.toU16() << 3); if (b8 > 200) { acc += 1; } acc += (b8 / 3).toI32() + (h16 % 1000).toI32() + (h16 >> 13).toI32();".to_owned(),
        36 => "u = u.wrappingMul(2654435761).wrappingAdd(i.toU32()); if (u > 3000000000) { acc += 2; } acc += (u >> 28).toI32() + (u % 97).toI32(); big = big.wrappingMul(6364136223846793005).wrappingAdd(u.toU64()); if (big > 9223372036854775808) { acc += 3; } acc += (big >> 60).toI32() + (big % 13).toI32() + (big / 3000000000000000000).toI32();".to_owned(),
        // An array of `i16` at its width, sorted by insertion when short, and
        // at 71 elements by the runtime's radix sort.
        37 => "w16 = w16.wrappingSub(1234).wrappingMul(3); s8 = (s8 ^ i.toI8()).wrappingAdd(-100); ws.push(w16); if (ws.length % 9 == 8) { ws.sort(); acc += ws[0].toI32() % 100 + ws[ws.length - 1].toI32() % 100; } if (ws.length > 70) { ws = []; } acc += (w16 / 7).toI32() % 50 + (s8 % 9).toI32(); if (s8 < -50 || w16 >= 1000) { acc += 4; }".to_owned(),
        // A field moved out of a structure made in the turn, on one branch
        // only; the rest of it is still read (decision 0025).
        38 => "let q = Pair(`${k}${i}`, i); if (i % 2 == 0) { const taken = q.name; acc += taken.length; } else { acc += q.name.length * 2; } acc += q.count;".to_owned(),
        // A dispatch through an array of functions, one an arrow function,
        // and a function chosen by a call (decision 0025).
        39 => "const fs: Array<(i32) => i32> = [inc, dbl, (x: i32): i32 => x % 7]; const g = pickFn(i); acc += (fs[i % 3])(acc % 1000) + g(i);".to_owned(),
        // Closures made in a loop over the turn's values, copied, and called
        // through an array of functions and after it (decision 0026).
        40 => "const c = i % 11; const d = acc % 13; let cs: Array<(i32) => i32> = []; for (let t = 0; t < 3; t++) { const e = t * c + d; cs.push((x: i32): i32 => x * 3 + e - c); } const h = cs[i % 3]; for (const q of cs) { acc += q(i) % 97; } acc += h(d) % 89;".to_owned(),
        // Strings captured (decision 0027): one closure only called, which
        // lends what it captures, one escaping into an array, which copies.
        // Two strings each, so that a wrong slot reads the wrong one.
        41 => "const word = `${k}${i % 7}`; const tail = `${s}|${word}|`; const lent = (x: i32): i32 => x + word.length * 3 + tail.length; let held: Array<(i32) => i32> = []; held.push((x: i32): i32 => x * 2 + tail.length * 5 + word.length); acc += lent(i) % 13 + (held[0])(acc % 50) % 17;".to_owned(),
        // Decision 0028: an option narrowed by a test that leaves, a string
        // read then moved and a number; a fallback with an effect, run only
        // when there is no value; a key moved at the call that a later
        // argument reads, into a map and into a constructor.
        42 => "const o = names.get(i % 4); if (o.isNone()) { names.set(i % 4, `n${i}`); acc += 1; continue; } acc += o.length * 3; names.set((i + 1) % 4, o); const q = nums.get(i % 7); if (q.isNone()) { acc += 2; continue; } acc += q % 11;".to_owned(),
        43 => "acc += nums.get(i % 5).unwrapOr(bumped(i)) % 13 + names.get(i % 6).unwrapOr(`f${tally % 7}`).length;".to_owned(),
        44 => "const key = `${k}${i % 3}`; m.set(key, m.get(key).unwrapOr(0) + key.length); const nm = `p${i % 4}`; pairs.set(`p${i % 4}`, Pair(nm, nm.length + i % 3));".to_owned(),
        // Decision 0036: small functions called in loops, one with an
        // argument read twice, a float division and a call to a small
        // function that do not change in the loop, one in a branch.
        45 => "for (let j = 0; j < 4; j++) { const fx = i.toF64() / 3.0; acc += (tri(fx + j.toF64() * 0.25) * 100.0).toI32() + (unit(i * 7919 + j) * 50.0).toI32(); }".to_owned(),
        46 => "for (let j = 0; j < 3; j++) { if (j % 2 == 0) { const q = i.toF64() / 7.0 + tri(i.toF64() * 0.1); acc += (q * 3.0).toI32() % 41 + j; } else { const r = acc.toF64() / 9.0; acc += mixed(i, j) % 23 + (r * 0.5).toI32() % 7; } }".to_owned(),
        // Arguments with an effect, whose order counts; a call on the right
        // of `&&`; a module variable read before a call that changes it.
        47 => "acc += mixed(bumped(i), bumped(i + 1)) % 31 + twice(acc % 100); if (acc % 3 == 0 && mixed(bumped(i), i) > 3) { acc += 1; } acc += tally % 7 + twice(bumped(i));".to_owned(),
        // A small function in the condition of a loop, evaluated at each turn.
        48 => "for (let j = 0; twice(j) < i % 9 + 2; j++) { acc += (unit(j * 31 + i) * 10.0).toI32() + 1; }".to_owned(),
        // Decision 0044: values inserted, some removed, slots given again,
        // and every handle read, those kept across a removal included.
        49 if pool => "let pl: Pool<i32> = Pool.new(); let hs: Array<Handle> = []; for (let t = 0; t < 8; t++) { hs.push(pl.insert(t * i + acc % 7)); } for (let t = 0; t < 8; t += 3) { pl.remove(hs[t]); } hs.push(pl.insert(i + 100)); hs.push(pl.insert(acc % 50)); for (const h of hs) { acc += pl.get(h).unwrapOr(-3) % 97; if (pl.has(h)) { acc += 1; } } acc += pl.size();".to_owned(),
        // The map changed between: looked up again.
        23 => format!(
            "let p = pairs.get({k}).unwrapOr(Pair(\"r\", 0)); pairs.delete(\"a\"); p.count += 3; pairs.set({}, p);",
            owned_key(&k)
        ),
        _ => {
            let mut then = String::new();
            let mut otherwise = String::new();
            let inner = format!("{indent}  ");
            statement(r, depth - 1, &mut then, &inner);
            statement(r, depth - 1, &mut otherwise, &inner);
            format!("if (acc % 2 == 0) {{\n{then}{indent}}} else {{\n{otherwise}{indent}}}")
        }
    };
    out.push_str(indent);
    out.push_str(&line);
    out.push('\n');
}

/// A program; under prototype p3 of decision 0044 it imports the pool.
fn program(seed: u64) -> String {
    let text = program_body(seed);
    if std::env::var("PERAS_PROTO_POOL").is_ok_and(|v| v == "p3") {
        format!("import {{ Pool, Handle }} from \"peras:pool\";\n{text}")
    } else {
        text
    }
}

fn program_body(seed: u64) -> String {
    let mut r = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut body = String::new();
    for _ in 0..(3 + r.below(6)) {
        // Each statement in its own block: `const` names do not clash.
        let mut one = String::new();
        statement(&mut r, 2, &mut one, "      ");
        body.push_str("    {\n");
        body.push_str(&one);
        body.push_str("    }\n");
    }
    format!(
        "struct Pair {{
  name: String;
  count: i32;
}}

fn made(i: i32): Pair {{
  return Pair(\"m\", i % 4);
}}

let tally = 0;

fn bumped(i: i32): i32 {{
  tally += i % 3 + 1;
  return tally % 50;
}}

fn spun(n: i32): i32 {{
  let total = 0;
  for (let j = 0; j < n % 5 + 1; j++) {{
    const before = tally;
    total += bumped(j) - before % 50;
  }}
  return total;
}}

fn inc(x: i32): i32 {{
  return x + 1;
}}

fn dbl(x: i32): i32 {{
  return x * 2 % 1000;
}}

fn tri(x: f64): f64 {{
  return (2.0 * (x - x.floor()) - 1.0).abs();
}}

fn unit(h: i32): f64 {{
  return (h & 16777215).toF64() / 16777216.0;
}}

fn mixed(a: i32, b: i32): i32 {{
  return a * 3 + b / 7 - a % 5;
}}

fn twice(x: i32): i32 {{
  return inc(x) + inc(x * 2);
}}

fn pickFn(i: i32): (i32) => i32 {{
  if (i % 2 == 0) {{
    return inc;
  }}
  return dbl;
}}

export fn f(n: i32): i32 {{
  let m: Map<String, i32> = Map.new();
  let pairs: Map<String, Pair> = Map.new();
  let nums: Map<i32, i32> = Map.new();
  let ids: Set<i32> = Set.new();
  let wide: Map<i64, i32> = Map.new();
  let names: Map<i32, String> = Map.new();
  let b8: u8 = 7;
  let h16: u16 = 0;
  let u: u32 = 4000000000;
  let big: u64 = 1;
  let s8: i8 = 0;
  let w16: i16 = -3;
  let ws: Array<i16> = [];
  let k = \"a\";
  let s = \"\";
  let acc = 0;
  for (let i = 0; i < n; i++) {{
{body}    acc = acc % 100003;
  }}
  for (const e of pairs) {{
    acc += e.1.count + e.1.name.length + e.0.length;
  }}
  for (const e of nums) {{
    acc += e.0 * e.1;
  }}
  for (const x of ids) {{
    acc = (acc + x % 1000) % 100003;
  }}
  acc = (acc + b8.toI32() + h16.toI32() + (u % 1000).toI32() + (big % 1000).toI32() + s8.toI32() + w16.toI32() + ws.length) % 100003;
  return acc + tally % 1000 + m.size * 1000 + names.size * 100 + ids.size + wide.size * 10 + s.length + k.length;
}}
"
    )
}

/// What the interpreter gives for `f(n)`, and whether the heap is empty after.
fn interpret(module: &perasc_ir::Module, n: i32) -> (Option<i32>, bool) {
    let id = module
        .functions
        .iter()
        .position(|f| f.name == "f")
        .and_then(|i| u32::try_from(i).ok())
        .map(perasc_ir::FnId);
    let Some(id) = id else {
        return (None, false);
    };
    match interp::run_with_heap(module, id, &[Value::I32(n)]) {
        Ok((Value::I32(x), heap)) => (Some(x), heap.live.is_empty()),
        _ => (None, false),
    }
}

/// What WebAssembly gives for `f(n)`, built so, and the blocks alive after.
fn run_wasm(
    module: &perasc_ir::Module,
    allocator: perasc_runtime::Allocator,
    n: i32,
) -> (Option<i32>, i32) {
    let options = DirectOptions {
        allocator,
        // A production build wraps, a debug one checks: the programs do not
        // overflow, so both give the same.
        overflow_checks: false,
        link: false,
        ..DirectOptions::default()
    };
    let wasm = perasc_driver::emit_direct_for(
        module,
        "m.wasm",
        options,
        &Localizer::new(Language::English),
    )
    .wasm;
    let runtime = match allocator {
        perasc_runtime::Allocator::Debug => perasc_runtime::DEBUG_ALLOCATOR,
        perasc_runtime::Allocator::Tlsf => perasc_runtime::TLSF_ALLOCATOR,
    };
    let Ok(mut m) = link::instantiate_with(&wasm, runtime) else {
        return (None, -1);
    };
    let mut results = vec![wasmi::Val::I32(0)];
    let value = m
        .module
        .get_func(&m.store, "f")
        .and_then(|f| {
            f.call(&mut m.store, &[wasmi::Val::I32(n)], &mut results)
                .ok()
        })
        .and_then(|()| results.first().and_then(wasmi::Val::i32));
    let mut live = vec![wasmi::Val::I32(-1)];
    let alive = m
        .allocator
        .get_func(&m.store, "live_count")
        .and_then(|f| f.call(&mut m.store, &[], &mut live).ok())
        .and_then(|()| live.first().and_then(wasmi::Val::i32))
        .unwrap_or(-1);
    (value, alive)
}

#[test]
fn optimized_programs_compute_what_they_did_as_written() {
    let seeds = std::env::var("PERAS_DIFFERENTIAL_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(150_u64);
    for seed in 0..seeds {
        let source = program(seed);
        let written = perasc_driver::compile_ir(&source)
            .unwrap_or_else(|e| unreachable!("seed {seed} does not compile: {e:?}\n{source}"));
        let mut optimized = written.clone();
        perasc_ir::optimize(&mut optimized);
        assert!(
            perasc_ir::verify(&optimized).is_ok(),
            "seed {seed}: the optimized IR does not verify\n{source}"
        );
        for n in [0, 1, 7, 40] {
            let (oracle, clean) = interpret(&written, n);
            assert!(
                oracle.is_some() && clean,
                "seed {seed}, n {n}: the oracle failed\n{source}"
            );
            let interpreted = interpret(&optimized, n);
            assert_eq!(
                interpreted,
                (oracle, true),
                "seed {seed}, n {n}: optimized IR\n{source}"
            );
            for (module, allocator, what) in [
                (
                    &written,
                    perasc_runtime::Allocator::Debug,
                    "as written, debug",
                ),
                (
                    &optimized,
                    perasc_runtime::Allocator::Debug,
                    "optimized, debug",
                ),
                (
                    &optimized,
                    perasc_runtime::Allocator::Tlsf,
                    "optimized, TLSF",
                ),
            ] {
                assert_eq!(
                    run_wasm(module, allocator, n),
                    (oracle, 0),
                    "seed {seed}, n {n}: WebAssembly {what}\n{source}"
                );
            }
        }
    }
}

/// The program of `seed` in one file, and split into `main.peras` and
/// `lib.peras` (decision 0022): `lib.peras` exports `Pair` and `made`, and
/// each file has a private `helper` of its own, which the one file names
/// apart.
fn split(seed: u64) -> (String, String, String) {
    let whole = program_body(seed);
    let header = "struct Pair {\n  name: String;\n  count: i32;\n}\n\n\
                  fn made(i: i32): Pair {\n  return Pair(\"m\", i % 4);\n}\n\n";
    let end = "+ s.length + k.length;\n";
    assert!(whole.starts_with(header) && whole.contains(end));
    let main_fn = whole
        .trim_start_matches(header)
        .replace(end, "+ s.length + k.length + helper(n);\n");
    let one = format!(
        "struct Pair {{\n  name: String;\n  count: i32;\n}}\n\n\
         fn helperLib(i: i32): i32 {{\n  return i % 4;\n}}\n\n\
         fn made(i: i32): Pair {{\n  return Pair(\"m\", helperLib(i));\n}}\n\n\
         fn helperMain(n: i32): i32 {{\n  return n * 3;\n}}\n\n{}",
        main_fn.replace("+ helper(n);", "+ helperMain(n);")
    );
    let lib = "export struct Pair {\n  name: String;\n  count: i32;\n}\n\n\
               fn helper(i: i32): i32 {\n  return i % 4;\n}\n\n\
               export fn made(i: i32): Pair {\n  return Pair(\"m\", helper(i));\n}\n"
        .to_owned();
    let main = format!(
        "import {{ Pair, made }} from \"./lib.peras\";\n\n\
         fn helper(n: i32): i32 {{\n  return n * 3;\n}}\n\n{main_fn}"
    );
    // Under prototype p3 of decision 0044, each file that may use a pool
    // imports it.
    if std::env::var("PERAS_PROTO_POOL").is_ok_and(|v| v == "p3") {
        let import = "import { Pool, Handle } from \"peras:pool\";\n";
        return (format!("{import}{one}"), format!("{import}{main}"), lib);
    }
    (one, main, lib)
}

#[test]
fn programs_split_in_two_files_compute_what_one_file_computes() {
    let seeds = std::env::var("PERAS_DIFFERENTIAL_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(150_u64);
    let dir = std::env::temp_dir().join(format!("perasc-split-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| unreachable!("{e}"));
    for seed in 0..seeds {
        let (one, main, lib) = split(seed);
        let whole = perasc_driver::compile_ir(&one)
            .unwrap_or_else(|e| unreachable!("seed {seed}: one file: {e:?}\n{one}"));
        std::fs::write(dir.join("lib.peras"), &lib).unwrap_or_else(|e| unreachable!("{e}"));
        let root = dir.join("main.peras");
        std::fs::write(&root, &main).unwrap_or_else(|e| unreachable!("{e}"));
        let (mut parts, _) = perasc_driver::compile_graph_ir(&root, &main, true)
            .unwrap_or_else(|e| unreachable!("seed {seed}: two files: {e:?}\n{main}"));
        for n in [0, 1, 7, 40] {
            let (oracle, clean) = interpret(&whole, n);
            assert!(oracle.is_some() && clean, "seed {seed}, n {n}: one file");
            assert_eq!(
                interpret(&parts, n),
                (oracle, true),
                "seed {seed}, n {n}: two files\n{main}"
            );
        }
        perasc_ir::optimize(&mut parts);
        assert_eq!(
            run_wasm(&parts, perasc_runtime::Allocator::Tlsf, 7),
            (interpret(&whole, 7).0, 0),
            "seed {seed}: two files, optimized, TLSF\n{main}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
