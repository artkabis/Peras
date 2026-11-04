//! Optimization passes on the IR.
//!
//! Each pass keeps the program's results exactly, traps included, and the IR
//! verifier re-checks the module after it. The Rust backend compiles the
//! unoptimized IR, so the differential test in CI also catches optimizer bugs.
//!
//! **Accumulator recursion elimination.** A `return` that calls the function
//! itself in last position becomes a jump back to the top of the function:
//!
//! - `return f(args)` (a tail call) sets the parameters to `args` and loops;
//! - `return e + f(args)` (or `*`, on integers) adds `e` to an accumulator, then
//!   loops; every other `return v` becomes `return acc + v`.
//!
//! `fib(n - 1) + fib(n - 2)` keeps one real call and turns the other into a loop,
//! as LLVM does. The evaluation order is unchanged: `e` was already evaluated
//! before the call. `f(args) + e` is accepted only when `e` cannot trap, call or
//! loop (constants, variables, arithmetic without division), since it is then
//! evaluated before the arguments. Floats are left alone: their addition is not
//! associative, and regrouping it would change rounding.

use crate::{BinOp, Const, Expr, ExprKind, FnId, Function, Local, LocalId, Module, Stmt, Ty};

/// Runs every optimization pass on a module.
pub fn optimize(module: &mut Module) {
    for (index, function) in module.functions.iter_mut().enumerate() {
        let id = FnId(u32::try_from(index).unwrap_or(u32::MAX));
        eliminate_tail_recursion(function, id);
    }
}

/// What a `return` does, seen from the transformation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Site {
    /// `return f(args)`.
    Tail,
    /// `return e op f(args)`.
    Accumulate(BinOp),
    /// Any other `return`.
    Base,
}

fn is_self_call(e: &Expr, id: FnId) -> bool {
    matches!(&e.kind, ExprKind::Call(callee, _) if *callee == id)
}

fn classify(value: &Expr, id: FnId, ret: Ty) -> Site {
    if is_self_call(value, id) {
        return Site::Tail;
    }
    if let ExprKind::Binary(op @ (BinOp::Add | BinOp::Mul), lhs, rhs) = &value.kind
        && ret.is_int()
        && (is_self_call(rhs, id) || (is_self_call(lhs, id) && is_simple(rhs)))
    {
        return Site::Accumulate(*op);
    }
    Site::Base
}

/// Whether an expression can be evaluated earlier without any visible change:
/// it never calls, never divides (no trap), and always ends.
fn is_simple(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Const(_) | ExprKind::Local(_) => true,
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => is_simple(x),
        ExprKind::Binary(op, a, b) => {
            !matches!(op, BinOp::Div | BinOp::Rem) && is_simple(a) && is_simple(b)
        }
        ExprKind::Call(..) => false,
    }
}

/// What the scan of a function found.
#[derive(Default)]
struct Scan {
    /// At least one `return` calls the function itself in last position.
    recursive: bool,
    /// The accumulating operator, if any.
    op: Option<BinOp>,
    /// Something prevents the transformation.
    blocked: bool,
}

fn scan(stmts: &[Stmt], id: FnId, ret: Ty, in_loop: bool, found: &mut Scan) {
    for stmt in stmts {
        match stmt {
            Stmt::Return(Some(value)) => match classify(value, id, ret) {
                Site::Base => {}
                // A `continue` here would target the inner loop, not the function.
                _ if in_loop => found.blocked = true,
                Site::Tail => found.recursive = true,
                Site::Accumulate(op) => {
                    found.recursive = true;
                    if found.op.is_some_and(|o| o != op) {
                        found.blocked = true;
                    }
                    found.op = Some(op);
                }
            },
            Stmt::If {
                then, otherwise, ..
            } => {
                scan(then, id, ret, in_loop, found);
                scan(otherwise, id, ret, in_loop, found);
            }
            Stmt::Loop { body, step, .. } => {
                scan(body, id, ret, true, found);
                scan(step, id, ret, true, found);
            }
            _ => {}
        }
    }
}

