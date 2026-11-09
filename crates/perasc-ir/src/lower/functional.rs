//! The methods of arrays that take an arrow function, compiled in place: a
//! loop over the elements whose body is the function's.
//!
//! `map` and `filter` fill new buffers, one per column; `reduce` keeps an
//! accumulator in locals; `find`, `findIndex`, `some` and `every` stop at the
//! first element that decides. The function's parameters lend the elements,
//! as `for…of` does: an element holding buffers is read where it is used.
//!
//! `min`, `max` and `clamp` on numbers are here too: a comparison and a
//! choice, in temporaries.

use perasc_check::ty::Ty as CheckTy;
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, Ident};

use super::heap::Mode;
use super::{Binding, FnState, Lowering};
use crate::{BinOp, BufOp, BufStmt, Const, Elem, Expr, ExprKind, LocalId, Stmt, Ty, UnOp};

/// What the loop builds, per method.
enum Goal {
    /// `map`: the function's value, pushed into new buffers.
    Map(Vec<(LocalId, Elem)>),
    /// `filter`: the elements kept, pushed into new buffers.
    Filter(Vec<(LocalId, Elem)>),
    /// `reduce`: the accumulator, given the function's value at each turn.
    Reduce(Vec<LocalId>),
    /// `find` and `findIndex`: the index of the first element that matches.
    FindIndex(LocalId),
    /// `some` (true) and `every` (false): the answer, decided by the first
    /// element whose test is `some`.
    Any(LocalId, bool),
}

