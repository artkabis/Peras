//! Maps and sets ([decision 0005]): one handle, owned as a string is, whose
//! table the runtime keeps. A set is a map whose values are not read.
//!
//! The runtime hashes keys of two kinds: integers, as an `i64`, and strings,
//! by their bytes. An `i32` or a `bool` key is widened to an `i64`. Values
//! are kept as an `i64` or an `f64`, and narrowed back when read: an `f32`
//! value written then read is the same `f32`.
//!
//! [decision 0005]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0005-maps-and-sets.md

use perasc_check::ty::{FloatTy, IntTy, Ty as CheckTy};
use perasc_diag::Span;
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, Ident, Stmt as AstStmt};

use super::heap::Mode;
use super::{FnState, Lowering};
use crate::{BinOp, BufOp, Const, Expr, ExprKind, RuntimeFn as F, Stmt, Ty};

/// How a map keeps a value: as an `i64` or as an `f64`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kept {
    Int,
    Float,
}

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

/// A `bool` as the `i64` 1 or 0.
fn widen_bool(state: &mut FnState, b: Expr) -> Expr {
    let t = state.temp(Ty::I64);
    state.pre.push(Stmt::Set(t, int(0, Ty::I64)));
    state.pre.push(Stmt::If {
        cond: b,
        then: vec![Stmt::Set(t, int(1, Ty::I64))],
        otherwise: Vec::new(),
    });
    super::local_expr(state, t)
}

/// A scalar of a key or a value as the map keeps it.
fn widen(state: &mut FnState, e: Expr, ty: &CheckTy) -> Expr {
    match ty {
        CheckTy::Bool => widen_bool(state, e),
        CheckTy::Float(_) => convert(e, Ty::F64),
        _ => convert(e, Ty::I64),
    }
}

/// A value read from the map, back to its type.
fn narrow(raw: Expr, ty: &CheckTy) -> Expr {
    match ty {
        CheckTy::Bool => super::binary(BinOp::Ne, raw, int(0, Ty::I64)),
        CheckTy::Float(FloatTy::F32) => convert(raw, Ty::F32),
        CheckTy::Int(IntTy::I32) => convert(raw, Ty::I32),
        _ => raw,
    }
}

