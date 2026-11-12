//! Compilation pipeline, configuration and cargo/wasm-bindgen calls for the Peras compiler.
//!
//! The pipeline: lexer, parser, then names and types; each new stage plugs into [`check`].

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use perasc_diag::Diagnostic;
use perasc_diag::i18n::Localizer;
pub use perasc_syntax::Mode;
pub use perasc_syntax::source_too_large;

/// The compiler version, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// At most this many errors are shown per file; the rest are counted.
pub const MAX_ERRORS_PER_FILE: usize = 50;

/// The result of checking one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// Diagnostics shown, sorted by position, at most [`MAX_ERRORS_PER_FILE`].
    pub diagnostics: Vec<Diagnostic>,
    /// Errors beyond the limit.
    pub hidden: usize,
}

impl Checked {
    /// Every error, shown or hidden.
    #[must_use]
    pub const fn error_count(&self) -> usize {
        self.diagnostics.len() + self.hidden
    }

    fn capped(mut diagnostics: Vec<Diagnostic>) -> Self {
        let hidden = diagnostics.len().saturating_sub(MAX_ERRORS_PER_FILE);
        diagnostics.truncate(MAX_ERRORS_PER_FILE);
        Self {
            diagnostics,
            hidden,
        }
    }
}

/// Runs every available stage on a source and gathers all its errors.
///
/// Names and types are checked only on a file without syntax errors: on a broken
/// tree they would mostly report consequences of the syntax errors.
#[must_use]
pub fn check(source: &str) -> Checked {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Checked::capped(parsed.diagnostics);
    }
    Checked::capped(perasc_check::check(&parsed.ast, source))
}

/// Checks a file in the mode its extension gives: a managed module against
/// the owned modules it imports, read from beside it.
#[must_use]
pub fn check_file(path: &Path, source: &str) -> Checked {
    if Mode::from_path(path) == Some(Mode::Owned) {
        return check_owned(path, source);
    }
    if Mode::from_path(path) != Some(Mode::Managed) {
        return check(source);
    }
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Checked::capped(parsed.diagnostics);
    }
    let context = managed_context(path, &parsed.ast);
    Checked::capped(perasc_check::analyze_with(&parsed.ast, source, &context).diagnostics)
}

/// An owned module, checked against the files it imports (decision 0022).
/// When one of them does not compile, its import is reported here: that
/// file's own errors are for `perasc build` or a check of it.
fn check_owned(path: &Path, source: &str) -> Checked {
    match owned_graph(path, source) {
        Ok(_) => Checked::capped(Vec::new()),
        Err(ToolError::Diagnostics(checked)) => checked,
        Err(_) => {
            let parsed = perasc_syntax::parse(source);
            if !parsed.diagnostics.is_empty() {
                return Checked::capped(parsed.diagnostics);
            }
            let mut context = imports_context(path, &parsed.ast);
            context.managed = false;
            context.path = canonical(path).display().to_string();
            Checked::capped(perasc_check::analyze_with(&parsed.ast, source, &context).diagnostics)
        }
    }
}

/// The exported functions, structures and enumerations of the owned module
/// at `file`, checked with the files it imports; `None` when it cannot be.
fn owned_exports(
    file: &Path,
) -> Option<(Vec<perasc_check::ImportedFn>, perasc_check::ExportedTypes)> {
    let text = read_source(file).ok()?;
    let graph = owned_graph(file, &text).ok()?;
    let root = graph.units.last()?;
    Some((
        root.analysis.exports.clone(),
        root.analysis.exported_types.clone(),
    ))
}

/// The path an `import` of the module at `path` names, when it is an owned
/// module in the same folder or below it: `./x.peras` or `./lib/x.peras`,
/// never `../` nor an absolute path.
#[must_use]
pub fn import_path(path: &Path, from: &str) -> Option<std::path::PathBuf> {
    let relative = from.strip_prefix("./")?;
    let inside = Path::new(relative)
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)));
    if !inside || Mode::from_path(Path::new(relative)) != Some(Mode::Owned) {
        return None;
    }
    Some(
        path.parent()
            .unwrap_or_else(|| Path::new(""))
            .join(relative),
    )
}

