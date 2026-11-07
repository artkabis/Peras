//! Direct WebAssembly code generation for owned Peras modules (`.peras`).
//!
//! Encodes the IR straight into a `.wasm` binary with `wasm-encoder`: no Rust
//! toolchain, no `cargo`, no `wasm-bindgen`. Since the checker knows every type,
//! the JavaScript loader and the TypeScript declarations are generated here too.
//!
//! Semantics match the Rust backend exactly: integers wrap, division by zero and
//! `i32::MIN / -1` trap, float-to-integer conversions saturate.

use std::fmt::Write as _;

use perasc_ir::{
    BinOp, BufOp, BufStmt, Const, Elem, Expr, ExprKind, Function, JsShape, MathOp, Module, Pass,
    Shape, Stmt, Ty, UnOp,
};
use wasm_encoder::{
    BlockType, CodeSection, DataCountSection, DataSection, EntityType, ExportKind, ExportSection,
    Function as WasmFunction, FunctionSection, Ieee32, Ieee64, ImportSection, Instruction as I,
    MemArg, MemArgTarget, MemoryType, TypeSection, ValType,
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

/// The functions a module using the heap imports from [`RUNTIME_MODULE`], in
/// order: their indices come before the module's own functions.
const ALLOC: u32 = 0;
const RELEASE: u32 = 1;
const BUF_NEW: u32 = 2;
const BUF_RESERVE: u32 = 3;
const BUF_RELEASE: u32 = 4;
const BOUNDS: u32 = 5;
const BUF_CLONE: u32 = 6;
const IMPORTED: u32 = 7;
/// Added to an element size: the elements are buffers the buffer owns
/// (`perasc_runtime::DEEP`).
const DEEP: i32 = 0x100;
/// The function types of the imports, which come first in the type section.
const IMPORT_TYPES: u32 = 4;

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
    let offset = if heap { IMPORTED } else { 0 };
    let type_offset = if heap { IMPORT_TYPES } else { 0 };
    if heap {
        use ValType::I32 as T;
        types.ty().function([T, T], [T]);
        types.ty().function([T, T], []);
        types.ty().function([T, T, T], [T]);
        types.ty().function([T, T, T, T], []);
        for (name, ty) in [
            ("alloc", 0),
            ("release", 1),
            ("buf_new", 2),
            ("buf_reserve", 3),
            ("buf_release", 1),
            ("bounds", 1),
            ("buf_clone", 0),
        ] {
            imports.import(RUNTIME_MODULE, name, EntityType::Function(ty));
        }
        // The allocator's memory, which buffers live in.
        imports.import(
            RUNTIME_MODULE,
            "memory",
            EntityType::Memory(MemoryType {
                minimum: 1,
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            }),
        );
    }
    let mut sites = Vec::new();
    for (position, function) in module.functions.iter().enumerate() {
        let position = u32::try_from(position).unwrap_or(u32::MAX);
        let index = position + offset;
        let params: Vec<ValType> = function.params().filter_map(|l| val_type(l.ty)).collect();
        let results: Vec<ValType> = function
            .results()
            .into_iter()
            .filter_map(val_type)
            .collect();
        types.ty().function(params, results);
        // Its type comes after the import types, as its index after the imports.
        functions.function(position + type_offset);
        if function.exported {
            exports.export(&function.name, ExportKind::Func, index);
        }
        code.function(&FnEncoder::encode(
            function,
            offset,
            &mut sites,
            &module.data,
        ));
    }
    let mut wasm = wasm_encoder::Module::new();
    wasm.section(&types);
    if heap {
        wasm.section(&imports);
    }
    wasm.section(&functions).section(&exports);
    // String literals: passive segments, copied into a new buffer when used.
    let segments = u32::try_from(module.data.len()).unwrap_or(u32::MAX);
    if segments > 0 {
        wasm.section(&DataCountSection { count: segments });
    }
    wasm.section(&code);
    if segments > 0 {
        let mut data = DataSection::new();
        for bytes in &module.data {
            data.passive(bytes.iter().copied());
        }
        wasm.section(&data);
    }
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
    /// Scratch locals, indexed after the function's own locals: for integer
    /// `abs`, a checked index, and the value a `buf.push` stores.
    temp_i32: u32,
    temp_i64: u32,
    temp_len: u32,
    temp_f32: u32,
    temp_f64: u32,
    /// The buffer a string literal is copied into.
    temp_buffer: u32,
    /// The bytes of each literal, for their lengths.
    data: &'a [Vec<u8>],
}

