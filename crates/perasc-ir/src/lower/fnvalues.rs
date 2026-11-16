//! Functions as values (decisions 0025 and 0026). A function value is one
//! handle, its environment: the position of its function in the module's
//! table, how many hold it, and the values it captured, copied when it was
//! made. A function of the module, or an arrow function that captures
//! nothing, has a static environment, which costs no allocation. Every
//! function in the table takes its environment last, lent: an arrow
//! function reads its captures from it, and a function of the module used as
//! a value is reached through an adapter that ignores it. A call through a
//! value is an indirect call, whose arguments are lent and whose results the
//! caller takes; a copy of the value adds to its environment's count.

use perasc_check::ty::Ty as CheckTy;
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, LambdaBody, Symbol};

use super::heap::Mode;
use super::{Binding, FnState, Lowering, values};
use crate::{BufOp, EnvOp, Expr, ExprKind, FnId, Function, LocalId, Stmt, Ty, UnOp};

/// A name an arrow function captures, its type, and whether it is compiled:
/// the function rebuilds it from its environment's slots, one per scalar. A
/// capture refused is only named there, so that its uses report nothing more.
pub(super) type Capture = (Symbol, CheckTy, bool);

/// An arrow function used as a value, to lower after the other functions:
/// the function it becomes, its expression, its unit, the type arguments of
/// the function around it, what it captures, and its name.
pub(super) type Arrow = (FnId, ExprId, usize, Vec<CheckTy>, Vec<Capture>, String);

/// A function not lowered yet: its place among the jobs, filled later.
fn placeholder() -> Function {
    Function {
        name: String::new(),
        exported: false,
        job: false,
        param_count: 0,
        locals: Vec::new(),
        ret: Ty::Void,
        more: Vec::new(),
        shape: None,
        body: Vec::new(),
    }
}

/// A scalar as the `i64` of a slot: an integer extended, a float by its
/// bits, a `bool` as 0 or 1 (decision 0026).
fn to_slot(value: Expr) -> Expr {
    let wide = |e: Expr| Expr {
        kind: ExprKind::Convert(Box::new(e)),
        ty: Ty::I64,
    };
    let bits = |e: Expr, ty: Ty| Expr {
        kind: ExprKind::Unary(UnOp::Bits, Box::new(e)),
        ty,
    };
    match value.ty {
        Ty::I64 => value,
        Ty::F64 => bits(value, Ty::I64),
        Ty::F32 => wide(bits(value, Ty::I32)),
        _ => wide(value),
    }
}

/// The scalar of type `ty` a slot holds: [`to_slot`] undone.
fn from_slot(slot: Expr, ty: Ty) -> Expr {
    let narrow = |e: Expr, ty: Ty| Expr {
        kind: ExprKind::Convert(Box::new(e)),
        ty,
    };
    let bits = |e: Expr, ty: Ty| Expr {
        kind: ExprKind::Unary(UnOp::Bits, Box::new(e)),
        ty,
    };
    match ty {
        Ty::I64 => slot,
        Ty::F64 => bits(slot, Ty::F64),
        Ty::F32 => bits(narrow(slot, Ty::I32), Ty::F32),
        other => narrow(slot, other),
    }
}

