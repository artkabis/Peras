//! **Vector loops.** WebAssembly's 128-bit vectors compute four `i32` or `f32`,
//! or two `f64`, in one instruction; JavaScript has nothing like them. A loop
//! whose turns are independent of each other gets a vector copy that runs
//! several turns at once, before the loop as written, which finishes the rest.
//!
//! A loop is vectorized when:
//!
//! - its condition is `i < bound` on an `i32` local `i`, its step exactly
//!   `i = i + 1`, and `bound` a constant, a local or a buffer's length that the
//!   body does not assign;
//! - its body is straight-line: assignments to locals and stores into buffers
//!   (no `if`, loop, call, check or exit), with at least one element read or
//!   stored;
//! - every element is read or stored at index `i` itself, and `i` is not used
//!   otherwise: turns touch disjoint elements, even when two names reach the
//!   same buffer;
//! - every element and every value computed from one has the same type, `i32`,
//!   `f32` or `f64`, and is computed by operations WebAssembly has on vectors
//!   with exactly the scalar result in every lane: `+ - *`, bitwise operations
//!   and shifts by a count the same in every lane on integers, `+ - * /`,
//!   negation and `sqrt` on floats. Integers wrap, the same in every lane; an
//!   integer division, which can trap, is left alone;
//! - a local the body assigns is assigned before the body reads it, so no
//!   value flows from one turn to the next, except an integer **reduction**
//!   `acc = acc op e`, with `op` one of `+ ^ | &`, which reads `acc` nowhere
//!   else. Those operations are associative and commutative on wrapping
//!   integers, so summing lanes apart then together gives the same result.
//!   Floats are never reduced this way: their addition is not associative.
//!
//! The vector loop runs while `i + lanes < bound`, strictly: the last turn is
//! always the scalar loop's, which leaves every local the body assigns with
//! the value the loop as written gives it. Values that do not depend on the
//! turn are computed once per vector turn, then copied into every lane.
//!
//! Elements are read only at indices below `bound`, as the loop as written
//! reads them: its body has no check, so the compiler proved those reads in
//! bounds ([`perasc_ir`]'s bounds check elimination), or wrote the loop that
//! way (`for…of`).

use perasc_ir::{
    BinOp, BufOp, BufStmt, Const, Elem, Expr, ExprKind, LocalId, MathOp, Stmt, Ty, UnOp,
};
use wasm_encoder::{Instruction as I, MemArg, ValType};

use crate::{FnEncoder, Frame};

/// How a loop is vectorized.
pub(crate) struct Plan {
    /// The lane type: `i32`, `f32` or `f64`.
    ty: Ty,
    /// The counter.
    index: LocalId,
    /// The locals the body assigns, each held in a vector.
    varying: Vec<LocalId>,
    /// The reductions, with their operation, each held in a vector too.
    reductions: Vec<(LocalId, BinOp)>,
}

impl Plan {
    /// Lanes per vector.
    fn lanes(&self) -> u32 {
        if self.ty == Ty::F64 { 2 } else { 4 }
    }

    /// Vector locals the loop needs.
    fn vectors(&self) -> u32 {
        u32::try_from(self.varying.len() + self.reductions.len()).unwrap_or(u32::MAX)
    }
}

