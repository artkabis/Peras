//! The `--json` output: the same content as the terminal rendering, structured for
//! CI, the Vite overlay and the future LSP.
//!
//! Every message carries its text in the chosen language, plus its Fluent ID and
//! arguments, so a tool can show it in another language. Positions carry the byte
//! offset and the line and column (from 1, columns in characters).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::i18n::Localizer;
use crate::source::SourceFile;
use crate::{Arg, Diagnostic, Message, Span};

/// Version of this format. Bumped on any breaking change.
pub const FORMAT_VERSION: u32 = 1;

/// The whole report of one compiler run.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// [`FORMAT_VERSION`].
    pub version: u32,
    /// `en` or `fr`.
    pub language: &'static str,
    /// Total number of errors, hidden ones included.
    pub error_count: usize,
    /// One entry per checked file, in the order given.
    pub files: Vec<FileReport>,
    /// For `perasc build`, the files the module was compiled from, the
    /// imported ones first and the file built last, so that a bundler can
    /// watch them all (decision 0022). Absent when not known.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
}

/// The errors of one file.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileReport {
    /// The path as given.
    pub path: String,
    /// Diagnostics shown, sorted by position.
    pub diagnostics: Vec<JsonDiagnostic>,
    /// Errors beyond the per-file limit, counted but not listed.
    pub hidden_count: usize,
}

/// A diagnostic.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonDiagnostic {
    /// `E0001`.
    pub code: String,
    /// Always `error`: Peras has no warnings.
    pub severity: &'static str,
    /// Title.
    pub message: JsonMessage,
    /// Underlined spans.
    pub labels: Vec<JsonLabel>,
    /// Why it is an error.
    pub cause: JsonMessage,
    /// The suggested fix.
    pub fix: JsonFix,
    /// Command showing the full explanation.
    pub explain: String,
}

/// A resolved message.
#[derive(Debug, Serialize)]
pub struct JsonMessage {
    /// Text in the report's language.
    pub text: String,
    /// Fluent message ID.
    pub id: &'static str,
    /// Fluent arguments.
    pub args: BTreeMap<&'static str, serde_json::Value>,
}

/// A label.
#[derive(Debug, Serialize)]
pub struct JsonLabel {
    /// Whether it marks where the error is.
    pub primary: bool,
    /// The underlined source.
    pub range: JsonRange,
    /// What it shows.
    pub message: JsonMessage,
}

/// A fix.
#[derive(Debug, Serialize)]
pub struct JsonFix {
    /// What it does.
    pub message: JsonMessage,
    /// Edits that apply it; empty when the fix depends on intent.
    pub edits: Vec<JsonEdit>,
}

/// An edit.
#[derive(Debug, Serialize)]
pub struct JsonEdit {
    /// The source replaced.
    pub range: JsonRange,
    /// The new text.
    pub replacement: String,
}

/// A range of source.
#[derive(Debug, Serialize)]
pub struct JsonRange {
    /// First position.
    pub start: JsonPosition,
    /// Position one past the end.
    pub end: JsonPosition,
}

/// A position.
#[derive(Debug, Serialize)]
pub struct JsonPosition {
    /// Byte offset.
    pub offset: u32,
    /// Line, from 1.
    pub line: usize,
    /// Column in characters, from 1.
    pub column: usize,
}

fn position(file: &SourceFile<'_>, offset: u32) -> JsonPosition {
    let p = file.position(offset);
    JsonPosition {
        offset,
        line: p.line,
        column: p.column,
    }
}

fn range(file: &SourceFile<'_>, span: Span) -> JsonRange {
    JsonRange {
        start: position(file, span.start),
        end: position(file, span.end),
    }
}

fn message(localizer: &Localizer, m: &Message) -> JsonMessage {
    JsonMessage {
        text: localizer.message(m),
        id: m.id,
        args: m
            .args
            .iter()
            .map(|(name, value)| {
                let value = match value {
                    Arg::Text(text) => serde_json::Value::from(text.as_str()),
                    Arg::Number(n) => serde_json::Value::from(*n),
                };
                (*name, value)
            })
            .collect(),
    }
}

/// Converts a diagnostic of `file`.
#[must_use]
pub fn diagnostic(d: &Diagnostic, file: &SourceFile<'_>, localizer: &Localizer) -> JsonDiagnostic {
    JsonDiagnostic {
        code: d.code.to_string(),
        severity: "error",
        message: message(localizer, &d.message),
        labels: d
            .labels
            .iter()
            .map(|label| JsonLabel {
                primary: label.primary,
                range: range(file, label.span),
                message: message(localizer, &label.message),
            })
            .collect(),
        cause: message(localizer, &d.cause),
        fix: JsonFix {
            message: message(localizer, &d.fix.message),
            edits: d
                .fix
                .edits
                .iter()
                .map(|edit| JsonEdit {
                    range: range(file, edit.span),
                    replacement: edit.replacement.clone(),
                })
                .collect(),
        },
        explain: format!("perasc explain {}", d.code),
    }
}
