//! `f64_rem`, the runtime's `%` on floats (decision 0021, finding F304),
//! against Rust's `%`, the same exact remainder as C's `fmod` and
//! JavaScript's `%`: the edge cases of IEEE 754, then random bit patterns,
//! every kind of float among them.

use perasc_runtime::{DEBUG_ALLOCATOR, TLSF_ALLOCATOR};
use wasmi::{Caller, Engine, Linker, Module, Store, TypedFunc};

fn rem(wasm: &[u8]) -> (Store<()>, TypedFunc<(f64, f64), f64>) {
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap(
            "peras",
            "fault",
            |_: Caller<'_, ()>, kind: i32, _: i32, _: i32, _: i32| -> Result<(), wasmi::Error> {
                Err(wasmi::Error::new(format!("memory fault {kind}")))
            },
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    linker
        .func_wrap(
            "peras",
            "float_text",
            |_: Caller<'_, ()>, _: f64, _: i32, _: i32| 0_i32,
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    linker
        .func_wrap(
            "peras",
            "float_parse",
            |_: Caller<'_, ()>, _: i32, _: i32| 0_f64,
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    let module = Module::new(&engine, wasm).unwrap_or_else(|e| unreachable!("{e}"));
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let f = instance
        .get_typed_func(&store, "f64_rem")
        .unwrap_or_else(|e| unreachable!("{e}"));
    (store, f)
}

/// The same float, NaN with NaN, `0.0` and `-0.0` apart.
fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

#[test]
fn the_remainder_of_floats_is_rusts_exactly() {
    let edges = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        3.0,
        -7.5,
        f64::MIN_POSITIVE,
        f64::MIN_POSITIVE / 3.0,
        5e-324,
        -5e-324,
        f64::MAX,
        f64::MIN,
        1e300,
        1e-300,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        0.1,
        2.0_f64.powi(52),
        2.0_f64.powi(53) + 2.0,
    ];
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let (mut store, f) = rem(wasm);
        let mut check = |x: f64, y: f64| {
            let got = f
                .call(&mut store, (x, y))
                .unwrap_or_else(|e| unreachable!("{e}"));
            assert!(same(got, x % y), "{x:e} % {y:e}: {got:e}, not {:e}", x % y);
        };
        for &x in &edges {
            for &y in &edges {
                check(x, y);
            }
        }
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for round in 0..1_000_000 {
            // Bits at random, every kind of float; and numbers near each
            // other, where the quotient is small and every bit counts.
            let x = f64::from_bits(next());
            let y = if round % 2 == 0 {
                f64::from_bits(next())
            } else {
                f64::from_bits(x.to_bits() ^ (next() & 0x000f_ffff_ffff_ffff)) / 3.0
            };
            check(x, y);
        }
    }
}
