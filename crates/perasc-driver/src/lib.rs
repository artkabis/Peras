//! Compilation pipeline, configuration and cargo/wasm-bindgen calls for the Peras compiler.

/// The compiler version, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
