//! A reference interpreter of the IR, for tests only: it runs a function on given
//! arguments with the semantics both backends implement, so an optimization can be
//! checked against the unoptimized program on many inputs without any engine.

// Truncating casts and exact float comparisons are the semantics being modelled.
#![allow(clippy::cast_possible_truncation, clippy::float_cmp)]

use crate::{
    BinOp, BufOp, BufStmt, Const, Elem, Expr, ExprKind, FnId, MathOp, Module, Stmt, Ty, UnOp,
};

/// A value.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Bool(bool),
    /// A heap block, by its allocation number.
    Handle(u32),
    #[default]
    Void,
}

/// Why a run stopped without a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trap {
    /// Division or remainder by zero, or `MIN / -1`.
    Arithmetic,
    /// The end of a function with a result was reached.
    Unreachable,
    /// More nested calls than the interpreter allows, like a stack overflow.
    Depth,
    /// Too many steps: the program probably does not end.
    Fuel,
    /// A block released while it was not live: twice, or never allocated.
    DoubleRelease,
    /// A block used after it was released, or an empty local read.
    UseAfterRelease,
    /// A negative size was allocated.
    BadAlloc,
    /// `check_index` failed: the program reports an index out of bounds.
    Bounds,
    /// A buffer was read or written past its length without a check first: a
    /// bug in `perasc`, which the backends would not catch.
    Unchecked,
}

/// What a run left on the heap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeapReport {
    /// Blocks allocated during the run.
    pub allocated: u32,
    /// Blocks still live at the end, in allocation order.
    pub live: Vec<u32>,
}

/// Calls function `id` of `module`.
///
/// Runs on a thread with a stack large enough for `MAX_DEPTH` nested calls.
pub fn run(module: &Module, id: FnId, args: &[Value]) -> Result<Value, Trap> {
    run_with_heap(module, id, args).map(|(value, _)| value)
}

/// Calls function `id`, and also reports what is left on the heap.
pub fn run_with_heap(
    module: &Module,
    id: FnId,
    args: &[Value],
) -> Result<(Value, HeapReport), Trap> {
    let module = module.clone();
    let args = args.to_vec();
    let worker = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || {
            let mut machine = Machine {
                module: &module,
                fuel: 50_000_000,
                depth: 0,
                heap: std::collections::BTreeMap::new(),
                deep: std::collections::BTreeSet::new(),
                allocated: 0,
            };
            let value = machine.call(id, &args)?;
            let report = HeapReport {
                allocated: machine.allocated,
                live: machine.heap.into_keys().collect(),
            };
            Ok((value, report))
        });
    worker
        .ok()
        .and_then(|handle| handle.join().ok())
        .unwrap_or(Err(Trap::Depth))
}

const MAX_DEPTH: usize = 5_000;

struct Machine<'a> {
    module: &'a Module,
    fuel: u64,
    depth: usize,
    /// Live blocks, with their elements when they are buffers.
    heap: std::collections::BTreeMap<u32, Vec<Value>>,
    /// The buffers whose elements are buffers they own.
    deep: std::collections::BTreeSet<u32>,
    allocated: u32,
}

enum Flow {
    Normal,
    Break,
    Continue,
    Leave(u32),
    /// Every result: none, one, or several.
    Return(Vec<Value>),
}

fn zero(ty: Ty) -> Value {
    match ty {
        Ty::I32 => Value::I32(0),
        Ty::I64 => Value::I64(0),
        Ty::F32 => Value::F32(0.0),
        Ty::F64 => Value::F64(0.0),
        Ty::Bool => Value::Bool(false),
        // An `owned` local starts empty.
        Ty::Void | Ty::Owned | Ty::Borrowed => Value::Void,
    }
}

