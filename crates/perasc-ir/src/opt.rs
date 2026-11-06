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
//!
//! **Branchless absolute value.** `if (x < 0) x = -x` on an integer becomes
//! `x = x.abs()`, which the WebAssembly backend writes with `select`, without a
//! branch. When the sign of `x` is unpredictable, as in a hash, the branch is
//! mispredicted about one time in two: on the `checksum` kernel this alone takes
//! the loop from 26 to 11 ms, Rust's time, since LLVM does the same. Both forms
//! wrap on the smallest value. Floats are left alone: `-0.0` and NaN would differ.
//!
//! **Exit test rotation.** `loop { if (c) { ...; return v; } rest }` becomes
//! `while (!c) { rest } ...; return v`: the backends check a loop condition once
//! before the loop and then at the bottom, one branch per turn, where the `if`
//! at the top cost two. It applies when `rest` has no `break` of its own, since
//! that `break` would now run the code after the loop.
//!
//! **Loop-invariant code motion** and **doubling as an addition**: see
//! [`crate::licm`].
//!
//! **Recursion unrolling** then copies the body into the remaining self calls; see
//! [`crate::inline`] for why Peras does it rather than leave it to the engine.

use crate::{BinOp, Const, Expr, ExprKind, FnId, Function, Local, LocalId, Module, Stmt, Ty};

/// Runs every optimization pass on a module.
pub fn optimize(module: &mut Module) {
    for (index, function) in module.functions.iter_mut().enumerate() {
        let id = FnId(u32::try_from(index).unwrap_or(u32::MAX));
        // No pass is proven yet to keep releases exact, nor written for several
        // results: such code is left as written, and the verifiers check it.
        if function.uses_heap() || function.uses_many() {
            continue;
        }
        function.body = branchless_abs(std::mem::take(&mut function.body));
        eliminate_tail_recursion(function, id);
        function.body = rotate(std::mem::take(&mut function.body));
        function.body = crate::licm::hoist_invariants(std::mem::take(&mut function.body));
        function.body = crate::licm::doubling_as_addition(std::mem::take(&mut function.body));
        crate::inline::unroll_recursion(function, id);
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
pub fn is_simple(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Const(_) | ExprKind::Local(_) => true,
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => is_simple(x),
        ExprKind::Binary(op, a, b) => {
            !matches!(op, BinOp::Div | BinOp::Rem) && is_simple(a) && is_simple(b)
        }
        // A call, an allocation or a change of owner is never moved.
        ExprKind::Call(..)
        | ExprKind::Alloc(_)
        | ExprKind::Move(_)
        | ExprKind::Borrow(_)
        | ExprKind::Buf(..) => false,
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
            Stmt::Block(body) => scan(body, id, ret, in_loop, found),
            _ => {}
        }
    }
}

