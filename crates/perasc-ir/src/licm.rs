//! Two passes found by comparing Peras and LLVM on the `mandel` kernel, each
//! measured on its own (median of nine runs of `mandel(600)` on Node 26).
//!
//! **Doubling as an addition.** `2.0 * x` on a float local becomes `x + x`. The
//! two are the same IEEE 754 operation: both are exact, overflow to the same
//! infinity, and keep NaN and `-0.0` alike. The addition does not compete with the
//! other multiplications of the loop for the processor's multiply units: on
//! `mandel`, 15.1 ms become 13.6 ms. Integers are left to the engine, which already
//! does it.
//!
//! **Loop-invariant code motion.** `const cy = py.toF64() * scale - 1.5;` inside
//! the loop over `px` computes the same value at every turn; it moves before the
//! loop. An assignment `l = e` moves out of a loop when:
//!
//! - `e` cannot trap, call or loop ([`is_simple`]), so computing it once more,
//!   even when the loop does not run, changes nothing;
//! - nothing in the loop assigns a local that `e` reads, so the value is the same
//!   at every turn;
//! - the loop assigns `l` only there, and reads `l` neither before it in the body
//!   nor in its condition or step, nor anywhere outside the loop.
//!
//! Inner loops are handled first, so a value can leave several loops at once.

use std::collections::HashMap;

use crate::opt::is_simple;
use crate::{BinOp, Const, Expr, ExprKind, LocalId, Stmt};

/// `2.0 * x` and `x * 2.0` become `x + x`, for a float local `x`.
pub fn doubling_as_addition(stmts: Vec<Stmt>) -> Vec<Stmt> {
    map_exprs(stmts, &mut |e| {
        let ExprKind::Binary(BinOp::Mul, lhs, rhs) = &e.kind else {
            return e;
        };
        let is_two = |x: &Expr| matches!(x.kind, ExprKind::Const(Const::Float(v)) if v.to_bits() == 2.0_f64.to_bits());
        let operand = match (&lhs.kind, &rhs.kind) {
            (_, ExprKind::Local(_)) if is_two(lhs) => (**rhs).clone(),
            (ExprKind::Local(_), _) if is_two(rhs) => (**lhs).clone(),
            _ => return e,
        };
        if !e.ty.is_float() {
            return e;
        }
        Expr {
            kind: ExprKind::Binary(BinOp::Add, Box::new(operand.clone()), Box::new(operand)),
            ty: e.ty,
        }
    })
}

/// Rewrites every expression of a body, innermost first.
fn map_exprs(stmts: Vec<Stmt>, f: &mut impl FnMut(Expr) -> Expr) -> Vec<Stmt> {
    stmts
        .into_iter()
        .map(|stmt| match stmt {
            Stmt::Set(l, e) => Stmt::Set(l, map_expr(e, f)),
            Stmt::Expr(e) => Stmt::Expr(map_expr(e, f)),
            Stmt::Return(e) => Stmt::Return(e.map(|e| map_expr(e, f))),
            Stmt::ReturnMany(values) => {
                Stmt::ReturnMany(values.into_iter().map(|e| map_expr(e, f)).collect())
            }
            Stmt::CallMany {
                targets,
                callee,
                args,
            } => Stmt::CallMany {
                targets,
                callee,
                args: args.into_iter().map(|e| map_expr(e, f)).collect(),
            },
            Stmt::If {
                cond,
                then,
                otherwise,
            } => Stmt::If {
                cond: map_expr(cond, f),
                then: map_exprs(then, f),
                otherwise: map_exprs(otherwise, f),
            },
            Stmt::Loop { cond, body, step } => Stmt::Loop {
                cond: cond.map(|c| map_expr(c, f)),
                body: map_exprs(body, f),
                step: map_exprs(step, f),
            },
            Stmt::Block(body) => Stmt::Block(map_exprs(body, f)),
            other @ (Stmt::Leave(_) | Stmt::Break | Stmt::Continue | Stmt::Release(_)) => other,
        })
        .collect()
}

fn map_expr(e: Expr, f: &mut impl FnMut(Expr) -> Expr) -> Expr {
    let kind = match e.kind {
        ExprKind::Unary(op, x) => ExprKind::Unary(op, Box::new(map_expr(*x, f))),
        ExprKind::Binary(op, a, b) => {
            ExprKind::Binary(op, Box::new(map_expr(*a, f)), Box::new(map_expr(*b, f)))
        }
        ExprKind::Call(id, args) => {
            ExprKind::Call(id, args.into_iter().map(|a| map_expr(a, f)).collect())
        }
        ExprKind::Convert(x) => ExprKind::Convert(Box::new(map_expr(*x, f))),
        ExprKind::Math(op, x) => ExprKind::Math(op, Box::new(map_expr(*x, f))),
        ExprKind::Alloc(x) => ExprKind::Alloc(Box::new(map_expr(*x, f))),
        kind @ (ExprKind::Const(_)
        | ExprKind::Local(_)
        | ExprKind::Move(_)
        | ExprKind::Borrow(_)) => kind,
    };
    f(Expr { kind, ty: e.ty })
}

