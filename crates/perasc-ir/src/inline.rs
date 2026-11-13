//! Recursion unrolling: a function's calls to itself are replaced by a copy of its
//! body, [`DEPTH`] levels deep, so that one real call does the work of several.
//!
//! **Why this pass exists.** Rust's `fib` runs about twice as fast as ours in V8,
//! yet the loop LLVM writes is no faster than ours: measured alone, both take the
//! same time. The difference is that V8 inlines Rust's `fib` into itself, and not
//! ours. V8 sizes its inlining budget from the module: when at least half of the
//! function bodies are under 50 bytes, it assumes the toolchain did not inline,
//! and allows five times the function's size; when a quarter or fewer are, it
//! allows two times, which is not enough for one level. The Rust module is full of
//! small allocator and glue functions; ours is not. Rust's gain is therefore a side
//! effect of an engine heuristic, which another engine, another V8 version or a
//! bigger Rust module can take away.
//!
//! Doing the unrolling here keeps the gain whatever the engine guesses: the copies
//! are in the module. Together with the loop rotation of [`crate::opt`], measured
//! on Node 26 (median of five runs), `fib(30)`, `fib(32)` and `fib(35)` take 2.1,
//! 5.4 and 22.7 ms with two levels, 1.6, 4.0 and 21.4 ms with three, and 1.8, 4.7
//! and 19.6 ms with four; the Rust build takes 1.7, 4.3 and 22.6 ms. Three levels
//! is the only depth ahead of Rust at every size.
//!
//! The copies change no result: a copy's locals are fresh, its arguments are
//! evaluated where the call evaluated them, and the rest of the statement, which
//! cannot call, trap or loop (see [`is_simple`]), is evaluated after it.

use std::collections::HashSet;

use crate::opt::is_simple;
use crate::{BinOp, Const, Expr, ExprKind, FnId, Function, Local, LocalId, Stmt, Ty};

/// Levels of copies.
const DEPTH: usize = 3;
/// A level is not added if the body would grow past this many times its size.
const MAX_GROWTH: usize = 8;

/// Replaces the self calls of a function by copies of its body.
pub fn unroll_recursion(function: &mut Function, id: FnId) {
    let ret = function.ret;
    if ret == Ty::Void || function.uses_many() {
        return;
    }
    let template = Template {
        body: function.body.clone(),
        locals: function.locals.clone(),
        reset: needs_reset(function),
    };
    let original = size(&template.body);
    for level in 1..=DEPTH {
        let kept = function.locals.len();
        let mut inliner = Inliner {
            id,
            ret,
            level,
            template: &template,
            locals: &mut function.locals,
            copies: 0,
        };
        let body = inliner.stmts(function.body.clone());
        if inliner.copies == 0 || size(&body) > MAX_GROWTH * original {
            function.locals.truncate(kept);
            return;
        }
        function.body = body;
    }
}

/// The body that is copied at each call, as it was before any copy.
struct Template {
    body: Vec<Stmt>,
    locals: Vec<Local>,
    /// Variables a copy must reset: the function reads them as zero on entry, but a
    /// copy run a second time would read the value left by the first.
    reset: Vec<LocalId>,
}

struct Inliner<'a> {
    id: FnId,
    ret: Ty,
    level: usize,
    template: &'a Template,
    locals: &'a mut Vec<Local>,
    copies: usize,
}

impl Inliner<'_> {
    fn stmts(&mut self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            match stmt {
                Stmt::Set(_, ref e) | Stmt::Expr(ref e) | Stmt::Return(Some(ref e))
                    if hoistable(e, self.id) =>
                {
                    self.inline(stmt, &mut out);
                }
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                } => out.push(Stmt::If {
                    cond,
                    then: self.stmts(then),
                    otherwise: self.stmts(otherwise),
                }),
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

    /// `x = e(f(args))` becomes `params' = args; block { copy }; x = e(ret')`.
    fn inline(&mut self, mut stmt: Stmt, out: &mut Vec<Stmt>) {
        let base = self.locals.len();
        for local in &self.template.locals {
            self.locals.push(Local {
                name: format!("{}_{}", local.name, self.level),
                ty: local.ty,
            });
        }
        let result = LocalId(index(self.locals.len()));
        self.locals.push(Local {
            name: format!("ret_{}", self.level),
            ty: self.ret,
        });
        let replacement = Expr {
            kind: ExprKind::Local(result),
            ty: self.ret,
        };
        let (Stmt::Set(_, e) | Stmt::Expr(e) | Stmt::Return(Some(e))) = &mut stmt else {
            return;
        };
        let args = take_call(e, self.id, replacement);
        for (i, arg) in args.into_iter().enumerate() {
            out.push(Stmt::Set(LocalId(index(base + i)), arg));
        }
        for local in &self.template.reset {
            let ty = self.template.locals[index_usize(local.0)].ty;
            out.push(Stmt::Set(shift(*local, base), zero(ty)));
        }
        let copy = Copy { base, result };
        let mut body = copy.stmts(&self.template.body, 0);
        // Leaving the block at its very end does nothing.
        if body.last() == Some(&Stmt::Leave(0)) {
            body.pop();
        }
        out.push(Stmt::Block(body));
        out.push(stmt);
        self.copies += 1;
    }
}