impl<'a> FnEncoder<'a> {
    fn encode(
        function: &'a Function,
        offset: u32,
        sites: &'a mut Vec<String>,
        data: &'a [Vec<u8>],
    ) -> WasmFunction {
        let params = usize::try_from(function.param_count).unwrap_or(0);
        let mut locals: Vec<(u32, ValType)> = function
            .locals
            .iter()
            .skip(params)
            .filter_map(|l| val_type(l.ty).map(|t| (1, t)))
            .collect();
        let count = u32::try_from(function.locals.len()).unwrap_or(u32::MAX);
        for ty in [
            ValType::I32,
            ValType::I64,
            ValType::I32,
            ValType::F32,
            ValType::F64,
            ValType::I32,
        ] {
            locals.push((1, ty));
        }
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
            temp_len: count + 2,
            temp_f32: count + 3,
            temp_f64: count + 4,
            temp_buffer: count + 5,
            data,
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
            Stmt::ReturnMany(values) => {
                for value in values {
                    self.expr(value);
                }
                self.emit(&I::Return);
            }
            Stmt::CallMany {
                targets,
                callee,
                args,
            } => {
                for arg in args {
                    self.expr(arg);
                }
                self.emit(&I::Call(callee.0 + self.offset));
                // The last result is on top of the stack.
                for target in targets.iter().rev() {
                    self.emit(&I::LocalSet(target.0));
                }
            }
            Stmt::Release(local) => {
                self.emit(&I::LocalGet(local.0));
                self.releases += 1;
                let number = self.site(format!("{} (release {})", self.name, self.releases));
                self.emit(&I::I32Const(number));
                self.emit(&I::Call(RELEASE));
            }
            Stmt::Buf(op, args) => self.buf_stmt(*op, args),
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
                let number = self.alloc_site();
                self.emit(&I::I32Const(number));
                self.emit(&I::Call(ALLOC));
            }
            ExprKind::Buf(op, args) => self.buf_op(*op, args),
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

    /// A new allocation site of this function.
    fn alloc_site(&mut self) -> i32 {
        self.allocs += 1;
        self.site(format!("{} (alloc {})", self.name, self.allocs))
    }

    /// The address of element `index` of a buffer: its elements, plus
    /// `index * size`. `buffer` is a local: evaluating it twice costs nothing.
    fn element_address(&mut self, buffer: &Expr, index: &Expr, elem: Elem) {
        self.expr(buffer);
        self.emit(&I::I32Load(mem(8, 4)));
        self.expr(index);
        self.scale(elem);
        self.emit(&I::I32Add);
    }

    /// Multiplies the `i32` on the stack by the size of an element.
    fn scale(&mut self, elem: Elem) {
        let shift = elem.size().trailing_zeros();
        if shift > 0 {
            self.emit(&I::I32Const(i32::try_from(shift).unwrap_or(0)));
            self.emit(&I::I32Shl);
        }
    }

    fn buf_op(&mut self, op: BufOp, args: &[Expr]) {
        match (op, args) {
            (BufOp::New(elem), [cap]) => {
                self.expr(cap);
                let deep = if elem == Elem::Handle { DEEP } else { 0 };
                self.emit(&I::I32Const(i32::try_from(elem.size()).unwrap_or(1) | deep));
                let site = self.alloc_site();
                self.emit(&I::I32Const(site));
                self.emit(&I::Call(BUF_NEW));
            }
            (BufOp::Len, [buffer]) => {
                self.expr(buffer);
                self.emit(&I::I32Load(mem(0, 4)));
            }
            (BufOp::Get(elem), [buffer, index]) => {
                self.element_address(buffer, index, elem);
                self.emit(&load(elem));
            }
            (BufOp::Take, [buffer, index]) => {
                self.element_address(buffer, index, Elem::Handle);
                self.emit(&I::I32Load(mem(0, 4)));
            }
            (BufOp::Clone, [buffer]) => {
                self.expr(buffer);
                let site = self.alloc_site();
                self.emit(&I::I32Const(site));
                self.emit(&I::Call(BUF_CLONE));
            }
            (BufOp::Literal(segment), []) => {
                let len = self
                    .data
                    .get(usize::try_from(segment).unwrap_or(usize::MAX))
                    .map_or(0, Vec::len);
                let len = i32::try_from(len).unwrap_or(i32::MAX);
                // h = buf_new(len, 1); memory.init into its bytes; length = len
                self.emit(&I::I32Const(len));
                self.emit(&I::I32Const(1));
                let site = self.alloc_site();
                self.emit(&I::I32Const(site));
                self.emit(&I::Call(BUF_NEW));
                self.emit(&I::LocalTee(self.temp_buffer));
                self.emit(&I::I32Load(mem(8, 4)));
                self.emit(&I::I32Const(0));
                self.emit(&I::I32Const(len));
                self.emit(&I::MemoryInit {
                    mem: 0,
                    data_index: segment,
                });
                self.emit(&I::LocalGet(self.temp_buffer));
                self.emit(&I::I32Const(len));
                self.emit(&I::I32Store(mem(0, 4)));
                self.emit(&I::LocalGet(self.temp_buffer));
            }
            // The verifier refuses any other shape.
            _ => self.emit(&I::Unreachable),
        }
    }

    /// Stores a buffer into an element of an array of buffers; the buffer it
    /// replaces is released.
    fn replace_element(&mut self, buffer: &Expr, index: &Expr, value: &Expr) {
        // The new buffer first; then the one it replaces is released.
        self.expr(value);
        self.emit(&I::LocalSet(self.temp_buffer));
        self.element_address(buffer, index, Elem::Handle);
        self.emit(&I::LocalTee(self.temp_len));
        self.emit(&I::I32Load(mem(0, 4)));
        self.releases += 1;
        let site = self.site(format!("{} (release {})", self.name, self.releases));
        self.emit(&I::I32Const(site));
        self.emit(&I::Call(BUF_RELEASE));
        self.emit(&I::LocalGet(self.temp_len));
        self.emit(&I::LocalGet(self.temp_buffer));
        self.emit(&I::I32Store(mem(0, 4)));
    }

