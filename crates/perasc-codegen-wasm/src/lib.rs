//! Direct WebAssembly code generation for owned Peras modules (`.peras`).
//!
//! Encodes the IR straight into a `.wasm` binary with `wasm-encoder`: no Rust
//! toolchain, no `cargo`, no `wasm-bindgen`. Since the checker knows every type,
//! the JavaScript loader and the TypeScript declarations are generated here too.
//!
//! Semantics match the Rust backend exactly: integers wrap, division by zero and
//! `i32::MIN / -1` trap, float-to-integer conversions saturate.

use std::fmt::Write as _;

use perasc_ir::{BinOp, Const, Expr, ExprKind, Function, MathOp, Module, Stmt, Ty, UnOp};
use wasm_encoder::{
    BlockType, CodeSection, EntityType, ExportKind, ExportSection, Function as WasmFunction,
    FunctionSection, Ieee32, Ieee64, ImportSection, Instruction as I, TypeSection, ValType,
};

/// The value type of an IR type; `None` for `void`.
const fn val_type(ty: Ty) -> Option<ValType> {
    match ty {
        // A handle is an address in linear memory.
        Ty::I32 | Ty::Bool | Ty::Owned | Ty::Borrowed => Some(ValType::I32),
        Ty::I64 => Some(ValType::I64),
        Ty::F32 => Some(ValType::F32),
        Ty::F64 => Some(ValType::F64),
        Ty::Void => None,
    }
}

/// Module that a module using the heap imports its allocator from.
pub const RUNTIME_MODULE: &str = "peras_rt";

/// A compiled module, and what its allocation sites are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    /// The WebAssembly binary.
    pub wasm: Vec<u8>,
    /// What each allocation or release site is, for messages: `sites[n - 1]` is
    /// site `n`, the number passed to the allocator; 0 means unknown.
    pub sites: Vec<String>,
}

/// Encodes a module as a WebAssembly binary.
#[must_use]
pub fn compile(module: &Module) -> Vec<u8> {
    compile_module(module).wasm
}

/// Encodes a module, and numbers its allocation sites.
///
/// A module that uses the heap imports `alloc(size, site) -> ptr` and
/// `release(ptr, site)` from [`RUNTIME_MODULE`], the debug allocator of
/// `perasc-runtime`; the imports come first, so its own functions follow them.
#[must_use]
pub fn compile_module(module: &Module) -> Compiled {
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut functions = FunctionSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let heap = module.uses_heap();
    let offset = if heap { 2 } else { 0 };
    if heap {
        types
            .ty()
            .function([ValType::I32, ValType::I32], [ValType::I32]);
        types.ty().function([ValType::I32, ValType::I32], []);
        imports.import(RUNTIME_MODULE, "alloc", EntityType::Function(0));
        imports.import(RUNTIME_MODULE, "release", EntityType::Function(1));
    }
    let mut sites = Vec::new();
    for (index, function) in module.functions.iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX) + offset;
        let params: Vec<ValType> = function.params().filter_map(|l| val_type(l.ty)).collect();
        let results: Vec<ValType> = val_type(function.ret).into_iter().collect();
        types.ty().function(params, results);
        // Its type comes after the two import types, as its index after the imports.
        functions.function(index);
        if function.exported {
            exports.export(&function.name, ExportKind::Func, index);
        }
        code.function(&FnEncoder::encode(function, offset, &mut sites));
    }
    let mut wasm = wasm_encoder::Module::new();
    wasm.section(&types);
    if heap {
        wasm.section(&imports);
    }
    wasm.section(&functions).section(&exports).section(&code);
    Compiled {
        wasm: wasm.finish(),
        sites,
    }
}

/// What an enclosing structured instruction is, to compute `br` depths.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Frame {
    /// The block a `break` leaves.
    Break,
    /// The `loop` itself.
    Top,
    /// The block a `continue` leaves, to reach the step.
    Continue,
    /// A block that `leave` exits.
    Leave,
    /// An `if`, or a short-circuit `&&` / `||`.
    Other,
}

struct FnEncoder<'a> {
    out: WasmFunction,
    frames: Vec<Frame>,
    /// Index of the module's first own function: the imports come before.
    offset: u32,
    /// The function's name, and how many sites of each kind it has so far.
    name: &'a str,
    allocs: u32,
    releases: u32,
    /// Every site of the module so far.
    sites: &'a mut Vec<String>,
    /// Scratch locals for integer `abs`, indexed after the function's own locals.
    temp_i32: u32,
    temp_i64: u32,
}