/// A copy of the template whose locals start at `base`.
struct Copy {
    base: usize,
    result: LocalId,
}

impl Copy {
    /// `blocks`: blocks between this statement and the copy's own block.
    fn stmts(&self, stmts: &[Stmt], blocks: u32) -> Vec<Stmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            match stmt {
                Stmt::Set(local, value) => {
                    out.push(Stmt::Set(shift(*local, self.base), self.expr(value)));
                }
                Stmt::SetGlobal(global, value) => {
                    out.push(Stmt::SetGlobal(*global, self.expr(value)));
                }
                Stmt::Expr(e) => out.push(Stmt::Expr(self.expr(e))),
                Stmt::CallMany {
                    targets,
                    callee,
                    args,
                } => out.push(Stmt::CallMany {
                    targets: targets.iter().map(|t| shift(*t, self.base)).collect(),
                    callee: *callee,
                    args: args.iter().map(|a| self.expr(a)).collect(),
                }),
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                } => out.push(Stmt::If {
                    cond: self.expr(cond),
                    then: self.stmts(then, blocks),
                    otherwise: self.stmts(otherwise, blocks),
                }),
                Stmt::Loop { cond, body, step } => out.push(Stmt::Loop {
                    cond: cond.as_ref().map(|c| self.expr(c)),
                    body: self.stmts(body, blocks),
                    step: self.stmts(step, blocks),
                }),
                Stmt::Block(body) => out.push(Stmt::Block(self.stmts(body, blocks + 1))),
                Stmt::Return(value) => {
                    if let Some(value) = value {
                        out.push(Stmt::Set(self.result, self.expr(value)));
                    }
                    out.push(Stmt::Leave(blocks));
                }
                // `ReturnMany` never occurs: `unroll_recursion` leaves functions
                // with several results alone.
                Stmt::Leave(_) | Stmt::Break | Stmt::Continue | Stmt::ReturnMany(_) => {
                    out.push(stmt.clone());
                }
                Stmt::Release(local) => out.push(Stmt::Release(shift(*local, self.base))),
                Stmt::Buf(op, args) => {
                    out.push(Stmt::Buf(*op, args.iter().map(|a| self.expr(a)).collect()));
                }
            }
        }
        out
    }

    fn expr(&self, e: &Expr) -> Expr {
        let kind = match &e.kind {
            ExprKind::Const(c) => ExprKind::Const(*c),
            ExprKind::Global(g) => ExprKind::Global(*g),
            ExprKind::Local(local) => ExprKind::Local(shift(*local, self.base)),
            ExprKind::Unary(op, x) => ExprKind::Unary(*op, Box::new(self.expr(x))),
            ExprKind::Binary(op, a, b) => {
                ExprKind::Binary(*op, Box::new(self.expr(a)), Box::new(self.expr(b)))
            }
            ExprKind::Call(id, args) => {
                ExprKind::Call(*id, args.iter().map(|a| self.expr(a)).collect())
            }
            ExprKind::Convert(x) => ExprKind::Convert(Box::new(self.expr(x))),
            ExprKind::Math(op, x) => ExprKind::Math(*op, Box::new(self.expr(x))),
            ExprKind::Alloc(x) => ExprKind::Alloc(Box::new(self.expr(x))),
            ExprKind::Move(local) => ExprKind::Move(shift(*local, self.base)),
            ExprKind::Borrow(local) => ExprKind::Borrow(shift(*local, self.base)),
            ExprKind::Buf(op, args) => {
                ExprKind::Buf(*op, args.iter().map(|a| self.expr(a)).collect())
            }
        };
        Expr { kind, ty: e.ty }
    }
}

/// Whether `e` holds exactly one self call that can be evaluated first: every
/// other operand on the way is [`is_simple`], and no `&&` / `||` makes the call
/// conditional.
fn hoistable(e: &Expr, id: FnId) -> bool {
    match &e.kind {
        ExprKind::Call(callee, _) => *callee == id,
        // Moving a call before a block changes hands would change who holds it,
        // and before a module variable is read, the value read (decision 0023).
        ExprKind::Binary(BinOp::And | BinOp::Or, ..)
        | ExprKind::Global(_)
        | ExprKind::Const(_)
        | ExprKind::Local(_)
        | ExprKind::Alloc(_)
        | ExprKind::Move(_)
        | ExprKind::Borrow(_)
        | ExprKind::Buf(..) => false,
        ExprKind::Binary(_, a, b) => {
            (hoistable(a, id) && is_simple(b)) || (is_simple(a) && hoistable(b, id))
        }
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => hoistable(x, id),
    }
}

