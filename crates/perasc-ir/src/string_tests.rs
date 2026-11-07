//! Strings, lowered to buffers of UTF-8 bytes and run in the interpreter:
//! literals, `+`, `+=`, `==`, `length` in bytes, and no buffer left alive.

use crate::interp::{Value, run_with_heap};
use crate::{FnId, Module, lower, verify};

fn compile(source: &str) -> Module {
    let parsed = perasc_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, source);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    module
}

fn gives(source: &str, cases: &[(Vec<Value>, Value)]) -> Module {
    let module = compile(source);
    for (args, expected) in cases {
        let (value, heap) = run_with_heap(&module, FnId(0), args)
            .unwrap_or_else(|t| unreachable!("{t:?}\n{module}"));
        assert_eq!(&value, expected, "{args:?}\n{module}");
        assert_eq!(heap.live, Vec::<u32>::new(), "leak on {args:?}\n{module}");
    }
    module
}

#[test]
fn length_counts_utf8_bytes() {
    // "é" is 2 bytes, the emoji 4.
    let source = "export fn f(): i32 {\n  return \"a\".length * 100 + \"é\".length * 10 + \"\\u{1F600}\".length;\n}\n";
    gives(source, &[(vec![], Value::I32(124))]);
}

#[test]
fn concatenation_and_appending() {
    let source = "export fn f(n: i32): i32 {\n  let s = \"\";\n  for (let i = 0; i < n; i++) {\n    s += \"ab\";\n  }\n  const t = s + \"!\" + s;\n  s += s;\n  if (t == s) {\n    return -1;\n  }\n  return t.length * 1000 + s.length;\n}\n";
    gives(
        source,
        &[
            (vec![Value::I32(0)], Value::I32(1000)),
            (vec![Value::I32(3)], Value::I32(13_012)),
        ],
    );
}

#[test]
fn equality_compares_every_byte() {
    let source = "export fn f(n: i32): i32 {\n  const a = \"abc\";\n  let b = \"ab\";\n  if (n > 0) {\n    b += \"c\";\n  }\n  if (n > 1) {\n    b = \"abd\";\n  }\n  let score = 0;\n  if (a == b) {\n    score += 1;\n  }\n  if (a != b) {\n    score += 10;\n  }\n  return score;\n}\n";
    gives(
        source,
        &[
            (vec![Value::I32(0)], Value::I32(10)),
            (vec![Value::I32(1)], Value::I32(1)),
            (vec![Value::I32(2)], Value::I32(10)),
        ],
    );
}

#[test]
fn literals_are_stored_once() {
    let source = "export fn f(): i32 {\n  const a = \"same\";\n  const b = \"same\";\n  const c = \"other\";\n  return a.length + b.length + c.length;\n}\n";
    let module = gives(source, &[(vec![], Value::I32(13))]);
    assert_eq!(module.data, vec![b"same".to_vec(), b"other".to_vec()]);
}

#[test]
fn strings_in_structures_move_and_are_released() {
    let source = "export fn f(n: i32): i32 {\n  let u = User(\"ann\", n);\n  for (let i = 0; i < n; i++) {\n    u = rename(u, i);\n  }\n  return u.name.length;\n}\n\
         struct User {\n  name: String;\n  age: i32;\n}\n\
         fn rename(u: sink User, i: i32): User {\n  let v = u;\n  if (i % 2 == 0) {\n    v.name = v.name + \"x\";\n  }\n  return v;\n}\n";
    gives(
        source,
        &[
            (vec![Value::I32(0)], Value::I32(3)),
            (vec![Value::I32(5)], Value::I32(6)),
        ],
    );
}