impl Machine<'_> {
    /// A call with one result, or none (`Void`).
    fn call(&mut self, id: FnId, args: &[Value]) -> Result<Value, Trap> {
        Ok(self
            .call_all(id, args)?
            .first()
            .copied()
            .unwrap_or(Value::Void))
    }

    /// A call, with every result.
    fn call_all(&mut self, id: FnId, args: &[Value]) -> Result<Vec<Value>, Trap> {
        let function = usize::try_from(id.0)
            .ok()
            .and_then(|i| self.module.functions.get(i))
            .ok_or(Trap::Unreachable)?;
        if self.depth >= MAX_DEPTH {
            return Err(Trap::Depth);
        }
        self.depth += 1;
        let mut locals: Vec<Value> = function.locals.iter().map(|l| zero(l.ty)).collect();
        for (slot, arg) in locals.iter_mut().zip(args) {
            *slot = *arg;
        }
        let flow = self.stmts(&function.body, &mut locals);
        self.depth -= 1;
        match flow? {
            Flow::Return(values) => Ok(values),
            _ if function.ret == Ty::Void => Ok(Vec::new()),
            _ => Err(Trap::Unreachable),
        }
    }

    fn stmts(&mut self, stmts: &[Stmt], locals: &mut Vec<Value>) -> Result<Flow, Trap> {
        for stmt in stmts {
            match self.stmt(stmt, locals)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    fn stmt(&mut self, stmt: &Stmt, locals: &mut Vec<Value>) -> Result<Flow, Trap> {
        self.fuel = self.fuel.checked_sub(1).ok_or(Trap::Fuel)?;
        Ok(match stmt {
            Stmt::Set(local, value) => {
                let v = self.expr(value, locals)?;
                if let Some(slot) = usize::try_from(local.0)
                    .ok()
                    .and_then(|i| locals.get_mut(i))
                {
                    *slot = v;
                }
                Flow::Normal
            }
            Stmt::Expr(e) => {
                self.expr(e, locals)?;
                Flow::Normal
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                if self.expr(cond, locals)? == Value::Bool(true) {
                    self.stmts(then, locals)?
                } else {
                    self.stmts(otherwise, locals)?
                }
            }
            Stmt::Loop { cond, body, step } => loop {
                self.fuel = self.fuel.checked_sub(1).ok_or(Trap::Fuel)?;
                if let Some(cond) = cond
                    && self.expr(cond, locals)? != Value::Bool(true)
                {
                    break Flow::Normal;
                }
                match self.stmts(body, locals)? {
                    Flow::Break => break Flow::Normal,
                    flow @ (Flow::Return(_) | Flow::Leave(_)) => break flow,
                    Flow::Normal | Flow::Continue => {}
                }
                if let flow @ (Flow::Return(_) | Flow::Leave(_)) = self.stmts(step, locals)? {
                    break flow;
                }
            },
            Stmt::Block(body) => match self.stmts(body, locals)? {
                Flow::Leave(0) => Flow::Normal,
                Flow::Leave(depth) => Flow::Leave(depth - 1),
                flow => flow,
            },
            Stmt::Leave(depth) => Flow::Leave(*depth),
            Stmt::Release(local) => {
                let slot = slot(locals, *local);
                match slot.map(std::mem::take) {
                    Some(Value::Handle(block)) if self.heap.remove(&block).is_some() => {}
                    _ => return Err(Trap::DoubleRelease),
                }
                Flow::Normal
            }
            Stmt::Buf(op, args) => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.expr(arg, locals)?);
                }
                self.buf_stmt(*op, &values)?;
                Flow::Normal
            }
            Stmt::Break => Flow::Break,
            Stmt::Continue => Flow::Continue,
            Stmt::Return(value) => {
                let values = match value {
                    Some(e) => vec![self.expr(e, locals)?],
                    None => Vec::new(),
                };
                Flow::Return(values)
            }
            Stmt::ReturnMany(values) => {
                let mut out = Vec::with_capacity(values.len());
                for e in values {
                    out.push(self.expr(e, locals)?);
                }
                Flow::Return(out)
            }
            Stmt::CallMany {
                targets,
                callee,
                args,
            } => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.expr(arg, locals)?);
                }
                for (target, value) in targets.iter().zip(self.call_all(*callee, &values)?) {
                    if let Some(slot) = slot(locals, *target) {
                        *slot = value;
                    }
                }
                Flow::Normal
            }
        })
    }

    fn expr(&mut self, e: &Expr, locals: &mut Vec<Value>) -> Result<Value, Trap> {
        Ok(match &e.kind {
            ExprKind::Const(c) => match (c, e.ty) {
                (Const::Int(v), Ty::I64) => Value::I64(*v),
                (Const::Int(v), _) => Value::I32(i32::try_from(*v).unwrap_or(i32::MIN)),
                #[allow(clippy::cast_possible_truncation)]
                (Const::Float(v), Ty::F32) => Value::F32(*v as f32),
                (Const::Float(v), _) => Value::F64(*v),
                (Const::Bool(v), _) => Value::Bool(*v),
            },
            ExprKind::Local(local) => {
                let value = slot(locals, *local).map_or(Value::Void, |v| *v);
                self.live(value)?
            }
            ExprKind::Borrow(local) => {
                let value = slot(locals, *local).map_or(Value::Void, |v| *v);
                self.block(value)?
            }
            ExprKind::Move(local) => {
                let value = slot(locals, *local).map_or(Value::Void, std::mem::take);
                self.block(value)?
            }
            ExprKind::Alloc(size) => match self.expr(size, locals)? {
                Value::I32(n) if n >= 0 => {
                    self.allocated += 1;
                    self.heap.insert(self.allocated, Vec::new());
                    Value::Handle(self.allocated)
                }
                _ => return Err(Trap::BadAlloc),
            },
            ExprKind::Buf(op, args) => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.expr(arg, locals)?);
                }
                self.buf_op(*op, &values)?
            }
            ExprKind::Unary(op, x) => unary(*op, self.expr(x, locals)?),
            ExprKind::Binary(BinOp::And, a, b) => {
                if self.expr(a, locals)? == Value::Bool(true) {
                    self.expr(b, locals)?
                } else {
                    Value::Bool(false)
                }
            }
            ExprKind::Binary(BinOp::Or, a, b) => {
                if self.expr(a, locals)? == Value::Bool(true) {
                    Value::Bool(true)
                } else {
                    self.expr(b, locals)?
                }
            }
            ExprKind::Binary(op, a, b) => {
                let (a, b) = (self.expr(a, locals)?, self.expr(b, locals)?);
                binary(*op, a, b)?
            }
            ExprKind::Call(id, args) => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.expr(arg, locals)?);
                }
                self.call(*id, &values)?
            }
            ExprKind::Convert(x) => convert(self.expr(x, locals)?, e.ty),
            ExprKind::Math(op, x) => math(*op, self.expr(x, locals)?),
        })
    }
}

