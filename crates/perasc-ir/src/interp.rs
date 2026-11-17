//! A reference interpreter of the IR, for tests only: it runs a function on given
//! arguments with the semantics both backends implement, so an optimization can be
//! checked against the unoptimized program on many inputs without any engine.

// Truncating casts and exact float comparisons are the semantics being modelled.
#![allow(clippy::cast_possible_truncation, clippy::float_cmp)]

use crate::{
    BinOp, BufOp, BufStmt, Const, Elem, EnvOp, Expr, ExprKind, FnId, MathOp, Module, Stmt, Ty, UnOp,
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
    /// `check_overflow` failed: an integer overflowed in a debug build.
    Overflow,
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
                maps: std::collections::BTreeMap::new(),
                globals: module
                    .globals
                    .iter()
                    .map(|g| constant(g.init, g.ty))
                    .collect(),
                counts: std::collections::BTreeMap::new(),
                weak: std::collections::BTreeMap::new(),
            };
            let value = machine.call(id, &args)?;
            let report = HeapReport {
                allocated: machine.allocated,
                live: machine
                    .heap
                    .into_keys()
                    .filter(|k| k & STATIC == 0)
                    .collect(),
            };
            Ok((value, report))
        });
    worker
        .ok()
        .and_then(|handle| handle.join().ok())
        .unwrap_or(Err(Trap::Depth))
}

const MAX_DEPTH: usize = 5_000;

/// The handles of static literals ([`crate::BufOp::Static`]): their index
/// with this bit set, out of the range of the blocks a run allocates. They
/// are never released, and never reported as left on the heap.
const STATIC: u32 = 0x8000_0000;

struct Machine<'a> {
    module: &'a Module,
    fuel: u64,
    depth: usize,
    /// Live blocks, with their elements when they are buffers.
    heap: std::collections::BTreeMap<u32, Vec<Value>>,
    /// The buffers whose elements are buffers they own.
    deep: std::collections::BTreeSet<u32>,
    allocated: u32,
    /// The entries of each map, in insertion order, by its block: its block's
    /// elements, one per live entry, give its `size`.
    maps: std::collections::BTreeMap<u32, Table>,
    /// The module variables, from their initial values (decision 0023).
    globals: Vec<Value>,
    /// How many hold each environment made by [`EnvOp::New`], as the
    /// runtime counts them (decision 0026): a copy adds one, a release takes
    /// one, and the last frees the block. Its elements are the function's
    /// number, how many of its first slots it owns, then the slots.
    counts: std::collections::BTreeMap<u32, u32>,
    /// How many weak holders name each shared value (decision 0032,
    /// prototype r2); a value none holds any more stays, empty, until the
    /// last weak holder lets go.
    weak: std::collections::BTreeMap<u32, u32>,
}

/// Marks a weak holder of a shared value: its block, with this bit set.
const WEAK: u32 = 0x2000_0000;

/// Marks a static environment among the static handles: the function's
/// number follows, or [`ZERO_ENV`] for [`EnvOp::Zero`].
const ENV: u32 = 0x4000_0000;
const ZERO_ENV: u32 = 0x3FFF_FFFF;

/// Whether a handle is a static environment, never counted nor freed.
const fn is_static_env(block: u32) -> bool {
    block & STATIC != 0 && block & ENV != 0
}

/// A constant of type `ty`, as a value.
fn constant(c: Const, ty: Ty) -> Value {
    match (c, ty) {
        (Const::Int(v), Ty::I64) => Value::I64(v),
        (Const::Int(v), _) => Value::I32(i32::try_from(v).unwrap_or(i32::MIN)),
        #[allow(clippy::cast_possible_truncation)]
        (Const::Float(v), Ty::F32) => Value::F32(v as f32),
        (Const::Float(v), _) => Value::F64(v),
        (Const::Bool(v), _) => Value::Bool(v),
    }
}

/// A key of a map: an integer, or a string's bytes.
#[derive(Clone, PartialEq, Eq)]
enum MapKey {
    Int(i64),
    Text(Vec<u8>),
}

/// A map: how many value slots its entries have, and its entries in
/// insertion order, `None` where one was removed.
#[derive(Default)]
struct Table {
    width: usize,
    entries: Vec<Option<Entry>>,
}

/// A live entry of a map: its key, the string the map owns as its key, lent
/// to `for…of`, and its value's slots, as the runtime keeps them: an `I64`,
/// an `F64`, or a `Handle` the map owns.
#[derive(Clone)]
struct Entry {
    key: MapKey,
    handle: Option<u32>,
    slots: Vec<Value>,
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

    /// A loop: its condition, its body, then its step, until it ends.
    fn run_loop(
        &mut self,
        cond: Option<&Expr>,
        body: &[Stmt],
        step: &[Stmt],
        locals: &mut Vec<Value>,
    ) -> Result<Flow, Trap> {
        Ok(loop {
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
        })
    }

    /// `variable = value` on a module variable.
    fn set_global(
        &mut self,
        global: crate::GlobalId,
        value: &Expr,
        locals: &mut Vec<Value>,
    ) -> Result<(), Trap> {
        let v = self.expr(value, locals)?;
        let slot = usize::try_from(global.0)
            .ok()
            .and_then(|i| self.globals.get_mut(i))
            .ok_or(Trap::Unreachable)?;
        *slot = v;
        Ok(())
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
            Stmt::SetGlobal(global, value) => {
                self.set_global(*global, value, locals)?;
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
            Stmt::Loop { cond, body, step } => self.run_loop(cond.as_ref(), body, step, locals)?,
            Stmt::Block(body) => match self.stmts(body, locals)? {
                Flow::Leave(0) => Flow::Normal,
                Flow::Leave(depth) => Flow::Leave(depth - 1),
                flow => flow,
            },
            Stmt::Leave(depth) => Flow::Leave(*depth),
            Stmt::Release(local) => {
                let slot = slot(locals, *local);
                match slot.map(std::mem::take) {
                    Some(Value::Handle(block))
                        if block & STATIC == 0 && self.heap.remove(&block).is_some() => {}
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
                let results = self.call_all(*callee, &values)?;
                store(locals, targets, results);
                Flow::Normal
            }
            Stmt::CallManyIndirect {
                targets,
                callee,
                args,
            } => {
                let mut values = Vec::with_capacity(args.len() + 1);
                for arg in args {
                    values.push(self.expr(arg, locals)?);
                }
                let (callee, env) = self.callee(callee, locals)?;
                values.push(env);
                let results = self.call_all(callee, &values)?;
                store(locals, targets, results);
                Flow::Normal
            }
        })
    }

