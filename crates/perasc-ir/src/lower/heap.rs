//! Arrays, on the heap: ownership, releases, and the operations on buffers.
//!
//! The checker decided where a value moves ([`Analysis::moves`]); lowering
//! turns that into `move` and `borrow`, and inserts every release, so that the
//! release verifier can prove each buffer is released exactly once:
//!
//! - an `owned` local is **live** while it holds a buffer. Lowering follows it
//!   on every path, as the verifier does;
//! - at the end of a scope, its live locals are released; before `return`, all
//!   of them; before `break` and `continue`, those of the scopes left;
//! - where paths meet, after an `if` or a loop, a local live on one path and
//!   not on another (moved on one side only) is released on the path that
//!   still holds it, so that it is held on all of them or on none;
//! - a new buffer used only for a statement, `make().length`, goes to a
//!   temporary released at the end of that statement.
//!
//! Every index is checked against the length before an element is read or
//! written: `check_index` stops the program with a `RangeError`.
//!
//! [`Analysis::moves`]: perasc_check::Analysis::moves

use std::collections::BTreeSet;

use perasc_check::ty::Ty as CheckTy;
use perasc_diag::Span;
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr};

use super::{FnState, Lowering};
use crate::opt::is_simple;
use crate::{BinOp, BufOp, BufStmt, Const, Elem, Expr, ExprKind, LocalId, Stmt, Ty, UnOp};

/// How a value is used: taken by its new owner, or only read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// An assignment, a `return`, a `sink` argument…: buffers move.
    Consume,
    /// Anything else: buffers are lent.
    Borrow,
}

/// What a loop being lowered needs to place its releases.
pub(super) struct LoopFrame {
    /// How many scopes are open around the loop.
    pub(super) depth: usize,
    /// The locals live when the loop starts, and at the start of each turn.
    pub(super) head: BTreeSet<u32>,
    /// The locals live at each `break`, in the order they are lowered.
    pub(super) breaks: Vec<BTreeSet<u32>>,
}

impl FnState {
    /// Whether a local holds buffers: an `owned` local.
    pub(super) fn is_owned(&self, local: LocalId) -> bool {
        usize::try_from(local.0)
            .ok()
            .and_then(|i| self.locals.get(i))
            .is_some_and(|l| l.ty == Ty::Owned)
    }

    /// Whether a local lends a buffer someone else owns.
    pub(super) fn is_borrowed(&self, local: LocalId) -> bool {
        usize::try_from(local.0)
            .ok()
            .and_then(|i| self.locals.get(i))
            .is_some_and(|l| l.ty == Ty::Borrowed)
    }

    /// The `inout` numbers, as the function gives them back when it returns.
    pub(super) fn writeback_values(&self) -> Vec<Expr> {
        self.writeback
            .iter()
            .map(|&l| super::local_expr(self, l))
            .collect()
    }

    /// The releases a `return` from here would need, without changing what
    /// is live: for an exit that only some paths take, such as `?`.
    pub(super) fn exit_releases(&self) -> Vec<Stmt> {
        self.live
            .iter()
            .map(|&l| release_stmt(LocalId(l)))
            .collect()
    }

    /// Releases a live local; it is empty afterwards.
    pub(super) fn release(&mut self, local: LocalId) -> Stmt {
        self.live.remove(&local.0);
        Stmt::Buf(
            BufStmt::Release,
            vec![Expr {
                kind: ExprKind::Move(local),
                ty: Ty::Owned,
            }],
        )
    }

    /// Releases the live locals of the scopes from `depth` on, innermost first.
    pub(super) fn release_scopes(&mut self, depth: usize, out: &mut Vec<Stmt>) {
        let locals: Vec<LocalId> = self
            .owned
            .iter()
            .skip(depth)
            .rev()
            .flat_map(|scope| scope.iter().rev().copied())
            .collect();
        for local in locals {
            if self.live.contains(&local.0) {
                out.push(self.release(local));
            }
        }
    }

    /// Releases every live local in `locals`, in order.
    pub(super) fn release_all(&mut self, locals: &BTreeSet<u32>, out: &mut Vec<Stmt>) {
        for &local in locals {
            if self.live.contains(&local) {
                out.push(self.release(LocalId(local)));
            }
        }
    }

    /// Releases the temporaries of the statement just lowered.
    pub(super) fn end_statement(&mut self, out: &mut Vec<Stmt>) {
        for local in std::mem::take(&mut self.drops) {
            if self.live.contains(&local.0) {
                out.push(self.release(local));
            }
        }
    }

    /// A new `owned` temporary, filled now with `value`.
    pub(super) fn owned_temp(&mut self, value: Expr) -> LocalId {
        let temp = self.temp(Ty::Owned);
        self.pre.push(Stmt::Set(temp, value));
        self.live.insert(temp.0);
        temp
    }