    /// Adds the elements of `other` at the end of `buffer`.
    fn append(&mut self, buffer: &Expr, other: &Expr, elem: Elem) {
        let width = i32::try_from(elem.size()).unwrap_or(1);
        let other_len = Expr {
            kind: ExprKind::Buf(BufOp::Len, vec![other.clone()]),
            ty: Ty::I32,
        };
        let len = Expr {
            kind: ExprKind::Buf(BufOp::Len, vec![buffer.clone()]),
            ty: Ty::I32,
        };
        // Room first: it may move the elements, of `other` too when
        // both are the same buffer.
        self.expr(buffer);
        self.expr(&other_len);
        self.emit(&I::I32Const(width));
        let site = self.alloc_site();
        self.emit(&I::I32Const(site));
        self.emit(&I::Call(BUF_RESERVE));
        self.element_address(buffer, &len, elem);
        self.expr(other);
        self.emit(&I::I32Load(mem(8, 4)));
        self.expr(&other_len);
        self.scale(elem);
        self.emit(&I::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });
        self.expr(buffer);
        self.expr(&len);
        self.expr(&other_len);
        self.emit(&I::I32Add);
        self.emit(&I::I32Store(mem(0, 4)));
    }

    fn buf_stmt(&mut self, op: BufStmt, args: &[Expr]) {
        match (op, args) {
            (BufStmt::Set(Elem::Handle), [buffer, index, value]) => {
                self.replace_element(buffer, index, value);
            }
            (BufStmt::Set(elem), [buffer, index, value]) => {
                self.element_address(buffer, index, elem);
                self.expr(value);
                self.emit(&store(elem));
            }
            (BufStmt::Push(elem), [buffer, value]) => {
                // The value first, as written; then room, then the store.
                let temp = match elem.ty() {
                    Ty::I64 => self.temp_i64,
                    Ty::F32 => self.temp_f32,
                    Ty::F64 => self.temp_f64,
                    _ => self.temp_i32,
                };
                self.expr(value);
                self.emit(&I::LocalSet(temp));
                self.expr(buffer);
                self.emit(&I::I32Const(1));
                self.emit(&I::I32Const(i32::try_from(elem.size()).unwrap_or(1)));
                let site = self.alloc_site();
                self.emit(&I::I32Const(site));
                self.emit(&I::Call(BUF_RESERVE));
                let len = Expr {
                    kind: ExprKind::Buf(BufOp::Len, vec![buffer.clone()]),
                    ty: Ty::I32,
                };
                self.element_address(buffer, &len, elem);
                self.emit(&I::LocalGet(temp));
                self.emit(&store(elem));
                self.expr(buffer);
                self.expr(&len);
                self.emit(&I::I32Const(1));
                self.emit(&I::I32Add);
                self.emit(&I::I32Store(mem(0, 4)));
            }
            (BufStmt::Append(elem), [buffer, other]) => self.append(buffer, other, elem),
            (BufStmt::Truncate, [buffer, len]) => {
                self.expr(buffer);
                self.expr(len);
                self.emit(&I::I32Store(mem(0, 4)));
            }
            (BufStmt::CheckIndex, [index, len]) => {
                // Unsigned: a negative index is out of bounds too.
                self.expr(index);
                self.emit(&I::LocalTee(self.temp_i32));
                self.expr(len);
                self.emit(&I::LocalTee(self.temp_len));
                self.emit(&I::I32GeU);
                self.emit(&I::If(BlockType::Empty));
                self.frames.push(Frame::Other);
                self.emit(&I::LocalGet(self.temp_i32));
                self.emit(&I::LocalGet(self.temp_len));
                self.emit(&I::Call(BOUNDS));
                self.emit(&I::Unreachable);
                self.frames.pop();
                self.emit(&I::End);
            }
            (BufStmt::Release, [buffer]) => {
                self.expr(buffer);
                self.releases += 1;
                let number = self.site(format!("{} (release {})", self.name, self.releases));
                self.emit(&I::I32Const(number));
                self.emit(&I::Call(BUF_RELEASE));
            }
            _ => self.emit(&I::Unreachable),
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

/// A memory access at `offset`, aligned to `size` bytes.
const fn mem(offset: u64, size: u32) -> MemArg {
    MemArg {
        offset,
        align: size.trailing_zeros(),
        target: MemArgTarget::Memory(0),
        ordering: None,
    }
}

const fn load(elem: Elem) -> I<'static> {
    match elem {
        Elem::U8 => I::I32Load8U(mem(0, 1)),
        Elem::I32 | Elem::Bool | Elem::Handle => I::I32Load(mem(0, 4)),
        Elem::I64 => I::I64Load(mem(0, 8)),
        Elem::F32 => I::F32Load(mem(0, 4)),
        Elem::F64 => I::F64Load(mem(0, 8)),
    }
}

const fn store(elem: Elem) -> I<'static> {
    match elem {
        Elem::U8 => I::I32Store8(mem(0, 1)),
        Elem::I32 | Elem::Bool | Elem::Handle => I::I32Store(mem(0, 4)),
        Elem::I64 => I::I64Store(mem(0, 8)),
        Elem::F32 => I::F32Store(mem(0, 4)),
        Elem::F64 => I::F64Store(mem(0, 8)),
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
/// The first line of the part that loads the module into `$instance`.
/// Every name the loader declares starts with `$`, which a Peras name cannot:
/// an exported function or a parameter never hides one.
pub const LOAD_MARKER: &str = "// perasc:load (needs `$imports`)";
/// The line after which the loader only needs `$instance`.
pub const EXPORTS_MARKER: &str = "// perasc:exports (needs `$instance`)";

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
    /// The message of an index out of bounds, with `{index}` and `{length}`.
    pub bounds: &'a str,
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
        None => js.push_str("const $imports = {};\n"),
    }
    let _ = write!(
        js,
        "{LOAD_MARKER}\n\
         const $url = new URL(\"./{wasm_file}\", import.meta.url);\n\
         const $bytes = $url.protocol === \"file:\"\n  \
         ? await (await import(\"node:fs/promises\")).readFile($url)\n  \
         : await (await fetch($url)).arrayBuffer();\n\
         const {{ instance: $instance }} = await WebAssembly.instantiate($bytes, $imports);\n\
         {EXPORTS_MARKER}\n\
         const $wasm = $instance.exports;\n"
    );
    if module
        .functions
        .iter()
        .any(|f| f.exported && f.shape.is_some())
    {
        js.push_str(VALUE_HELPERS);
    }
    for f in module.functions.iter().filter(|f| f.exported) {
        if let Some(shape) = &f.shape {
            js.push_str(&object_wrapper(&f.name, shape));
        } else if f.ret == Ty::Bool {
            let _ = writeln!(
                js,
                "export const {0} = (...args) => $wasm.{0}(...args) !== 0;",
                f.name
            );
        } else {
            let _ = writeln!(js, "export const {0} = $wasm.{0};", f.name);
        }
    }
    if heap {
        js.push_str(
            "export const __perasHeap = {\n  \
             live: () => $runtime.live_count(),\n  \
             checkLeaks: () => $runtime.check_leaks(),\n};\n",
        );
    }
    js
}

