//! Vector loops give exactly what the loop as written gives: each program is
//! built optimized, which proves its bounds checks away and lets its loops run
//! several turns at once, and unoptimized, which keeps them scalar; both run in
//! wasmi on arrays of every length from 0 to 19, around the four lanes.

#[path = "fixtures/link.rs"]
mod link;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_driver::{Allocator, DirectOptions, build_direct};
use wasmi::Val;

fn build(source: &str, optimize: bool) -> Vec<u8> {
    build_direct(
        source,
        "m.wasm",
        DirectOptions {
            optimize,
            allocator: Allocator::Debug,
        },
        &Localizer::new(Language::English),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .wasm
}

/// Whether the module uses a 128-bit vector instruction.
fn has_vectors(wasm: &[u8]) -> bool {
    wasmparser::Parser::new(0).parse_all(wasm).any(|payload| {
        let Ok(wasmparser::Payload::CodeSectionEntry(body)) = payload else {
            return false;
        };
        body.get_operators_reader().is_ok_and(|ops| {
            ops.into_iter()
                .any(|op| matches!(op, Ok(wasmparser::Operator::V128Load { .. })))
        })
    })
}

/// The result of `name(n)`, its bits for a float; `None` on a trap. Checks
/// that the call leaves no block alive.
fn call(wasm: &[u8], name: &str, n: i32) -> Option<u64> {
    let mut linked = link::instantiate(wasm).unwrap_or_else(|e| unreachable!("{e}"));
    let func = linked.module.get_func(&linked.store, name)?;
    let mut results = [Val::I32(0)];
    func.call(&mut linked.store, &[Val::I32(n)], &mut results)
        .ok()?;
    // Nothing is left alive.
    let live = linked
        .allocator
        .get_typed_func::<(), i32>(&linked.store, "live_count")
        .and_then(|f| f.call(&mut linked.store, ()));
    assert!(matches!(live, Ok(0)), "{name}({n}): {live:?}");
    Some(match results[0] {
        Val::I32(v) => u64::from(v.cast_unsigned()),
        Val::I64(v) => v.cast_unsigned(),
        Val::F32(v) => u64::from(f32::from(v).to_bits()),
        Val::F64(v) => f64::from(v).to_bits(),
        _ => return None,
    })
}

fn same_both_ways(source: &str, name: &str) {
    let (vector, scalar) = (build(source, true), build(source, false));
    assert!(has_vectors(&vector), "{name}: no vector loop");
    assert!(
        !has_vectors(&scalar),
        "{name}: vectors without the optimizer"
    );
    for n in 0..20 {
        assert_eq!(
            call(&vector, name, n),
            call(&scalar, name, n),
            "{name}({n})"
        );
    }
}

#[test]
fn integer_lanes_shift_wrap_and_reduce_as_the_scalar_loop() {
    // `last` is read after the loop: the last turn is always the scalar one.
    let source = "export fn ints(n: i32): i32 {
  let xs: Array<i32> = [];
  let ys: Array<i32> = [];
  for (let k = 0; k < n; k++) {
    xs.push(k * 40503 - 7);
    ys.push(k * k * 99991 - 3);
  }
  let total = 5;
  let bits = -1;
  let last = 0;
  const shift = n % 5;
  for (let i = 0; i < xs.length; i++) {
    const a = xs[i] * 77 + (ys[i] >> shift);
    last = a ^ 21845;
    xs[i] = (a << 2) | (last & 255);
    total = total + a;
    bits = bits & ~last;
  }
  let h = 0;
  for (const x of xs) {
    h = h * 31 + x;
  }
  return h + total * 7 + bits + last;
}
";
    same_both_ways(source, "ints");
}

#[test]
fn float_lanes_compute_each_element_exactly() {
    let source = "export fn floats(n: i32): f32 {
  let a: Array<f32> = [];
  let b: Array<f32> = [];
  for (let k = 0; k < n; k++) {
    a.push(k.toF32() * 0.37);
    b.push(1.5 - k.toF32());
  }
  const g: f32 = 0.25;
  for (let i = 0; i < a.length; i++) {
    a[i] = (a[i] * g - b[i]) / (b[i] + 3.0) + (a[i] * a[i]).sqrt();
  }
  let s: f32 = 0.0;
  for (const x of a) {
    s = s * 0.5 + x;
  }
  return s;
}
export fn doubles(n: i32): f64 {
  let a: Array<f64> = [];
  for (let k = 0; k < n; k++) {
    a.push(k.toF64() * 1.1 - 4.0);
  }
  let last = 0.0;
  for (let i = 0; i < a.length; i++) {
    last = -(a[i] * a[i]) / 3.0;
    a[i] = last + a[i];
  }
  let s = last;
  for (const x of a) {
    s = s * 0.5 + x;
  }
  return s;
}
";
    same_both_ways(source, "floats");
    same_both_ways(source, "doubles");
}