/// What a managed module is checked against: the exported functions of each
/// owned module it imports, read and checked; `None` for one that cannot be.
fn managed_context(path: &Path, ast: &perasc_syntax::ast::Ast) -> perasc_check::Context {
    imports_context(path, ast)
}

/// The modules the imports of the module at `path` name, each checked with
/// the files it imports in turn.
fn imports_context(path: &Path, ast: &perasc_syntax::ast::Ast) -> perasc_check::Context {
    let imports = ast
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            perasc_syntax::ast::ItemKind::Import(decl) => Some(decl.from.clone()),
            _ => None,
        })
        .map(|from| {
            let exports = import_path(path, &from).and_then(|file| owned_exports(&file));
            let (functions, types) = match exports {
                Some((functions, types)) => (Some(functions), types),
                None => (None, perasc_check::ExportedTypes::default()),
            };
            perasc_check::ImportedModule {
                from,
                functions,
                types,
            }
        })
        .collect();
    perasc_check::Context {
        managed: true,
        imports,
        path: String::new(),
    }
}

/// One owned file of a build: read, parsed, checked against the files it
/// imports (decision 0022).
pub struct GraphUnit {
    /// Its path, as reached from the root.
    pub path: std::path::PathBuf,
    /// Its text.
    pub source: String,
    /// Its syntax tree.
    pub ast: perasc_syntax::ast::Ast,
    /// Its checks.
    pub analysis: perasc_check::Analysis,
    /// The unit each of its imports names, by the path the import writes.
    pub imports: std::collections::HashMap<String, usize>,
}

/// The owned files a build reaches from its root, each checked: the files
/// it imports before it, the root last (decision 0022).
pub struct Graph {
    /// The files, in that order.
    pub units: Vec<GraphUnit>,
}

impl Graph {
    /// The paths of its files, the root last.
    #[must_use]
    pub fn paths(&self) -> Vec<std::path::PathBuf> {
        self.units.iter().map(|u| u.path.clone()).collect()
    }
}

