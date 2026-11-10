//! The generated JavaScript, run by Node when it is installed: each module
//! is checked as a managed one, compiled, then its `main` function is called
//! and what it returns is compared with what the owned backends give.

use std::path::PathBuf;
use std::process::Command;

use crate::{Options, generate};

/// The JavaScript of a managed module; panics on any error.
fn js(source: &str) -> String {
    let parsed = perasc_syntax::parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let context = perasc_check::Context {
        managed: true,
        imports: Vec::new(),
    };
    let analysis = perasc_check::analyze_with(&parsed.ast, source, &context);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
    generate(&parsed.ast, &analysis, &Options::default()).unwrap_or_else(|errors| {
        assert!(errors.is_empty(), "{errors:?}");
        String::new()
    })
}

/// Node, from `PATH` or the folders it is installed in here.
fn node() -> Option<PathBuf> {
    let candidates = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .chain(
            std::fs::read_dir("/opt")
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path().join("bin")),
        );
    candidates
        .map(|dir| dir.join("node"))
        .find(|path| path.is_file())
}

/// What `main()` returns, as JSON with `BigInt`s as text with an `n`, or
/// the message of what it throws.
fn run(source: &str) -> Option<String> {
    let node = node()?;
    let module = js(source);
    let dir = std::env::temp_dir().join(format!("perasc-js-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let file = dir.join(format!(
        "m{}.mjs",
        source.len() * 31 + source.bytes().map(usize::from).sum::<usize>()
    ));
    std::fs::write(&file, &module).ok()?;
    let script = "import(process.argv[1]).then(async (m) => { try { const r = await m.main(); \
         console.log(JSON.stringify(r, (k, v) => typeof v === 'bigint' ? v + 'n' : v instanceof Map ? [...v] : v instanceof Set ? [...v] : v === undefined ? null : v)); } \
         catch (e) { console.log('throws: ' + e.message); } });";
    let output = Command::new(node)
        .arg("--input-type=module")
        .arg("-e")
        .arg(script)
        .arg(&file)
        .output()
        .ok()?;
    let _ = std::fs::remove_file(&file);
    assert!(
        output.status.success(),
        "{}\n{module}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Checks `main()` gives `expected`, when Node is installed.
fn gives(source: &str, expected: &str) {
    if let Some(found) = run(source) {
        assert_eq!(found, expected, "{}", js(source));
    }
}

#[test]
fn integers_wrap_as_in_owned_modules() {
    gives(
        "export fn main(): Array<i32> {
           const big: i32 = 2147483647;
           let n = big + 1;
           const m = big * 3;
           const d = -7 / 2;
           const r = -7 % 2;
           const s = 1 << 33;
           let k = big;
           k += 1;
           let j = big;
           j++;
           return [n, m, d, r, s, (-5).abs(), big.wrappingAdd(10), k, j];
         }",
        "[-2147483648,2147483645,-3,-1,2,5,-2147483639,-2147483648,-2147483648]",
    );
    gives(
        "export fn main(): Array<i64> {
           const big: i64 = 9223372036854775807;
           const n: i64 = 7;
           return [big + 1, n / 2, n * -3, n.min(3), 5.toI64()];
         }",
        "[\"-9223372036854775808n\",\"3n\",\"-21n\",\"3n\",\"5n\"]",
    );
}

#[test]
fn division_by_zero_throws() {
    gives(
        "export fn main(): i32 {
           const zero = 0;
           return 1 / zero;
         }",
        "throws: divide by zero",
    );
}

#[test]
fn conversions_and_checked_arithmetic() {
    gives(
        "export fn main(): Array<String> {
           const x = 1.0e10;
           const y: f64 = -2.7;
           const nan = 0.0 / 0.0;
           const big: i32 = 2147483647;
           return [
             x.toI32().toString(), y.toI32().toString(), nan.toI32().toString(),
             big.checkedAdd(1).isNone().toString(), big.checkedSub(1).unwrapOr(0).toString(),
             0.1.toF32().toString(), (0.1.toF32() + 0.2.toF32()).toString(),
             300.toU8().toString(), (-1).toU32().toString(), (-1).toI64().toU64().toString(),
           ];
         }",
        "[\"2147483647\",\"-2\",\"0\",\"true\",\"2147483646\",\"0.1\",\"0.3\",\"44\",\"4294967295\",\"18446744073709551615\"]",
    );
}

