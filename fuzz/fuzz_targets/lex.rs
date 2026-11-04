//! The lexer never panics, and its tokens stay inside the source, in order,
//! without overlapping, ending with one `Eof`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use perasc_syntax::{TokenKind, lex};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let lexed = lex(source);
    let len = u32::try_from(source.len()).unwrap_or(u32::MAX);
    let mut previous_end = 0;
    for token in &lexed.tokens {
        assert!(token.span.start <= token.span.end, "{token:?}");
        assert!(token.span.end <= len, "{token:?} past the end");
        assert!(token.span.start >= previous_end, "{token:?} overlaps");
        previous_end = token.span.end;
    }
    let eof = lexed
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Eof)
        .count();
    assert_eq!(eof, 1, "exactly one Eof");
    assert_eq!(lexed.tokens.last().map(|t| t.kind), Some(TokenKind::Eof));
    for d in &lexed.diagnostics {
        for label in &d.labels {
            assert!(label.span.end <= len, "{d:?} points past the end");
        }
    }
});