/// Reads, parses and checks an owned module and every owned module it
/// imports, through the files it reaches, each checked against what the
/// files it imports export (decision 0022).
///
/// # Errors
///
/// The errors of the root as [`ToolError::Diagnostics`]; those of another
/// file, a cycle of imports among them, as [`ToolError::InFile`].
pub fn owned_graph(path: &Path, source: &str) -> Result<Graph, ToolError> {
    let mut walk = Walk {
        units: Vec::new(),
        done: std::collections::HashMap::new(),
        stack: Vec::new(),
        base: canonical(path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
    };
    walk.visit(path, source.to_owned())?;
    Ok(Graph { units: walk.units })
}

/// The path, resolved, so that a file reached by two paths is one.
fn canonical(path: &Path) -> std::path::PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The walk of [`owned_graph`]: the files checked, and those being.
struct Walk {
    units: Vec<GraphUnit>,
    done: std::collections::HashMap<std::path::PathBuf, usize>,
    stack: Vec<std::path::PathBuf>,
    /// The root's folder, from which the files of a cycle are named.
    base: std::path::PathBuf,
}

impl Walk {
    /// A file's name as a cycle shows it: from the root's folder.
    fn shown(&self, path: &Path) -> String {
        path.strip_prefix(&self.base)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    /// Checks the file at `path`, after the files it imports; its index.
    fn visit(&mut self, path: &Path, source: String) -> Result<usize, ToolError> {
        let root = self.stack.is_empty();
        let failed = |source: String, diagnostics: Vec<Diagnostic>| {
            let checked = Checked::capped(diagnostics);
            if root {
                ToolError::Diagnostics(checked)
            } else {
                ToolError::InFile {
                    path: path.to_path_buf(),
                    source,
                    checked,
                }
            }
        };
        let parsed = perasc_syntax::parse(&source);
        if !parsed.diagnostics.is_empty() {
            return Err(failed(source, parsed.diagnostics));
        }
        let here = canonical(path);
        self.stack.push(here.clone());
        let mut imports = std::collections::HashMap::new();
        let mut modules = Vec::new();
        for item in &parsed.ast.items {
            let perasc_syntax::ast::ItemKind::Import(decl) = &item.kind else {
                continue;
            };
            let file = import_path(path, &decl.from).filter(|f| f.is_file());
            let mut module = perasc_check::ImportedModule {
                from: decl.from.clone(),
                functions: None,
                types: perasc_check::ExportedTypes::default(),
            };
            if let Some(file) = file {
                let target = canonical(&file);
                if let Some(at) = self.stack.iter().position(|p| *p == target) {
                    let mut names: Vec<String> =
                        self.stack[at..].iter().map(|p| self.shown(p)).collect();
                    names.push(self.shown(&target));
                    let cycle = perasc_check::import_cycle(decl.from_span, &names.join(" → "));
                    self.stack.pop();
                    return Err(failed(source, vec![cycle]));
                }
                let index = match self.done.get(&target) {
                    Some(&index) => Some(index),
                    None => match read_source(&file) {
                        Ok(text) => Some(self.visit(&file, text)?),
                        Err(_) => None,
                    },
                };
                if let Some(unit) = index.and_then(|i| self.units.get(i).map(|u| (i, u))) {
                    imports.insert(decl.from.clone(), unit.0);
                    module.functions = Some(unit.1.analysis.exports.clone());
                    module.types = unit.1.analysis.exported_types.clone();
                }
            }
            modules.push(module);
        }
        self.stack.pop();
        let context = perasc_check::Context {
            managed: false,
            imports: modules,
            path: here.display().to_string(),
        };
        let analysis = perasc_check::analyze_with(&parsed.ast, &source, &context);
        if !analysis.diagnostics.is_empty() {
            return Err(failed(source, analysis.diagnostics));
        }
        let index = self.units.len();
        self.done.insert(here, index);
        self.units.push(GraphUnit {
            path: path.to_path_buf(),
            source,
            ast: parsed.ast,
            analysis,
            imports,
        });
        Ok(index)
    }
}

/// An owned module and the files it imports, lowered into one module of the
/// IR and verified (decision 0022); the paths of its files, the root last.
///
/// # Errors
///
/// As [`owned_graph`], and the code generation limits (`E0900`) of each file.
pub fn compile_graph_ir(
    path: &Path,
    source: &str,
    overflow_checks: bool,
) -> Result<(perasc_ir::Module, Vec<std::path::PathBuf>), ToolError> {
    let graph = owned_graph(path, source)?;
    let units: Vec<perasc_ir::Unit<'_>> = graph
        .units
        .iter()
        .map(|u| perasc_ir::Unit {
            ast: &u.ast,
            analysis: &u.analysis,
            imports: u.imports.clone(),
        })
        .collect();
    let root = graph.units.len().saturating_sub(1);
    let module = perasc_ir::lower_units(&units, overflow_checks).map_err(|failed| {
        let (index, diagnostics) = failed.into_iter().next().unwrap_or_default();
        let checked = Checked::capped(diagnostics);
        match graph.units.get(index) {
            Some(unit) if index != root => ToolError::InFile {
                path: unit.path.clone(),
                source: unit.source.clone(),
                checked,
            },
            _ => ToolError::Diagnostics(checked),
        }
    })?;
    perasc_ir::verify(&module).map_err(ToolError::Internal)?;
    Ok((module, graph.paths()))
}

/// What a managed module compiles to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedOutput {
    /// The ES module.
    pub js: String,
    /// TypeScript declarations of its exports.
    pub dts: String,
    /// The owned modules it imports, as written, with their paths.
    pub imports: Vec<(String, std::path::PathBuf)>,
}

/// Checks a managed module and compiles it to JavaScript. With
/// `peras_imports_as_js`, its imports of `./x.peras` name `./x.js`, the
/// loader [`build_direct`] writes. With `debug`, the module reports a
/// resource collected without being released, with its file and line.
///
/// # Errors
///
/// The errors of the first stage that has any: syntax, names and types, or
/// what JavaScript generation does not support yet (`E0900`).
pub fn build_managed(
    path: &Path,
    source: &str,
    peras_imports_as_js: bool,
    debug: bool,
    localizer: &Localizer,
) -> Result<ManagedOutput, ToolError> {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Err(ToolError::Diagnostics(Checked::capped(parsed.diagnostics)));
    }
    let context = managed_context(path, &parsed.ast);
    let analysis = perasc_check::analyze_with(&parsed.ast, source, &context);
    if !analysis.diagnostics.is_empty() {
        return Err(ToolError::Diagnostics(Checked::capped(
            analysis.diagnostics,
        )));
    }
    let bounds = localizer.message(
        &perasc_diag::Message::new("runtime-index")
            .text("index", "{index}")
            .text("length", "{length}"),
    );
    let file = path.file_name().map_or_else(
        || path.display().to_string(),
        |f| f.to_string_lossy().into_owned(),
    );
    let overflow = localizer.message(&perasc_diag::Message::new("runtime-integer-overflow"));
    let options = perasc_codegen_js::Options {
        peras_imports_as_js,
        bounds,
        overflow,
        debug: debug.then(|| perasc_codegen_js::Debug::new(&file, source)),
    };
    let js = perasc_codegen_js::generate(&parsed.ast, &analysis, &options)
        .map_err(|d| ToolError::Diagnostics(Checked::capped(d)))?;
    let dts = perasc_codegen_js::declarations(&parsed.ast, &analysis);
    let imports = context
        .imports
        .into_iter()
        .filter_map(|m| Some((m.from.clone(), import_path(path, &m.from)?)))
        .collect();
    Ok(ManagedOutput { js, dts, imports })
}

