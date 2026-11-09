use super::{lex, lex_bounded, string_value};
use crate::token::TokenKind::{self, *};
use perasc_diag::messages::{Language, has_message};
use perasc_diag::{Diagnostic, ErrorCode, codes};

fn kinds(source: &str) -> Vec<TokenKind> {
    lex(source).tokens.iter().map(|t| t.kind).collect()
}

fn codes_of(source: &str) -> Vec<ErrorCode> {
    lex(source).diagnostics.iter().map(|d| d.code).collect()
}

fn only(source: &str) -> Diagnostic {
    let lexed = lex(source);
    assert_eq!(
        lexed.diagnostics.len(),
        1,
        "{source:?}: {:#?}",
        lexed.diagnostics
    );
    lexed
        .diagnostics
        .into_iter()
        .next()
        .unwrap_or_else(|| unreachable!())
}

/// Applies a diagnostic's edits to the source, last edit first.
fn apply_fix(source: &str, diagnostic: &Diagnostic) -> std::string::String {
    let mut fixed = source.to_owned();
    let mut edits = diagnostic.fix.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.span.start));
    for e in edits {
        let range = super::index(e.span.start)..super::index(e.span.end);
        fixed.replace_range(range, &e.replacement);
    }
    fixed
}

#[test]
fn lexes_a_function() {
    let source = "export fn gain(buf: inout Buffer, factor: f32): void {\n  buf.x *= factor;\n}";
    assert_eq!(
        kinds(source),
        [
            Export, Fn, Ident, LParen, Ident, Colon, Inout, Ident, Comma, Ident, Colon, Ident,
            RParen, Colon, Ident, LBrace, Ident, Dot, Ident, StarEq, Ident, Semicolon, RBrace, Eof
        ]
    );
    assert_eq!(lex(source).diagnostics, []);
}

#[test]
fn keywords_need_the_whole_word() {
    assert_eq!(kinds("fn fnx _fn match"), [Fn, Ident, Ident, Match, Eof]);
    // Contextual words and removed TypeScript words stay identifiers.
    assert_eq!(
        kinds("from of as test var null this"),
        [Ident; 7].iter().copied().chain([Eof]).collect::<Vec<_>>()
    );
}

#[test]
fn operators_take_the_longest_match() {
    assert_eq!(
        kinds("= == => ! != < <= > >= + += ++ - -= -- & && | || ? @ ^ ~ % %= / /="),
        [
            Eq, EqEq, FatArrow, Bang, BangEq, Lt, LtEq, Gt, GtEq, Plus, PlusEq, PlusPlus, Minus,
            MinusEq, MinusMinus, Amp, AmpAmp, Pipe, PipePipe, Question, At, Caret, Tilde, Percent,
            PercentEq, Slash, SlashEq, Eof
        ]
    );
}

#[test]
fn nested_generics_close_with_single_tokens() {
    assert_eq!(
        kinds("Array<Option<f32>>"),
        [Ident, Lt, Ident, Lt, Ident, Gt, Gt, Eof]
    );
}

#[test]
fn comments_and_whitespace_are_skipped() {
    assert_eq!(kinds("a // line\r\n/* block\n * */ b"), [Ident, Ident, Eof]);
    assert_eq!(kinds("/**/"), [Eof]);
}

#[test]
fn valid_numbers() {
    for (source, kind) in [
        ("0", Int),
        ("42", Int),
        ("1_000_000", Int),
        ("0xFF", Int),
        ("0xdead_beef", Int),
        ("0o755", Int),
        ("0b1010_0101", Int),
        ("0.5", Float),
        ("160.0", Float),
        ("1e3", Float),
        ("2.5e-4", Float),
        ("1E+10", Float),
        ("3_000.141_592", Float),
        ("0xe", Int),
    ] {
        assert_eq!(kinds(source), [kind, Eof], "{source}");
        assert!(lex(source).diagnostics.is_empty(), "{source}");
    }
}

