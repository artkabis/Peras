//! `perasc build`: compiles one `.peras` file to WebAssembly.
//!
//! Two backends: `direct` (the default) encodes the `.wasm` itself and needs no
//! other tool; `rust` generates a Rust crate and builds it with `cargo` and
//! `wasm-bindgen`. Both run every check first and produce nothing on an error.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use perasc_diag::Message;
use perasc_diag::i18n::Localizer;
use perasc_diag::render::{RenderOptions, render};
use perasc_diag::source::SourceFile;
use perasc_driver::{
    Checked, ReadError, ToolError, build_direct, build_rust, crate_name, read_source,
};

use crate::{COMPILER_BUG, ERRORS_FOUND, SUCCESS, USAGE_OR_IO, error_line, to_i64};

/// The code generator to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Encodes WebAssembly directly.
    Direct,
    /// Generates Rust, then runs `cargo` and `wasm-bindgen`.
    Rust,
}

/// Builds `path` into `out_dir`, printing the outcome.
#[allow(clippy::too_many_arguments)]
pub fn build(
    path: &str,
    backend: Backend,
    optimize: bool,
    out_dir: &Path,
    localizer: &Localizer,
    color: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    let text = match read_source(Path::new(path)) {
        Ok(text) => text,
        Err(problem) => {
            let message = match problem {
                ReadError::NotASource => Message::new("cli-not-a-source"),
                ReadError::NotFound => Message::new("cli-not-found"),
                ReadError::NotUtf8 => Message::new("cli-not-utf8"),
                ReadError::TooLarge(len) => {
                    let file = SourceFile::new(path, "");
                    let d = perasc_driver::source_too_large(len);
                    writeln!(
                        err,
                        "{}",
                        render(&d, &file, localizer, RenderOptions { color })
                    )?;
                    return Ok(ERRORS_FOUND);
                }
                ReadError::Io(e) => Message::new("cli-read-error").text("reason", e.to_string()),
            };
            writeln!(
                err,
                "{}",
                error_line(localizer, &message.text("path", path))
            )?;
            return Ok(USAGE_OR_IO);
        }
    };
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module")
        .to_owned();
    let started = Instant::now();
    let result = match backend {
        Backend::Direct => direct(&text, &stem, out_dir, optimize),
        Backend::Rust => {
            let name = crate_name(&stem);
            let crate_dir = out_dir.join(".rust").join(&stem);
            build_rust(&text, &name, &crate_dir, out_dir)
                .map(|()| out_dir.join(format!("{name}_bg.wasm")))
        }
    };
    let elapsed = started.elapsed().as_millis();
    match result {
        Ok(wasm) => {
            let bytes = fs::metadata(&wasm).map_or(0, |m| m.len());
            let ok = Message::new("cli-build-ok")
                .text("file", path)
                .text("out", wasm.display().to_string())
                .number("bytes", i64::try_from(bytes).unwrap_or(i64::MAX))
                .number("ms", i64::try_from(elapsed).unwrap_or(i64::MAX));
            writeln!(out, "{}", localizer.message(&ok))?;
            Ok(SUCCESS)
        }
        Err(ToolError::Diagnostics(checked)) => {
            report(&checked, path, &text, localizer, color, err)?;
            Ok(ERRORS_FOUND)
        }
        Err(ToolError::Io(e)) => {
            let message = Message::new("cli-write-error").text("reason", e.to_string());
            writeln!(err, "{}", error_line(localizer, &message))?;
            Ok(USAGE_OR_IO)
        }
        Err(ToolError::Missing(tool)) => {
            writeln!(
                err,
                "{}",
                error_line(
                    localizer,
                    &Message::new("cli-tool-missing").text("tool", tool)
                )
            )?;
            Ok(USAGE_OR_IO)
        }
        Err(ToolError::Internal(problems)) => {
            writeln!(
                err,
                "{}",
                error_line(localizer, &Message::new("cli-internal-error"))
            )?;
            for problem in problems {
                writeln!(err, "  {problem}")?;
            }
            Ok(COMPILER_BUG)
        }
        Err(ToolError::Failed { tool, output }) => {
            writeln!(
                err,
                "{}",
                error_line(
                    localizer,
                    &Message::new("cli-tool-failed").text("tool", tool)
                )
            )?;
            writeln!(err, "{output}")?;
            Ok(COMPILER_BUG)
        }
    }
}

/// The direct backend: writes `<stem>.wasm`, `<stem>.js` and `<stem>.d.ts`.
fn direct(text: &str, stem: &str, out_dir: &Path, optimize: bool) -> Result<PathBuf, ToolError> {
    let wasm_file = format!("{stem}.wasm");
    let output = build_direct(text, &wasm_file, optimize)?;
    fs::create_dir_all(out_dir).map_err(ToolError::Io)?;
    let wasm = out_dir.join(&wasm_file);
    fs::write(&wasm, &output.wasm).map_err(ToolError::Io)?;
    fs::write(out_dir.join(format!("{stem}.js")), &output.js).map_err(ToolError::Io)?;
    fs::write(out_dir.join(format!("{stem}.d.ts")), &output.dts).map_err(ToolError::Io)?;
    Ok(wasm)
}

fn report(
    checked: &Checked,
    path: &str,
    text: &str,
    localizer: &Localizer,
    color: bool,
    err: &mut dyn Write,
) -> io::Result<()> {
    let file = SourceFile::new(path, text);
    for d in &checked.diagnostics {
        writeln!(
            err,
            "{}",
            render(d, &file, localizer, RenderOptions { color })
        )?;
    }
    let summary =
        Message::new("cli-summary-errors").number("errors", to_i64(checked.error_count()));
    writeln!(err, "{}", localizer.message(&summary))
}