    /// The function a function value names: its position in the table.
    /// The function a value names, and its environment, which the function
    /// takes last (decision 0026).
    fn callee(&mut self, callee: &Expr, locals: &mut Vec<Value>) -> Result<(FnId, Value), Trap> {
        let env = self.expr(callee, locals)?;
        match self.elements(env)?.first() {
            Some(Value::I32(id)) => Ok((
                FnId(u32::try_from(*id).map_err(|_| Trap::Unreachable)?),
                env,
            )),
            _ => Err(Trap::Unreachable),
        }
    }

    /// An environment (decision 0026).
    fn env_op(&mut self, op: EnvOp, values: &[Value]) -> Result<Value, Trap> {
        let named = |id: FnId| Value::I32(i32::try_from(id.0).unwrap_or(-1));
        Ok(match (op, values) {
            (EnvOp::Static(id, _), []) => self.static_env(id.0, named(id)),
            (EnvOp::Zero, []) => self.static_env(ZERO_ENV, Value::I32(-1)),
            (EnvOp::New(id, owned), slots) => {
                self.allocated += 1;
                let owned = i32::try_from(owned).unwrap_or(i32::MAX);
                let mut elements = vec![named(id), Value::I32(owned)];
                elements.extend_from_slice(slots);
                self.heap.insert(self.allocated, elements);
                self.counts.insert(self.allocated, 1);
                Value::Handle(self.allocated)
            }
            (EnvOp::Shared(owned), slots) => {
                self.allocated += 1;
                let owned = i32::try_from(owned).unwrap_or(i32::MAX);
                let mut elements = vec![Value::I32(-1), Value::I32(owned)];
                elements.extend_from_slice(slots);
                self.heap.insert(self.allocated, elements);
                self.counts.insert(self.allocated, 1);
                Value::Handle(self.allocated)
            }
            (EnvOp::Load(slot) | EnvOp::LoadHandle(slot), [env]) => {
                let index = usize::try_from(slot).map_err(|_| Trap::Unchecked)? + 2;
                *self.elements(*env)?.get(index).ok_or(Trap::Unchecked)?
            }
            (EnvOp::Store(slot) | EnvOp::StoreHandle(slot), [env, value]) => {
                let index = usize::try_from(slot).map_err(|_| Trap::Unchecked)? + 2;
                let Value::Handle(block) = env else {
                    return Err(Trap::UseAfterRelease);
                };
                let at = self
                    .heap
                    .get_mut(block)
                    .and_then(|e| e.get_mut(index))
                    .ok_or(Trap::Unchecked)?;
                let old = std::mem::replace(at, *value);
                if let (EnvOp::StoreHandle(_), Value::Handle(old)) = (op, old) {
                    self.free(old)?;
                }
                Value::Void
            }
            _ => return Err(Trap::Unreachable),
        })
    }

    /// The static environment `number`, made the first time it is used.
    fn static_env(&mut self, number: u32, function: Value) -> Value {
        let key = STATIC | ENV | number;
        self.heap
            .entry(key)
            .or_insert_with(|| vec![function, Value::I32(0)]);
        Value::Handle(key)
    }