#[test]
fn structures_enumerations_and_match() {
    gives(
        "struct Point { x: f64, y: f64 }
         enum Shape { Circle(f64), Square(f64), Empty }
         fn area(s: Shape): f64 {
           return match (s) {
             Shape.Circle(r) => 3.0 * r * r,
             Square(side) => side * side,
             Empty => 0.0,
           };
         }
         fn first(xs: Array<i32>): Option<i32> {
           if (xs.length == 0) {
             return None;
           }
           return Some(xs[0]);
         }
         fn sum2(xs: Array<i32>): Option<i32> {
           const a = first(xs)?;
           return Some(a + xs[1]);
         }
         export fn main(): Array<f64> {
           const p = Point(1.0, 2.0);
           const shapes = [Shape.Circle(1.0), Shape.Square(2.0), Shape.Empty];
           let total = 0.0;
           for (const s of shapes) {
             total += area(s);
           }
           const t = (1, 2.5);
           const none = sum2([]).isNone();
           return [p.x + p.y, total, t.1, sum2([3, 4]).unwrapOr(0).toF64(), none.toString().length.toF64()];
         }",
        "[3,7,2.5,7,4]",
    );
}

#[test]
fn arrays_strings_maps_and_sets() {
    gives(
        "export fn main(): Array<String> {
           let xs = [3, 1, 2];
           xs.sort();
           xs.push(10);
           const doubled = xs.map((x) => x * 2).filter((x) => x > 2);
           const total = doubled.reduce((a, x) => a + x, 0);
           let counts: Map<String, i32> = Map.new();
           for (const w of \"a b a c a\".split(\" \")) {
             counts.set(w, counts.get(w).unwrapOr(0) + 1);
           }
           let seen: Set<i32> = Set.new();
           seen.add(1);
           seen.add(1);
           let keys = \"\";
           for (const entry of counts) {
             keys += `${entry.0}=${entry.1};`;
           }
           const text = \"  Hello, World  \".trim();
           return [
             total.toString(), keys, seen.size.toString(), text.toUpperCase(),
             text.replace(\"World\", \"$&\"), \"12\".parseI32().unwrapOr(-1).toString(),
             \"1e3\".parseI32().isNone().toString(), xs.indexOf(10).toString(),
           ];
         }",
        "[\"30\",\"a=3;b=1;c=1;\",\"1\",\"HELLO, WORLD\",\"Hello, $&\",\"12\",\"true\",\"3\"]",
    );
}

#[test]
fn out_of_bounds_throws() {
    gives(
        "export fn main(): i32 {
           const xs = [1, 2];
           return xs[2];
         }",
        "throws: index 2 is out of bounds: the length is 2",
    );
}

#[test]
fn async_functions_wait() {
    gives(
        "async fn later(n: number): Promise<number> {
           return n * 2.0;
         }
         export async fn main(): Promise<number> {
           const a = await later(1.5);
           return a + (await later(2.0));
         }",
        "7",
    );
}

#[test]
fn generic_functions_compute_by_their_types() {
    gives(
        "fn twice<T: Numeric>(x: T): T {
           return x + x;
         }
         export fn main(): Array<String> {
           const big: i32 = 2000000000;
           return [twice(big).toString(), twice(2000000000.0).toString()];
         }",
        "[\"-294967296\",\"4000000000\"]",
    );
}

#[test]
fn methods_and_interfaces() {
    gives(
        "struct Point { x: f64, y: f64 }
         impl Point {
           fn norm(self): f64 {
             return (self.x * self.x + self.y * self.y).sqrt();
           }
           fn origin(): Point {
             return Point(0.0, 0.0);
           }
         }
         export fn main(): f64 {
           const p = Point(3.0, 4.0);
           return p.norm() + Point.origin().x;
         }",
        "5",
    );
}

