//! A module and its runtime in one: the runtime's functions, memory, globals
//! and data first, then the module's, renumbered, and each call the module
//! made through an import of the runtime made to the function itself. A call
//! between two instances is a call the engine does not inline; inside one
//! module, it is (decision 0016, finding F212).
//!
//! The runtime's exports stay, named `peras_rt.` and their name, a name no
//! Peras function can have; the loader reads them there. The module's imports
//! that the runtime does not give, `bounds` and the functions of `Math`, stay
//! imports. A runtime that imports its memory, built for threads, is not
//! linked: its memory is shared with other instances.

use wasm_encoder::reencode::{Error as ReencodeError, Reencode, RoundtripReencoder};
use wasm_encoder::{
    CodeSection, DataCountSection, DataSection, EntityType, ExportKind, ExportSection,
    FunctionSection, GlobalSection, ImportSection, MemorySection, StartSection, TypeSection,
};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef};

use crate::prune::PruneError;

/// The prefix of the runtime's exports in a linked module.
pub const RUNTIME_EXPORT: &str = "peras_rt.";

/// The module that a module not linked imports its runtime from.
const RUNTIME_MODULE: &str = "peras_rt";

/// What is read of one module.
#[derive(Default)]
struct Parts<'a> {
    types: Option<wasmparser::TypeSectionReader<'a>>,
    imports: Vec<wasmparser::Import<'a>>,
    functions: Vec<u32>,
    memories: Vec<wasmparser::MemoryType>,
    globals: Option<wasmparser::GlobalSectionReader<'a>>,
    exports: Vec<wasmparser::Export<'a>>,
    start: Option<u32>,
    bodies: Vec<wasmparser::FunctionBody<'a>>,
    data: Option<wasmparser::DataSectionReader<'a>>,
    data_count: u32,
    type_count: u32,
    global_count: u32,
}

impl Parts<'_> {
    fn imported_functions(&self) -> u32 {
        u32::try_from(
            self.imports
                .iter()
                .filter(|i| matches!(i.ty, TypeRef::Func(_)))
                .count(),
        )
        .unwrap_or(u32::MAX)
    }
}

fn read(wasm: &[u8]) -> Result<Parts<'_>, PruneError> {
    let mut parts = Parts::default();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.map_err(|e| e.to_string())? {
            Payload::TypeSection(section) => {
                parts.type_count = section.count();
                parts.types = Some(section);
            }
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    parts.imports.push(import.map_err(|e| e.to_string())?);
                }
            }
            Payload::FunctionSection(section) => {
                for ty in section {
                    parts.functions.push(ty.map_err(|e| e.to_string())?);
                }
            }
            Payload::MemorySection(section) => {
                for memory in section {
                    parts.memories.push(memory.map_err(|e| e.to_string())?);
                }
            }
            Payload::GlobalSection(section) => {
                parts.global_count = section.count();
                parts.globals = Some(section);
            }
            Payload::ExportSection(section) => {
                for export in section {
                    parts.exports.push(export.map_err(|e| e.to_string())?);
                }
            }
            Payload::StartSection { func, .. } => parts.start = Some(func),
            Payload::CodeSectionEntry(body) => parts.bodies.push(body),
            Payload::DataSection(section) => {
                parts.data_count = section.count();
                parts.data = Some(section);
            }
            Payload::TableSection(_) | Payload::ElementSection(_) | Payload::TagSection(_) => {
                return Err("a module with tables, elements or tags is not linked".to_owned());
            }
            _ => {}
        }
    }
    Ok(parts)
}

/// The new index of every function, type, global and data segment of one of
/// the two modules.
struct Renumber {
    functions: Vec<u32>,
    types: u32,
    globals: u32,
    data: u32,
}

impl Reencode for Renumber {
    type Error = PruneError;

    fn function_index(&mut self, func: u32) -> Result<u32, ReencodeError<PruneError>> {
        self.functions
            .get(usize::try_from(func).unwrap_or(usize::MAX))
            .copied()
            .ok_or_else(|| ReencodeError::UserError(format!("function {func} is out of range")))
    }

