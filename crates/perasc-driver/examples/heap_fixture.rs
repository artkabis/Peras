//! Writes the heap fixtures as modules and loaders, for the JavaScript tests:
//! `cargo run -p perasc-driver --example heap_fixture -- <dir> <en|fr>`.

use std::path::PathBuf;
use std::{env, fs};

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;

#[path = "../tests/fixtures/heap_ir.rs"]
mod heap_ir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let dir = PathBuf::from(args.next().ok_or("usage: heap_fixture <dir> <en|fr>")?);
    let tag = args.next().unwrap_or_else(|| "en".to_owned());
    let language = Language::from_tag(&tag).ok_or("language: en or fr")?;
    let localizer = Localizer::new(language);
    fs::create_dir_all(&dir)?;
    for (name, module) in [
        ("work", heap_ir::work()),
        ("twice", heap_ir::twice()),
        ("keep", heap_ir::keep()),
    ] {
        let wasm_file = format!("{name}.wasm");
        let output = perasc_driver::emit_direct(&module, &wasm_file, &localizer);
        fs::write(dir.join(&wasm_file), &output.wasm)?;
        fs::write(dir.join(format!("{name}.js")), &output.js)?;
    }
    Ok(())
}
