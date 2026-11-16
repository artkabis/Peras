//! Functions as values (decision 0025): a function of the module, or an arrow
//! function that captures nothing, is an `i32`, its position in the module's
//! table of functions; a call through one is an indirect call, whose
//! arguments are lent and whose results the caller takes.

use perasc_check::ty::Ty as CheckTy;
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, LambdaBody};

use super::heap::Mode;
use super::{FnState, Lowering, values};
use crate::{Expr, ExprKind, FnId, Function, LocalId, Stmt, Ty};

impl Lowering<'_> {
    /// An arrow function used as a value: a function of its own, lowered
    /// after the others, in a place kept for it now.
    pub(super) fn arrow_ref(&mut self, id: ExprId) -> Expr {
        let position = u32::try_from(self.jobs.len()).unwrap_or(u32::MAX);
        let function = FnId(self.job_base + position);
        self.jobs.push(Function {
            name: String::new(),
            exported: false,
            job: false,
            param_count: 0,
            locals: Vec::new(),
            ret: Ty::Void,
            more: Vec::new(),
            shape: None,
            body: Vec::new(),
        });
        self.arrows.push((function, id, self.unit));
        Expr {
            kind: ExprKind::FnRef(function),
            ty: Ty::I32,
        }
    }

    /// The function an arrow function used as a value is: its parameters
    /// lent, its value its result. A name it does not declare is one it
    /// captures, which is not compiled yet (`closure`).
    pub(super) fn arrow_function(&mut self, id: ExprId) -> Function {
        let expr = self.ast.expr(id);
        let span = expr.span;
        let AstExpr::Lambda { params, body, .. } = &expr.kind else {
            return Function {
                name: String::new(),
                exported: false,
                job: false,
                param_count: 0,
                locals: Vec::new(),
                ret: Ty::Void,
                more: Vec::new(),
                shape: None,
                body: Vec::new(),
            };
        };
        let (param_types, ret) = match self.check_type(id) {
            Some(CheckTy::Callback(params, ret)) => (params, *ret),
            _ => (Vec::new(), CheckTy::Void),
        };
        let outer = (self.current.clone(), self.body_span, self.in_arrow);
        let name = format!("{}__arrow{}", self.current, self.arrows.len());
        self.current.clone_from(&name);
        self.body_span = span;
        self.in_arrow = true;
        let mut state = FnState::new();
        for ((param, _), ty) in params.iter().zip(&param_types) {
            let text = self.ast.name(param.symbol).to_owned();
            let leaves: Vec<(String, Ty)> = self
                .leaves(ty)
                .unwrap_or_default()
                .into_iter()
                .map(|(path, t)| {
                    if t == Ty::Owned {
                        (path, Ty::Borrowed)
                    } else {
                        (path, t)
                    }
                })
                .collect();
            if values::is_aggregate(ty) {
                let ids = state.add_struct(param.symbol, &text, &leaves);
                state.fixed.extend(ids);
            } else {
                let ty = leaves.first().map_or(Ty::Void, |(_, t)| *t);
                let local = state.add(param.symbol, &text, ty);
                state.fixed.insert(local);
            }
        }
        let param_count = u32::try_from(state.locals.len()).unwrap_or(u32::MAX);
        let results: Vec<Ty> = if ret == CheckTy::Void {
            Vec::new()
        } else {
            self.leaves(&ret)
                .map(|l| l.into_iter().map(|(_, t)| t).collect())
                .unwrap_or_default()
        };
        if matches!(ret, CheckTy::Option(_) | CheckTy::Result(..)) {
            state.ret_option = Some(ret.clone());
        }
        let mut out = Vec::new();
        match body {
            LambdaBody::Expr(value) if ret == CheckTy::Void => {
                let values = self.value_as(&mut state, *value, Mode::Borrow);
                state.flush(&mut out);
                for value in values {
                    if !super::heap::pure(&value) {
                        out.push(Stmt::Expr(value));
                    }
                }
                state.end_statement(&mut out);
                state.release_scopes(0, &mut out);
            }
            LambdaBody::Expr(value) => self.return_stmt(&mut state, Some(*value), &mut out),
            LambdaBody::Block(block) => {
                self.block(&mut state, &block.stmts, &mut out);
                if !state.gone {
                    state.release_scopes(0, &mut out);
                }
            }
        }
        (self.current, self.body_span, self.in_arrow) = outer;
        Function {
            name,
            exported: false,
            job: false,
            param_count,
            locals: state.locals,
            ret: results.first().copied().unwrap_or(Ty::Void),
            more: results.get(1..).map(<[Ty]>::to_vec).unwrap_or_default(),
            shape: None,
            body: out,
        }
    }

    /// Whether a call goes through a function value: its callee is a
    /// variable, a parameter, or any expression other than the name of a
    /// function or a method.
    pub(super) fn is_value_callee(&self, state: &FnState, callee: ExprId) -> bool {
        match &self.ast.expr(callee).kind {
            AstExpr::Name(symbol) => state.lookup(*symbol).is_some(),
            AstExpr::Field { .. } => false,
            _ => true,
        }
    }

    /// `f(a, b)` through the value `f`: the value first, then the arguments,
    /// lent; its results as [`Self::call_results`] gives them.
    pub(super) fn call_value(
        &mut self,
        state: &mut FnState,
        callee: ExprId,
        args: &[ExprId],
    ) -> Vec<Expr> {
        let ret = match self.check_type(callee) {
            Some(CheckTy::Callback(_, ret)) => *ret,
            _ => CheckTy::Void,
        };
        let value = self.expr(state, callee);
        let value = state.spill(value);
        let mut lent = Vec::new();
        for &arg in args {
            let mark = state.pre.len();
            let values = self.value_as(state, arg, Mode::Borrow);
            let mut values = values;
            state.keep_before(mark, &mut values);
            lent.extend(values);
        }
        self.indirect_results(state, value, lent, &ret)
    }

    /// The call a function given by its value to `map` and the others stands
    /// for, with this turn's element: a direct call when it names a function
    /// of the module, as `(x) => twice(x)` would be.
    pub(super) fn apply(&mut self, state: &mut FnState, id: ExprId) -> Vec<Expr> {
        let Some((args, ret)) = self.applied.remove(&id) else {
            return Vec::new();
        };
        let direct = match &self.ast.expr(id).kind {
            AstExpr::Name(symbol) if state.lookup(*symbol).is_none() => {
                self.fns.get(symbol).copied()
            }
            _ => None,
        };
        let values = if let Some(function) = direct {
            self.call_results(state, function, args.clone(), &[])
        } else {
            let value = self.expr(state, id);
            let value = state.spill(value);
            self.indirect_results(state, value, args.clone(), &ret)
        };
        self.applied.insert(id, (args, ret));
        values
    }

    /// [`Self::apply`], as one scalar.
    pub(super) fn applied_expr(&mut self, state: &mut FnState, id: ExprId) -> Expr {
        let ty = match self.applied.get(&id) {
            Some((_, ret)) => Self::ir_type(ret).unwrap_or(Ty::Void),
            None => Ty::Void,
        };
        self.apply(state, id)
            .into_iter()
            .next()
            .unwrap_or_else(|| super::zero(ty))
    }

    /// An indirect call of `value` with these scalars, as its results.
    pub(super) fn indirect_results(
        &mut self,
        state: &mut FnState,
        value: Expr,
        args: Vec<Expr>,
        ret: &CheckTy,
    ) -> Vec<Expr> {
        let results: Vec<Ty> = if *ret == CheckTy::Void {
            Vec::new()
        } else {
            self.leaves(ret)
                .map(|l| l.into_iter().map(|(_, t)| t).collect())
                .unwrap_or_default()
        };
        match results.as_slice() {
            [] => {
                state.pre.push(Stmt::Expr(Expr {
                    kind: ExprKind::CallIndirect(Box::new(value), args),
                    ty: Ty::Void,
                }));
                Vec::new()
            }
            [one] => vec![Expr {
                kind: ExprKind::CallIndirect(Box::new(value), args),
                ty: *one,
            }],
            many => {
                let targets: Vec<LocalId> = many.iter().map(|&ty| state.temp(ty)).collect();
                state.pre.push(Stmt::CallManyIndirect {
                    targets: targets.clone(),
                    callee: value,
                    args,
                });
                targets
                    .iter()
                    .map(|&t| super::local_expr(state, t))
                    .collect()
            }
        }
    }
}
