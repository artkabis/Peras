use super::{ERRORS_FOUND, SUCCESS, USAGE_OR_IO, run};

/// Runs the CLI without colours and returns (exit code, stdout, stderr).
fn cli(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|&a| a.to_owned()).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run(&args, &mut out, &mut err, false);
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

const DEMO: &str = "tests/fixtures/demo.peras";

#[test]
fn renders_every_error_in_english() {
    let (code, out, err) = cli(&["check", DEMO, "--lang", "en"]);
    assert_eq!(code, ERRORS_FOUND);
    assert_eq!(out, "");
    insta::assert_snapshot!(err);
}

#[test]
fn renders_every_error_in_french() {
    let (code, _, err) = cli(&["check", DEMO, "--lang", "fr"]);
    assert_eq!(code, ERRORS_FOUND);
    insta::assert_snapshot!(err);
}

#[test]
fn rendered_output_contains_no_raw_bidirectional_control() {
    // The excerpt shows them as <U+2067>: printing them would reorder the terminal.
    for lang in ["en", "fr"] {
        let (_, _, err) = cli(&["check", DEMO, "--lang", lang]);
        assert!(!err.contains([
            '\u{202A}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}'
        ]));
        assert!(err.contains("<U+2067>"));
    }
}

#[test]
fn colours_only_on_request() {
    let (_, _, plain) = cli(&["check", DEMO, "--lang", "en"]);
    assert!(!plain.contains('\u{1b}'));
    let (_, _, coloured) = cli(&["check", DEMO, "--lang", "en", "--color", "always"]);
    assert!(coloured.contains("\u{1b}[1;31m"));
}