/// An exported function taking or returning structures, as JavaScript objects:
/// a parameter object is passed field by field, and the results, which
/// WebAssembly returns as an array when there are several, become an object.
fn object_wrapper(name: &str, shape: &JsShape) -> String {
    let params: Vec<&str> = shape.params.iter().map(|(n, _)| n.as_str()).collect();
    let mut marshal = Marshal::default();
    for (param, value) in &shape.params {
        flatten(param, value, false, &mut marshal);
    }
    let call = format!("$wasm.{name}({})", marshal.args.join(", "));
    // WebAssembly returns the result, then the numbers of the `inout` objects.
    let back: usize = shape.writeback.iter().map(|(_, s)| numbers(s)).sum();
    let given = shape.result.as_ref().map_or(0, |r| r.scalars().len());
    let many = given + back > 1;
    let mut index = 0;
    let value = shape
        .result
        .as_ref()
        .map(|result| rebuild(result, many, &mut index));
    let mut assigns = Vec::new();
    for (param, value) in &shape.writeback {
        write_back(param, value, many, &mut index, &mut assigns);
    }
    let assigns = assigns.join(" ");
    // `$` cannot start a Peras name, so `$r` never hides a parameter.
    let body = match (value, assigns.is_empty()) {
        (None, true) if marshal.pre.is_empty() => call,
        (None, true) => format!("{call};"),
        (None, false) => format!("const $r = {call}; {assigns}"),
        (Some(value), true) => format!("const $r = {call}; return {value};"),
        (Some(value), false) => format!("const $r = {call}; {assigns} return {value};"),
    };
    if marshal.pre.is_empty() {
        let body = if body.ends_with(';') {
            format!("{{ {body} }}")
        } else {
            body
        };
        return format!("export const {name} = ({}) => {body};\n", params.join(", "));
    }
    // Arrays are copied into buffers first, and released afterwards, also when
    // the call throws.
    let pre = marshal.pre.join(" ");
    let body = if marshal.post.is_empty() {
        format!("{{ {pre} {body} }}")
    } else {
        format!(
            "{{ {pre} try {{ {body} }} finally {{ {} }} }}",
            marshal.post.join(" ")
        )
    };
    format!("export const {name} = ({}) => {body};\n", params.join(", "))
}

/// The columns of an array of structures: their kinds as JavaScript reads
/// them (`bool` as a number, turned back by `!== 0`), and a function building
/// element `i` from the columns `c`.
/// The names of the variants of an enumeration, as a JavaScript array.
fn variant_names(variants: &[(String, Vec<Shape>)]) -> String {
    let names: Vec<String> = variants.iter().map(|(v, _)| format!("\"{v}\"")).collect();
    format!("[{}]", names.join(", "))
}

fn rows(element: &Shape) -> (String, String) {
    let mut columns = Vec::new();
    record_columns("v", element, 0, false, &mut columns);
    let kinds: Vec<String> = columns
        .iter()
        .map(|(_, kind)| {
            let kind = if kind == "bool" { "i32" } else { kind.as_str() };
            format!("\"{kind}\"")
        })
        .collect();
    let mut index = 0;
    let object = rebuild_at(element, &mut index, &|k| format!("c[{k}][i]"), false);
    (kinds.join(", "), format!("(c, i) => {object}"))
}

