//! Compilation pipeline, configuration and cargo/wasm-bindgen calls for the Peras compiler.
//!
//! The pipeline: lexer, parser, then names and types; each new stage plugs into [`check`].

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use perasc_diag::Diagnostic;
use perasc_syntax::Mode;
pub use perasc_syntax::source_too_large;

/// The compiler version, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// At most this many errors are shown per file; the rest are counted.
pub const MAX_ERRORS_PER_FILE: usize = 50;

/// The result of checking one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// Diagnostics shown, sorted by position, at most [`MAX_ERRORS_PER_FILE`].
    pub diagnostics: Vec<Diagnostic>,
    /// Errors beyond the limit.
    pub hidden: usize,
}

impl Checked {
    /// Every error, shown or hidden.
    #[must_use]
    pub const fn error_count(&self) -> usize {
        self.diagnostics.len() + self.hidden
    }

    fn capped(mut diagnostics: Vec<Diagnostic>) -> Self {
        let hidden = diagnostics.len().saturating_sub(MAX_ERRORS_PER_FILE);
        diagnostics.truncate(MAX_ERRORS_PER_FILE);
        Self {
            diagnostics,
            hidden,
        }
    }
}

/// Runs every available stage on a source and gathers all its errors.
///
/// Names and types are checked only on a file without syntax errors: on a broken
/// tree they would mostly report consequences of the syntax errors.
#[must_use]
pub fn check(source: &str) -> Checked {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Checked::capped(parsed.diagnostics);
    }
    Checked::capped(perasc_check::check(&parsed.ast, source))
}

/// Checks a source and lowers it to the IR, verified, or returns its errors.
///
/// # Errors
///
/// The errors of the first stage that has any: syntax, names and types, or code
/// generation limits (`E0900`); [`ToolError::Internal`] if the IR verifier fails.
pub fn compile_ir(source: &str) -> Result<perasc_ir::Module, ToolError> {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Err(ToolError::Diagnostics(Checked::capped(parsed.diagnostics)));
    }
    let analysis = perasc_check::analyze(&parsed.ast, source);
    if !analysis.diagnostics.is_empty() {
        return Err(ToolError::Diagnostics(Checked::capped(
            analysis.diagnostics,
        )));
    }
    let module = perasc_ir::lower(&parsed.ast, &analysis)
        .map_err(|d| ToolError::Diagnostics(Checked::capped(d)))?;
    perasc_ir::verify(&module).map_err(ToolError::Internal)?;
    Ok(module)
}

/// What the direct backend produces for one module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectOutput {
    /// The WebAssembly binary.
    pub wasm: Vec<u8>,
    /// An ES module loading the binary and exporting its functions.
    pub js: String,
    /// TypeScript declarations of the exported functions.
    pub dts: String,
}

/// Compiles a source straight to WebAssembly, without any external tool, after
/// the IR optimizations unless `optimize` is false.
///
/// # Errors
///
/// See [`compile_ir`].
pub fn build_direct(
    source: &str,
    wasm_file: &str,
    optimize: bool,
) -> Result<DirectOutput, ToolError> {
    let mut module = compile_ir(source)?;
    if optimize {
        perasc_ir::optimize(&mut module);
        perasc_ir::verify(&module).map_err(ToolError::Internal)?;
    }
    Ok(DirectOutput {
        wasm: perasc_codegen_wasm::compile(&module),
        js: perasc_codegen_wasm::js_loader(&module, wasm_file),
        dts: perasc_codegen_wasm::dts(&module),
    })
}

