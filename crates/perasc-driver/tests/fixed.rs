//! `toFixed` compiled to WebAssembly: the runtime asks the host for the
//! text, as the loader asks JavaScript, with room for 100 decimals; a number
//! of digits outside 0 to 100 stops the program before any text is made.

#[path = "fixtures/link.rs"]
mod link;

const PROGRAM: &str = "
export fn digits(n: i32): i32 {
  const x = 2.5;
  const y = -0.0001;
  const big = 123.456;
  return x.toFixed(0).length * 1000000 + y.toFixed(2).length * 10000 + big.toFixed(n).length;
}
";

#[test]
fn compiled_to_fixed_gives_javascripts_text() {
    let module = perasc_driver::compile_ir(PROGRAM)
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
    let call = |m: &mut link::Linked, n: i32| {
        let func = m.module.get_func(&m.store, "digits");
        let mut results = vec![wasmi::Val::I32(0)];
        func.and_then(|f| {
            f.call(&mut m.store, &[wasmi::Val::I32(n)], &mut results)
                .ok()
        })
        .and_then(|()| results.first().and_then(wasmi::Val::i32))
    };
    // "3", "-0.00", "123.456…" with 0, 2 and 100 decimals.
    assert_eq!(call(&mut m, 0), Some(1_050_003));
    assert_eq!(call(&mut m, 2), Some(1_050_006));
    assert_eq!(call(&mut m, 100), Some(1_050_104));
    // Every text made is released.
    let live = m
        .allocator
        .get_typed_func::<(), i32>(&m.store, "live_count")
        .map(|f| f.call(&mut m.store, ()));
    assert!(matches!(live, Ok(Ok(0))), "a block is left alive");
    assert_eq!(call(&mut m, 101), None, "101 digits");
    assert_eq!(call(&mut m, -1), None, "-1 digits");
}
