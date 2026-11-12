//! Maps and sets ([decision 0005]): one handle, owned as a string is, whose
//! table the runtime keeps. A set is a map without values.
//!
//! The runtime hashes keys of two kinds: integers, as an `i64`, and strings,
//! by their bytes. An `i32` or a `bool` key is widened to an `i64`.
//!
//! A value is kept as its scalars, one slot each, as an array of structures
//! keeps one column per scalar: an integer or a boolean widened to an `i64`,
//! a float as an `f64`, and a string or an array as its handle, which the
//! map then owns. The map is made with its shape, the number of slots and
//! those that own a buffer, so that it releases them when a key is set
//! again, deleted, cleared, or the map released. Values are narrowed back
//! when read: an `f32` written then read is the same `f32`.
//!
//! `m.get(k)` gives a copy of a value that owns buffers; `for…of` lends them.
//!
//! [decision 0005]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0005-maps-and-sets.md

use perasc_check::ty::Ty as CheckTy;
use perasc_diag::Span;
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, Ident, Stmt as AstStmt};

use super::heap::{Mode, release_stmt};
use super::{FnState, Lowering};
use crate::{BinOp, BufOp, Const, Elem, Expr, ExprKind, RuntimeFn as F, Stmt, Ty};

/// The most value slots a map has: one bit each in its shape.
const MOST_SLOTS: usize = 24;

fn int(value: i64, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(value)),
        ty,
    }
}

fn runtime(f: F, operands: Vec<Expr>) -> Expr {
    Expr {
        kind: ExprKind::Buf(BufOp::Runtime(f), operands),
        ty: f.result(),
    }
}

fn convert(e: Expr, ty: Ty) -> Expr {
    if e.ty == ty {
        return e;
    }
    Expr {
        kind: ExprKind::Convert(Box::new(e)),
        ty,
    }
}

/// A scalar as the map keeps it: an `i64`, an `f64`, or a handle. A `bool`
/// becomes 1 or 0.
fn widen(state: &mut FnState, e: Expr) -> Expr {
    match e.ty {
        Ty::Bool => {
            let t = state.temp(Ty::I64);
            state.pre.push(Stmt::Set(t, int(0, Ty::I64)));
            state.pre.push(Stmt::If {
                cond: e,
                then: vec![Stmt::Set(t, int(1, Ty::I64))],
                otherwise: Vec::new(),
            });
            super::local_expr(state, t)
        }
        Ty::F32 | Ty::F64 => convert(e, Ty::F64),
        Ty::Owned | Ty::Borrowed => e,
        _ => convert(e, Ty::I64),
    }
}

/// A scalar read from the map, back to its type.
fn narrow(raw: Expr, ty: Ty) -> Expr {
    match ty {
        Ty::Bool => super::binary(BinOp::Ne, raw, int(0, Ty::I64)),
        Ty::F32 | Ty::I32 => convert(raw, ty),
        _ => raw,
    }
}

/// Reads slot `slot` of entry `entry`: a buffer lent, a scalar narrowed.
fn read(table: &Expr, entry: &Expr, slot: usize, ty: Ty) -> Expr {
    let slot = int(i64::try_from(slot).unwrap_or(0), Ty::I32);
    let operands = vec![table.clone(), entry.clone(), slot];
    match ty {
        Ty::Owned => runtime(F::MapValueH, operands),
        Ty::F32 | Ty::F64 => narrow(runtime(F::MapValueF, operands), ty),
        _ => narrow(runtime(F::MapValue, operands), ty),
    }
}

/// The function that stores a slot of this type.
const fn store(ty: Ty) -> F {
    match ty {
        Ty::Owned => F::MapStoreH,
        Ty::F32 | Ty::F64 => F::MapStoreF,
        _ => F::MapStore,
    }
}

/// What lowering a map needs of its type: whether its keys are strings, the
/// type of its key's scalar, and its value's scalars, none for a set.
struct Table {
    strings: bool,
    key: Ty,
    slots: Vec<Ty>,
    set: bool,
}