/// How a map keeps values of this type, if it can yet.
const fn kept(ty: &CheckTy) -> Option<Kept> {
    match ty {
        CheckTy::Int(IntTy::I32 | IntTy::I64) | CheckTy::Bool => Some(Kept::Int),
        CheckTy::Float(_) => Some(Kept::Float),
        _ => None,
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

    /// The key and value types of a map or a set; a set's values are `bool`s
    /// nobody reads.
    fn map_types(&self, id: ExprId) -> Option<(CheckTy, CheckTy)> {
        match self.check_type(id)? {
            CheckTy::Map(key, value) => Some((*key, *value)),
            CheckTy::Set(key) => Some((*key, CheckTy::Bool)),
            _ => None,
        }
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
        let (key, _) = self.map_types(call)?;
        let strings = i64::from(key == CheckTy::String);
        let table = runtime(F::MapNew, vec![int(strings, Ty::I32)]);
        Some(Self::fresh(state, vec![table], mode))
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

    /// A key, as the runtime takes it: an `i64`, or the string, lent or given.
    fn map_key(&mut self, state: &mut FnState, arg: ExprId, key: &CheckTy, given: bool) -> Expr {
        if *key == CheckTy::String {
            let mode = if given { Mode::Consume } else { Mode::Borrow };
            return self
                .value_as(state, arg, mode)
                .into_iter()
                .next()
                .unwrap_or_else(|| int(0, Ty::I32));
        }
        let e = self.expr(state, arg);
        widen(state, e, key)
    }

    /// A method of a map or a set: `set`, `add`, `get`, `has`, `delete`,
    /// `clear`. Its scalars: none for a statement, one, or two for `get`.
    pub(super) fn map_method(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        method: &str,
        args: &[ExprId],
        span: Span,
    ) -> Vec<Expr> {
        let Some((key, value)) = self.map_types(base) else {
            return Vec::new();
        };
        let Some(keep) = kept(&value) else {
            self.unsupported(span, "map-value");
            return Vec::new();
        };
        let Some(table) = self.value_as(state, base, Mode::Borrow).into_iter().next() else {
            return Vec::new();
        };
        let strings = key == CheckTy::String;
        let pick = |int_key: F, string_key: F| if strings { string_key } else { int_key };
        let mark = state.pre.len();
        match (method, args) {
            ("set" | "add", [k, rest @ ..]) => {
                let k = self.map_key(state, *k, &key, true);
                // The table first, then the key, then the value: each one
                // evaluated before what the next one needs to run.
                let mut operands = [table];
                state.keep_before(mark, &mut operands);
                let [table] = operands;
                let mark = state.pre.len();
                let v = match rest.first() {
                    Some(&v) => {
                        let e = self.expr(state, v);
                        widen(state, e, &value)
                    }
                    // A set's value: present.
                    None => int(1, Ty::I64),
                };
                let mut operands = [table, k];
                state.keep_before(mark, &mut operands);
                let [table, k] = operands;
                let f = match keep {
                    Kept::Int => pick(F::MapPutI, F::MapPutS),
                    Kept::Float => pick(F::MapPutIF, F::MapPutSF),
                };
                state.pre.push(Stmt::Expr(runtime(f, vec![table, k, v])));
                Vec::new()
            }
            ("has" | "get" | "delete", [k]) => {
                let k = self.map_key(state, *k, &key, false);
                let mut operands = [table];
                state.keep_before(mark, &mut operands);
                let [table] = operands;
                if method == "delete" {
                    let f = pick(F::MapDeleteI, F::MapDeleteS);
                    return vec![runtime(f, vec![table, k])];
                }
                let found = runtime(pick(F::MapFindI, F::MapFindS), vec![table.clone(), k]);
                if method == "has" {
                    return vec![super::binary(BinOp::Ge, found, int(0, Ty::I32))];
                }
                Self::map_get(state, table, found, &value, keep)
            }
            ("clear", []) => {
                state
                    .pre
                    .push(Stmt::Expr(runtime(F::MapClear, vec![table])));
                Vec::new()
            }
            _ => {
                self.unsupported(span, "method");
                Vec::new()
            }
        }
    }

    /// `m.get(k)`, the entry found: whether there is one, and its value, or
    /// zero.
    fn map_get(
        state: &mut FnState,
        table: Expr,
        found: Expr,
        value: &CheckTy,
        keep: Kept,
    ) -> Vec<Expr> {
        let entry = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(entry, found));
        let entry = super::local_expr(state, entry);
        let some = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(
            some,
            super::binary(BinOp::Ge, entry.clone(), int(0, Ty::I32)),
        ));
        let ty = Self::ir_type(value).unwrap_or(Ty::I64);
        let out = state.temp(ty);
        state.pre.push(Stmt::Set(out, super::zero(ty)));
        let f = match keep {
            Kept::Int => F::MapValue,
            Kept::Float => F::MapValueF,
        };
        let raw = runtime(f, vec![table, entry]);
        state.pre.push(Stmt::If {
            cond: super::local_expr(state, some),
            then: vec![Stmt::Set(out, narrow(raw, value))],
            otherwise: Vec::new(),
        });
        vec![
            super::local_expr(state, some),
            super::local_expr(state, out),
        ]
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
        let Some((key, value)) = self.map_types(iterable) else {
            return;
        };
        let set = matches!(self.check_type(iterable), Some(CheckTy::Set(_)));
        let Some(keep) = kept(&value) else {
            self.unsupported(span, "map-value");
            return;
        };
        let Some(table) = self
            .value_as(state, iterable, Mode::Borrow)
            .into_iter()
            .next()
        else {
            return;
        };
        let e = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(
            e,
            runtime(F::MapNext, vec![table.clone(), int(0, Ty::I32)]),
        ));
        state.flush(out);
        let entry = super::local_expr(state, e);
        state.open();
        // A temporary map, `for (const k of make())`, lives through the loop.
        let temporaries = std::mem::take(&mut state.drops);
        if let Some(scope) = state.owned.last_mut() {
            scope.extend(temporaries);
        }
        let operands = vec![table.clone(), entry.clone()];
        let key_value = if key == CheckTy::String {
            runtime(F::MapKeyS, operands.clone())
        } else {
            narrow(runtime(F::MapKeyI, operands.clone()), &key)
        };
        let mut values = vec![key_value];
        if !set {
            let f = match keep {
                Kept::Int => F::MapValue,
                Kept::Float => F::MapValueF,
            };
            values.push(narrow(runtime(f, operands), &value));
        }
        let mut turn = Vec::new();
        if key == CheckTy::String {
            // A string key is read where it is used: a lent buffer cannot sit
            // in a local.
            if let Some(scope) = state.scopes.last_mut() {
                scope.insert(name.symbol, super::Binding::Lent(values));
            }
        } else {
            let text = self.ast.name(name.symbol);
            let locals = if set {
                vec![state.add(name.symbol, text, values.first().map_or(Ty::I64, |v| v.ty))]
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
        let step = vec![Stmt::Set(e, runtime(F::MapNext, vec![table, after]))];
        let lowered = self.loop_body(state, Some(cond), Vec::new(), body, step, &mut turn);
        out.extend(lowered);
        state.close(out);
        state.end_statement(out);
    }
}