/// The plan of a loop, when it can be vectorized (see the module's
/// documentation).
pub(crate) fn plan(cond: Option<&Expr>, body: &[Stmt], step: &[Stmt]) -> Option<Plan> {
    let ExprKind::Binary(BinOp::Lt, lhs, bound) = &cond?.kind else {
        return None;
    };
    let ExprKind::Local(index) = lhs.kind else {
        return None;
    };
    if lhs.ty != Ty::I32 {
        return None;
    }
    let [Stmt::Set(target, next)] = step else {
        return None;
    };
    let increments = matches!(
        &next.kind,
        ExprKind::Binary(BinOp::Add, a, b)
            if a.kind == ExprKind::Local(index) && b.kind == ExprKind::Const(Const::Int(1))
    );
    if *target != index || !increments {
        return None;
    }
    let assigned: Vec<LocalId> = body
        .iter()
        .filter_map(|s| match s {
            Stmt::Set(l, _) => Some(*l),
            _ => None,
        })
        .collect();
    if assigned.contains(&index) || reads_any(bound, &assigned) || reads_index(bound, index) {
        return None;
    }
    // The lane type: that of the first element.
    let ty = body.iter().find_map(first_elem)?;
    if !matches!(ty, Ty::I32 | Ty::F32 | Ty::F64) {
        return None;
    }
    let mut plan = Plan {
        ty,
        index,
        varying: Vec::new(),
        reductions: Vec::new(),
    };
    for (position, stmt) in body.iter().enumerate() {
        match stmt {
            Stmt::Set(local, value) => {
                if let Some(op) = reduction(*local, value, ty) {
                    let elsewhere = body
                        .iter()
                        .enumerate()
                        .any(|(p, s)| p != position && stmt_reads(s, *local));
                    let ExprKind::Binary(_, _, other) = &value.kind else {
                        return None;
                    };
                    if elsewhere
                        || plan.reductions.iter().any(|(l, _)| l == local)
                        || plan.varying.contains(local)
                        || !plan.lanewise(other)
                    {
                        return None;
                    }
                    plan.reductions.push((*local, op));
                    continue;
                }
                // Assigned before read: nothing flows from the turn before.
                let read_before = body[..position].iter().any(|s| stmt_reads(s, *local))
                    || reads_any(value, &[*local]);
                if read_before
                    || plan.reductions.iter().any(|(l, _)| l == local)
                    || value.ty != ty
                    || !plan.lanewise(value)
                {
                    return None;
                }
                if !plan.varying.contains(local) {
                    plan.varying.push(*local);
                }
            }
            Stmt::Buf(BufStmt::Set(elem), args) => {
                let [buffer, at, value] = args.as_slice() else {
                    return None;
                };
                if elem.ty() != ty
                    || !is_buffer(buffer)
                    || at.kind != ExprKind::Local(index)
                    || !plan.lanewise(value)
                {
                    return None;
                }
            }
            _ => return None,
        }
    }
    // A reduction read after the body is fine; one read inside is not, and
    // a local assigned in the body may not also be reduced.
    Some(plan)
}

impl Plan {
    /// Whether `e`, of the lane type, computes lane by lane exactly.
    fn lanewise(&self, e: &Expr) -> bool {
        if self.invariant(e) {
            return e.ty == self.ty && !reads_index(e, self.index);
        }
        if e.ty != self.ty {
            return false;
        }
        match &e.kind {
            ExprKind::Local(l) => self.varying.contains(l),
            ExprKind::Buf(BufOp::Get(elem), args) => {
                elem.ty() == self.ty
                    && matches!(args.as_slice(), [b, at] if is_buffer(b) && at.kind == ExprKind::Local(self.index))
            }
            ExprKind::Binary(op, a, b) => {
                let shift = matches!(op, BinOp::Shl | BinOp::Shr);
                if shift {
                    return self.ty == Ty::I32
                        && self.lanewise(a)
                        && self.invariant(b)
                        && !reads_index(b, self.index);
                }
                vector_binary(*op, self.ty).is_some() && self.lanewise(a) && self.lanewise(b)
            }
            ExprKind::Unary(op, x) => vector_unary(*op, self.ty).is_some() && self.lanewise(x),
            ExprKind::Math(MathOp::Sqrt, x) => self.ty != Ty::I32 && self.lanewise(x),
            _ => false,
        }
    }

    /// Whether `e` is the same in every turn: it reads no element at the
    /// counter and no local the body assigns.
    fn invariant(&self, e: &Expr) -> bool {
        !reads_any(e, &self.varying)
            && !reads_any(
                e,
                &self.reductions.iter().map(|(l, _)| *l).collect::<Vec<_>>(),
            )
            && !reads_index(e, self.index)
            && simple(e)
    }
}