/// Checks a source and lowers it to the IR, verified, or returns its errors.
///
/// # Errors
///
/// The errors of the first stage that has any: syntax, names and types, or code
/// generation limits (`E0900`); [`ToolError::Internal`] if the IR verifier fails.
pub fn compile_ir(source: &str) -> Result<perasc_ir::Module, ToolError> {
    compile_ir_with(source, false)
}

/// [`compile_ir`], with `+`, `-` and `*` on integers stopping the program on
/// an overflow when `overflow_checks`, as a debug build does.
///
/// # Errors
///
/// As [`compile_ir`].
pub fn compile_ir_with(
    source: &str,
    overflow_checks: bool,
) -> Result<perasc_ir::Module, ToolError> {
    let parsed = perasc_syntax::parse(source);
    if !parsed.diagnostics.is_empty() {
        return Err(ToolError::Diagnostics(Checked::capped(parsed.diagnostics)));
    }
    let analysis = perasc_check::analyze(&parsed.ast, source);
    if !analysis.diagnostics.is_empty() {
        return Err(ToolError::Diagnostics(Checked::capped(
            analysis.diagnostics,
        )));
    }
    let module = perasc_ir::lower_with(&parsed.ast, &analysis, overflow_checks)
        .map_err(|d| ToolError::Diagnostics(Checked::capped(d)))?;
    perasc_ir::verify(&module).map_err(ToolError::Internal)?;
    Ok(module)
}

/// What the direct backend produces for one module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectOutput {
    /// The WebAssembly binary.
    pub wasm: Vec<u8>,
    /// An ES module loading the binary and exporting its functions.
    pub js: String,
    /// TypeScript declarations of the exported functions.
    pub dts: String,
}

/// Compiles a source straight to WebAssembly, without any external tool, after
/// the IR optimizations unless `optimize` is false.
///
/// # Errors
///
/// See [`compile_ir`].
pub fn build_direct(
    source: &str,
    wasm_file: &str,
    options: DirectOptions,
    localizer: &Localizer,
) -> Result<DirectOutput, ToolError> {
    let mut module = compile_ir_with(source, options.overflow_checks)?;
    if options.optimize {
        perasc_ir::optimize(&mut module);
        perasc_ir::verify(&module).map_err(ToolError::Internal)?;
    }
    Ok(emit_direct_for(&module, wasm_file, options, localizer))
}

/// [`build_direct`] for a file, with the files it imports compiled into the
/// same module (decision 0022); the paths of those files, the root last.
///
/// # Errors
///
/// See [`compile_graph_ir`].
pub fn build_direct_file(
    path: &Path,
    source: &str,
    wasm_file: &str,
    options: DirectOptions,
    localizer: &Localizer,
) -> Result<(DirectOutput, Vec<std::path::PathBuf>), ToolError> {
    let (mut module, inputs) = compile_graph_ir(path, source, options.overflow_checks)?;
    if options.optimize {
        perasc_ir::optimize(&mut module);
        perasc_ir::verify(&module).map_err(ToolError::Internal)?;
    }
    Ok((
        emit_direct_for(&module, wasm_file, options, localizer),
        inputs,
    ))
}

