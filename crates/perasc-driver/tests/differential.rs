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
    let choices = if depth == 0 { 29 } else { 30 };
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

fn program(seed: u64) -> String {
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

export fn f(n: i32): i32 {{
  let m: Map<String, i32> = Map.new();
  let pairs: Map<String, Pair> = Map.new();
  let nums: Map<i32, i32> = Map.new();
  let ids: Set<i32> = Set.new();
  let wide: Map<i64, i32> = Map.new();
  let names: Map<i32, String> = Map.new();
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
  return acc + m.size * 1000 + names.size * 100 + ids.size + wide.size * 10 + s.length + k.length;
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