/// `acc = acc op e` with `op` an associative and commutative operation on
/// wrapping integers.
fn reduction(local: LocalId, value: &Expr, ty: Ty) -> Option<BinOp> {
    let ExprKind::Binary(op, a, b) = &value.kind else {
        return None;
    };
    let reducible = matches!(
        op,
        BinOp::Add | BinOp::BitXor | BinOp::BitOr | BinOp::BitAnd
    );
    (ty == Ty::I32 && reducible && a.kind == ExprKind::Local(local) && !reads_any(b, &[local]))
        .then_some(*op)
}

/// Whether evaluating `e` cannot trap, call or allocate.
fn simple(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Const(_) | ExprKind::Local(_) => true,
        ExprKind::Binary(BinOp::Div | BinOp::Rem, ..) if e.ty == Ty::I32 || e.ty == Ty::I64 => {
            false
        }
        ExprKind::Binary(_, a, b) => simple(a) && simple(b),
        ExprKind::Unary(_, x) | ExprKind::Convert(x) => simple(x),
        ExprKind::Buf(BufOp::Len, args) => args.iter().all(is_buffer),
        _ => false,
    }
}

fn is_buffer(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Local(_) | ExprKind::Borrow(_))
}

/// The type of the first element `stmt` reads or stores.
fn first_elem(stmt: &Stmt) -> Option<Ty> {
    match stmt {
        Stmt::Buf(BufStmt::Set(elem), _) => Some(elem.ty()),
        Stmt::Set(_, e) => first_get(e),
        _ => None,
    }
    .or_else(|| match stmt {
        Stmt::Buf(BufStmt::Set(_), args) => args.iter().find_map(first_get),
        _ => None,
    })
}

fn first_get(e: &Expr) -> Option<Ty> {
    match &e.kind {
        ExprKind::Buf(BufOp::Get(elem), _) if *elem != Elem::Handle => Some(elem.ty()),
        ExprKind::Buf(_, args) | ExprKind::Call(_, args) => args.iter().find_map(first_get),
        ExprKind::Binary(_, a, b) => first_get(a).or_else(|| first_get(b)),
        ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Math(_, x)
        | ExprKind::Alloc(x) => first_get(x),
        _ => None,
    }
}

/// Whether `e` reads one of `locals`.
fn reads_any(e: &Expr, locals: &[LocalId]) -> bool {
    match &e.kind {
        ExprKind::Local(l) | ExprKind::Move(l) | ExprKind::Borrow(l) => locals.contains(l),
        ExprKind::Const(_) => false,
        ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Math(_, x)
        | ExprKind::Alloc(x) => reads_any(x, locals),
        ExprKind::Binary(_, a, b) => reads_any(a, locals) || reads_any(b, locals),
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => {
            args.iter().any(|a| reads_any(a, locals))
        }
    }
}

/// Whether `e` reads the counter other than as the index of an element.
fn reads_index(e: &Expr, index: LocalId) -> bool {
    match &e.kind {
        ExprKind::Buf(BufOp::Get(_), args) => match args.as_slice() {
            [b, at] if at.kind == ExprKind::Local(index) => reads_index(b, index),
            _ => args.iter().any(|a| reads_any(a, &[index])),
        },
        _ => reads_any(e, &[index]),
    }
}

fn stmt_reads(stmt: &Stmt, local: LocalId) -> bool {
    match stmt {
        Stmt::Set(_, e) => reads_any(e, &[local]),
        Stmt::Buf(_, args) => args.iter().any(|a| reads_any(a, &[local])),
        _ => true,
    }
}

/// The vector instruction of a binary operation, if lanes compute it exactly.
fn vector_binary(op: BinOp, ty: Ty) -> Option<I<'static>> {
    Some(match (ty, op) {
        (Ty::I32, BinOp::Add) => I::I32x4Add,
        (Ty::I32, BinOp::Sub) => I::I32x4Sub,
        (Ty::I32, BinOp::Mul) => I::I32x4Mul,
        (Ty::I32, BinOp::BitAnd) => I::V128And,
        (Ty::I32, BinOp::BitOr) => I::V128Or,
        (Ty::I32, BinOp::BitXor) => I::V128Xor,
        (Ty::I32, BinOp::Shl) => I::I32x4Shl,
        (Ty::I32, BinOp::Shr) => I::I32x4ShrS,
        (Ty::F32, BinOp::Add) => I::F32x4Add,
        (Ty::F32, BinOp::Sub) => I::F32x4Sub,
        (Ty::F32, BinOp::Mul) => I::F32x4Mul,
        (Ty::F32, BinOp::Div) => I::F32x4Div,
        (Ty::F64, BinOp::Add) => I::F64x2Add,
        (Ty::F64, BinOp::Sub) => I::F64x2Sub,
        (Ty::F64, BinOp::Mul) => I::F64x2Mul,
        (Ty::F64, BinOp::Div) => I::F64x2Div,
        _ => return None,
    })
}

