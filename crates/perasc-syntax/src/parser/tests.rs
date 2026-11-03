use super::{MAX_DEPTH, parse};
use crate::ast::{ExprId, ItemKind, StmtKind};
use perasc_diag::messages::{Language, has_message};
use perasc_diag::{Diagnostic, ErrorCode, codes};

/// Parses `let _ = <expr>;` inside a function and dumps the expression.
fn expr(source: &str) -> String {
    let parsed = parse(&format!("fn f(): void {{ let v = {source}; }}"));
    assert_eq!(parsed.diagnostics, [], "{source}");
    let Some(item) = parsed.ast.items.first() else {
        unreachable!("no item")
    };
    let ItemKind::Fn(f) = &item.kind else {
        unreachable!("not a fn")
    };
    let Some(StmtKind::Local(local)) = f.body.stmts.first().map(|s| &s.kind) else {
        unreachable!("not a local")
    };
    let init: ExprId = local.init.unwrap_or_else(|| unreachable!("no init"));
    parsed.ast.sexpr(init)
}

fn codes_of(source: &str) -> Vec<ErrorCode> {
    parse(source).diagnostics.iter().map(|d| d.code).collect()
}

fn in_fn(body: &str) -> String {
    format!("fn f(): void {{\n{body}\n}}\n")
}

/// Applies a diagnostic's edits, last first.
fn apply_fix(source: &str, d: &Diagnostic) -> String {
    let mut fixed = source.to_owned();
    let mut edits = d.fix.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.span.start));
    for e in edits {
        let start = usize::try_from(e.span.start).unwrap_or(0);
        let end = usize::try_from(e.span.end).unwrap_or(0);
        fixed.replace_range(start..end, &e.replacement);
    }
    fixed
}

#[test]
fn precedence_follows_javascript() {
    assert_eq!(expr("1 + 2 * 3"), "(+ 1 (* 2 3))");
    assert_eq!(expr("a - b - c"), "(- (- a b) c)");
    assert_eq!(expr("a || b && c"), "(|| a (&& b c))");
    assert_eq!(expr("a | b ^ c & d"), "(| a (^ b (& c d)))");
    assert_eq!(expr("1 << 2 + 3"), "(<< 1 (+ 2 3))");
    assert_eq!(expr("a >> 1 < b"), "(< (>> a 1) b)");
    assert_eq!(expr("a + b == c * d"), "(== (+ a b) (* c d))");
    assert_eq!(expr("x % 2 != 0 && ok"), "(&& (!= (% x 2) 0) ok)");
}

#[test]
fn prefix_and_postfix_operators() {
    assert_eq!(expr("-a.b(c)[0]?"), "(neg (? (index (call (. a b) c) 0)))");
    assert_eq!(expr("!!done"), "(! (! done))");
    assert_eq!(expr("~mask & 0xFF"), "(& (~ mask) 255)");
    assert_eq!(expr("gain(&buf, 0.8,)"), "(call gain (& buf) 0.8)");
    assert_eq!(expr("(a + b) * c"), "(* (paren (+ a b)) c)");
}