impl Lowering<'_> {
    /// `xs.method((x) => …)`, as scalars; `None` when `method` takes no
    /// function.
    pub(super) fn function_method(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        method: &str,
        args: &[ExprId],
        mode: Mode,
        array: (&[Elem], &[Expr]),
    ) -> Option<Vec<Expr>> {
        let (columns, buffers) = array;
        if !perasc_check::stdlib_methods().contains(&method) {
            return None;
        }
        let &lambda = args.first()?;
        let AstExpr::Lambda { params, body } = &self.ast.expr(lambda).kind else {
            return Some(Vec::new());
        };
        let (params, body) = (params.clone(), *body);
        let element = match self.check_type(base) {
            Some(CheckTy::Array(inner)) => *inner,
            _ => return Some(Vec::new()),
        };
        let first = buffers.first()?.clone();
        let n = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(n, length(&first)));
        let n_expr = super::local_expr(state, n);
        let goal = self.goal(state, method, args, lambda, body, columns, &n_expr)?;
        let i = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(i, int(0)));
        let i_expr = super::local_expr(state, i);
        let outer_pre = std::mem::take(&mut state.pre);
        let outer_drops = std::mem::take(&mut state.drops);
        let cond = super::binary(BinOp::Lt, i_expr.clone(), n_expr);
        let step = vec![Stmt::Set(
            i,
            super::binary(BinOp::Add, i_expr.clone(), int(1)),
        )];
        let mut turn = Vec::new();
        let element_param = usize::from(method == "reduce");
        let lowered = self.loop_lowered(
            state,
            Some(cond),
            Vec::new(),
            step,
            &mut turn,
            |this, state| {
                let mut out = Vec::new();
                state.open();
                if let (Goal::Reduce(acc), Some((name, _))) = (&goal, params.first()) {
                    bind_locals(state, name, acc);
                }
                if let Some((name, _)) = params.get(element_param) {
                    this.bind_element(state, name, &element, (columns, buffers), &i_expr, &mut out);
                }
                this.turn(state, &goal, body, buffers, &i_expr, &mut out);
                state.end_statement(&mut out);
                state.close(&mut out);
                out
            },
        );
        state.pre = outer_pre;
        state.drops = outer_drops;
        state.pre.extend(lowered);
        let values = match goal {
            Goal::Map(result) | Goal::Filter(result) => result
                .into_iter()
                .map(|(local, _)| Expr {
                    kind: ExprKind::Local(local),
                    ty: Ty::Owned,
                })
                .collect(),
            Goal::Reduce(acc) => acc.iter().map(|&a| super::local_expr(state, a)).collect(),
            Goal::Any(answer, _) => vec![super::local_expr(state, answer)],
            Goal::FindIndex(index) if method == "findIndex" => {
                vec![super::local_expr(state, index)]
            }
            Goal::FindIndex(index) => Self::found(state, index, columns, buffers),
        };
        Some(Self::fresh(state, values, mode))
    }

    /// What the loop of `method` builds, set up before it.
    #[allow(clippy::too_many_arguments)]
    fn goal(
        &mut self,
        state: &mut FnState,
        method: &str,
        args: &[ExprId],
        lambda: ExprId,
        body: ExprId,
        columns: &[Elem],
        n_expr: &Expr,
    ) -> Option<Goal> {
        let n_expr = n_expr.clone();
        Some(match method {
            "map" => {
                let value_ty = self.check_type(body).unwrap_or(CheckTy::Void);
                let result = self.columns(Some(&CheckTy::Array(Box::new(value_ty))))?;
                Goal::Map(
                    result
                        .into_iter()
                        .map(|elem| (state.owned_temp(new_buffer(n_expr.clone(), elem)), elem))
                        .collect(),
                )
            }
            "filter" => Goal::Filter(
                columns
                    .iter()
                    .map(|&elem| (state.owned_temp(new_buffer(int(0), elem)), elem))
                    .collect(),
            ),
            "reduce" => {
                let start = *args.get(1)?;
                let start_ty = self.check_type(start).unwrap_or(CheckTy::Void);
                let leaves = self.leaves(&start_ty).ok()?;
                if leaves.iter().any(|(_, t)| t.is_handle()) {
                    let span = self.ast.expr(lambda).span;
                    self.unsupported(span, "reduce-heap");
                    return None;
                }
                let values = self.value(state, start);
                let locals: Vec<LocalId> = leaves.iter().map(|(_, t)| state.temp(*t)).collect();
                for (&local, value) in locals.iter().zip(values) {
                    state.pre.push(Stmt::Set(local, value));
                }
                Goal::Reduce(locals)
            }
            "find" | "findIndex" => {
                let index = state.temp(Ty::I32);
                state.pre.push(Stmt::Set(index, int(-1)));
                Goal::FindIndex(index)
            }
            _ => {
                let some = method == "some";
                let answer = state.temp(Ty::Bool);
                state.pre.push(Stmt::Set(answer, boolean(!some)));
                Goal::Any(answer, some)
            }
        })
    }

    /// One turn of the loop: the function's body, and what `goal` does with
    /// its value.
    fn turn(
        &mut self,
        state: &mut FnState,
        goal: &Goal,
        body: ExprId,
        buffers: &[Expr],
        i_expr: &Expr,
        out: &mut Vec<Stmt>,
    ) {
        let this = self;
        match &goal {
            Goal::Map(result) => {
                let values = this.value(state, body);
                state.flush(out);
                for ((local, elem), value) in result.iter().zip(values) {
                    out.push(push(*local, *elem, value));
                }
            }
            Goal::Filter(result) => {
                let keep = this.expr(state, body);
                let keep = state
                    .settle(vec![keep])
                    .pop()
                    .unwrap_or_else(|| boolean(false));
                state.flush(out);
                let then = result
                    .iter()
                    .zip(buffers)
                    .map(|((local, elem), buffer)| {
                        let value = element_at(buffer, i_expr, *elem, true);
                        push(*local, *elem, value)
                    })
                    .collect();
                out.push(Stmt::If {
                    cond: keep,
                    then,
                    otherwise: Vec::new(),
                });
            }
            Goal::Reduce(acc) => {
                let values = this.value(state, body);
                // Each value computed before any is stored: the body may
                // read the accumulator after a field of it changed.
                let temps: Vec<LocalId> = values
                    .into_iter()
                    .map(|v| {
                        let t = state.temp(v.ty);
                        state.pre.push(Stmt::Set(t, v));
                        t
                    })
                    .collect();
                state.flush(out);
                for (&a, t) in acc.iter().zip(temps) {
                    let value = super::local_expr(state, t);
                    out.push(Stmt::Set(a, value));
                }
            }
            Goal::FindIndex(index) => {
                let found = this.expr(state, body);
                let found = state
                    .settle(vec![found])
                    .pop()
                    .unwrap_or_else(|| boolean(false));
                state.flush(out);
                state.end_statement(out);
                let mut then = vec![Stmt::Set(*index, i_expr.clone())];
                Self::leave_loop(state, &mut then);
                out.push(Stmt::If {
                    cond: found,
                    then,
                    otherwise: Vec::new(),
                });
            }
            Goal::Any(answer, some) => {
                let test = this.expr(state, body);
                let test = state
                    .settle(vec![test])
                    .pop()
                    .unwrap_or_else(|| boolean(false));
                state.flush(out);
                // `some` stops at the first true, `every` at the first false.
                let decides = if *some {
                    test
                } else {
                    Expr {
                        kind: ExprKind::Unary(UnOp::Not, Box::new(test)),
                        ty: Ty::Bool,
                    }
                };
                state.end_statement(out);
                let mut then = vec![Stmt::Set(*answer, boolean(*some))];
                Self::leave_loop(state, &mut then);
                out.push(Stmt::If {
                    cond: decides,
                    then,
                    otherwise: Vec::new(),
                });
            }
        }
    }

    /// The option `find` gives, from the index of the element found, or -1:
    /// whether there is one, then a copy of it, or zeros and empty buffers.
    fn found(state: &mut FnState, index: LocalId, columns: &[Elem], buffers: &[Expr]) -> Vec<Expr> {
        let index = super::local_expr(state, index);
        let some = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(
            some,
            super::binary(BinOp::Ge, index.clone(), int(0)),
        ));
        let mut out = vec![super::local_expr(state, some)];
        let mut then = Vec::new();
        for (buffer, &elem) in buffers.iter().zip(columns) {
            if elem == Elem::Handle {
                let value = state.owned_temp(new_buffer(int(0), Elem::U8));
                then.push(super::heap::release_stmt(value));
                then.push(Stmt::Set(value, element_at(buffer, &index, elem, true)));
                out.push(Expr {
                    kind: ExprKind::Local(value),
                    ty: Ty::Owned,
                });
            } else {
                let value = state.temp(elem.ty());
                state.pre.push(Stmt::Set(value, super::zero(elem.ty())));
                then.push(Stmt::Set(value, element_at(buffer, &index, elem, false)));
                out.push(super::local_expr(state, value));
            }
        }
        let cond = super::local_expr(state, some);
        state.pre.push(Stmt::If {
            cond,
            then,
            otherwise: Vec::new(),
        });
        out
    }

    /// Gives the function's parameter the element at index `i`: locals for
    /// numbers, read in place for buffers, which are only lent.
    fn bind_element(
        &self,
        state: &mut FnState,
        name: &Ident,
        element: &CheckTy,
        array: (&[Elem], &[Expr]),
        i: &Expr,
        out: &mut Vec<Stmt>,
    ) {
        let (columns, buffers) = array;
        if columns.contains(&Elem::Handle) {
            let values = buffers
                .iter()
                .zip(columns)
                .map(|(buffer, &elem)| element_at(buffer, i, elem, false))
                .collect();
            if let Some(scope) = state.scopes.last_mut() {
                scope.insert(name.symbol, Binding::Lent(values));
            }
            return;
        }
        let text = self.ast.name(name.symbol);
        let locals = if super::values::is_aggregate(element) {
            let leaves = self.leaves(element).unwrap_or_default();
            state.add_struct(name.symbol, text, &leaves)
        } else {
            let ty = columns.first().map_or(Ty::I32, |e| e.ty());
            vec![state.add(name.symbol, text, ty)]
        };
        for ((&x, buffer), &elem) in locals.iter().zip(buffers).zip(columns) {
            out.push(Stmt::Set(x, element_at(buffer, i, elem, false)));
        }
    }

    /// A `break` out of the loop the function's body is in.
    fn leave_loop(state: &mut FnState, out: &mut Vec<Stmt>) {
        Self::break_stmt(state, out);
        state.gone = false;
    }

    /// `a.min(b)`, `a.max(b)`, `a.clamp(lo, hi)`: the value chosen by a
    /// comparison, in a temporary.
    pub(super) fn ordering(
        &mut self,
        state: &mut FnState,
        receiver: Expr,
        method: &str,
        args: &[ExprId],
    ) -> Option<Expr> {
        let ops: &[BinOp] = match method {
            // Replaced when the other is smaller, or larger.
            "min" => &[BinOp::Lt],
            "max" => &[BinOp::Gt],
            "clamp" => &[BinOp::Lt, BinOp::Gt],
            _ => return None,
        };
        let result = state.temp(receiver.ty);
        state.pre.push(Stmt::Set(result, receiver));
        for (&op, &arg) in ops.iter().zip(args) {
            let bound = self.expr(state, arg);
            let bound_local = state.temp(bound.ty);
            state.pre.push(Stmt::Set(bound_local, bound));
            let current = super::local_expr(state, result);
            let bound = super::local_expr(state, bound_local);
            // `min`: other < current; clamp: current < lo, then current > hi.
            let cond = if method == "clamp" {
                super::binary(op, current, bound.clone())
            } else {
                super::binary(op, bound.clone(), current)
            };
            state.pre.push(Stmt::If {
                cond,
                then: vec![Stmt::Set(result, bound)],
                otherwise: Vec::new(),
            });
        }
        Some(super::local_expr(state, result))
    }
}