pub fn eliminate_tail_recursion(function: &mut Function, id: FnId) {
    let ret = function.ret;
    if ret == Ty::Void {
        return;
    }
    let mut found = Scan::default();
    scan(&function.body, id, ret, false, &mut found);
    if !found.recursive || found.blocked {
        return;
    }
    let reset: Vec<Stmt> = crate::inline::needs_reset(function)
        .into_iter()
        .map(|l| {
            let ty = function.locals[usize::try_from(l.0).unwrap_or(usize::MAX)].ty;
            Stmt::Set(l, crate::inline::zero(ty))
        })
        .collect();
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
    // A parameter goes through a temporary only if a later argument still reads
    // it: `gcd(b, a % b)` needs one for `a`, `fib(n - 2)` none.
    let mut sites = Vec::new();
    self_call_args(&function.body, id, ret, &mut sites);
    let temps: Vec<Option<LocalId>> = params
        .iter()
        .map(|(i, ty)| {
            let read_later = sites.iter().any(|args| {
                args.iter()
                    .skip(usize::try_from(i.0).unwrap_or(usize::MAX) + 1)
                    .any(|a| reads(a, *i))
            });
            read_later.then(|| add_local(format!("next{}", i.0), *ty))
        })
        .collect();
    let rewriter = Rewriter {
        id,
        ret,
        acc,
        params: &params,
        temps: &temps,
    };
    // Each turn of the loop is a new call: its variables start at zero again.
    let mut body = reset;
    body.extend(rewriter.stmts(std::mem::take(&mut function.body)));
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
    temps: &'a [Option<LocalId>],
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
                Stmt::Block(body) => out.push(Stmt::Block(self.stmts(body))),
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
        for (((param, _), temp), arg) in self.params.iter().zip(self.temps).zip(args) {
            out.push(Stmt::Set(temp.unwrap_or(*param), arg));
        }
        for ((param, ty), temp) in self.params.iter().zip(self.temps) {
            if let Some(temp) = temp {
                out.push(Stmt::Set(*param, local(*temp, *ty)));
            }
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

/// The arguments of every self call that the rewrite turns into a jump.
fn self_call_args<'a>(stmts: &'a [Stmt], id: FnId, ret: Ty, out: &mut Vec<&'a [Expr]>) {
    for stmt in stmts {
        match stmt {
            Stmt::Return(Some(value)) => {
                let call = match (&value.kind, classify(value, id, ret)) {
                    (_, Site::Tail) => Some(value),
                    (ExprKind::Binary(_, lhs, rhs), Site::Accumulate(_)) => {
                        Some(if is_self_call(rhs, id) {
                            &**rhs
                        } else {
                            &**lhs
                        })
                    }
                    _ => None,
                };
                if let Some(Expr {
                    kind: ExprKind::Call(_, args),
                    ..
                }) = call
                {
                    out.push(args);
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                self_call_args(then, id, ret, out);
                self_call_args(otherwise, id, ret, out);
            }
            Stmt::Loop { body, step, .. } => {
                self_call_args(body, id, ret, out);
                self_call_args(step, id, ret, out);
            }
            Stmt::Block(body) => self_call_args(body, id, ret, out),
            _ => {}
        }
    }
}

/// Whether an expression reads a local.
fn reads(e: &Expr, target: LocalId) -> bool {
    match &e.kind {
        ExprKind::Const(_) => false,
        ExprKind::Local(l) | ExprKind::Move(l) | ExprKind::Borrow(l) => *l == target,
        ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Math(_, x)
        | ExprKind::Alloc(x) => reads(x, target),
        ExprKind::Binary(_, a, b) => reads(a, target) || reads(b, target),
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => args.iter().any(|a| reads(a, target)),
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

/// Exit test rotation, in every loop of a body.
pub fn rotate(stmts: Vec<Stmt>) -> Vec<Stmt> {
    let mut out = Vec::with_capacity(stmts.len());
    for stmt in stmts {
        match stmt {
            Stmt::Loop {
                cond: None,
                mut body,
                step,
            } if step.is_empty() && rotatable(&body) => {
                let Stmt::If { cond, then, .. } = body.remove(0) else {
                    unreachable!("checked by `rotatable`")
                };
                let negated = Expr {
                    kind: ExprKind::Unary(crate::UnOp::Not, Box::new(cond)),
                    ty: Ty::Bool,
                };
                out.push(Stmt::Loop {
                    cond: Some(negated),
                    body: rotate(body),
                    step,
                });
                out.extend(rotate(then));
            }
            Stmt::Loop { cond, body, step } => out.push(Stmt::Loop {
                cond,
                body: rotate(body),
                step: rotate(step),
            }),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => out.push(Stmt::If {
                cond,
                then: rotate(then),
                otherwise: rotate(otherwise),
            }),
            Stmt::Block(body) => out.push(Stmt::Block(rotate(body))),
            other => out.push(other),
        }
    }
    out
}

/// `[if (c) { ...; return v; }, rest]`, where the `if` has no `else` and never
/// falls through or jumps within the loop, and `rest` never leaves it by `break`.
fn rotatable(body: &[Stmt]) -> bool {
    let Some((
        Stmt::If {
            then, otherwise, ..
        },
        rest,
    )) = body.split_first()
    else {
        return false;
    };
    otherwise.is_empty()
        && matches!(then.last(), Some(Stmt::Return(_)))
        && !jumps(then, true)
        && !jumps(rest, false)
}

/// Whether statements `break` the enclosing loop, or `continue` it too if asked.
fn jumps(stmts: &[Stmt], or_continue: bool) -> bool {
    stmts.iter().any(|stmt| match stmt {
        Stmt::Break => true,
        Stmt::Continue => or_continue,
        Stmt::If {
            then, otherwise, ..
        } => jumps(then, or_continue) || jumps(otherwise, or_continue),
        Stmt::Block(body) => jumps(body, or_continue),
        // Jumps inside an inner loop target that loop.
        _ => false,
    })
}

/// `if (x < 0) x = -x` (also `x <= 0`, `0 > x`, `0 >= x`) on an integer local
/// becomes `x = x.abs()`, in every statement list of a body.
pub fn branchless_abs(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts
        .into_iter()
        .map(|stmt| match stmt {
            Stmt::If {
                cond,
                then,
                otherwise,
            } => match abs_target(&cond, &then, &otherwise) {
                Some((x, ty)) => Stmt::Set(
                    x,
                    Expr {
                        kind: ExprKind::Math(crate::MathOp::Abs, Box::new(local(x, ty))),
                        ty,
                    },
                ),
                None => Stmt::If {
                    cond,
                    then: branchless_abs(then),
                    otherwise: branchless_abs(otherwise),
                },
            },
            Stmt::Loop { cond, body, step } => Stmt::Loop {
                cond,
                body: branchless_abs(body),
                step: branchless_abs(step),
            },
            Stmt::Block(body) => Stmt::Block(branchless_abs(body)),
            other => other,
        })
        .collect()
}

/// The local of an `if` shaped like `if (x < 0) x = -x`.
fn abs_target(cond: &Expr, then: &[Stmt], otherwise: &[Stmt]) -> Option<(LocalId, Ty)> {
    let ExprKind::Binary(op, lhs, rhs) = &cond.kind else {
        return None;
    };
    let is_zero = |e: &Expr| matches!(e.kind, ExprKind::Const(Const::Int(0)));
    let x = match (op, &lhs.kind, &rhs.kind) {
        (BinOp::Lt | BinOp::Le, ExprKind::Local(x), _) if is_zero(rhs) => *x,
        (BinOp::Gt | BinOp::Ge, _, ExprKind::Local(x)) if is_zero(lhs) => *x,
        _ => return None,
    };
    let ty = lhs.ty;
    let [Stmt::Set(target, value)] = then else {
        return None;
    };
    let negates_x = matches!(&value.kind,
        ExprKind::Unary(crate::UnOp::Neg, operand)
            if matches!(operand.kind, ExprKind::Local(l) if l == x));
    (ty.is_int() && otherwise.is_empty() && *target == x && negates_x).then_some((x, ty))
}
