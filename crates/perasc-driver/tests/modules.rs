//! Owned modules that import each other (decision 0022): each refusal, shown
//! in the right file with its reason; and a program split into three files
//! computes what the same program in one file computes, in a module of the
//! same size.

use std::path::{Path, PathBuf};

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_driver::ToolError;
use perasc_ir::FnId;
use perasc_ir::interp::{self, Value};

/// A folder of its own for one test's files.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("perasc-modules-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| unreachable!("{e}"));
    dir
}

/// Writes `files` into a new folder; the path of the first.
fn write(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = folder(name);
    for (file, text) in files {
        let path = dir.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| unreachable!("{e}"));
        }
        std::fs::write(&path, text).unwrap_or_else(|e| unreachable!("{e}"));
    }
    dir.join(files.first().map_or("main.peras", |f| f.0))
}

fn compile(root: &Path) -> Result<(perasc_ir::Module, Vec<PathBuf>), ToolError> {
    let source = std::fs::read_to_string(root).unwrap_or_default();
    perasc_driver::compile_graph_ir(root, &source, true)
}

/// The refusal of building `files`: the file it is shown in, by name, and
/// its diagnostics' codes and messages.
fn refusal(name: &str, files: &[(&str, &str)]) -> (String, Vec<String>) {
    let root = write(name, files);
    let (file, checked) = match compile(&root) {
        Ok(_) => unreachable!("{name}: compiles"),
        Err(ToolError::Diagnostics(checked)) => (root.clone(), checked),
        Err(ToolError::InFile { path, checked, .. }) => (path, checked),
        Err(other) => unreachable!("{name}: {other:?}"),
    };
    let shown = file
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let found = checked
        .diagnostics
        .iter()
        .map(|d| format!("{} {:?}", d.code, d.message))
        .collect();
    (shown, found)
}

fn assert_refused(name: &str, files: &[(&str, &str)], in_file: &str, wanted: &[&str]) {
    let (file, found) = refusal(name, files);
    assert_eq!(file, in_file, "{name}: {found:?}");
    for w in wanted {
        assert!(
            found.iter().any(|f| f.contains(w)),
            "{name}: no `{w}` in {found:?}"
        );
    }
}

/// How an `E0244` of `reason` on `name` reads in a diagnostic's message.
fn import(reason: &str, name: &str) -> String {
    format!("Text(\"{reason}\")), (\"name\", Text(\"{name}\"))")
}

const LIB: &str = "export fn ok(): i32 {\n  return 1;\n}\n\
                   fn hidden(): i32 {\n  return 2;\n}\n\
                   export struct Box<T> {\n  v: T;\n}\n";

#[test]
fn a_file_not_found_is_reported_at_its_import() {
    assert_refused(
        "missing",
        &[(
            "main.peras",
            "import { f } from \"./nope.peras\";\nexport fn g(): i32 {\n  return 1;\n}\n",
        )],
        "main.peras",
        &["E0244", "Text(\"module\")"],
    );
}

#[test]
fn an_imported_file_that_does_not_compile_is_reported_in_it() {
    assert_refused(
        "broken",
        &[
            (
                "main.peras",
                "import { bad } from \"./bad.peras\";\nexport fn f(): i32 {\n  return bad();\n}\n",
            ),
            ("bad.peras", "export fn bad(): i32 {\n  return \"x\";\n}\n"),
        ],
        "bad.peras",
        &["E0210"],
    );
}

#[test]
fn names_not_exported_generic_or_taken_are_refused() {
    assert_refused(
        "names",
        &[
            (
                "main.peras",
                "import { hidden, Box, ok } from \"./lib.peras\";\nimport { ok } from \"./other.peras\";\n\
                 export fn f(): i32 {\n  return 1;\n}\n",
            ),
            ("lib.peras", LIB),
            ("other.peras", "export fn ok(): i32 {\n  return 3;\n}\n"),
        ],
        "main.peras",
        &[
            &import("name", "hidden"),
            &import("generic", "Box"),
            &import("clash", "ok"),
        ],
    );
    assert_refused(
        "local",
        &[
            (
                "main.peras",
                "import { ok } from \"./lib.peras\";\nfn ok(): i32 {\n  return 2;\n}\n\
                 export fn f(): i32 {\n  return ok();\n}\n",
            ),
            ("lib.peras", LIB),
        ],
        "main.peras",
        &[&import("clash", "ok")],
    );
}