    fn type_index(&mut self, ty: u32) -> Result<u32, ReencodeError<PruneError>> {
        Ok(self.types + ty)
    }

    fn global_index(&mut self, global: u32) -> Result<u32, ReencodeError<PruneError>> {
        Ok(self.globals + global)
    }

    fn data_index(&mut self, data: u32) -> Result<u32, ReencodeError<PruneError>> {
        Ok(self.data + data)
    }
}

fn err(e: impl std::fmt::Debug) -> PruneError {
    format!("{e:?}")
}

/// `module`, which imports its runtime from `peras_rt`, with `runtime` linked
/// into it.
///
/// # Errors
///
/// When either is not a module this crate and perasc build in that shape:
/// the runtime imports its memory, or either has a table.
pub fn link(runtime: &[u8], module: &[u8]) -> Result<Vec<u8>, PruneError> {
    let r = read(runtime)?;
    let m = read(module)?;
    if r.memories.len() != 1 || r.imports.iter().any(|i| !matches!(i.ty, TypeRef::Func(_))) {
        return Err("the runtime must define its memory and import only functions".to_owned());
    }
    if !m.memories.is_empty() || m.imports.iter().any(|i| matches!(i.ty, TypeRef::Global(_))) {
        return Err("the module must import its memory and no global".to_owned());
    }
    let (mut r_map, mut m_map, kept) = plan(&r, &m)?;
    let mut out = wasm_encoder::Module::new();
    head(&mut out, &r, &m, &mut r_map, &mut m_map, &kept)?;
    out.section(&exports(&r, &m, &mut r_map, &mut m_map)?);
    tail(&mut out, r, m, &mut r_map, &mut m_map)?;
    Ok(out.finish())
}

/// The module's function imports the runtime does not give: kept as imports.
type Kept<'a> = Vec<(&'a str, &'a str, u32)>;

/// How the runtime's and the module's indices are renumbered, and the
/// imports kept.
fn plan<'a>(r: &Parts<'a>, m: &Parts<'a>) -> Result<(Renumber, Renumber, Kept<'a>), PruneError> {
    let r_imported = r.imported_functions();
    let r_defined = u32::try_from(r.functions.len()).map_err(err)?;
    // The runtime's functions by export name.
    let exported = |name: &str| {
        r.exports
            .iter()
            .find(|e| e.kind == ExternalKind::Func && e.name == name)
            .map(|e| e.index)
    };
    // The module's function imports: given by the runtime, or kept.
    let mut kept = Vec::new();
    let mut module_imports = Vec::new();
    for import in &m.imports {
        if let TypeRef::Func(ty) = import.ty {
            if let Some(index) = (import.module == RUNTIME_MODULE)
                .then(|| exported(import.name))
                .flatten()
            {
                module_imports.push(Err(index));
            } else {
                module_imports.push(Ok(u32::try_from(kept.len()).map_err(err)?));
                kept.push((import.module, import.name, ty));
            }
        }
    }
    let kept_count = u32::try_from(kept.len()).map_err(err)?;
    let r_base = r_imported + kept_count;
    let m_base = r_base + r_defined;
    let runtime_function = |f: u32| {
        if f < r_imported {
            f
        } else {
            r_base + (f - r_imported)
        }
    };
    let r_map = Renumber {
        functions: (0..r_imported + r_defined).map(runtime_function).collect(),
        types: 0,
        globals: 0,
        data: 0,
    };
    let mut m_functions: Vec<u32> = module_imports
        .iter()
        .map(|i| match i {
            Ok(k) => r_imported + k,
            Err(index) => runtime_function(*index),
        })
        .collect();
    m_functions.extend((0..u32::try_from(m.functions.len()).map_err(err)?).map(|f| m_base + f));
    let m_map = Renumber {
        functions: m_functions,
        types: r.type_count,
        globals: r.global_count,
        data: r.data_count,
    };
    Ok((r_map, m_map, kept))
}

