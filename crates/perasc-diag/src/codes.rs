//! The error code registry: every code ever issued, with its title message.
//!
//! Codes are permanent. A retired code stays here and keeps its documentation
//! pages (`docs/en/errors/Exxxx.md`, `docs/fr/errors/Exxxx.md`), marked as retired.
//! Lexical errors use `E0001` to `E0099`, syntax errors `E0100` to `E0199`.

use crate::ErrorCode;
use crate::messages::Language;

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
/// A value was expected: a literal, a name, a call, a calculation.
pub const EXPECTED_EXPRESSION: ErrorCode = ErrorCode::new(100);
/// A statement does not end with `;`.
pub const MISSING_SEMICOLON: ErrorCode = ErrorCode::new(101);
/// A name was expected: of a function, variable, parameter, field or attribute.
pub const EXPECTED_NAME: ErrorCode = ErrorCode::new(102);
/// A `(`, `[`, `{` or `<` without its closing delimiter.
pub const UNCLOSED_DELIMITER: ErrorCode = ErrorCode::new(103);
/// A token that does not fit the construct being parsed.
pub const UNEXPECTED_TOKEN: ErrorCode = ErrorCode::new(104);
/// A type was expected after `:` or in `<…>`.
pub const EXPECTED_TYPE: ErrorCode = ErrorCode::new(105);
/// `var` or `function`, which Peras replaces with `let` and `fn`.
pub const REMOVED_KEYWORD: ErrorCode = ErrorCode::new(106);
/// A number literal that does not fit in 64 bits.
pub const NUMBER_TOO_LARGE: ErrorCode = ErrorCode::new(107);
/// `a < b < c`: comparisons cannot be chained.
pub const CHAINED_COMPARISON: ErrorCode = ErrorCode::new(108);
/// `a & b == c`: bitwise operators and comparisons need parentheses.
pub const AMBIGUOUS_PRECEDENCE: ErrorCode = ErrorCode::new(109);
/// Something other than a declaration at the top level of a module.
pub const EXPECTED_ITEM: ErrorCode = ErrorCode::new(110);
/// Expressions or types nested deeper than the parser allows.
pub const NESTING_TOO_DEEP: ErrorCode = ErrorCode::new(111);
/// A `const` declared without a value.
pub const CONST_WITHOUT_VALUE: ErrorCode = ErrorCode::new(112);
/// `=` or `+=` used inside an expression.
pub const ASSIGNMENT_IN_EXPRESSION: ErrorCode = ErrorCode::new(113);
/// An assignment to something that is not a variable, field or element.
pub const INVALID_ASSIGNMENT_TARGET: ErrorCode = ErrorCode::new(114);
/// `if x {`: a condition without parentheses.
pub const CONDITION_WITHOUT_PARENTHESES: ErrorCode = ErrorCode::new(115);
/// `for (k in obj)`: Peras iterates values with `for…of`.
pub const FOR_IN: ErrorCode = ErrorCode::new(116);
/// `break` or `continue` outside a loop.
pub const JUMP_OUTSIDE_LOOP: ErrorCode = ErrorCode::new(117);
/// `name?: T`: optional values are written `Option<T>`.
pub const OPTIONAL_MARKER: ErrorCode = ErrorCode::new(118);
/// `if (c);`: a `;` that empties the body of an `if` or a loop.
pub const EMPTY_BODY: ErrorCode = ErrorCode::new(119);

/// One entry of the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    /// The code.
    pub code: ErrorCode,
    /// Fluent ID of its title, also the prefix of its other messages.
    pub message_id: &'static str,
    /// Whether the compiler no longer emits it.
    pub retired: bool,
    /// Its documentation page in English, shown by `perasc explain`.
    pub docs_en: &'static str,
    /// Its documentation page in French.
    pub docs_fr: &'static str,
}

impl CodeInfo {
    /// The documentation page in `language`.
    #[must_use]
    pub const fn docs(&self, language: Language) -> &'static str {
        match language {
            Language::English => self.docs_en,
            Language::French => self.docs_fr,
        }
    }
}

/// A registry entry. The pages are embedded at compile time, so a missing page
/// fails the build.
macro_rules! active {
    ($code:expr, $id:literal, $number:literal) => {
        CodeInfo {
            code: $code,
            message_id: $id,
            retired: false,
            docs_en: include_str!(concat!("../../../docs/en/errors/", $number, ".md")),
            docs_fr: include_str!(concat!("../../../docs/fr/errors/", $number, ".md")),
        }
    };
}

