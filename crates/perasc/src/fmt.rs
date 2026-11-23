//! `perasc fmt`, decision 0041's prototypes: `PERAS_PROTO_FMT=f1` or `f2`
//! picks the formatter; without it the command says it is a prototype.

use std::io::{self, Write};

use perasc_syntax::format::{Style, format};

use crate::{ERRORS_FOUND, SUCCESS, USAGE_OR_IO};

/// Formats each file in place, or, with `check`, names those that would
/// change. A file with a syntax error is left as it is.
pub fn fmt(
    files: &[String],
    check: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    let style = match std::env::var("PERAS_PROTO_FMT").as_deref() {
        Ok("f1") => Style::Tokens,
        Ok("f2") => Style::Width,
        _ => {
            writeln!(
                err,
                "perasc fmt is a prototype of decision 0041: set PERAS_PROTO_FMT to f1 or f2"
            )?;
            return Ok(USAGE_OR_IO);
        }
    };
    let mut status = SUCCESS;
    for file in files {
        let Ok(source) = std::fs::read_to_string(file) else {
            writeln!(err, "{file}: cannot be read")?;
            status = USAGE_OR_IO;
            continue;
        };
        match format(&source, style) {
            Err(errors) => {
                writeln!(err, "{file}: refused, {} syntax errors", errors.len())?;
                status = status.max(ERRORS_FOUND);
            }
            Ok(text) if text == source => {}
            Ok(text) => {
                if check {
                    writeln!(out, "{file}")?;
                    status = status.max(ERRORS_FOUND);
                } else if std::fs::write(file, text).is_err() {
                    writeln!(err, "{file}: cannot be written")?;
                    status = USAGE_OR_IO;
                }
            }
        }
    }
    // For scripts/bench-fmt.mjs: the peak of memory, as Linux counts it.
    if std::env::var_os("PERAS_FMT_PEAK").is_some()
        && let Ok(status) = std::fs::read_to_string("/proc/self/status")
        && let Some(line) = status.lines().find(|l| l.starts_with("VmHWM:"))
    {
        writeln!(err, "{line}")?;
    }
    Ok(status)
}
