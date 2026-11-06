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
    /// The element type of an array type, if it is compiled.
    pub(super) fn element(ty: Option<&CheckTy>) -> Option<Elem> {
        match ty {
            Some(CheckTy::Array(inner)) => Self::ir_type(inner).and_then(Elem::of),
            _ => None,
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

    /// The buffer of an array expression, lent.
    pub(super) fn buffer(&mut self, state: &mut FnState, id: ExprId) -> Option<Expr> {
        self.value_as(state, id, Mode::Borrow).into_iter().next()
    }

    /// `[a, b, c]`: a new buffer with each element pushed, in order.
    pub(super) fn array_literal(
        &mut self,
        state: &mut FnState,
        id: ExprId,
        items: &[ExprId],
    ) -> Option<Expr> {
        let span = self.ast.expr(id).span;
        let Some(elem) = Self::element(self.check_type(id)) else {
            self.unsupported(span, "array-element");
            return None;
        };
        let count = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let temp = state.owned_temp(new_buffer(int(count), elem));
        for &item in items {
            let value = self.expr(state, item);
            state
                .pre
                .push(Stmt::Buf(BufStmt::Push(elem), vec![borrowed(temp), value]));
        }
        Some(Expr {
            kind: ExprKind::Local(temp),
            ty: Ty::Owned,
        })
    }

    /// `xs.length`.
    pub(super) fn array_length(&mut self, state: &mut FnState, base: ExprId) -> Expr {
        match self.buffer(state, base) {
            Some(buffer) => len(&buffer),
            None => int(0),
        }
    }

    /// `xs[i]`, checked.
    pub(super) fn index_get(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        index: ExprId,
        span: Span,
    ) -> Expr {
        let Some(elem) = Self::element(self.check_type(base)) else {
            self.unsupported(span, "array-element");
            return int(0);
        };
        let Some(buffer) = self.buffer(state, base) else {
            return int(0);
        };
        let index = self.expr(state, index);
        let index = state.spill(index);
        state.pre.push(Stmt::Buf(
            BufStmt::CheckIndex,
            vec![index.clone(), len(&buffer)],
        ));
        get(&buffer, index, elem)
    }

    /// `xs[i] = v` and `xs[i] op= v`: the index and the value first, then the
    /// check, then the store.
    pub(super) fn index_assign(
        &mut self,
        state: &mut FnState,
        target: ExprId,
        op: Option<BinOp>,
        value: Option<ExprId>,
        out: &mut Vec<Stmt>,
    ) {
        let AstExpr::Index { base, index } = self.ast.expr(target).kind else {
            return;
        };
        let span = self.ast.expr(target).span;
        let Some(elem) = Self::element(self.check_type(base)) else {
            self.unsupported(span, "array-element");
            return;
        };
        let Some(buffer) = self.buffer(state, base) else {
            return;
        };
        let index = self.expr(state, index);
        let index = state.spill(index);
        let value = match value {
            Some(value) => self.expr(state, value),
            None => super::one(elem.ty()),
        };
        let value = state.spill(value);
        state.pre.push(Stmt::Buf(
            BufStmt::CheckIndex,
            vec![index.clone(), len(&buffer)],
        ));
        let value = match op {
            Some(op) => super::binary(op, get(&buffer, index.clone(), elem), value),
            None => value,
        };
        state
            .pre
            .push(Stmt::Buf(BufStmt::Set(elem), vec![buffer, index, value]));
        state.flush(out);
        state.end_statement(out);
    }

    /// A method of an array, as scalars: `push` gives none, `pop` an option,
    /// `copy` a new array.
    pub(super) fn array_method(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        method: &str,
        args: &[ExprId],
        mode: Mode,
        span: Span,
    ) -> Vec<Expr> {
        let Some(elem) = Self::element(self.check_type(base)) else {
            self.unsupported(span, "array-element");
            return Vec::new();
        };
        let Some(buffer) = self.buffer(state, base) else {
            return Vec::new();
        };
        match method {
            "push" => {
                let Some(&arg) = args.first() else {
                    return Vec::new();
                };
                let value = self.expr(state, arg);
                state
                    .pre
                    .push(Stmt::Buf(BufStmt::Push(elem), vec![buffer, value]));
                Vec::new()
            }
            "pop" => {
                // n = len; if (n > 0) { v = xs[n - 1]; truncate to n - 1 }
                let n = state.temp(Ty::I32);
                state.pre.push(Stmt::Set(n, len(&buffer)));
                let n = super::local_expr(state, n);
                let some = super::binary(BinOp::Gt, n.clone(), int(0));
                let last = super::binary(BinOp::Sub, n, int(1));
                let value = state.temp(elem.ty());
                state.pre.push(Stmt::Set(value, super::zero(elem.ty())));
                let some_temp = state.temp(Ty::Bool);
                state.pre.push(Stmt::Set(some_temp, some));
                state.pre.push(Stmt::If {
                    cond: super::local_expr(state, some_temp),
                    then: vec![
                        Stmt::Set(value, get(&buffer, last.clone(), elem)),
                        Stmt::Buf(BufStmt::Truncate, vec![buffer, last]),
                    ],
                    otherwise: Vec::new(),
                });
                vec![
                    super::local_expr(state, some_temp),
                    super::local_expr(state, value),
                ]
            }
            "copy" => {
                // t = new(len); for (i = 0; i < len; i++) push(t, xs[i])
                let n = state.temp(Ty::I32);
                state.pre.push(Stmt::Set(n, len(&buffer)));
                let n = super::local_expr(state, n);
                let copy = state.owned_temp(new_buffer(n.clone(), elem));
                let i = state.temp(Ty::I32);
                state.pre.push(Stmt::Set(i, int(0)));
                let i_expr = super::local_expr(state, i);
                state.pre.push(Stmt::Loop {
                    cond: Some(super::binary(BinOp::Lt, i_expr.clone(), n)),
                    body: vec![Stmt::Buf(
                        BufStmt::Push(elem),
                        vec![borrowed(copy), get(&buffer, i_expr.clone(), elem)],
                    )],
                    step: vec![Stmt::Set(i, super::binary(BinOp::Add, i_expr, int(1)))],
                });
                let copied = Expr {
                    kind: ExprKind::Local(copy),
                    ty: Ty::Owned,
                };
                Self::fresh(state, vec![copied], mode)
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
        let Some(elem) = Self::element(self.check_type(iterable)) else {
            self.unsupported(span, "array-element");
            return;
        };
        let Some(buffer) = self.buffer(state, iterable) else {
            return;
        };
        let i = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(i, int(0)));
        state.flush(out);
        let i_expr = super::local_expr(state, i);
        state.open();
        let x = state.add(name.symbol, self.ast.name(name.symbol), elem.ty());
        let mut turn = vec![Stmt::Set(x, get(&buffer, i_expr.clone(), elem))];
        let cond = super::binary(BinOp::Lt, i_expr.clone(), len(&buffer));
        let step = vec![Stmt::Set(i, super::binary(BinOp::Add, i_expr, int(1)))];
        let lowered = self.loop_body(state, Some(cond), Vec::new(), body, step, &mut turn);
        out.extend(lowered);
        state.close(out);
        state.end_statement(out);
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
        let head = state.live.clone();
        state.loops.push(LoopFrame {
            depth: state.owned.len(),
            head: head.clone(),
            breaks: Vec::new(),
        });
        let lowered = self.body(state, body);
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