#[test]
fn json_output_is_structured_and_complete() {
    let (code, out, err) = cli(&["check", DEMO, "--json", "--lang", "fr"]);
    assert_eq!(code, ERRORS_FOUND);
    assert_eq!(err, "");
    let value: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
    assert_eq!(value["version"], 1);
    assert_eq!(value["language"], "fr");
    let diagnostics = value["files"][0]["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(value["errorCount"], diagnostics.len());
    insta::assert_snapshot!(out);
}

#[test]
fn a_valid_file_passes() {
    let (code, out, err) = cli(&["check", "tests/fixtures/valid.peras", "--lang", "en"]);
    assert_eq!((code, err.as_str()), (SUCCESS, ""));
    assert_eq!(out, "1 file checked, no error\n");
    let (_, out, _) = cli(&["check", "tests/fixtures/valid.peras", "--lang", "fr"]);
    assert_eq!(out, "1 fichier vérifié, aucune erreur\n");
}

#[test]
fn explain_prints_the_page_in_the_chosen_language() {
    let (code, out, _) = cli(&["explain", "E0006", "--lang", "en"]);
    assert_eq!(code, SUCCESS);
    assert!(out.starts_with("# E0006: Strict equality operator"));
    let (_, out, _) = cli(&["explain", "e6", "--lang", "fr"]);
    assert!(out.starts_with("# E0006 : Opérateur d'égalité stricte"));
    let (code, _, err) = cli(&["explain", "E9999", "--lang", "en"]);
    assert_eq!(code, USAGE_OR_IO);
    assert_eq!(err, "error: unknown error code `E9999`\n");
}

#[test]
fn unreadable_files_are_reported_and_the_rest_still_checked() {
    let (code, out, err) = cli(&[
        "check",
        "tests/fixtures/notes.txt",
        "tests/fixtures/missing.peras",
        "tests/fixtures/valid.peras",
        "--lang",
        "en",
    ]);
    assert_eq!(code, USAGE_OR_IO);
    assert_eq!(out, "");
    insta::assert_snapshot!(err);
}

#[test]
fn usage_errors_are_explained() {
    for (args, expected) in [
        (&["compile"][..], "error: unknown command `compile`"),
        (&["build"][..], "error: `build` needs a .peras file"),
        (&["check"][..], "error: `check` needs at least one file"),
        (
            &["check", "a.peras", "--wat"][..],
            "error: unknown argument `--wat`",
        ),
        (&["--lang"][..], "error: `--lang` needs a value"),
        (
            &["--lang", "de"][..],
            "error: invalid value `de` for `--lang`",
        ),
        (
            &["--color", "red", "--lang", "en"][..],
            "error: invalid value `red` for `--color`",
        ),
    ] {
        let mut args = args.to_vec();
        if !args.contains(&"--lang") {
            args.extend(["--lang", "en"]);
        }
        let (code, _, err) = cli(&args);
        assert_eq!(code, USAGE_OR_IO, "{args:?}");
        assert!(err.starts_with(expected), "{args:?}: {err}");
    }
    let (_, _, err) = cli(&["compile", "--lang", "fr"]);
    assert!(
        err.starts_with("erreur : commande inconnue `compile`"),
        "{err}"
    );
}

#[test]
fn help_and_version() {
    let (code, out, _) = cli(&["--help", "--lang", "en"]);
    assert_eq!(code, SUCCESS);
    assert!(out.starts_with("Usage: perasc <command> [options]"));
    let (_, out, _) = cli(&["--version"]);
    assert_eq!(out, format!("perasc {}\n", perasc_driver::VERSION));
}

/// A fresh output folder for a build test.
fn out_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("perasc-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn build_writes_the_module_its_loader_and_its_types() {
    let dir = out_dir("ok");
    let out_path = dir.to_string_lossy().into_owned();
    let (code, out, err) = cli(&[
        "build",
        "../../tests/ui/build/supported.peras",
        "--out",
        &out_path,
        "--lang",
        "en",
    ]);
    assert_eq!((code, err.as_str()), (SUCCESS, ""));
    assert!(out.contains("compiled to"), "{out}");
    let wasm = std::fs::read(dir.join("supported.wasm")).unwrap_or_default();
    assert!(wasm.starts_with(b"\0asm"), "not a wasm module");
    let dts = std::fs::read_to_string(dir.join("supported.d.ts")).unwrap_or_default();
    assert!(
        dts.contains(
            "export declare function clamp(x: bigint, low: bigint, high: bigint): bigint;"
        ),
        "{dts}"
    );
    assert!(dir.join("supported.js").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_refuses_what_it_cannot_compile_and_writes_nothing() {
    let dir = out_dir("unsupported");
    let out_path = dir.to_string_lossy().into_owned();
    let (code, _, err) = cli(&[
        "build",
        "../../tests/ui/build/E0900-not-supported-yet.peras",
        "--out",
        &out_path,
        "--lang",
        "fr",
    ]);
    assert_eq!(code, ERRORS_FOUND);
    assert!(err.contains("erreur[E0900]"), "{err}");
    assert!(!dir.exists(), "nothing must be written");
    let (code, _, err) = cli(&["build", "x.peras", "--backend", "llvm", "--lang", "en"]);
    assert_eq!(code, USAGE_OR_IO);
    assert!(
        err.starts_with("error: invalid value `llvm` for `--backend`"),
        "{err}"
    );
}

#[test]
fn build_can_skip_the_ir_optimizations() {
    let dir = out_dir("plain");
    let out_path = dir.to_string_lossy().into_owned();
    let (code, out, err) = cli(&[
        "build",
        "../../tests/ui/build/supported.peras",
        "--no-optimize",
        "--out",
        &out_path,
        "--lang",
        "en",
    ]);
    assert_eq!((code, err.as_str()), (SUCCESS, ""));
    assert!(out.contains("compiled to"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_links_the_allocator_asked_for() {
    let source = "../../tests/ui/build/supported.peras";
    let heap = std::env::temp_dir().join("perasc-heap-tlsf.peras");
    let _ = std::fs::write(
        &heap,
        "export fn words(): i32 {\n  let names: Array<String> = [\"a\"];\n  names.push(\"b\");\n  return names.length;\n}\n",
    );
    let heap_path = heap.to_string_lossy().into_owned();
    let mut loaders = Vec::new();
    for allocator in ["debug", "tlsf"] {
        let dir = out_dir(allocator);
        let out_path = dir.to_string_lossy().into_owned();
        let (code, _, err) = cli(&[
            "build",
            &heap_path,
            "--allocator",
            allocator,
            "--out",
            &out_path,
        ]);
        assert_eq!((code, err.as_str()), (SUCCESS, ""));
        loaders.push(std::fs::read_to_string(dir.join("perasc-heap-tlsf.js")).unwrap_or_default());
        let _ = std::fs::remove_dir_all(&dir);
    }
    // The allocator is inlined in the loader: the two differ, TLSF is smaller.
    assert!(
        loaders.iter().all(|l| l.contains("const $allocator")),
        "{loaders:?}"
    );
    assert_ne!(loaders.first(), loaders.get(1));
    let (code, _, err) = cli(&["build", source, "--allocator", "malloc"]);
    assert_eq!(code, USAGE_OR_IO);
    assert!(err.contains("`--allocator`"), "{err}");
    let _ = std::fs::remove_file(&heap);
}

#[test]
fn build_uses_simd_unless_told_otherwise() {
    let heap = std::env::temp_dir().join("perasc-heap-simd.peras");
    let _ = std::fs::write(
        &heap,
        "export fn count(text: String): i32 {\n  return text.split(\" \").length;\n}\n",
    );
    let heap_path = heap.to_string_lossy().into_owned();
    let mut loaders = Vec::new();
    for simd in [None, Some("on"), Some("off")] {
        let dir = out_dir(&format!("simd-{}", simd.unwrap_or("default")));
        let out_path = dir.to_string_lossy().into_owned();
        let mut args = vec!["build", heap_path.as_str(), "--out", out_path.as_str()];
        if let Some(value) = simd {
            args.extend(["--simd", value]);
        }
        let (code, _, err) = cli(&args);
        assert_eq!((code, err.as_str()), (SUCCESS, ""));
        loaders.push(std::fs::read_to_string(dir.join("perasc-heap-simd.js")).unwrap_or_default());
        let _ = std::fs::remove_dir_all(&dir);
    }
    // With SIMD, by default or asked, the loader checks for it first.
    let checks: Vec<bool> = loaders
        .iter()
        .map(|l| l.contains("WebAssembly.validate"))
        .collect();
    assert_eq!(checks, [true, true, false]);
    let (code, _, err) = cli(&["build", &heap_path, "--simd", "maybe"]);
    assert_eq!(code, USAGE_OR_IO);
    assert!(err.contains("`--simd`"), "{err}");
    let _ = std::fs::remove_file(&heap);
}

#[test]
fn build_reports_as_json_on_request() {
    let dir = out_dir("json");
    let out_path = dir.to_string_lossy().into_owned();
    let (code, out, err) = cli(&[
        "build",
        "../../tests/ui/build/E0900-not-supported-yet.peras",
        "--out",
        &out_path,
        "--json",
        "--lang",
        "fr",
    ]);
    assert_eq!((code, err.as_str()), (ERRORS_FOUND, ""));
    let value: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
    assert_eq!(value["language"], "fr");
    let first = &value["files"][0]["diagnostics"][0];
    assert_eq!(first["code"], "E0900", "{out}");
    assert!(first["labels"][0]["range"]["start"]["line"].as_u64() > Some(0));
    assert!(!dir.exists(), "nothing must be written");

    let (code, out, err) = cli(&[
        "build",
        "../../tests/ui/build/supported.peras",
        "--out",
        &out_path,
        "--json",
    ]);
    assert_eq!((code, err.as_str()), (SUCCESS, ""));
    let value: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
    assert_eq!(value["errorCount"], 0, "{out}");
    assert!(dir.join("supported.wasm").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
