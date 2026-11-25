use super::format;

fn fmt(source: &str) -> String {
    format(source).unwrap_or_else(|e| format!("refused: {e:?}"))
}

#[test]
fn a_syntax_error_is_refused() {
    assert!(format("fn f( {").is_err());
}

#[test]
fn an_error_inside_a_test_is_refused() {
    let source = "fn f(): i32 { return 1; }\ntest \"t\" { expect(f(.toBe(1); }\n";
    assert!(format(source).is_err());
}

#[test]
fn breaks_a_long_list_one_element_a_line() {
    let long = "fn f(alpha: i32, beta: i32, gamma: i32, delta: i32, epsilon: i32, zeta: i32, eta: i32, theta: i32): i32 { return alpha; }\n";
    assert_eq!(
        fmt(long),
        "fn f(\n  alpha: i32,\n  beta: i32,\n  gamma: i32,\n  delta: i32,\n  epsilon: i32,\n  zeta: i32,\n  eta: i32,\n  theta: i32,\n): i32 {\n  return alpha;\n}\n"
    );
}

#[test]
fn joins_what_fits() {
    let source = "fn f(\n  a: i32,\n  b: i32,\n): i32 {\n  return a\n    + b;\n}\n";
    assert_eq!(
        fmt(source),
        "fn f(a: i32, b: i32): i32 {\n  return a + b;\n}\n"
    );
}

#[test]
fn hugs_a_block_given_last() {
    let source =
        "fn f(xs: Array<i32>): i32 { return xs.reduce(0, (a, x) => { return a + x; }); }\n";
    assert_eq!(
        fmt(source),
        "fn f(xs: Array<i32>): i32 {\n  return xs.reduce(0, (a, x) => {\n    return a + x;\n  });\n}\n"
    );
}

#[test]
fn keeps_comments() {
    let source = "// head\nfn f(): i32 {\n  // why\n  return 1; // one\n}\n";
    assert_eq!(fmt(source), source);
}

#[test]
fn it_is_idempotent_on_its_tests() {
    for source in [
        "fn f(a:i32):i32{return a;}\n",
        "// c\nstruct P { x: i32, y: i32 }\n\nfn g(p: P): i32 {\n  return p.x; // x\n}\n",
    ] {
        let once = fmt(source);
        assert_eq!(fmt(&once), once);
    }
}

#[test]
fn keeps_the_comment_that_opens_a_file() {
    let source = "// a\n\nfn f(): i32 {\n  return 1;\n}\n";
    assert_eq!(fmt(source), source);
}

#[test]
fn a_long_sum_breaks_after_each_operator() {
    let source = "fn f(a: i32, b: i32, c: i32): i32 {\n  return a * 100000 + b * 10000 + c * 1000 + a * 100 + b * 10 + c + a * 3 + b * 7 + c * 11 + a * 13 + b * 17;\n}\n";
    assert_eq!(
        fmt(source),
        "fn f(a: i32, b: i32, c: i32): i32 {\n  return a * 100000 +\n    b * 10000 +\n    c * 1000 +\n    a * 100 +\n    b * 10 +\n    c +\n    a * 3 +\n    b * 7 +\n    c * 11 +\n    a * 13 +\n    b * 17;\n}\n"
    );
}

#[test]
fn a_sign_is_not_broken_after() {
    let source = "fn f(a: i32): i32 {\n  return -a;\n}\n";
    assert_eq!(fmt(source), source);
}

#[test]
fn a_declaration_keeps_its_blank_lines() {
    let source = "struct P {\n  x: i32,\n\n  // The second.\n  y: i32,\n\n  z: i32,\n}\n";
    assert_eq!(fmt(source), source);
}

/// An `inout` argument's `&` is a prefix, `bump(&k)`, as the guide writes it
/// and E0206's fix offers; a bitwise `&` between two operands keeps its
/// spaces (finding F130).
#[test]
fn an_inout_marker_is_a_prefix() {
    let source = "fn f(n: inout i32, m: i32): i32 {\n  g(&n, &self.frame);\n  return m & 255;\n}\n";
    assert_eq!(fmt(source), source);
}