/// Why a build failed.
#[derive(Debug)]
pub enum ToolError {
    /// The source has errors.
    Diagnostics(Checked),
    /// The IR verifier rejected the IR: a bug in perasc, with what it found.
    Internal(Vec<String>),
    /// A file could not be written.
    Io(io::Error),
    /// `cargo` or `wasm-bindgen` is not installed.
    Missing(&'static str),
    /// `cargo` or `wasm-bindgen` failed on generated code: a bug in perasc.
    Failed {
        /// The tool.
        tool: &'static str,
        /// What it printed.
        output: String,
    },
}

/// A crate name for a module: `peras_` and the file stem, in `[a-z0-9_]`.
#[must_use]
pub fn crate_name(stem: &str) -> String {
    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("peras_{clean}")
}

/// Compiles a source through the Rust backend: writes a crate in `crate_dir`,
/// builds it with `cargo`, then runs `wasm-bindgen` into `out_dir`.
///
/// # Errors
///
/// See [`ToolError`].
pub fn build_rust(
    source: &str,
    name: &str,
    crate_dir: &Path,
    out_dir: &Path,
) -> Result<(), ToolError> {
    // The Rust backend compiles the unoptimized IR: the differential test in CI
    // then checks the IR optimizations too.
    let module = compile_ir(source)?;
    for (path, contents) in perasc_codegen_rs::crate_files(&module, name) {
        let file = crate_dir.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(ToolError::Io)?;
        }
        // Rewriting an identical file would make cargo rebuild for nothing.
        if fs::read_to_string(&file).ok().as_deref() != Some(contents.as_str()) {
            fs::write(&file, contents).map_err(ToolError::Io)?;
        }
    }
    run(
        "cargo",
        Command::new("cargo")
            .args([
                "build",
                "--quiet",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
            ])
            .current_dir(crate_dir),
    )?;
    let wasm = crate_dir
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{name}.wasm"));
    fs::create_dir_all(out_dir).map_err(ToolError::Io)?;
    run(
        "wasm-bindgen",
        Command::new("wasm-bindgen")
            .args(["--target", "nodejs", "--out-dir"])
            .arg(out_dir)
            .arg(wasm),
    )
}

fn run(tool: &'static str, command: &mut Command) -> Result<(), ToolError> {
    let output = command.output().map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => ToolError::Missing(tool),
        _ => ToolError::Io(e),
    })?;
    if output.status.success() {
        return Ok(());
    }
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Err(ToolError::Failed { tool, output: text })
}

/// Why a source file could not be read.
#[derive(Debug)]
pub enum ReadError {
    /// The extension is neither `.peras` nor `.apeiron`.
    NotASource,
    /// No such file.
    NotFound,
    /// The file is too large to compile; its size in bytes. Report it with
    /// [`source_too_large`] (`E0008`).
    TooLarge(u64),
    /// The file is not UTF-8 text.
    NotUtf8,
    /// Any other I/O error.
    Io(io::Error),
}

/// Reads a source file, refusing what the compiler cannot handle before reading it all.
///
/// # Errors
///
/// See [`ReadError`].
pub fn read_source(path: &Path) -> Result<String, ReadError> {
    if Mode::from_path(path).is_none() {
        return Err(ReadError::NotASource);
    }
    let not_found = |e: io::Error| match e.kind() {
        io::ErrorKind::NotFound => ReadError::NotFound,
        _ => ReadError::Io(e),
    };
    let len = fs::metadata(path).map_err(not_found)?.len();
    if len > u64::from(u32::MAX) {
        return Err(ReadError::TooLarge(len));
    }
    let bytes = fs::read(path).map_err(not_found)?;
    String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8)
}

#[cfg(test)]
mod tests {
    use super::{MAX_ERRORS_PER_FILE, ReadError, check, read_source};
    use std::path::Path;

    #[test]
    fn errors_beyond_the_limit_are_counted_not_shown() {
        let source = "#".repeat(10) + &" #".repeat(MAX_ERRORS_PER_FILE + 5);
        let checked = check(&source);
        assert_eq!(checked.diagnostics.len(), MAX_ERRORS_PER_FILE);
        assert_eq!(checked.hidden, 6);
        assert_eq!(checked.error_count(), MAX_ERRORS_PER_FILE + 6);
    }

    #[test]
    fn only_peras_sources_are_read() {
        assert!(matches!(
            read_source(Path::new("Cargo.toml")),
            Err(ReadError::NotASource)
        ));
        assert!(matches!(
            read_source(Path::new("missing.peras")),
            Err(ReadError::NotFound)
        ));
    }
}
