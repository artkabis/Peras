//! Command-line interface of the Peras compiler.
//!
//! Every text comes from the Fluent files (`perasc-diag/messages`), in English or French.
//! Exit codes: 0 when everything is valid, 1 when a source has errors, 2 when the
//! command itself is wrong or a file cannot be read.

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use perasc_diag::i18n::Localizer;
use perasc_diag::json::{self, FileReport, Report};
use perasc_diag::messages::Language;
use perasc_diag::render::{RenderOptions, colon, render};
use perasc_diag::source::SourceFile;
use perasc_diag::{Message, codes};
use perasc_driver::{MAX_ERRORS_PER_FILE, ReadError, VERSION, check, read_source};

const SUCCESS: u8 = 0;
const ERRORS_FOUND: u8 = 1;
const USAGE_OR_IO: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Auto,
    Always,
    Never,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Version,
    Check(Vec<String>),
    Explain(String),
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    command: Command,
    language: Option<Language>,
    json: bool,
    color: Color,
}

/// A command-line mistake, as a message to show.
type UsageError = Message;

fn parse_args(args: &[String]) -> Result<Options, UsageError> {
    let mut options = Options {
        command: Command::Help,
        language: None,
        json: false,
        color: Color::Auto,
    };
    let mut positional = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |option: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| Message::new("cli-missing-value").text("option", option.to_owned()))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                return Ok(Options {
                    command: Command::Help,
                    ..options
                });
            }
            "-V" | "--version" => {
                return Ok(Options {
                    command: Command::Version,
                    ..options
                });
            }
            "--json" => options.json = true,
            "--lang" => {
                let tag = value("--lang")?;
                options.language = Some(Language::from_tag(&tag).ok_or_else(|| {
                    Message::new("cli-invalid-value")
                        .text("option", "--lang")
                        .text("value", tag.clone())
                })?);
            }
            "--color" => {
                let choice = value("--color")?;
                options.color = match choice.as_str() {
                    "auto" => Color::Auto,
                    "always" => Color::Always,
                    "never" => Color::Never,
                    _ => {
                        return Err(Message::new("cli-invalid-value")
                            .text("option", "--color")
                            .text("value", choice));
                    }
                };
            }
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(Message::new("cli-unknown-argument").text("argument", other));
            }
            other => positional.push(other.to_owned()),
        }
    }
    let mut positional = positional.into_iter();
    options.command = match positional.next().as_deref() {
        None => Command::Help,
        Some("check") => {
            let files: Vec<String> = positional.collect();
            if files.is_empty() {
                return Err(Message::new("cli-no-files"));
            }
            Command::Check(files)
        }
        Some("explain") => {
            let code = positional
                .next()
                .ok_or_else(|| Message::new("cli-missing-code"))?;
            if let Some(extra) = positional.next() {
                return Err(Message::new("cli-unknown-argument").text("argument", extra));
            }
            Command::Explain(code)
        }
        Some(other) => {
            return Err(Message::new("cli-unknown-command").text("command", other));
        }
    };
    Ok(options)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let code = run(
        &args,
        &mut stdout.lock(),
        &mut stderr.lock(),
        stderr.is_terminal(),
    );
    ExitCode::from(code)
}

/// Runs the CLI with explicit streams, so tests can capture the output.
fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write, err_is_terminal: bool) -> u8 {
    // The language is needed to report an error in `--lang` itself: find it first.
    let early_language = args
        .windows(2)
        .find(|pair| pair.first().is_some_and(|a| a == "--lang"))
        .and_then(|pair| pair.get(1))
        .and_then(|tag| Language::from_tag(tag));
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(problem) => {
            let localizer = Localizer::new(early_language.unwrap_or_else(Language::from_env));
            let _ = writeln!(err, "{}", error_line(&localizer, &problem));
            let _ = writeln!(err, "{}", localizer.text("cli-usage"));
            return USAGE_OR_IO;
        }
    };
    let localizer = Localizer::new(options.language.unwrap_or_else(Language::from_env));
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let color = match options.color {
        Color::Always => true,
        Color::Never => false,
        Color::Auto => err_is_terminal && !no_color,
    };

    let written = match &options.command {
        Command::Help => writeln!(out, "{}", localizer.text("cli-usage")).map(|()| SUCCESS),
        Command::Version => writeln!(
            out,
            "{}",
            localizer.message(&Message::new("cli-version").text("version", VERSION))
        )
        .map(|()| SUCCESS),
        Command::Explain(code) => explain(code, &localizer, out, err),
        Command::Check(files) => check_files(files, &options, &localizer, color, out, err),
    };
    written.unwrap_or(USAGE_OR_IO)
}

