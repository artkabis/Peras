//! Assembles the runtime from WebAssembly text: a mistake in it fails the build.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "runtime/alloc_debug.wat";
    println!("cargo::rerun-if-changed={source}");
    let wasm = wat::parse_file(source)?;
    let out = PathBuf::from(env::var("OUT_DIR")?).join("alloc_debug.wasm");
    fs::write(out, wasm)?;
    Ok(())
}
