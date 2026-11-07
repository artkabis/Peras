//! Modules that use the heap, linked to the debug allocator and run in an
//! interpreter: what the release verifier accepts runs cleanly, and what it
//! refuses is caught by the allocator at run time, with its sites.

#[path = "fixtures/heap_ir.rs"]
mod heap_ir;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_ir::{Module, verify};
use perasc_runtime::{DEBUG_ALLOCATOR, Fault};
use wasmi::{Caller, Engine, Extern, Instance, Linker, Store};

/// The first fault reported: its kind and the sites it names, by number.
type Reported = Option<(Option<Fault>, i32, i32)>;

/// The allocator and the module, linked.
struct Linked {
    store: Store<Reported>,
    allocator: Instance,
    module: Instance,
}

fn link(module: &Module) -> Result<Linked, wasmi::Error> {
    let output = perasc_driver::emit_direct(module, "m.wasm", &Localizer::new(Language::English));
    let validated = wasmparser::Validator::new().validate_all(&output.wasm);
    assert!(validated.is_ok(), "{:?}", validated.err());
    let engine = Engine::default();
    let mut store = Store::new(&engine, None);
    let mut runtime = Linker::new(&engine);
    runtime.func_wrap(
        "peras",
        "fault",
        |mut caller: Caller<'_, Reported>, kind: i32, _ptr: i32, alloc: i32, release: i32| {
            caller
                .data_mut()
                .get_or_insert((Fault::from_kind(kind), alloc, release));
            Err::<(), _>(wasmi::Error::new("peras fault"))
        },
    )?;
    let allocator = runtime
        .instantiate_and_start(&mut store, &wasmi::Module::new(&engine, DEBUG_ALLOCATOR)?)?;
    let mut linker = Linker::new(&engine);
    for name in ["alloc", "release", "buf_new", "buf_reserve", "buf_release"] {
        let func = allocator
            .get_func(&store, name)
            .ok_or_else(|| wasmi::Error::new(name))?;
        linker.define(
            perasc_codegen_wasm::RUNTIME_MODULE,
            name,
            Extern::Func(func),
        )?;
    }
    let memory = allocator
        .get_memory(&store, "memory")
        .ok_or_else(|| wasmi::Error::new("memory"))?;
    linker.define(
        perasc_codegen_wasm::RUNTIME_MODULE,
        "memory",
        Extern::Memory(memory),
    )?;
    // An index out of bounds: JavaScript throws a `RangeError` here.
    linker.func_wrap(
        perasc_codegen_wasm::RUNTIME_MODULE,
        "bounds",
        |_: Caller<'_, Reported>, _index: i32, _len: i32| {
            Err::<(), _>(wasmi::Error::new("index out of bounds"))
        },
    )?;
    let module =
        linker.instantiate_and_start(&mut store, &wasmi::Module::new(&engine, &output.wasm)?)?;
    Ok(Linked {
        store,
        allocator,
        module,
    })
}

impl Linked {
    fn call(&mut self, name: &str, n: i32) -> Result<Option<i32>, Reported> {
        let func = self.module.get_func(&self.store, name);
        let mut results =
            vec![wasmi::Val::I32(0); func.map_or(0, |f| f.ty(&self.store).results().len())];
        let outcome = func.map(|f| f.call(&mut self.store, &[wasmi::Val::I32(n)], &mut results));
        match outcome {
            Some(Ok(())) => Ok(results.first().and_then(wasmi::Val::i32)),
            _ => Err(self.store.data_mut().take()),
        }
    }

    fn runtime(&mut self, name: &str) -> Result<i32, Reported> {
        let func = self
            .allocator
            .get_typed_func::<(), i32>(&self.store, name)
            .map_err(|_| None)?;
        func.call(&mut self.store, ())
            .map_err(|_| self.store.data_mut().take())
    }

    fn check_leaks(&mut self) -> Result<(), Reported> {
        let func = self
            .allocator
            .get_typed_func::<(), ()>(&self.store, "check_leaks")
            .map_err(|_| None)?;
        func.call(&mut self.store, ())
            .map_err(|_| self.store.data_mut().take())
    }
}

fn linked(module: &Module) -> Linked {
    link(module).unwrap_or_else(|e| unreachable!("the modules do not link: {e}"))
}

#[test]
fn a_verified_module_allocates_and_releases_through_the_allocator() {
    let module = heap_ir::work();
    assert_eq!(verify(&module), Ok(()));
    let mut m = linked(&module);
    assert_eq!(m.call("work", 1000), Ok(Some(1000)));
    assert_eq!(m.runtime("allocations"), Ok(1000));
    assert_eq!(m.runtime("live_count"), Ok(0));
    assert_eq!(m.check_leaks(), Ok(()));
}

#[test]
fn what_the_verifier_refuses_the_allocator_catches_with_its_sites() {
    let twice = heap_ir::twice();
    assert!(verify(&twice).is_err());
    let sites = perasc_codegen_wasm::compile_module(&twice).sites;
    assert_eq!(
        sites,
        ["twice (alloc 1)", "twice (release 1)", "twice (release 2)"]
    );
    // Site numbers start at 1: the second release is site 3.
    assert_eq!(
        linked(&twice).call("twice", 0),
        Err(Some((Some(Fault::DoubleRelease), 1, 3)))
    );

    let keep = heap_ir::keep();
    assert!(verify(&keep).is_err());
    let mut m = linked(&keep);
    assert_eq!(m.call("keep", 0), Ok(None));
    assert_eq!(m.call("keep", 0), Ok(None));
    // The leak names the first block's site; the count travels as the last number.
    assert_eq!(m.check_leaks(), Err(Some((Some(Fault::Leak), 1, 2))));
}