/// The columns of an array of `shape`, for an element at `path`: the
/// JavaScript expression of each stored number, and its kind. A string or an
/// array inside an element becomes a new buffer, stored as a pointer (`ptr`),
/// which the column owns.
fn record_columns(
    path: &str,
    shape: &Shape,
    depth: usize,
    optional: bool,
    out: &mut Vec<(String, String)>,
) {
    match shape {
        Shape::Value(ty) => {
            let elem = Elem::of(*ty).unwrap_or(Elem::I32);
            let value = if optional {
                let zero = match ty {
                    Ty::I64 => "0n",
                    Ty::Bool => "false",
                    _ => "0",
                };
                format!("({path} ?? {zero})")
            } else {
                path.to_owned()
            };
            out.push((value, elem_kind(elem).to_owned()));
        }
        Shape::Object(_, fields) => {
            let dot = if optional { "?." } else { "." };
            for (field, value) in fields {
                record_columns(&format!("{path}{dot}{field}"), value, depth, optional, out);
            }
        }
        Shape::Option(inner) => {
            out.push((format!("{path} !== undefined"), "bool".to_owned()));
            record_columns(path, inner, depth, true, out);
        }
        Shape::Enum(_, variants) => {
            out.push((
                format!("$tagOf({path}.kind, {})", variant_names(variants)),
                "i32".to_owned(),
            ));
            for (variant, fields) in variants {
                for (j, field) in fields.iter().enumerate() {
                    let part =
                        format!("({path}.kind === \"{variant}\" ? {path}.values[{j}] : undefined)");
                    record_columns(&part, field, depth, true, out);
                }
            }
        }
        Shape::String(_) => {
            let text = if optional {
                format!("({path} ?? \"\")")
            } else {
                path.to_owned()
            };
            out.push((format!("$toStr({text})"), "ptr".to_owned()));
        }
        Shape::Array(elem, _) => {
            let values = if optional {
                format!("({path} ?? [])")
            } else {
                path.to_owned()
            };
            out.push((
                format!("$toBuf({values}, \"{}\")", elem_kind(*elem)),
                "ptr".to_owned(),
            ));
        }
        Shape::Records(inner, _) => {
            let item = format!("w{depth}");
            let path = &if optional {
                format!("({path} ?? [])")
            } else {
                path.to_owned()
            };
            let mut columns = Vec::new();
            record_columns(&item, inner, depth + 1, false, &mut columns);
            for (column, kind) in columns {
                out.push((
                    format!("$toBuf({path}.map(({item}) => {column}), \"{kind}\")"),
                    "ptr".to_owned(),
                ));
            }
        }
    }
}

/// How many numbers an `inout` value gives back: its scalars, not its buffers,
/// which are changed in place.
fn numbers(shape: &Shape) -> usize {
    match shape {
        Shape::Value(_) => 1,
        Shape::Object(_, fields) => fields.iter().map(|(_, f)| numbers(f)).sum(),
        Shape::Option(inner) => 1 + numbers(inner),
        Shape::Enum(..) => shape.scalars().len(),
        Shape::Array(..) | Shape::String(_) | Shape::Records(..) => 0,
    }
}

/// `p.x = $r[1];` for each number of an `inout` object, in order.
fn write_back(path: &str, shape: &Shape, many: bool, index: &mut usize, out: &mut Vec<String>) {
    match shape {
        Shape::Value(_) => {
            let value = rebuild(shape, many, index);
            out.push(format!("{path} = {value};"));
        }
        Shape::Object(_, fields) => {
            for (field, value) in fields {
                write_back(&format!("{path}.{field}"), value, many, index, out);
            }
        }
        // perasc refuses options passed `inout` from JavaScript.
        Shape::Option(_)
        | Shape::Enum(..)
        | Shape::Array(..)
        | Shape::String(_)
        | Shape::Records(..) => {}
    }
}

/// What calling a function from JavaScript needs: the arguments, and the
/// statements before and after the call for the arrays among them.
#[derive(Default)]
struct Marshal {
    args: Vec<String>,
    pre: Vec<String>,
    post: Vec<String>,
}

/// The name the loader's helpers give an element type.
const fn elem_kind(elem: Elem) -> &'static str {
    match elem {
        Elem::I32 => "i32",
        Elem::I64 => "i64",
        Elem::F32 => "f32",
        Elem::F64 => "f64",
        Elem::Bool => "bool",
        Elem::U8 => "u8",
        Elem::Handle => "ptr",
    }
}