/// The types, imports, functions, memory and globals of both, in order.
fn head(
    out: &mut wasm_encoder::Module,
    r: &Parts<'_>,
    m: &Parts<'_>,
    r_map: &mut Renumber,
    m_map: &mut Renumber,
    kept: &Kept<'_>,
) -> Result<(), PruneError> {
    let mut types = TypeSection::new();
    if let Some(section) = r.types.clone() {
        r_map.parse_type_section(&mut types, section).map_err(err)?;
    }
    if let Some(section) = m.types.clone() {
        m_map.parse_type_section(&mut types, section).map_err(err)?;
    }
    out.section(&types);

    let mut imports = ImportSection::new();
    for import in &r.imports {
        let ty = r_map.entity_type(import.ty).map_err(err)?;
        imports.import(import.module, import.name, ty);
    }
    for (module_name, name, ty) in kept {
        let ty = m_map.type_index(*ty).map_err(err)?;
        imports.import(module_name, name, EntityType::Function(ty));
    }
    out.section(&imports);

    let mut functions = FunctionSection::new();
    for &ty in &r.functions {
        functions.function(r_map.type_index(ty).map_err(err)?);
    }
    for &ty in &m.functions {
        functions.function(m_map.type_index(ty).map_err(err)?);
    }
    out.section(&functions);

    let mut memories = MemorySection::new();
    for &memory in &r.memories {
        memories.memory(RoundtripReencoder.memory_type(memory).map_err(err)?);
    }
    out.section(&memories);

    let mut globals = GlobalSection::new();
    if let Some(section) = r.globals.clone() {
        r_map
            .parse_global_section(&mut globals, section)
            .map_err(err)?;
    }
    if let Some(section) = m.globals.clone() {
        m_map
            .parse_global_section(&mut globals, section)
            .map_err(err)?;
    }
    out.section(&globals);
    Ok(())
}

/// The module's exports, then the runtime's, named [`RUNTIME_EXPORT`] and
/// their name.
fn exports(
    r: &Parts<'_>,
    m: &Parts<'_>,
    r_map: &mut Renumber,
    m_map: &mut Renumber,
) -> Result<ExportSection, PruneError> {
    let mut exports = ExportSection::new();
    for export in &m.exports {
        let (kind, index) = match export.kind {
            ExternalKind::Func => (
                ExportKind::Func,
                m_map.function_index(export.index).map_err(err)?,
            ),
            ExternalKind::Global => (
                ExportKind::Global,
                m_map.global_index(export.index).map_err(err)?,
            ),
            other => return Err(format!("the module exports a {other:?}")),
        };
        exports.export(export.name, kind, index);
    }
    for export in &r.exports {
        let (kind, index) = match export.kind {
            ExternalKind::Func => (
                ExportKind::Func,
                r_map.function_index(export.index).map_err(err)?,
            ),
            ExternalKind::Global => (ExportKind::Global, export.index),
            ExternalKind::Memory => (ExportKind::Memory, export.index),
            other => return Err(format!("the runtime exports a {other:?}")),
        };
        exports.export(&format!("{RUNTIME_EXPORT}{}", export.name), kind, index);
    }
    Ok(exports)
}

/// The start function, the code and the data of both, in order.
fn tail(
    out: &mut wasm_encoder::Module,
    r: Parts<'_>,
    m: Parts<'_>,
    r_map: &mut Renumber,
    m_map: &mut Renumber,
) -> Result<(), PruneError> {
    if let Some(start) = r.start {
        out.section(&StartSection {
            function_index: r_map.function_index(start).map_err(err)?,
        });
    }
    let data_count = r.data_count + m.data_count;
    if data_count > 0 {
        out.section(&DataCountSection { count: data_count });
    }

    let mut code = CodeSection::new();
    for body in r.bodies {
        r_map.parse_function_body(&mut code, body).map_err(err)?;
    }
    for body in m.bodies {
        m_map.parse_function_body(&mut code, body).map_err(err)?;
    }
    out.section(&code);

    if data_count > 0 {
        let mut data = DataSection::new();
        if let Some(section) = r.data {
            r_map.parse_data_section(&mut data, section).map_err(err)?;
        }
        if let Some(section) = m.data {
            m_map.parse_data_section(&mut data, section).map_err(err)?;
        }
        out.section(&data);
    }
    Ok(())
}
