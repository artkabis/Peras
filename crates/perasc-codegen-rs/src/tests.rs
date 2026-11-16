use super::{RUST_TOOLCHAIN, WASM_BINDGEN_VERSION, crate_files, source};

fn module(text: &str) -> perasc_ir::Module {
    let parsed = perasc_syntax::parse(text);
    assert_eq!(parsed.diagnostics, []);
    let analysis = perasc_check::analyze(&parsed.ast, text);
    assert_eq!(analysis.diagnostics, []);
    perasc_ir::lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:?}"))
}

#[test]
fn generated_rust_is_readable_and_has_no_unsafe() {
    let rust = source(&module(
        "export fn fib(n: i32): i32 {\n  if (n < 2) return n;\n  return fib(n - 1) + fib(n - 2);\n}\n\
         fn loop_(match_: i32): i32 {\n  let total = 0;\n  for (let i = 0; i < match_; i++) { if (i == 2) continue; total += i; }\n  return total;\n}\n",
    ));
    assert!(
        !rust.contains("unsafe {") && !rust.contains("unsafe fn"),
        "{rust}"
    );
    assert!(rust.contains("#![forbid(unsafe_code)]"), "{rust}");
    insta::assert_snapshot!(rust);
}

#[test]
fn the_toolchain_matches_the_repository() {
    let pinned = include_str!("../../../rust-toolchain.toml");
    assert!(
        pinned.contains(&format!("channel = \"{RUST_TOOLCHAIN}\"")),
        "{pinned}"
    );
    let files = crate_files(&module("export fn f(): i32 { return 1; }"), "peras_f");
    let manifest = files
        .iter()
        .find(|(p, _)| p == "Cargo.toml")
        .map_or("", |(_, c)| c.as_str());
    assert!(
        manifest.contains(&format!("wasm-bindgen = \"={WASM_BINDGEN_VERSION}\"")),
        "{manifest}"
    );
}

#[test]
fn ci_installs_the_same_wasm_bindgen() {
    let ci = include_str!("../../../.github/workflows/ci.yml");
    let install = format!("wasm-bindgen-cli --version ={WASM_BINDGEN_VERSION}");
    assert!(ci.contains(&install), "CI must install {install}");
}

#[test]
fn blocks_of_the_optimized_ir_get_labels() {
    // The return inside the loop leaves each copy early.
    let mut ir = module(
        "export fn f(n: i32): i32 {\n  while (n > 10) { return f(n - 1) + 1; }\n  return n;\n}\n",
    );
    perasc_ir::optimize(&mut ir);
    let rust = source(&ir);
    if let Ok(path) = std::env::var("PERAS_DUMP_RUST") {
        let _ = std::fs::write(path, &rust);
    }
    for label in ["'b1: {", "'b3: {", "break 'b1;", "break 'b3;"] {
        assert!(rust.contains(label), "{label}\n{rust}");
    }
}

/// Module variables: a cell of its thread each, set to its initial value,
/// with no `unsafe` (decision 0023).
#[test]
fn module_variables_are_cells_with_no_unsafe() {
    let rust = source(&module(
        "let count = 20;\nlet total = 0.5;\nexport fn bump(): i32 {\n  count++;\n  total += 1.0;\n  return count + total.toI32();\n}\n",
    ));
    assert!(
        !rust.contains("unsafe {") && !rust.contains("static mut"),
        "{rust}"
    );
    assert!(rust.contains("thread_local!"), "{rust}");
    insta::assert_snapshot!(rust);
}

/// Decision 0024: the unsigned operations read their operands as unsigned,
/// through `u32` or `u64`, and a conversion of a `u32` to a wider type
/// extends it by zeros.
#[test]
fn unsigned_operations_go_through_unsigned_types() {
    let rust = source(&module(
        "export fn f(a: u32, b: u32): bool {\n  return a / b > a % b && (a >> 3) < b;\n}\n\
         export fn g(a: u32): u64 {\n  return a.toU64();\n}\n\
         export fn h(a: u64, b: u64): u64 {\n  return a / b;\n}\n",
    ));
    for expected in [
        "as u32) / ((",
        "as u32) % ((",
        "as u32).wrapping_shr(",
        "as u32) > ((",
        "as u32) < ((",
        "as u32) as i64",
        "as u64) / ((",
    ] {
        assert!(rust.contains(expected), "{expected} in\n{rust}");
    }
}

/// Decision 0025: a call through a function value is a `match` on its
/// position in the table, over the functions of its signature only.
#[test]
fn a_call_through_a_value_matches_on_the_table() {
    let rust = source(&module(
        "fn twice(x: i32): i32 {\n  return x * 2;\n}\n\
         fn odd(x: i32): bool {\n  return x % 2 == 1;\n}\n\
         export fn f(n: i32): i32 {\n  const g = twice;\n  const h = odd;\n  if (h(n)) {\n    return g(n);\n  }\n  return 0;\n}\n",
    ));
    // `twice` is at 0 and `odd` at 1; each call lists its own signature's.
    assert!(rust.contains("0 => p_twice("), "{rust}");
    assert!(rust.contains("1 => p_odd("), "{rust}");
    assert!(
        !rust.contains("0 => p_odd(") && !rust.contains("1 => p_twice("),
        "{rust}"
    );
}
