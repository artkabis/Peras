//! The error code registry: every code ever issued, with its title message.
//!
//! Codes are permanent. A retired code stays here and keeps its documentation
//! pages (`docs/en/errors/Exxxx.md`, `docs/fr/errors/Exxxx.md`), marked as retired.
//! Lexical errors use `E0001` to `E0099`, syntax errors `E0100` to `E0199`, name
//! and type errors `E0200` to `E0299`. `E0204` is reserved for "value used after
//! move", the example of the language spec. `E0900` to `E0999` are limits of the
//! compiler itself: valid programs it cannot compile yet.

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
/// A name that is not declared, or not visible here.
pub const UNDEFINED_NAME: ErrorCode = ErrorCode::new(200);
/// A declaration hiding another one of an enclosing scope.
pub const SHADOWING: ErrorCode = ErrorCode::new(201);
/// The same name declared twice in one scope.
pub const DUPLICATE_DECLARATION: ErrorCode = ErrorCode::new(202);
/// A variable or parameter whose value is never read.
pub const UNUSED: ErrorCode = ErrorCode::new(203);
/// A variable used after its heap value moved out of it.
pub const USE_AFTER_MOVE: ErrorCode = ErrorCode::new(204);
/// An assignment to a `const`, a `for…of` constant or a read-only parameter.
pub const ASSIGN_TO_IMMUTABLE: ErrorCode = ErrorCode::new(205);
/// An `inout` argument without `&`, or `&` on an argument that is not `inout`.
pub const INOUT_MARKER: ErrorCode = ErrorCode::new(206);
/// The same variable reached by two arguments of a call, one of them `inout`.
pub const ALIASING: ErrorCode = ErrorCode::new(207);
/// A heap value moved out of what only borrows it: a parameter, a field…
pub const MOVE_OUT_OF_BORROW: ErrorCode = ErrorCode::new(209);
/// A value of one type where another is expected.
pub const MISMATCHED_TYPES: ErrorCode = ErrorCode::new(210);
/// An integer literal where a float is expected.
pub const INTEGER_FOR_FLOAT: ErrorCode = ErrorCode::new(211);
/// A literal that does not fit its type.
pub const LITERAL_OUT_OF_RANGE: ErrorCode = ErrorCode::new(212);
/// A type name that does not exist.
pub const UNKNOWN_TYPE: ErrorCode = ErrorCode::new(213);
/// A call with too many or too few arguments.
pub const ARGUMENT_COUNT: ErrorCode = ErrorCode::new(214);
/// A field or method that the type does not have.
pub const UNKNOWN_MEMBER: ErrorCode = ErrorCode::new(215);
/// A generic type with the wrong number of type arguments.
pub const TYPE_ARGUMENT_COUNT: ErrorCode = ErrorCode::new(216);
/// A condition that is not a `bool`: Peras has no truthiness.
pub const CONDITION_NOT_BOOL: ErrorCode = ErrorCode::new(217);
/// `return;` in a function with a value, or `return x;` in a `void` one.
pub const RETURN_MISMATCH: ErrorCode = ErrorCode::new(218);
/// A function with a return type that can end without returning.
pub const MISSING_RETURN: ErrorCode = ErrorCode::new(219);
/// An exported function, or one returning a value, without a return type.
pub const MISSING_RETURN_TYPE: ErrorCode = ErrorCode::new(220);
/// An operator applied to a type that does not support it.
pub const INVALID_OPERATOR: ErrorCode = ErrorCode::new(221);
/// A `match` that does not cover every value of its scrutinee.
pub const NON_EXHAUSTIVE_MATCH: ErrorCode = ErrorCode::new(222);
/// A variant that the enumeration does not have.
pub const UNKNOWN_VARIANT: ErrorCode = ErrorCode::new(223);
/// A `match` arm that no value can reach.
pub const UNREACHABLE_ARM: ErrorCode = ErrorCode::new(224);
/// `?` in a function that does not return what `?` gives back early.
pub const TRY_RETURN: ErrorCode = ErrorCode::new(225);
/// A constraint name that does not exist.
pub const UNKNOWN_CONSTRAINT: ErrorCode = ErrorCode::new(226);
/// A type argument that does not meet a constraint of its parameter.
pub const CONSTRAINT_NOT_MET: ErrorCode = ErrorCode::new(227);
/// An operation on a type parameter whose constraints do not allow it.
pub const MISSING_CONSTRAINT: ErrorCode = ErrorCode::new(228);
/// A type argument that nothing determines.
pub const CANNOT_INFER: ErrorCode = ErrorCode::new(229);
/// An exported generic function.
pub const GENERIC_EXPORT: ErrorCode = ErrorCode::new(230);
/// Generic code that instantiates itself with ever larger types.
pub const ENDLESS_INSTANCES: ErrorCode = ErrorCode::new(231);
/// A type alias that contains itself.
pub const ALIAS_CYCLE: ErrorCode = ErrorCode::new(232);
/// A module constant whose value is not known when compiling.
pub const NOT_CONSTANT: ErrorCode = ErrorCode::new(233);
/// Methods for a type that cannot have them.
pub const NO_METHODS: ErrorCode = ErrorCode::new(234);
/// An implementation that misses methods of its interface, or adds others.
pub const IMPL_MISMATCH: ErrorCode = ErrorCode::new(235);
/// A method whose signature is not the one its interface declares.
pub const SIGNATURE_MISMATCH: ErrorCode = ErrorCode::new(236);
/// An interface used as a type.
pub const INTERFACE_AS_TYPE: ErrorCode = ErrorCode::new(237);
/// An arrow function where no method takes one, or a value where one is needed.
pub const ARROW_FUNCTION: ErrorCode = ErrorCode::new(238);
/// A template literal is given a value it cannot write as text yet.
pub const TEMPLATE_VALUE: ErrorCode = ErrorCode::new(239);
/// A map's key or a set's element of a type that cannot be one yet.
pub const MAP_KEY: ErrorCode = ErrorCode::new(240);
/// A `parallel for` whose turns could see each other.
pub const PARALLEL: ErrorCode = ErrorCode::new(241);
/// Something only a managed module (`.apeiron`) has, in an owned one.
pub const MANAGED_ONLY: ErrorCode = ErrorCode::new(242);
/// Something only an owned module (`.peras`) has, in a managed one.
pub const OWNED_ONLY: ErrorCode = ErrorCode::new(243);
/// An `import` that names a missing module, function, or signature.
pub const IMPORT: ErrorCode = ErrorCode::new(244);
/// A resource kept, given away or used as its owner does not allow.
pub const RESOURCE: ErrorCode = ErrorCode::new(245);
/// A property of the browser that can only be read, assigned.
pub const READ_ONLY: ErrorCode = ErrorCode::new(246);
/// A declaration of an `extern` block that cannot be kept.
pub const EXTERN: ErrorCode = ErrorCode::new(247);
/// A valid construct that code generation does not support yet.
pub const NOT_SUPPORTED_YET: ErrorCode = ErrorCode::new(900);

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
    active!(UNDEFINED_NAME, "check-undefined", "E0200"),
    active!(SHADOWING, "check-shadowing", "E0201"),
    active!(DUPLICATE_DECLARATION, "check-duplicate", "E0202"),
    active!(UNUSED, "check-unused", "E0203"),
    active!(USE_AFTER_MOVE, "check-moved", "E0204"),
    active!(ASSIGN_TO_IMMUTABLE, "check-immutable", "E0205"),
    active!(INOUT_MARKER, "check-inout-marker", "E0206"),
    active!(ALIASING, "check-alias", "E0207"),
    active!(MOVE_OUT_OF_BORROW, "check-move-out", "E0209"),
    active!(MISMATCHED_TYPES, "check-mismatch", "E0210"),
    active!(INTEGER_FOR_FLOAT, "check-integer-for-float", "E0211"),
    active!(LITERAL_OUT_OF_RANGE, "check-out-of-range", "E0212"),
    active!(UNKNOWN_TYPE, "check-unknown-type", "E0213"),
    active!(ARGUMENT_COUNT, "check-argument-count", "E0214"),
    active!(UNKNOWN_MEMBER, "check-unknown-member", "E0215"),
    active!(TYPE_ARGUMENT_COUNT, "check-type-argument-count", "E0216"),
    active!(CONDITION_NOT_BOOL, "check-condition", "E0217"),
    active!(RETURN_MISMATCH, "check-return", "E0218"),
    active!(MISSING_RETURN, "check-missing-return", "E0219"),
    active!(MISSING_RETURN_TYPE, "check-missing-return-type", "E0220"),
    active!(INVALID_OPERATOR, "check-operator", "E0221"),
    active!(NON_EXHAUSTIVE_MATCH, "check-match-missing", "E0222"),
    active!(UNKNOWN_VARIANT, "check-unknown-variant", "E0223"),
    active!(UNREACHABLE_ARM, "check-unreachable-arm", "E0224"),
    active!(TRY_RETURN, "check-try-return", "E0225"),
    active!(UNKNOWN_CONSTRAINT, "check-unknown-constraint", "E0226"),
    active!(CONSTRAINT_NOT_MET, "check-constraint", "E0227"),
    active!(MISSING_CONSTRAINT, "check-missing-constraint", "E0228"),
    active!(CANNOT_INFER, "check-cannot-infer", "E0229"),
    active!(GENERIC_EXPORT, "check-generic-export", "E0230"),
    active!(ENDLESS_INSTANCES, "check-endless-instances", "E0231"),
    active!(ALIAS_CYCLE, "check-alias-cycle", "E0232"),
    active!(NOT_CONSTANT, "check-not-constant", "E0233"),
    active!(NO_METHODS, "check-no-methods", "E0234"),
    active!(IMPL_MISMATCH, "check-impl-mismatch", "E0235"),
    active!(SIGNATURE_MISMATCH, "check-signature", "E0236"),
    active!(INTERFACE_AS_TYPE, "check-interface-as-type", "E0237"),
    active!(ARROW_FUNCTION, "check-arrow", "E0238"),
    active!(TEMPLATE_VALUE, "check-template", "E0239"),
    active!(MAP_KEY, "check-map-key", "E0240"),
    active!(PARALLEL, "check-parallel", "E0241"),
    active!(MANAGED_ONLY, "check-managed-only", "E0242"),
    active!(OWNED_ONLY, "check-owned-only", "E0243"),
    active!(IMPORT, "check-import", "E0244"),
    active!(RESOURCE, "check-resource", "E0245"),
    active!(READ_ONLY, "check-read-only", "E0246"),
    active!(EXTERN, "check-extern", "E0247"),
    active!(NOT_SUPPORTED_YET, "gen-unsupported", "E0900"),
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
