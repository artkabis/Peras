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
use crate::{BinOp, BufOp, BufStmt, Const, EnvOp, Expr, ExprKind, RuntimeFn, Stmt, Ty};

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

    /// The shared value a field written goes through, `rc` in `rc.a.b = v`:
    /// its expression, the type it holds, and the leaves written; `None`
    /// when the target reaches no `Rc`.
    pub(super) fn shared_place(&self, target: ExprId) -> Option<(ExprId, CheckTy, usize, usize)> {
        let perasc_syntax::ast::ExprKind::Field { base, name } = &self.ast.expr(target).kind else {
            return None;
        };
        let (base, name) = (*base, self.ast.name(name.symbol));
        if let Some(CheckTy::Rc(inner)) = self.check_type(base) {
            let (offset, len, _) = self.field(&inner, name)?;
            return Some((base, *inner, offset, len));
        }
        let (rc, inner, outer, _) = self.shared_place(base)?;
        let base_ty = self.check_type(base)?;
        let (offset, len, _) = self.field(&base_ty, name)?;
        Some((rc, inner, outer + offset, len))
    }

    /// `rc.field = value`, `op=`, or `rc.field++` (no value): the value made
    /// the holder's own first, under r1, then its slots written; under r2,
    /// written in place, where every holder sees it.
    pub(super) fn shared_assign(
        &mut self,
        state: &mut FnState,
        (rc, inner, offset, len): (ExprId, CheckTy, usize, usize),
        op: Option<BinOp>,
        (target, value): (ExprId, Option<ExprId>),
        out: &mut Vec<Stmt>,
    ) {
        let Some((leaves, slots, _)) = self.shared_slots(&inner) else {
            return;
        };
        let mut values = match value {
            Some(value) => self.value(state, value),
            None => leaves
                .get(offset)
                .map(|t| vec![super::one(*t)])
                .unwrap_or_default(),
        };
        if let (Some(op), [one]) = (op, values.as_mut_slice()) {
            let Some(current) = self.value_as(state, rc, Mode::Borrow).into_iter().next() else {
                return;
            };
            let current = self
                .shared_fields(&current, &inner, offset, 1)
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
        let Some(handle) = self.unique_handle(state, rc) else {
            self.unsupported(self.ast.expr(target).span, "rc-write");
            return;
        };
        state.flush(out);
        for ((leaf, &slot), value) in leaves.iter().zip(&slots).skip(offset).take(len).zip(values) {
            let (op, value) = if leaf.is_handle() {
                (EnvOp::StoreHandle(slot), value)
            } else {
                (EnvOp::Store(slot), to_slot(value))
            };
            out.push(Stmt::Expr(Expr {
                kind: ExprKind::Env(op, vec![handle.clone(), value]),
                ty: Ty::Void,
            }));
        }
    }

    /// The shared value `rc` names made its holder's own, under r1, where it
    /// is held: a variable, a slot of another shared value, made its
    /// holder's own first, or an element of an array; its handle, lent, to
    /// read and write it. Under r2, the value as it is.
    pub(super) fn unique_handle(&mut self, state: &mut FnState, rc: ExprId) -> Option<Expr> {
        let slots = match self.check_type(rc) {
            Some(CheckTy::Rc(inner)) => self.shared_slots(&inner)?.0.len(),
            _ => return None,
        };
        if perasc_check::proto_rc() == "r2" {
            return self.value_as(state, rc, Mode::Borrow).into_iter().next();
        }
        let bytes = Expr {
            kind: ExprKind::Const(Const::Int(16 + 8 * i64::try_from(slots).ok()?)),
            ty: Ty::I32,
        };
        let holder = self.holder(rc);
        if holder.is_none()
            && let Some([local]) = self.place(state, rc).as_deref()
        {
            let unique = Expr {
                kind: ExprKind::Buf(
                    BufOp::Runtime(RuntimeFn::SharedUnique),
                    vec![bytes, heap::moved(state, *local)],
                ),
                ty: Ty::Owned,
            };
            state.pre.push(Stmt::Set(*local, unique));
            state.live.insert(local.0);
            return Some(heap::borrowed(*local));
        }
        match holder? {
            Holder::Shared(parent, offset) => {
                let Some(CheckTy::Rc(outer)) = self.check_type(parent) else {
                    return None;
                };
                let slot = *self.shared_slots(&outer)?.1.get(offset)?;
                let parent = self.unique_handle(state, parent)?;
                let at = Expr {
                    kind: ExprKind::Const(Const::Int(i64::from(slot))),
                    ty: Ty::I32,
                };
                state.pre.push(Stmt::Expr(Expr {
                    kind: ExprKind::Buf(
                        BufOp::Runtime(RuntimeFn::SharedUniqueSlot),
                        vec![bytes, parent.clone(), at],
                    ),
                    ty: Ty::Void,
                }));
                Some(Expr {
                    kind: ExprKind::Env(EnvOp::LoadHandle(slot), vec![parent]),
                    ty: Ty::Borrowed,
                })
            }
            Holder::Element(array, index, offset) => {
                let column = self
                    .value_as(state, array, Mode::Borrow)
                    .into_iter()
                    .nth(offset)?;
                let index = self.expr(state, index);
                let index = state.spill(index);
                let len = Expr {
                    kind: ExprKind::Buf(BufOp::Len, vec![column.clone()]),
                    ty: Ty::I32,
                };
                state
                    .pre
                    .push(Stmt::Buf(BufStmt::CheckIndex, vec![index.clone(), len]));
                state.pre.push(Stmt::Expr(Expr {
                    kind: ExprKind::Buf(
                        BufOp::Runtime(RuntimeFn::SharedUniqueElem),
                        vec![bytes, column.clone(), index.clone()],
                    ),
                    ty: Ty::Void,
                }));
                Some(Expr {
                    kind: ExprKind::Buf(BufOp::Get(crate::Elem::Handle), vec![column, index]),
                    ty: Ty::Borrowed,
                })
            }
        }
    }

    /// Where a shared value that is no variable is held: a leaf of another
    /// shared value, `a.next`, or a column of an element, `fleet[i].mesh`.
    fn holder(&self, rc: ExprId) -> Option<Holder> {
        use perasc_syntax::ast::ExprKind as Ast;
        let mut at = rc;
        let mut offset = 0;
        loop {
            match &self.ast.expr(at).kind {
                Ast::Field { base, name } => {
                    let name = self.ast.name(name.symbol);
                    match self.check_type(*base)? {
                        CheckTy::Rc(inner) => {
                            let (o, _, _) = self.field(&inner, name)?;
                            return Some(Holder::Shared(*base, offset + o));
                        }
                        ty => {
                            let (o, _, _) = self.field(&ty, name)?;
                            offset += o;
                            at = *base;
                        }
                    }
                }
                Ast::Index { base, index } => return Some(Holder::Element(*base, *index, offset)),
                Ast::Paren(inner) => at = *inner,
                _ => return None,
            }
        }
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
        let handle = if self.analysis.changed.contains(&id) {
            let handle = self.unique_handle(state, base);
            if handle.is_none() {
                self.unsupported(self.ast.expr(id).span, "rc-write");
            }
            handle
        } else {
            self.value_as(state, base, Mode::Borrow).into_iter().next()
        };
        let picked = self.field(&inner, self.ast.name(name));
        let (Some(handle), Some((offset, len, _))) = (handle, picked) else {
            return Vec::new();
        };
        let (offset, len) = self.narrowed_leaves(id, offset, len);
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

/// Where a shared value that is no variable is held.
enum Holder {
    /// A leaf of the value another shared value holds: that one, and the
    /// leaf.
    Shared(ExprId, usize),
    /// An element of an array: the array, the index, and the column.
    Element(ExprId, ExprId, usize),
}
