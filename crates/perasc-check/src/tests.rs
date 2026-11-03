use perasc_diag::messages::{Language, has_message};
use perasc_diag::{Diagnostic, ErrorCode, codes};

/// Parses (asserting no syntax error) and checks a module.
fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let parsed = perasc_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "syntax errors in {source}");
    super::check(&parsed.ast, source)
}

fn codes_of(source: &str) -> Vec<ErrorCode> {
    diagnostics(source).iter().map(|d| d.code).collect()
}

/// Wraps statements in a function whose parameters are all read at the end.
fn body(stmts: &str) -> String {
    format!(
        "fn f(a: i32, b: i64, x: f32, y: f64, ok: bool, s: String, xs: Array<f32>): void {{\n{stmts}\nlet _use = [a.toF64(), b.toF64(), x.toF64(), y, xs.length.toF64(), s.length.toF64()]; if (ok) {{}}\n}}\n"
    )
}

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

const SPEC_DSP: &str = "export struct Buffer {
  samples: Array<f32>;
}

export fn gain(buf: inout Buffer, factor: f32): void {
  for (let i: i32 = 0; i < buf.samples.length; i++) {
    buf.samples[i] = buf.samples[i] * factor;
  }
}

export fn peak(buf: Buffer): Option<f32> {
  if (buf.samples.length == 0) return None;
  let max: f32 = 0.0;
  for (const s of buf.samples) { if (s.abs() > max) max = s.abs(); }
  return Some(max);
}

export fn consume(buf: sink Buffer): i32 {
  return buf.samples.length;
}
";

#[test]
fn the_spec_examples_are_valid() {
    assert_eq!(codes_of(SPEC_DSP), []);
    let fib = "export fn fib(n: i32): i32 {\n  if (n < 2) return n;\n  return fib(n - 1) + fib(n - 2);\n}\n";
    assert_eq!(codes_of(fib), []);
}

#[test]
fn literals_take_the_type_the_context_gives() {
    let ok = body(
        "let i8v: i8 = -128; let u: u8 = 255; let big: u64 = 18446744073709551615;\n\
         let fl: f32 = 0.5; let sum = b + 1; let half = x * 0.5; let n = 3; let g = 2.5;\n\
         let arr: Array<u8> = [1, 2, 255]; let o: Option<i64> = Some(5); let none: Option<f32> = None;\n\
         let _all = [i8v.toF64(), u.toF64(), big.toF64(), fl.toF64(), sum.toF64(), half.toF64(), n.toF64(), g, arr.length.toF64()];\n\
         if (o.isSome() && none.isNone()) {}",
    );
    assert_eq!(codes_of(&ok), []);
}

#[test]
fn every_check_error_has_its_case() {
    for (stmts, code) in [
        ("let _t = totl;", codes::UNDEFINED_NAME),
        ("let a = 1;", codes::DUPLICATE_DECLARATION),
        ("{ let a = 1; let _b = a; }", codes::SHADOWING),
        (
            "{ let z = 1; let z = 2; let _q = z; }",
            codes::DUPLICATE_DECLARATION,
        ),
        ("let unused = 1;", codes::UNUSED),
        (
            "const c = 1; c = 2; let _r = c;",
            codes::ASSIGN_TO_IMMUTABLE,
        ),
        ("a = 2;", codes::ASSIGN_TO_IMMUTABLE),
        ("let _m = a + y;", codes::MISMATCHED_TYPES),
        ("let _w: f32 = 1;", codes::INTEGER_FOR_FLOAT),
        ("let _v: u8 = 256;", codes::LITERAL_OUT_OF_RANGE),
        ("let _q: i32 = -2147483649;", codes::LITERAL_OUT_OF_RANGE),
        ("let _k: float = 0.5;", codes::UNKNOWN_TYPE),
        ("let _k = xs.size;", codes::UNKNOWN_MEMBER),
        ("let _k: Array = [];", codes::TYPE_ARGUMENT_COUNT),
        ("if (a) {}", codes::CONDITION_NOT_BOOL),
        ("while (s) {}", codes::CONDITION_NOT_BOOL),
        ("let _k = ok + 1;", codes::INVALID_OPERATOR),
        ("let _k = -xs;", codes::INVALID_OPERATOR),
        ("let _k = !a;", codes::INVALID_OPERATOR),
        ("let _k = a[0];", codes::INVALID_OPERATOR),
    ] {
        let source = body(stmts);
        let found = codes_of(&source);
        assert!(
            found.contains(&code),
            "{stmts}: expected {code}, found {found:?}"
        );
        assert!(
            found.iter().all(|c| *c == code),
            "{stmts}: extra errors {found:?}"
        );
    }
}

