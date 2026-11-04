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
    BlockType, CodeSection, ExportKind, ExportSection, Function as WasmFunction, FunctionSection,
    Ieee32, Ieee64, Instruction as I, TypeSection, ValType,
};

/// The value type of an IR type; `None` for `void`.
const fn val_type(ty: Ty) -> Option<ValType> {
    match ty {
        Ty::I32 | Ty::Bool => Some(ValType::I32),
        Ty::I64 => Some(ValType::I64),
        Ty::F32 => Some(ValType::F32),
        Ty::F64 => Some(ValType::F64),
        Ty::Void => None,
    }
}

/// Encodes a module as a WebAssembly binary.
#[must_use]
pub fn compile(module: &Module) -> Vec<u8> {
    let mut types = TypeSection::new();
    let mut functions = FunctionSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    for (index, function) in module.functions.iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let params: Vec<ValType> = function.params().filter_map(|l| val_type(l.ty)).collect();
        let results: Vec<ValType> = val_type(function.ret).into_iter().collect();
        types.ty().function(params, results);
        functions.function(index);
        if function.exported {
            exports.export(&function.name, ExportKind::Func, index);
        }
        code.function(&FnEncoder::encode(function));
    }
    let mut wasm = wasm_encoder::Module::new();
    wasm.section(&types)
        .section(&functions)
        .section(&exports)
        .section(&code);
    wasm.finish()
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
    /// An `if`, or a short-circuit `&&` / `||`.
    Other,
}

struct FnEncoder {
    out: WasmFunction,
    frames: Vec<Frame>,
    /// Scratch locals for integer `abs`, indexed after the function's own locals.
    temp_i32: u32,
    temp_i64: u32,
}

impl FnEncoder {
    fn encode(function: &Function) -> WasmFunction {
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

    /// `block $break { loop $top { br_if $break !cond; block $continue { body } step; br $top } }`
    fn loop_(&mut self, cond: Option<&Expr>, body: &[Stmt], step: &[Stmt]) {
        self.emit(&I::Block(BlockType::Empty));
        self.frames.push(Frame::Break);
        self.emit(&I::Loop(BlockType::Empty));
        self.frames.push(Frame::Top);
        if let Some(cond) = cond {
            self.expr(cond);
            self.emit(&I::I32Eqz);
            let depth = self.depth(Frame::Break);
            self.emit(&I::BrIf(depth));
        }
        self.emit(&I::Block(BlockType::Empty));
        self.frames.push(Frame::Continue);
        self.stmts(body);
        self.frames.pop();
        self.emit(&I::End);
        self.stmts(step);
        let depth = self.depth(Frame::Top);
        self.emit(&I::Br(depth));
        self.frames.pop();
        self.emit(&I::End);
        self.frames.pop();
        self.emit(&I::End);
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Const(c) => self.constant(*c, e.ty),
            ExprKind::Local(local) => self.emit(&I::LocalGet(local.0)),
            ExprKind::Unary(op, operand) => self.unary(*op, operand, e.ty),
            ExprKind::Binary(op, lhs, rhs) => self.binary(*op, lhs, rhs),
            ExprKind::Call(id, args) => {
                for arg in args {
                    self.expr(arg);
                }
                self.emit(&I::Call(id.0));
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
            Ty::I32 | Ty::Bool => int32(op),
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

/// A JavaScript ES module that loads the `.wasm` file next to it and exports its
/// functions; `bool` results become JavaScript booleans.
#[must_use]
pub fn js_loader(module: &Module, wasm_file: &str) -> String {
    let mut js = format!(
        "// Generated by perasc. Loads {wasm_file} and exports its functions.\n\
         const url = new URL(\"./{wasm_file}\", import.meta.url);\n\
         const bytes = url.protocol === \"file:\"\n  \
         ? await (await import(\"node:fs/promises\")).readFile(url)\n  \
         : await (await fetch(url)).arrayBuffer();\n\
         const {{ instance }} = await WebAssembly.instantiate(bytes);\n\
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
    js
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