    /// The values the statement computes, made safe to keep across the release
    /// of its temporaries: each one that reads memory is computed first.
    pub(super) fn settle(&mut self, values: Vec<Expr>) -> Vec<Expr> {
        if self.drops.is_empty() {
            return values;
        }
        let values = values
            .into_iter()
            .map(|value| match value.kind {
                ExprKind::Const(_) => value,
                _ if value.ty == Ty::Owned => {
                    let temp = self.owned_temp(value);
                    moved(self, temp)
                }
                _ => {
                    let temp = self.temp(value.ty);
                    self.pre.push(Stmt::Set(temp, value));
                    super::local_expr(self, temp)
                }
            })
            .collect();
        let mut released = Vec::new();
        self.end_statement(&mut released);
        self.pre.extend(released);
        values
    }
}

/// `move local`, which empties it.
pub(super) fn moved(state: &mut FnState, local: LocalId) -> Expr {
    state.live.remove(&local.0);
    Expr {
        kind: ExprKind::Move(local),
        ty: Ty::Owned,
    }
}

/// `borrow local`, for an `owned` local.
pub(super) const fn borrowed(local: LocalId) -> Expr {
    Expr {
        kind: ExprKind::Borrow(local),
        ty: Ty::Borrowed,
    }
}

/// An expression that cannot trap, call, or change anything: evaluating it
/// earlier or later, once or twice, gives the same value.
pub(super) fn pure(e: &Expr) -> bool {
    is_simple(e)
        || matches!(e.kind, ExprKind::Borrow(_) | ExprKind::Move(_))
        || e.ty == Ty::Borrowed
}

fn int(value: i64) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(value)),
        ty: Ty::I32,
    }
}

fn len(buffer: &Expr) -> Expr {
    Expr {
        kind: ExprKind::Buf(BufOp::Len, vec![buffer.clone()]),
        ty: Ty::I32,
    }
}

fn get(buffer: &Expr, index: Expr, elem: Elem) -> Expr {
    Expr {
        kind: ExprKind::Buf(BufOp::Get(elem), vec![buffer.clone(), index]),
        ty: elem.ty(),
    }
}

fn new_buffer(capacity: Expr, elem: Elem) -> Expr {
    Expr {
        kind: ExprKind::Buf(BufOp::New(elem), vec![capacity]),
        ty: Ty::Owned,
    }
}