#[test]
fn signature_errors() {
    for (source, code) in [
        (
            "fn f(x: i32, x: i32): i32 { return x; }",
            codes::DUPLICATE_DECLARATION,
        ),
        (
            "fn f(): void {}\nfn f(): void {}",
            codes::DUPLICATE_DECLARATION,
        ),
        ("fn f(unused: i32): void {}", codes::UNUSED),
        (
            "fn g(n: i32): i32 { return n; }\nfn f(): void { let _r = g(1, 2); }",
            codes::ARGUMENT_COUNT,
        ),
        (
            "struct P { x: f64; y: f64; }\nfn f(): void { let _p = P(1.0); }",
            codes::ARGUMENT_COUNT,
        ),
        ("fn f(): i32 { return; }", codes::RETURN_MISMATCH),
        ("fn f(): void { return 1; }", codes::RETURN_MISMATCH),
        (
            "fn f(x: i32): i32 { if (x > 0) return 1; }",
            codes::MISSING_RETURN,
        ),
        ("export fn f() {}\n", codes::MISSING_RETURN_TYPE),
        ("fn f() { return 1; }", codes::MISSING_RETURN_TYPE),
        (
            "fn f(buf: Buffer): void { let _b = buf; }",
            codes::UNKNOWN_TYPE,
        ),
        (
            "struct P { x: f64; }\nfn f(p: P): f64 { return p.y; }",
            codes::UNKNOWN_MEMBER,
        ),
        (
            "fn f(xs: Array<f32>): void { xs[0] = 1.0; }",
            codes::ASSIGN_TO_IMMUTABLE,
        ),
        (
            "fn f(xs: Array<f32>): void { for (const v of xs) { v = 1.0; let _r = v; } }",
            codes::ASSIGN_TO_IMMUTABLE,
        ),
    ] {
        let found = codes_of(source);
        assert_eq!(found, [code], "{source}");
    }
    // `export fn f(): void {}` is fine; only the missing annotation is an error.
    assert_eq!(codes_of("export fn f(): void {}"), []);
}

#[test]
fn infinite_loops_and_complete_if_else_return() {
    for source in [
        "fn f(x: i32): i32 { if (x > 0) { return 1; } else { return 0; } }",
        "fn f(): i32 { for (;;) {} }",
        "fn f(): i32 { while (true) {} }",
    ] {
        assert_eq!(codes_of(source), [], "{source}");
    }
    assert_eq!(
        codes_of("fn f(): i32 { while (true) { break; } }"),
        [codes::MISSING_RETURN]
    );
}

#[test]
fn names_starting_with_an_underscore_may_be_unused() {
    assert_eq!(codes_of("fn f(_x: i32): void { let _y = 1; }"), []);
}

#[test]
fn assigning_is_not_reading() {
    assert_eq!(
        codes_of("fn f(): void { let count = 0; count += 1; count++; }"),
        [codes::UNUSED]
    );
}

#[test]
fn shadowing_the_language_names_is_refused() {
    let found = codes_of("fn f(): void { let Some = 1; let _x = Some; }");
    assert_eq!(found, [codes::SHADOWING]);
}

#[test]
fn one_mistake_gives_one_error() {
    // The undefined name becomes an error type: no mismatch follows.
    assert_eq!(
        codes_of(&body("let _t: f64 = totl * 2.0 + y;")),
        [codes::UNDEFINED_NAME]
    );
    // An unknown type is reported once, not at every use.
    assert_eq!(
        codes_of("fn f(p: Pont): void { let _a = p.x + 1; let _b = p.y; }"),
        [codes::UNKNOWN_TYPE]
    );
}

#[test]
fn the_conversion_fix_follows_the_spec() {
    // `i32 + f64`: the fix converts the integer, as in `x.toF64() + y`.
    let source = "fn f(x: i32, y: f64): f64 { return x + y; }";
    let found = diagnostics(source);
    assert_eq!(found.len(), 1);
    assert_eq!(
        apply_fix(source, &found[0]),
        "fn f(x: i32, y: f64): f64 { return x.toF64() + y; }"
    );
    // A compound expression is parenthesized first.
    let source = "fn f(x: i32, y: f64): f64 { return y * (x + 1); }";
    let found = diagnostics(source);
    assert_eq!(
        apply_fix(source, &found[0]),
        "fn f(x: i32, y: f64): f64 { return y * (x + 1).toF64(); }"
    );
}

/// Sources producing every check error, for the global checks below.
const CORPUS: &[&str] = &[
    "fn f(unused: i32, p: i32): i32 { let t = totl; let v: u8 = 256; let w: f32 = 1; p = 2; if (p) {} return; }",
    "struct P { x: f64; x: f64; }\nexport fn g(xs: Array<f32>, q: P) { let k = xs.size; let z: Array = []; let o = q.x + 1; }",
    "fn h(a: i32, b: f64): i32 { let m = a + b; let n = true + 1; const c = 1; c = 3; let a = 2; if (m > 0) return 1; }",
    "fn k(): void { return 1; }\nfn j(n: i32): i32 { return k(n); }",
];

#[test]
fn every_message_exists_in_english_and_french() {
    for source in CORPUS {
        for d in diagnostics(source) {
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
        for d in diagnostics(source) {
            if d.fix.edits.is_empty() {
                continue;
            }
            let fixed = apply_fix(source, &d);
            let count = |s: &str| {
                let parsed = perasc_syntax::parse(s);
                super::check(&parsed.ast, s)
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
