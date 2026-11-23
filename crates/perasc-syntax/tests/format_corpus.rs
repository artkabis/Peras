//! `perasc fmt` on every Peras file of the repository (decision 0041): a
//! formatted file holds the same tokens and the same comments, in order, but
//! for a trailing comma; formatting it again changes nothing.

use std::path::Path;
use std::process::Command;

use perasc_syntax::format::format;
use perasc_syntax::{TokenKind, lex};

/// The tokens' texts, a comma before a closing bracket left out, and the
/// comments, in order.
fn kept(source: &str) -> (Vec<String>, Vec<String>) {
    let tokens = lex(source).tokens;
    let mut texts = Vec::new();
    let mut comments = Vec::new();
    let mut at = 0_usize;
    for (i, t) in tokens.iter().enumerate() {
        let (start, end) = (t.span.start as usize, t.span.end as usize);
        let gap = &source[at..start];
        let mut rest = gap;
        while let Some(k) = rest.find("//").into_iter().chain(rest.find("/*")).min() {
            rest = &rest[k..];
            let end = if rest.starts_with("//") {
                rest.find('\n').unwrap_or(rest.len())
            } else {
                rest.find("*/").map_or(rest.len(), |e| e + 2)
            };
            comments.push(rest[..end].trim_end().to_owned());
            rest = &rest[end..];
        }
        at = end;
        let before_close = t.kind == TokenKind::Comma
            && tokens.get(i + 1).is_some_and(|n| {
                matches!(
                    n.kind,
                    TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace
                )
            });
        if !before_close {
            texts.push(source[start..end].to_owned());
        }
    }
    (texts, comments)
}

#[test]
fn every_file_keeps_its_tokens_and_comments() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let listed = Command::new("git")
        .args(["ls-files", "*.peras", "*.apeiron"])
        .current_dir(&root)
        .output()
        .map(|o| o.stdout)
        .unwrap_or_default();
    let files: Vec<String> = String::from_utf8_lossy(&listed)
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(files.len() > 300, "the corpus is found: {}", files.len());
    let mut failures = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(root.join(file)).unwrap_or_default();
        let Ok(once) = format(&source) else {
            continue;
        };
        if kept(&once) != kept(&source) {
            failures.push(format!("{file}: a token or a comment changed"));
        }
        if format(&once).ok().as_deref() != Some(once.as_str()) {
            failures.push(format!("{file}: not idempotent"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
