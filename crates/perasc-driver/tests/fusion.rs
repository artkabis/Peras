//! `xs.filter((x) => …).length` counts the elements kept, with no array
//! between (decision 0004, fusion): run in wasmi on the debug allocator,
//! each result checked against Rust. Counting allocates what making the
//! array alone does: no array of the elements kept, no flags.

#[path = "fixtures/link.rs"]
mod link;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_driver::{Allocator, DirectOptions, build_direct};
use wasmi::Val;

const SOURCE: &str = r#"struct Item {
  name: String;
  price: i32;
}

fn sample(n: i32): String {
  return " ab  c déjà ".repeat(n) + "vu";
}

fn numbers(n: i32): Array<i32> {
  let xs: Array<i32> = [];
  for (let i = 0; i < n; i++) {
    xs.push((i * 7919) % 101 - 50);
  }
  return xs;
}

export fn words(n: i32): i32 {
  return sample(n).split(" ").filter((w) => w.length > 0).length;
}

export fn parts(n: i32): i32 {
  return sample(n).split(" ").length;
}

export fn positives(n: i32): i32 {
  const xs = numbers(n);
  const limit = n % 5;
  return xs.filter((x) => x > limit).length * 1000 + xs.length;
}

export fn positives_kept(n: i32): i32 {
  const xs = numbers(n);
  const limit = n % 5;
  const kept = xs.filter((x) => x > limit);
  return kept.length * 1000 + xs.length;
}

export fn made(n: i32): i32 {
  const xs = numbers(n);
  return xs.length;
}

export fn items(n: i32): i32 {
  let all: Array<Item> = [];
  for (let i = 0; i < n; i++) {
    all.push(Item("item" + i.toString(), (i * 37) % 100));
  }
  return (all.filter((it) => it.price < 50 && it.name.endsWith("1"))).length;
}

export fn nested(n: i32): i32 {
  let total = 0;
  for (let k = 0; k < 3; k++) {
    total += numbers(n).filter((x) => x % (k + 2) == 0).length + k;
  }
  return total;
}
"#;

/// The integers `numbers(n)` gives.
fn numbers(n: i32) -> Vec<i32> {
    (0..n).map(|i| (i * 7919) % 101 - 50).collect()
}

/// What each function gives, computed in Rust.
fn expected(name: &str, n: i32) -> i32 {
    let count = |v: usize| i32::try_from(v).unwrap_or(i32::MAX);
    let text = format!(
        "{}vu",
        " ab  c déjà ".repeat(usize::try_from(n).unwrap_or(0))
    );
    match name {
        "words" => count(text.split(' ').filter(|w| !w.is_empty()).count()),
        "parts" => count(text.split(' ').count()),
        "made" => n,
        "positives" | "positives_kept" => {
            let xs = numbers(n);
            let limit = n % 5;
            count(xs.iter().filter(|&&x| x > limit).count()) * 1000 + count(xs.len())
        }
        "items" => count(
            (0..n)
                .filter(|i| (i * 37) % 100 < 50 && format!("item{i}").ends_with('1'))
                .count(),
        ),
        "nested" => (0..3)
            .map(|k| count(numbers(n).iter().filter(|&&x| x % (k + 2) == 0).count()) + k)
            .sum(),
        _ => unreachable!("{name}"),
    }
}

/// Calls `name(n)` in a fresh instance: its result, and the blocks it
/// allocated.
fn run(wasm: &[u8], name: &str, n: i32) -> (i32, i32) {
    let mut linked = link::instantiate_with(wasm, perasc_runtime::DEBUG_ALLOCATOR)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let func = linked
        .module
        .get_func(&linked.store, name)
        .unwrap_or_else(|| unreachable!("{name}"));
    let mut results = [Val::I32(0)];
    let called = func.call(&mut linked.store, &[Val::I32(n)], &mut results);
    assert!(called.is_ok(), "{name}({n}): {called:?}");
    let mut ask = |what: &str| {
        linked
            .allocator
            .get_typed_func::<(), i32>(&linked.store, what)
            .and_then(|f| f.call(&mut linked.store, ()))
            .unwrap_or_else(|e| unreachable!("{what}: {e}"))
    };
    assert_eq!(ask("live_count"), 0, "{name}({n}) leaves blocks alive");
    (results[0].i32().unwrap_or(i32::MIN), ask("allocations"))
}

#[test]
fn a_filter_whose_length_is_read_counts_and_allocates_no_array() {
    let wasm = build_direct(
        SOURCE,
        "m.wasm",
        DirectOptions {
            optimize: true,
            allocator: Allocator::Debug,
            simd: true,
            threads: false,
            // The code a production build runs: no overflow checks.
            overflow_checks: false,
        },
        &Localizer::new(Language::English),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .wasm;
    for n in [0, 1, 2, 7, 40, 300] {
        for name in ["words", "positives", "items", "nested"] {
            let (value, _) = run(&wasm, name, n);
            assert_eq!(value, expected(name, n), "{name}({n})");
        }
        // Counting allocates what making the numbers alone does; the same
        // filter, its array kept in a variable, gives the same count with at
        // least one block more, the array the fused loop never makes.
        let (a, fused) = run(&wasm, "positives", n);
        let (b, kept) = run(&wasm, "positives_kept", n);
        let (_, made) = run(&wasm, "made", n);
        assert_eq!(a, b, "positives({n})");
        assert_eq!(fused, made, "positives({n}) allocations");
        assert!(kept > fused, "positives_kept({n}) allocations");
        // Counting the words allocates what splitting alone does: the text
        // and its block of views, no flags and no array of words.
        let (_, words) = run(&wasm, "words", n);
        let (parts, split) = run(&wasm, "parts", n);
        assert_eq!(parts, expected("parts", n));
        assert_eq!(words, split, "words({n}) allocations");
    }
}
