//! Assembles the runtime from WebAssembly text: a mistake in it fails the build.
//!
//! Each allocator is a module of its own; the buffer functions, the same for
//! both, are written once in `runtime/buffers.wat` and included where an
//! allocator says `;; @include buffers.wat`.

use std::env;
use std::fs;
use std::path::PathBuf;

const BUFFERS: &str = "runtime/buffers.wat";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed={BUFFERS}");
    let buffers = fs::read_to_string(BUFFERS)?;
    for name in ["alloc_debug", "alloc_tlsf"] {
        let source = format!("runtime/{name}.wat");
        println!("cargo::rerun-if-changed={source}");
        let text = fs::read_to_string(&source)?;
        if !text.contains(";; @include buffers.wat") {
            return Err(format!("{source} does not include {BUFFERS}").into());
        }
        let text = text.replace(";; @include buffers.wat", &buffers);
        let wasm = wat::parse_str(&text).map_err(|e| format!("{source}: {e}"))?;
        let out = PathBuf::from(env::var("OUT_DIR")?).join(format!("{name}.wasm"));
        fs::write(out, wasm)?;
    }
    Ok(())
}
