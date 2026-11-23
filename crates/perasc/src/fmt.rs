//! `perasc fmt` (decision 0041): each file printed at a width of 100.

use std::io::{self, Write};
use std::path::Path;

use perasc_diag::Message;
use perasc_diag::i18n::Localizer;
use perasc_driver::{ReadError, read_source};
use perasc_syntax::format::format;

use crate::{ERRORS_FOUND, SUCCESS, USAGE_OR_IO, error_line};

/// Formats each file in place, or, with `check`, names those that would
/// change. A file with a syntax error is left as it is.
pub fn fmt(
    files: &[String],
    check: bool,
    localizer: &Localizer,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    let mut status = SUCCESS;
    for path in files {
        let source = match read_source(Path::new(path)) {
            Ok(source) => source,
            Err(problem) => {
                let message = match problem {
                    ReadError::NotASource => Message::new("cli-not-a-source"),
                    ReadError::NotFound => Message::new("cli-not-found"),
                    ReadError::NotUtf8 => Message::new("cli-not-utf8"),
                    ReadError::TooLarge(_) => Message::new("cli-fmt-too-large"),
                    ReadError::Io(e) => {
                        Message::new("cli-read-error").text("reason", e.to_string())
                    }
                };
                let message = message.text("path", path.clone());
                writeln!(err, "{}", error_line(localizer, &message))?;
                status = USAGE_OR_IO;
                continue;
            }
        };
        match format(&source) {
            Err(errors) => {
                let message = Message::new("cli-fmt-refused")
                    .text("path", path.clone())
                    .number("count", i64::try_from(errors.len()).unwrap_or(i64::MAX));
                writeln!(err, "{}", error_line(localizer, &message))?;
                status = status.max(ERRORS_FOUND);
            }
            Ok(text) if text == source => {}
            Ok(text) => {
                if check {
                    writeln!(out, "{path}")?;
                    status = status.max(ERRORS_FOUND);
                } else if let Err(e) = std::fs::write(path, text) {
                    let message = Message::new("cli-fmt-write-error")
                        .text("path", path.clone())
                        .text("reason", e.to_string());
                    writeln!(err, "{}", error_line(localizer, &message))?;
                    status = USAGE_OR_IO;
                }
            }
        }
    }
    Ok(status)
}
