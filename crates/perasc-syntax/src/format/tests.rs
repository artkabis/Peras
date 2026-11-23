use super::{Style, format};

fn f1(source: &str) -> String {
    format(source, Style::Tokens).unwrap_or_else(|e| format!("refused: {e:?}"))
}

fn f2(source: &str) -> String {
    format(source, Style::Width).unwrap_or_else(|e| format!("refused: {e:?}"))
}

#[test]
fn a_syntax_error_is_refused() {
    assert!(format("fn f( {", Style::Tokens).is_err());
    assert!(format("fn f( {", Style::Width).is_err());
}

#[test]
fn an_error_inside_a_test_is_refused() {
    let source = "fn f(): i32 { return 1; }\ntest \"t\" { expect(f(.toBe(1); }\n";
    assert!(format(source, Style::Tokens).is_err());
}

#[test]
fn f1_makes_spaces_and_indentation_regular() {
    assert_eq!(
        f1("fn  add(a:i32,b :i32):i32{\nreturn a+b;\n    }\n"),
        "fn add(a: i32, b: i32): i32 {\n  return a + b;\n}\n"
    );
}

#[test]
fn f1_keeps_the_line_breaks_and_one_blank_line() {
    let source = "fn f(): i32 {\n  const x = 1;\n\n\n  return x;\n}\n";
    assert_eq!(
        f1(source),
        "fn f(): i32 {\n  const x = 1;\n\n  return x;\n}\n"
    );
}

#[test]
fn f1_keeps_comments_where_they_are() {
    let source = "// head\nfn f(): i32 { // why\n  /* a */ return 1; // one\n}\n";
    assert_eq!(
        f1(source),
        "// head\nfn f(): i32 { // why\n  /* a */ return 1; // one\n}\n"
    );
}

#[test]
fn f1_tells_a_sign_from_an_operator() {
    assert_eq!(
        f1("fn f(a: i32): i32 { return -a - -1 * (a-2); }\n"),
        "fn f(a: i32): i32 { return -a - -1 * (a - 2); }\n"
    );
}

#[test]
fn f1_keeps_generics_tight() {
    let source = "fn f(xs: Array<i32>): bool { return xs.length < 3; }\n";
    assert_eq!(f1(source), source);
}

#[test]
fn f2_breaks_a_long_list_one_element_a_line() {
    let long = "fn f(alpha: i32, beta: i32, gamma: i32, delta: i32, epsilon: i32, zeta: i32, eta: i32, theta: i32): i32 { return alpha; }\n";
    assert_eq!(
        f2(long),
        "fn f(\n  alpha: i32,\n  beta: i32,\n  gamma: i32,\n  delta: i32,\n  epsilon: i32,\n  zeta: i32,\n  eta: i32,\n  theta: i32,\n): i32 {\n  return alpha;\n}\n"
    );
}

#[test]
fn f2_joins_what_fits() {
    let source = "fn f(\n  a: i32,\n  b: i32,\n): i32 {\n  return a\n    + b;\n}\n";
    assert_eq!(
        f2(source),
        "fn f(a: i32, b: i32): i32 {\n  return a + b;\n}\n"
    );
}

#[test]
fn f2_hugs_a_block_given_last() {
    let source =
        "fn f(xs: Array<i32>): i32 { return xs.reduce(0, (a, x) => { return a + x; }); }\n";
    assert_eq!(
        f2(source),
        "fn f(xs: Array<i32>): i32 {\n  return xs.reduce(0, (a, x) => {\n    return a + x;\n  });\n}\n"
    );
}

#[test]
fn f2_keeps_comments() {
    let source = "// head\nfn f(): i32 {\n  // why\n  return 1; // one\n}\n";
    assert_eq!(f2(source), source);
}

#[test]
fn both_are_idempotent_on_their_tests() {
    for source in [
        "fn f(a:i32):i32{return a;}\n",
        "// c\nstruct P { x: i32, y: i32 }\n\nfn g(p: P): i32 {\n  return p.x; // x\n}\n",
    ] {
        let once = f1(source);
        assert_eq!(f1(&once), once);
        let once = f2(source);
        assert_eq!(f2(&once), once);
    }
}

#[test]
fn f2_keeps_the_comment_that_opens_a_file() {
    let source = "// a\n\nfn f(): i32 {\n  return 1;\n}\n";
    assert_eq!(f2(source), source);
}
