//! The runtime of a module built with threads ([decision 0007]): its memory
//! imported instead of its own, so that the loader can give the same memory,
//! shared, to the runtime, the module and every worker.
//!
//! The memory is declared unshared, with a maximum. A page that can share
//! memory (cross-origin isolated) sets its shared bit before compiling: one
//! byte, at the offset [`memory_flag`] gives; any other page runs the module
//! as it is, on one thread.
//!
//! [decision 0007]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0007-threads-over-shared-memory.md

use wasm_encoder::reencode::{Error as ReencodeError, Reencode, utils};
use wasm_encoder::{EntityType, ImportSection, MemoryType};
use wasmparser::{Parser, Payload};

use crate::PruneError;

/// Re-encodes the runtime with its memory imported from `env.memory`.
struct Importer {
    pages: u64,
}

impl Reencode for Importer {
    type Error = PruneError;

    fn parse_import_section(
        &mut self,
        imports: &mut ImportSection,
        section: wasmparser::ImportSectionReader<'_>,
    ) -> Result<(), ReencodeError<PruneError>> {
        utils::parse_import_section(self, imports, section)?;
        imports.import(
            "env",
            "memory",
            EntityType::Memory(MemoryType {
                minimum: 1,
                maximum: Some(self.pages),
                memory64: false,
                shared: false,
                page_size_log2: None,
            }),
        );
        Ok(())
    }

    fn parse_memory_section(
        &mut self,
        _memories: &mut wasm_encoder::MemorySection,
        _section: wasmparser::MemorySectionReader<'_>,
    ) -> Result<(), ReencodeError<PruneError>> {
        // The memory is the one imported, index 0 still.
        Ok(())
    }
}

/// The runtime `wasm`, which defines its memory and imports functions, with
/// its memory imported from `env.memory` instead, of at most `pages` pages.
///
/// # Errors
///
/// When `wasm` is not a runtime this crate built, which a test rules out.
pub fn import_memory(wasm: &[u8], pages: u64) -> Result<Vec<u8>, PruneError> {
    let mut module = wasm_encoder::Module::new();
    Importer { pages }
        .parse_core_module(&mut module, Parser::new(0), wasm)
        .map_err(|e| format!("{e:?}"))?;
    Ok(module.finish())
}

/// Reads an unsigned LEB128 number at `at`, and moves past it.
fn leb(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut value = 0_u64;
    let mut shift = 0;
    loop {
        let byte = *bytes.get(*at)?;
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

/// The offset of the limits flag of the memory `wasm` imports: `0x01` for a
/// memory with a maximum, which `0x03` makes shared. `None` when it imports
/// no memory.
#[must_use]
pub fn memory_flag(wasm: &[u8]) -> Option<usize> {
    for payload in Parser::new(0).parse_all(wasm) {
        let Ok(Payload::ImportSection(section)) = payload else {
            continue;
        };
        let range = section.range();
        let mut at = usize::try_from(range.start).ok()?;
        let count = leb(wasm, &mut at)?;
        for _ in 0..count {
            for _ in 0..2 {
                let len = usize::try_from(leb(wasm, &mut at)?).ok()?;
                at += len;
            }
            let kind = *wasm.get(at)?;
            at += 1;
            match kind {
                // A function: its type.
                0x00 => {
                    leb(wasm, &mut at)?;
                }
                // A table: its element type, then its limits.
                0x01 => {
                    at += 1;
                    let flag = *wasm.get(at)?;
                    at += 1;
                    leb(wasm, &mut at)?;
                    if flag & 1 == 1 {
                        leb(wasm, &mut at)?;
                    }
                }
                0x02 => return Some(at),
                // A global: its type and mutability.
                0x03 => at += 2,
                _ => return None,
            }
        }
    }
    None
}