/// The scalars of a parameter, as JavaScript expressions: `p.x`, `p.inner.y`.
/// Inside an option, which JavaScript passes as `undefined` when it is empty,
/// fields are read with `?.` and an absent scalar is zero.
fn flatten(path: &str, shape: &Shape, optional: bool, out: &mut Marshal) {
    match shape {
        Shape::Array(elem, pass) => {
            let buffer = format!("$a{}", out.pre.len());
            let kind = elem_kind(*elem);
            let values = if optional {
                format!("({path} ?? [])")
            } else {
                path.to_owned()
            };
            out.pre
                .push(format!("const {buffer} = $toBuf({values}, \"{kind}\");"));
            if *pass == Pass::Inout {
                out.post
                    .push(format!("$copyBack({path}, {buffer}, \"{kind}\");"));
            }
            // A `sink` parameter is released by the function itself.
            if *pass != Pass::Sink {
                out.post.push(format!("$runtime.buf_release({buffer}, 0);"));
            }
            out.args.push(buffer);
        }
        Shape::Records(element, pass) => {
            // One column per scalar of an element: `(v) => v.x`, or a new
            // buffer for a string or an array inside it.
            let mut columns = Vec::new();
            record_columns("v", element, 0, false, &mut columns);
            let mut buffers = Vec::new();
            for (column, kind) in &columns {
                let buffer = format!("$a{}", out.pre.len());
                let rows = if optional {
                    format!("({path} ?? [])")
                } else {
                    path.to_owned()
                };
                out.pre.push(format!(
                    "const {buffer} = $toBuf({rows}.map((v) => {column}), \"{kind}\");"
                ));
                buffers.push(buffer);
            }
            if *pass == Pass::Inout {
                let (kinds, build) = rows(element);
                out.post.push(format!(
                    "$setRows({path}, [{}], [{kinds}], {build});",
                    buffers.join(", ")
                ));
            }
            // Releasing a column of buffers releases the buffers too.
            if *pass != Pass::Sink {
                for buffer in &buffers {
                    out.post.push(format!("$runtime.buf_release({buffer}, 0);"));
                }
            }
            out.args.extend(buffers);
        }
        Shape::Enum(_, variants) => {
            let names = variant_names(variants);
            out.args.push(format!("$tagOf({path}.kind, {names})"));
            for (variant, fields) in variants {
                for (j, field) in fields.iter().enumerate() {
                    let part =
                        format!("({path}.kind === \"{variant}\" ? {path}.values[{j}] : undefined)");
                    flatten(&part, field, true, out);
                }
            }
        }
        Shape::String(pass) => {
            let buffer = format!("$a{}", out.pre.len());
            let text = if optional {
                format!("({path} ?? \"\")")
            } else {
                path.to_owned()
            };
            out.pre.push(format!("const {buffer} = $toStr({text});"));
            // JavaScript strings never change: `inout` is not exported (perasc
            // refuses it), and a `sink` parameter is released by the function.
            if *pass != Pass::Sink {
                out.post.push(format!("$runtime.buf_release({buffer}, 0);"));
            }
            out.args.push(buffer);
        }
        Shape::Value(ty) if optional => {
            let zero = match ty {
                Ty::I64 => "0n",
                Ty::Bool => "false",
                _ => "0",
            };
            out.args.push(format!("({path} ?? {zero})"));
        }
        Shape::Value(_) => out.args.push(path.to_owned()),
        Shape::Object(_, fields) => {
            let dot = if optional { "?." } else { "." };
            for (field, value) in fields {
                flatten(&format!("{path}{dot}{field}"), value, optional, out);
            }
        }
        Shape::Option(inner) => {
            out.args.push(format!("{path} !== undefined"));
            flatten(path, inner, true, out);
        }
    }
}

/// A result rebuilt from `$r`, the WebAssembly result: an array when `many`.
fn rebuild(shape: &Shape, many: bool, index: &mut usize) -> String {
    let at = |k: usize| {
        if many {
            format!("$r[{k}]")
        } else {
            "$r".to_owned()
        }
    };
    rebuild_at(shape, index, &at, true)
}

/// A value rebuilt from scalars that `at` reads by their position. The
/// buffers it reaches are released when `owned`; otherwise their owner,
/// a column of an array, releases them afterwards.
fn rebuild_at(
    shape: &Shape,
    index: &mut usize,
    at: &dyn Fn(usize) -> String,
    owned: bool,
) -> String {
    match shape {
        Shape::Value(ty) => {
            let scalar = at(*index);
            *index += 1;
            if *ty == Ty::Bool {
                format!("{scalar} !== 0")
            } else {
                scalar
            }
        }
        Shape::Object(_, fields) => {
            let fields: Vec<String> = fields
                .iter()
                .map(|(field, value)| format!("{field}: {}", rebuild_at(value, index, at, owned)))
                .collect();
            format!("({{ {} }})", fields.join(", "))
        }
        Shape::Array(elem, _) => {
            let handle = at(*index);
            *index += 1;
            let take = if owned { "$take" } else { "$fromBuf" };
            format!("{take}({handle}, \"{}\")", elem_kind(*elem))
        }
        Shape::Records(element, _) => {
            let (kinds, build) = rows(element);
            let mut columns = Vec::new();
            record_columns("v", element, 0, false, &mut columns);
            let count = columns.len();
            let handles: Vec<String> = (0..count).map(|k| at(*index + k)).collect();
            *index += count;
            let take = if owned { "$takeRows" } else { "$readRows" };
            format!("{take}([{}], [{kinds}], {build})", handles.join(", "))
        }
        Shape::String(_) => {
            let handle = at(*index);
            *index += 1;
            let take = if owned { "$takeStr" } else { "$readStr" };
            format!("{take}({handle})")
        }
        Shape::Option(inner) => {
            // Both read, so that a buffer inside is released even when empty.
            let some = rebuild_at(&Shape::Value(Ty::Bool), index, at, owned);
            let value = rebuild_at(inner, index, at, owned);
            format!("$opt({some}, {value})")
        }
        Shape::Enum(_, variants) => {
            let tag = at(*index);
            *index += 1;
            let payloads: Vec<String> = variants
                .iter()
                .map(|(_, fields)| {
                    let values: Vec<String> = fields
                        .iter()
                        .map(|f| rebuild_at(f, index, at, owned))
                        .collect();
                    format!("[{}]", values.join(", "))
                })
                .collect();
            format!(
                "$variant({tag}, {}, [{}])",
                variant_names(variants),
                payloads.join(", ")
            )
        }
    }
}

