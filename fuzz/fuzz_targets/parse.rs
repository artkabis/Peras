//! The parser never panics nor overflows its stack, whatever the nesting, and
//! every diagnostic points inside the source.
#![no_main]

use libfuzzer_sys::fuzz_target;
use perasc_syntax::parse;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let parsed = parse(source);
    let len = u32::try_from(source.len()).unwrap_or(u32::MAX);
    for d in &parsed.diagnostics {
        for label in &d.labels {
            assert!(
                label.span.start <= label.span.end && label.span.end <= len,
                "{d:?}"
            );
        }
    }
});