#[test]
fn member_access_on_a_number_is_not_a_fraction() {
    assert_eq!(
        kinds("1.toString()"),
        [Int, Dot, Ident, LParen, RParen, Eof]
    );
}

#[test]
fn malformed_numbers_explain_why_and_fix_when_certain() {
    for (source, label, fixed) in [
        ("0x", "lex-number-no-digits-label", None),
        ("0b102", "lex-number-invalid-digit-label", None),
        ("0o8", "lex-number-invalid-digit-label", None),
        ("0xFG", "lex-number-invalid-digit-label", None),
        ("012", "lex-number-leading-zero-label", None),
        ("089", "lex-number-leading-zero-label", Some("89")),
        ("00.5", "lex-number-leading-zero-label", Some("0.5")),
        ("1__000", "lex-number-underscore-label", Some("1_000")),
        ("1_", "lex-number-underscore-label", Some("1")),
        ("0x_FF", "lex-number-underscore-label", Some("0xFF")),
        ("1_.5", "lex-number-underscore-label", Some("1.5")),
        ("1e", "lex-number-exponent-label", None),
        ("1e+", "lex-number-exponent-label", None),
        ("10px", "lex-number-suffix-label", Some("10")),
        ("5f32", "lex-number-suffix-label", Some("5")),
        ("1em", "lex-number-suffix-label", Some("1")),
        ("0x1.5", "lex-number-radix-fraction-label", None),
    ] {
        let d = only(source);
        assert_eq!(d.code, codes::MALFORMED_NUMBER, "{source}");
        assert_eq!(
            d.labels.first().map(|l| l.message.id),
            Some(label),
            "{source}"
        );
        assert_eq!(kinds(source), [Error, Eof], "{source}");
        match fixed {
            Some(expected) => assert_eq!(apply_fix(source, &d), expected, "{source}"),
            None => assert!(d.fix.edits.is_empty(), "{source}"),
        }
    }
}

