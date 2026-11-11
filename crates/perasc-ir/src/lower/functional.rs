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
    /// `filter` on a temporary array: whether each element is kept, pushed
    /// into an array of `bool`; the runtime then keeps them in place
    /// (`arr_keep`), and the temporary becomes the result, with no copy.
    Keep(LocalId),
    /// `reduce`: the accumulator, given the function's value at each turn.
    Reduce(Vec<LocalId>),
    /// `find` and `findIndex`: the index of the first element that matches.
    FindIndex(LocalId),
    /// `some` (true) and `every` (false): the answer, decided by the first
    /// element whose test is `some`.
    Any(LocalId, bool),
    /// `filter` whose result only gives its length: the elements kept,
    /// counted, with no array.
    Count(LocalId),
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
        // Statements in an arrow function are refused in an owned module.
        let AstExpr::Lambda {
            params,
            body: perasc_syntax::ast::LambdaBody::Expr(body),
            ..
        } = &self.ast.expr(lambda).kind
        else {
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
        // A temporary array, released after the statement: no one else sees
        // it, so `filter` may change it and give it as its result.
        let temporaries = temporaries(state, buffers);
        let counted = method == "filter" && state.counted == Some(base);
        if counted {
            state.counted = None;
        }
        let goal = match &temporaries {
            _ if counted => {
                let count = state.temp(Ty::I32);
                state.pre.push(Stmt::Set(count, int(0)));
                Goal::Count(count)
            }
            Some(_) if method == "filter" => {
                Goal::Keep(state.owned_temp(new_buffer(n_expr.clone(), Elem::Bool)))
            }
            _ => self.goal(state, method, args, lambda, body, columns, &n_expr)?,
        };
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
            Goal::Keep(flags) => {
                let taken = temporaries.unwrap_or_default();
                return Some(Self::kept(state, flags, buffers, taken, mode));
            }
            Goal::Map(result) | Goal::Filter(result) => result
                .into_iter()
                .map(|(local, _)| Expr {
                    kind: ExprKind::Local(local),
                    ty: Ty::Owned,
                })
                .collect(),
            Goal::Reduce(acc) => acc.iter().map(|&a| super::local_expr(state, a)).collect(),
            Goal::Any(answer, _) | Goal::Count(answer) => {
                vec![super::local_expr(state, answer)]
            }
            Goal::FindIndex(index) if method == "findIndex" => {
                vec![super::local_expr(state, index)]
            }
            Goal::FindIndex(index) => Self::found(state, index, columns, buffers),
        };
        Some(Self::fresh(state, values, mode))
    }

    /// `filter` on temporaries, after the loop gathered its flags: the
    /// runtime keeps the elements in place, and the temporaries are the
    /// result, or a copy of them when they are views leaving the statement.
    fn kept(
        state: &mut FnState,
        flags: LocalId,
        buffers: &[Expr],
        taken: Vec<LocalId>,
        mode: Mode,
    ) -> Vec<Expr> {
        for buffer in buffers {
            state.pre.push(Stmt::Expr(Expr {
                kind: ExprKind::Buf(
                    BufOp::Runtime(crate::RuntimeFn::ArrKeep),
                    vec![buffer.clone(), super::heap::borrowed(flags)],
                ),
                ty: Ty::Void,
            }));
        }
        state.drops.push(flags);
        // Views that leave the statement: the result is a copy, an ordinary
        // array; the views stay temporaries.
        if mode == Mode::Consume && taken.iter().any(|t| state.views.contains_key(t)) {
            let copies = taken
                .into_iter()
                .map(|t| Expr {
                    kind: ExprKind::Buf(BufOp::Clone, vec![super::heap::borrowed(t)]),
                    ty: Ty::Owned,
                })
                .collect();
            return Self::fresh(state, copies, mode);
        }
        // The temporaries are the result now, not released here.
        state.drops.retain(|d| !taken.contains(d));
        let values = taken
            .into_iter()
            .map(|local| Expr {
                kind: ExprKind::Local(local),
                ty: Ty::Owned,
            })
            .collect();
        Self::fresh(state, values, mode)
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
            Goal::Keep(flags) => {
                let keep = this.test(state, body, out);
                out.push(push(*flags, Elem::Bool, keep));
            }
            Goal::Filter(result) => {
                let keep = this.test(state, body, out);
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
            Goal::Count(count) => {
                let keep = this.test(state, body, out);
                let count_expr = super::local_expr(state, *count);
                out.push(Stmt::If {
                    cond: keep,
                    then: vec![Stmt::Set(
                        *count,
                        super::binary(BinOp::Add, count_expr, int(1)),
                    )],
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
                let found = this.test(state, body, out);
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
                let test = this.test(state, body, out);
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

    /// The function's body as a test, its statements flushed into `out`.
    fn test(&mut self, state: &mut FnState, body: ExprId, out: &mut Vec<Stmt>) -> Expr {
        let value = self.expr(state, body);
        let value = state
            .settle(vec![value])
            .pop()
            .unwrap_or_else(|| boolean(false));
        state.flush(out);
        value
    }

    /// A method of strings, or `join` on an array of strings: a function of
    /// the runtime, its operands lent; `None` for another method.
    pub(super) fn text_method(
        &mut self,
        state: &mut FnState,
        receiver: &Expr,
        method: &str,
        args: &[ExprId],
        mode: Mode,
        views: bool,
    ) -> Option<Vec<Expr>> {
        use crate::RuntimeFn as F;
        let f = match method {
            "split" if views => F::SplitViews,
            "indexOf" | "includes" => F::IndexOf,
            "startsWith" => F::StartsWith,
            "endsWith" => F::EndsWith,
            "slice" => F::Slice,
            "trim" => F::Trim,
            "toUpperCase" => F::Upper,
            "toLowerCase" => F::Lower,
            "repeat" => F::Repeat,
            "replace" => F::Replace,
            "replaceAll" => F::ReplaceAll,
            "split" => F::Split,
            "join" => F::Join,
            _ => return None,
        };
        let mut operands = vec![receiver.clone()];
        for &arg in args {
            let value = if self.is_aggregate(arg) {
                self.value_as(state, arg, Mode::Borrow).into_iter().next()?
            } else {
                self.expr(state, arg)
            };
            operands.push(value);
        }
        if f == F::IndexOf {
            operands.push(int(0));
        }
        let call = Expr {
            kind: ExprKind::Buf(BufOp::Runtime(f), operands),
            ty: f.result(),
        };
        let value = if method == "includes" {
            super::binary(BinOp::Ge, call, int(0))
        } else {
            call
        };
        Some(Self::fresh(state, vec![value], mode))
    }

    /// `slice`, `concat`, `reverse`, `sort`, `includes` and `indexOf` on an
    /// array, column by column; `None` for another method.
    pub(super) fn column_method(
        &mut self,
        state: &mut FnState,
        method: &str,
        args: &[ExprId],
        mode: Mode,
        array: (&[Elem], &[Expr]),
    ) -> Option<Vec<Expr>> {
        use crate::RuntimeFn as F;
        let (columns, buffers) = array;
        let runtime = |f: F, operands: Vec<Expr>| Expr {
            kind: ExprKind::Buf(BufOp::Runtime(f), operands),
            ty: f.result(),
        };
        match method {
            "slice" => {
                // Each bound once, for every column.
                let bounds: Vec<Expr> = args
                    .iter()
                    .map(|&a| {
                        let value = self.expr(state, a);
                        let t = state.temp(Ty::I32);
                        state.pre.push(Stmt::Set(t, value));
                        super::local_expr(state, t)
                    })
                    .collect();
                let values = buffers
                    .iter()
                    .map(|b| {
                        let mut operands = vec![b.clone()];
                        operands.extend(bounds.iter().cloned());
                        runtime(F::ArrSlice, operands)
                    })
                    .collect();
                Some(Self::fresh(state, values, mode))
            }
            "concat" => {
                let other = self.value_as(state, *args.first()?, Mode::Borrow);
                let values = buffers
                    .iter()
                    .zip(other)
                    .map(|(a, b)| runtime(F::ArrConcat, vec![a.clone(), b]))
                    .collect();
                Some(Self::fresh(state, values, mode))
            }
            "reverse" => {
                for b in buffers {
                    state
                        .pre
                        .push(Stmt::Expr(runtime(F::ArrReverse, vec![b.clone()])));
                }
                Some(Vec::new())
            }
            "sort" => {
                let kind = match columns.first()? {
                    Elem::I32 => 0,
                    Elem::I64 => 1,
                    Elem::F32 => 2,
                    _ => 3,
                };
                let buffer = buffers.first()?.clone();
                state
                    .pre
                    .push(Stmt::Expr(runtime(F::ArrSort, vec![buffer, int(kind)])));
                Some(Vec::new())
            }
            "includes" | "indexOf" => {
                let buffer = buffers.first()?.clone();
                let &arg = args.first()?;
                let index = if columns.first() == Some(&Elem::Handle) {
                    let needle = self.value_as(state, arg, Mode::Borrow).into_iter().next()?;
                    runtime(F::ArrIndexOfStr, vec![buffer, needle])
                } else {
                    self.search(state, &buffer, *columns.first()?, arg)
                };
                let value = if method == "includes" {
                    super::binary(BinOp::Ge, index, int(0))
                } else {
                    index
                };
                Some(vec![value])
            }
            _ => None,
        }
    }

    /// The index of the first element of a column of numbers equal to the
    /// value of `arg`, or -1: a loop that stops when it finds it.
    fn search(&mut self, state: &mut FnState, buffer: &Expr, elem: Elem, arg: ExprId) -> Expr {
        let value = self.expr(state, arg);
        let wanted = state.temp(value.ty);
        state.pre.push(Stmt::Set(wanted, value));
        let index = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(index, int(-1)));
        let i = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(i, int(0)));
        let i_expr = super::local_expr(state, i);
        let found = super::binary(
            BinOp::Eq,
            element_at(buffer, &i_expr, elem, false),
            super::local_expr(state, wanted),
        );
        state.pre.push(Stmt::Loop {
            cond: Some(super::binary(BinOp::Lt, i_expr.clone(), length(buffer))),
            body: vec![Stmt::If {
                cond: found,
                then: vec![Stmt::Set(index, i_expr.clone()), Stmt::Break],
                otherwise: Vec::new(),
            }],
            step: vec![Stmt::Set(i, super::binary(BinOp::Add, i_expr, int(1)))],
        });
        super::local_expr(state, index)
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

/// The locals of `buffers`, when every one is an owned temporary of the
/// statement, released at its end.
fn temporaries(state: &FnState, buffers: &[Expr]) -> Option<Vec<LocalId>> {
    buffers
        .iter()
        .map(|b| match b.kind {
            ExprKind::Borrow(local) if state.drops.contains(&local) => Some(local),
            _ => None,
        })
        .collect()
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