/// Replaces the call found by [`hoistable`] and returns its arguments.
fn take_call(e: &mut Expr, id: FnId, replacement: Expr) -> Vec<Expr> {
    if let ExprKind::Call(callee, _) = &e.kind
        && *callee == id
    {
        let call = std::mem::replace(e, replacement);
        return match call.kind {
            ExprKind::Call(_, args) => args,
            _ => Vec::new(),
        };
    }
    match &mut e.kind {
        ExprKind::Binary(_, a, b) => {
            if hoistable(a, id) {
                take_call(a, id, replacement)
            } else {
                take_call(b, id, replacement)
            }
        }
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => {
            take_call(x, id, replacement)
        }
        _ => Vec::new(),
    }
}

/// Variables not assigned by the straight-line start of the body, from values
/// that only read parameters and variables already assigned there. A function
/// reads its variables as zero on entry; code that runs the body again (a copy,
/// a loop replacing a call) must set these back to zero first. Lowering assigns a
/// variable where it is declared, so this is usually empty or never read.
pub fn needs_reset(function: &Function) -> Vec<LocalId> {
    let params = usize::try_from(function.param_count).unwrap_or(usize::MAX);
    let mut assigned: HashSet<u32> = (0..index(params)).collect();
    for stmt in &function.body {
        let Stmt::Set(local, value) = stmt else {
            break;
        };
        if !reads_only(value, &assigned) {
            break;
        }
        assigned.insert(local.0);
    }
    (params..function.locals.len())
        .map(|i| LocalId(index(i)))
        .filter(|l| !assigned.contains(&l.0))
        .collect()
}

fn reads_only(e: &Expr, assigned: &HashSet<u32>) -> bool {
    match &e.kind {
        ExprKind::Const(_) | ExprKind::Global(_) => true,
        ExprKind::Local(local) | ExprKind::Move(local) | ExprKind::Borrow(local) => {
            assigned.contains(&local.0)
        }
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => {
            reads_only(x, assigned)
        }
        ExprKind::Binary(_, a, b) => reads_only(a, assigned) && reads_only(b, assigned),
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => {
            args.iter().all(|a| reads_only(a, assigned))
        }
        ExprKind::Alloc(x) => reads_only(x, assigned),
    }
}

/// Statements and expressions, to bound the growth.
fn size(stmts: &[Stmt]) -> usize {
    stmts
        .iter()
        .map(|stmt| match stmt {
            Stmt::Set(_, e) | Stmt::SetGlobal(_, e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => {
                1 + expr_size(e)
            }
            Stmt::ReturnMany(values)
            | Stmt::CallMany { args: values, .. }
            | Stmt::Buf(_, values) => 1 + values.iter().map(expr_size).sum::<usize>(),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => 1 + expr_size(cond) + size(then) + size(otherwise),
            Stmt::Loop { cond, body, step } => {
                1 + cond.as_ref().map_or(0, expr_size) + size(body) + size(step)
            }
            Stmt::Block(body) => 1 + size(body),
            Stmt::Return(None)
            | Stmt::Leave(_)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::Release(_) => 1,
        })
        .sum()
}

fn expr_size(e: &Expr) -> usize {
    1 + match &e.kind {
        ExprKind::Const(_)
        | ExprKind::Local(_)
        | ExprKind::Move(_)
        | ExprKind::Borrow(_)
        | ExprKind::Global(_) => 0,
        ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Math(_, x)
        | ExprKind::Alloc(x) => expr_size(x),
        ExprKind::Binary(_, a, b) => expr_size(a) + expr_size(b),
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => args.iter().map(expr_size).sum(),
    }
}

pub fn zero(ty: Ty) -> Expr {
    let c = match ty {
        Ty::F32 | Ty::F64 => Const::Float(0.0),
        Ty::Bool => Const::Bool(false),
        // Handles never reach a copy: functions using the heap are not optimized.
        Ty::I32 | Ty::I64 | Ty::Void | Ty::Owned | Ty::Borrowed => Const::Int(0),
    };
    Expr {
        kind: ExprKind::Const(c),
        ty,
    }
}

fn shift(local: LocalId, base: usize) -> LocalId {
    LocalId(index(index_usize(local.0) + base))
}

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn index_usize(i: u32) -> usize {
    usize::try_from(i).unwrap_or(usize::MAX)
}