impl<'a> FnEncoder<'a> {
    fn encode(function: &'a Function, offset: u32, sites: &'a mut Vec<String>) -> WasmFunction {
        let params = usize::try_from(function.param_count).unwrap_or(0);
        let mut locals: Vec<(u32, ValType)> = function
            .locals
            .iter()
            .skip(params)
            .filter_map(|l| val_type(l.ty).map(|t| (1, t)))
            .collect();
        let count = u32::try_from(function.locals.len()).unwrap_or(u32::MAX);
        locals.push((1, ValType::I32));
        locals.push((1, ValType::I64));
        let mut encoder = Self {
            out: WasmFunction::new(locals),
            frames: Vec::new(),
            offset,
            name: &function.name,
            allocs: 0,
            releases: 0,
            sites,
            temp_i32: count,
            temp_i64: count + 1,
        };
        encoder.stmts(&function.body);
        if function.ret != Ty::Void {
            // The checker proved every path returns: this end is never reached.
            encoder.emit(&I::Unreachable);
        }
        encoder.emit(&I::End);
        encoder.out
    }

    /// A new site number, with what it is.
    fn site(&mut self, what: String) -> i32 {
        self.sites.push(what);
        i32::try_from(self.sites.len()).unwrap_or(0)
    }

    fn emit(&mut self, instruction: &I<'_>) {
        self.out.instruction(instruction);
    }

    /// The `br` depth of the innermost frame of this kind.
    fn depth(&self, target: Frame) -> u32 {
        let position = self
            .frames
            .iter()
            .rev()
            .position(|f| *f == target)
            .unwrap_or(0);
        u32::try_from(position).unwrap_or(0)
    }

    /// The `br` depth of the `n`-th enclosing [`Frame::Leave`], innermost first.
    fn leave_depth(&self, n: u32) -> u32 {
        let position = self
            .frames
            .iter()
            .rev()
            .enumerate()
            .filter(|(_, f)| **f == Frame::Leave)
            .nth(usize::try_from(n).unwrap_or(usize::MAX))
            .map_or(0, |(i, _)| i);
        u32::try_from(position).unwrap_or(0)
    }

