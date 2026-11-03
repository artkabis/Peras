# UI tests

Each `.peras` or `.apeiron` file here is compiled by the test suite, and its diagnostics are compared with an `insta` snapshot. Every compiler error has at least one UI test. The harness lives in `crates/perasc-syntax/tests/ui.rs`; snapshots sit next to it in `snapshots/`. Review changes with `cargo insta review`.