impl Table {
    /// Its shape: the number of slots, and above it a bit per owning slot.
    fn shape(&self) -> i64 {
        let owning = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == Ty::Owned)
            .fold(0_i64, |bits, (i, _)| bits | (1 << i));
        i64::try_from(self.slots.len()).unwrap_or(0) | (owning << 8)
    }

    /// The function for an integer key, or the one for a string key.
    const fn pick(&self, int_key: F, string_key: F) -> F {
        if self.strings { string_key } else { int_key }
    }
}

impl Lowering<'_> {
    /// Whether an expression is a map or a set.
    pub(super) fn is_map(&self, id: ExprId) -> bool {
        matches!(
            self.check_type(id),
            Some(CheckTy::Map(..) | CheckTy::Set(_))
        )
    }

    /// The table of a map or a set's type, or the feature not compiled yet.
    fn table(&self, ty: &CheckTy) -> Result<Table, &'static str> {
        let (key, value, set) = match ty {
            CheckTy::Map(key, value) => (key, Some(value), false),
            CheckTy::Set(key) => (key, None, true),
            _ => return Err("map-value"),
        };
        let key_ty = self.leaves(key)?.first().map_or(Ty::I64, |(_, t)| *t);
        let slots: Vec<Ty> = match value {
            Some(value) => self.leaves(value)?.into_iter().map(|(_, t)| t).collect(),
            None => Vec::new(),
        };
        if slots.len() > MOST_SLOTS {
            return Err("map-value");
        }
        Ok(Table {
            strings: **key == CheckTy::String,
            key: key_ty,
            slots,
            set,
        })
    }

    /// The table of an expression's type, or `E0900` reported at `span`.
    fn table_of(&mut self, id: ExprId, span: Span) -> Option<Table> {
        let ty = self.check_type(id)?;
        match self.table(&ty) {
            Ok(table) => Some(table),
            Err(feature) => {
                self.unsupported(span, feature);
                None
            }
        }
    }

    /// The shape the runtime makes a map of type `ty` with.
    pub(super) fn map_slots(&self, ty: &CheckTy) -> i64 {
        self.table(ty).map_or(0, |t| t.shape())
    }

    /// A new, empty map of type `ty`.
    pub(super) fn empty_map(&self, ty: &CheckTy) -> Option<Expr> {
        let table = self.table(ty).ok()?;
        // 1: string keys; 2: keys of 32 bits, kept in the index (decision
        // 0018); 0: other integers.
        let keys = if table.strings {
            1
        } else if table.key == Ty::I32 {
            2
        } else {
            0
        };
        Some(runtime(
            F::MapNew,
            vec![int(keys, Ty::I32), int(table.shape(), Ty::I32)],
        ))
    }

    /// `Map.new()` or `Set.new()`, if `call` is one: a new, empty table.
    pub(super) fn map_new(
        &mut self,
        state: &mut FnState,
        call: ExprId,
        base: ExprId,
        method: &str,
        mode: Mode,
    ) -> Option<Vec<Expr>> {
        let AstExpr::Name(symbol) = self.ast.expr(base).kind else {
            return None;
        };
        if method != "new" || !matches!(self.ast.name(symbol), "Map" | "Set") {
            return None;
        }
        let span = self.ast.expr(call).span;
        self.table_of(call, span)?;
        let ty = self.check_type(call)?;
        let empty = self.empty_map(&ty)?;
        Some(Self::fresh(state, vec![empty], mode))
    }

    /// `m.size`: how many keys the table holds.
    pub(super) fn map_size(&mut self, state: &mut FnState, base: ExprId) -> Expr {
        match self.value_as(state, base, Mode::Borrow).into_iter().next() {
            Some(table) => Expr {
                kind: ExprKind::Buf(BufOp::Len, vec![table]),
                ty: Ty::I32,
            },
            None => int(0, Ty::I32),
        }
    }

    /// The string `x` of an argument written `x.copy()`.
    fn copied_string(&self, arg: ExprId) -> Option<ExprId> {
        let AstExpr::Call { callee, args, .. } = &self.ast.expr(arg).kind else {
            return None;
        };
        let AstExpr::Field { base, name } = &self.ast.expr(*callee).kind else {
            return None;
        };
        (args.is_empty()
            && self.ast.name(name.symbol) == "copy"
            && self.check_type(*base) == Some(CheckTy::String))
        .then_some(*base)
    }

    /// A key, as the runtime takes it: an `i64`, or the string, lent or given.
    fn map_key(&mut self, state: &mut FnState, arg: ExprId, table: &Table, given: bool) -> Expr {
        if table.strings {
            let mode = if given { Mode::Consume } else { Mode::Borrow };
            return self
                .value_as(state, arg, mode)
                .into_iter()
                .next()
                .unwrap_or_else(|| int(0, Ty::I32));
        }
        let e = self.expr(state, arg);
        widen(state, e)
    }

    /// A method of a map or a set: `set`, `add`, `get`, `has`, `delete`,
    /// `clear`. Its scalars: none for a statement, a `bool`, or those of an
    /// option for `get`.
    pub(super) fn map_method(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        method: &str,
        args: &[ExprId],
        mode: Mode,
        span: Span,
    ) -> Vec<Expr> {
        let Some(table) = self.table_of(base, span) else {
            return Vec::new();
        };
        let Some(handle) = self.value_as(state, base, Mode::Borrow).into_iter().next() else {
            return Vec::new();
        };
        let mark = state.pre.len();
        match (method, args) {
            ("set" | "add", [k, rest @ ..]) => {
                // `m.set(word.copy(), …)`: the word lent, copied by the map
                // only when it is a new key.
                let copied = self.copied_string(*k).filter(|_| table.strings);
                let k = match copied {
                    Some(text) => self.map_key(state, text, &table, false),
                    None => self.map_key(state, *k, &table, true),
                };
                // The table, then the key, then the value: each evaluated
                // before what the next one needs to run.
                let mut operands = [handle];
                state.keep_before(mark, &mut operands);
                let [handle] = operands;
                let mark = state.pre.len();
                let values: Vec<Expr> = match rest.first() {
                    Some(&v) => self
                        .value(state, v)
                        .into_iter()
                        .map(|e| widen(state, e))
                        .collect(),
                    None => Vec::new(),
                };
                let mut operands = [handle, k];
                state.keep_before(mark, &mut operands);
                let [handle, k] = operands;
                // The value is computed before `put` releases the old one,
                // which it may read: `m.set(k, m.get(k).unwrapOr(0) + 1)`.
                let values: Vec<Expr> = values
                    .into_iter()
                    .map(|v| if v.ty == Ty::Owned { v } else { state.spill(v) })
                    .collect();
                let f = if copied.is_some() {
                    F::MapPutSLent
                } else {
                    table.pick(F::MapPutI, F::MapPutS)
                };
                let put = runtime(f, vec![handle.clone(), k]);
                if values.is_empty() {
                    state.pre.push(Stmt::Expr(put));
                    return Vec::new();
                }
                let entry = state.temp(Ty::I32);
                state.pre.push(Stmt::Set(entry, put));
                let entry = super::local_expr(state, entry);
                for (slot, (value, &ty)) in values.into_iter().zip(&table.slots).enumerate() {
                    let slot = int(i64::try_from(slot).unwrap_or(0), Ty::I32);
                    state.pre.push(Stmt::Expr(runtime(
                        store(ty),
                        vec![handle.clone(), entry.clone(), slot, value],
                    )));
                }
                Vec::new()
            }
            ("has" | "get" | "delete", [k]) => {
                let k = self.map_key(state, *k, &table, false);
                let mut operands = [handle];
                state.keep_before(mark, &mut operands);
                let [handle] = operands;
                if method == "delete" {
                    let f = table.pick(F::MapDeleteI, F::MapDeleteS);
                    return vec![runtime(f, vec![handle, k])];
                }
                let found = runtime(
                    table.pick(F::MapFindI, F::MapFindS),
                    vec![handle.clone(), k],
                );
                if method == "has" {
                    return vec![super::binary(BinOp::Ge, found, int(0, Ty::I32))];
                }
                Self::map_get(state, &handle, found, &table.slots, mode)
            }
            ("clear", []) => {
                state
                    .pre
                    .push(Stmt::Expr(runtime(F::MapClear, vec![handle])));
                Vec::new()
            }
            // A map of its own: keys and values copied.
            ("copy", []) => {
                let copy = Expr {
                    kind: ExprKind::Buf(BufOp::Clone, vec![handle]),
                    ty: Ty::Owned,
                };
                Self::fresh(state, vec![copy], mode)
            }
            _ => {
                self.unsupported(span, "method");
                Vec::new()
            }
        }
    }

    /// `m.get(k).unwrapOr(fallback)`, when the option is only a step: the
    /// entry looked up, then, if the fallback is quiet, each scalar of its
    /// value read, a buffer copied once, only when it is there, and the
    /// fallback made only when it is not. No option is made, and no empty
    /// buffer stands for a missing value (decision 0017, step 2). A fallback
    /// that is not quiet runs after the entry is read, as written. `None`
    /// when `base` is not a map's `get`.
    pub(super) fn map_get_or(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        args: &[ExprId],
        mode: Mode,
    ) -> Option<Vec<Expr>> {
        let AstExpr::Call {
            callee,
            args: get_args,
            ..
        } = &self.ast.expr(base).kind
        else {
            return None;
        };
        let AstExpr::Field { base: map, name } = &self.ast.expr(*callee).kind else {
            return None;
        };
        let ([k], true) = (
            get_args.as_slice(),
            self.ast.name(name.symbol) == "get" && self.is_map(*map),
        ) else {
            return None;
        };
        let (map, k) = (*map, *k);
        let span = self.ast.expr(base).span;
        let table = self.table_of(map, span)?;
        if table.slots.is_empty() {
            return None;
        }
        let handle = self.value_as(state, map, Mode::Borrow).into_iter().next()?;
        let mark = state.pre.len();
        let k = self.map_key(state, k, &table, false);
        let mut operands = [handle];
        state.keep_before(mark, &mut operands);
        let [handle] = operands;
        let found = runtime(
            table.pick(F::MapFindI, F::MapFindS),
            vec![handle.clone(), k],
        );
        let entry = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(entry, found));
        let entry = super::local_expr(state, entry);
        let some = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(
            some,
            super::binary(BinOp::Ge, entry.clone(), int(0, Ty::I32)),
        ));
        let some = super::local_expr(state, some);
        let mark = state.pre.len();
        let drops = state.drops.len();
        let fallback = self.arguments(state, None, args);
        if Self::quiet_since(state, mark, drops, &fallback) {
            let making = state.pre.split_off(mark);
            let mut fields = vec![some];
            fields.extend(
                table
                    .slots
                    .iter()
                    .enumerate()
                    .map(|(slot, &ty)| read(&handle, &entry, slot, ty)),
            );
            return Some(Self::unwrap_or_lazily(
                state, fields, fallback, making, false, mode,
            ));
        }
        // The value read before the fallback runs, as `get` then `unwrapOr`
        // would.
        let making = state.pre.split_off(mark);
        let fields = Self::map_get(state, &handle, entry, &table.slots, Mode::Borrow);
        let mark = state.pre.len();
        state.pre.extend(making);
        Some(Self::unwrap_or_eagerly(state, fields, fallback, mark, mode))
    }

    /// `m.get(k)`, the entry found: whether there is one, then each scalar
    /// of its value, or zero and empty buffers. A buffer is copied: the map
    /// keeps its own.
    fn map_get(
        state: &mut FnState,
        handle: &Expr,
        found: Expr,
        slots: &[Ty],
        mode: Mode,
    ) -> Vec<Expr> {
        let entry = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(entry, found));
        let entry = super::local_expr(state, entry);
        let some = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(
            some,
            super::binary(BinOp::Ge, entry.clone(), int(0, Ty::I32)),
        ));
        let mut out = vec![super::local_expr(state, some)];
        let mut then = Vec::new();
        for (slot, &ty) in slots.iter().enumerate() {
            let read = read(handle, &entry, slot, ty);
            if ty == Ty::Owned {
                let value = state.owned_temp(Expr {
                    kind: ExprKind::Buf(BufOp::New(Elem::U8), vec![int(0, Ty::I32)]),
                    ty: Ty::Owned,
                });
                then.push(release_stmt(value));
                then.push(Stmt::Set(
                    value,
                    Expr {
                        kind: ExprKind::Buf(BufOp::Clone, vec![read]),
                        ty: Ty::Owned,
                    },
                ));
                out.push(Expr {
                    kind: ExprKind::Local(value),
                    ty: Ty::Owned,
                });
            } else {
                let value = state.temp(ty);
                state.pre.push(Stmt::Set(value, super::zero(ty)));
                then.push(Stmt::Set(value, read));
                out.push(super::local_expr(state, value));
            }
        }
        if !then.is_empty() {
            state.pre.push(Stmt::If {
                cond: super::local_expr(state, some),
                then,
                otherwise: Vec::new(),
            });
        }
        Self::fresh(state, out, mode)
    }

    /// `for (const entry of m)`: the entries in insertion order, from the
    /// first live one, each lent to the body as `(key, value)`, or as its key
    /// for a set. The checker refuses a body that changes the map, so the
    /// entries stay where they are while it runs.
    pub(super) fn map_for_of(
        &mut self,
        state: &mut FnState,
        name: &Ident,
        iterable: ExprId,
        body: &AstStmt,
        out: &mut Vec<Stmt>,
    ) {
        let span = self.ast.expr(iterable).span;
        let Some(table) = self.table_of(iterable, span) else {
            return;
        };
        let Some(handle) = self
            .value_as(state, iterable, Mode::Borrow)
            .into_iter()
            .next()
        else {
            return;
        };
        let e = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(
            e,
            runtime(F::MapNext, vec![handle.clone(), int(0, Ty::I32)]),
        ));
        state.flush(out);
        let entry = super::local_expr(state, e);
        state.open();
        // A temporary map, `for (const k of make())`, lives through the loop.
        let temporaries = std::mem::take(&mut state.drops);
        if let Some(scope) = state.owned.last_mut() {
            scope.extend(temporaries);
        }
        let operands = vec![handle.clone(), entry.clone()];
        let key = if table.strings {
            runtime(F::MapKeyS, operands)
        } else {
            narrow(runtime(F::MapKeyI, operands), table.key)
        };
        let mut values = vec![key];
        values.extend(
            table
                .slots
                .iter()
                .enumerate()
                .map(|(slot, &ty)| read(&handle, &entry, slot, ty)),
        );
        let mut turn = Vec::new();
        if values.iter().any(|v| v.ty == Ty::Borrowed) {
            // A buffer is read where it is used: a lent buffer cannot sit in
            // a local.
            if let Some(scope) = state.scopes.last_mut() {
                scope.insert(name.symbol, super::Binding::Lent(values));
            }
        } else {
            let text = self.ast.name(name.symbol);
            let locals = if table.set {
                vec![state.add(name.symbol, text, table.key)]
            } else {
                let leaves: Vec<(String, Ty)> = values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i.to_string(), v.ty))
                    .collect();
                state.add_struct(name.symbol, text, &leaves)
            };
            turn = locals
                .into_iter()
                .zip(values)
                .map(|(x, v)| Stmt::Set(x, v))
                .collect();
        }
        let cond = super::binary(BinOp::Ge, entry.clone(), int(0, Ty::I32));
        let after = super::binary(BinOp::Add, entry, int(1, Ty::I32));
        let step = vec![Stmt::Set(e, runtime(F::MapNext, vec![handle, after]))];
        let lowered = self.loop_body(state, Some(cond), Vec::new(), body, step, &mut turn);
        out.extend(lowered);
        state.close(out);
        state.end_statement(out);
    }
}