fn explain(
    code: &str,
    localizer: &Localizer,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    if let Some(entry) = codes::parse(code) {
        write!(out, "{}", entry.docs(localizer.language()))?;
        Ok(SUCCESS)
    } else {
        let problem = Message::new("cli-unknown-code").text("code", code);
        writeln!(err, "{}", error_line(localizer, &problem))?;
        Ok(USAGE_OR_IO)
    }
}

fn check_files(
    files: &[String],
    options: &Options,
    localizer: &Localizer,
    color: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    let mut report = Report {
        version: json::FORMAT_VERSION,
        language: localizer.language().tag(),
        error_count: 0,
        files: Vec::new(),
    };
    let mut unreadable = false;
    let render_options = RenderOptions { color };

    for path in files {
        let (text, checked) = match read_source(Path::new(path)) {
            Ok(text) => {
                let checked = check(&text);
                (text, checked)
            }
            Err(ReadError::TooLarge(len)) => (
                String::new(),
                perasc_driver::Checked {
                    diagnostics: vec![perasc_driver::source_too_large(len)],
                    hidden: 0,
                },
            ),
            Err(problem) => {
                unreadable = true;
                let message = match problem {
                    ReadError::NotASource => Message::new("cli-not-a-source"),
                    ReadError::NotFound => Message::new("cli-not-found"),
                    ReadError::NotUtf8 => Message::new("cli-not-utf8"),
                    ReadError::Io(e) => {
                        Message::new("cli-read-error").text("reason", e.to_string())
                    }
                    ReadError::TooLarge(_) => unreachable_message(),
                };
                let message = message.text("path", path.clone());
                writeln!(err, "{}", error_line(localizer, &message))?;
                continue;
            }
        };
        let file = SourceFile::new(path, &text);
        report.error_count += checked.error_count();
        if options.json {
            report.files.push(FileReport {
                path: path.clone(),
                diagnostics: checked
                    .diagnostics
                    .iter()
                    .map(|d| json::diagnostic(d, &file, localizer))
                    .collect(),
                hidden_count: checked.hidden,
            });
        } else {
            for diagnostic in &checked.diagnostics {
                writeln!(
                    err,
                    "{}",
                    render(diagnostic, &file, localizer, render_options)
                )?;
            }
            if checked.hidden > 0 {
                let hidden = Message::new("cli-hidden-errors")
                    .number("count", to_i64(checked.hidden))
                    .number("limit", to_i64(MAX_ERRORS_PER_FILE))
                    .text("path", path.clone());
                writeln!(err, "{}\n", localizer.message(&hidden))?;
            }
        }
    }

    if options.json {
        let text = serde_json::to_string_pretty(&report).map_err(io::Error::other)?;
        writeln!(out, "{text}")?;
    } else if report.error_count > 0 {
        let summary =
            Message::new("cli-summary-errors").number("errors", to_i64(report.error_count));
        writeln!(err, "{}", localizer.message(&summary))?;
    } else if !unreadable {
        let summary = Message::new("cli-summary-ok").number("files", to_i64(files.len()));
        writeln!(out, "{}", localizer.message(&summary))?;
    }

    Ok(if unreadable {
        USAGE_OR_IO
    } else if report.error_count > 0 {
        ERRORS_FOUND
    } else {
        SUCCESS
    })
}

/// `error: <message>`, with the punctuation of the language.
fn error_line(localizer: &Localizer, message: &Message) -> String {
    format!(
        "{}{} {}",
        localizer.text("diag-error"),
        colon(localizer.language()),
        localizer.message(message)
    )
}

/// `ReadError::TooLarge` is handled before; this keeps the match exhaustive without panicking.
fn unreachable_message() -> Message {
    Message::new("cli-read-error").text("reason", "too large")
}

fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests;
