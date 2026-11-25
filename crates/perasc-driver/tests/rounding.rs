//! `roundEven()`, a half to the even, as a `Uint8ClampedArray` stores a
//! number (decision 0047, f3), against an oracle that is not the compiler:
//! Rust's `round_ties_even`, bit for bit, on the edges of rounding and on
//! random values; and the channel a framebuffer writes, clamped then
//! rounded, against the rule of a `Uint8ClampedArray`.

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use wasmi::{Caller, Engine, Instance, Linker, Module, Store, Val};

const SOURCE: &str = "export fn even64(x: f64): f64 {\n  return x.roundEven();\n}\n\
export fn even32(x: f32): f32 {\n  return x.roundEven();\n}\n\
export fn channel(v: f64): u8 {\n  return v.clamp(0.0, 255.0).roundEven().toU8();\n}\n";

fn instance() -> (Store<()>, Instance) {
    let module = perasc_driver::compile_ir(SOURCE)
        .unwrap_or_else(|e| unreachable!("does not compile: {e:?}"));
    let mut optimized = module;
    perasc_ir::optimize(&mut optimized);
    let wasm = perasc_driver::emit_direct_for(
        &optimized,
        "m.wasm",
        perasc_driver::DirectOptions::default(),
        &Localizer::new(Language::English),
    )
    .wasm;
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);
    let fault = |_: Caller<'_, ()>, _: i32, _: i32, _: i32, _: i32| {
        Err::<(), _>(wasmi::Error::new("peras fault"))
    };
    let bounds =
        |_: Caller<'_, ()>, _: i32, _: i32| Err::<(), _>(wasmi::Error::new("peras bounds"));
    let defined = linker
        .func_wrap("peras", "fault", fault)
        .and_then(|l| l.func_wrap(perasc_codegen_wasm::RUNTIME_MODULE, "bounds", bounds));
    assert!(defined.is_ok());
    let module = Module::new(&engine, &wasm[..]).unwrap_or_else(|e| unreachable!("{e}"));
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|e| unreachable!("{e}"));
    (store, instance)
}

fn call(store: &mut Store<()>, instance: Instance, name: &str, arg: Val, result: Val) -> Val {
    let func = instance
        .get_func(&*store, name)
        .unwrap_or_else(|| unreachable!("no export {name}"));
    let mut results = vec![result];
    func.call(store, &[arg], &mut results)
        .unwrap_or_else(|e| unreachable!("{name}: {e}"));
    results
        .pop()
        .unwrap_or_else(|| unreachable!("{name} gives nothing"))
}

/// What a `Uint8ClampedArray` stores for `v`: NaN as 0, clamped to 0 to
/// 255, rounded to the nearest, a half to the even.
fn clamped(value: f64) -> u8 {
    if value.is_nan() {
        return 0;
    }
    let bounded = value.clamp(0.0, 255.0);
    let whole = bounded.floor();
    // A whole number from 0 to 255.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let low = whole as u8;
    // Exact comparisons: a half is a half, not one near it.
    match (bounded - whole).total_cmp(&0.5) {
        std::cmp::Ordering::Greater => low + 1,
        std::cmp::Ordering::Equal if low % 2 == 1 => low + 1,
        _ => low,
    }
}

fn edges() -> Vec<f64> {
    let mut xs = vec![
        0.0,
        -0.0,
        0.5,
        -0.5,
        1.5,
        -1.5,
        2.5,
        -2.5,
        0.499_999_999_999_999_94,
        -0.499_999_999_999_999_94,
        4_503_599_627_370_495.5,
        4_503_599_627_370_496.0,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        254.5,
        255.5,
        -0.25,
        127.5,
        128.5,
    ];
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..5000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        // Halves and near halves around the bytes, and any bits at all.
        #[allow(clippy::cast_precision_loss)]
        let near = (seed % 1200) as f64 / 4.0 - 20.0;
        xs.push(near);
        xs.push(f64::from_bits(seed));
    }
    xs
}

#[test]
fn round_even_is_a_half_to_the_even_bit_for_bit() {
    let (mut store, instance) = instance();
    for x in edges() {
        let got = call(
            &mut store,
            instance,
            "even64",
            Val::F64(x.into()),
            Val::F64(0.0.into()),
        );
        let Val::F64(got) = got else {
            unreachable!("even64 gives {got:?}")
        };
        let want = x.round_ties_even();
        let same = got.to_bits() == want.to_bits() || (want.is_nan() && f64::from(got).is_nan());
        assert!(same, "even64({x:e}): {:e}, Rust {want:e}", f64::from(got));
        #[allow(clippy::cast_possible_truncation)]
        let x32 = x as f32;
        let got = call(
            &mut store,
            instance,
            "even32",
            Val::F32(x32.into()),
            Val::F32(0.0.into()),
        );
        let Val::F32(got) = got else {
            unreachable!("even32 gives {got:?}")
        };
        let want = x32.round_ties_even();
        let same = got.to_bits() == want.to_bits() || (want.is_nan() && f32::from(got).is_nan());
        assert!(same, "even32({x32:e}): {:e}, Rust {want:e}", f32::from(got));
    }
}

#[test]
fn a_channel_is_stored_as_a_uint8_clamped_array_stores_it() {
    let (mut store, instance) = instance();
    for v in edges() {
        let got = call(
            &mut store,
            instance,
            "channel",
            Val::F64(v.into()),
            Val::I32(0),
        );
        let Val::I32(got) = got else {
            unreachable!("channel gives {got:?}")
        };
        assert_eq!(got, i32::from(clamped(v)), "channel({v:e})");
    }
}