/// Every code issued so far, in numeric order.
pub const REGISTRY: &[CodeInfo] = &[
    active!(INVALID_CHARACTER, "lex-invalid-character", "E0001"),
    active!(UNTERMINATED_STRING, "lex-unterminated-string", "E0002"),
    active!(
        UNTERMINATED_BLOCK_COMMENT,
        "lex-unterminated-block-comment",
        "E0003"
    ),
    active!(INVALID_ESCAPE, "lex-invalid-escape", "E0004"),
    active!(MALFORMED_NUMBER, "lex-malformed-number", "E0005"),
    active!(STRICT_EQUALITY, "lex-strict-equality", "E0006"),
    active!(BIDI_CONTROL_CHARACTER, "lex-bidi-control", "E0007"),
    active!(SOURCE_TOO_LARGE, "lex-source-too-large", "E0008"),
    active!(EXPECTED_EXPRESSION, "parse-expected-expression", "E0100"),
    active!(MISSING_SEMICOLON, "parse-missing-semicolon", "E0101"),
    active!(EXPECTED_NAME, "parse-expected-name", "E0102"),
    active!(UNCLOSED_DELIMITER, "parse-unclosed", "E0103"),
    active!(UNEXPECTED_TOKEN, "parse-unexpected", "E0104"),
    active!(EXPECTED_TYPE, "parse-expected-type", "E0105"),
    active!(REMOVED_KEYWORD, "parse-removed-keyword", "E0106"),
    active!(NUMBER_TOO_LARGE, "parse-number-too-large", "E0107"),
    active!(CHAINED_COMPARISON, "parse-chained-comparison", "E0108"),
    active!(AMBIGUOUS_PRECEDENCE, "parse-bitwise-mix", "E0109"),
    active!(EXPECTED_ITEM, "parse-expected-item", "E0110"),
    active!(NESTING_TOO_DEEP, "parse-too-deep", "E0111"),
    active!(CONST_WITHOUT_VALUE, "parse-const-without-value", "E0112"),
    active!(
        ASSIGNMENT_IN_EXPRESSION,
        "parse-assignment-in-expression",
        "E0113"
    ),
    active!(INVALID_ASSIGNMENT_TARGET, "parse-invalid-target", "E0114"),
    active!(
        CONDITION_WITHOUT_PARENTHESES,
        "parse-condition-parentheses",
        "E0115"
    ),
    active!(FOR_IN, "parse-for-in", "E0116"),
    active!(JUMP_OUTSIDE_LOOP, "parse-jump-outside-loop", "E0117"),
    active!(OPTIONAL_MARKER, "parse-optional-marker", "E0118"),
    active!(EMPTY_BODY, "parse-empty-body", "E0119"),
];

/// Looks up a code in the registry.
#[must_use]
pub fn info(code: ErrorCode) -> Option<&'static CodeInfo> {
    REGISTRY.iter().find(|entry| entry.code == code)
}

/// Parses `E0001`, `e0001` or `1` into a registered code.
#[must_use]
pub fn parse(text: &str) -> Option<&'static CodeInfo> {
    let digits = text.strip_prefix(['E', 'e']).unwrap_or(text);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number = digits.parse::<u16>().ok()?;
    info(ErrorCode::new(number))
}

#[cfg(test)]
mod tests {
    use super::{REGISTRY, parse};
    use crate::messages::{Language, has_message};

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
    fn every_page_is_titled_with_its_code() {
        for entry in REGISTRY {
            for language in Language::ALL {
                let title = entry.docs(language).lines().next().unwrap_or("");
                let expected = format!("# {}", entry.code);
                assert!(title.starts_with(&expected), "{}: {title}", entry.code);
            }
        }
    }

    #[test]
    fn codes_parse_in_any_case() {
        assert_eq!(parse("E0006").map(|e| e.code), Some(super::STRICT_EQUALITY));
        assert_eq!(parse("e6").map(|e| e.code), Some(super::STRICT_EQUALITY));
        assert_eq!(parse("6").map(|e| e.code), Some(super::STRICT_EQUALITY));
        assert_eq!(parse("E9999"), None);
        assert_eq!(parse("E"), None);
        assert_eq!(parse("E+1"), None);
    }
}