/// Loop-invariant code motion over a function body.
pub fn hoist_invariants(body: Vec<Stmt>) -> Vec<Stmt> {
    let mut reads = HashMap::new();
    count_reads(&body, &mut reads);
    Hoister { reads: &reads }.stmts(body)
}

struct Hoister<'a> {
    /// Reads of each local in the whole function.
    reads: &'a HashMap<u32, usize>,
}

impl Hoister<'_> {
    fn stmts(&self, stmts: Vec<Stmt>) -> Vec<Stmt> {
        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            match stmt {
                Stmt::Loop { cond, body, step } => {
                    let body = self.stmts(body);
                    let step = self.stmts(step);
                    let (hoisted, body) = self.split(cond.as_ref(), body, &step);
                    out.extend(hoisted);
                    out.push(Stmt::Loop { cond, body, step });
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
                Stmt::Block(body) => out.push(Stmt::Block(self.stmts(body))),
                other => out.push(other),
            }
        }
        out
    }

    /// Splits a loop body into the assignments that can move out, and the rest.
    fn split(&self, cond: Option<&Expr>, body: Vec<Stmt>, step: &[Stmt]) -> (Vec<Stmt>, Vec<Stmt>) {
        let mut sets = HashMap::new();
        count_sets(&body, &mut sets);
        count_sets(step, &mut sets);
        let mut inside = HashMap::new();
        count_reads(&body, &mut inside);
        count_reads(step, &mut inside);
        let mut before = HashMap::new();
        if let Some(cond) = cond {
            count_expr_reads(cond, &mut before);
            count_expr_reads(cond, &mut inside);
        }
        count_reads(step, &mut before);
        let (mut hoisted, mut kept) = (Vec::new(), Vec::with_capacity(body.len()));
        for stmt in body {
            let movable = match &stmt {
                Stmt::Set(l, e) => {
                    let mut operands = HashMap::new();
                    count_expr_reads(e, &mut operands);
                    is_simple(e)
                        && operands.keys().all(|o| !sets.contains_key(o))
                        && sets.get(&l.0) == Some(&1)
                        && !before.contains_key(&l.0)
                        && inside.get(&l.0) == self.reads.get(&l.0)
                }
                _ => false,
            };
            if movable {
                hoisted.push(stmt);
            } else {
                count_reads(std::slice::from_ref(&stmt), &mut before);
                kept.push(stmt);
            }
        }
        (hoisted, kept)
    }
}

fn count_sets(stmts: &[Stmt], out: &mut HashMap<u32, usize>) {
    for stmt in stmts {
        match stmt {
            Stmt::Set(LocalId(l), _) => *out.entry(*l).or_default() += 1,
            Stmt::CallMany { targets, .. } => {
                for LocalId(l) in targets {
                    *out.entry(*l).or_default() += 1;
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                count_sets(then, out);
                count_sets(otherwise, out);
            }
            Stmt::Loop { body, step, .. } => {
                count_sets(body, out);
                count_sets(step, out);
            }
            Stmt::Block(body) => count_sets(body, out),
            _ => {}
        }
    }
}

fn count_reads(stmts: &[Stmt], out: &mut HashMap<u32, usize>) {
    for stmt in stmts {
        match stmt {
            Stmt::Set(_, e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => count_expr_reads(e, out),
            Stmt::ReturnMany(values) | Stmt::CallMany { args: values, .. } => {
                for e in values {
                    count_expr_reads(e, out);
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                count_expr_reads(cond, out);
                count_reads(then, out);
                count_reads(otherwise, out);
            }
            Stmt::Loop { cond, body, step } => {
                if let Some(cond) = cond {
                    count_expr_reads(cond, out);
                }
                count_reads(body, out);
                count_reads(step, out);
            }
            Stmt::Block(body) => count_reads(body, out),
            Stmt::Return(None)
            | Stmt::Leave(_)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::Release(_) => {}
        }
    }
}

fn count_expr_reads(e: &Expr, out: &mut HashMap<u32, usize>) {
    match &e.kind {
        ExprKind::Const(_) => {}
        ExprKind::Local(LocalId(l)) | ExprKind::Move(LocalId(l)) | ExprKind::Borrow(LocalId(l)) => {
            *out.entry(*l).or_default() += 1;
        }
        ExprKind::Alloc(x) => count_expr_reads(x, out),
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Math(_, x) => {
            count_expr_reads(x, out);
        }
        ExprKind::Binary(_, a, b) => {
            count_expr_reads(a, out);
            count_expr_reads(b, out);
        }
        ExprKind::Call(_, args) => {
            for a in args {
                count_expr_reads(a, out);
            }
        }
    }
}
