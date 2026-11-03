//! Command-line interface of the Peras compiler.
//!
//! Day 1 only answers `--version` and `--help`. Messages move to Fluent
//! (`perasc-diag/messages`) on day 2.

use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = io::stdout().lock();
    let mut err = io::stderr().lock();

    let result = match args.first().map(String::as_str) {
        Some("--version" | "-V") => writeln!(out, "perasc {}", perasc_driver::VERSION),
        None | Some("--help" | "-h") => writeln!(out, "Usage: perasc [--version] [--help]"),
        Some(other) => {
            let _ = writeln!(err, "error: unknown argument `{other}`");
            let _ = writeln!(err, "Usage: perasc [--version] [--help]");
            return ExitCode::FAILURE;
        }
    };

    if result.is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
