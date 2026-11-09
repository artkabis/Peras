//! Modules that use the heap, linked to the debug allocator and run in an
//! interpreter: what the release verifier accepts runs cleanly, and what it
//! refuses is caught by the allocator at run time, with its sites.

#[path = "fixtures/heap_ir.rs"]
mod heap_ir;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_ir::{Module, verify};
use perasc_runtime::Fault;

#[path = "fixtures/link.rs"]
mod link;

use link::{Linked, Reported};

fn link(module: &Module) -> Result<Linked, wasmi::Error> {
    let output = perasc_driver::emit_direct(
        module,
        "m.wasm",
        perasc_runtime::Allocator::Debug,
        &Localizer::new(Language::English),
    );
    link::instantiate(&output.wasm)
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
        let output = perasc_driver::emit_direct(
            &heap_ir::twice(),
            "m.wasm",
            perasc_runtime::Allocator::Debug,
            &Localizer::new(language),
        );
        assert!(output.js.contains(double), "{language:?}:\n{}", output.js);
        assert!(output.js.contains("\"twice (release 2)\""), "{}", output.js);
        assert!(output.dts.contains("__perasHeap"), "{}", output.dts);
    }
    // A module that does not use the heap carries no allocator.
    let plain = perasc_driver::build_direct(
        "export fn f(n: i32): i32 { return n; }",
        "f.wasm",
        perasc_driver::DirectOptions::default(),
        &Localizer::new(Language::English),
    );
    let js = plain.map(|o| o.js).unwrap_or_default();
    assert!(
        js.contains("const $imports = { Math };") && !js.contains("PerasMemoryError"),
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

/// Arrays of strings and of arrays: buffers that own buffers, released and
/// copied deeply by the runtime, without a leak.
#[test]
fn arrays_of_buffers_release_and_copy_what_they_own() {
    let source = "export fn wordy(n: i32): i32 {\n  let words: Array<String> = [];\n  for (let i = 0; i < n; i++) {\n    words.push(\"w\" + \"é\");\n  }\n  let copy = words.copy();\n  copy[0] = \"replaced\";\n  words[n - 1] += \"!\";\n  let total = 0;\n  for (const w of words) {\n    total += w.length;\n  }\n  return total + copy[0].length * 1000;\n}\n\
         export fn table(n: i32): i32 {\n  let rows: Array<Array<i32>> = [];\n  for (let i = 0; i < n; i++) {\n    let row: Array<i32> = [];\n    for (let j = 0; j < n; j++) {\n      row.push(i * j);\n    }\n    rows.push(row);\n  }\n  rows = rows.copy();\n  return rows[n - 1][n - 1];\n}\n";
    let module = perasc_driver::compile_ir(source)
        .unwrap_or_else(|e| unreachable!("does not compile: {e:?}"));
    let mut m = linked(&module);
    // 100 words of 3 bytes, one with "!", and "replaced" in the copy.
    assert_eq!(m.call("wordy", 100), Ok(Some(8301)));
    assert_eq!(m.call("table", 30), Ok(Some(841)));
    assert_eq!(m.runtime("live_count"), Ok(0));
    assert_eq!(m.check_leaks(), Ok(()));
}