    fn stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Set(local, value) => {
                self.expr(value);
                self.emit(&I::LocalSet(local.0));
            }
            Stmt::Expr(e) => {
                self.expr(e);
                if e.ty != Ty::Void {
                    self.emit(&I::Drop);
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond);
                self.emit(&I::If(BlockType::Empty));
                self.frames.push(Frame::Other);
                self.stmts(then);
                if !otherwise.is_empty() {
                    self.emit(&I::Else);
                    self.stmts(otherwise);
                }
                self.frames.pop();
                self.emit(&I::End);
            }
            Stmt::Loop { cond, body, step } => self.loop_(cond.as_ref(), body, step),
            Stmt::Block(body) => {
                self.emit(&I::Block(BlockType::Empty));
                self.frames.push(Frame::Leave);
                self.stmts(body);
                self.frames.pop();
                self.emit(&I::End);
            }
            Stmt::Leave(n) => {
                let depth = self.leave_depth(*n);
                self.emit(&I::Br(depth));
            }
            Stmt::Release(local) => {
                self.emit(&I::LocalGet(local.0));
                self.releases += 1;
                let number = self.site(format!("{} (release {})", self.name, self.releases));
                self.emit(&I::I32Const(number));
                self.emit(&I::Call(1));
            }
            Stmt::Break => {
                let depth = self.depth(Frame::Break);
                self.emit(&I::Br(depth));
            }
            Stmt::Continue => {
                let depth = self.depth(Frame::Continue);
                self.emit(&I::Br(depth));
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    self.expr(value);
                }
                self.emit(&I::Return);
            }
        }
    }

    /// A rotated loop, as LLVM writes them: the condition is checked once before
    /// the loop, then at the bottom, so that each turn takes one branch.
    ///
    /// `block $break { br_if $break !cond; loop $top { block $continue { body } step;
    /// br_if $top cond } }`; without a condition, the bottom is `br $top`.
    fn loop_(&mut self, cond: Option<&Expr>, body: &[Stmt], step: &[Stmt]) {
        self.emit(&I::Block(BlockType::Empty));
        self.frames.push(Frame::Break);
        if let Some(cond) = cond {
            self.condition(cond, true);
            let depth = self.depth(Frame::Break);
            self.emit(&I::BrIf(depth));
        }
        self.emit(&I::Loop(BlockType::Empty));
        self.frames.push(Frame::Top);
        self.emit(&I::Block(BlockType::Empty));
        self.frames.push(Frame::Continue);
        self.stmts(body);
        self.frames.pop();
        self.emit(&I::End);
        self.stmts(step);
        let depth = self.depth(Frame::Top);
        if let Some(cond) = cond {
            self.condition(cond, false);
            self.emit(&I::BrIf(depth));
        } else {
            self.emit(&I::Br(depth));
        }
        self.frames.pop();
        self.emit(&I::End);
        self.frames.pop();
        self.emit(&I::End);
    }

    /// A condition for `br_if`, negated if asked; `!x` costs no instruction.
    fn condition(&mut self, cond: &Expr, negate: bool) {
        if let ExprKind::Unary(UnOp::Not, inner) = &cond.kind {
            self.condition(inner, !negate);
            return;
        }
        self.expr(cond);
        if negate {
            self.emit(&I::I32Eqz);
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Const(c) => self.constant(*c, e.ty),
            // A handle is an address: moving or lending it copies the number. The
            // release verifier proved a moved-from local is not used again.
            ExprKind::Local(local) | ExprKind::Move(local) | ExprKind::Borrow(local) => {
                self.emit(&I::LocalGet(local.0));
            }
            ExprKind::Alloc(size) => {
                self.expr(size);
                self.allocs += 1;
                let number = self.site(format!("{} (alloc {})", self.name, self.allocs));
                self.emit(&I::I32Const(number));
                self.emit(&I::Call(0));
            }
            ExprKind::Unary(op, operand) => self.unary(*op, operand, e.ty),
            ExprKind::Binary(op, lhs, rhs) => self.binary(*op, lhs, rhs),
            ExprKind::Call(id, args) => {
                for arg in args {
                    self.expr(arg);
                }
                self.emit(&I::Call(id.0 + self.offset));
            }
            ExprKind::Convert(operand) => {
                self.expr(operand);
                if let Some(instruction) = conversion(operand.ty, e.ty) {
                    self.emit(&instruction);
                }
            }
            ExprKind::Math(op, operand) => self.math(*op, operand),
        }
    }

    fn constant(&mut self, c: Const, ty: Ty) {
        let instruction = match (c, ty) {
            (Const::Int(v), Ty::I64) => I::I64Const(v),
            (Const::Int(v), _) => I::I32Const(i32::try_from(v).unwrap_or(i32::MIN)),
            (Const::Float(v), Ty::F32) => I::F32Const(Ieee32::from(narrow(v))),
            (Const::Float(v), _) => I::F64Const(Ieee64::from(v)),
            (Const::Bool(v), _) => I::I32Const(i32::from(v)),
        };
        self.emit(&instruction);
    }

    fn unary(&mut self, op: UnOp, operand: &Expr, ty: Ty) {
        match (op, ty) {
            (UnOp::Neg, Ty::I32) => {
                self.emit(&I::I32Const(0));
                self.expr(operand);
                self.emit(&I::I32Sub);
            }
            (UnOp::Neg, Ty::I64) => {
                self.emit(&I::I64Const(0));
                self.expr(operand);
                self.emit(&I::I64Sub);
            }
            (UnOp::Neg, Ty::F32) => {
                self.expr(operand);
                self.emit(&I::F32Neg);
            }
            (UnOp::Neg, _) => {
                self.expr(operand);
                self.emit(&I::F64Neg);
            }
            (UnOp::Not, _) => {
                self.expr(operand);
                self.emit(&I::I32Eqz);
            }
            (UnOp::BitNot, Ty::I64) => {
                self.expr(operand);
                self.emit(&I::I64Const(-1));
                self.emit(&I::I64Xor);
            }
            (UnOp::BitNot, _) => {
                self.expr(operand);
                self.emit(&I::I32Const(-1));
                self.emit(&I::I32Xor);
            }
        }
    }

    fn binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr) {
        // Short-circuit: the right operand runs only when needed.
        if matches!(op, BinOp::And | BinOp::Or) {
            self.expr(lhs);
            self.emit(&I::If(BlockType::Result(ValType::I32)));
            self.frames.push(Frame::Other);
            if op == BinOp::And {
                self.expr(rhs);
                self.emit(&I::Else);
                self.emit(&I::I32Const(0));
            } else {
                self.emit(&I::I32Const(1));
                self.emit(&I::Else);
                self.expr(rhs);
            }
            self.frames.pop();
            self.emit(&I::End);
            return;
        }
        self.expr(lhs);
        self.expr(rhs);
        let instruction = match lhs.ty {
            Ty::I32 | Ty::Bool | Ty::Owned | Ty::Borrowed => int32(op),
            Ty::I64 => int64(op),
            Ty::F32 => float32(op),
            Ty::F64 | Ty::Void => float64(op),
        };
        self.emit(&instruction);
    }

    fn math(&mut self, op: MathOp, operand: &Expr) {
        self.expr(operand);
        let instruction = match (op, operand.ty) {
            (MathOp::Abs, Ty::I32) => return self.int_abs(false),
            (MathOp::Abs, Ty::I64) => return self.int_abs(true),
            (MathOp::Abs, Ty::F32) => I::F32Abs,
            (MathOp::Sqrt, Ty::F32) => I::F32Sqrt,
            (MathOp::Floor, Ty::F32) => I::F32Floor,
            (MathOp::Ceil, Ty::F32) => I::F32Ceil,
            (MathOp::Abs, _) => I::F64Abs,
            (MathOp::Sqrt, _) => I::F64Sqrt,
            (MathOp::Floor, _) => I::F64Floor,
            (MathOp::Ceil, _) => I::F64Ceil,
        };
        self.emit(&instruction);
    }

    /// `x < 0 ? 0 - x : x`, wrapping for the smallest value, with a scratch local.
    fn int_abs(&mut self, wide: bool) {
        let (temp, zero, sub, lt) = if wide {
            (self.temp_i64, I::I64Const(0), I::I64Sub, I::I64LtS)
        } else {
            (self.temp_i32, I::I32Const(0), I::I32Sub, I::I32LtS)
        };
        self.emit(&I::LocalSet(temp));
        self.emit(&zero);
        self.emit(&I::LocalGet(temp));
        self.emit(&sub);
        self.emit(&I::LocalGet(temp));
        self.emit(&I::LocalGet(temp));
        self.emit(&zero);
        self.emit(&lt);
        self.emit(&I::Select);
    }
}

