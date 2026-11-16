//! The integer types other than `i32` and `i64` (decision 0024), against an
//! oracle that is not the compiler: Rust's own arithmetic on each type. For
//! each type, a module exports one function per operation and conversion;
//! random operands, the edges among them, go through the compiled module,
//! run by `wasmi`, and through Rust, and the results must be equal bit for
//! bit. Where Rust panics, on a division by zero or `MIN / -1`, the module
//! must stop.

use std::fmt::Write as _;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use wasmi::{Caller, Engine, Instance, Linker, Module, Store, Val};

/// Cases per type: many in a release build, fewer in the debug build the CI
/// runs, where `wasmi` is not optimized (finding M022).
const CASES: usize = if cfg!(debug_assertions) { 400 } else { 10_000 };

/// The module of the operations on `ty`, a Peras type name.
fn source(ty: &str) -> String {
    let mut out = String::new();
    for (name, body) in [
        ("add", "a.wrappingAdd(b)"),
        ("sub", "a.wrappingSub(b)"),
        ("mul", "a.wrappingMul(b)"),
        ("div", "a / b"),
        ("rem", "a % b"),
        ("shl", "a << b"),
        ("shr", "a >> b"),
        ("and", "a & b"),
        ("or", "a | b"),
        ("xor", "a ^ b"),
        ("min", "a.min(b)"),
        ("max", "a.max(b)"),
    ] {
        let _ = write!(
            out,
            "export fn {name}(a: {ty}, b: {ty}): {ty} {{\n  return {body};\n}}\n"
        );
    }
    for (name, op) in [
        ("lt", "<"),
        ("le", "<="),
        ("gt", ">"),
        ("ge", ">="),
        ("eq", "=="),
    ] {
        let _ = write!(
            out,
            "export fn {name}(a: {ty}, b: {ty}): bool {{\n  return a {op} b;\n}}\n"
        );
    }
    for target in [
        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
    ] {
        let method = format!("to{}{}", target[..1].to_uppercase(), &target[1..]);
        let _ = write!(
            out,
            "export fn to_{target}(a: {ty}): {target} {{\n  return a.{method}();\n}}\n"
        );
    }
    let method = format!("to{}{}", ty[..1].to_uppercase(), &ty[1..]);
    for from in ["f32", "f64"] {
        let _ = write!(
            out,
            "export fn from_{from}(x: {from}): {ty} {{\n  return x.{method}();\n}}\n"
        );
    }
    out
}

/// The compiled module of `ty`, instantiated.
fn instance(ty: &str) -> (Store<()>, Instance) {
    let source = source(ty);
    let module = perasc_driver::compile_ir(&source)
        .unwrap_or_else(|e| unreachable!("{ty} does not compile: {e:?}\n{source}"));
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
        .and_then(|l| {
            l.func_wrap(
                "peras",
                "float_text",
                |_: Caller<'_, ()>, _: f64, _: i32, _: i32| 0,
            )
        })
        .and_then(|l| {
            l.func_wrap(
                "peras",
                "float_parse",
                |_: Caller<'_, ()>, _: i32, _: i32| 0.0,
            )
        })
        .and_then(|l| l.func_wrap(perasc_codegen_wasm::RUNTIME_MODULE, "bounds", bounds));
    assert!(defined.is_ok());
    let module = Module::new(&engine, &wasm[..]).unwrap_or_else(|e| unreachable!("{e}"));
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|e| unreachable!("{ty}: {e}"));
    (store, instance)
}

/// Calls an export; `None` when it traps.
fn call(
    store: &mut Store<()>,
    instance: Instance,
    name: &str,
    args: &[Val],
    result: Val,
) -> Option<Val> {
    let func = instance.get_func(&*store, name)?;
    let mut results = vec![result];
    func.call(store, args, &mut results).ok()?;
    results.pop()
}

/// A small generator, so that a failure can be replayed.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// A value as WebAssembly holds it: an `i32` sign- or zero-extended, an
/// `i64`, or a float.
trait Wasm: Copy {
    fn val(self) -> Val;
}

macro_rules! held_as_i32 {
    ($($t:ty),*) => {$(
        impl Wasm for $t {
            #[allow(clippy::cast_possible_wrap, clippy::cast_lossless)]
            fn val(self) -> Val {
                Val::I32(self as i32)
            }
        }
    )*};
}
held_as_i32!(i8, i16, i32, u8, u16, u32);

impl Wasm for i64 {
    fn val(self) -> Val {
        Val::I64(self)
    }
}

impl Wasm for u64 {
    #[allow(clippy::cast_possible_wrap)]
    fn val(self) -> Val {
        Val::I64(self as i64)
    }
}

impl Wasm for f32 {
    fn val(self) -> Val {
        Val::F32(self.into())
    }
}

impl Wasm for f64 {
    fn val(self) -> Val {
        Val::F64(self.into())
    }
}

impl Wasm for bool {
    fn val(self) -> Val {
        Val::I32(i32::from(self))
    }
}

/// The bits of a value, to compare: floats by their bits, `NaN` as one.
fn bits(v: &Val) -> Option<u64> {
    #[allow(clippy::cast_sign_loss)]
    Some(match v {
        Val::I32(x) => u64::from(*x as u32),
        Val::I64(x) => *x as u64,
        Val::F32(x) if f32::from(*x).is_nan() => 1 << 40,
        Val::F64(x) if f64::from(*x).is_nan() => 1 << 41,
        Val::F32(x) => u64::from(f32::from(*x).to_bits()),
        Val::F64(x) => f64::from(*x).to_bits(),
        _ => return None,
    })
}

