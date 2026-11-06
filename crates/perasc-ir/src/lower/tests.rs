use crate::lower;
use perasc_diag::messages::{Language, has_message};
use perasc_diag::{Diagnostic, codes};

/// Parses, checks (asserting no error) and lowers a module.
fn lowered(source: &str) -> Result<crate::Module, Vec<Diagnostic>> {
    let parsed = perasc_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, source);
    assert_eq!(analysis.diagnostics, [], "check errors");
    lower(&parsed.ast, &analysis)
}

#[test]
fn literals_take_their_final_type_and_loops_are_desugared() {
    let source = "export fn f(n: i64, x: f32): i64 {\n  let total = n + 1;\n  for (let i: i64 = 0; i < n; i++) {\n    if (i == 3) continue;\n    total += i * 2;\n  }\n  while (true) { break; }\n  const y = (x * 0.5).toF64() - 1.0;\n  return total - y.toI64();\n}\n";
    let module = lowered(source).unwrap_or_else(|e| unreachable!("{e:?}"));
    insta::assert_snapshot!(module.to_string(), @r"
    export fn f(%0 n: i64, %1 x: f32) -> i64
      local %2 total: i64
      local %3 i: i64
      local %4 y: f64
      %2 = (i64.+ %0 1:i64)
      %3 = 0:i64
      loop while (i64.< %3 %0)
        if (i64.== %3 3:i64)
          continue
        %2 = (i64.+ %2 (i64.* %3 2:i64))
      step
        %3 = (i64.+ %3 1:i64)
      loop while forever
        break
      %4 = (f64.- (f64.from_f32 (f32.* %1 0.5:f32)) 1.0:f64)
      return (i64.- %2 (i64.from_f64 %4))
    ");
}

#[test]
fn negative_literals_are_folded_including_the_smallest() {
    let module = lowered("export fn f(): i32 { return -2147483648; }")
        .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert!(
        module.to_string().contains("return -2147483648:i32"),
        "{module}"
    );
}

#[test]
fn every_unsupported_construct_is_reported_with_its_name() {
    for (source, feature) in [
        // A structure is compiled; one holding a type that is not, is reported.
        (
            "struct P { xs: Array<String>; }\nfn f(_p: P): i32 { return 0; }",
            "array-element",
        ),
        ("const LIMIT = 3;", "global"),
        ("fn f(x: inout i32): void { x = x + 1; }", "inout"),
        (
            "fn f(_o: Option<Array<i32>>): i32 { return 0; }",
            "heap-option",
        ),
        ("fn f(): String { return \"a\"; }", "string"),
        ("fn f(x: u8): u8 { return x; }", "integer-type"),
        ("fn f(x: f64): f64 { return x % 2.0; }", "float-remainder"),
        (
            "fn f(xs: Array<Array<i32>>): i32 { return xs.length; }",
            "array-element",
        ),
        ("fn f(x: f64): f64 { return x.sin(); }", "method"),
    ] {
        let errors = lowered(source).err().unwrap_or_default();
        assert!(!errors.is_empty(), "{source}: no error");
        assert!(
            errors.iter().all(|d| d.code == codes::NOT_SUPPORTED_YET),
            "{source}"
        );
        let found = errors.iter().any(|d| {
            d.message.args.iter().any(|(name, value)| {
                *name == "feature" && *value == perasc_diag::Arg::Text(feature.to_owned())
            })
        });
        assert!(found, "{source}: {errors:#?}");
        for d in &errors {
            for id in d.message_ids() {
                for language in Language::ALL {
                    assert!(has_message(language, id), "`{id}` missing in {language:?}");
                }
            }
        }
    }
}