/// An `f64` constant as `f32`, rounded to nearest as in Rust's `as`.
#[allow(clippy::cast_possible_truncation)]
const fn narrow(v: f64) -> f32 {
    v as f32
}

const fn int32(op: BinOp) -> I<'static> {
    match op {
        BinOp::Add => I::I32Add,
        BinOp::Sub => I::I32Sub,
        BinOp::Mul => I::I32Mul,
        BinOp::Div => I::I32DivS,
        BinOp::Rem => I::I32RemS,
        BinOp::BitAnd | BinOp::And => I::I32And,
        BinOp::BitOr | BinOp::Or => I::I32Or,
        BinOp::BitXor => I::I32Xor,
        BinOp::Shl => I::I32Shl,
        BinOp::Shr => I::I32ShrS,
        BinOp::Eq => I::I32Eq,
        BinOp::Ne => I::I32Ne,
        BinOp::Lt => I::I32LtS,
        BinOp::Le => I::I32LeS,
        BinOp::Gt => I::I32GtS,
        BinOp::Ge => I::I32GeS,
    }
}

const fn int64(op: BinOp) -> I<'static> {
    match op {
        BinOp::Add => I::I64Add,
        BinOp::Sub => I::I64Sub,
        BinOp::Mul => I::I64Mul,
        BinOp::Div => I::I64DivS,
        BinOp::Rem => I::I64RemS,
        BinOp::BitAnd | BinOp::And => I::I64And,
        BinOp::BitOr | BinOp::Or => I::I64Or,
        BinOp::BitXor => I::I64Xor,
        BinOp::Shl => I::I64Shl,
        BinOp::Shr => I::I64ShrS,
        BinOp::Eq => I::I64Eq,
        BinOp::Ne => I::I64Ne,
        BinOp::Lt => I::I64LtS,
        BinOp::Le => I::I64LeS,
        BinOp::Gt => I::I64GtS,
        BinOp::Ge => I::I64GeS,
    }
}