#[test]
fn a_managed_module_is_not_imported_by_an_owned_one() {
    assert_refused(
        "managed",
        &[
            (
                "main.peras",
                "import { f } from \"./page.apeiron\";\nexport fn g(): i32 {\n  return 1;\n}\n",
            ),
            ("page.apeiron", "export fn f(): number {\n  return 1;\n}\n"),
        ],
        "main.peras",
        &["Text(\"managed\")"],
    );
}

#[test]
fn cycles_are_refused_with_their_path() {
    assert_refused(
        "cycle2",
        &[
            (
                "a.peras",
                "import { b } from \"./b.peras\";\nexport fn a(): i32 {\n  return b();\n}\n",
            ),
            (
                "b.peras",
                "import { a } from \"./a.peras\";\nexport fn b(): i32 {\n  return 1;\n}\n",
            ),
        ],
        "b.peras",
        &["E0250", "a.peras → b.peras → a.peras"],
    );
    assert_refused(
        "cycle3",
        &[
            (
                "a.peras",
                "import { b } from \"./lib/b.peras\";\nexport fn a(): i32 {\n  return b();\n}\n",
            ),
            (
                "lib/b.peras",
                "import { c } from \"./c.peras\";\nexport fn b(): i32 {\n  return c();\n}\n",
            ),
            (
                "lib/c.peras",
                "import { b } from \"./b.peras\";\nexport fn c(): i32 {\n  return 1;\n}\n",
            ),
        ],
        "c.peras",
        &["E0250", "lib/b.peras → lib/c.peras → lib/b.peras"],
    );
}

/// Three files: `size.peras`, imported by `area.peras` and by `main.peras`,
/// which imports `area.peras` too: a diamond. Each has a private `helper`.
const SIZE: &str = r"
export struct Item {
  name: String;
  tags: Array<String>;
  weight: i32;
}

export enum Kind {
  Light,
  Heavy(i32),
}

fn helper(x: i32): i32 {
  return x * 2;
}

export fn make(n: i32): Item {
  let tags: Array<String> = [];
  for (let i = 0; i < n % 4; i++) {
    tags.push(`t${i}`);
  }
  return Item(`item${n}`, tags, helper(n));
}

export fn kind(item: Item): Kind {
  if (item.weight > 10) {
    return Kind.Heavy(item.weight);
  }
  return Kind.Light;
}
";

const AREA: &str = r#"
import { Item, Kind, kind } from "./size.peras";

fn helper(x: i32): i32 {
  return x + 1;
}

export fn renamed(item: sink Item): Item {
  let out = item;
  out.name += "!";
  out.tags.push("r");
  return out;
}

export fn score(item: sink Item): i32 {
  const k = match (kind(item)) {
    Kind.Light => 0,
    Kind.Heavy(w) => w,
  };
  return helper(k + item.name.length + item.tags.length);
}
"#;

const MAIN: &str = r#"
import { Item, make } from "./lib/size.peras";
import { renamed, score } from "./lib/area.peras";

fn helper(x: i32): i32 {
  return x - 3;
}

export fn run(n: i32): i32 {
  const a = make(n);
  const b = renamed(a);
  return helper(score(b));
}
"#;

/// The same program in one file, its helpers renamed apart.
const ONE: &str = r#"
struct Item {
  name: String;
  tags: Array<String>;
  weight: i32;
}

enum Kind {
  Light,
  Heavy(i32),
}

fn helper1(x: i32): i32 {
  return x * 2;
}

fn make(n: i32): Item {
  let tags: Array<String> = [];
  for (let i = 0; i < n % 4; i++) {
    tags.push(`t${i}`);
  }
  return Item(`item${n}`, tags, helper1(n));
}

fn kind(item: Item): Kind {
  if (item.weight > 10) {
    return Kind.Heavy(item.weight);
  }
  return Kind.Light;
}

fn helper2(x: i32): i32 {
  return x + 1;
}

fn renamed(item: sink Item): Item {
  let out = item;
  out.name += "!";
  out.tags.push("r");
  return out;
}

