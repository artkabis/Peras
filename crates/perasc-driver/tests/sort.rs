//! `sortBy` compiled: random records sorted as WebAssembly on the debug
//! allocator give what a stable sort of the standard library gives, without
//! a leak. The IR interpreter runs the same program in `perasc-ir`'s
//! `sort_tests`, and JavaScript in `perasc-codegen-js`'s tests.

#[path = "fixtures/sort.rs"]
mod sort;

#[path = "fixtures/link.rs"]
mod link;

use link::{Linked, Reported};

fn call(m: &mut Linked, name: &str, n: i32) -> Result<Option<i32>, Reported> {
    let func = m.module.get_func(&m.store, name);
    let mut results = vec![wasmi::Val::I32(0); func.map_or(0, |f| f.ty(&m.store).results().len())];
    let outcome = func.map(|f| f.call(&mut m.store, &[wasmi::Val::I32(n)], &mut results));
    match outcome {
        Some(Ok(())) => Ok(results.first().and_then(wasmi::Val::i32)),
        _ => Err(m.store.data_mut().take()),
    }
}

#[test]
fn compiled_sort_by_gives_what_a_stable_sort_gives() {
    let module = perasc_driver::compile_ir(sort::PROGRAM)
        .unwrap_or_else(|e| unreachable!("does not compile: {e:?}"));
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
    for (name, model) in sort::CASES {
        for n in sort::SIZES {
            assert_eq!(call(&mut m, name, n), Ok(Some(model(n))), "{name}({n})");
        }
    }
    let live = m
        .allocator
        .get_typed_func::<(), i32>(&m.store, "live_count")
        .map(|f| f.call(&mut m.store, ()));
    assert!(matches!(live, Ok(Ok(0))), "a block is left alive");
}
