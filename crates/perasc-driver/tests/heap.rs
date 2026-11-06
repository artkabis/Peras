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
    for name in ["alloc", "release"] {
        let func = allocator
            .get_func(&store, name)
            .ok_or_else(|| wasmi::Error::new(name))?;
        linker.define(
            perasc_codegen_wasm::RUNTIME_MODULE,
            name,
            Extern::Func(func),
        )?;
    }
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