impl Lowering<'_> {
    /// A function value made here, an arrow function: its environment,
    /// static when it captures nothing, else a new one holding what it
    /// captures, copied now. A capture that holds a buffer is not compiled
    /// yet (`closure-heap`).
    pub(super) fn arrow_value(&mut self, state: &mut FnState, id: ExprId, mode: Mode) -> Vec<Expr> {
        let span = self.ast.expr(id).span;
        let found = self.analysis.captures.get(&id).cloned().unwrap_or_default();
        let mut captures = Vec::new();
        let mut slots = Vec::new();
        for (symbol, index) in found {
            // A name the arrow function reads but lowering does not hold as
            // a local, a constant made where it is read, is not a capture.
            let Some(binding) = state.lookup(symbol) else {
                continue;
            };
            let Some(ty) = self
                .analysis
                .bindings
                .get(index)
                .map(|b| self.analysis.subst(&b.ty, &self.args))
            else {
                continue;
            };
            let scalars = self
                .leaves(&ty)
                .is_ok_and(|l| l.iter().all(|(_, t)| !t.is_handle()));
            if !scalars {
                self.unsupported(span, "closure-heap");
                captures.push((symbol, ty, false));
                continue;
            }
            let values = match binding {
                Binding::Scalar(local) => vec![super::local_expr(state, local)],
                Binding::Struct(locals) => locals
                    .iter()
                    .map(|&l| super::local_expr(state, l))
                    .collect(),
                Binding::Lent(values) => values,
            };
            slots.extend(values.into_iter().map(to_slot));
            captures.push((symbol, ty, true));
        }
        let function = self.arrow_ref(id, captures);
        let env = if slots.is_empty() {
            self.static_env(function)
        } else {
            Expr {
                kind: ExprKind::Env(EnvOp::New(function), slots),
                ty: Ty::Owned,
            }
        };
        Self::fresh(state, vec![env], mode)
    }

    /// A function of the module used as a value: the static environment of
    /// its adapter, made once.
    pub(super) fn named_value(
        &mut self,
        state: &mut FnState,
        target: FnId,
        mode: Mode,
    ) -> Vec<Expr> {
        let adapter = if let Some(&adapter) = self.adapters.get(&target) {
            adapter
        } else {
            let position = u32::try_from(self.jobs.len()).unwrap_or(u32::MAX);
            let adapter = FnId(self.job_base + position);
            self.jobs.push(placeholder());
            self.adapters.insert(target, adapter);
            adapter
        };
        let env = self.static_env(adapter);
        Self::fresh(state, vec![env], mode)
    }

    /// The static environment of a function that captures nothing, written
    /// below the heap when the module is instantiated, 16 bytes; while there
    /// is room, as for the literals only read, else a new one, counted.
    fn static_env(&mut self, function: FnId) -> Expr {
        let at = if let Some(&at) = self.static_envs.get(&function) {
            Some(at)
        } else if self.static_top + 16 <= crate::STATIC_END {
            let at = self.static_top;
            self.static_top += 16;
            self.static_envs.insert(function, at);
            Some(at)
        } else {
            None
        };
        let op = at.map_or(EnvOp::New(function), |at| EnvOp::Static(function, at));
        Expr {
            kind: ExprKind::Env(op, Vec::new()),
            ty: Ty::Owned,
        }
    }

    /// The adapter through which a function of the module is called as a
    /// value: the function's own parameters, given on, then its environment,
    /// ignored; the function's results, given back.
    pub(super) fn adapter(target: &Function, id: FnId) -> Function {
        let mut locals: Vec<crate::Local> = target.params().cloned().collect();
        locals.push(crate::Local {
            name: "env".to_owned(),
            ty: Ty::Borrowed,
        });
        let args: Vec<Expr> = target
            .params()
            .enumerate()
            .map(|(i, p)| Expr {
                kind: ExprKind::Local(LocalId(u32::try_from(i).unwrap_or(u32::MAX))),
                ty: p.ty,
            })
            .collect();
        let results = target.results();
        let body = match results.as_slice() {
            [] => vec![
                Stmt::Expr(Expr {
                    kind: ExprKind::Call(id, args),
                    ty: Ty::Void,
                }),
                Stmt::Return(None),
            ],
            [one] => vec![Stmt::Return(Some(Expr {
                kind: ExprKind::Call(id, args),
                ty: *one,
            }))],
            many => {
                let first = locals.len();
                let targets: Vec<LocalId> = many
                    .iter()
                    .enumerate()
                    .map(|(k, &ty)| {
                        locals.push(crate::Local {
                            name: "tmp".to_owned(),
                            ty,
                        });
                        LocalId(u32::try_from(first + k).unwrap_or(u32::MAX))
                    })
                    .collect();
                let values = targets
                    .iter()
                    .zip(many)
                    .map(|(&t, &ty)| Expr {
                        kind: if ty == Ty::Owned {
                            ExprKind::Move(t)
                        } else {
                            ExprKind::Local(t)
                        },
                        ty,
                    })
                    .collect();
                vec![
                    Stmt::CallMany {
                        targets,
                        callee: id,
                        args,
                    },
                    Stmt::ReturnMany(values),
                ]
            }
        };
        Function {
            name: format!("{}__value", target.name),
            exported: false,
            job: false,
            param_count: target.param_count + 1,
            locals,
            ret: target.ret,
            more: target.more.clone(),
            shape: None,
            body,
        }
    }

    /// An arrow function used as a value: a function of its own, lowered
    /// after the others, in a place kept for it now.
    fn arrow_ref(&mut self, id: ExprId, captures: Vec<Capture>) -> FnId {
        let position = u32::try_from(self.jobs.len()).unwrap_or(u32::MAX);
        let function = FnId(self.job_base + position);
        self.jobs.push(placeholder());
        // Named after the function that makes it, numbered in the module.
        let name = format!("{}__arrow{}", self.current, self.arrows.len());
        self.arrows
            .push((function, id, self.unit, self.args.clone(), captures, name));
        function
    }

    /// The locals of the names an arrow function captures, each set from its
    /// environment's slots, in order; a capture refused is only named.
    fn read_captures(&self, state: &mut FnState, captures: &[Capture], env: LocalId) -> Vec<Stmt> {
        let mut out = Vec::new();
        let mut slot = 0;
        for (symbol, ty, compiled) in captures {
            let text = self.ast.name(*symbol).to_owned();
            let leaves = self.leaves(ty).unwrap_or_default();
            if !compiled {
                let lent: Vec<(String, Ty)> = leaves
                    .into_iter()
                    .map(|(path, t)| (path, if t.is_handle() { Ty::Borrowed } else { t }))
                    .collect();
                state.add_struct(*symbol, &text, &lent);
                continue;
            }
            let locals = if values::is_aggregate(ty) {
                state.add_struct(*symbol, &text, &leaves)
            } else {
                let ty = leaves.first().map_or(Ty::Void, |(_, t)| *t);
                vec![state.add(*symbol, &text, ty)]
            };
            for (local, (_, ty)) in locals.into_iter().zip(&leaves) {
                let read = Expr {
                    kind: ExprKind::Env(
                        EnvOp::Load(slot),
                        vec![Expr {
                            kind: ExprKind::Local(env),
                            ty: Ty::Borrowed,
                        }],
                    ),
                    ty: Ty::I64,
                };
                out.push(Stmt::Set(local, from_slot(read, *ty)));
                slot += 1;
            }
        }
        out
    }

    /// The function an arrow function used as a value is: its parameters,
    /// lent, then its environment, lent, its value its result. It starts by
    /// reading what it captured from its environment's slots, into locals of
    /// the names it captured.
    pub(super) fn arrow_function(
        &mut self,
        id: ExprId,
        captures: &[Capture],
        name: String,
    ) -> Function {
        let expr = self.ast.expr(id);
        let span = expr.span;
        let AstExpr::Lambda { params, body, .. } = &expr.kind else {
            return placeholder();
        };
        let (param_types, ret) = match self.check_type(id) {
            Some(CheckTy::Callback(params, ret)) => (params, *ret),
            _ => (Vec::new(), CheckTy::Void),
        };
        let outer = (self.current.clone(), self.body_span, self.in_arrow);
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
        let env = state.local("env", Ty::Borrowed);
        state.fixed.insert(env);
        let param_count = u32::try_from(state.locals.len()).unwrap_or(u32::MAX);
        let mut out = self.read_captures(&mut state, captures, env);
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
        let value = self.callee_env(state, callee);
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
            let value = self.callee_env(state, id);
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

    /// The environment a call through a value goes through, lent: a
    /// variable's, a parameter's, an element's, or a new one's, held until
    /// the end of the statement.
    fn callee_env(&mut self, state: &mut FnState, callee: ExprId) -> Expr {
        self.value_as(state, callee, Mode::Borrow)
            .into_iter()
            .next()
            .unwrap_or_else(|| Expr {
                kind: ExprKind::Env(EnvOp::Zero, Vec::new()),
                ty: Ty::Owned,
            })
    }

    /// A value given to a new owner without being moved: a function value,
    /// which copies freely, or what holds one. Each environment it lends is
    /// held once more ([`BufOp::Clone`], decision 0026).
    pub(super) fn retained(values: Vec<Expr>) -> Vec<Expr> {
        values
            .into_iter()
            .map(|e| {
                if e.ty == Ty::Borrowed {
                    Expr {
                        kind: ExprKind::Buf(BufOp::Clone, vec![e]),
                        ty: Ty::Owned,
                    }
                } else {
                    e
                }
            })
            .collect()
    }

    /// Whether a value of this type holds a function value, and copies
    /// freely: a function type, or a structure, an option, a result or an
    /// enumeration holding one, and no other buffer.
    pub(super) fn holds_fn(&self, ty: &CheckTy) -> bool {
        let Ok(leaves) = self.leaves(ty) else {
            return false;
        };
        let handles = leaves.iter().filter(|(_, t)| t.is_handle()).count();
        handles > 0 && handles == self.fn_leaves(ty, 0)
    }

    /// How many function values a value of this type holds, by its leaves.
    fn fn_leaves(&self, ty: &CheckTy, depth: usize) -> usize {
        if depth > 64 {
            return 0;
        }
        let deeper = depth + 1;
        match ty {
            CheckTy::Callback(..) => 1,
            CheckTy::Struct(id, _) => self.analysis.structs.get(id.0).map_or(0, |(_, fields)| {
                fields.iter().map(|(_, f)| self.fn_leaves(f, deeper)).sum()
            }),
            CheckTy::Option(inner) => self.fn_leaves(inner, deeper),
            CheckTy::Result(ok, err) => self.fn_leaves(ok, deeper) + self.fn_leaves(err, deeper),
            CheckTy::Enum(id, _) => self.analysis.enums.get(id.0).map_or(0, |(_, variants)| {
                variants
                    .iter()
                    .flat_map(|(_, fields)| fields.iter())
                    .map(|f| self.fn_leaves(f, deeper))
                    .sum()
            }),
            _ => 0,
        }
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