impl Lowering<'_> {
    /// The numbers an `inout` parameter gives back, by the types of its scalars.
    pub(super) fn inout_numbers(&self, decl: &perasc_syntax::ast::FnDecl) -> Vec<Ty> {
        decl.params
            .iter()
            .filter(|p| p.convention == perasc_syntax::ast::Convention::Inout)
            .filter_map(|p| self.resolve(&p.ty).ok())
            .filter_map(|ty| self.leaves(&ty).ok())
            .flatten()
            .map(|(_, ty)| ty)
            .filter(|ty| !ty.is_handle())
            .collect()
    }

    /// Whether a type holds a string, which JavaScript cannot change in place.
    pub(super) fn holds_string(&self, ty: &CheckTy) -> bool {
        match ty {
            CheckTy::String => true,
            CheckTy::Struct(id, _) => self
                .analysis
                .structs
                .get(id.0)
                .is_some_and(|(_, fields)| fields.iter().any(|(_, f)| self.holds_string(f))),
            _ => false,
        }
    }

    /// Whether a type is a buffer: an array or a string.
    pub(super) fn is_buffer(&self, id: ExprId) -> bool {
        matches!(
            self.check_type(id),
            Some(CheckTy::Array(_) | CheckTy::String)
        )
    }

    /// A string literal: its bytes, stored once in the module.
    pub(super) fn string_literal(
        &mut self,
        state: &mut FnState,
        text: &str,
        mode: Mode,
    ) -> Vec<Expr> {
        let bytes = text.as_bytes().to_vec();
        let index = self
            .data
            .iter()
            .position(|d| *d == bytes)
            .unwrap_or_else(|| {
                self.data.push(bytes);
                self.data.len() - 1
            });
        let literal = Expr {
            kind: ExprKind::Buf(
                BufOp::Literal(u32::try_from(index).unwrap_or(u32::MAX)),
                Vec::new(),
            ),
            ty: Ty::Owned,
        };
        Self::fresh(state, vec![literal], mode)
    }

    /// `a + b` on strings: a new buffer with room for both, then each appended.
    pub(super) fn concat(
        &mut self,
        state: &mut FnState,
        lhs: ExprId,
        rhs: ExprId,
        mode: Mode,
    ) -> Vec<Expr> {
        let Some(a) = self.buffer(state, lhs) else {
            return Vec::new();
        };
        let Some(b) = self.buffer(state, rhs) else {
            return Vec::new();
        };
        let room = super::binary(BinOp::Add, len(&a), len(&b));
        let joined = state.owned_temp(new_buffer(room, Elem::U8));
        state.pre.push(Stmt::Buf(
            BufStmt::Append(Elem::U8),
            vec![borrowed(joined), a],
        ));
        state.pre.push(Stmt::Buf(
            BufStmt::Append(Elem::U8),
            vec![borrowed(joined), b],
        ));
        let joined_value = Expr {
            kind: ExprKind::Local(joined),
            ty: Ty::Owned,
        };
        Self::fresh(state, vec![joined_value], mode)
    }

    /// `target += other` on a string: appended in place.
    pub(super) fn append(
        &mut self,
        state: &mut FnState,
        target: ExprId,
        other: ExprId,
        out: &mut Vec<Stmt>,
    ) {
        // The target is a place: the variable or field that holds the string.
        let Some(&local) = self.place(state, target).as_ref().and_then(|l| l.first()) else {
            return;
        };
        let buffer = if state.is_owned(local) {
            borrowed(local)
        } else {
            super::local_expr(state, local)
        };
        let Some(other) = self.buffer(state, other) else {
            return;
        };
        state
            .pre
            .push(Stmt::Buf(BufStmt::Append(Elem::U8), vec![buffer, other]));
        state.flush(out);
        state.end_statement(out);
    }

    /// `a == b` and `a != b` on strings and arrays: the same length, and the
    /// same elements in order, column by column.
    pub(super) fn buffer_eq(
        &mut self,
        state: &mut FnState,
        lhs: ExprId,
        rhs: ExprId,
        equal: bool,
        span: Span,
    ) -> Expr {
        let Some(columns) = self.columns_of(lhs, span) else {
            return int(0);
        };
        if columns.contains(&Elem::Handle) {
            self.unsupported(span, "deep-equality");
            return int(0);
        }
        let a = self.value_as(state, lhs, Mode::Borrow);
        let b = self.value_as(state, rhs, Mode::Borrow);
        let (Some(a0), Some(b0)) = (a.first(), b.first()) else {
            return int(0);
        };
        let n = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(n, len(a0)));
        let n = super::local_expr(state, n);
        let same = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(
            same,
            super::binary(BinOp::Eq, n.clone(), len(b0)),
        ));
        let same_expr = super::local_expr(state, same);
        let i = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(i, int(0)));
        let i_expr = super::local_expr(state, i);
        let element_equal = a
            .iter()
            .zip(&b)
            .zip(&columns)
            .map(|((x, y), &elem)| {
                super::binary(
                    BinOp::Eq,
                    get(x, i_expr.clone(), elem),
                    get(y, i_expr.clone(), elem),
                )
            })
            .reduce(|acc, e| super::binary(BinOp::And, acc, e))
            .unwrap_or(Expr {
                kind: ExprKind::Const(Const::Bool(true)),
                ty: Ty::Bool,
            });
        state.pre.push(Stmt::Loop {
            cond: Some(super::binary(
                BinOp::And,
                same_expr.clone(),
                super::binary(BinOp::Lt, i_expr.clone(), n),
            )),
            body: vec![Stmt::Set(same, element_equal)],
            step: vec![Stmt::Set(i, super::binary(BinOp::Add, i_expr, int(1)))],
        });
        if equal {
            same_expr
        } else {
            Expr {
                kind: ExprKind::Unary(UnOp::Not, Box::new(same_expr)),
                ty: Ty::Bool,
            }
        }
    }

    /// Values just made, `owned`: given as they are to a new owner, or kept in
    /// a temporary released after the statement when they are only read.
    pub(super) fn fresh(state: &mut FnState, values: Vec<Expr>, mode: Mode) -> Vec<Expr> {
        values
            .into_iter()
            .map(|value| {
                if value.ty != Ty::Owned {
                    return value;
                }
                // A temporary filled by `call many` holds it already.
                let temp = match value.kind {
                    ExprKind::Local(local) => {
                        state.live.insert(local.0);
                        local
                    }
                    _ => state.owned_temp(value),
                };
                match mode {
                    Mode::Consume => moved(state, temp),
                    Mode::Borrow => {
                        state.drops.push(temp);
                        borrowed(temp)
                    }
                }
            })
            .collect()
    }

    /// The buffer of a string, or of an array of scalars, lent.
    pub(super) fn buffer(&mut self, state: &mut FnState, id: ExprId) -> Option<Expr> {
        self.value_as(state, id, Mode::Borrow).into_iter().next()
    }

    /// The columns of an array type: the element type of each scalar of an
    /// element, in order. An array of structures keeps one buffer per field,
    /// so that every element operation is one per column.
    pub(super) fn columns(&self, ty: Option<&CheckTy>) -> Option<Vec<Elem>> {
        match ty {
            Some(CheckTy::String) => Some(vec![Elem::U8]),
            Some(CheckTy::Array(inner)) => self
                .leaves(inner)
                .ok()?
                .iter()
                .map(|(_, t)| Elem::of(*t))
                .collect(),
            _ => None,
        }
    }

    /// The columns of an array expression, or `E0900` if they are not compiled.
    fn columns_of(&mut self, id: ExprId, span: Span) -> Option<Vec<Elem>> {
        let columns = self
            .columns(self.check_type(id).as_ref())
            .filter(|c| !c.is_empty());
        if columns.is_none() {
            self.unsupported(span, "array-element");
        }
        columns
    }

    /// `[a, b, c]`: new buffers, one per column, with each element pushed.
    pub(super) fn array_literal(
        &mut self,
        state: &mut FnState,
        id: ExprId,
        items: &[ExprId],
    ) -> Vec<Expr> {
        let span = self.ast.expr(id).span;
        let Some(columns) = self.columns_of(id, span) else {
            return Vec::new();
        };
        let count = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let temps: Vec<LocalId> = columns
            .iter()
            .map(|&elem| state.owned_temp(new_buffer(int(count), elem)))
            .collect();
        for &item in items {
            let values = self.value(state, item);
            for ((&temp, &elem), value) in temps.iter().zip(&columns).zip(values) {
                state
                    .pre
                    .push(Stmt::Buf(BufStmt::Push(elem), vec![borrowed(temp), value]));
            }
        }
        temps
            .into_iter()
            .map(|temp| Expr {
                kind: ExprKind::Local(temp),
                ty: Ty::Owned,
            })
            .collect()
    }

    /// `xs.length`: the length of the first column; `m.size` for a map.
    pub(super) fn array_length(&mut self, state: &mut FnState, base: ExprId) -> Expr {
        if self.is_map(base) {
            return self.map_size(state, base);
        }
        // `xs.filter((x) => …).length`: the elements kept are counted, with
        // no array between. One function runs, as written, element by
        // element: the order of its calls is that of the program.
        if let Some((array, args)) = self.filter_call(base) {
            state.counted = Some(array);
            let span = self.ast.expr(base).span;
            let values = self.array_method(state, array, "filter", &args, Mode::Borrow, span);
            state.counted = None;
            return values.into_iter().next().unwrap_or_else(|| int(0));
        }
        // `text.split(" ").length`: read at once, views are enough.
        state.views_for = self.split_source(base);
        let buffer = self.buffer(state, base);
        state.views_for = None;
        match buffer {
            Some(buffer) => len(&buffer),
            None => int(0),
        }
    }

    /// `xs[i]`, checked: one scalar per column.
    pub(super) fn index_values(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        index: ExprId,
        span: Span,
    ) -> Vec<Expr> {
        let Some(columns) = self.columns_of(base, span) else {
            return Vec::new();
        };
        let buffers = self.value_as(state, base, Mode::Borrow);
        let Some(first) = buffers.first().cloned() else {
            return Vec::new();
        };
        let index = self.expr(state, index);
        let index = state.spill(index);
        state.pre.push(Stmt::Buf(
            BufStmt::CheckIndex,
            vec![index.clone(), len(&first)],
        ));
        buffers
            .iter()
            .zip(columns)
            .map(|(buffer, elem)| get(buffer, index.clone(), elem))
            .collect()
    }

    /// `xs[i]` of an array of scalars.
    pub(super) fn index_get(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        index: ExprId,
        span: Span,
    ) -> Expr {
        self.index_values(state, base, index, span)
            .into_iter()
            .next()
            .unwrap_or_else(|| int(0))
    }

    /// Whether an assignment target is an element, or a field of one:
    /// `xs[i]`, `points[i].x`.
    pub(super) fn is_element_place(&self, mut target: ExprId) -> bool {
        loop {
            match self.ast.expr(target).kind {
                AstExpr::Index { .. } => return true,
                AstExpr::Field { base, .. } | AstExpr::Paren(base) => target = base,
                _ => return false,
            }
        }
    }

    /// `xs[i] = v`, `xs[i] op= v`, `points[i].x = v`: the index and the value
    /// first, then the check, then a store into each column the place covers.
    pub(super) fn index_assign(
        &mut self,
        state: &mut FnState,
        target: ExprId,
        op: Option<BinOp>,
        value: Option<ExprId>,
        out: &mut Vec<Stmt>,
    ) {
        // The fields on the way from the element to the place, outermost last.
        let mut fields = Vec::new();
        let mut place = target;
        let (base, index) = loop {
            match self.ast.expr(place).kind {
                AstExpr::Index { base, index } => break (base, index),
                AstExpr::Field { base, name } => {
                    fields.push((base, self.ast.name(name.symbol)));
                    place = base;
                }
                AstExpr::Paren(inner) => place = inner,
                _ => return,
            }
        };
        let span = self.ast.expr(target).span;
        let Some(columns) = self.columns_of(base, span) else {
            return;
        };
        // Which columns the place covers.
        let (mut offset, mut count) = (0, columns.len());
        for (owner, name) in fields.iter().rev() {
            let Some(ty) = self.check_type(*owner) else {
                return;
            };
            let Some((at, len, _)) = self.field(&ty, name) else {
                return;
            };
            offset += at;
            count = len;
        }
        let buffers = self.value_as(state, base, Mode::Borrow);
        let index = self.expr(state, index);
        let index = state.spill(index);
        // `names[i] += s`: the string element grows in place.
        if let (Some(BinOp::Add), Some(&Elem::Handle), Some(value)) =
            (op, columns.get(offset), value)
        {
            let Some(other) = self.buffer(state, value) else {
                return;
            };
            let Some(buffer) = buffers.get(offset) else {
                return;
            };
            state.pre.push(Stmt::Buf(
                BufStmt::CheckIndex,
                vec![index.clone(), len(buffer)],
            ));
            let element = get(buffer, index, Elem::Handle);
            state
                .pre
                .push(Stmt::Buf(BufStmt::Append(Elem::U8), vec![element, other]));
            state.flush(out);
            state.end_statement(out);
            return;
        }
        let values: Vec<Expr> = match value {
            Some(value) => self.value(state, value),
            None => columns
                .get(offset)
                .map(|e| vec![super::one(e.ty())])
                .unwrap_or_default(),
        };
        let values: Vec<Expr> = values.into_iter().map(|v| state.spill(v)).collect();
        let Some(first) = buffers.first() else {
            return;
        };
        state.pre.push(Stmt::Buf(
            BufStmt::CheckIndex,
            vec![index.clone(), len(first)],
        ));
        let covered = buffers.iter().zip(&columns).skip(offset).take(count);
        for ((buffer, &elem), value) in covered.zip(values) {
            let value = match op {
                Some(op) => self.arith(state, op, get(buffer, index.clone(), elem), value),
                None => value,
            };
            state.pre.push(Stmt::Buf(
                BufStmt::Set(elem),
                vec![buffer.clone(), index.clone(), value],
            ));
        }
        state.flush(out);
        state.end_statement(out);
    }

    /// A method of an array, as scalars: `push` gives none, `pop` an option,
    /// `copy` a new array. Each works column by column.
    pub(super) fn array_method(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        method: &str,
        args: &[ExprId],
        mode: Mode,
        span: Span,
    ) -> Vec<Expr> {
        let views = Self::gives_views(state, base, method, mode);
        let Some(columns) = self.columns_of(base, span) else {
            return Vec::new();
        };
        let buffers = self.receiver(state, base, args);
        let Some(first) = buffers.first().cloned() else {
            return Vec::new();
        };
        if let Some(values) =
            self.function_method(state, base, method, args, mode, (&columns, &buffers))
        {
            return values;
        }
        let is_string = matches!(self.check_type(base), Some(CheckTy::String));
        if !is_string
            && let Some(values) =
                self.column_method(state, method, args, mode, (&columns, &buffers))
        {
            return values;
        }
        if is_string
            && let Some(values) =
                self.string_method(state, (base, &first), method, args, mode, views)
        {
            return values;
        }
        if let Some(values) = self.text_method(state, &first, method, args, mode, false) {
            return values;
        }
        match method {
            "push" => {
                let Some(&arg) = args.first() else {
                    return Vec::new();
                };
                let values = self.value(state, arg);
                for ((buffer, elem), value) in buffers.into_iter().zip(columns).zip(values) {
                    state
                        .pre
                        .push(Stmt::Buf(BufStmt::Push(elem), vec![buffer, value]));
                }
                Vec::new()
            }
            "pop" => Self::pop_values(state, &first, buffers, &columns, mode),
            "copy" => {
                // Each column copied by the runtime, with the buffers it owns.
                let copies = buffers
                    .into_iter()
                    .map(|buffer| Expr {
                        kind: ExprKind::Buf(BufOp::Clone, vec![buffer]),
                        ty: Ty::Owned,
                    })
                    .collect();
                Self::fresh(state, copies, mode)
            }
            _ => {
                self.unsupported(span, "method");
                Vec::new()
            }
        }
    }

    /// `for (const x of xs) body`: an index from 0 while it is below the length,
    /// read again at each turn, so that the body may change the array.
    pub(super) fn for_of(
        &mut self,
        state: &mut FnState,
        name: &perasc_syntax::ast::Ident,
        iterable: ExprId,
        body: &perasc_syntax::ast::Stmt,
        out: &mut Vec<Stmt>,
    ) {
        let span = self.ast.expr(iterable).span;
        if self.is_map(iterable) {
            self.map_for_of(state, name, iterable, body, out);
            return;
        }
        let Some(columns) = self.columns_of(iterable, span) else {
            return;
        };
        let element = match self.check_type(iterable) {
            Some(CheckTy::Array(inner)) => *inner,
            _ => CheckTy::Int(perasc_check::ty::IntTy::U8),
        };
        state.views_for = self.split_source(iterable);
        let buffers = self.value_as(state, iterable, Mode::Borrow);
        state.views_for = None;
        let buffers = self.unviewed(state, buffers, &[body.span]);
        let Some(first) = buffers.first().cloned() else {
            return;
        };
        let i = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(i, int(0)));
        state.flush(out);
        let i_expr = super::local_expr(state, i);
        state.open();
        // A temporary array, `for (const w of text.split(" "))`, belongs to
        // the loop's scope: it lives through every turn, which releases only
        // its own temporaries, and goes after the loop or on a `return`.
        let temporaries = std::mem::take(&mut state.drops);
        if let Some(scope) = state.owned.last_mut() {
            scope.extend(temporaries);
        }
        let text = self.ast.name(name.symbol);
        let mut turn: Vec<Stmt> = Vec::new();
        if columns.contains(&Elem::Handle) {
            // An element holding buffers is read where it is used: a lent
            // buffer cannot sit in a local.
            let values = buffers
                .iter()
                .zip(&columns)
                .map(|(buffer, &elem)| get(buffer, i_expr.clone(), elem))
                .collect();
            if let Some(scope) = state.scopes.last_mut() {
                scope.insert(name.symbol, super::Binding::Lent(values));
            }
        } else {
            let locals = if super::values::is_aggregate(&element) {
                let leaves = self.leaves(&element).unwrap_or_default();
                state.add_struct(name.symbol, text, &leaves)
            } else {
                let ty = columns.first().map_or(Ty::I32, |e| e.ty());
                vec![state.add(name.symbol, text, ty)]
            };
            turn = locals
                .iter()
                .zip(&buffers)
                .zip(&columns)
                .map(|((&x, buffer), &elem)| Stmt::Set(x, get(buffer, i_expr.clone(), elem)))
                .collect();
        }
        let cond = super::binary(BinOp::Lt, i_expr.clone(), len(&first));
        let step = vec![Stmt::Set(i, super::binary(BinOp::Add, i_expr, int(1)))];
        let lowered = self.loop_body(state, Some(cond), Vec::new(), body, step, &mut turn);
        out.extend(lowered);
        state.close(out);
        state.end_statement(out);
    }

    /// The receiver of a method, lent. The method reads it at once: a
    /// `split` there may give views, copied when an argument could change
    /// their text.
    fn receiver(&mut self, state: &mut FnState, base: ExprId, args: &[ExprId]) -> Vec<Expr> {
        state.views_for = self.split_source(base);
        let buffers = self.value_as(state, base, Mode::Borrow);
        state.views_for = None;
        let spans: Vec<Span> = args.iter().map(|&a| self.ast.expr(a).span).collect();
        self.unviewed(state, buffers, &spans)
    }

    /// `xs.pop()`: whether there was a last element, and the element, taken
    /// out of each column before the array is shortened.
    fn pop_values(
        state: &mut FnState,
        first: &Expr,
        buffers: Vec<Expr>,
        columns: &[Elem],
        mode: Mode,
    ) -> Vec<Expr> {
        // n = len; if (n > 0) { v = xs[n - 1]; truncate to n - 1 }
        let n = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(n, len(first)));
        let n = super::local_expr(state, n);
        let some = super::binary(BinOp::Gt, n.clone(), int(0));
        let last = super::binary(BinOp::Sub, n, int(1));
        let some_temp = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(some_temp, some));
        let mut out = vec![super::local_expr(state, some_temp)];
        let mut then = Vec::new();
        for (buffer, &elem) in buffers.iter().zip(columns) {
            if elem == Elem::Handle {
                // An empty buffer when there is none, else the element
                // itself, taken out before the array is shortened.
                let value = state.owned_temp(new_buffer(int(0), Elem::U8));
                then.push(release_stmt(value));
                then.push(Stmt::Set(
                    value,
                    Expr {
                        kind: ExprKind::Buf(BufOp::Take, vec![buffer.clone(), last.clone()]),
                        ty: Ty::Owned,
                    },
                ));
                out.push(Expr {
                    kind: ExprKind::Local(value),
                    ty: Ty::Owned,
                });
                continue;
            }
            let value = state.temp(elem.ty());
            state.pre.push(Stmt::Set(value, super::zero(elem.ty())));
            then.push(Stmt::Set(value, get(buffer, last.clone(), elem)));
            out.push(super::local_expr(state, value));
        }
        for buffer in buffers {
            then.push(Stmt::Buf(BufStmt::Truncate, vec![buffer, last.clone()]));
        }
        state.pre.push(Stmt::If {
            cond: super::local_expr(state, some_temp),
            then,
            otherwise: Vec::new(),
        });
        Self::fresh(state, out, mode)
    }

    /// A method of a string: an integer read from it, or a function of the
    /// runtime, `split` giving views when `views` holds.
    fn string_method(
        &mut self,
        state: &mut FnState,
        text: (ExprId, &Expr),
        method: &str,
        args: &[ExprId],
        mode: Mode,
        views: bool,
    ) -> Option<Vec<Expr>> {
        let (id, buffer) = text;
        if matches!(method, "parseI32" | "parseI64") {
            return Some(Self::parse_int(state, buffer.clone(), method == "parseI32"));
        }
        let values = self.text_method(state, buffer, method, args, mode, views)?;
        if views {
            self.note_views(state, &values, id);
        }
        Some(values)
    }

    /// Whether this `split` of `text` gives views: the array it makes is
    /// read at once, by the consumer that asked (`views_for`).
    fn gives_views(state: &mut FnState, text: ExprId, method: &str, mode: Mode) -> bool {
        let asked = state.views_for.take() == Some(text);
        asked && method == "split" && (mode == Mode::Borrow || state.views_owned)
    }

    /// Notes the temporary `split` just filled with views of `text`.
    fn note_views(&self, state: &mut FnState, values: &[Expr], text: ExprId) {
        if let Some(ExprKind::Borrow(t)) = values.first().map(|v| &v.kind) {
            let source = self.root(text);
            state.views.insert(*t, source);
        }
    }

    /// The array `id` filters with an arrow function, and the arguments, when
    /// it is `xs.filter((x) => …)`.
    fn filter_call(&self, id: ExprId) -> Option<(ExprId, Vec<ExprId>)> {
        match &self.ast.expr(id).kind {
            AstExpr::Paren(inner) => self.filter_call(*inner),
            AstExpr::Call { callee, args, .. } => match &self.ast.expr(*callee).kind {
                AstExpr::Field { base, name }
                    if self.ast.name(name.symbol) == "filter"
                        && self.is_buffer(*base)
                        && args.first().is_some_and(|&a| {
                            matches!(self.ast.expr(a).kind, AstExpr::Lambda { .. })
                        }) =>
                {
                    Some((*base, args.clone()))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The text `id` splits, when it is `text.split(…)`.
    pub(super) fn split_source(&self, id: ExprId) -> Option<ExprId> {
        match &self.ast.expr(id).kind {
            AstExpr::Paren(inner) => self.split_source(*inner),
            AstExpr::Call { callee, .. } => match &self.ast.expr(*callee).kind {
                AstExpr::Field { base, name } if self.ast.name(name.symbol) == "split" => {
                    Some(*base)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The local a value is read from, through fields, indices and
    /// parentheses; `None` for a value made by the expression itself.
    pub(super) fn root(&self, id: ExprId) -> Option<perasc_syntax::ast::Symbol> {
        match &self.ast.expr(id).kind {
            AstExpr::Name(symbol) => Some(*symbol),
            AstExpr::Paren(inner) => self.root(*inner),
            AstExpr::Field { base, .. } | AstExpr::Index { base, .. } => self.root(*base),
            _ => None,
        }
    }

    /// Whether code in `spans` names `symbol`: it could then change it.
    fn mentions(&self, spans: &[Span], symbol: perasc_syntax::ast::Symbol) -> bool {
        self.ast.exprs.iter().any(|e| {
            matches!(e.kind, AstExpr::Name(s) if s == symbol)
                && spans
                    .iter()
                    .any(|span| span.start <= e.span.start && e.span.end <= span.end)
        })
    }

    /// `buffers`, with each array of views whose text the code in `spans`
    /// names replaced by a copy, an ordinary array: that code could change the
    /// text while the views are read.
    fn unviewed(&self, state: &mut FnState, buffers: Vec<Expr>, spans: &[Span]) -> Vec<Expr> {
        buffers
            .into_iter()
            .map(|buffer| {
                let ExprKind::Borrow(t) = buffer.kind else {
                    return buffer;
                };
                match state.views.get(&t) {
                    Some(Some(symbol)) if self.mentions(spans, *symbol) => {
                        let copy = state.owned_temp(Expr {
                            kind: ExprKind::Buf(BufOp::Clone, vec![buffer]),
                            ty: Ty::Owned,
                        });
                        state.drops.push(copy);
                        borrowed(copy)
                    }
                    _ => buffer,
                }
            })
            .collect()
    }

    /// Two paths meet: a local live on one and not the other is released on
    /// the one that holds it, at its end. A path that has left (`gone`) does
    /// not count.
    pub(super) fn merge(
        state: &mut FnState,
        a: (BTreeSet<u32>, bool),
        a_stmts: &mut Vec<Stmt>,
        b: (BTreeSet<u32>, bool),
        b_stmts: &mut Vec<Stmt>,
    ) {
        let (live_a, gone_a) = a;
        let (live_b, gone_b) = b;
        match (gone_a, gone_b) {
            (true, true) => {
                state.live = live_a;
                state.gone = true;
            }
            (true, false) => {
                state.live = live_b;
                state.gone = false;
            }
            (false, true) => {
                state.live = live_a;
                state.gone = false;
            }
            (false, false) => {
                let both: BTreeSet<u32> = live_a.intersection(&live_b).copied().collect();
                for (live, stmts) in [(&live_a, a_stmts), (&live_b, b_stmts)] {
                    for &local in live.difference(&both) {
                        stmts.push(Stmt::Buf(
                            BufStmt::Release,
                            vec![Expr {
                                kind: ExprKind::Move(LocalId(local)),
                                ty: Ty::Owned,
                            }],
                        ));
                    }
                }
                state.live = both;
                state.gone = false;
            }
        }
    }

    /// A loop: its body, and the releases that make every turn leave the
    /// locals as it found them, and every way out agree.
    ///
    /// `turn` holds statements to run at the start of each turn, before the
    /// body; `before` those the condition needs first.
    pub(super) fn loop_body(
        &mut self,
        state: &mut FnState,
        cond: Option<Expr>,
        before: Vec<Stmt>,
        body: &perasc_syntax::ast::Stmt,
        step: Vec<Stmt>,
        turn: &mut Vec<Stmt>,
    ) -> Vec<Stmt> {
        self.loop_lowered(state, cond, before, step, turn, |this, state| {
            this.body(state, body)
        })
    }

    /// [`Self::loop_body`], the body given by `lower`, which runs with the
    /// loop open: a `break` it lowers leaves this loop.
    pub(super) fn loop_lowered(
        &mut self,
        state: &mut FnState,
        cond: Option<Expr>,
        before: Vec<Stmt>,
        step: Vec<Stmt>,
        turn: &mut Vec<Stmt>,
        lower: impl FnOnce(&mut Self, &mut FnState) -> Vec<Stmt>,
    ) -> Vec<Stmt> {
        let head = state.live.clone();
        state.loops.push(LoopFrame {
            depth: state.owned.len(),
            head: head.clone(),
            breaks: Vec::new(),
        });
        let lowered = lower(self, state);
        turn.extend(lowered);
        if state.gone {
            state.gone = false;
        } else {
            // A local given a buffer in this turn, empty at its start: released
            // before the next turn (no later code can read it).
            let extra: BTreeSet<u32> = state.live.difference(&head).copied().collect();
            state.release_all(&extra, turn);
        }
        let frame = state.loops.pop().unwrap_or(LoopFrame {
            depth: 0,
            head: head.clone(),
            breaks: Vec::new(),
        });
        // Every way out: the condition, false at the start of a turn, and each break.
        let mut exits: Vec<&BTreeSet<u32>> = frame.breaks.iter().collect();
        if cond.is_some() {
            exits.push(&head);
        }
        let Some((first, rest)) = exits.split_first() else {
            // A loop without condition or `break` never ends.
            state.live = head;
            state.gone = true;
            return vec![Self::loop_with(cond, before, std::mem::take(turn), step)];
        };
        let target: BTreeSet<u32> = rest.iter().fold((*first).clone(), |acc, s| {
            acc.intersection(s).copied().collect()
        });
        let mut breaks = frame.breaks.iter().map(|live| {
            live.difference(&target)
                .map(|&l| release_stmt(LocalId(l)))
                .collect::<Vec<Stmt>>()
        });
        *turn = fill_breaks(std::mem::take(turn), &mut breaks);
        let at_head: Vec<Stmt> = head
            .difference(&target)
            .map(|&l| release_stmt(LocalId(l)))
            .collect();
        state.live = target;
        state.gone = false;
        let mut body = std::mem::take(turn);
        match cond {
            Some(cond) if !at_head.is_empty() || !before.is_empty() => {
                // `loop { before; if (!cond) { releases; break } … }`
                let mut start = before;
                let mut leave = at_head;
                leave.push(Stmt::Break);
                start.push(Stmt::If {
                    cond: Expr {
                        kind: ExprKind::Unary(UnOp::Not, Box::new(cond)),
                        ty: Ty::Bool,
                    },
                    then: leave,
                    otherwise: Vec::new(),
                });
                start.append(&mut body);
                vec![Stmt::Loop {
                    cond: None,
                    body: start,
                    step,
                }]
            }
            cond => vec![Stmt::Loop { cond, body, step }],
        }
    }
}

pub(super) fn release_stmt(local: LocalId) -> Stmt {
    Stmt::Buf(
        BufStmt::Release,
        vec![Expr {
            kind: ExprKind::Move(local),
            ty: Ty::Owned,
        }],
    )
}

/// The `break`s of a loop are lowered as `block { break }`, a marker; each
/// is replaced by the releases its path needs, in the order they were
/// lowered, then the `break`. Nested loops replaced their own already.
fn fill_breaks(stmts: Vec<Stmt>, releases: &mut impl Iterator<Item = Vec<Stmt>>) -> Vec<Stmt> {
    let mut out = Vec::with_capacity(stmts.len());
    for stmt in stmts {
        match stmt {
            Stmt::Block(inner) if matches!(inner.as_slice(), [Stmt::Break]) => {
                out.extend(releases.next().unwrap_or_default());
                out.push(Stmt::Break);
            }
            Stmt::Block(inner) => out.push(Stmt::Block(fill_breaks(inner, releases))),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                let then = fill_breaks(then, releases);
                let otherwise = fill_breaks(otherwise, releases);
                out.push(Stmt::If {
                    cond,
                    then,
                    otherwise,
                });
            }
            other => out.push(other),
        }
    }
    out
}
