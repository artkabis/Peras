//! Decision 0036, option i1: small functions inlined.
//!
//! A function whose body is one `return e`, `e` of at most [`MAX_NODES`] nodes,
//! reading only its parameters, numbers or booleans all, is replaced at each
//! call by `e`, each parameter bound to its argument. The arguments are
//! evaluated once and in order:
//!
//! - an argument that is a variable or a constant takes its parameter's place;
//! - one that cannot trap, call or loop ([`is_simple`]), read once by `e`, too:
//!   evaluated later, it gives the same value and does nothing else;
//! - any other is computed into a new variable just before the statement, which
//!   is only done when everything the statement evaluates before the call is
//!   simple, and the call is not on the right of `&&` or `||`, nor in the
//!   condition of a loop: otherwise the call stays.

use crate::opt::is_simple;
use crate::{BinOp, Expr, ExprKind, FnId, Function, Local, LocalId, Module, Stmt, Ty};

/// The largest body inlined, in nodes of its expression.
const MAX_NODES: usize = 40;
/// Small functions calling small functions are inlined this deep.
const MAX_DEPTH: usize = 4;

/// The expression of each small function, by index; `None` for the others.
pub fn small_functions(functions: &[Function]) -> Vec<Option<Expr>> {
    functions
        .iter()
        .enumerate()
        .map(|(index, f)| {
            let id = FnId(u32::try_from(index).unwrap_or(u32::MAX));
            let [Stmt::Return(Some(e))] = f.body.as_slice() else {
                return None;
            };
            let scalars = f.locals.iter().all(|l| is_scalar(l.ty)) && is_scalar(f.ret);
            (scalars
                && f.more.is_empty()
                && !f.job
                && nodes(e) <= MAX_NODES
                && only_params(e, f.param_count, id))
            .then(|| e.clone())
        })
        .collect()
}

const fn is_scalar(ty: Ty) -> bool {
    !ty.is_handle() && !matches!(ty, Ty::Void)
}

fn nodes(e: &Expr) -> usize {
    1 + match &e.kind {
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => nodes(x),
        ExprKind::Binary(_, a, b) => nodes(a) + nodes(b),
        ExprKind::Call(_, args) => args.iter().map(nodes).sum(),
        _ => 0,
    }
}

/// `e` reads only parameters, calls no function but others of the module, never
/// itself, and touches no block, module variable or function value.
fn only_params(e: &Expr, params: u32, id: FnId) -> bool {
    match &e.kind {
        ExprKind::Const(_) => true,
        ExprKind::Local(LocalId(l)) => *l < params,
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => {
            only_params(x, params, id)
        }
        ExprKind::Binary(_, a, b) => only_params(a, params, id) && only_params(b, params, id),
        ExprKind::Call(callee, args) => {
            *callee != id && args.iter().all(|a| only_params(a, params, id))
        }
        _ => false,
    }
}

/// Inlines the small functions into every function of the module.
pub fn inline_small(module: &mut Module) {
    let small = small_functions(&module.functions);
    if small.iter().all(Option::is_none) {
        return;
    }
    for function in &mut module.functions {
        let mut inliner = Inliner {
            small: &small,
            locals: &mut function.locals,
        };
        function.body = inliner.stmts(std::mem::take(&mut function.body));
    }
}

struct Inliner<'a> {
    small: &'a [Option<Expr>],
    locals: &'a mut Vec<Local>,
}

/// Where the walk of one statement's expressions is.
struct Walk {
    /// Assignments to new variables, to run before the statement.
    before: Vec<Stmt>,
    /// Everything evaluated so far is simple, and the call is evaluated
    /// whenever the statement is: a new variable can be computed before it.
    clean: bool,
}