/// The allocator, instantiated first, and the imports it gives the module.
fn runtime_imports(runtime: &Runtime<'_>) -> String {
    let faults: Vec<String> = runtime.faults.iter().map(|m| js_string(m)).collect();
    let sites: Vec<String> = std::iter::once("?".to_owned())
        .chain(runtime.sites.iter().cloned())
        .map(|s| js_string(&s))
        .collect();
    let imports = format!(
        "// The debug allocator (decision 0001): a memory bug throws a PerasMemoryError.\n\
         const $faults = [{faults}];\n\
         const $sites = [{sites}];\n\
         const $site = (n) => $sites[n] ?? \"?\";\n\
         const $fault = (kind, ptr, allocSite, releaseSite) => {{\n  \
         const text = ($faults[kind - 1] ?? `memory fault ${{kind}}`)\n    \
         .replaceAll(\"{{alloc}}\", $site(allocSite))\n    \
         .replaceAll(\"{{release}}\", $site(releaseSite))\n    \
         .replaceAll(\"{{count}}\", String(releaseSite))\n    \
         .replaceAll(\"{{size}}\", String(ptr >>> 0));\n  \
         const error = new Error(text);\n  \
         error.name = \"PerasMemoryError\";\n  \
         Object.assign(error, {{ kind, allocSite: $site(allocSite), releaseSite: $site(releaseSite) }});\n  \
         throw error;\n\
         }};\n\
         const $allocator = Uint8Array.from(atob(\"{allocator}\"), (c) => c.charCodeAt(0));\n\
         const $runtime = (await WebAssembly.instantiate($allocator, {{ peras: {{ fault: $fault }} }})).instance.exports;\n\
         const $bounds = (index, length) => {{\n  \
         throw new RangeError({bounds}.replace(\"{{index}}\", String(index)).replace(\"{{length}}\", String(length)));\n\
         }};\n\
         const $imports = {{ {module}: {{ alloc: $runtime.alloc, release: $runtime.release, buf_new: $runtime.buf_new, buf_reserve: $runtime.buf_reserve, buf_release: $runtime.buf_release, bounds: $bounds, buf_clone: $runtime.buf_clone, memory: $runtime.memory }} }};\n\
",
        bounds = js_string(runtime.bounds),
        faults = faults.join(", "),
        sites = sites.join(", "),
        allocator = base64(runtime.allocator),
        module = RUNTIME_MODULE,
    );
    imports + HEAP_HELPERS
}

/// The loader's helpers for options and enumerations: every part is read,
/// so that the buffers inside are released, then one is chosen.
const VALUE_HELPERS: &str = "const $opt = (some, value) => (some ? value : undefined);
const $variant = (tag, names, payloads) => ({ kind: names[tag], values: payloads[tag] });
const $tagOf = (kind, names) => {
  const tag = names.indexOf(kind);
  if (tag < 0) throw new TypeError(`unknown variant ${String(kind)}`);
  return tag;
};
";

/// The loader's helpers for arrays and strings, which cross the boundary as
/// copies: into a new buffer, and out of one.
const HEAP_HELPERS: &str = "// Arrays cross as copies: into a new buffer, and out of one.\n\
         const $kinds = { i32: [4, \"getInt32\", \"setInt32\"], i64: [8, \"getBigInt64\", \"setBigInt64\"], f32: [4, \"getFloat32\", \"setFloat32\"], f64: [8, \"getFloat64\", \"setFloat64\"], bool: [4, \"getInt32\", \"setInt32\"], u8: [1, \"getUint8\", \"setUint8\"], ptr: [4, \"getInt32\", \"setInt32\"] };\n\
         const $toBuf = (values, kind) => {\n  \
         const [size, , set] = $kinds[kind];\n  \
         const n = values.length;\n  \
         // A column of pointers owns the buffers they point to (DEEP).\n  \
         const h = $runtime.buf_new(n, kind === \"ptr\" ? size | 0x100 : size, 0);\n  \
         const view = new DataView($runtime.memory.buffer);\n  \
         const data = view.getUint32(h + 8, true);\n  \
         for (let i = 0; i < n; i++) view[set](data + i * size, kind === \"bool\" ? (values[i] ? 1 : 0) : values[i], true);\n  \
         view.setUint32(h, n, true);\n  \
         return h;\n\
         };\n\
         const $fromBuf = (h, kind) => {\n  \
         const [size, get] = $kinds[kind];\n  \
         const view = new DataView($runtime.memory.buffer);\n  \
         const n = view.getUint32(h, true);\n  \
         const data = view.getUint32(h + 8, true);\n  \
         const out = new Array(n);\n  \
         for (let i = 0; i < n; i++) {\n    \
         const v = view[get](data + i * size, true);\n    \
         out[i] = kind === \"bool\" ? v !== 0 : v;\n  \
         }\n  \
         return out;\n\
         };\n\
         const $take = (h, kind) => {\n  \
         const out = $fromBuf(h, kind);\n  \
         $runtime.buf_release(h, 0);\n  \
         return out;\n\
         };\n\
         // Strings cross as UTF-8 bytes.\n\
         const $encoder = new TextEncoder();\n\
         const $decoder = new TextDecoder(\"utf-8\", { fatal: true });\n\
         const $toStr = (text) => {\n  \
         const bytes = $encoder.encode(text);\n  \
         const h = $runtime.buf_new(bytes.length, 1, 0);\n  \
         const view = new DataView($runtime.memory.buffer);\n  \
         new Uint8Array($runtime.memory.buffer, view.getUint32(h + 8, true), bytes.length).set(bytes);\n  \
         view.setUint32(h, bytes.length, true);\n  \
         return h;\n\
         };\n\
         const $readStr = (h) => {\n  \
         const view = new DataView($runtime.memory.buffer);\n  \
         const n = view.getUint32(h, true);\n  \
         return $decoder.decode(new Uint8Array($runtime.memory.buffer, view.getUint32(h + 8, true), n));\n\
         };\n\
         const $takeStr = (h) => {\n  \
         const text = $readStr(h);\n  \
         $runtime.buf_release(h, 0);\n  \
         return text;\n\
         };\n\
         // Arrays of structures: one buffer per field, rebuilt into objects.\n\
         // The buffers inside are read first; releasing the columns releases them.\n\
         const $readRows = (handles, kinds, build) => {\n  \
         const c = handles.map((h, k) => $fromBuf(h, kinds[k]));\n  \
         const n = c.length > 0 ? c[0].length : 0;\n  \
         const out = new Array(n);\n  \
         for (let i = 0; i < n; i++) out[i] = build(c, i);\n  \
         return out;\n\
         };\n\
         const $takeRows = (handles, kinds, build) => {\n  \
         const out = $readRows(handles, kinds, build);\n  \
         for (const h of handles) $runtime.buf_release(h, 0);\n  \
         return out;\n\
         };\n\
         const $setRows = (values, handles, kinds, build) => {\n  \
         const c = handles.map((h, k) => $fromBuf(h, kinds[k]));\n  \
         const n = c.length > 0 ? c[0].length : 0;\n  \
         values.length = n;\n  \
         for (let i = 0; i < n; i++) values[i] = build(c, i);\n\
         };\n\
         const $copyBack = (values, h, kind) => {\n  \
         const out = $fromBuf(h, kind);\n  \
         values.length = out.length;\n  \
         for (let i = 0; i < out.length; i++) values[i] = out[i];\n\
         };\n";

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
    // Each structure seen by JavaScript, once, before the functions.
    let mut interfaces: Vec<(String, String)> = Vec::new();
    for f in module.functions.iter().filter(|f| f.exported) {
        if let Some(shape) = &f.shape {
            let values = shape
                .params
                .iter()
                .map(|(_, s)| s)
                .chain(shape.result.iter());
            for value in values {
                interface(value, &mut interfaces);
            }
        }
    }
    for (_, declaration) in &interfaces {
        out.push_str(declaration);
    }
    if module.uses_heap() {
        out.push_str(
            "/** Blocks still alive in the debug allocator, and a check that throws on a leak. */\n\
             export declare const __perasHeap: { live(): number; checkLeaks(): void };\n",
        );
    }
    for f in module.functions.iter().filter(|f| f.exported) {
        if let Some(shape) = &f.shape {
            let params: Vec<String> = shape
                .params
                .iter()
                .map(|(name, value)| format!("{name}: {}", ts_shape(value)))
                .collect();
            let result = shape
                .result
                .as_ref()
                .map_or_else(|| "void".to_owned(), ts_shape);
            let _ = writeln!(
                out,
                "export declare function {}({}): {result};",
                f.name,
                params.join(", ")
            );
            continue;
        }
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

/// The TypeScript type of a scalar.
const fn ts_scalar(ty: Ty) -> &'static str {
    match ty {
        Ty::I64 => "bigint",
        Ty::Bool => "boolean",
        Ty::Void => "void",
        _ => "number",
    }
}

