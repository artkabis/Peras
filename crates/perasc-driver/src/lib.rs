//! Compilation pipeline, configuration and cargo/wasm-bindgen calls for the Peras compiler.
//!
//! For now the pipeline is the lexer; each new stage plugs into [`check`].

use std::fs;
use std::io;
use std::path::Path;

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
#[must_use]
pub fn check(source: &str) -> Checked {
    Checked::capped(perasc_syntax::lex(source).diagnostics)
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