impl Machine<'_> {
    /// The elements of a live buffer.
    fn elements(&mut self, value: Value) -> Result<&mut Vec<Value>, Trap> {
        match value {
            Value::Handle(block) => self.heap.get_mut(&block).ok_or(Trap::UseAfterRelease),
            _ => Err(Trap::UseAfterRelease),
        }
    }

    fn buf_op(&mut self, op: BufOp, values: &[Value]) -> Result<Value, Trap> {
        Ok(match (op, values) {
            (BufOp::New(elem), [Value::I32(cap)]) if *cap >= 0 => {
                self.allocated += 1;
                self.heap.insert(self.allocated, Vec::new());
                if elem == Elem::Handle {
                    self.deep.insert(self.allocated);
                }
                Value::Handle(self.allocated)
            }
            (BufOp::Clone, [buf]) => self.clone_buffer(*buf)?,
            (BufOp::Take, [buf, Value::I32(index)]) => {
                let index = usize::try_from(*index).map_err(|_| Trap::Unchecked)?;
                *self.elements(*buf)?.get(index).ok_or(Trap::Unchecked)?
            }
            (BufOp::New(_), _) => return Err(Trap::BadAlloc),
            (BufOp::Literal(index), []) => {
                let bytes = usize::try_from(index)
                    .ok()
                    .and_then(|i| self.module.data.get(i))
                    .ok_or(Trap::Unreachable)?;
                let elements = bytes.iter().map(|&b| Value::I32(i32::from(b))).collect();
                self.allocated += 1;
                self.heap.insert(self.allocated, elements);
                Value::Handle(self.allocated)
            }
            (BufOp::Len, [buf]) => {
                let len = self.elements(*buf)?.len();
                Value::I32(i32::try_from(len).unwrap_or(i32::MAX))
            }
            (BufOp::Get(_), [buf, Value::I32(index)]) => {
                let index = usize::try_from(*index).map_err(|_| Trap::Unchecked)?;
                *self.elements(*buf)?.get(index).ok_or(Trap::Unchecked)?
            }
            _ => return Err(Trap::Unreachable),
        })
    }

    /// Releases a buffer, and the buffers it owns.
    fn free(&mut self, block: u32) -> Result<(), Trap> {
        let elements = self.heap.remove(&block).ok_or(Trap::DoubleRelease)?;
        if self.deep.remove(&block) {
            for element in elements {
                if let Value::Handle(inner) = element {
                    self.free(inner)?;
                }
            }
        }
        Ok(())
    }

    /// A copy of a buffer, with copies of the buffers it owns.
    fn clone_buffer(&mut self, buf: Value) -> Result<Value, Trap> {
        let mut elements = self.elements(buf)?.clone();
        let Value::Handle(block) = buf else {
            return Err(Trap::UseAfterRelease);
        };
        let deep = self.deep.contains(&block);
        if deep {
            for element in &mut elements {
                *element = self.clone_buffer(*element)?;
            }
        }
        self.allocated += 1;
        self.heap.insert(self.allocated, elements);
        if deep {
            self.deep.insert(self.allocated);
        }
        Ok(Value::Handle(self.allocated))
    }

    fn buf_stmt(&mut self, op: BufStmt, values: &[Value]) -> Result<(), Trap> {
        match (op, values) {
            (BufStmt::Release, [Value::Handle(block)]) => self.free(*block)?,
            (BufStmt::Release, _) => return Err(Trap::DoubleRelease),
            (BufStmt::Set(elem), [buf, Value::I32(index), value]) => {
                let index = usize::try_from(*index).map_err(|_| Trap::Unchecked)?;
                let slot = self.elements(*buf)?.get_mut(index).ok_or(Trap::Unchecked)?;
                let old = std::mem::replace(slot, stored(elem, *value));
                // The element replaced is released.
                if let (Elem::Handle, Value::Handle(old)) = (elem, old) {
                    self.free(old)?;
                }
            }
            (BufStmt::Push(elem), [buf, value]) => self.elements(*buf)?.push(stored(elem, *value)),
            (BufStmt::Append(_), [buf, other]) => {
                let added = self.elements(*other)?.clone();
                self.elements(*buf)?.extend(added);
            }
            (BufStmt::Truncate, [buf, Value::I32(len)]) => {
                let elements = self.elements(*buf)?;
                let len = usize::try_from(*len).map_err(|_| Trap::Unchecked)?;
                if len > elements.len() {
                    return Err(Trap::Unchecked);
                }
                elements.truncate(len);
            }
            (BufStmt::CheckIndex, [Value::I32(index), Value::I32(len)]) => {
                if *index < 0 || index >= len {
                    return Err(Trap::Bounds);
                }
            }
            _ => return Err(Trap::Unreachable),
        }
        Ok(())
    }

    /// A handle must name a live block; an empty local holds `Void`.
    fn live(&self, value: Value) -> Result<Value, Trap> {
        match value {
            Value::Handle(block) if !self.heap.contains_key(&block) => Err(Trap::UseAfterRelease),
            other => Ok(other),
        }
    }

    /// A moved or borrowed local must hold a live block.
    fn block(&self, value: Value) -> Result<Value, Trap> {
        match value {
            Value::Handle(block) if self.heap.contains_key(&block) => Ok(value),
            _ => Err(Trap::UseAfterRelease),
        }
    }
}

