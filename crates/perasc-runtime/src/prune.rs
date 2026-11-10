//! The runtime one module needs: the functions it exports that the module or
//! its loader use, and those they call, nothing else. The allocator module
//! holds every function of the runtime; a module that only adds numbers in an
//! array does not need `split` or the radix sort. Pruning keeps the exports
//! asked for, follows calls from them, renumbers the functions kept, and
//! drops the names section, which only debuggers read.

use std::collections::BTreeSet;

use wasm_encoder::reencode::{Error as ReencodeError, Reencode, utils};
use wasmparser::{ExternalKind, Operator, Parser, Payload, TypeRef};

/// Why a module could not be pruned: it is not one this crate built.
pub type PruneError = String;

/// What pruning learns from a first reading of the module.
struct Shape {
    /// Functions imported, which come before the module's own.
    imported: u32,
    /// For each of the module's own functions, the functions it calls or
    /// takes a reference to.
    callees: Vec<Vec<u32>>,
    /// Functions that must stay whatever is asked: the start function, and
    /// those an element segment or a global refers to.
    pinned: Vec<u32>,
    /// Every export: its name, kind and index.
    exports: Vec<(String, ExternalKind, u32)>,
}

/// The functions an instruction sequence calls or refers to.
fn referred(
    reader: &mut wasmparser::OperatorsReader<'_>,
    out: &mut Vec<u32>,
) -> Result<(), PruneError> {
    while !reader.eof() {
        match reader.read().map_err(|e| e.to_string())? {
            Operator::Call { function_index }
            | Operator::ReturnCall { function_index }
            | Operator::RefFunc { function_index } => out.push(function_index),
            _ => {}
        }
    }
    Ok(())
}

/// Reads what pruning needs to know.
fn shape(wasm: &[u8]) -> Result<Shape, PruneError> {
    let mut shape = Shape {
        imported: 0,
        callees: Vec::new(),
        pinned: Vec::new(),
        exports: Vec::new(),
    };
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.map_err(|e| e.to_string())? {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    if matches!(import.map_err(|e| e.to_string())?.ty, TypeRef::Func(_)) {
                        shape.imported += 1;
                    }
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.map_err(|e| e.to_string())?;
                    shape
                        .exports
                        .push((export.name.to_owned(), export.kind, export.index));
                }
            }
            Payload::StartSection { func, .. } => shape.pinned.push(func),
            Payload::ElementSection(section) => {
                for element in section {
                    if let wasmparser::ElementItems::Functions(functions) =
                        element.map_err(|e| e.to_string())?.items
                    {
                        for f in functions {
                            shape.pinned.push(f.map_err(|e| e.to_string())?);
                        }
                    }
                }
            }
            Payload::GlobalSection(section) => {
                for global in section {
                    let global = global.map_err(|e| e.to_string())?;
                    referred(
                        &mut global.init_expr.get_operators_reader(),
                        &mut shape.pinned,
                    )?;
                }
            }
            Payload::CodeSectionEntry(body) => {
                let mut callees = Vec::new();
                referred(
                    &mut body.get_operators_reader().map_err(|e| e.to_string())?,
                    &mut callees,
                )?;
                shape.callees.push(callees);
            }
            _ => {}
        }
    }
    Ok(shape)
}

/// Re-encodes the module with only the functions in `kept`, renumbered.
struct Pruner {
    imported: u32,
    /// The new index of each function, `None` for one dropped.
    index: Vec<Option<u32>>,
    /// The names of the exports kept.
    keep: BTreeSet<String>,
    /// The position of the next function of the function and code sections.
    next_function: u32,
    next_body: u32,
}

impl Pruner {
    fn kept(&self, func: u32) -> bool {
        self.index
            .get(usize::try_from(func).unwrap_or(usize::MAX))
            .is_some_and(Option::is_some)
    }
}

impl Reencode for Pruner {
    type Error = PruneError;

    fn function_index(&mut self, func: u32) -> Result<u32, ReencodeError<PruneError>> {
        self.index
            .get(usize::try_from(func).unwrap_or(usize::MAX))
            .copied()
            .flatten()
            .ok_or_else(|| ReencodeError::UserError(format!("function {func} was dropped")))
    }

    fn parse_function_section(
        &mut self,
        functions: &mut wasm_encoder::FunctionSection,
        section: wasmparser::FunctionSectionReader<'_>,
    ) -> Result<(), ReencodeError<PruneError>> {
        for ty in section {
            let ty = ty?;
            let func = self.imported + self.next_function;
            self.next_function += 1;
            if self.kept(func) {
                functions.function(self.type_index(ty)?);
            }
        }
        Ok(())
    }

    fn parse_code_section(
        &mut self,
        code: &mut wasm_encoder::CodeSection,
        section: wasmparser::CodeSectionReader<'_>,
    ) -> Result<(), ReencodeError<PruneError>> {
        for body in section {
            let body = body?;
            let func = self.imported + self.next_body;
            self.next_body += 1;
            if self.kept(func) {
                self.parse_function_body(code, body)?;
            }
        }
        Ok(())
    }

    fn parse_export(
        &mut self,
        exports: &mut wasm_encoder::ExportSection,
        export: wasmparser::Export<'_>,
    ) -> Result<(), ReencodeError<PruneError>> {
        if self.keep.contains(export.name) {
            utils::parse_export(self, exports, export)?;
        }
        Ok(())
    }

    fn parse_custom_section(
        &mut self,
        _module: &mut wasm_encoder::Module,
        _section: wasmparser::CustomSectionReader<'_>,
    ) -> Result<(), ReencodeError<PruneError>> {
        // Names and other custom sections: debuggers read them, engines do not.
        Ok(())
    }
}

/// `wasm` with only the exports named in `keep` and the functions they reach.
/// An export named in `keep` that the module lacks is ignored.
///
/// # Errors
///
/// When `wasm` is not a module this crate built, which a test rules out.
pub fn prune(wasm: &[u8], keep: &[&str]) -> Result<Vec<u8>, PruneError> {
    let shape = shape(wasm)?;
    let total = shape.imported + u32::try_from(shape.callees.len()).map_err(|e| e.to_string())?;
    let mut reached = vec![false; usize::try_from(total).map_err(|e| e.to_string())?];
    // Imports stay: the module's imports are its contract with the loader.
    let mut stack: Vec<u32> = (0..shape.imported).collect();
    stack.extend(&shape.pinned);
    for (name, kind, index) in &shape.exports {
        if *kind == ExternalKind::Func && keep.contains(&name.as_str()) {
            stack.push(*index);
        }
    }
    while let Some(func) = stack.pop() {
        let Some(slot) = reached.get_mut(usize::try_from(func).map_err(|e| e.to_string())?) else {
            return Err(format!("function {func} is out of range"));
        };
        if *slot {
            continue;
        }
        *slot = true;
        if let Some(own) = func.checked_sub(shape.imported)
            && let Some(callees) = shape
                .callees
                .get(usize::try_from(own).map_err(|e| e.to_string())?)
        {
            stack.extend(callees);
        }
    }
    let mut next = 0;
    let index = reached
        .iter()
        .map(|&r| {
            r.then(|| {
                next += 1;
                next - 1
            })
        })
        .collect();
    let mut pruner = Pruner {
        imported: shape.imported,
        index,
        keep: keep.iter().map(|&k| k.to_owned()).collect(),
        next_function: 0,
        next_body: 0,
    };
    let mut module = wasm_encoder::Module::new();
    pruner
        .parse_core_module(&mut module, Parser::new(0), wasm)
        .map_err(|e| format!("{e:?}"))?;
    Ok(module.finish())
}
