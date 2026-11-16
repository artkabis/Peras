//! Closures in owned modules (decision 0026): what an arrow function captures
//! is copied into its environment's `i64` slots, numbers by their bits, and
//! read back unchanged; the reinterpretations of the IR give what Rust's
//! `to_bits` and `from_bits` give, in the interpreter and in WebAssembly; an
//! environment copied many times is freed once, by the last who holds it.

#[path = "fixtures/link.rs"]
mod link;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_ir::interp::{self, Value};
use perasc_ir::{Expr, ExprKind, FnId, Function, Local, LocalId, Module, Stmt, Ty, UnOp};

fn emit(module: &Module) -> link::Linked {
    let output = perasc_driver::emit_direct(
        module,
        "m.wasm",
        perasc_runtime::Allocator::Debug,
        &Localizer::new(Language::English),
    );
    link::instantiate_with(&output.wasm, perasc_runtime::DEBUG_ALLOCATOR)
        .unwrap_or_else(|e| unreachable!("the modules do not link: {e}"))
}

/// A call of an exported function with one argument and one result, in
/// WebAssembly.
fn call(m: &mut link::Linked, name: &str, arg: wasmi::Val) -> wasmi::Val {
    let func = m.module.get_func(&m.store, name);
    let mut results = vec![wasmi::Val::I32(0)];
    let done = func.map(|f| f.call(&mut m.store, &[arg], &mut results));
    assert!(matches!(done, Some(Ok(()))), "{name} failed: {done:?}");
    results.first().cloned().unwrap_or(wasmi::Val::I32(0))
}

/// `name(x: from): to`, returning `x` read as `to`, the same bits.
fn bits_fn(name: &str, from: Ty, to: Ty) -> Function {
    Function {
        name: name.to_owned(),
        exported: true,
        job: false,
        param_count: 1,
        locals: vec![Local {
            name: "x".to_owned(),
            ty: from,
        }],
        ret: to,
        more: Vec::new(),
        shape: None,
        body: vec![Stmt::Return(Some(Expr {
            kind: ExprKind::Unary(
                UnOp::Bits,
                Box::new(Expr {
                    kind: ExprKind::Local(LocalId(0)),
                    ty: from,
                }),
            ),
            ty: to,
        }))],
    }
}

const F64_EDGES: [f64; 12] = [
    0.0,
    -0.0,
    1.0,
    -1.5,
    f64::MIN_POSITIVE,
    5e-324,
    f64::MAX,
    f64::MIN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NAN,
    f64::EPSILON,
];

const F32_EDGES: [f32; 12] = [
    0.0,
    -0.0,
    1.0,
    -1.5,
    f32::MIN_POSITIVE,
    1e-45,
    f32::MAX,
    f32::MIN,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    f32::EPSILON,
];

#[test]
fn reinterpretations_match_rusts_bits() {
    let module = Module {
        functions: vec![
            bits_fn("f64_bits", Ty::F64, Ty::I64),
            bits_fn("bits_f64", Ty::I64, Ty::F64),
            bits_fn("f32_bits", Ty::F32, Ty::I32),
            bits_fn("bits_f32", Ty::I32, Ty::F32),
        ],
        globals: Vec::new(),
        data: Vec::new(),
    };
    assert_eq!(perasc_ir::verify(&module), Ok(()));
    let mut m = emit(&module);
    // A NaN with a payload, and a signalling one, keep their bits too.
    let payloads = [
        f64::from_bits(0x7ff8_0000_0000_0001),
        f64::from_bits(0x7ff0_0000_0000_0001),
    ];
    for x in F64_EDGES.into_iter().chain(payloads) {
        let bits = x.to_bits().cast_signed();
        let interp = interp::run(&module, FnId(0), &[Value::F64(x)]);
        assert!(
            matches!(interp, Ok(Value::I64(b)) if b == bits),
            "{x}: {interp:?}"
        );
        let wasm = call(&mut m, "f64_bits", wasmi::Val::F64(x.into()));
        assert_eq!(wasm.i64(), Some(bits), "{x}");
        let back = interp::run(&module, FnId(1), &[Value::I64(bits)]);
        assert!(
            matches!(back, Ok(Value::F64(y)) if y.to_bits() == x.to_bits()),
            "{x}: {back:?}"
        );
        let wasm = call(&mut m, "bits_f64", wasmi::Val::I64(bits));
        assert_eq!(
            wasm.f64().map(wasmi::F64::to_bits),
            Some(x.to_bits()),
            "{x}"
        );
    }
    for x in F32_EDGES {
        let bits = x.to_bits().cast_signed();
        let interp = interp::run(&module, FnId(2), &[Value::F32(x)]);
        assert!(
            matches!(interp, Ok(Value::I32(b)) if b == bits),
            "{x}: {interp:?}"
        );
        let wasm = call(&mut m, "f32_bits", wasmi::Val::F32(x.into()));
        assert_eq!(wasm.i32(), Some(bits), "{x}");
        let back = interp::run(&module, FnId(3), &[Value::I32(bits)]);
        assert!(
            matches!(back, Ok(Value::F32(y)) if y.to_bits() == x.to_bits()),
            "{x}: {back:?}"
        );
        let wasm = call(&mut m, "bits_f32", wasmi::Val::I32(bits));
        assert_eq!(
            wasm.f32().map(wasmi::F32::to_bits),
            Some(x.to_bits()),
            "{x}"
        );
    }
}

