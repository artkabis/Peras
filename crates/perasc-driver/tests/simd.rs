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
            simd: true,
            threads: false,
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
    let mut linked = link::instantiate_with(wasm, perasc_runtime::DEBUG_ALLOCATOR)
        .unwrap_or_else(|e| unreachable!("{e}"));
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

#[test]
fn branches_run_in_every_lane_and_keep_their_results_where_they_hold() {
    // The particles of the browser benchmark: a bounce writes another array
    // and recomputes a local, in the lanes that bounce only.
    let source = "export fn bounce(n: i32): f64 {
  let xs: Array<f64> = [];
  let vs: Array<f64> = [];
  for (let k = 0; k < n; k++) {
    xs.push((k * 7 % 10).toF64() * 0.11);
    vs.push((k % 3).toF64() - 1.0);
  }
  const dt = 0.3;
  for (let i = 0; i < xs.length; i++) {
    const v = vs[i];
    let x = xs[i] + v * dt;
    if (x < 0.0 || x > 1.0) {
      vs[i] = -v;
      x = xs[i] - v * dt;
    }
    xs[i] = x;
  }
  let s = 0.0;
  for (let i = 0; i < xs.length; i++) {
    s = s * 0.5 + xs[i] + vs[i] * 3.0;
  }
  return s;
}
export fn nested(n: i32): i32 {
  let xs: Array<i32> = [];
  for (let k = 0; k < n; k++) {
    xs.push(k * 37 % 23 - 11);
  }
  let total = 0;
  for (let i = 0; i < xs.length; i++) {
    let y = xs[i] * 3;
    if (y > 0 && !(y > 20)) {
      y = y - 5;
      if (y < 2) {
        y = 1000;
      }
    } else {
      xs[i] = -y;
      y = y + 1;
    }
    xs[i] = xs[i] + y;
    total = total ^ y;
  }
  let h = total;
  for (const x of xs) {
    h = h * 31 + x;
  }
  return h;
}
";
    same_both_ways(source, "bounce");
    same_both_ways(source, "nested");
}

#[test]
fn a_local_first_assigned_in_a_branch_keeps_the_loop_scalar() {
    // `last` keeps, in the turns that skip the branch, the value of an
    // earlier turn: lanes would not see it.
    let source = "export fn carried(n: i32): i32 {
  let xs: Array<i32> = [];
  for (let k = 0; k < n; k++) {
    xs.push(k * 5 % 7);
  }
  let last = -1;
  for (let i = 0; i < xs.length; i++) {
    if (xs[i] > 3) {
      last = xs[i];
    }
    xs[i] = xs[i] + last;
  }
  let h = 0;
  for (const x of xs) {
    h = h * 31 + x;
  }
  return h;
}
";
    let (vector, scalar) = (build(source, true), build(source, false));
    for n in 0..20 {
        assert_eq!(
            call(&vector, "carried", n),
            call(&scalar, "carried", n),
            "carried({n})"
        );
    }
}

/// Whether a module uses any SIMD instruction: by its name in wasmparser,
/// `V128…` or a shape of lanes, `I8x16…` to `F64x2…`.
fn any_simd(wasm: &[u8]) -> bool {
    const SHAPES: [&str; 7] = ["V128", "I8x16", "I16x8", "I32x4", "I64x2", "F32x4", "F64x2"];
    wasmparser::Parser::new(0).parse_all(wasm).any(|payload| {
        let Ok(wasmparser::Payload::CodeSectionEntry(body)) = payload else {
            return false;
        };
        body.get_operators_reader().is_ok_and(|ops| {
            ops.into_iter().any(|op| {
                op.is_ok_and(|op| {
                    let name = format!("{op:?}");
                    SHAPES.iter().any(|shape| name.starts_with(shape))
                })
            })
        })
    })
}

#[test]
fn simd_off_gives_modules_and_runtimes_without_it_and_a_loader_without_the_check() {
    let source = "export fn scale(xs: inout Array<f32>, k: f32): void {
  for (let i = 0; i < xs.length; i++) {
    xs[i] = xs[i] * k;
  }
}
";
    let build = |simd: bool| {
        perasc_driver::build_direct(
            source,
            "m.wasm",
            DirectOptions {
                optimize: true,
                allocator: Allocator::Tlsf,
                simd,
                threads: false,
            },
            &Localizer::new(Language::English),
        )
        .unwrap_or_else(|e| unreachable!("{e:?}"))
    };
    let (on, off) = (build(true), build(false));
    assert!(any_simd(&on.wasm) && !any_simd(&off.wasm));
    for allocator in [Allocator::Debug, Allocator::Tlsf] {
        assert!(any_simd(allocator.runtime(true)), "{allocator:?} with SIMD");
        assert!(!any_simd(allocator.runtime(false)), "{allocator:?} without");
    }
    // The loader checks for SIMD before loading, only when it is used.
    assert!(on.js.contains("WebAssembly.validate"), "{}", on.js);
    assert!(on.js.contains("--simd off"), "{}", on.js);
    assert!(!off.js.contains("WebAssembly.validate"), "{}", off.js);
}