fn vector_unary(op: UnOp, ty: Ty) -> Option<I<'static>> {
    Some(match (ty, op) {
        (Ty::I32, UnOp::Neg) => I::I32x4Neg,
        (Ty::I32, UnOp::BitNot) => I::V128Not,
        (Ty::F32, UnOp::Neg) => I::F32x4Neg,
        (Ty::F64, UnOp::Neg) => I::F64x2Neg,
        _ => return None,
    })
}

/// The vector locals the function's loops need at most at once.
pub(crate) fn vectors_needed(stmts: &[Stmt]) -> u32 {
    stmts
        .iter()
        .map(|stmt| match stmt {
            Stmt::Loop { cond, body, step } => {
                let own = plan(cond.as_ref(), body, step).map_or(0, |p| p.vectors());
                own.max(vectors_needed(body))
            }
            Stmt::If {
                then, otherwise, ..
            } => vectors_needed(then).max(vectors_needed(otherwise)),
            Stmt::Block(body) => vectors_needed(body),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

/// The `v128` value type, for the function's locals.
pub(crate) const V128: ValType = ValType::V128;

impl FnEncoder<'_> {
    /// The vector copy of a loop: runs `lanes` turns at once while more than
    /// that many remain, then lets the loop as written finish.
    pub(crate) fn vector_loop(&mut self, plan: &Plan, cond: &Expr, body: &[Stmt]) {
        let ExprKind::Binary(_, _, bound) = &cond.kind else {
            return;
        };
        let lanes = plan.lanes();
        let index = plan.index.0;
        // The reductions start at their operation's identity.
        for (k, (_, op)) in plan.reductions.iter().enumerate() {
            let identity = if *op == BinOp::BitAnd { -1 } else { 0 };
            self.emit(&I::I32Const(identity));
            self.emit(&I::I32x4Splat);
            self.emit(&I::LocalSet(
                self.vector_local(plan, plan.varying.len() + k),
            ));
        }
        self.emit(&I::Block(wasm_encoder::BlockType::Empty));
        self.frames.push(Frame::Other);
        self.emit(&I::Loop(wasm_encoder::BlockType::Empty));
        self.frames.push(Frame::Other);
        // Out when `i + lanes < bound` no longer holds.
        self.emit(&I::LocalGet(index));
        self.emit(&I::I32Const(i32::try_from(lanes).unwrap_or(4)));
        self.emit(&I::I32Add);
        self.expr(bound);
        self.emit(&I::I32GeS);
        self.emit(&I::BrIf(1));
        for stmt in body {
            match stmt {
                Stmt::Set(local, value) => {
                    if let Some(k) = plan.reductions.iter().position(|(l, _)| l == local) {
                        let (_, op) = plan.reductions[k];
                        let at = self.vector_local(plan, plan.varying.len() + k);
                        self.emit(&I::LocalGet(at));
                        if let ExprKind::Binary(_, _, other) = &value.kind {
                            self.vector_expr(plan, other);
                        }
                        if let Some(instruction) = vector_binary(op, Ty::I32) {
                            self.emit(&instruction);
                        }
                        self.emit(&I::LocalSet(at));
                    } else if let Some(k) = plan.varying.iter().position(|l| l == local) {
                        self.vector_expr(plan, value);
                        self.emit(&I::LocalSet(self.vector_local(plan, k)));
                    }
                }
                Stmt::Buf(BufStmt::Set(elem), args) => {
                    if let [buffer, _, value] = args.as_slice() {
                        self.vector_address(buffer, index, *elem);
                        self.vector_expr(plan, value);
                        self.emit(&I::V128Store(memarg()));
                    }
                }
                _ => {}
            }
        }
        self.emit(&I::LocalGet(index));
        self.emit(&I::I32Const(i32::try_from(lanes).unwrap_or(4)));
        self.emit(&I::I32Add);
        self.emit(&I::LocalSet(index));
        self.emit(&I::Br(0));
        self.frames.pop();
        self.emit(&I::End);
        self.frames.pop();
        self.emit(&I::End);
        // Each reduction takes in its lanes.
        for (k, (local, op)) in plan.reductions.iter().enumerate() {
            let at = self.vector_local(plan, plan.varying.len() + k);
            let Some(scalar) = scalar_op(*op) else {
                continue;
            };
            self.emit(&I::LocalGet(local.0));
            for lane in 0..4 {
                self.emit(&I::LocalGet(at));
                self.emit(&I::I32x4ExtractLane(lane));
                self.emit(&scalar);
            }
            self.emit(&I::LocalSet(local.0));
        }
    }

    fn vector_local(&self, _plan: &Plan, k: usize) -> u32 {
        self.temp_v128 + u32::try_from(k).unwrap_or(0)
    }

    /// The address of the element at the counter, for a vector load or store.
    fn vector_address(&mut self, buffer: &Expr, index: u32, elem: Elem) {
        self.expr(buffer);
        self.emit(&I::I32Load(crate::mem(8, 4)));
        self.emit(&I::LocalGet(index));
        let shift = elem.size().trailing_zeros();
        if shift > 0 {
            self.emit(&I::I32Const(i32::try_from(shift).unwrap_or(0)));
            self.emit(&I::I32Shl);
        }
        self.emit(&I::I32Add);
    }

    /// `e` in every lane: computed lane by lane, or once and copied.
    fn vector_expr(&mut self, plan: &Plan, e: &Expr) {
        if plan.invariant(e) {
            self.expr(e);
            self.emit(&splat(plan.ty));
            return;
        }
        match &e.kind {
            ExprKind::Local(l) => {
                if let Some(k) = plan.varying.iter().position(|v| v == l) {
                    self.emit(&I::LocalGet(self.vector_local(plan, k)));
                }
            }
            ExprKind::Buf(BufOp::Get(elem), args) => {
                if let Some(buffer) = args.first() {
                    self.vector_address(buffer, plan.index.0, *elem);
                    self.emit(&I::V128Load(memarg()));
                }
            }
            ExprKind::Binary(op @ (BinOp::Shl | BinOp::Shr), a, b) => {
                self.vector_expr(plan, a);
                self.expr(b);
                if let Some(instruction) = vector_binary(*op, plan.ty) {
                    self.emit(&instruction);
                }
            }
            ExprKind::Binary(op, a, b) => {
                self.vector_expr(plan, a);
                self.vector_expr(plan, b);
                if let Some(instruction) = vector_binary(*op, plan.ty) {
                    self.emit(&instruction);
                }
            }
            ExprKind::Unary(op, x) => {
                self.vector_expr(plan, x);
                if let Some(instruction) = vector_unary(*op, plan.ty) {
                    self.emit(&instruction);
                }
            }
            ExprKind::Math(MathOp::Sqrt, x) => {
                self.vector_expr(plan, x);
                self.emit(if plan.ty == Ty::F32 {
                    &I::F32x4Sqrt
                } else {
                    &I::F64x2Sqrt
                });
            }
            _ => {}
        }
    }
}

fn splat(ty: Ty) -> I<'static> {
    match ty {
        Ty::F32 => I::F32x4Splat,
        Ty::F64 => I::F64x2Splat,
        _ => I::I32x4Splat,
    }
}

fn scalar_op(op: BinOp) -> Option<I<'static>> {
    Some(match op {
        BinOp::Add => I::I32Add,
        BinOp::BitXor => I::I32Xor,
        BinOp::BitOr => I::I32Or,
        BinOp::BitAnd => I::I32And,
        _ => return None,
    })
}

/// No alignment promised: an element of a buffer is aligned to its own size
/// only.
const fn memarg() -> MemArg {
    crate::mem(0, 1)
}
