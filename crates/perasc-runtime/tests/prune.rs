//! Pruning the runtime for one module: the exports asked for stay, with
//! everything they call; the others go, and the module is still valid and
//! works. Breaking the call graph (dropping a callee) makes it invalid, which
//! the validation catches.

use perasc_runtime::{Allocator, prune};
use wasmi::{Caller, Engine, Linker, Module, Store};

/// The names of the exports of a module.
fn exports(wasm: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::ExportSection(section)) = payload {
            for export in section.into_iter().flatten() {
                names.push(export.name.to_owned());
            }
        }
    }
    names.sort();
    names
}

/// Instantiates a runtime in wasmi, its host functions stubbed.
fn instantiate(wasm: &[u8]) -> (Store<()>, wasmi::Instance) {
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap(
            "peras",
            "fault",
            |_: Caller<'_, ()>, _: i32, _: i32, _: i32, _: i32| {},
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    linker
        .func_wrap(
            "peras",
            "float_text",
            |_: Caller<'_, ()>, _: f64, _: i32, _: i32| 0_i32,
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    let module = Module::new(&engine, wasm).unwrap_or_else(|e| unreachable!("{e}"));
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|e| unreachable!("{e}"));
    (store, instance)
}

#[test]
fn a_pruned_runtime_keeps_what_is_asked_and_still_works() {
    for allocator in [Allocator::Debug, Allocator::Tlsf] {
        for simd in [true, false] {
            let full = allocator.runtime(simd);
            let keep = [
                "memory",
                "alloc",
                "release",
                "buf_new",
                "buf_release",
                "live_count",
                "str_split",
            ];
            let pruned = prune(full, &keep).unwrap_or_else(|e| unreachable!("{e}"));
            assert!(
                wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                    .validate_all(&pruned)
                    .is_ok(),
                "{allocator:?}, simd {simd}: not valid"
            );
            let mut wanted: Vec<String> = keep.iter().map(|&k| k.to_owned()).collect();
            wanted.sort();
            assert_eq!(exports(&pruned), wanted, "{allocator:?}, simd {simd}");
            assert!(
                pruned.len() * 2 < full.len(),
                "{allocator:?}, simd {simd}: {} of {} bytes left",
                pruned.len(),
                full.len()
            );
            // It still allocates, splits and releases, leaving nothing alive.
            let (mut store, instance) = instantiate(&pruned);
            let new = instance
                .get_typed_func::<(i32, i32, i32), i32>(&store, "buf_new")
                .unwrap_or_else(|e| unreachable!("{e}"));
            let release = instance
                .get_typed_func::<(i32, i32), ()>(&store, "buf_release")
                .unwrap_or_else(|e| unreachable!("{e}"));
            let live = instance
                .get_typed_func::<(), i32>(&store, "live_count")
                .unwrap_or_else(|e| unreachable!("{e}"));
            let text = new
                .call(&mut store, (8, 1, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let separator = new
                .call(&mut store, (1, 1, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let split = instance
                .get_typed_func::<(i32, i32), i32>(&store, "str_split")
                .unwrap_or_else(|e| unreachable!("{e}"));
            let parts = split
                .call(&mut store, (text, separator))
                .unwrap_or_else(|e| unreachable!("{e}"));
            for h in [parts, separator, text] {
                release
                    .call(&mut store, (h, 0))
                    .unwrap_or_else(|e| unreachable!("{e}"));
            }
            assert_eq!(
                live.call(&mut store, ()).ok(),
                Some(0),
                "{allocator:?}, simd {simd}"
            );
        }
    }
}

#[test]
fn asking_for_every_export_keeps_every_export() {
    let full = Allocator::Tlsf.runtime(true);
    let all = exports(full);
    let names: Vec<&str> = all.iter().map(String::as_str).collect();
    let pruned = prune(full, &names).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(exports(&pruned), all);
    // Only the names section goes.
    assert!(pruned.len() < full.len());
}
