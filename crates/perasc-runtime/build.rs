//! Assembles the runtime from WebAssembly text: a mistake in it fails the build.
//!
//! Each allocator is a module of its own; the buffer functions, the same for
//! both, are written once in `runtime/buffers.wat` and included where an
//! allocator says `;; @include buffers.wat`.
//!
//! Each is built twice: with WebAssembly SIMD, and without, for
//! `--simd off`. Between `;; @simd` and `;; @scalar`, `buffers.wat` holds what
//! only the first keeps; between `;; @scalar` and `;; @end`, what only the
//! second keeps.

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
        for (simd, suffix) in [(true, ""), (false, "_scalar")] {
            let text = text.replace(";; @include buffers.wat", &variant(&buffers, simd)?);
            let wasm = wat::parse_str(&text).map_err(|e| format!("{source}: {e}"))?;
            let out = PathBuf::from(env::var("OUT_DIR")?).join(format!("{name}{suffix}.wasm"));
            fs::write(out, wasm)?;
        }
    }
    Ok(())
}

/// `buffers.wat` with SIMD or without: each `;; @simd … ;; @scalar … ;; @end`
/// keeps its first part or its second.
fn variant(buffers: &str, simd: bool) -> Result<String, Box<dyn std::error::Error>> {
    let mut out = String::new();
    let mut rest = buffers;
    while let Some(start) = rest.find(";; @simd") {
        let (Some(middle), Some(end)) = (rest.find(";; @scalar"), rest.find(";; @end")) else {
            return Err(format!("{BUFFERS}: `;; @simd` without `;; @scalar` and `;; @end`").into());
        };
        if !(start < middle && middle < end) {
            return Err(
                format!("{BUFFERS}: `;; @simd`, `;; @scalar`, `;; @end` out of order").into(),
            );
        }
        out.push_str(&rest[..start]);
        out.push_str(if simd {
            &rest[start..middle]
        } else {
            &rest[middle..end]
        });
        rest = &rest[end + ";; @end".len()..];
    }
    out.push_str(rest);
    Ok(out)
}