pub use perasc_runtime::Allocator;

/// How the direct backend builds a module.
// Independent switches of a build, each a flag of the command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectOptions {
    /// Runs the IR optimizations.
    pub optimize: bool,
    /// The allocator a module that uses the heap is linked with: the debug
    /// one by default, TLSF for production.
    pub allocator: perasc_runtime::Allocator,
    /// Uses WebAssembly SIMD, the default: in loops whose turns are
    /// independent, and in the runtime's search for bytes. Without, the
    /// module loads in engines that have no SIMD (Safari before 16.4).
    pub simd: bool,
    /// Runs `parallel for` loops on threads where the page can share memory
    /// ([decision 0007]); without, they run on one thread.
    ///
    /// [decision 0007]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0007-threads-over-shared-memory.md
    pub threads: bool,
    /// `+`, `-` and `*` on integers stop the program on an overflow, as a
    /// debug build does, the default; without, they wrap, as in production
    /// ([decision 0011]).
    ///
    /// [decision 0011]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0011-what-to-build-next.md
    pub overflow_checks: bool,
    /// Links the runtime into the module's `.wasm`, so that its calls are
    /// inside one module, which the engine can inline (decision 0016), the
    /// default. Never with threads, whose runtime imports a shared memory.
    pub link: bool,
}

impl Default for DirectOptions {
    fn default() -> Self {
        Self {
            optimize: true,
            allocator: perasc_runtime::Allocator::Debug,
            simd: true,
            threads: false,
            overflow_checks: true,
            link: true,
        }
    }
}

/// The direct backend's output for a verified module, for a host that
/// instantiates the runtime beside it, as the tests' does: a module that uses
/// the heap imports `allocator`'s functions, whose messages are in the
/// language of `localizer`, instead of having them linked in.
#[must_use]
pub fn emit_direct(
    module: &perasc_ir::Module,
    wasm_file: &str,
    allocator: perasc_runtime::Allocator,
    localizer: &Localizer,
) -> DirectOutput {
    let options = DirectOptions {
        allocator,
        link: false,
        ..DirectOptions::default()
    };
    emit_direct_for(module, wasm_file, options, localizer)
}

/// [`emit_direct`], with SIMD as `options` says.
#[must_use]
pub fn emit_direct_for(
    module: &perasc_ir::Module,
    wasm_file: &str,
    options: DirectOptions,
    localizer: &Localizer,
) -> DirectOutput {
    let threads = options.threads && module.uses_heap();
    let compiled = perasc_codegen_wasm::compile_with(module, options.simd, threads);
    // SIMD is needed by the runtime of a module that uses the heap, and by
    // the loops that run several turns at once.
    let needs_simd = options.simd && (module.needs_runtime() || compiled.vectors);
    let simd = localizer.message(&perasc_diag::Message::new("runtime-simd"));
    let faults = fault_messages(localizer);
    let bounds = localizer.message(
        &perasc_diag::Message::new("runtime-index")
            .text("index", "{index}")
            .text("length", "{length}"),
    );
    let overflow = localizer.message(&perasc_diag::Message::new("runtime-integer-overflow"));
    // The runtime this module needs: what its loader calls, and the
    // functions on strings and arrays the module calls.
    let full = options.allocator.runtime(options.simd);
    // With threads, the runtime imports the memory the loader may share.
    let imported = threads
        .then(|| perasc_runtime::import_memory(full, perasc_codegen_wasm::THREAD_PAGES).ok())
        .flatten();
    let full: &[u8] = imported.as_deref().unwrap_or(full);
    let used = module.runtime_fns();
    let module_flag = perasc_runtime::memory_flag(&compiled.wasm);
    let loader = |allocator: &[u8]| {
        let flags = perasc_runtime::memory_flag(allocator).zip(module_flag);
        let runtime = perasc_codegen_wasm::Runtime {
            allocator,
            sites: &compiled.sites,
            faults: &faults,
            bounds: &bounds,
            overflow: &overflow,
            simd: needs_simd.then_some(simd.as_str()),
            linked: options.link && !threads,
            threads: flags
                .filter(|_| threads)
                .map(|(runtime_flag, module_flag)| perasc_codegen_wasm::Threads {
                    runtime_flag,
                    module_flag,
                }),
        };
        perasc_codegen_wasm::js_loader(module, wasm_file, Some(&runtime))
    };
    // The functions the loader calls itself, as `$runtime.map_put_s` to copy
    // a map in, read from a first loader made with the whole runtime.
    let draft = loader(full);
    let called = loader_calls(&draft);
    let keep: Vec<&str> = LOADER_EXPORTS
        .iter()
        .copied()
        .chain(used.iter().map(|f| f.name()))
        .chain(called.iter().map(String::as_str))
        .collect();
    let allocator = perasc_runtime::prune(full, &keep).unwrap_or_else(|_| full.to_vec());
    // Linked: one module, the runtime's functions called from inside it.
    let wasm = if options.link && !threads && module.needs_runtime() {
        perasc_runtime::link(&allocator, &compiled.wasm).unwrap_or(compiled.wasm)
    } else {
        compiled.wasm
    };
    DirectOutput {
        js: loader(&allocator),
        dts: perasc_codegen_wasm::dts(module),
        wasm,
    }
}