#[test]
fn strings_and_their_values() {
    assert_eq!(
        kinds(r#""a" 'b' "it's" 'say "hi"'"#),
        [String, String, String, String, Eof]
    );
    assert_eq!(string_value(r#""a\n\t\\\"b""#), "a\n\t\\\"b");
    assert_eq!(string_value(r"'\u{1F600}\u{e9}'"), "\u{1F600}\u{e9}");
    assert_eq!(string_value("\"\""), "");
}

#[test]
fn invalid_escapes_keep_the_string_and_suggest_a_fix() {
    for (source, label, fixed) in [
        (
            r#""\d""#,
            "lex-invalid-escape-unknown-label",
            Some(r#""\\d""#),
        ),
        (
            r#""\u0041""#,
            "lex-invalid-escape-braces-label",
            Some(r#""\u{0041}""#),
        ),
        (
            r#""\x41""#,
            "lex-invalid-escape-braces-label",
            Some(r#""\u{41}""#),
        ),
        (r#""\u{110000}""#, "lex-invalid-escape-unicode-label", None),
        (r#""\u{D800}""#, "lex-invalid-escape-unicode-label", None),
        (r#""\u{}""#, "lex-invalid-escape-unicode-label", None),
        (r#""\u{41""#, "lex-invalid-escape-unicode-label", None),
        (r#""\x4""#, "lex-invalid-escape-unicode-label", None),
    ] {
        let d = only(source);
        assert_eq!(d.code, codes::INVALID_ESCAPE, "{source}");
        assert_eq!(
            d.labels.first().map(|l| l.message.id),
            Some(label),
            "{source}"
        );
        assert_eq!(kinds(source), [String, Eof], "{source}");
        if let Some(expected) = fixed {
            assert_eq!(apply_fix(source, &d), expected, "{source}");
            assert!(lex(expected).diagnostics.is_empty(), "{expected}");
        }
    }
}

#[test]
fn every_invalid_escape_in_a_string_is_reported() {
    assert_eq!(
        codes_of(r#""\a \b \c""#),
        [
            codes::INVALID_ESCAPE,
            codes::INVALID_ESCAPE,
            codes::INVALID_ESCAPE
        ]
    );
}

#[test]
fn an_unterminated_string_stops_at_the_end_of_its_line() {
    let source = "let s = \"abc\r\nlet t = 1;";
    let lexed = lex(source);
    assert_eq!(
        lexed.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
        [Let, Ident, Eq, String, Let, Ident, Eq, Int, Semicolon, Eof]
    );
    let d = only(source);
    assert_eq!(d.code, codes::UNTERMINATED_STRING);
    assert_eq!(apply_fix(source, &d), "let s = \"abc\"\r\nlet t = 1;");
}

#[test]
fn an_unterminated_string_closes_before_a_trailing_semicolon() {
    for (source, fixed) in [
        ("f(\"abc);", "f(\"abc\");"),
        ("let s = 'abc;  ", "let s = 'abc';  "),
        ("let s = 'a b", "let s = 'a b'"),
        ("\";", "\";\""),
    ] {
        assert_eq!(apply_fix(source, &only(source)), fixed, "{source}");
    }
}

#[test]
fn a_leading_zero_on_octal_digits_is_ambiguous() {
    let d = only("0755");
    assert_eq!(d.fix.edits, []);
    assert_eq!(d.fix.message.id, "lex-number-leading-zero-fix-ambiguous");
}

#[test]
fn a_backslash_at_the_end_of_a_line_does_not_continue_a_string() {
    assert_eq!(codes_of("'abc\\\nx"), [codes::UNTERMINATED_STRING]);
    assert_eq!(kinds("'abc\\\nx"), [String, Ident, Eof]);
}

#[test]
fn an_unterminated_block_comment_swallows_the_rest_of_the_file() {
    let source = "a /* never closed\nb c";
    assert_eq!(kinds(source), [Ident, Eof]);
    let d = only(source);
    assert_eq!(d.code, codes::UNTERMINATED_BLOCK_COMMENT);
    assert_eq!(apply_fix(source, &d), "a /* never closed\nb c*/");
}

#[test]
fn strict_equality_becomes_equality() {
    assert_eq!(
        kinds("a === b !== c"),
        [Ident, EqEq, Ident, BangEq, Ident, Eof]
    );
    let d = only("a === b");
    assert_eq!(d.code, codes::STRICT_EQUALITY);
    assert_eq!(apply_fix("a === b", &d), "a == b");
}

#[test]
fn invalid_characters_are_merged_when_they_share_a_fix() {
    let lexed = lex("a #€ b");
    assert_eq!(lexed.diagnostics.len(), 1);
    let d = &lexed.diagnostics[0];
    assert_eq!(d.code, codes::INVALID_CHARACTER);
    assert_eq!(apply_fix("a #€ b", d), "a  b");
    assert!(lexed.tokens.iter().any(|t| t.kind == Error));
}

#[test]
fn look_alike_characters_are_replaced() {
    for (source, fixed) in [
        ("x = \u{201C}hi\u{201D};", "x = \"hi\u{201D};"),
        ("a \u{2212} b", "a - b"),
        ("a\u{00A0}= 1", "a = 1"),
        ("a \u{2260} b", "a != b"),
        ("x\u{037E}", "x;"),
    ] {
        let first = lex(source).diagnostics.into_iter().next();
        let Some(d) = first else {
            unreachable!("{source}: no diagnostic")
        };
        assert_eq!(d.code, codes::INVALID_CHARACTER, "{source}");
        assert_eq!(apply_fix(source, &d), fixed, "{source}");
    }
}

#[test]
fn non_ascii_names_get_a_specific_fix() {
    let d = only("caf\u{e9}");
    assert_eq!(d.fix.message.id, "lex-invalid-character-fix-identifier");
}

#[test]
fn bidirectional_controls_are_refused_everywhere() {
    // Trojan Source (CVE-2021-42574): in code, in a comment and in a string.
    let source = "a\u{202E}; // \u{2066}x\n\"\u{2069}\"";
    let lexed = lex(source);
    let bidi: Vec<&Diagnostic> = lexed
        .diagnostics
        .iter()
        .filter(|d| d.code == codes::BIDI_CONTROL_CHARACTER)
        .collect();
    assert_eq!(bidi.len(), 3);
    // Reported once each: no extra invalid-character error for the one in code.
    assert_eq!(lexed.diagnostics.len(), 3);
    // Deleted in code and comments, escaped in strings.
    assert_eq!(bidi[0].fix.message.id, "lex-bidi-control-fix-remove");
    assert_eq!(bidi[1].fix.message.id, "lex-bidi-control-fix-remove");
    assert_eq!(bidi[2].fix.message.id, "lex-bidi-control-fix-escape");
    let in_string = only("\"\u{2069}\"");
    assert_eq!(apply_fix("\"\u{2069}\"", &in_string), "\"\\u{2069}\"");
}

#[test]
fn a_leading_byte_order_mark_is_ignored() {
    let lexed = lex("\u{FEFF}let");
    assert_eq!(lexed.diagnostics, []);
    assert_eq!(lexed.tokens[0].kind, Let);
    assert_eq!(lexed.tokens[0].text("\u{FEFF}let"), "let");
    // Anywhere else it is an invisible character to remove.
    assert_eq!(codes_of("let\u{FEFF}"), [codes::INVALID_CHARACTER]);
}

#[test]
fn files_over_the_limit_are_refused() {
    let lexed = lex_bounded("let x = 1;", 4);
    assert_eq!(lexed.tokens.len(), 1);
    let codes: Vec<ErrorCode> = lexed.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, [codes::SOURCE_TOO_LARGE]);
}

/// Inputs exercising every error, used by the checks below.
const CORPUS: &[&str] = &[
    "fn main(): void { let x = 1; }",
    "a # b € c",
    "x = \u{201C}hi\u{201D}",
    "caf\u{e9} `t` $x",
    "\"open\nnext",
    "/* open",
    r#""\d \u0041 \x41 \u{110000} \u{}""#,
    "0x 0b2 012 1__0 1e 10px 0x1.5",
    "a === b !== c",
    "a\u{202E}b // \u{2066}\n'\u{2069}'",
    "\u{FEFF}\u{FEFF}",
];

#[test]
fn every_message_exists_in_english_and_french() {
    for source in CORPUS {
        for d in lex(source).diagnostics {
            for id in d.message_ids() {
                for language in Language::ALL {
                    assert!(has_message(language, id), "`{id}` missing in {language:?}");
                }
            }
            assert!(
                perasc_diag::codes::info(d.code).is_some(),
                "{} not registered",
                d.code
            );
        }
    }
}

#[test]
fn every_diagnostic_has_exactly_one_primary_label() {
    for source in CORPUS {
        for d in lex(source).diagnostics {
            assert_eq!(
                d.labels.iter().filter(|l| l.primary).count(),
                1,
                "{source}: {d:?}"
            );
        }
    }
}

#[test]
fn applying_a_fix_removes_its_error() {
    for source in CORPUS {
        for d in lex(source).diagnostics {
            if d.fix.edits.is_empty() {
                continue;
            }
            let fixed = apply_fix(source, &d);
            let before = lex(source)
                .diagnostics
                .iter()
                .filter(|x| x.code == d.code)
                .count();
            let after = lex(&fixed)
                .diagnostics
                .iter()
                .filter(|x| x.code == d.code)
                .count();
            assert!(
                after < before,
                "{source:?} → {fixed:?} still has {}",
                d.code
            );
        }
    }
}

/// Checks the invariants every lexing result must hold.
fn check_invariants(source: &str) {
    let lexed = lex(source);
    let last = lexed.tokens.last().map(|t| t.kind);
    assert_eq!(last, Some(Eof), "{source:?}");
    let mut previous_end = 0;
    for token in &lexed.tokens {
        assert!(
            token.span.start >= previous_end,
            "{source:?}: overlapping tokens"
        );
        assert!(token.span.start <= token.span.end);
        let (start, end) = (super::index(token.span.start), super::index(token.span.end));
        assert!(
            source.is_char_boundary(start) && source.is_char_boundary(end),
            "{source:?}"
        );
        previous_end = token.span.end;
    }
    for d in &lexed.diagnostics {
        for label in &d.labels {
            let (start, end) = (super::index(label.span.start), super::index(label.span.end));
            assert!(end <= source.len(), "{source:?}");
            assert!(
                source.is_char_boundary(start) && source.is_char_boundary(end),
                "{source:?}"
            );
        }
    }
}

#[test]
fn invariants_hold_on_the_corpus() {
    for source in CORPUS {
        check_invariants(source);
    }
}

#[test]
fn invariants_hold_on_random_input() {
    // Deterministic pseudo-random sources mixing syntax, quotes, escapes and Unicode.
    // Nightly `cargo-fuzz` (day 10) goes much further; this keeps every CI run honest.
    const PIECES: &[&str] = &[
        "a",
        "fn",
        "0",
        "9",
        "x",
        "_",
        ".",
        "e",
        "+",
        "-",
        "=",
        "!",
        "\"",
        "'",
        "\\",
        "u",
        "{",
        "}",
        "/",
        "*",
        " ",
        "\n",
        "\r",
        "\t",
        "#",
        "`",
        "\u{e9}",
        "\u{202E}",
        "\u{FEFF}",
        "\u{1F600}",
        "\u{201C}",
        "\u{00A0}",
        "0x",
        "1_",
        "/*",
        "*/",
        "//",
        "===",
    ];
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..2_000 {
        let mut source = std::string::String::new();
        let len = (state % 24) as usize;
        for _ in 0..len {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let pick = usize::try_from(state % PIECES.len() as u64).unwrap_or(0);
            source.push_str(PIECES[pick]);
        }
        check_invariants(&source);
    }
}

#[test]
fn template_literals_resume_their_text_after_each_value() {
    // Without a value: a string. With values: pieces around their tokens,
    // a `{}` inside a value counted, a template inside a value its own.
    assert_eq!(kinds("`plain`"), [String, Eof]);
    assert_eq!(
        kinds("`a${b}c${ match (x) { _ => `in${y}` } }d`"),
        [
            TemplateHead,
            Ident,
            TemplateMiddle,
            Match,
            LParen,
            Ident,
            RParen,
            LBrace,
            Ident,
            FatArrow,
            TemplateHead,
            Ident,
            TemplateTail,
            RBrace,
            TemplateTail,
            Eof
        ]
    );
    assert_eq!(
        kinds("`${a}${b}`"),
        [
            TemplateHead,
            Ident,
            TemplateMiddle,
            Ident,
            TemplateTail,
            Eof
        ]
    );
    assert_eq!(
        kinds("{ `x${ {} }y` }"),
        [
            LBrace,
            TemplateHead,
            LBrace,
            RBrace,
            TemplateTail,
            RBrace,
            Eof
        ]
    );
    assert_eq!(super::template_value("`a\\`b\\${c\\n${"), "a`b${c\n");
    assert_eq!(super::template_value("}line\nbreak`"), "line\nbreak");
    // Unclosed: reported as a string is, its quote a backtick.
    assert_eq!(codes_of("`open ${x} still"), [codes::UNTERMINATED_STRING]);
    assert_eq!(codes_of("`multi\nline`"), [] as [perasc_diag::ErrorCode; 0]);
}
