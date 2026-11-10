//! Maps and sets compiled: random operations run as WebAssembly on the debug
//! allocator give what the standard library's maps give, without a leak. The
//! IR interpreter runs the same program in `perasc-ir`'s `map_tests`.

use perasc_ir::Module;

#[path = "fixtures/maps.rs"]
mod maps;

use maps::{CASES, PROGRAM};

#[path = "fixtures/link.rs"]
mod link;

use link::{Linked, Reported};

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
}

fn compiled() -> Module {
    perasc_driver::compile_ir(PROGRAM).unwrap_or_else(|e| unreachable!("does not compile: {e:?}"))
}

#[test]
fn compiled_maps_give_what_the_standard_library_gives() {
    let module = compiled();
    let mut m = link::instantiate_with(
        &perasc_driver::emit_direct(
            &module,
            "m.wasm",
            perasc_runtime::Allocator::Debug,
            &perasc_diag::i18n::Localizer::new(perasc_diag::messages::Language::English),
        )
        .wasm,
        perasc_runtime::DEBUG_ALLOCATOR,
    )
    .unwrap_or_else(|e| unreachable!("the modules do not link: {e}"));
    for (name, model) in CASES {
        // Enough steps for tables to grow several times, and to shrink
        // through deletions and `clear`.
        for n in [0, 1, 50, 2500] {
            assert_eq!(m.call(name, n), Ok(Some(model(n))), "{name}({n})");
        }
    }
    assert_eq!(m.runtime("live_count"), Ok(0));
}

/// The loader draws the hashing key of a module's maps, and only of a module
/// that has maps.
#[test]
fn the_loader_seeds_the_maps_of_a_module_that_has_some() {
    let english = perasc_diag::i18n::Localizer::new(perasc_diag::messages::Language::English);
    let build = |source: &str| {
        perasc_driver::build_direct(
            source,
            "m.wasm",
            perasc_driver::DirectOptions::default(),
            &english,
        )
        .unwrap_or_else(|e| unreachable!("does not build: {e:?}"))
        .js
    };
    let js = build(PROGRAM);
    assert!(js.contains("$runtime.map_seed(k[0], k[1])"), "{js}");
    assert!(js.contains("getRandomValues"), "{js}");
    let plain = build("export fn f(n: i32): i32 {\n  let xs = [n];\n  return xs.length;\n}\n");
    assert!(!plain.contains("map_seed"), "{plain}");
}