#[test]
fn literals() {
    assert_eq!(expr("0xFF_FF"), "65535");
    assert_eq!(expr("18446744073709551615"), "18446744073709551615");
    assert_eq!(expr("2.5e-3"), "0.0025");
    assert_eq!(expr(r#""a\tb""#), r#""a\tb""#);
    assert_eq!(expr("true || false"), "(|| true false)");
}

#[test]
fn declarations_statements_and_types() {
    let source = "@frame\nexport async fn draw(fb: inout Framebuffer<Rgba8>, t: f32, out: sink Array<Option<f32>>): void {\n  let x: i32 = 1;\n  const y = x;\n  x += y;\n  x++;\n  p.q[0] = 2;\n  run();\n  ;\n  return;\n}\nconst LIMIT: i32 = 4;\n";
    let parsed = parse(source);
    assert_eq!(parsed.diagnostics, []);
    insta::assert_snapshot!(parsed.ast.dump(), @r"
    (fn draw @frame export async (params (fb inout Framebuffer<Rgba8>) (t f32) (out sink Array<Option<f32>>)) -> void
      (let x: i32 1)
      (const y x)
      (+= x y)
      (++ x)
      (= (index (. p q) 0) 2)
      (call run)
      (return)
    )
    (const LIMIT: i32 4)
    ");
}

#[test]
fn every_parser_error_has_its_case() {
    for (body, code) in [
        ("let v = ;", codes::EXPECTED_EXPRESSION),
        ("let a = 1\nlet b = 2;", codes::MISSING_SEMICOLON),
        ("let match = 1;", codes::EXPECTED_NAME),
        ("let n = max(a, b;", codes::UNCLOSED_DELIMITER),
        ("let n = max(a b);", codes::UNEXPECTED_TOKEN),
        ("let x: = 1;", codes::EXPECTED_TYPE),
        ("var y = 2;", codes::REMOVED_KEYWORD),
        ("let n = 99999999999999999999;", codes::NUMBER_TOO_LARGE),
        ("let ok = 0 < x < 10;", codes::CHAINED_COMPARISON),
        ("let z = flags & 1 == 0;", codes::AMBIGUOUS_PRECEDENCE),
        ("const k;", codes::CONST_WITHOUT_VALUE),
        ("let q = (a = b);", codes::ASSIGNMENT_IN_EXPRESSION),
        ("f() = 1;", codes::INVALID_ASSIGNMENT_TARGET),
    ] {
        assert_eq!(codes_of(&in_fn(body)), [code], "{body}");
    }
    assert_eq!(codes_of("log(1);"), [codes::EXPECTED_ITEM]);
    assert_eq!(codes_of("function f(): void {}"), [codes::REMOVED_KEYWORD]);
}

#[test]
fn one_error_per_mistake_and_parsing_goes_on() {
    let source = in_fn("let a = ;\nlet b = 1 +;\nlet c = (;\nlet d = 4;");
    assert_eq!(
        codes_of(&source),
        [
            codes::EXPECTED_EXPRESSION,
            codes::EXPECTED_EXPRESSION,
            codes::EXPECTED_EXPRESSION
        ]
    );
    // The declaration after the errors is still parsed.
    let parsed = parse(&source);
    assert!(
        parsed.ast.dump().contains("(let d 4)"),
        "{}",
        parsed.ast.dump()
    );
}

#[test]
fn lexer_errors_are_not_reported_twice() {
    assert_eq!(
        codes_of(&in_fn("let x = 1 # 2;")),
        [codes::INVALID_CHARACTER]
    );
    assert_eq!(codes_of(&in_fn("let x = 10px;")), [codes::MALFORMED_NUMBER]);
    assert_eq!(
        codes_of(&in_fn("let x = a === b;")),
        [codes::STRICT_EQUALITY]
    );
    assert_eq!(codes_of("# fn f(): void {}"), [codes::INVALID_CHARACTER]);
    // An unterminated string swallows the `;` or `)` after it: one error, not two.
    assert_eq!(
        codes_of(&in_fn("let s = \"abc;\nlet t = 1;")),
        [codes::UNTERMINATED_STRING]
    );
    assert_eq!(
        codes_of(&in_fn("log(\"abc);\nlet t = 1;")),
        [codes::UNTERMINATED_STRING]
    );
}

#[test]
fn a_bad_declaration_does_not_swallow_the_next_one() {
    let source = "fn (): void {}\nfn ok(): i32 { return 1; }\n";
    let parsed = parse(source);
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>(),
        [codes::EXPECTED_NAME]
    );
    assert!(
        parsed.ast.dump().contains("(fn ok"),
        "{}",
        parsed.ast.dump()
    );
}

#[test]
fn an_unclosed_body_is_reported_at_the_end_of_the_file() {
    let source = "fn f(): void {\n  let x = 1;\n";
    let parsed = parse(source);
    assert_eq!(parsed.diagnostics.len(), 1);
    let d = &parsed.diagnostics[0];
    assert_eq!(d.code, codes::UNCLOSED_DELIMITER);
    assert_eq!(apply_fix(source, d), "fn f(): void {\n  let x = 1;\n}\n");
}

#[test]
fn nesting_is_bounded_even_on_a_small_stack() {
    let deep = format!(
        "fn f(): void {{ let x = {}1{}; let y = {}1; let t: {}i32{}; }}",
        "(".repeat(100_000),
        ")".repeat(100_000),
        "- ".repeat(50_000),
        "Array<".repeat(10_000),
        ">".repeat(10_000)
    );
    // 1 MiB is half the default test-thread stack: the depth limit, not luck, keeps it safe.
    let handle = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || codes_of(&deep));
    let result = handle.ok().and_then(|h| h.join().ok());
    assert_eq!(
        result,
        Some(vec![
            codes::NESTING_TOO_DEEP,
            codes::NESTING_TOO_DEEP,
            codes::NESTING_TOO_DEEP
        ])
    );
    let at_limit = format!(
        "fn f(): void {{ let x = {}1{}; }}",
        "(".repeat(MAX_DEPTH - 3),
        ")".repeat(MAX_DEPTH - 3)
    );
    assert_eq!(codes_of(&at_limit), []);
}

/// Sources exercising every parser error, for the checks below.
const CORPUS: &[&str] = &[
    "fn f(): void {\nlet v = ;\nlet a = 1\nlet b = 2;\nlet match = 1;\n}",
    "fn f(): void { let n = max(a, b; }",
    "fn f(): void { let n = max(a b); let x: = 1; }",
    "function f(): void { var y = 2; }",
    "fn f(): void { let n = 99999999999999999999; let m = 1e999; }",
    "fn f(): void { let ok = 0 < x < 10; let z = flags & 1 == 0; let w = a == b < c; }",
    "fn f(): void { let z = a | b != c; }",
    "fn f(): void { const k; let q = (a = b); let r = (a += b); f() = 1; 2++; }",
    "log(1);\nfn f(): void {",
];

#[test]
fn every_message_exists_in_english_and_french() {
    for source in CORPUS {
        for d in parse(source).diagnostics {
            for id in d.message_ids() {
                for language in Language::ALL {
                    assert!(has_message(language, id), "`{id}` missing in {language:?}");
                }
            }
            assert!(codes::info(d.code).is_some(), "{} not registered", d.code);
            assert_eq!(d.labels.iter().filter(|l| l.primary).count(), 1, "{d:?}");
        }
    }
}

#[test]
fn applying_a_fix_removes_its_error() {
    for source in CORPUS {
        for d in parse(source).diagnostics {
            if d.fix.edits.is_empty() {
                continue;
            }
            let fixed = apply_fix(source, &d);
            let count = |s: &str| {
                parse(s)
                    .diagnostics
                    .iter()
                    .filter(|x| x.code == d.code)
                    .count()
            };
            assert!(
                count(&fixed) < count(source),
                "{source:?} -> {fixed:?} still has {}",
                d.code
            );
        }
    }
}

#[test]
fn random_token_soup_never_panics_and_stays_in_bounds() {
    const PIECES: &[&str] = &[
        "fn", "let", "const", "return", "var", "function", "export", "@", "x", "f", "1", "2.5",
        "\"s\"", "(", ")", "{", "}", "[", "]", "<", ">", ",", ";", ":", ".", "?", "=", "==", "+",
        "-", "*", "&", "|", "!", "++", "+=", "i32", "\n", " ", "#", "0x", "===",
    ];
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    for _ in 0..3_000 {
        let mut source = String::new();
        let len = usize::try_from(state % 40).unwrap_or(0);
        for _ in 0..len {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let pick = usize::try_from(state % PIECES.len() as u64).unwrap_or(0);
            source.push_str(PIECES[pick]);
            source.push(' ');
        }
        let parsed = parse(&source);
        for d in &parsed.diagnostics {
            for label in &d.labels {
                assert!(usize::try_from(label.span.end).unwrap_or(usize::MAX) <= source.len());
            }
        }
        for e in &parsed.ast.exprs {
            assert!(e.span.start <= e.span.end, "{source:?}");
        }
    }
}
