//! `perasc build`: compiles one `.peras` file to WebAssembly.
//!
//! Two backends: `direct` (the default) encodes the `.wasm` itself and needs no
//! other tool; `rust` generates a Rust crate and builds it with `cargo` and
//! `wasm-bindgen`. Both run every check first and produce nothing on an error.
//!
//! With `--json`, the diagnostics are printed on standard output in the format of
//! `perasc check --json`, also when there are none: this is what the Vite plugin
//! reads to place an error in its overlay.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use perasc_diag::Message;
use perasc_diag::i18n::Localizer;
use perasc_diag::json::{self, FileReport, Report};
use perasc_diag::render::{RenderOptions, render};
use perasc_diag::source::SourceFile;
use perasc_driver::{
    Checked, ReadError, ToolError, build_direct, build_rust, crate_name, read_source,
};

use crate::{COMPILER_BUG, ERRORS_FOUND, SUCCESS, USAGE_OR_IO, error_line, to_i64};

/// How the outcome is printed.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// ANSI colours in rendered diagnostics.
    pub color: bool,
    /// Diagnostics as JSON on standard output.
    pub json: bool,
}

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
    style: Style,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    let Style { color, json } = style;
    let text = match read(path, localizer, style, out, err)? {
        Ok(text) => text,
        Err(code) => return Ok(code),
    };
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module")
        .to_owned();
    let started = Instant::now();
    let result = match backend {
        Backend::Direct => direct(&text, &stem, out_dir, optimize, localizer),
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
            if json {
                print_json(None, path, &text, localizer, out)?;
            } else {
                writeln!(out, "{}", localizer.message(&ok))?;
            }
            Ok(SUCCESS)
        }
        Err(ToolError::Diagnostics(checked)) => {
            if json {
                print_json(Some(&checked), path, &text, localizer, out)?;
            } else {
                report(&checked, path, &text, localizer, color, err)?;
            }
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

/// Reads the source, or prints why it cannot and returns the exit code.
fn read(
    path: &str,
    localizer: &Localizer,
    Style { color, json }: Style,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<Result<String, u8>> {
    Ok(match read_source(Path::new(path)) {
        Ok(text) => Ok(text),
        Err(problem) => {
            let message = match problem {
                ReadError::NotASource => Message::new("cli-not-a-source"),
                ReadError::NotFound => Message::new("cli-not-found"),
                ReadError::NotUtf8 => Message::new("cli-not-utf8"),
                ReadError::TooLarge(len) => {
                    let checked = Checked {
                        diagnostics: vec![perasc_driver::source_too_large(len)],
                        hidden: 0,
                    };
                    if json {
                        print_json(Some(&checked), path, "", localizer, out)?;
                    } else {
                        report(&checked, path, "", localizer, color, err)?;
                    }
                    return Ok(Err(ERRORS_FOUND));
                }
                ReadError::Io(e) => Message::new("cli-read-error").text("reason", e.to_string()),
            };
            writeln!(
                err,
                "{}",
                error_line(localizer, &message.text("path", path))
            )?;
            Err(USAGE_OR_IO)
        }
    })
}

/// The direct backend: writes `<stem>.wasm`, `<stem>.js` and `<stem>.d.ts`.
fn direct(
    text: &str,
    stem: &str,
    out_dir: &Path,
    optimize: bool,
    localizer: &Localizer,
) -> Result<PathBuf, ToolError> {
    let wasm_file = format!("{stem}.wasm");
    let output = build_direct(text, &wasm_file, optimize, localizer)?;
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

/// The report of `perasc check --json`, for one file; `None` when it compiled.
fn print_json(
    checked: Option<&Checked>,
    path: &str,
    text: &str,
    localizer: &Localizer,
    out: &mut dyn Write,
) -> io::Result<()> {
    let file = SourceFile::new(path, text);
    let report = Report {
        version: json::FORMAT_VERSION,
        language: localizer.language().tag(),
        error_count: checked.map_or(0, Checked::error_count),
        files: vec![FileReport {
            path: path.to_owned(),
            diagnostics: checked.map_or_else(Vec::new, |c| {
                c.diagnostics
                    .iter()
                    .map(|d| json::diagnostic(d, &file, localizer))
                    .collect()
            }),
            hidden_count: checked.map_or(0, |c| c.hidden),
        }],
    };
    let text = serde_json::to_string_pretty(&report).map_err(io::Error::other)?;
    writeln!(out, "{text}")
}
