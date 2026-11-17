//! Shared values, `Rc<T>` (decision 0032, prototype). An `Rc<T>` is one
//! handle: the counted block function values use ([`EnvOp::Shared`]), whose
//! slots hold the leaves of `T`, its buffers and shared values first, which
//! its last holder releases, then its numbers, each an `i64`. A copy holds
//! the block once more; a field is read from its slot, a buffer lent.

use perasc_check::ty::Ty as CheckTy;
use perasc_syntax::ast::ExprId;

use super::fnvalues::{from_slot, to_slot};
use super::heap::{self, Mode};
use super::{FnState, Lowering};
use crate::{BinOp, BufOp, Const, EnvOp, Expr, ExprKind, RuntimeFn, Stmt, Ty};

impl Lowering<'_> {
    /// `const x = place`, `x` a shared value: lent rather than held once
    /// more, read from the place at each use, when nothing from there to the
    /// end of the function changes or moves the variable the place starts
    /// from or a name its indices read: `const mesh = fleet[i].mesh` in a
    /// loop that draws makes no count operation (decision 0032).
    pub(super) fn lent_shared(
        &mut self,
        state: &mut FnState,
        local: &perasc_syntax::ast::Local,
    ) -> Option<Vec<Expr>> {
        use perasc_syntax::ast::ExprKind as Ast;
        let init = local.init?;
        if local.mutable || local.ty.is_some() || !self.is_shared(init) {
            return None;
        }
        let mut at = init;
        loop {
            match &self.ast.expr(at).kind {
                Ast::Field { base, .. } | Ast::Index { base, .. } => at = *base,
                Ast::Paren(inner) => at = *inner,
                Ast::Name(_) => break,
                _ => return None,
            }
        }
        let place = self.ast.expr(init).span;
        let within = |s: perasc_diag::Span, from: u32, to: u32| from <= s.start && s.end <= to;
        // Every variable the place reads: the one it starts from, and those
        // of its indices; each must be a variable of the function.
        let read: std::collections::HashSet<usize> = self
            .analysis
            .names
            .iter()
            .filter(|(id, _)| {
                within(self.ast.expr(**id).span, place.start, place.end)
                    && matches!(self.ast.expr(**id).kind, Ast::Name(_))
            })
            .map(|(_, &binding)| binding)
            .collect();
        if !self.analysis.names.contains_key(&at) {
            return None;
        }
        let (from, to) = (local.name.span.end, self.body_span.end);
        let changed = self
            .analysis
            .mutations
            .iter()
            .any(|(binding, span)| read.contains(binding) && within(*span, from, to));
        let moved = self.analysis.moves.iter().any(|id| {
            within(self.ast.expr(*id).span, from, to)
                && self
                    .analysis
                    .names
                    .get(id)
                    .is_some_and(|b| read.contains(b))
        });
        if changed || moved {
            return None;
        }
        Some(self.value_as(state, init, Mode::Borrow))
    }

    /// Whether an expression is a shared value, `Rc<T>`.
    pub(super) fn is_shared(&self, id: ExprId) -> bool {
        matches!(self.check_type(id), Some(CheckTy::Rc(_)))
    }

    /// The slot of each leaf of `T`, its buffers first, and how many those
    /// are.
    pub(super) fn shared_slots(&self, inner: &CheckTy) -> Option<(Vec<Ty>, Vec<u32>, u32)> {
        let leaves: Vec<Ty> = self
            .leaves(inner)
            .ok()?
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        let owned = leaves.iter().filter(|t| t.is_handle()).count();
        let (mut handles, mut numbers) = (0_u32, u32::try_from(owned).ok()?);
        let slots = leaves
            .iter()
            .map(|t| {
                let next = if t.is_handle() {
                    &mut handles
                } else {
                    &mut numbers
                };
                let slot = *next;
                *next += 1;
                slot
            })
            .collect();
        Some((leaves, slots, u32::try_from(owned).ok()?))
    }

    /// `Rc.new(value)`: the value's leaves put in a new block, which holds
    /// its buffers from now on.
    pub(super) fn shared_new(
        &mut self,
        state: &mut FnState,
        id: ExprId,
        value: ExprId,
        mode: Mode,
    ) -> Vec<Expr> {
        let Some(CheckTy::Rc(inner)) = self.check_type(id) else {
            return Vec::new();
        };
        let Some((leaves, slots, owned)) = self.shared_slots(&inner) else {
            self.unsupported(self.ast.expr(id).span, "rc");
            return Vec::new();
        };
        let values = self.value_as(state, value, Mode::Consume);
        let mut ordered: Vec<Option<Expr>> = vec![None; leaves.len()];
        for ((value, leaf), &slot) in values.into_iter().zip(&leaves).zip(&slots) {
            let value = if leaf.is_handle() {
                value
            } else {
                to_slot(value)
            };
            if let Some(at) = ordered.get_mut(slot as usize) {
                *at = Some(value);
            }
        }
        let block = Expr {
            kind: ExprKind::Env(
                EnvOp::Shared(owned),
                ordered.into_iter().flatten().collect(),
            ),
            ty: Ty::Owned,
        };
        Self::fresh(state, vec![block], mode)
    }

    /// The shared value a field written goes through: the local holding it,
    /// the type it holds, and the leaves written, `None` when the target
    /// reaches no `Rc`.
    pub(super) fn shared_place(
        &mut self,
        state: &FnState,
        target: ExprId,
    ) -> Option<(crate::LocalId, CheckTy, usize, usize)> {
        let perasc_syntax::ast::ExprKind::Field { base, name } = &self.ast.expr(target).kind else {
            return None;
        };
        let (base, name) = (*base, self.ast.name(name.symbol).to_owned());
        if let Some(CheckTy::Rc(inner)) = self.check_type(base) {
            let (offset, len, _) = self.field(&inner, &name)?;
            // A shared value held in a variable, or in a field of one; one
            // held by another shared value or an element is not written yet.
            let locals = self.place(state, base);
            let Some([local]) = locals.as_deref() else {
                self.unsupported(self.ast.expr(target).span, "rc-write");
                return None;
            };
            return Some((*local, *inner, offset, len));
        }
        let (local, inner, outer, _) = self.shared_place(state, base)?;
        let base_ty = self.check_type(base)?;
        let (offset, len, _) = self.field(&base_ty, &name)?;
        Some((local, inner, outer + offset, len))
    }

    /// `rc.field = value`, or `op=`: the value made the holder's own first,
    /// under r1, then its slots written; under r2, written in place, where
    /// every holder sees it.
    pub(super) fn shared_assign(
        &mut self,
        state: &mut FnState,
        (handle, inner, offset, len): (crate::LocalId, CheckTy, usize, usize),
        op: Option<BinOp>,
        (target, value): (ExprId, ExprId),
        out: &mut Vec<Stmt>,
    ) {
        let Some((leaves, slots, _)) = self.shared_slots(&inner) else {
            return;
        };
        let mut values = self.value(state, value);
        if let (Some(op), [one]) = (op, values.as_mut_slice()) {
            let current = self
                .shared_fields(&heap::borrowed(handle), &inner, offset, 1)
                .pop()
                .unwrap_or_else(|| super::zero(one.ty));
            let kind = self.int_kind(target);
            *one = self.arith(state, op, current, one.clone(), kind);
        }
        // Each value in a local first: a value may read the field it
        // replaces, which the store releases.
        let values: Vec<Expr> = values
            .into_iter()
            .map(|v| {
                let owned = v.ty == Ty::Owned;
                let temp = state.temp(v.ty);
                state.pre.push(Stmt::Set(temp, v));
                if owned {
                    heap::moved(state, temp)
                } else {
                    super::local_expr(state, temp)
                }
            })
            .collect();
        state.flush(out);
        Self::make_unique(state, handle, leaves.len(), out);
        for ((leaf, &slot), value) in leaves.iter().zip(&slots).skip(offset).take(len).zip(values) {
            let (op, value) = if leaf.is_handle() {
                (EnvOp::StoreHandle(slot), value)
            } else {
                (EnvOp::Store(slot), to_slot(value))
            };
            out.push(Stmt::Expr(Expr {
                kind: ExprKind::Env(op, vec![heap::borrowed(handle), value]),
                ty: Ty::Void,
            }));
        }
    }

    /// Under r1, the shared value a local holds made its own: the same
    /// block when it is the only holder, a copy otherwise; under r2, left
    /// as it is, every holder seeing what is written.
    fn make_unique(state: &mut FnState, handle: crate::LocalId, slots: usize, out: &mut Vec<Stmt>) {
        if perasc_check::proto_rc() == "r2" {
            return;
        }
        let bytes = 16 + 8 * i64::try_from(slots).unwrap_or(0);
        let unique = Expr {
            kind: ExprKind::Buf(
                BufOp::Runtime(RuntimeFn::SharedUnique),
                vec![
                    Expr {
                        kind: ExprKind::Const(Const::Int(bytes)),
                        ty: Ty::I32,
                    },
                    heap::moved(state, handle),
                ],
            ),
            ty: Ty::Owned,
        };
        out.push(Stmt::Set(handle, unique));
        state.live.insert(handle.0);
    }

    /// `rc.field`: read from its slots, a buffer lent, or held once more
    /// where it is taken. On the way to a place the program changes, as
    /// `rc.items` in `rc.items.push(4)`, the value is made the holder's own
    /// first.
    pub(super) fn shared_field(
        &mut self,
        state: &mut FnState,
        (id, base): (ExprId, ExprId),
        name: perasc_syntax::ast::Symbol,
        mode: Mode,
    ) -> Vec<Expr> {
        let Some(CheckTy::Rc(inner)) = self.check_type(base) else {
            return Vec::new();
        };
        if self.analysis.changed.contains(&id) {
            let locals = self.place(state, base);
            let Some([local]) = locals.as_deref() else {
                self.unsupported(self.ast.expr(id).span, "rc-write");
                return Vec::new();
            };
            let slots = self.shared_slots(&inner).map_or(0, |(l, _, _)| l.len());
            let mut pre = Vec::new();
            Self::make_unique(state, *local, slots, &mut pre);
            state.pre.extend(pre);
        }
        let handle = self.value_as(state, base, Mode::Borrow).into_iter().next();
        let picked = self.field(&inner, self.ast.name(name));
        let (Some(handle), Some((offset, len, _))) = (handle, picked) else {
            return Vec::new();
        };
        let values = self.shared_fields(&handle, &inner, offset, len);
        if mode == Mode::Consume {
            let values = Self::retained(values);
            Self::fresh(state, values, mode)
        } else {
            values
        }
    }

    /// `Rc.weak(rc)`, a holder that does not keep the value alive, or
    /// `weak.get()`, the value while it lives, as an option (r2).
    pub(super) fn weak_call(
        &mut self,
        state: &mut FnState,
        (base, arg): (ExprId, Option<ExprId>),
        mode: Mode,
    ) -> Vec<Expr> {
        let runtime = |f: RuntimeFn, of: Expr| Expr {
            kind: ExprKind::Buf(BufOp::Runtime(f), vec![of]),
            ty: f.result(),
        };
        let values = if matches!(self.check_type(base), Some(CheckTy::Weak(_))) {
            let Some(weak) = self.value_as(state, base, Mode::Borrow).into_iter().next() else {
                return Vec::new();
            };
            vec![
                runtime(RuntimeFn::WeakAlive, weak.clone()),
                runtime(RuntimeFn::WeakGet, weak),
            ]
        } else {
            let Some(arg) = arg else {
                return Vec::new();
            };
            let Some(rc) = self.value_as(state, arg, Mode::Borrow).into_iter().next() else {
                return Vec::new();
            };
            vec![runtime(RuntimeFn::WeakNew, rc)]
        };
        Self::fresh(state, values, mode)
    }

    /// The leaves `offset` to `offset + len` of the value an `Rc` holds,
    /// read from their slots, the buffers lent.
    pub(super) fn shared_fields(
        &self,
        handle: &Expr,
        inner: &CheckTy,
        offset: usize,
        len: usize,
    ) -> Vec<Expr> {
        let Some((leaves, slots, _)) = self.shared_slots(inner) else {
            return Vec::new();
        };
        leaves
            .iter()
            .zip(&slots)
            .skip(offset)
            .take(len)
            .map(|(leaf, &slot)| {
                if leaf.is_handle() {
                    Expr {
                        kind: ExprKind::Env(EnvOp::LoadHandle(slot), vec![handle.clone()]),
                        ty: Ty::Borrowed,
                    }
                } else {
                    let read = Expr {
                        kind: ExprKind::Env(EnvOp::Load(slot), vec![handle.clone()]),
                        ty: Ty::I64,
                    };
                    from_slot(read, *leaf)
                }
            })
            .collect()
    }
}
