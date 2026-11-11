//! An integer overflow compiled to WebAssembly in a debug build stops the
//! program through the host, and wraps in a production build (decision
//! 0011, step 5).

#[path = "fixtures/link.rs"]
mod link;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_driver::{Allocator, DirectOptions, build_direct};

const PROGRAM: &str = "
export fn add(a: i32, b: i32): i32 {
  return a + b;
}
";

fn add(checks: bool, a: i32, b: i32) -> Option<i32> {
    let built = build_direct(
        PROGRAM,
        "m.wasm",
        DirectOptions {
            allocator: Allocator::Debug,
            overflow_checks: checks,
            ..DirectOptions::default()
        },
        &Localizer::new(Language::English),
    )
    .unwrap_or_else(|e| unreachable!("does not build: {e:?}"));
    let mut m = link::instantiate_with(&built.wasm, perasc_runtime::DEBUG_ALLOCATOR)
        .unwrap_or_else(|e| unreachable!("the modules do not link: {e}"));
    let func = m.module.get_func(&m.store, "add")?;
    let mut results = vec![wasmi::Val::I32(0)];
    func.call(
        &mut m.store,
        &[wasmi::Val::I32(a), wasmi::Val::I32(b)],
        &mut results,
    )
    .ok()?;
    // The check links the runtime; nothing is left alive by a call.
    let live = m
        .allocator
        .get_typed_func::<(), i32>(&m.store, "live_count")
        .map(|f| f.call(&mut m.store, ()));
    assert!(matches!(live, Ok(Ok(0))), "a block is left alive");
    results.first().and_then(wasmi::Val::i32)
}

#[test]
fn a_debug_build_stops_on_an_overflow_and_production_wraps() {
    assert_eq!(add(true, 2, 3), Some(5));
    assert_eq!(add(true, i32::MAX, 1), None);
    assert_eq!(add(false, i32::MAX, 1), Some(i32::MIN));
}