/// A value as a buffer stores it: a byte keeps its low 8 bits.
const fn stored(elem: Elem, value: Value) -> Value {
    match (elem, value) {
        (Elem::U8, Value::I32(v)) => Value::I32(v & 0xFF),
        _ => value,
    }
}

/// The slot of a local. A moved or released local is left `Void`.
fn slot(locals: &mut [Value], local: crate::LocalId) -> Option<&mut Value> {
    usize::try_from(local.0)
        .ok()
        .and_then(|i| locals.get_mut(i))
}

fn unary(op: UnOp, v: Value) -> Value {
    match (op, v) {
        (UnOp::Neg, Value::I32(x)) => Value::I32(x.wrapping_neg()),
        (UnOp::Neg, Value::I64(x)) => Value::I64(x.wrapping_neg()),
        (UnOp::Neg, Value::F32(x)) => Value::F32(-x),
        (UnOp::Neg, Value::F64(x)) => Value::F64(-x),
        (UnOp::Not, Value::Bool(x)) => Value::Bool(!x),
        (UnOp::BitNot, Value::I32(x)) => Value::I32(!x),
        (UnOp::BitNot, Value::I64(x)) => Value::I64(!x),
        (_, other) => other,
    }
}

macro_rules! int_op {
    ($op:expr, $a:expr, $b:expr, $wrap:ident) => {{
        let (a, b) = ($a, $b);
        Ok($wrap(match $op {
            BinOp::Add => a.wrapping_add(b),
            BinOp::Sub => a.wrapping_sub(b),
            BinOp::Mul => a.wrapping_mul(b),
            BinOp::Div => a.checked_div(b).ok_or(Trap::Arithmetic)?,
            BinOp::Rem => {
                if b == 0 {
                    return Err(Trap::Arithmetic);
                }
                a.wrapping_rem(b)
            }
            BinOp::BitAnd => a & b,
            BinOp::BitOr => a | b,
            BinOp::BitXor => a ^ b,
            #[allow(clippy::cast_sign_loss)]
            BinOp::Shl => a.wrapping_shl(b as u32),
            #[allow(clippy::cast_sign_loss)]
            BinOp::Shr => a.wrapping_shr(b as u32),
            BinOp::Eq => return Ok(Value::Bool(a == b)),
            BinOp::Ne => return Ok(Value::Bool(a != b)),
            BinOp::Lt => return Ok(Value::Bool(a < b)),
            BinOp::Le => return Ok(Value::Bool(a <= b)),
            BinOp::Gt => return Ok(Value::Bool(a > b)),
            BinOp::Ge => return Ok(Value::Bool(a >= b)),
            BinOp::And | BinOp::Or => return Err(Trap::Unreachable),
        }))
    }};
}