impl Inliner<'_> {
    fn stmts(&mut self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            let mut walk = Walk {
                before: Vec::new(),
                clean: true,
            };
            let stmt = match stmt {
                Stmt::Set(l, e) => Stmt::Set(l, self.expr(e, &mut walk, 0)),
                Stmt::SetGlobal(g, e) => Stmt::SetGlobal(g, self.expr(e, &mut walk, 0)),
                Stmt::Expr(e) => Stmt::Expr(self.expr(e, &mut walk, 0)),
                Stmt::Return(Some(e)) => Stmt::Return(Some(self.expr(e, &mut walk, 0))),
                Stmt::ReturnMany(values) => Stmt::ReturnMany(self.all(values, &mut walk)),
                Stmt::CallMany {
                    targets,
                    callee,
                    args,
                } => Stmt::CallMany {
                    targets,
                    callee,
                    args: self.all(args, &mut walk),
                },
                Stmt::CallManyIndirect {
                    targets,
                    callee,
                    args,
                } => {
                    let callee = self.expr(callee, &mut walk, 0);
                    Stmt::CallManyIndirect {
                        targets,
                        callee,
                        args: self.all(args, &mut walk),
                    }
                }
                Stmt::Buf(op, args) => Stmt::Buf(op, self.all(args, &mut walk)),
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                } => Stmt::If {
                    cond: self.expr(cond, &mut walk, 0),
                    then: self.stmts(then),
                    otherwise: self.stmts(otherwise),
                },
                Stmt::Loop { cond, body, step } => {
                    // The condition is evaluated at every turn: nothing before it.
                    walk.clean = false;
                    Stmt::Loop {
                        cond: cond.map(|c| self.expr(c, &mut walk, 0)),
                        body: self.stmts(body),
                        step: self.stmts(step),
                    }
                }
                Stmt::Block(body) => Stmt::Block(self.stmts(body)),
                other => other,
            };
            out.append(&mut walk.before);
            out.push(stmt);
        }
        out
    }

    fn all(&mut self, values: Vec<Expr>, walk: &mut Walk) -> Vec<Expr> {
        values.into_iter().map(|e| self.expr(e, walk, 0)).collect()
    }

    /// Walks `e` in the order it is evaluated, inlining the calls it can.
    fn expr(&mut self, e: Expr, walk: &mut Walk, depth: usize) -> Expr {
        let ty = e.ty;
        let kind = match e.kind {
            ExprKind::Unary(op, x) => ExprKind::Unary(op, Box::new(self.expr(*x, walk, depth))),
            ExprKind::Convert(x) => ExprKind::Convert(Box::new(self.expr(*x, walk, depth))),
            ExprKind::Math(op, x) => ExprKind::Math(op, Box::new(self.expr(*x, walk, depth))),
            ExprKind::Binary(op @ (BinOp::And | BinOp::Or), a, b) => {
                let a = self.expr(*a, walk, depth);
                // The right side is not always evaluated.
                let clean = walk.clean;
                walk.clean = false;
                let b = self.expr(*b, walk, depth);
                walk.clean = clean && is_simple(&b);
                ExprKind::Binary(op, Box::new(a), Box::new(b))
            }
            ExprKind::Binary(op, a, b) => {
                let a = self.expr(*a, walk, depth);
                let b = self.expr(*b, walk, depth);
                ExprKind::Binary(op, Box::new(a), Box::new(b))
            }
            ExprKind::Call(callee, args) => return self.call(callee, args, ty, walk, depth),
            ExprKind::CallIndirect(callee, args) => {
                let callee = self.expr(*callee, walk, depth);
                let args = args
                    .into_iter()
                    .map(|a| self.expr(a, walk, depth))
                    .collect();
                ExprKind::CallIndirect(Box::new(callee), args)
            }
            ExprKind::Alloc(x) => ExprKind::Alloc(Box::new(self.expr(*x, walk, depth))),
            ExprKind::Buf(op, args) => ExprKind::Buf(
                op,
                args.into_iter()
                    .map(|a| self.expr(a, walk, depth))
                    .collect(),
            ),
            ExprKind::Env(op, args) => ExprKind::Env(
                op,
                args.into_iter()
                    .map(|a| self.expr(a, walk, depth))
                    .collect(),
            ),
            kind @ (ExprKind::Const(_)
            | ExprKind::Local(_)
            | ExprKind::Move(_)
            | ExprKind::Borrow(_)
            | ExprKind::Global(_)) => kind,
        };
        let e = Expr { kind, ty };
        if !is_simple(&e) && !matches!(e.kind, ExprKind::Binary(BinOp::And | BinOp::Or, ..)) {
            walk.clean = false;
        }
        e
    }

    fn call(
        &mut self,
        callee: FnId,
        args: Vec<Expr>,
        ty: Ty,
        walk: &mut Walk,
        depth: usize,
    ) -> Expr {
        let clean = walk.clean;
        let args: Vec<Expr> = args
            .into_iter()
            .map(|a| self.expr(a, walk, depth))
            .collect();
        let body = self.small.get(callee.0 as usize).and_then(Option::as_ref);
        let Some(body) = body.filter(|_| depth < MAX_DEPTH) else {
            walk.clean = false;
            return Expr {
                kind: ExprKind::Call(callee, args),
                ty,
            };
        };
        let mut reads = vec![0_usize; args.len()];
        count_params(body, &mut reads);
        // Each argument as the body reads it, or the call as it was.
        let mut bound = Vec::with_capacity(args.len());
        let mut before = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            let direct = matches!(arg.kind, ExprKind::Local(_) | ExprKind::Const(_))
                || (is_simple(arg) && reads[i] <= 1);
            if direct {
                bound.push(arg.clone());
            } else if clean {
                let local = LocalId(u32::try_from(self.locals.len()).unwrap_or(u32::MAX));
                self.locals.push(Local {
                    name: format!("arg{i}"),
                    ty: arg.ty,
                });
                before.push(Stmt::Set(local, arg.clone()));
                bound.push(Expr {
                    kind: ExprKind::Local(local),
                    ty: arg.ty,
                });
            } else if is_simple(arg) {
                // Computed again at each read: the same value, nothing else.
                bound.push(arg.clone());
            } else {
                walk.clean = false;
                return Expr {
                    kind: ExprKind::Call(callee, args),
                    ty,
                };
            }
        }
        walk.before.append(&mut before);
        let inlined = substitute(body, &bound);
        // The body may call other small functions.
        let mut inner = Walk {
            before: Vec::new(),
            clean: false,
        };
        let e = self.expr(inlined, &mut inner, depth + 1);
        walk.clean = clean && is_simple(&e);
        e
    }
}

