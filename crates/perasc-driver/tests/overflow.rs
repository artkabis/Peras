//! An integer overflow compiled to WebAssembly in a debug build stops the
//! program through the host, and wraps in a production build (decision
//! 0011, step 5).

#[path = "fixtures/link.rs"]
mod link;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_driver::{Allocator, DirectOptions, build_direct};

const PROGRAM: &str = "
export fn add(a: i32, b: i32): i32 {
  return a + b;
}
";

fn add(checks: bool, a: i32, b: i32) -> Option<i32> {
    let built = build_direct(
        PROGRAM,
        "m.wasm",
        DirectOptions {
            allocator: Allocator::Debug,
            overflow_checks: checks,
            // Instantiated beside its allocator, as the tests' host does.
            link: false,
            ..DirectOptions::default()
        },
        &Localizer::new(Language::English),
    )
    .unwrap_or_else(|e| unreachable!("does not build: {e:?}"));
    let mut m = link::instantiate_with(&built.wasm, perasc_runtime::DEBUG_ALLOCATOR)
        .unwrap_or_else(|e| unreachable!("the modules do not link: {e}"));
    let func = m.module.get_func(&m.store, "add")?;
    let mut results = vec![wasmi::Val::I32(0)];
    func.call(
        &mut m.store,
        &[wasmi::Val::I32(a), wasmi::Val::I32(b)],
        &mut results,
    )
    .ok()?;
    // The check links the runtime; nothing is left alive by a call.
    let live = m
        .allocator
        .get_typed_func::<(), i32>(&m.store, "live_count")
        .map(|f| f.call(&mut m.store, ()));
    assert!(matches!(live, Ok(Ok(0))), "a block is left alive");
    results.first().and_then(wasmi::Val::i32)
}

#[test]
fn a_debug_build_stops_on_an_overflow_and_production_wraps() {
    assert_eq!(add(true, 2, 3), Some(5));
    assert_eq!(add(true, i32::MAX, 1), None);
    assert_eq!(add(false, i32::MAX, 1), Some(i32::MIN));
}

/// `name(a, b)` of a program built with or without the checks: `None` when
/// it stops.
fn run(
    source: &str,
    checks: bool,
    name: &str,
    args: &[wasmi::Val],
    shape: wasmi::Val,
) -> Option<i64> {
    let built = build_direct(
        source,
        "m.wasm",
        DirectOptions {
            allocator: Allocator::Debug,
            overflow_checks: checks,
            link: false,
            ..DirectOptions::default()
        },
        &Localizer::new(Language::English),
    )
    .unwrap_or_else(|e| unreachable!("does not build: {e:?}\n{source}"));
    let mut m = link::instantiate_with(&built.wasm, perasc_runtime::DEBUG_ALLOCATOR)
        .unwrap_or_else(|e| unreachable!("the modules do not link: {e}"));
    let func = m.module.get_func(&m.store, name)?;
    let mut results = vec![shape];
    func.call(&mut m.store, args, &mut results).ok()?;
    results
        .first()
        .and_then(|v| v.i64().or_else(|| v.i32().map(i64::from)))
}

/// A type, an overflowing pair of operands for `+`, `-` and `*`, then what
/// `+` and `-` wrap to.
type Case = (&'static str, [i64; 2], [i64; 2], [i64; 2], [i64; 2]);

/// Decision 0024: `+`, `-` and `*` on the other integer types stop a debug
/// build when the exact result leaves the type, and wrap in production;
/// `/` of the smallest `i8` or `i16` by -1 stops both, as it does for `i32`.
#[test]
fn the_other_integer_types_overflow_as_i32_does() {
    use wasmi::Val::{I32, I64};
    let cases: [Case; 6] = [
        // The type, an overflowing pair for `+`, `-` and `*`, then what
        // `+` and `-` wrap to; each `*` wraps to 0.
        ("i8", [127, 1], [-128, 1], [16, 16], [-128, 127]),
        (
            "i16",
            [32_767, 1],
            [-32_768, 1],
            [256, 256],
            [-32_768, 32_767],
        ),
        ("u8", [255, 1], [0, 1], [16, 16], [0, 255]),
        ("u16", [65_535, 1], [0, 1], [256, 256], [0, 65_535]),
        ("u32", [-1, 1], [0, 1], [65_536, 65_536], [0, -1]),
        ("u64", [-1, 1], [0, 1], [1 << 32, 1 << 32], [0, -1]),
    ];
    for (ty, add, sub, mul, wrapped) in cases {
        let source = format!(
            "export fn add(a: {ty}, b: {ty}): {ty} {{ return a + b; }}\n\
             export fn sub(a: {ty}, b: {ty}): {ty} {{ return a - b; }}\n\
             export fn mul(a: {ty}, b: {ty}): {ty} {{ return a * b; }}\n\
             export fn div(a: {ty}, b: {ty}): {ty} {{ return a / b; }}\n"
        );
        #[allow(clippy::cast_possible_truncation)]
        let val = |v: i64| if ty == "u64" { I64(v) } else { I32(v as i32) };
        let shape = val(0);
        for (name, [a, b], expected) in [
            ("add", add, wrapped[0]),
            ("sub", sub, wrapped[1]),
            ("mul", mul, 0),
        ] {
            let args = [val(a), val(b)];
            assert_eq!(
                run(&source, true, name, &args, shape.clone()),
                None,
                "{ty} {name} checked"
            );
            assert_eq!(
                run(&source, false, name, &args, shape.clone()),
                Some(expected),
                "{ty} {name} wrapped"
            );
        }
        // Within the range, both builds give the exact result.
        let args = [val(2), val(3)];
        for checks in [true, false] {
            assert_eq!(
                run(&source, checks, "add", &args, shape.clone()),
                Some(5),
                "{ty}"
            );
            assert_eq!(
                run(&source, checks, "mul", &args, shape.clone()),
                Some(6),
                "{ty}"
            );
        }
        if ty.starts_with('i') {
            let args = [val(sub[0]), val(-1)];
            for checks in [true, false] {
                assert_eq!(
                    run(&source, checks, "div", &args, shape.clone()),
                    None,
                    "{ty} MIN / -1"
                );
            }
        }
    }
}

/// A division by an unsigned divisor stops the program when it runs: never
/// moved out of a loop that does not turn, where it would stop a program
/// that divides nothing (decision 0024).
#[test]
fn an_unsigned_division_is_not_moved_where_it_would_run_more() {
    use wasmi::Val::I32;
    let source = "export fn g(a: u32, d: u32, n: i32): u32 {
  let s: u32 = 0;
  for (let i = 0; i < n; i++) {
    const q = a / d;
    const r = a % d;
    s = s.wrappingAdd(q).wrappingAdd(r);
  }
  return s;
}
";
    for checks in [true, false] {
        assert_eq!(
            run(source, checks, "g", &[I32(7), I32(0), I32(0)], I32(0)),
            Some(0)
        );
        assert_eq!(
            run(source, checks, "g", &[I32(7), I32(0), I32(1)], I32(0)),
            None
        );
        assert_eq!(
            run(source, checks, "g", &[I32(-1), I32(2), I32(1)], I32(0)),
            // The bits of 2³¹, which `run` reads as a signed `i32`.
            Some(i64::from(i32::MIN))
        );
    }
}
