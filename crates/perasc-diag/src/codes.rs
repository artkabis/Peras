//! The error code registry: every code ever issued, with its title message.
//!
//! Codes are permanent. A retired code stays here and keeps its documentation
//! pages (`docs/en/errors/Exxxx.md`, `docs/fr/errors/Exxxx.md`), marked as retired.
//! Lexical errors use `E0001` to `E0099`.

use crate::ErrorCode;

/// A character that cannot appear in Peras source outside strings and comments.
pub const INVALID_CHARACTER: ErrorCode = ErrorCode::new(1);
/// A string literal with no closing quote on its line.
pub const UNTERMINATED_STRING: ErrorCode = ErrorCode::new(2);
/// A `/*` comment with no closing `*/`.
pub const UNTERMINATED_BLOCK_COMMENT: ErrorCode = ErrorCode::new(3);
/// An unknown or malformed escape sequence in a string.
pub const INVALID_ESCAPE: ErrorCode = ErrorCode::new(4);
/// A number literal that does not follow the number syntax.
pub const MALFORMED_NUMBER: ErrorCode = ErrorCode::new(5);
/// The JavaScript strict equality operators `===` and `!==`.
pub const STRICT_EQUALITY: ErrorCode = ErrorCode::new(6);
/// A Unicode bidirectional control character, which can hide code (Trojan Source).
pub const BIDI_CONTROL_CHARACTER: ErrorCode = ErrorCode::new(7);
/// A source file of 4 GiB or more.
pub const SOURCE_TOO_LARGE: ErrorCode = ErrorCode::new(8);

/// One entry of the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    /// The code.
    pub code: ErrorCode,
    /// Fluent ID of its title, also the prefix of its other messages.
    pub message_id: &'static str,
    /// Whether the compiler no longer emits it.
    pub retired: bool,
}

/// Every code issued so far, in numeric order.
pub const REGISTRY: &[CodeInfo] = &[
    active(INVALID_CHARACTER, "lex-invalid-character"),
    active(UNTERMINATED_STRING, "lex-unterminated-string"),
    active(UNTERMINATED_BLOCK_COMMENT, "lex-unterminated-block-comment"),
    active(INVALID_ESCAPE, "lex-invalid-escape"),
    active(MALFORMED_NUMBER, "lex-malformed-number"),
    active(STRICT_EQUALITY, "lex-strict-equality"),
    active(BIDI_CONTROL_CHARACTER, "lex-bidi-control"),
    active(SOURCE_TOO_LARGE, "lex-source-too-large"),
];

const fn active(code: ErrorCode, message_id: &'static str) -> CodeInfo {
    CodeInfo {
        code,
        message_id,
        retired: false,
    }
}

/// Looks up a code in the registry.
#[must_use]
pub fn info(code: ErrorCode) -> Option<&'static CodeInfo> {
    REGISTRY.iter().find(|entry| entry.code == code)
}

#[cfg(test)]
mod tests {
    use super::REGISTRY;
    use crate::messages::{Language, has_message};
    use std::path::Path;

    #[test]
    fn codes_are_unique_and_in_order() {
        for pair in REGISTRY.windows(2) {
            if let [a, b] = pair {
                assert!(a.code < b.code, "{} must come before {}", a.code, b.code);
            }
        }
    }

    #[test]
    fn every_code_has_messages_in_both_languages() {
        for entry in REGISTRY {
            for language in Language::ALL {
                assert!(
                    has_message(language, entry.message_id),
                    "{}: message `{}` missing in {language:?}",
                    entry.code,
                    entry.message_id
                );
            }
        }
    }

    #[test]
    fn every_code_has_documentation_in_both_languages() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
        for entry in REGISTRY {
            for language in ["en", "fr"] {
                let page = docs
                    .join(language)
                    .join("errors")
                    .join(format!("{}.md", entry.code));
                assert!(page.is_file(), "missing page {}", page.display());
            }
        }
    }
}