macro_rules! float_op {
    ($op:expr, $a:expr, $b:expr, $wrap:ident) => {{
        let (a, b) = ($a, $b);
        Ok(match $op {
            BinOp::Add => $wrap(a + b),
            BinOp::Sub => $wrap(a - b),
            BinOp::Mul => $wrap(a * b),
            BinOp::Div => $wrap(a / b),
            BinOp::Eq => Value::Bool(a == b),
            BinOp::Ne => Value::Bool(a != b),
            BinOp::Lt => Value::Bool(a < b),
            BinOp::Le => Value::Bool(a <= b),
            BinOp::Gt => Value::Bool(a > b),
            BinOp::Ge => Value::Bool(a >= b),
            _ => return Err(Trap::Unreachable),
        })
    }};
}

fn binary(op: BinOp, a: Value, b: Value) -> Result<Value, Trap> {
    use Value::{Bool, F32, F64, I32, I64};
    match (a, b) {
        (I32(a), I32(b)) => int_op!(op, a, b, I32),
        (I64(a), I64(b)) => int_op!(op, a, b, I64),
        (F32(a), F32(b)) => float_op!(op, a, b, F32),
        (F64(a), F64(b)) => float_op!(op, a, b, F64),
        (Bool(a), Bool(b)) => Ok(Bool(if op == BinOp::Eq { a == b } else { a != b })),
        _ => Err(Trap::Unreachable),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn convert(v: Value, to: Ty) -> Value {
    let wide = match v {
        Value::I32(x) => f64::from(x),
        Value::I64(x) => x as f64,
        Value::F32(x) => f64::from(x),
        Value::F64(x) => x,
        _ => 0.0,
    };
    match (v, to) {
        (Value::I32(x), Ty::I64) => Value::I64(i64::from(x)),
        (Value::I64(x), Ty::I32) => Value::I32(x as i32),
        (Value::I64(x), Ty::F32) => Value::F32(x as f32),
        (Value::I32(x), Ty::F32) => Value::F32(x as f32),
        (Value::F32(x), Ty::I64) => Value::I64(x as i64),
        (Value::F32(x), Ty::I32) => Value::I32(x as i32),
        (_, Ty::I32) => Value::I32(wide as i32),
        (_, Ty::I64) => Value::I64(wide as i64),
        (_, Ty::F32) => Value::F32(wide as f32),
        (_, Ty::F64) => Value::F64(wide),
        (other, _) => other,
    }
}

fn math(op: MathOp, v: Value) -> Value {
    match (op, v) {
        (MathOp::Abs, Value::I32(x)) => Value::I32(x.wrapping_abs()),
        (MathOp::Abs, Value::I64(x)) => Value::I64(x.wrapping_abs()),
        (MathOp::Abs, Value::F32(x)) => Value::F32(x.abs()),
        (MathOp::Abs, Value::F64(x)) => Value::F64(x.abs()),
        (MathOp::Sqrt, Value::F32(x)) => Value::F32(x.sqrt()),
        (MathOp::Sqrt, Value::F64(x)) => Value::F64(x.sqrt()),
        (MathOp::Floor, Value::F32(x)) => Value::F32(x.floor()),
        (MathOp::Floor, Value::F64(x)) => Value::F64(x.floor()),
        (MathOp::Ceil, Value::F32(x)) => Value::F32(x.ceil()),
        (MathOp::Ceil, Value::F64(x)) => Value::F64(x.ceil()),
        (_, other) => other,
    }
}
