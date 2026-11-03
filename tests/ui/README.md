# UI tests

Each `.peras` or `.apeiron` file here is compiled by the test suite: files in `lexer/` show their tokens, files in `parser/` their syntax tree, and all of them their diagnostics, compared with an `insta` snapshot; files in `check/` are also checked for names and types. Every compiler error has at least one UI test. The harnesses live in `crates/perasc-syntax/tests/ui.rs` and `crates/perasc-check/tests/ui.rs`; snapshots sit next to them in `snapshots/`. Review changes with `cargo insta review`.
