//! A module with its runtime linked into it (decision 0016): one module,
//! instantiated alone, with only the host's functions as imports, gives what
//! the module beside its runtime gives, and leaves no block alive.

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use wasmi::{Caller, Engine, Linker, Store, Val};

const PROGRAM: &str = r#"
export fn words(n: i32): i32 {
  let total = 0;
  for (let i = 0; i < n; i++) {
    const parts = "a,bb,ccc,dddd".split(",");
    for (const p of parts) {
      if (p != "bb") {
        total += p.length;
      }
    }
  }
  return total;
}
"#;

fn build(allocator: perasc_runtime::Allocator) -> Vec<u8> {
    let module = perasc_driver::compile_ir(PROGRAM)
        .unwrap_or_else(|e| unreachable!("does not compile: {e:?}"));
    let options = perasc_driver::DirectOptions {
        allocator,
        ..perasc_driver::DirectOptions::default()
    };
    perasc_driver::emit_direct_for(
        &module,
        "m.wasm",
        options,
        &Localizer::new(Language::English),
    )
    .wasm
}

#[test]
fn a_linked_module_runs_alone() {
    for allocator in [
        perasc_runtime::Allocator::Debug,
        perasc_runtime::Allocator::Tlsf,
    ] {
        let wasm = build(allocator);
        let validated = wasmparser::Validator::new().validate_all(&wasm);
        assert!(validated.is_ok(), "{:?}", validated.err());
        let engine = Engine::default();
        let mut store = Store::new(&engine, ());
        let mut linker = Linker::new(&engine);
        let defined = linker
            .func_wrap(
                "peras",
                "fault",
                |_: Caller<'_, ()>, _: i32, _: i32, _: i32, _: i32| {
                    Err::<(), _>(wasmi::Error::new("peras fault"))
                },
            )
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
            .and_then(|l| {
                l.func_wrap(
                    perasc_codegen_wasm::RUNTIME_MODULE,
                    "bounds",
                    |_: Caller<'_, ()>, _: i32, _: i32| Err::<(), _>(wasmi::Error::new("bounds")),
                )
            });
        assert!(defined.is_ok());
        let module = wasmi::Module::new(&engine, &wasm[..])
            .unwrap_or_else(|e| unreachable!("not a module: {e}"));
        // Only the host's functions: no runtime instance beside it.
        let names: Vec<String> = module
            .imports()
            .map(|i| format!("{}.{}", i.module(), i.name()))
            .collect();
        assert!(
            names
                .iter()
                .all(|n| n.starts_with("peras.") || n == "peras_rt.bounds"),
            "{names:?}"
        );
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .unwrap_or_else(|e| unreachable!("does not instantiate: {e}"));
        let call = |store: &mut Store<()>, name: &str, args: &[Val]| {
            let func = instance.get_func(&*store, name)?;
            let mut results = vec![Val::I32(0)];
            func.call(store, args, &mut results).ok()?;
            results.first().and_then(Val::i32)
        };
        assert_eq!(call(&mut store, "words", &[Val::I32(1000)]), Some(8000));
        let live = format!("{}live_count", perasc_runtime::RUNTIME_EXPORT);
        assert_eq!(call(&mut store, &live, &[]), Some(0));
    }
}
