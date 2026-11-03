//! Lexer, parser and AST for Peras source files.
//!
//! The lexer ([`lex`]) turns a file into [`Token`]s and reports every lexical error
//! in one pass. The hand-written parser arrives on days 4 and 5.

use std::path::Path;

pub mod lexer;
pub mod token;

pub use lexer::{Lexed, lex, source_too_large, string_value};
pub use token::{Token, TokenKind};

/// The mode of a source module, set by its file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `.peras`: owned values, linear memory, no garbage collector.
    Owned,
    /// `.apeiron`: managed values, compiled to JavaScript, garbage collected.
    Managed,
}

impl Mode {
    /// Extension of owned modules, without the leading dot.
    pub const OWNED_EXTENSION: &'static str = "peras";
    /// Extension of managed modules, without the leading dot.
    pub const MANAGED_EXTENSION: &'static str = "apeiron";

    /// Returns the mode for a path, or `None` if it is not a Peras source file.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            Self::OWNED_EXTENSION => Some(Self::Owned),
            Self::MANAGED_EXTENSION => Some(Self::Managed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mode;
    use std::path::Path;

    #[test]
    fn extension_sets_the_mode() {
        assert_eq!(
            Mode::from_path(Path::new("src/dsp.peras")),
            Some(Mode::Owned)
        );
        assert_eq!(
            Mode::from_path(Path::new("src/ui.apeiron")),
            Some(Mode::Managed)
        );
    }

    #[test]
    fn other_files_are_not_sources() {
        assert_eq!(Mode::from_path(Path::new("src/main.ts")), None);
        assert_eq!(Mode::from_path(Path::new("README")), None);
        assert_eq!(Mode::from_path(Path::new("src/ui.PERAS")), None);
    }
}