/// The TypeScript type of a value: a scalar type, or a structure's name.
fn ts_shape(shape: &Shape) -> String {
    match shape {
        Shape::Value(ty) => ts_scalar(*ty).to_owned(),
        Shape::Object(name, _) | Shape::Enum(name, _) => name.clone(),
        Shape::Option(inner) => format!("{} | undefined", ts_shape(inner)),
        Shape::String(_) => "string".to_owned(),
        Shape::Records(element, pass) => {
            let item = ts_shape(element);
            // `(number | undefined)[]`, not `number | undefined[]`.
            let item = if item.contains('|') {
                format!("({item})")
            } else {
                item
            };
            if matches!(pass, Pass::Borrow | Pass::Sink) {
                format!("readonly {item}[]")
            } else {
                format!("{item}[]")
            }
        }
        Shape::Array(elem, pass) => {
            let item = ts_scalar(elem.ty());
            // An array the function does not change can be read-only.
            if matches!(pass, Pass::Borrow | Pass::Sink) {
                format!("readonly {item}[]")
            } else {
                format!("{item}[]")
            }
        }
    }
}

/// `export interface Point { x: number; y: number; }`, for a structure and the
/// structures in it, each once.
fn interface(shape: &Shape, out: &mut Vec<(String, String)>) {
    if let Shape::Option(inner) | Shape::Records(inner, _) = shape {
        return interface(inner, out);
    }
    if let Shape::Enum(name, variants) = shape {
        for (_, fields) in variants {
            for field in fields {
                interface(field, out);
            }
        }
        if out.iter().any(|(n, _)| n == name) {
            return;
        }
        let cases: Vec<String> = variants
            .iter()
            .map(|(variant, fields)| {
                let values: Vec<String> = fields.iter().map(ts_shape).collect();
                format!("{{ kind: \"{variant}\"; values: [{}] }}", values.join(", "))
            })
            .collect();
        out.push((
            name.clone(),
            format!("export type {name} = {};\n", cases.join(" | ")),
        ));
        return;
    }
    let Shape::Object(name, fields) = shape else {
        return;
    };
    for (_, field) in fields {
        interface(field, out);
    }
    if out.iter().any(|(n, _)| n == name) {
        return;
    }
    let fields: Vec<String> = fields
        .iter()
        .map(|(field, value)| format!("{field}: {};", ts_shape(value)))
        .collect();
    out.push((
        name.clone(),
        format!("export interface {name} {{ {} }}\n", fields.join(" ")),
    ));
}

#[cfg(test)]
mod tests;