/// Names the accumulator of `reduce`.
fn bind_locals(state: &mut FnState, name: &Ident, locals: &[LocalId]) {
    let binding = match locals {
        [one] => Binding::Scalar(*one),
        many => Binding::Struct(many.to_vec()),
    };
    if let Some(scope) = state.scopes.last_mut() {
        scope.insert(name.symbol, binding);
    }
}

/// The element of a column at `i`; a buffer copied when `owned` is asked.
fn element_at(buffer: &Expr, i: &Expr, elem: Elem, owned: bool) -> Expr {
    let value = Expr {
        kind: ExprKind::Buf(BufOp::Get(elem), vec![buffer.clone(), i.clone()]),
        ty: elem.ty(),
    };
    if owned && elem == Elem::Handle {
        Expr {
            kind: ExprKind::Buf(BufOp::Clone, vec![value]),
            ty: Ty::Owned,
        }
    } else {
        value
    }
}

fn push(buffer: LocalId, elem: Elem, value: Expr) -> Stmt {
    Stmt::Buf(
        BufStmt::Push(elem),
        vec![super::heap::borrowed(buffer), value],
    )
}

fn length(buffer: &Expr) -> Expr {
    Expr {
        kind: ExprKind::Buf(BufOp::Len, vec![buffer.clone()]),
        ty: Ty::I32,
    }
}

fn new_buffer(capacity: Expr, elem: Elem) -> Expr {
    Expr {
        kind: ExprKind::Buf(BufOp::New(elem), vec![capacity]),
        ty: Ty::Owned,
    }
}

fn int(value: i64) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(value)),
        ty: Ty::I32,
    }
}

fn boolean(value: bool) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Bool(value)),
        ty: Ty::Bool,
    }
}