    fn expr(&mut self, e: &Expr, locals: &mut Vec<Value>) -> Result<Value, Trap> {
        Ok(match &e.kind {
            ExprKind::Const(c) => constant(*c, e.ty),
            ExprKind::Global(g) => usize::try_from(g.0)
                .ok()
                .and_then(|i| self.globals.get(i))
                .copied()
                .ok_or(Trap::Unreachable)?,
            ExprKind::Local(local) => {
                let value = slot(locals, *local).map_or(Value::Void, |v| *v);
                self.live(value)?
            }
            ExprKind::Borrow(local) => {
                let value = slot(locals, *local).map_or(Value::Void, |v| *v);
                self.block(value)?
            }
            // Under decision 0028's option 1b, a later argument
            // may read what an earlier one moves: the move is the call's.
            ExprKind::Move(local) if perasc_check::proto("PERAS_PROTO_MOVE") == "b" => {
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
            ExprKind::Unary(UnOp::ConvertU, x) => convert_u(self.expr(x, locals)?, e.ty),
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
            ExprKind::Env(op, args) => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.expr(arg, locals)?);
                }
                self.env_op(*op, &values)?
            }
            ExprKind::CallIndirect(callee, args) => {
                let mut values = Vec::with_capacity(args.len() + 1);
                for arg in args {
                    values.push(self.expr(arg, locals)?);
                }
                let (callee, env) = self.callee(callee, locals)?;
                values.push(env);
                self.call(callee, &values)?
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
            (BufOp::Runtime(f), operands) => self.runtime(f, operands)?,
            (BufOp::Take, [buf, Value::I32(index)]) => {
                let index = usize::try_from(*index).map_err(|_| Trap::Unchecked)?;
                *self.elements(*buf)?.get(index).ok_or(Trap::Unchecked)?
            }
            (BufOp::New(_), _) => return Err(Trap::BadAlloc),
            (BufOp::Static(index, _), []) => {
                let key = STATIC | index;
                if !self.heap.contains_key(&key) {
                    let bytes = usize::try_from(index)
                        .ok()
                        .and_then(|i| self.module.data.get(i))
                        .ok_or(Trap::Unreachable)?;
                    let elements = bytes.iter().map(|&b| Value::I32(i32::from(b))).collect();
                    self.heap.insert(key, elements);
                }
                Value::Handle(key)
            }
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

    /// The bytes of a string.
    fn bytes(&mut self, value: Value) -> Result<Vec<u8>, Trap> {
        Ok(self
            .elements(value)?
            .iter()
            .map(|v| match v {
                Value::I32(b) => u8::try_from(*b).unwrap_or(0),
                _ => 0,
            })
            .collect())
    }

    /// The runtime's `str_next_part`: the part of `s` from `from` to the next
    /// `separator`, or to the end, written into `cursor`, a copy here of
    /// what the runtime's view shows; where the next part starts, or -1 when
    /// this one is the last.
    /// The parts of `s` between each `separator`, as a new array of strings.
    fn split(&mut self, s: &[u8], separator: &[u8]) -> Value {
        let parts: Vec<Value> = crate::text::split(s, separator)
            .iter()
            .map(|p| self.string(p))
            .collect();
        self.allocated += 1;
        self.heap.insert(self.allocated, parts);
        self.deep.insert(self.allocated);
        Value::Handle(self.allocated)
    }

    fn next_part(
        &mut self,
        s: &[u8],
        separator: &[u8],
        cursor: Option<Value>,
        from: i32,
    ) -> Result<Value, Trap> {
        let Some(Value::Handle(cursor)) = cursor else {
            return Err(Trap::Unreachable);
        };
        let from = usize::try_from(from).map_err(|_| Trap::Unreachable)?;
        let rest = s.get(from..).ok_or(Trap::Unreachable)?;
        if separator.is_empty() {
            // One character: its lead byte and its continuation bytes.
            let mut end = from + 1;
            while end < s.len() && crate::text::continues(s, end) {
                end += 1;
            }
            let bytes = s
                .get(from..end)
                .unwrap_or_default()
                .iter()
                .map(|&b| Value::I32(i32::from(b)))
                .collect();
            self.heap.insert(cursor, bytes);
            let next = if end < s.len() {
                i32::try_from(end).unwrap_or(-1)
            } else {
                -1
            };
            return Ok(Value::I32(next));
        }
        let at = rest.windows(separator.len()).position(|w| w == separator);
        let (part, next) = match at {
            Some(i) => (
                rest.get(..i).unwrap_or_default(),
                i32::try_from(from + i + separator.len()).unwrap_or(-1),
            ),
            None => (rest, -1),
        };
        let bytes = part.iter().map(|&b| Value::I32(i32::from(b))).collect();
        self.heap.insert(cursor, bytes);
        Ok(Value::I32(next))
    }

    /// A new string of these bytes.
    fn string(&mut self, bytes: &[u8]) -> Value {
        self.allocated += 1;
        let elements = bytes.iter().map(|&b| Value::I32(i32::from(b))).collect();
        self.heap.insert(self.allocated, elements);
        Value::Handle(self.allocated)
    }

    /// The entry of `wanted` in map `table`, made when it is not there. The
    /// key given is the map's when the entry is new, released otherwise, and
    /// the buffers the old value owned are released, as the runtime does.
    fn map_put(&mut self, table: u32, wanted: MapKey, handle: Option<u32>) -> Result<usize, Trap> {
        let map = self.maps.get_mut(&table).ok_or(Trap::UseAfterRelease)?;
        let width = map.width;
        let found = map
            .entries
            .iter()
            .position(|e| e.as_ref().is_some_and(|e| e.key == wanted));
        if let Some(at) = found {
            let old = map
                .entries
                .get_mut(at)
                .and_then(Option::as_mut)
                .map(|e| std::mem::replace(&mut e.slots, vec![Value::I64(0); width]))
                .unwrap_or_default();
            self.free_slots(old)?;
            if let Some(h) = handle {
                self.free(h)?;
            }
            return Ok(at);
        }
        map.entries.push(Some(Entry {
            key: wanted,
            handle,
            slots: vec![Value::I64(0); width],
        }));
        let at = map.entries.len() - 1;
        self.heap
            .get_mut(&table)
            .ok_or(Trap::UseAfterRelease)?
            .push(Value::I32(0));
        Ok(at)
    }

    /// [`crate::RuntimeFn::BufEqual`]: the same elements, by `==` on each, a
    /// handle's buffer compared one level down (decision 0021). The shape the
    /// runtime is given is not needed: the interpreter's values carry theirs.
    fn buf_equal(&mut self, a: Value, b: Value) -> Result<bool, Trap> {
        let xs = self.elements(a)?.clone();
        let ys = self.elements(b)?.clone();
        if xs.len() != ys.len() {
            return Ok(false);
        }
        for (x, y) in xs.into_iter().zip(ys) {
            let same = match (x, y) {
                (Value::Handle(_), Value::Handle(_)) => self.buf_equal(x, y)?,
                (Value::F32(p), Value::F32(q)) => p == q,
                (Value::F64(p), Value::F64(q)) => p == q,
                _ => x == y,
            };
            if !same {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `set` of a key given, or lent.
    fn put_given(
        &mut self,
        table: u32,
        wanted: MapKey,
        given: Value,
        lent: bool,
    ) -> Result<usize, Trap> {
        if lent {
            return self.map_put_lent(table, wanted, given);
        }
        let handle = match given {
            Value::Handle(h) => Some(h),
            _ => None,
        };
        self.map_put(table, wanted, handle)
    }

    /// Traps unless `at` is the entry of `wanted`, or -1 when it is missing:
    /// the entry a lookup pass gave to a `set` (decision 0017).
    fn check_entry(&self, table: u32, wanted: &MapKey, at: Value) -> Result<(), Trap> {
        let Value::I32(at) = at else {
            return Err(Trap::Unreachable);
        };
        let map = self.maps.get(&table).ok_or(Trap::UseAfterRelease)?;
        let found = map
            .entries
            .iter()
            .position(|e| e.as_ref().is_some_and(|e| &e.key == wanted));
        if found.map_or(-1, |e| i32::try_from(e).unwrap_or(-1)) == at {
            Ok(())
        } else {
            Err(Trap::Unchecked)
        }
    }

    fn map_put_lent(&mut self, table: u32, wanted: MapKey, given: Value) -> Result<usize, Trap> {
        let there = self
            .maps
            .get(&table)
            .ok_or(Trap::UseAfterRelease)?
            .entries
            .iter()
            .any(|e| e.as_ref().is_some_and(|e| e.key == wanted));
        let handle = if there {
            None
        } else {
            match self.clone_buffer(given)? {
                Value::Handle(h) => Some(h),
                _ => return Err(Trap::Unreachable),
            }
        };
        self.map_put(table, wanted, handle)
    }

    /// The live entry `e` of map `table`.
    fn map_entry(&mut self, table: u32, e: usize) -> Result<&mut Entry, Trap> {
        self.maps
            .get_mut(&table)
            .ok_or(Trap::UseAfterRelease)?
            .entries
            .get_mut(e)
            .and_then(Option::as_mut)
            .ok_or(Trap::Unchecked)
    }

    /// A slot or the key of entry `e` of map `table`.
    fn map_read(
        &mut self,
        f: crate::RuntimeFn,
        table: u32,
        e: usize,
        slot: usize,
    ) -> Result<Value, Trap> {
        use crate::RuntimeFn as F;
        let live = self.map_entry(table, e)?;
        Ok(match (f, &live.key) {
            (F::MapKeyI, MapKey::Int(n)) => Value::I64(*n),
            (F::MapKeyS, MapKey::Text(_)) => Value::Handle(live.handle.ok_or(Trap::Unreachable)?),
            (F::MapValue | F::MapValueF | F::MapValueH, _) => {
                live.slots.get(slot).copied().ok_or(Trap::Unchecked)?
            }
            _ => return Err(Trap::Unreachable),
        })
    }

    /// A new, empty map whose values have the slots `shape` says.
    fn map_new(&mut self, shape: i32) -> Value {
        self.allocated += 1;
        self.heap.insert(self.allocated, Vec::new());
        let width = usize::try_from(shape & 0xff).unwrap_or(0);
        self.maps.insert(
            self.allocated,
            Table {
                width,
                entries: Vec::new(),
            },
        );
        Value::Handle(self.allocated)
    }

    /// Removes `wanted` from map `table`, with what its entry owned: whether
    /// it was there.
    fn map_delete(&mut self, table: u32, wanted: &MapKey) -> Result<bool, Trap> {
        let map = self.maps.get_mut(&table).ok_or(Trap::UseAfterRelease)?;
        let found = map
            .entries
            .iter()
            .position(|e| e.as_ref().is_some_and(|e| e.key == *wanted));
        if let Some(e) = found {
            let gone = map.entries.get_mut(e).and_then(Option::take);
            self.free_entries(vec![gone])?;
            self.heap
                .get_mut(&table)
                .ok_or(Trap::UseAfterRelease)?
                .pop();
        }
        Ok(found.is_some())
    }

    /// The functions of the runtime on maps (decision 0005); `None` for
    /// another function. A string key given to the map is the map's: kept,
    /// or released when the key was there already, as the runtime does.
    fn map_runtime(
        &mut self,
        f: crate::RuntimeFn,
        operands: &[Value],
    ) -> Result<Option<Value>, Trap> {
        use crate::RuntimeFn as F;
        let operand = |i: usize| operands.get(i).copied().ok_or(Trap::Unreachable);
        let block = || match operands.first() {
            Some(Value::Handle(b)) => Ok(*b),
            _ => Err(Trap::UseAfterRelease),
        };
        let index = |i: usize| match operands.get(i) {
            Some(Value::I32(e)) => usize::try_from(*e).map_err(|_| Trap::Unchecked),
            _ => Err(Trap::Unreachable),
        };
        let key = |this: &mut Self, v: Value| match v {
            Value::I64(n) => Ok(MapKey::Int(n)),
            Value::Handle(_) => Ok(MapKey::Text(this.bytes(v)?)),
            _ => Err(Trap::Unreachable),
        };
        let entry =
            |found: Option<usize>| Value::I32(found.map_or(-1, |e| i32::try_from(e).unwrap_or(-1)));
        let value = match f {
            F::MapNew => {
                let Some(Value::I32(shape)) = operands.get(1) else {
                    return Err(Trap::Unreachable);
                };
                self.map_new(*shape)
            }
            F::MapFindI | F::MapFindS => {
                let wanted = key(self, operand(1)?)?;
                let map = self.maps.get(&block()?).ok_or(Trap::UseAfterRelease)?;
                entry(
                    map.entries
                        .iter()
                        .position(|e| e.as_ref().is_some_and(|e| e.key == wanted)),
                )
            }
            F::MapPutI | F::MapPutS | F::MapPutSLent => {
                let given = operand(1)?;
                let wanted = key(self, given)?;
                let lent = f == F::MapPutSLent;
                entry(Some(self.put_given(block()?, wanted, given, lent)?))
            }
            // The entry a lookup gave: checked to hold the key, so that a
            // lookup pass that proved wrong traps here (decision 0017).
            F::MapPutAtI | F::MapPutAtS | F::MapPutAtSLent => {
                let given = operand(2)?;
                let wanted = key(self, given)?;
                self.check_entry(block()?, &wanted, operand(1)?)?;
                let lent = f == F::MapPutAtSLent;
                entry(Some(self.put_given(block()?, wanted, given, lent)?))
            }
            F::MapStore | F::MapStoreF | F::MapStoreH => {
                let stored = operand(3)?;
                let slot = index(2)?;
                let live = self.map_entry(block()?, index(1)?)?;
                *live.slots.get_mut(slot).ok_or(Trap::Unchecked)? = stored;
                Value::Void
            }
            F::MapValue | F::MapValueF | F::MapValueH | F::MapKeyI | F::MapKeyS => {
                let slot = if matches!(f, F::MapKeyI | F::MapKeyS) {
                    0
                } else {
                    index(2)?
                };
                self.map_read(f, block()?, index(1)?, slot)?
            }
            F::MapNext => {
                let map = self.maps.get(&block()?).ok_or(Trap::UseAfterRelease)?;
                let from = index(1)?;
                entry(
                    map.entries
                        .iter()
                        .enumerate()
                        .skip(from)
                        .find(|(_, e)| e.is_some())
                        .map(|(i, _)| i),
                )
            }
            F::MapDeleteI | F::MapDeleteS => {
                let wanted = key(self, operand(1)?)?;
                Value::Bool(self.map_delete(block()?, &wanted)?)
            }
            F::MapClear => {
                let table = block()?;
                let map = self.maps.get_mut(&table).ok_or(Trap::UseAfterRelease)?;
                let gone = std::mem::take(&mut map.entries);
                self.free_entries(gone)?;
                self.heap.insert(table, Vec::new());
                Value::Void
            }
            _ => return Ok(None),
        };
        Ok(Some(value))
    }

    /// The functions of the runtime that write numbers and booleans as text.
    fn format_runtime(
        &mut self,
        f: crate::RuntimeFn,
        operands: &[Value],
    ) -> Result<Option<Value>, Trap> {
        use crate::RuntimeFn as F;
        Ok(Some(match (f, operands.first()) {
            (F::FmtInt, Some(Value::I64(n))) => self.string(n.to_string().as_bytes()),
            // A `u64`: the same bits, read as unsigned (decision 0024).
            #[allow(clippy::cast_sign_loss)]
            (F::FmtUint, Some(Value::I64(n))) => self.string((*n as u64).to_string().as_bytes()),
            #[allow(clippy::cast_sign_loss)]
            (F::FmtUintLen, Some(Value::I64(n))) => {
                Value::I32(i32::try_from((*n as u64).to_string().len()).unwrap_or(0))
            }
            (F::FmtUintAppend, Some(&buf)) => {
                let Some(Value::I64(n)) = operands.get(1) else {
                    return Err(Trap::Unreachable);
                };
                #[allow(clippy::cast_sign_loss)]
                let digits = (*n as u64).to_string();
                self.elements(buf)?
                    .extend(digits.bytes().map(|b| Value::I32(i32::from(b))));
                Value::Void
            }
            (F::FloatRem, Some(Value::F64(x))) => match operands.get(1) {
                Some(Value::F64(y)) => Value::F64(x % y),
                _ => return Err(Trap::Unreachable),
            },
            (F::FmtIntLen, Some(Value::I64(n))) => {
                Value::I32(i32::try_from(n.to_string().len()).unwrap_or(0))
            }
            (F::FmtIntAppend, Some(&buf)) => {
                let Some(Value::I64(n)) = operands.get(1) else {
                    return Err(Trap::Unreachable);
                };
                let digits = n.to_string();
                self.elements(buf)?
                    .extend(digits.bytes().map(|b| Value::I32(i32::from(b))));
                Value::Void
            }
            (F::FmtBool, Some(Value::Bool(b))) => self.string(b.to_string().as_bytes()),
            (F::FmtFloat, Some(Value::F64(x))) => {
                let text = float_mode(operands.get(1)).map(|m| crate::text::float_text(*x, m));
                self.string(text?.as_bytes())
            }
            (
                F::FmtInt
                | F::FmtIntLen
                | F::FmtIntAppend
                | F::FmtUint
                | F::FmtUintLen
                | F::FmtUintAppend
                | F::FmtBool
                | F::FmtFloat
                | F::FloatRem,
                _,
            ) => {
                return Err(Trap::Unreachable);
            }
            _ => return Ok(None),
        }))
    }

    /// A function of the runtime on strings.
    fn runtime(&mut self, f: crate::RuntimeFn, operands: &[Value]) -> Result<Value, Trap> {
        use crate::RuntimeFn as F;
        let int = |i: usize| -> Result<i32, Trap> {
            match operands.get(i) {
                Some(Value::I32(n)) => Ok(*n),
                _ => Err(Trap::Unreachable),
            }
        };
        let text = |this: &mut Self, i: usize| -> Result<Vec<u8>, Trap> {
            let value = *operands.get(i).ok_or(Trap::Unreachable)?;
            this.bytes(value)
        };
        if let F::SharedUnique | F::WeakNew | F::WeakAlive | F::WeakGet = f {
            return self.shared_runtime(f, operands);
        }
        if f == crate::RuntimeFn::BufEqual {
            let (Some(a), Some(b)) = (operands.first(), operands.get(1)) else {
                return Err(Trap::Unreachable);
            };
            return Ok(Value::Bool(self.buf_equal(*a, *b)?));
        }
        if let Some(value) = self.array_runtime(f, operands)? {
            return Ok(value);
        }
        if let Some(value) = self.map_runtime(f, operands)? {
            return Ok(value);
        }
        if let Some(value) = self.format_runtime(f, operands)? {
            return Ok(value);
        }
        let s = text(self, 0)?;
        Ok(match f {
            F::IndexOf => Value::I32(crate::text::index_of(&s, &text(self, 1)?, int(2)?)),
            F::StartsWith => Value::Bool(s.starts_with(&text(self, 1)?)),
            F::EndsWith => Value::Bool(s.ends_with(&text(self, 1)?)),
            F::Slice => {
                let part = crate::text::slice(&s, int(1)?, int(2)?);
                self.string(&part)
            }
            F::Trim => {
                let part = crate::text::trim(&s);
                self.string(&part)
            }
            F::Upper => self.string(&s.to_ascii_uppercase()),
            F::Lower => self.string(&s.to_ascii_lowercase()),
            F::Repeat => {
                let n = usize::try_from(int(1)?.max(0)).unwrap_or(0);
                self.string(&s.repeat(n))
            }
            F::Replace | F::ReplaceAll => {
                let (from, to) = (text(self, 1)?, text(self, 2)?);
                let out = crate::text::replace(&s, &from, &to, f == F::ReplaceAll);
                self.string(&out)
            }
            // Views are an encoding of the WebAssembly runtime: here, the same
            // parts as `split`.
            F::Split | F::SplitViews => {
                let separator = text(self, 1)?;
                self.split(&s, &separator)
            }
            F::StrCursor => self.string(&[]),
            F::NextPart | F::NextChar => {
                let separator = text(self, 1)?;
                self.next_part(&s, &separator, operands.get(2).copied(), int(3)?)?
            }
            F::ParseFloat => Value::F64(crate::text::parse_decimal(&s).unwrap_or(f64::NAN)),
            F::IsInt => Value::Bool(crate::text::parse_int(&s, int(1)?).is_some()),
            F::ParseInt => Value::I64(crate::text::parse_int(&s, 64).unwrap_or(0)),
            F::ArrSlice
            | F::ArrConcat
            | F::ArrReverse
            | F::ArrSort
            | F::ArrSortIndex
            | F::ArrPermute
            | F::ArrIndexOfStr
            | F::ArrKeep
            | F::FmtInt
            | F::FmtIntLen
            | F::FmtIntAppend
            | F::FmtUint
            | F::FmtUintLen
            | F::FmtUintAppend
            | F::FloatRem
            | F::FmtBool
            | F::FmtFloat => {
                return Err(Trap::Unreachable);
            }
            F::Join => {
                let separator = text(self, 1)?;
                let parts = self
                    .elements(*operands.first().ok_or(Trap::Unreachable)?)?
                    .clone();
                let mut out = Vec::new();
                for (i, part) in parts.into_iter().enumerate() {
                    if i > 0 {
                        out.extend_from_slice(&separator);
                    }
                    out.extend(self.bytes(part)?);
                }
                self.string(&out)
            }
            // The maps' functions, answered by `map_runtime`.
            _ => return Err(Trap::Unreachable),
        })
    }

    /// `arr_sort_index` and `arr_permute`.
    fn sorting(&mut self, f: crate::RuntimeFn, operands: &[Value]) -> Result<Value, Trap> {
        let operand = |i: usize| operands.get(i).copied().ok_or(Trap::Unreachable);
        if f == crate::RuntimeFn::ArrPermute {
            return self.permute(operand(0)?, operand(1)?);
        }
        let desc = operand(2)? != Value::I32(0);
        self.sort_index(operand(0)?, desc, unsigned_kind(operands.get(1)))
    }

    /// `arr_sort_index`: the order of `sortBy`, as the runtime's: numbers as
    /// `sort()` orders them, every NaN last; strings by their bytes, UTF-8,
    /// the order of their code points. Stable; `desc` reverses the order of
    /// the keys, not that of equal ones.
    fn sort_index(&mut self, keys: Value, desc: bool, unsigned: bool) -> Result<Value, Trap> {
        let keys = self.elements(keys)?.clone();
        let mut texts = Vec::new();
        for key in &keys {
            texts.push(match key {
                Value::Handle(_) => Some(self.bytes(*key)?),
                _ => None,
            });
        }
        let number = |v: &Value| match v {
            Value::F32(x) if x.is_nan() => Value::F32(f32::NAN),
            Value::F64(x) if x.is_nan() => Value::F64(f64::NAN),
            other => *other,
        };
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_by(|&i, &j| {
            let o = match (keys.get(i).map(number), keys.get(j).map(number)) {
                (Some(x), Some(y))
                    if !matches!(x, Value::Handle(_)) && !matches!(y, Value::Handle(_)) =>
                {
                    compare_numbers(&x, &y, unsigned)
                }
                _ => texts.get(i).cmp(&texts.get(j)),
            };
            if desc { o.reverse() } else { o }
        });
        let indices = order
            .into_iter()
            .map(|i| Value::I32(i32::try_from(i).unwrap_or(i32::MAX)))
            .collect();
        self.allocated += 1;
        self.heap.insert(self.allocated, indices);
        Ok(Value::Handle(self.allocated))
    }

    /// `arr_permute`: puts the element at `order[i]` at `i`.
    fn permute(&mut self, column: Value, order: Value) -> Result<Value, Trap> {
        let order = self.elements(order)?.clone();
        let elements = self.elements(column)?;
        let before = elements.clone();
        for (slot, at) in elements.iter_mut().zip(order) {
            let Value::I32(at) = at else {
                return Err(Trap::Unreachable);
            };
            *slot = *usize::try_from(at)
                .ok()
                .and_then(|a| before.get(a))
                .ok_or(Trap::Unreachable)?;
        }
        Ok(Value::Void)
    }

    /// The functions of the runtime on columns of arrays; `None` for the
    /// others.
    fn array_runtime(
        &mut self,
        f: crate::RuntimeFn,
        operands: &[Value],
    ) -> Result<Option<Value>, Trap> {
        use crate::RuntimeFn as F;
        let first = *operands.first().ok_or(Trap::Unreachable)?;
        let int = |i: usize| match operands.get(i) {
            Some(Value::I32(n)) => Ok(*n),
            _ => Err(Trap::Unreachable),
        };
        let deep = matches!(first, Value::Handle(b) if self.deep.contains(&b));
        let column = |this: &mut Self, elements: Vec<Value>| -> Result<Value, Trap> {
            let mut elements = elements;
            if deep {
                for element in &mut elements {
                    *element = this.clone_buffer(*element)?;
                }
            }
            this.allocated += 1;
            this.heap.insert(this.allocated, elements);
            if deep {
                this.deep.insert(this.allocated);
            }
            Ok(Value::Handle(this.allocated))
        };
        let clamp = |len: usize, i: i32| -> usize {
            let len = i64::try_from(len).unwrap_or(i64::MAX);
            let i = if i < 0 {
                i64::from(i) + len
            } else {
                i64::from(i)
            };
            usize::try_from(i.clamp(0, len)).unwrap_or(0)
        };
        Ok(Some(match f {
            F::ArrSlice => {
                let elements = self.elements(first)?.clone();
                let a = clamp(elements.len(), int(1)?);
                let b = clamp(elements.len(), int(2)?).max(a);
                let part = elements.get(a..b).unwrap_or_default().to_vec();
                column(self, part)?
            }
            F::ArrConcat => {
                let mut elements = self.elements(first)?.clone();
                let other = *operands.get(1).ok_or(Trap::Unreachable)?;
                elements.extend(self.elements(other)?.clone());
                column(self, elements)?
            }
            F::ArrReverse => {
                self.elements(first)?.reverse();
                Value::Void
            }
            F::ArrSortIndex | F::ArrPermute => self.sorting(f, operands)?,
            F::ArrKeep => {
                let flags = self
                    .elements(*operands.get(1).ok_or(Trap::Unreachable)?)?
                    .clone();
                let elements = std::mem::take(self.elements(first)?);
                let mut kept = Vec::with_capacity(elements.len());
                for (element, flag) in elements
                    .into_iter()
                    .zip(flags.iter().chain(std::iter::repeat(&Value::Bool(false))))
                {
                    if *flag == Value::Bool(true) {
                        kept.push(element);
                    } else if let (true, Value::Handle(block)) = (deep, element) {
                        self.free(block)?;
                    }
                }
                *self.elements(first)? = kept;
                Value::Void
            }
            F::ArrSort => {
                let unsigned = unsigned_kind(operands.get(1));
                let elements = self.elements(first)?;
                // JavaScript's order for typed arrays, as the runtime's: -0
                // before +0, every NaN last, as the canonical NaN.
                for v in elements.iter_mut() {
                    match v {
                        Value::F32(x) if x.is_nan() => *x = f32::NAN,
                        Value::F64(x) if x.is_nan() => *x = f64::NAN,
                        _ => {}
                    }
                }
                elements.sort_by(|a, b| compare_numbers(a, b, unsigned));
                Value::Void
            }
            F::ArrIndexOfStr => {
                let needle = self.bytes(*operands.get(1).ok_or(Trap::Unreachable)?)?;
                let strings = self.elements(first)?.clone();
                let mut found = -1;
                for (i, s) in strings.into_iter().enumerate() {
                    if self.bytes(s)? == needle {
                        found = i32::try_from(i).unwrap_or(-1);
                        break;
                    }
                }
                Value::I32(found)
            }
            _ => return Ok(None),
        }))
    }

    /// Releases a buffer, and the buffers it owns.
    fn free(&mut self, block: u32) -> Result<(), Trap> {
        // A static environment is never freed; a counted one, by the last
        // who holds it (decision 0026).
        if is_static_env(block) {
            return Ok(());
        }
        if block & STATIC != 0 {
            return Err(Trap::DoubleRelease);
        }
        // A weak holder: the value's block goes with the last, once no one
        // holds the value itself.
        if block & WEAK != 0 {
            let shared = block & !WEAK;
            let left = self.weak.get_mut(&shared).ok_or(Trap::DoubleRelease)?;
            *left -= 1;
            if *left == 0 {
                self.weak.remove(&shared);
                if !self.counts.contains_key(&shared) {
                    self.heap.remove(&shared);
                }
            }
            return Ok(());
        }
        if let Some(count) = self.counts.get_mut(&block) {
            *count -= 1;
            if *count > 0 {
                return Ok(());
            }
            self.counts.remove(&block);
            // The buffers the environment owns, its first slots, go with it.
            let elements = self.heap.remove(&block).ok_or(Trap::DoubleRelease)?;
            if self.weak.contains_key(&block) {
                self.heap.insert(block, Vec::new());
            }
            let owned = match elements.get(1) {
                Some(Value::I32(n)) => usize::try_from(*n).unwrap_or(0),
                _ => 0,
            };
            for slot in elements.iter().skip(2).take(owned) {
                if let Value::Handle(h) = slot {
                    self.free(*h)?;
                }
            }
            return Ok(());
        }
        let elements = self.heap.remove(&block).ok_or(Trap::DoubleRelease)?;
        if let Some(map) = self.maps.remove(&block) {
            self.free_entries(map.entries)?;
        }
        if self.deep.remove(&block) {
            for element in elements {
                if let Value::Handle(inner) = element {
                    self.free(inner)?;
                }
            }
        }
        Ok(())
    }

    /// Releases what the entries of a map owned: their string keys, and the
    /// buffers of their values.
    fn free_entries(&mut self, entries: Vec<Option<Entry>>) -> Result<(), Trap> {
        for e in entries.into_iter().flatten() {
            if let Some(h) = e.handle {
                self.free(h)?;
            }
            self.free_slots(e.slots)?;
        }
        Ok(())
    }

    /// Releases the buffers among the slots of a value.
    fn free_slots(&mut self, slots: Vec<Value>) -> Result<(), Trap> {
        for slot in slots {
            if let Value::Handle(h) = slot {
                self.free(h)?;
            }
        }
        Ok(())
    }

    /// The functions of the runtime on shared values and their weak holders
    /// (decision 0032, prototypes).
    fn shared_runtime(&mut self, f: crate::RuntimeFn, operands: &[Value]) -> Result<Value, Trap> {
        use crate::RuntimeFn as F;
        if f == F::SharedUnique {
            return self.shared_unique(operands.get(1).copied());
        }
        let Some(Value::Handle(block)) = operands.first() else {
            return Err(Trap::UseAfterRelease);
        };
        let shared = block & !WEAK;
        let alive = self.counts.contains_key(&shared);
        Ok(match f {
            F::WeakNew => {
                *self.weak.entry(shared).or_insert(0) += 1;
                Value::Handle(shared | WEAK)
            }
            F::WeakAlive => Value::Bool(alive),
            _ if alive => self.clone_buffer(Value::Handle(shared))?,
            _ => self.static_env(ZERO_ENV, Value::I32(-1)),
        })
    }

    /// [`crate::RuntimeFn::SharedUnique`]: a shared value only this holder
    /// holds, its own block or a copy (decision 0032, prototype r1).
    fn shared_unique(&mut self, shared: Option<Value>) -> Result<Value, Trap> {
        let Some(Value::Handle(block)) = shared else {
            return Err(Trap::UseAfterRelease);
        };
        let count = self
            .counts
            .get(&block)
            .copied()
            .ok_or(Trap::UseAfterRelease)?;
        if count == 1 {
            return Ok(Value::Handle(block));
        }
        let mut elements = self.elements(Value::Handle(block))?.clone();
        let owned = match elements.get(1) {
            Some(Value::I32(n)) => usize::try_from(*n).unwrap_or(0),
            _ => 0,
        };
        for slot in elements.iter_mut().skip(2).take(owned) {
            *slot = self.clone_buffer(*slot)?;
        }
        self.free(block)?;
        self.allocated += 1;
        self.heap.insert(self.allocated, elements);
        self.counts.insert(self.allocated, 1);
        Ok(Value::Handle(self.allocated))
    }

    /// A copy of a buffer, with copies of the buffers it owns.
    fn clone_buffer(&mut self, buf: Value) -> Result<Value, Trap> {
        if let Value::Handle(block) = buf
            && block & WEAK != 0
            && block & STATIC == 0
        {
            *self
                .weak
                .get_mut(&(block & !WEAK))
                .ok_or(Trap::UseAfterRelease)? += 1;
            return Ok(buf);
        }
        let mut elements = self.elements(buf)?.clone();
        let Value::Handle(block) = buf else {
            return Err(Trap::UseAfterRelease);
        };
        // An environment is shared: one more holds it (decision 0026).
        if is_static_env(block) {
            return Ok(buf);
        }
        if let Some(count) = self.counts.get_mut(&block) {
            *count += 1;
            return Ok(buf);
        }
        if self.maps.contains_key(&block) {
            return self.clone_map(block, elements);
        }
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

    /// A copy of map `block`, with copies of its string keys and of the
    /// buffers its values own.
    fn clone_map(&mut self, block: u32, elements: Vec<Value>) -> Result<Value, Trap> {
        let (width, live) = {
            let map = self.maps.get(&block).ok_or(Trap::UseAfterRelease)?;
            (map.width, map.entries.clone())
        };
        let mut entries = Vec::with_capacity(live.len());
        for entry in live {
            entries.push(match entry {
                Some(Entry { key, handle, slots }) => {
                    let handle = match handle {
                        Some(h) => match self.clone_buffer(Value::Handle(h))? {
                            Value::Handle(copy) => Some(copy),
                            _ => return Err(Trap::Unreachable),
                        },
                        None => None,
                    };
                    let mut copied = Vec::with_capacity(slots.len());
                    for slot in slots {
                        copied.push(match slot {
                            Value::Handle(_) => self.clone_buffer(slot)?,
                            other => other,
                        });
                    }
                    Some(Entry {
                        key,
                        handle,
                        slots: copied,
                    })
                }
                None => None,
            });
        }
        self.allocated += 1;
        self.heap.insert(self.allocated, elements);
        self.maps.insert(self.allocated, Table { width, entries });
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
            (BufStmt::CheckOverflow, [Value::Bool(fits)]) => {
                if !fits {
                    return Err(Trap::Overflow);
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

/// Whether the `kind` of a runtime sort says unsigned integers
/// (`Elem::sort_kind`).
fn unsigned_kind(kind: Option<&Value>) -> bool {
    matches!(kind, Some(Value::I32(k)) if k & 4 != 0)
}

/// Two numbers in the order of the runtime's sorts: unsigned integers by
/// their bits read unsigned, floats with every NaN last.
#[allow(clippy::cast_sign_loss)]
fn compare_numbers(a: &Value, b: &Value, unsigned: bool) -> std::cmp::Ordering {
    match (a, b) {
        (Value::I32(x), Value::I32(y)) if unsigned => (*x as u32).cmp(&(*y as u32)),
        (Value::I64(x), Value::I64(y)) if unsigned => (*x as u64).cmp(&(*y as u64)),
        (Value::I32(x), Value::I32(y)) => x.cmp(y),
        (Value::I64(x), Value::I64(y)) => x.cmp(y),
        (Value::F32(x), Value::F32(y)) => x.total_cmp(y),
        (Value::F64(x), Value::F64(y)) => x.total_cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

/// A value as a buffer stores it: a byte keeps its low 8 bits, extended as
/// its type reads it, and two bytes their low 16.
const fn stored(elem: Elem, value: Value) -> Value {
    match (elem, value) {
        (Elem::U8, Value::I32(v)) => Value::I32(v & 0xFF),
        (Elem::U16, Value::I32(v)) => Value::I32(v & 0xFFFF),
        #[allow(clippy::cast_possible_truncation)]
        (Elem::I8, Value::I32(v)) => Value::I32(v as i8 as i32),
        #[allow(clippy::cast_possible_truncation)]
        (Elem::I16, Value::I32(v)) => Value::I32(v as i16 as i32),
        _ => value,
    }
}

/// The results of a call, each into its target.
fn store(locals: &mut [Value], targets: &[crate::LocalId], results: Vec<Value>) {
    for (target, value) in targets.iter().zip(results) {
        if let Some(slot) = slot(locals, *target) {
            *slot = value;
        }
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
        (UnOp::Bits, Value::F64(x)) => Value::I64(x.to_bits().cast_signed()),
        (UnOp::Bits, Value::I64(x)) => Value::F64(f64::from_bits(x.cast_unsigned())),
        (UnOp::Bits, Value::F32(x)) => Value::I32(x.to_bits().cast_signed()),
        (UnOp::Bits, Value::I32(x)) => Value::F32(f32::from_bits(x.cast_unsigned())),
        (_, other) => other,
    }
}

macro_rules! int_op {
    ($op:expr, $a:expr, $b:expr, $wrap:ident, $u:ty) => {{
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
            // Unsigned: the same bits, read as unsigned (decision 0024).
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
            BinOp::DivU | BinOp::RemU => {
                if b == 0 {
                    return Err(Trap::Arithmetic);
                }
                let (x, y) = (a as $u, b as $u);
                (if $op == BinOp::DivU { x / y } else { x % y }) as _
            }
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
            BinOp::ShrU => (a as $u).wrapping_shr(b as u32) as _,
            #[allow(clippy::cast_sign_loss)]
            BinOp::LtU => return Ok(Value::Bool((a as $u) < (b as $u))),
            #[allow(clippy::cast_sign_loss)]
            BinOp::LeU => return Ok(Value::Bool((a as $u) <= (b as $u))),
            #[allow(clippy::cast_sign_loss)]
            BinOp::GtU => return Ok(Value::Bool((a as $u) > (b as $u))),
            #[allow(clippy::cast_sign_loss)]
            BinOp::GeU => return Ok(Value::Bool((a as $u) >= (b as $u))),
            BinOp::And | BinOp::Or | BinOp::Pow | BinOp::Atan2 => {
                return Err(Trap::Unreachable);
            }
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
            #[allow(clippy::cast_possible_truncation)]
            BinOp::Pow => $wrap((f64::from(a).powf(f64::from(b))) as _),
            #[allow(clippy::cast_possible_truncation)]
            BinOp::Atan2 => $wrap((f64::from(a).atan2(f64::from(b))) as _),
            _ => return Err(Trap::Unreachable),
        })
    }};
}

fn binary(op: BinOp, a: Value, b: Value) -> Result<Value, Trap> {
    use Value::{Bool, F32, F64, I32, I64};
    match (a, b) {
        (I32(a), I32(b)) => int_op!(op, a, b, I32, u32),
        (I64(a), I64(b)) => int_op!(op, a, b, I64, u64),
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
        // A `bool` in an environment's slot (decision 0026).
        (Value::Bool(x), Ty::I64) => Value::I64(i64::from(x)),
        (Value::I64(x), Ty::Bool) => Value::Bool(x & 1 != 0),
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

/// [`UnOp::ConvertU`]: an unsigned integer to a float or widened with
/// zeros, or a float to an unsigned integer, saturating, `NaN` to 0, as
/// WebAssembly's `trunc_sat_u` does (decision 0024).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn convert_u(v: Value, to: Ty) -> Value {
    match (v, to) {
        (Value::I32(x), Ty::I64) => Value::I64(i64::from(x as u32)),
        (Value::I32(x), Ty::F32) => Value::F32(x as u32 as f32),
        (Value::I32(x), Ty::F64) => Value::F64(f64::from(x as u32)),
        (Value::I64(x), Ty::F32) => Value::F32(x as u64 as f32),
        (Value::I64(x), Ty::F64) => Value::F64(x as u64 as f64),
        (Value::I64(x), Ty::I32) => Value::I32(x as i32),
        (Value::F32(x), Ty::I32) => Value::I32(x as u32 as i32),
        (Value::F64(x), Ty::I32) => Value::I32(x as u32 as i32),
        (Value::F32(x), Ty::I64) => Value::I64(x as u64 as i64),
        (Value::F64(x), Ty::I64) => Value::I64(x as u64 as i64),
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
        // The functions of `Math`, on `f64`, an `f32` widened then narrowed.
        #[allow(clippy::cast_possible_truncation)]
        (op, Value::F32(x)) => match math(op, Value::F64(f64::from(x))) {
            Value::F64(y) => Value::F32(y as f32),
            other => other,
        },
        (MathOp::Exp, Value::F64(x)) => Value::F64(x.exp()),
        (MathOp::Log, Value::F64(x)) => Value::F64(x.ln()),
        (MathOp::Sin, Value::F64(x)) => Value::F64(x.sin()),
        (MathOp::Cos, Value::F64(x)) => Value::F64(x.cos()),
        (MathOp::Tan, Value::F64(x)) => Value::F64(x.tan()),
        (MathOp::Round, Value::F64(x)) => Value::F64(js_round(x)),
        (_, other) => other,
    }
}

/// `Math.round`: the nearest integer, halves toward positive infinity.
fn js_round(x: f64) -> f64 {
    let down = x.floor();
    if x - down >= 0.5 { down + 1.0 } else { down }
}

/// The mode of `fmt_float`: 0, 1, or `2 + d` for `toFixed(d)`, whose `d` is
/// from 0 to 100, as JavaScript's.
fn float_mode(operand: Option<&Value>) -> Result<i32, Trap> {
    let mode = match operand {
        Some(Value::I32(m)) => *m,
        _ => 0,
    };
    if (0..=102).contains(&mode) {
        Ok(mode)
    } else {
        Err(Trap::Unreachable)
    }
}