const fn float32(op: BinOp) -> I<'static> {
    match op {
        BinOp::Sub => I::F32Sub,
        BinOp::Mul => I::F32Mul,
        BinOp::Div => I::F32Div,
        BinOp::Eq => I::F32Eq,
        BinOp::Ne => I::F32Ne,
        BinOp::Lt => I::F32Lt,
        BinOp::Le => I::F32Le,
        BinOp::Gt => I::F32Gt,
        BinOp::Ge => I::F32Ge,
        // `+`; other operators never reach floats (the checker refuses them).
        _ => I::F32Add,
    }
}

const fn float64(op: BinOp) -> I<'static> {
    match op {
        BinOp::Sub => I::F64Sub,
        BinOp::Mul => I::F64Mul,
        BinOp::Div => I::F64Div,
        BinOp::Eq => I::F64Eq,
        BinOp::Ne => I::F64Ne,
        BinOp::Lt => I::F64Lt,
        BinOp::Le => I::F64Le,
        BinOp::Gt => I::F64Gt,
        BinOp::Ge => I::F64Ge,
        _ => I::F64Add,
    }
}

/// The instruction converting `from` to `to`; float-to-integer saturates.
const fn conversion(from: Ty, to: Ty) -> Option<I<'static>> {
    Some(match (from, to) {
        (Ty::I32, Ty::I64) => I::I64ExtendI32S,
        (Ty::I64, Ty::I32) => I::I32WrapI64,
        (Ty::I32, Ty::F32) => I::F32ConvertI32S,
        (Ty::I32, Ty::F64) => I::F64ConvertI32S,
        (Ty::I64, Ty::F32) => I::F32ConvertI64S,
        (Ty::I64, Ty::F64) => I::F64ConvertI64S,
        (Ty::F32, Ty::I32) => I::I32TruncSatF32S,
        (Ty::F64, Ty::I32) => I::I32TruncSatF64S,
        (Ty::F32, Ty::I64) => I::I64TruncSatF32S,
        (Ty::F64, Ty::I64) => I::I64TruncSatF64S,
        (Ty::F32, Ty::F64) => I::F64PromoteF32,
        (Ty::F64, Ty::F32) => I::F32DemoteF64,
        _ => return None,
    })
}

/// The first line of the loader's imports: what the module is instantiated
/// with. `vite-plugin-peras` keeps the imports and the exports, and replaces the
/// part between [`LOAD_MARKER`] and [`EXPORTS_MARKER`]: do not change them.
pub const IMPORTS_MARKER: &str = "// perasc:imports";
/// The first line of the part that loads the module into `instance`.
pub const LOAD_MARKER: &str = "// perasc:load (needs `imports`)";
/// The line after which the loader only needs `instance`.
pub const EXPORTS_MARKER: &str = "// perasc:exports (needs `instance`)";

/// What a loader needs to link the debug allocator into a module using the heap.
#[derive(Debug, Clone, Copy)]
pub struct Runtime<'a> {
    /// The allocator module: `perasc_runtime::DEBUG_ALLOCATOR`.
    pub allocator: &'a [u8],
    /// The sites of the module, from [`compile_module`].
    pub sites: &'a [String],
    /// The message of each fault kind, kind 1 first, in the user's language,
    /// with `{alloc}`, `{release}`, `{count}` and `{size}` to fill in.
    pub faults: &'a [String],
}