fn score(item: sink Item): i32 {
  const k = match (kind(item)) {
    Kind.Light => 0,
    Kind.Heavy(w) => w,
  };
  return helper2(k + item.name.length + item.tags.length);
}

fn helper3(x: i32): i32 {
  return x - 3;
}

export fn run(n: i32): i32 {
  const a = make(n);
  const b = renamed(a);
  return helper3(score(b));
}
"#;

fn exported(module: &perasc_ir::Module, name: &str) -> FnId {
    let at = module
        .functions
        .iter()
        .position(|f| f.exported && f.name == name)
        .unwrap_or_else(|| unreachable!("no export `{name}`"));
    FnId(u32::try_from(at).unwrap_or(u32::MAX))
}

fn run(module: &perasc_ir::Module, n: i32) -> (Value, bool) {
    let id = exported(module, "run");
    match interp::run_with_heap(module, id, &[Value::I32(n)]) {
        Ok((value, report)) => (value, report.live.is_empty()),
        Err(trap) => unreachable!("traps: {trap:?}"),
    }
}

fn wasm(module: &perasc_ir::Module) -> usize {
    let mut module = module.clone();
    perasc_ir::optimize(&mut module);
    perasc_driver::emit_direct_for(
        &module,
        "m.wasm",
        perasc_driver::DirectOptions::default(),
        &Localizer::new(Language::English),
    )
    .wasm
    .len()
}

#[test]
fn three_files_compute_what_one_file_computes() {
    let root = write(
        "diamond",
        &[
            ("main.peras", MAIN),
            ("lib/size.peras", SIZE),
            ("lib/area.peras", AREA),
        ],
    );
    let (split, inputs) = compile(&root).unwrap_or_else(|e| unreachable!("{e:?}"));
    // The diamond: `size.peras` once, before what imports it, the root last.
    let names: Vec<String> = inputs
        .iter()
        .filter_map(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
        .collect();
    assert_eq!(names, ["size.peras", "area.peras", "main.peras"]);
    // Only the root's exports are JavaScript's.
    let exports: Vec<&str> = split
        .functions
        .iter()
        .filter(|f| f.exported)
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(exports, ["run"]);
    let one = perasc_driver::compile_ir_with(ONE, true).unwrap_or_else(|e| unreachable!("{e:?}"));
    for n in [-5, 0, 1, 3, 4, 7, 20, 1000] {
        let (a, clean_a) = run(&split, n);
        let (b, clean_b) = run(&one, n);
        assert_eq!(a, b, "run({n})");
        assert!(clean_a && clean_b, "run({n}) leaves blocks alive");
    }
    let (s, o) = (wasm(&split), wasm(&one));
    let ratio =
        f64::from(u32::try_from(s).unwrap_or(u32::MAX)) / f64::from(u32::try_from(o).unwrap_or(1));
    assert!(
        (0.99..=1.01).contains(&ratio),
        "three files give {s} bytes, one file {o}: {ratio}"
    );
}

/// A file imported twice under two paths is one: its structure passes from
/// one importer to the other.
#[test]
fn a_type_reached_by_two_paths_is_one_type() {
    let root = write(
        "two-paths",
        &[
            (
                "main.peras",
                "import { Item, make } from \"./lib/size.peras\";\nimport { score } from \"./lib/area.peras\";\n\
                 export fn run(n: i32): i32 {\n  return score(make(n));\n}\n",
            ),
            ("lib/size.peras", SIZE),
            ("lib/area.peras", AREA),
        ],
    );
    let (module, _) = compile(&root).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(run(&module, 7).0, Value::I32(23));
}

/// A managed module imports an owned one that imports others: the owned one
/// is checked with the files it imports, and its exports cross.
#[test]
fn a_managed_module_imports_an_owned_one_that_imports_others() {
    let page = "import { run } from \"./main.peras\";\n\
                export fn shown(n: i32): i32 {\n  return run(n);\n}\n";
    let root = write(
        "managed-root",
        &[
            ("page.apeiron", page),
            ("main.peras", MAIN),
            ("lib/size.peras", SIZE),
            ("lib/area.peras", AREA),
        ],
    );
    let checked = perasc_driver::check_file(&root, page);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}