const KEPT: &str = "
fn apply(f: (i32) => i32, x: i32): i32 {
  return f(x);
}

export fn keptF64(v: f64): f64 {
  const f = (): f64 => v;
  return f();
}

export fn keptF32(v: f32): f32 {
  const f = (): f32 => v;
  return f();
}

export fn keptI64(v: i64): i64 {
  const f = (): i64 => v;
  return f();
}

export fn keptI32(v: i32): i32 {
  const f = (x: i32): i32 => x + v;
  return apply(f, 0);
}

export fn copies(k: i32): i32 {
  const f = (x: i32): i32 => x + k;
  let all: Array<(i32) => i32> = [];
  for (let i = 0; i < 100; i++) {
    all.push(f);
  }
  let total = 0;
  for (const g of all) {
    total += g(1);
  }
  return total;
}
";

fn position(module: &Module, name: &str) -> FnId {
    let at = module.functions.iter().position(|f| f.name == name);
    FnId(at.and_then(|p| u32::try_from(p).ok()).unwrap_or(u32::MAX))
}

#[test]
fn captured_numbers_come_back_bit_for_bit() {
    let module =
        perasc_driver::compile_ir(KEPT).unwrap_or_else(|e| unreachable!("does not compile: {e:?}"));
    let mut m = emit(&module);
    let payload = f64::from_bits(0x7ff8_0000_0000_0001);
    for x in F64_EDGES.into_iter().chain([payload]) {
        let interp = interp::run(&module, position(&module, "keptF64"), &[Value::F64(x)]);
        assert!(
            matches!(interp, Ok(Value::F64(y)) if y.to_bits() == x.to_bits()),
            "{x}: {interp:?}"
        );
        let wasm = call(&mut m, "keptF64", wasmi::Val::F64(x.into()));
        assert_eq!(
            wasm.f64().map(wasmi::F64::to_bits),
            Some(x.to_bits()),
            "{x}"
        );
    }
    for x in F32_EDGES {
        let wasm = call(&mut m, "keptF32", wasmi::Val::F32(x.into()));
        assert_eq!(
            wasm.f32().map(wasmi::F32::to_bits),
            Some(x.to_bits()),
            "{x}"
        );
    }
    for x in [i64::MIN, -1, 0, 1, i64::MAX] {
        let wasm = call(&mut m, "keptI64", wasmi::Val::I64(x));
        assert_eq!(wasm.i64(), Some(x));
    }
    for x in [i32::MIN, -1, 0, i32::MAX] {
        let wasm = call(&mut m, "keptI32", wasmi::Val::I32(x));
        assert_eq!(wasm.i32(), Some(x));
    }
    // Nothing made by these calls is left alive.
    let live = m
        .allocator
        .get_typed_func::<(), i32>(&m.store, "live_count")
        .map(|f| f.call(&mut m.store, ()));
    assert!(matches!(live, Ok(Ok(0))), "a block is left alive: {live:?}");
}

#[test]
fn an_environment_copied_is_freed_once() {
    let module =
        perasc_driver::compile_ir(KEPT).unwrap_or_else(|e| unreachable!("does not compile: {e:?}"));
    let copies = position(&module, "copies");
    let run = interp::run_with_heap(&module, copies, &[Value::I32(4)]);
    assert!(
        matches!(&run, Ok((Value::I32(500), report)) if report.live.is_empty()),
        "{run:?}"
    );
    let mut m = emit(&module);
    assert_eq!(call(&mut m, "copies", wasmi::Val::I32(4)).i32(), Some(500));
    let live = m
        .allocator
        .get_typed_func::<(), i32>(&m.store, "live_count")
        .map(|f| f.call(&mut m.store, ()));
    assert!(matches!(live, Ok(Ok(0))), "a block is left alive: {live:?}");
}