/// A JavaScript ES module that loads the `.wasm` file next to it and exports its
/// functions; `bool` results become JavaScript booleans. A module that uses the
/// heap also gets the debug allocator, which throws a `PerasMemoryError` on a
/// memory bug, and an `__perasHeap` export to count live blocks and check leaks.
#[must_use]
pub fn js_loader(module: &Module, wasm_file: &str, runtime: Option<&Runtime<'_>>) -> String {
    let mut js = format!(
        "// Generated by perasc. Loads {wasm_file} and exports its functions.\n{IMPORTS_MARKER}\n"
    );
    let heap = module.uses_heap();
    match runtime.filter(|_| heap) {
        Some(runtime) => js.push_str(&runtime_imports(runtime)),
        None if heap => js.push_str(
            "throw new Error(\"perasc: this module uses the heap but was built without its runtime\");\n",
        ),
        None => js.push_str("const imports = {};\n"),
    }
    let _ = write!(
        js,
        "{LOAD_MARKER}\n\
         const url = new URL(\"./{wasm_file}\", import.meta.url);\n\
         const bytes = url.protocol === \"file:\"\n  \
         ? await (await import(\"node:fs/promises\")).readFile(url)\n  \
         : await (await fetch(url)).arrayBuffer();\n\
         const {{ instance }} = await WebAssembly.instantiate(bytes, imports);\n\
         {EXPORTS_MARKER}\n\
         const wasm = instance.exports;\n"
    );
    for f in module.functions.iter().filter(|f| f.exported) {
        if f.ret == Ty::Bool {
            let _ = writeln!(
                js,
                "export const {0} = (...args) => wasm.{0}(...args) !== 0;",
                f.name
            );
        } else {
            let _ = writeln!(js, "export const {0} = wasm.{0};", f.name);
        }
    }
    if heap {
        js.push_str(
            "export const __perasHeap = {\n  \
             live: () => runtime.live_count(),\n  \
             checkLeaks: () => runtime.check_leaks(),\n};\n",
        );
    }
    js
}

/// The allocator, instantiated first, and the imports it gives the module.
fn runtime_imports(runtime: &Runtime<'_>) -> String {
    let faults: Vec<String> = runtime.faults.iter().map(|m| js_string(m)).collect();
    let sites: Vec<String> = std::iter::once("?".to_owned())
        .chain(runtime.sites.iter().cloned())
        .map(|s| js_string(&s))
        .collect();
    format!(
        "// The debug allocator (decision 0001): a memory bug throws a PerasMemoryError.\n\
         const faults = [{faults}];\n\
         const sites = [{sites}];\n\
         const site = (n) => sites[n] ?? \"?\";\n\
         const fault = (kind, ptr, allocSite, releaseSite) => {{\n  \
         const text = (faults[kind - 1] ?? `memory fault ${{kind}}`)\n    \
         .replaceAll(\"{{alloc}}\", site(allocSite))\n    \
         .replaceAll(\"{{release}}\", site(releaseSite))\n    \
         .replaceAll(\"{{count}}\", String(releaseSite))\n    \
         .replaceAll(\"{{size}}\", String(ptr >>> 0));\n  \
         const error = new Error(text);\n  \
         error.name = \"PerasMemoryError\";\n  \
         Object.assign(error, {{ kind, allocSite: site(allocSite), releaseSite: site(releaseSite) }});\n  \
         throw error;\n\
         }};\n\
         const allocator = Uint8Array.from(atob(\"{allocator}\"), (c) => c.charCodeAt(0));\n\
         const runtime = (await WebAssembly.instantiate(allocator, {{ peras: {{ fault }} }})).instance.exports;\n\
         const imports = {{ {module}: {{ alloc: runtime.alloc, release: runtime.release }} }};\n",
        faults = faults.join(", "),
        sites = sites.join(", "),
        allocator = base64(runtime.allocator),
        module = RUNTIME_MODULE,
    )
}

/// A JavaScript string literal.
fn js_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Standard base64, for `atob`.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                let index = usize::try_from((n >> (18 - 6 * i)) & 63).unwrap_or(0);
                out.push(char::from(ALPHABET[index]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// TypeScript declarations of the exported functions: `i64` is a `bigint`.
#[must_use]
pub fn dts(module: &Module) -> String {
    let ts = |ty: Ty| match ty {
        Ty::I64 => "bigint",
        Ty::Bool => "boolean",
        Ty::Void => "void",
        _ => "number",
    };
    let mut out = String::from("// Generated by perasc.\n");
    if module.uses_heap() {
        out.push_str(
            "/** Blocks still alive in the debug allocator, and a check that throws on a leak. */\n\
             export declare const __perasHeap: { live(): number; checkLeaks(): void };\n",
        );
    }
    for f in module.functions.iter().filter(|f| f.exported) {
        let params: Vec<String> = f
            .params()
            .map(|p| format!("{}: {}", p.name, ts(p.ty)))
            .collect();
        let _ = writeln!(
            out,
            "export declare function {}({}): {};",
            f.name,
            params.join(", "),
            ts(f.ret)
        );
    }
    out
}

#[cfg(test)]
mod tests;