fn eliminate_tail_recursion(function: &mut Function, id: FnId) {
    let ret = function.ret;
    if ret == Ty::Void {
        return;
    }
    let mut found = Scan::default();
    scan(&function.body, id, ret, false, &mut found);
    if !found.recursive || found.blocked {
        return;
    }
    let params: Vec<(LocalId, Ty)> = function
        .params()
        .enumerate()
        .map(|(i, p)| (LocalId(u32::try_from(i).unwrap_or(u32::MAX)), p.ty))
        .collect();
    let mut add_local = |name: String, ty: Ty| {
        let id = LocalId(u32::try_from(function.locals.len()).unwrap_or(u32::MAX));
        function.locals.push(Local { name, ty });
        id
    };
    let acc = found.op.map(|op| (op, add_local("acc".to_owned(), ret)));
    let temps: Vec<LocalId> = params
        .iter()
        .map(|(i, ty)| add_local(format!("next{}", i.0), *ty))
        .collect();
    let rewriter = Rewriter {
        id,
        ret,
        acc,
        params: &params,
        temps: &temps,
    };
    let body = rewriter.stmts(std::mem::take(&mut function.body));
    let mut new_body = Vec::new();
    if let Some((op, acc)) = acc {
        let identity = i64::from(op == BinOp::Mul);
        new_body.push(Stmt::Set(acc, constant(identity, ret)));
    }
    new_body.push(Stmt::Loop {
        cond: None,
        body,
        step: Vec::new(),
    });
    function.body = new_body;
}

struct Rewriter<'a> {
    id: FnId,
    ret: Ty,
    acc: Option<(BinOp, LocalId)>,
    params: &'a [(LocalId, Ty)],
    temps: &'a [LocalId],
}

impl Rewriter<'_> {
    fn stmts(&self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            match stmt {
                Stmt::Return(Some(value)) => self.ret_stmt(value, &mut out),
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                } => out.push(Stmt::If {
                    cond,
                    then: self.stmts(then),
                    otherwise: self.stmts(otherwise),
                }),
                // Recursive returns inside loops were ruled out: only base returns
                // remain there, and they still need the accumulator.
                Stmt::Loop { cond, body, step } => out.push(Stmt::Loop {
                    cond,
                    body: self.stmts(body),
                    step: self.stmts(step),
                }),
                other => out.push(other),
            }
        }
        out
    }

    fn ret_stmt(&self, value: Expr, out: &mut Vec<Stmt>) {
        match classify(&value, self.id, self.ret) {
            Site::Base => out.push(Stmt::Return(Some(self.with_acc(value)))),
            Site::Tail => {
                if let ExprKind::Call(_, args) = value.kind {
                    self.jump(args, out);
                }
            }
            Site::Accumulate(_) => {
                let ExprKind::Binary(_, lhs, rhs) = value.kind else {
                    return;
                };
                let (call, other) = if is_self_call(&rhs, self.id) {
                    (*rhs, *lhs)
                } else {
                    (*lhs, *rhs)
                };
                if let Some((op, acc)) = self.acc {
                    out.push(Stmt::Set(acc, binary(op, local(acc, self.ret), other)));
                }
                if let ExprKind::Call(_, args) = call.kind {
                    self.jump(args, out);
                }
            }
        }
    }

    /// Evaluates the new arguments, then replaces the parameters and loops.
    fn jump(&self, args: Vec<Expr>, out: &mut Vec<Stmt>) {
        for (temp, arg) in self.temps.iter().zip(args) {
            out.push(Stmt::Set(*temp, arg));
        }
        for ((param, ty), temp) in self.params.iter().zip(self.temps) {
            out.push(Stmt::Set(*param, local(*temp, *ty)));
        }
        out.push(Stmt::Continue);
    }

    fn with_acc(&self, value: Expr) -> Expr {
        match self.acc {
            Some((op, acc)) => binary(op, local(acc, self.ret), value),
            None => value,
        }
    }
}

fn local(id: LocalId, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Local(id),
        ty,
    }
}

fn constant(value: i64, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(value)),
        ty,
    }
}

fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let ty = lhs.ty;
    Expr {
        kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
        ty,
    }
}