/// The runtime's functions a loader calls: each `$runtime.name` in it.
fn loader_calls(js: &str) -> Vec<String> {
    let mut names: Vec<String> = js
        .match_indices("$runtime.")
        .map(|(at, found)| {
            js[at + found.len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect()
        })
        .filter(|name: &String| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// What the JavaScript loader takes from the runtime, whatever the module
/// calls: its memory, the allocation and buffer functions, and the counts its
/// `__perasHeap` and the A/B benchmark read.
const LOADER_EXPORTS: [&str; 10] = [
    "memory",
    "alloc",
    "release",
    "buf_new",
    "buf_reserve",
    "buf_release",
    "buf_clone",
    "live_count",
    "check_leaks",
    "allocations",
];

/// The message of each fault of the debug allocator, kind 1 first, with the
/// placeholders the generated JavaScript fills in.
#[must_use]
pub fn fault_messages(localizer: &Localizer) -> Vec<String> {
    [
        "runtime-out-of-memory",
        "runtime-too-large",
        "runtime-overflow",
        "runtime-invalid-release",
        "runtime-double-release",
        "runtime-use-after-release",
        "runtime-leak",
    ]
    .into_iter()
    .map(|id| {
        let message = perasc_diag::Message::new(id)
            .text("alloc", "{alloc}")
            .text("release", "{release}")
            .text("count", "{count}")
            .text("size", "{size}");
        localizer.message(&message)
    })
    .collect()
}

/// Why a build failed.
#[derive(Debug)]
pub enum ToolError {
    /// The source has errors.
    Diagnostics(Checked),
    /// A file the source imports has errors, or closes a cycle of imports:
    /// shown in that file (decision 0022).
    InFile {
        /// The file, as reached from the root.
        path: std::path::PathBuf,
        /// Its text.
        source: String,
        /// Its errors.
        checked: Checked,
    },
    /// The IR verifier rejected the IR: a bug in perasc, with what it found.
    Internal(Vec<String>),
    /// A file could not be written.
    Io(io::Error),
    /// `cargo` or `wasm-bindgen` is not installed.
    Missing(&'static str),
    /// `cargo` or `wasm-bindgen` failed on generated code: a bug in perasc.
    Failed {
        /// The tool.
        tool: &'static str,
        /// What it printed.
        output: String,
    },
}

/// A crate name for a module: `peras_` and the file stem, in `[a-z0-9_]`.
#[must_use]
pub fn crate_name(stem: &str) -> String {
    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("peras_{clean}")
}

/// Compiles a source through the Rust backend: writes a crate in `crate_dir`,
/// builds it with `cargo`, then runs `wasm-bindgen` into `out_dir`.
///
/// # Errors
///
/// See [`ToolError`].
pub fn build_rust(
    source: &str,
    name: &str,
    crate_dir: &Path,
    out_dir: &Path,
) -> Result<(), ToolError> {
    // The Rust backend compiles the unoptimized IR: the differential test in CI
    // then checks the IR optimizations too.
    let module = compile_ir(source)?;
    // It has no allocator yet; lowering produces no heap IR today.
    if module.uses_heap() {
        return Err(ToolError::Internal(vec![
            "the IR uses the heap, which the Rust backend does not compile yet".to_owned(),
        ]));
    }
    // wasm-bindgen cannot export several results: only inner functions return them.
    if let Some(f) = module
        .functions
        .iter()
        .find(|f| f.exported && !f.more.is_empty())
    {
        return Err(ToolError::Internal(vec![format!(
            "the Rust backend cannot export {}, which returns a structure",
            f.name
        )]));
    }
    for (path, contents) in perasc_codegen_rs::crate_files(&module, name) {
        let file = crate_dir.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(ToolError::Io)?;
        }
        // Rewriting an identical file would make cargo rebuild for nothing.
        if fs::read_to_string(&file).ok().as_deref() != Some(contents.as_str()) {
            fs::write(&file, contents).map_err(ToolError::Io)?;
        }
    }
    run(
        "cargo",
        Command::new("cargo")
            .args([
                "build",
                "--quiet",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
            ])
            .current_dir(crate_dir),
    )?;
    let wasm = crate_dir
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{name}.wasm"));
    fs::create_dir_all(out_dir).map_err(ToolError::Io)?;
    run(
        "wasm-bindgen",
        Command::new("wasm-bindgen")
            .args(["--target", "nodejs", "--out-dir"])
            .arg(out_dir)
            .arg(wasm),
    )
}

fn run(tool: &'static str, command: &mut Command) -> Result<(), ToolError> {
    let output = command.output().map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => ToolError::Missing(tool),
        _ => ToolError::Io(e),
    })?;
    if output.status.success() {
        return Ok(());
    }
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Err(ToolError::Failed { tool, output: text })
}

/// Why a source file could not be read.
#[derive(Debug)]
pub enum ReadError {
    /// The extension is neither `.peras` nor `.apeiron`.
    NotASource,
    /// No such file.
    NotFound,
    /// The file is too large to compile; its size in bytes. Report it with
    /// [`source_too_large`] (`E0008`).
    TooLarge(u64),
    /// The file is not UTF-8 text.
    NotUtf8,
    /// Any other I/O error.
    Io(io::Error),
}

/// Reads a source file, refusing what the compiler cannot handle before reading it all.
///
/// # Errors
///
/// See [`ReadError`].
pub fn read_source(path: &Path) -> Result<String, ReadError> {
    if Mode::from_path(path).is_none() {
        return Err(ReadError::NotASource);
    }
    let not_found = |e: io::Error| match e.kind() {
        io::ErrorKind::NotFound => ReadError::NotFound,
        _ => ReadError::Io(e),
    };
    let len = fs::metadata(path).map_err(not_found)?.len();
    if len > u64::from(u32::MAX) {
        return Err(ReadError::TooLarge(len));
    }
    let bytes = fs::read(path).map_err(not_found)?;
    String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8)
}

#[cfg(test)]
mod tests {
    use super::{MAX_ERRORS_PER_FILE, ReadError, check, read_source};
    use std::path::Path;

    #[test]
    fn errors_beyond_the_limit_are_counted_not_shown() {
        let source = "#".repeat(10) + &" #".repeat(MAX_ERRORS_PER_FILE + 5);
        let checked = check(&source);
        assert_eq!(checked.diagnostics.len(), MAX_ERRORS_PER_FILE);
        assert_eq!(checked.hidden, 6);
        assert_eq!(checked.error_count(), MAX_ERRORS_PER_FILE + 6);
    }

    #[test]
    fn only_peras_sources_are_read() {
        assert!(matches!(
            read_source(Path::new("Cargo.toml")),
            Err(ReadError::NotASource)
        ));
        assert!(matches!(
            read_source(Path::new("missing.peras")),
            Err(ReadError::NotFound)
        ));
    }
}