#[test]
fn the_loader_speaks_the_language_asked() {
    for (language, double) in [
        (Language::English, "is released a second time, at {release}"),
        (Language::French, "est libéré une seconde fois, à {release}"),
    ] {
        let output =
            perasc_driver::emit_direct(&heap_ir::twice(), "m.wasm", &Localizer::new(language));
        assert!(output.js.contains(double), "{language:?}:\n{}", output.js);
        assert!(output.js.contains("\"twice (release 2)\""), "{}", output.js);
        assert!(output.dts.contains("__perasHeap"), "{}", output.dts);
    }
    // A module that does not use the heap carries no allocator.
    let plain = perasc_driver::build_direct(
        "export fn f(n: i32): i32 { return n; }",
        "f.wasm",
        true,
        &Localizer::new(Language::English),
    );
    let js = plain.map(|o| o.js).unwrap_or_default();
    assert!(
        js.contains("const $imports = {};") && !js.contains("PerasMemoryError"),
        "{js}"
    );
}

/// Arrays written in Peras, compiled to WebAssembly, linked to the debug
/// allocator: the results, then nothing left alive. Any memory bug the release
/// verifier missed would be a fault of the allocator here.
#[test]
fn peras_arrays_run_on_the_debug_allocator_without_a_leak() {
    let source = "export fn squares(n: i32): i32 {\n  let xs: Array<i32> = [];\n  for (let i = 0; i < n; i++) {\n    xs.push(i * i);\n  }\n  let total = 0;\n  for (const x of xs) {\n    total += x;\n  }\n  return total;\n}\n\
         export fn churn(n: i32): i32 {\n  let xs: Array<i32> = [];\n  for (let i = 0; i < n; i++) {\n    xs = [i, i + 1, i + 2];\n    let copy = xs.copy();\n    copy.push(i);\n    if (i % 7 == 3) {\n      continue;\n    }\n    xs = grow(copy);\n  }\n  return xs.length;\n}\n\
         fn grow(xs: sink Array<i32>): Array<i32> {\n  let ys = xs;\n  ys.push(0);\n  return ys;\n}\n\
         export fn early(n: i32): i32 {\n  let kept = [1, 2];\n  for (let i = 0; i < 100; i++) {\n    let inner = [i];\n    if (i == n) {\n      return inner[0] + kept.length;\n    }\n    if (i == 50) {\n      break;\n    }\n  }\n  return -1;\n}\n\
         export fn pick(n: i32): i32 {\n  let xs = [5, 6, 7];\n  return xs[n];\n}\n";
    let module = perasc_driver::compile_ir(source)
        .unwrap_or_else(|e| unreachable!("does not compile: {e:?}"));
    let mut m = linked(&module);
    assert_eq!(m.call("squares", 100), Ok(Some(328_350)));
    // 10 000 pushes, one by one: the buffer grows by doubling.
    assert_eq!(m.call("squares", 10_000), Ok(Some(-1_724_114_088)));
    assert_eq!(m.call("churn", 1000), Ok(Some(5)));
    assert_eq!(m.call("early", 10), Ok(Some(12)));
    assert_eq!(m.call("early", 70), Ok(Some(-1)));
    assert_eq!(m.call("pick", 2), Ok(Some(7)));
    assert_eq!(m.runtime("live_count"), Ok(0));
    assert_eq!(m.check_leaks(), Ok(()));
    // Out of bounds: the program stops, through `bounds`, before any access.
    assert!(m.call("pick", 3).is_err());
}

/// Strings: literals copied from passive data segments, appended and
/// compared, on the debug allocator, without a leak.
#[test]
fn peras_strings_run_on_the_debug_allocator_without_a_leak() {
    let source = "export fn build(n: i32): i32 {\n  let s = \"\";\n  for (let i = 0; i < n; i++) {\n    s += \"héllo \";\n  }\n  const t = s + s;\n  if (t == s + s) {\n    return t.length;\n  }\n  return -1;\n}\n\
         export fn differ(n: i32): i32 {\n  let a = \"abc\";\n  if (n > 0) {\n    a = \"abd\";\n  }\n  if (a == \"abc\") {\n    return 1;\n  }\n  return 0;\n}\n";
    let module = perasc_driver::compile_ir(source)
        .unwrap_or_else(|e| unreachable!("does not compile: {e:?}"));
    let mut m = linked(&module);
    // "héllo " is 7 bytes; doubled.
    assert_eq!(m.call("build", 1000), Ok(Some(14_000)));
    assert_eq!(m.call("build", 0), Ok(Some(0)));
    assert_eq!(m.call("differ", 0), Ok(Some(1)));
    assert_eq!(m.call("differ", 1), Ok(Some(0)));
    assert_eq!(m.runtime("live_count"), Ok(0));
    assert_eq!(m.check_leaks(), Ok(()));
}
