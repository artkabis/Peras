//! Compilation pipeline, configuration and cargo/wasm-bindgen calls for the Peras compiler.
//!
//! The pipeline: lexer, parser, then names and types; each new stage plugs into [`check`].

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use perasc_diag::Diagnostic;
use perasc_diag::i18n::Localizer;
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
    options: DirectOptions,
    localizer: &Localizer,
) -> Result<DirectOutput, ToolError> {
    let mut module = compile_ir(source)?;
    if options.optimize {
        perasc_ir::optimize(&mut module);
        perasc_ir::verify(&module).map_err(ToolError::Internal)?;
    }
    Ok(emit_direct_for(&module, wasm_file, options, localizer))
}

pub use perasc_runtime::Allocator;

/// How the direct backend builds a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectOptions {
    /// Runs the IR optimizations.
    pub optimize: bool,
    /// The allocator a module that uses the heap is linked with: the debug
    /// one by default, TLSF for production.
    pub allocator: perasc_runtime::Allocator,
    /// Uses WebAssembly SIMD, the default: in loops whose turns are
    /// independent, and in the runtime's search for bytes. Without, the
    /// module loads in engines that have no SIMD (Safari before 16.4).
    pub simd: bool,
}

impl Default for DirectOptions {
    fn default() -> Self {
        Self {
            optimize: true,
            allocator: perasc_runtime::Allocator::Debug,
            simd: true,
        }
    }
}

/// The direct backend's output for a verified module. A module that uses the
/// heap is linked to `allocator`, whose messages are in the language of
/// `localizer`.
#[must_use]
pub fn emit_direct(
    module: &perasc_ir::Module,
    wasm_file: &str,
    allocator: perasc_runtime::Allocator,
    localizer: &Localizer,
) -> DirectOutput {
    let options = DirectOptions {
        allocator,
        ..DirectOptions::default()
    };
    emit_direct_for(module, wasm_file, options, localizer)
}

/// [`emit_direct`], with SIMD as `options` says.
#[must_use]
pub fn emit_direct_for(
    module: &perasc_ir::Module,
    wasm_file: &str,
    options: DirectOptions,
    localizer: &Localizer,
) -> DirectOutput {
    let compiled = perasc_codegen_wasm::compile_for(module, options.simd);
    // SIMD is needed by the runtime of a module that uses the heap, and by
    // the loops that run several turns at once.
    let needs_simd = options.simd && (module.uses_heap() || compiled.vectors);
    let simd = localizer.message(&perasc_diag::Message::new("runtime-simd"));
    let faults = fault_messages(localizer);
    let bounds = localizer.message(
        &perasc_diag::Message::new("runtime-index")
            .text("index", "{index}")
            .text("length", "{length}"),
    );
    // The runtime this module needs: what its loader calls, and the
    // functions on strings and arrays the module calls.
    let full = options.allocator.runtime(options.simd);
    let used = module.runtime_fns();
    let loader = |allocator: &[u8]| {
        let runtime = perasc_codegen_wasm::Runtime {
            allocator,
            sites: &compiled.sites,
            faults: &faults,
            bounds: &bounds,
            simd: needs_simd.then_some(simd.as_str()),
        };
        perasc_codegen_wasm::js_loader(module, wasm_file, Some(&runtime))
    };
    // The functions the loader calls itself, as `$runtime.map_put_s` to copy
    // a map in, read from a first loader made with the whole runtime.
    let draft = loader(full);
    let called = loader_calls(&draft);
    let keep: Vec<&str> = LOADER_EXPORTS
        .iter()
        .copied()
        .chain(used.iter().map(|f| f.name()))
        .chain(called.iter().map(String::as_str))
        .collect();
    let allocator = perasc_runtime::prune(full, &keep).unwrap_or_else(|_| full.to_vec());
    DirectOutput {
        js: loader(&allocator),
        dts: perasc_codegen_wasm::dts(module),
        wasm: compiled.wasm,
    }
}

/// The runtime's functions a loader calls: each `$runtime.name` in it.
fn loader_calls(js: &str) -> Vec<String> {
    let mut names: Vec<String> = js
        .match_indices("$runtime.")
        .map(|(at, found)| {
            js[at + found.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect()
        })
        .filter(|name: &String| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// What the JavaScript loader takes from the runtime, whatever the module
/// calls: its memory, the allocation and buffer functions, and the counts its
/// `__perasHeap` and the A/B benchmark read.
const LOADER_EXPORTS: [&str; 10] = [
    "memory",
    "alloc",
    "release",
    "buf_new",
    "buf_reserve",
    "buf_release",
    "buf_clone",
    "live_count",
    "check_leaks",
    "allocations",
];

/// The message of each fault of the debug allocator, kind 1 first, with the
/// placeholders the generated JavaScript fills in.
#[must_use]
pub fn fault_messages(localizer: &Localizer) -> Vec<String> {
    [
        "runtime-out-of-memory",
        "runtime-too-large",
        "runtime-overflow",
        "runtime-invalid-release",
        "runtime-double-release",
        "runtime-use-after-release",
        "runtime-leak",
    ]
    .into_iter()
    .map(|id| {
        let message = perasc_diag::Message::new(id)
            .text("alloc", "{alloc}")
            .text("release", "{release}")
            .text("count", "{count}")
            .text("size", "{size}");
        localizer.message(&message)
    })
    .collect()
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
    // It has no allocator yet; lowering produces no heap IR today.
    if module.uses_heap() {
        return Err(ToolError::Internal(vec![
            "the IR uses the heap, which the Rust backend does not compile yet".to_owned(),
        ]));
    }
    // wasm-bindgen cannot export several results: only inner functions return them.
    if let Some(f) = module
        .functions
        .iter()
        .find(|f| f.exported && !f.more.is_empty())
    {
        return Err(ToolError::Internal(vec![format!(
            "the Rust backend cannot export {}, which returns a structure",
            f.name
        )]));
    }
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
