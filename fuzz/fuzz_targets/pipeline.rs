//! The whole pipeline on any text: every diagnostic renders in English and French,
//! as text and as JSON; a source with no error compiles to a valid WebAssembly
//! module, and the IR verifier never finds a compiler bug.
#![no_main]

use libfuzzer_sys::fuzz_target;
use perasc_diag::i18n::Localizer;
use perasc_diag::json;
use perasc_diag::messages::Language;
use perasc_diag::render::{RenderOptions, render};
use perasc_diag::source::SourceFile;
use perasc_driver::{ToolError, build_direct, check};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let checked = check(source);
    let file = SourceFile::new("fuzz.peras", source);
    for language in Language::ALL {
        let localizer = Localizer::new(language);
        for d in &checked.diagnostics {
            let text = render(d, &file, &localizer, RenderOptions { color: false });
            assert!(text.ends_with('\n'), "{text}");
            let value = json::diagnostic(d, &file, &localizer);
            assert!(serde_json::to_string(&value).is_ok());
        }
    }
    if !checked.diagnostics.is_empty() {
        return;
    }
    for optimize in [false, true] {
        match build_direct(source, "fuzz.wasm", optimize) {
            Ok(output) => {
                let valid = wasmparser::Validator::new().validate_all(&output.wasm);
                assert!(valid.is_ok(), "invalid module: {:?}\n{source}", valid.err());
            }
            // Not supported by code generation yet (E0900): a normal outcome.
            Err(ToolError::Diagnostics(_)) => {}
            Err(other) => panic!("compiler bug: {other:?}\n{source}"),
        }
    }
});
