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
        (&["build"][..], "error: unknown command `build`"),
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
    let (_, _, err) = cli(&["build", "--lang", "fr"]);
    assert!(
        err.starts_with("erreur : commande inconnue `build`"),
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