fn count_params(e: &Expr, reads: &mut [usize]) {
    match &e.kind {
        ExprKind::Local(LocalId(l)) => {
            if let Some(n) = reads.get_mut(*l as usize) {
                *n += 1;
            }
        }
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => {
            count_params(x, reads);
        }
        ExprKind::Binary(_, a, b) => {
            count_params(a, reads);
            count_params(b, reads);
        }
        ExprKind::Call(_, args) => args.iter().for_each(|a| count_params(a, reads)),
        _ => {}
    }
}

fn substitute(e: &Expr, bound: &[Expr]) -> Expr {
    let kind = match &e.kind {
        ExprKind::Local(LocalId(l)) => return bound[*l as usize].clone(),
        ExprKind::Unary(op, x) => ExprKind::Unary(*op, Box::new(substitute(x, bound))),
        ExprKind::Convert(x) => ExprKind::Convert(Box::new(substitute(x, bound))),
        ExprKind::Math(op, x) => ExprKind::Math(*op, Box::new(substitute(x, bound))),
        ExprKind::Binary(op, a, b) => ExprKind::Binary(
            *op,
            Box::new(substitute(a, bound)),
            Box::new(substitute(b, bound)),
        ),
        ExprKind::Call(callee, args) => {
            ExprKind::Call(*callee, args.iter().map(|a| substitute(a, bound)).collect())
        }
        other => other.clone(),
    };
    Expr { kind, ty: e.ty }
}