#[test]
fn match_as_an_argument_keeps_the_order() {
    gives(
        "fn pick(o: Option<i32>, d: i32): i32 {
           return match (o) {
             Some(v) => v,
             None => d,
           };
         }
         export fn main(): Array<i32> {
           let log: Array<i32> = [];
           const a = Some(5);
           log.push(pick(a, 1) + match (a) { Some(v) => v * 10, None => 0 });
           const ok = log.length > 0 && match (a) { Some(v) => v > 3, None => false };
           if (ok) {
             log.push(1);
           }
           return log;
         }",
        "[55,1]",
    );
}

#[test]
fn reserved_words_and_names_in_sibling_blocks() {
    let source = "export fn main(): i32 {
           const new = 1;
           let x = new;
           {
             let y = 5;
             x += y;
           }
           {
             let y = 7;
             x += y;
           }
           return x;
         }";
    let out = js(source);
    assert!(out.contains("const new$ = 1;"), "{out}");
    assert!(out.contains("let y$2 = 7;"), "{out}");
    gives(source, "13");
}

/// The sources of the UI tests, of the end-to-end fixtures, and the
/// `peras build` examples of the documentation.
fn corpus() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    let mut dirs = vec![
        root.join("tests/ui/check"),
        root.join("tests/ui/build"),
        root.join("packages/vite-plugin-peras/e2e/fixtures"),
    ];
    let mut pages = Vec::new();
    for lang in ["en", "fr"] {
        dirs.push(root.join("docs").join(lang).join("guide"));
        dirs.push(root.join("docs").join(lang).join("reference"));
    }
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            match path.extension().and_then(|e| e.to_str()) {
                _ if path.is_dir() => dirs.push(path),
                Some("peras" | "apeiron") => {
                    sources.push(std::fs::read_to_string(&path).unwrap_or_default());
                }
                Some("md") => pages.push(std::fs::read_to_string(&path).unwrap_or_default()),
                _ => {}
            }
        }
    }
    for page in pages {
        for block in page.split("```peras build\n").skip(1) {
            if let Some((code, _)) = block.split_once("\n```") {
                sources.push(code.to_owned());
            }
        }
    }
    sources
}

/// Every source of the corpus that checks as a managed module compiles to
/// JavaScript, which Node parses.
#[test]
fn every_valid_source_gives_valid_javascript() {
    let mut compiled = 0;
    let mut unsupported = Vec::new();
    let dir = std::env::temp_dir().join(format!("perasc-js-syntax-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    for (i, source) in corpus().iter().enumerate() {
        let parsed = perasc_syntax::parse(source);
        if !parsed.diagnostics.is_empty() {
            continue;
        }
        let context = perasc_check::Context {
            managed: true,
            imports: Vec::new(),
        };
        let analysis = perasc_check::analyze_with(&parsed.ast, source, &context);
        if !analysis.diagnostics.is_empty() {
            continue;
        }
        let js = match generate(&parsed.ast, &analysis, &Options::default()) {
            Ok(js) => js,
            Err(errors) => {
                unsupported.push(format!("{errors:?}\n{source}"));
                continue;
            }
        };
        compiled += 1;
        if let Some(node) = node() {
            let file = dir.join(format!("m{i}.mjs"));
            let _ = std::fs::write(&file, &js);
            let output = Command::new(node).arg("--check").arg(&file).output();
            let ok = output.as_ref().is_ok_and(|o| o.status.success());
            assert!(
                ok,
                "{}\n{js}",
                output
                    .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
                    .unwrap_or_default()
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(unsupported.is_empty(), "{}", unsupported.join("\n\n"));
    assert!(compiled >= 100, "only {compiled} sources compiled");
}

#[test]
fn options_of_options_are_refused() {
    let source = "export fn main(xs: Array<Option<i32>>): bool {
           let ys = xs;
           return ys.pop().isSome();
         }";
    let parsed = perasc_syntax::parse(source);
    let context = perasc_check::Context {
        managed: true,
        imports: Vec::new(),
    };
    let analysis = perasc_check::analyze_with(&parsed.ast, source, &context);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
    let errors = generate(&parsed.ast, &analysis, &Options::default())
        .err()
        .unwrap_or_default();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, perasc_diag::codes::NOT_SUPPORTED_YET);
}