/// Checks one export against Rust's result, `None` for a trap. `shape` is a
/// value of the result's type, which a trap leaves unknown.
fn check(
    store: &mut Store<()>,
    instance: Instance,
    (name, args): (&str, &[Val]),
    shape: Val,
    expected: Option<&Val>,
    what: &str,
) {
    let found = call(store, instance, name, args, shape);
    assert_eq!(
        found.as_ref().and_then(bits),
        expected.and_then(bits),
        "{what}: {name}({args:?})"
    );
}

/// Checks an export that cannot trap against Rust's result.
fn same(
    store: &mut Store<()>,
    instance: Instance,
    call: (&str, &[Val]),
    expected: Val,
    what: &str,
) {
    let want = bits(&expected);
    let found = self::call(store, instance, call.0, call.1, expected);
    assert_eq!(
        found.as_ref().and_then(bits),
        want,
        "{what}: {}({:?})",
        call.0,
        call.1
    );
}

macro_rules! oracle {
    ($test:ident, $t:ty, $name:literal) => {
        #[test]
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            clippy::cast_lossless
        )]
        fn $test() {
            let (mut store, instance) = instance($name);
            let edges: [$t; 5] = [0, 1, <$t>::MIN, <$t>::MAX, (0 as $t).wrapping_sub(1)];
            let mut r = XorShift(0x9E37_79B9_7F4A_7C15 ^ u64::from(<$t>::BITS));
            let pick = |r: &mut XorShift| -> $t {
                let n = r.next();
                if n % 4 == 0 {
                    edges[(n >> 8) as usize % edges.len()]
                } else {
                    (n >> 3) as $t
                }
            };
            let zero: $t = 0;
            for case in 0..CASES {
                let (a, b) = (pick(&mut r), pick(&mut r));
                let what = format!("{} case {case}", $name);
                let args = [a.val(), b.val()];
                let s = &mut store;
                same(s, instance, ("add", &args), a.wrapping_add(b).val(), &what);
                same(s, instance, ("sub", &args), a.wrapping_sub(b).val(), &what);
                same(s, instance, ("mul", &args), a.wrapping_mul(b).val(), &what);
                check(
                    s,
                    instance,
                    ("div", &args),
                    zero.val(),
                    a.checked_div(b).map(Wasm::val).as_ref(),
                    &what,
                );
                // `%` traps on zero only: the smallest by -1 gives 0, as
                // `i32.rem_s` does.
                let rem = if b == 0 {
                    None
                } else {
                    Some(a.wrapping_rem(b).val())
                };
                check(s, instance, ("rem", &args), zero.val(), rem.as_ref(), &what);
                same(
                    s,
                    instance,
                    ("shl", &args),
                    a.wrapping_shl(b as u32).val(),
                    &what,
                );
                same(
                    s,
                    instance,
                    ("shr", &args),
                    a.wrapping_shr(b as u32).val(),
                    &what,
                );
                same(s, instance, ("and", &args), (a & b).val(), &what);
                same(s, instance, ("or", &args), (a | b).val(), &what);
                same(s, instance, ("xor", &args), (a ^ b).val(), &what);
                same(s, instance, ("min", &args), a.min(b).val(), &what);
                same(s, instance, ("max", &args), a.max(b).val(), &what);
                same(s, instance, ("lt", &args), (a < b).val(), &what);
                same(s, instance, ("le", &args), (a <= b).val(), &what);
                same(s, instance, ("gt", &args), (a > b).val(), &what);
                same(s, instance, ("ge", &args), (a >= b).val(), &what);
                same(s, instance, ("eq", &args), (a == b).val(), &what);
                let one = [a.val()];
                same(s, instance, ("to_i8", &one), (a as i8).val(), &what);
                same(s, instance, ("to_i16", &one), (a as i16).val(), &what);
                same(s, instance, ("to_i32", &one), (a as i32).val(), &what);
                same(s, instance, ("to_i64", &one), (a as i64).val(), &what);
                same(s, instance, ("to_u8", &one), (a as u8).val(), &what);
                same(s, instance, ("to_u16", &one), (a as u16).val(), &what);
                same(s, instance, ("to_u32", &one), (a as u32).val(), &what);
                same(s, instance, ("to_u64", &one), (a as u64).val(), &what);
                same(s, instance, ("to_f32", &one), (a as f32).val(), &what);
                same(s, instance, ("to_f64", &one), (a as f64).val(), &what);
                // Floats near the type's range and beyond, `NaN` and the
                // infinities among them.
                let x = match r.next() % 4 {
                    0 => f64::from_bits(r.next()),
                    1 => (r.next() as $t) as f64 + 0.5,
                    2 => <$t>::MAX as f64 * 2.0 - (r.next() % 1000) as f64,
                    _ => <$t>::MIN as f64 - (r.next() % 1000) as f64 - 0.25,
                };
                same(
                    s,
                    instance,
                    ("from_f64", &[x.val()]),
                    (x as $t).val(),
                    &what,
                );
                let y = x as f32;
                same(
                    s,
                    instance,
                    ("from_f32", &[y.val()]),
                    (y as $t).val(),
                    &what,
                );
            }
        }
    };
}

oracle!(i8_against_rust, i8, "i8");
oracle!(i16_against_rust, i16, "i16");
oracle!(u8_against_rust, u8, "u8");
oracle!(u16_against_rust, u16, "u16");
oracle!(u32_against_rust, u32, "u32");
oracle!(u64_against_rust, u64, "u64");
