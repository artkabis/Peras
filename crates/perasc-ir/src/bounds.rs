//! **Bounds check elimination.** `for (let i = 0; i < xs.length; i++)`
//! checks `i` against the length of `xs` at every `xs[i]`, though the loop's
//! condition already proves it. The check is a compare and a branch per access;
//! in a loop that does little else, it is what separates WebAssembly from
//! JavaScript, whose engine removes it the same way.
//!
//! A loop is _counted_ when:
//!
//! - its condition is `i < bound`, on an `i32` local `i`;
//! - the statement just before it sets `i` to a constant of zero or more, or
//!   to a local the loop never assigns, as the job of a `parallel for` starts
//!   at its own `start`: then `start >= 0` joins the guard;
//! - its step is exactly `i = i + 1`, and its body never assigns `i`;
//! - `bound` is the same at every turn: a constant, a local the loop never
//!   assigns, or the length of such a buffer.
//!
//! Inside, `0 <= i < bound` holds; the step cannot overflow, since `i + 1` is
//! at most `bound`. A check of `i` against the length of a buffer `c` is then
//! needless when that length is at least `bound`, and stays so through the
//! body. The body must be _quiet_: it changes the length of no buffer (no
//! `push`, truncation, append or release), moves none, assigns none of the
//! buffers it checks, and calls no function, which could do any of that through
//! an `inout` parameter.
//!
//! `len(c) >= bound` is known when `bound` is the length of `c` itself, or when
//! an enclosing loop already proved it. Otherwise the loop is **versioned**:
//! `if (len(c) >= bound && ...) { the loop without checks } else { the loop as
//! written }`. The guard is computed once, before the loop. When it fails, the
//! original loop runs and traps exactly where it did, after the same effects:
//! the results, traps included, are always those of the program as written.
//!
//! What a counted loop proves holds through its whole body, nested loops
//! included, since the body is quiet: an inner loop `j < bound` with the same
//! `bound` needs no guard of its own.
//!
//! Measured in Chromium, alternating builds in one page: the particles kernel
//! of `bench/browser` goes from 24 to 21 ms. Reading each buffer's length and
//! the address of its elements into locals before such a loop was tried as
//! well, and was slower (23 ms): V8 already keeps those loads out of the loop,
//! and the extra locals only cost registers.
//!
//! The pass never moves, adds or removes a release, so unlike the passes of
//! [`crate::opt`] it applies to functions that use the heap.

use crate::{BinOp, BufOp, BufStmt, Const, Expr, ExprKind, Function, LocalId, Stmt, Ty};

/// What holds in a region: indices within their bound, and buffers at least
/// that long.
#[derive(Clone, Default)]
struct Facts {
    /// `0 <= index < bound`.
    ranges: Vec<(LocalId, Expr)>,
    /// `len(buffer) >= bound`.
    lengths: Vec<(LocalId, Expr)>,
}

impl Facts {
    /// Whether `index < len(buffer)` is already proven.
    fn proves(&self, index: LocalId, buffer: LocalId) -> bool {
        self.ranges.iter().any(|(i, bound)| {
            *i == index && (is_len_of(bound, buffer) || self.long_enough(buffer, bound))
        })
    }

    fn long_enough(&self, buffer: LocalId, bound: &Expr) -> bool {
        self.lengths.iter().any(|(b, e)| *b == buffer && e == bound)
    }
}

/// Removes the bounds checks that counted loops prove, versioning a loop when
/// the proof needs a guard.
pub fn eliminate_bounds_checks(function: &mut Function) {
    let body = std::mem::take(&mut function.body);
    function.body = block(body, &Facts::default());
}

fn block(stmts: Vec<Stmt>, facts: &Facts) -> Vec<Stmt> {
    let mut out: Vec<Stmt> = Vec::with_capacity(stmts.len());
    for stmt in stmts {
        match stmt {
            Stmt::Buf(BufStmt::CheckIndex, args) => {
                if let [index, len] = args.as_slice()
                    && let (ExprKind::Local(index), Some(buffer)) = (&index.kind, len_of(len))
                    && facts.proves(*index, buffer)
                {
                    continue;
                }
                out.push(Stmt::Buf(BufStmt::CheckIndex, args));
            }
            Stmt::Loop { cond, body, step } => {
                let start = out.last().and_then(start_of);
                out.push(counted_loop(cond, body, step, start, facts));
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => out.push(Stmt::If {
                cond,
                then: block(then, facts),
                otherwise: block(otherwise, facts),
            }),
            Stmt::Block(body) => out.push(Stmt::Block(block(body, facts))),
            other => out.push(other),
        }
    }
    out
}

/// A loop, without the checks it proves; inside an `if` when it needs a guard.
fn counted_loop(
    cond: Option<Expr>,
    body: Vec<Stmt>,
    step: Vec<Stmt>,
    start: Option<(LocalId, Option<LocalId>)>,
    facts: &Facts,
) -> Stmt {
    // A start that is a local must be at least zero, and stay as it is.
    let from = start.and_then(|(_, from)| from);
    let start = start
        .filter(|_| from.is_none_or(|f| !assigns(&body, f)))
        .map(|(index, _)| index);
    let Some((index, bound)) = counted(cond.as_ref(), &body, &step, start) else {
        // Not counted: what encloses it still holds inside.
        return Stmt::Loop {
            cond,
            body: block(body, facts),
            step,
        };
    };
    // The buffers `index` is checked against, anywhere in the body, with the
    // expression of their length.
    let mut checked: Vec<(LocalId, Expr)> = Vec::new();
    checks_of(&body, index, &mut checked);
    let buffers: Vec<LocalId> = checked.iter().map(|(c, _)| *c).collect();
    if !quiet(&body, index, &bound, &buffers) {
        return Stmt::Loop {
            cond,
            body: block(body, facts),
            step,
        };
    }
    let guards: Vec<&Expr> = checked
        .iter()
        .filter(|(c, _)| !is_len_of(&bound, *c) && !facts.long_enough(*c, &bound))
        .map(|(_, len)| len)
        .collect();
    let mut inner = facts.clone();
    inner.ranges.push((index, bound.clone()));
    inner
        .lengths
        .extend(buffers.iter().map(|&c| (c, bound.clone())));
    let fast = Stmt::Loop {
        cond: cond.clone(),
        body: block(body.clone(), &inner),
        step: step.clone(),
    };
    let zero = Expr {
        kind: ExprKind::Const(Const::Int(0)),
        ty: Ty::I32,
    };
    let Some(guard) = from
        .map(|f| {
            let start = Expr {
                kind: ExprKind::Local(f),
                ty: Ty::I32,
            };
            binary(BinOp::Ge, start, zero, Ty::Bool)
        })
        .into_iter()
        .chain(
            guards
                .iter()
                .map(|len| binary(BinOp::Ge, (*len).clone(), bound.clone(), Ty::Bool)),
        )
        .reduce(|a, b| binary(BinOp::And, a, b, Ty::Bool))
    else {
        return fast;
    };
    Stmt::If {
        cond: guard,
        then: vec![fast],
        otherwise: vec![Stmt::Loop { cond, body, step }],
    }
}

/// `(i, bound)` when the loop is counted (see the module's documentation).
fn counted(
    cond: Option<&Expr>,
    body: &[Stmt],
    step: &[Stmt],
    start: Option<LocalId>,
) -> Option<(LocalId, Expr)> {
    let ExprKind::Binary(BinOp::Lt, lhs, bound) = &cond?.kind else {
        return None;
    };
    let ExprKind::Local(index) = lhs.kind else {
        return None;
    };
    if lhs.ty != Ty::I32 || start != Some(index) {
        return None;
    }
    let [Stmt::Set(target, next)] = step else {
        return None;
    };
    let increments = matches!(
        &next.kind,
        ExprKind::Binary(BinOp::Add, a, b)
            if a.kind == ExprKind::Local(index) && b.kind == ExprKind::Const(Const::Int(1))
    );
    let invariant = match &bound.kind {
        ExprKind::Const(Const::Int(_)) => true,
        ExprKind::Local(l) => !assigns(body, *l),
        _ => len_of(bound).is_some_and(|b| !assigns(body, b)),
    };
    (*target == index && increments && invariant && !assigns(body, index))
        .then(|| (index, (**bound).clone()))
}

/// The local `stmt` sets to a constant of zero or more, or to another local,
/// which a guard must then prove at least zero.
fn start_of(stmt: &Stmt) -> Option<(LocalId, Option<LocalId>)> {
    match stmt {
        Stmt::Set(
            local,
            Expr {
                kind: ExprKind::Const(Const::Int(k)),
                ty: Ty::I32,
            },
        ) if *k >= 0 => Some((*local, None)),
        Stmt::Set(
            local,
            Expr {
                kind: ExprKind::Local(from),
                ty: Ty::I32,
            },
        ) if local != from => Some((*local, Some(*from))),
        _ => None,
    }
}

/// The buffers `index` is checked against in `stmts`, each once, with the
/// expression of their length.
fn checks_of(stmts: &[Stmt], index: LocalId, out: &mut Vec<(LocalId, Expr)>) {
    for stmt in stmts {
        match stmt {
            Stmt::Buf(BufStmt::CheckIndex, args) => {
                if let [i, len] = args.as_slice()
                    && i.kind == ExprKind::Local(index)
                    && let Some(buffer) = len_of(len)
                    && !out.iter().any(|(b, _)| *b == buffer)
                {
                    out.push((buffer, len.clone()));
                }
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                checks_of(then, index, out);
                checks_of(otherwise, index, out);
            }
            Stmt::Loop { body, step, .. } => {
                checks_of(body, index, out);
                checks_of(step, index, out);
            }
            Stmt::Block(body) => checks_of(body, index, out),
            _ => {}
        }
    }
}

/// Whether the body keeps every length, the bound and the checked buffers as
/// they are (see the module's documentation).
fn quiet(stmts: &[Stmt], index: LocalId, bound: &Expr, checked: &[LocalId]) -> bool {
    let mut held = vec![index];
    held.extend_from_slice(checked);
    match &bound.kind {
        ExprKind::Local(l) => held.push(*l),
        _ => held.extend(len_of(bound)),
    }
    stmts.iter().all(|s| quiet_stmt(s, &held))
}

fn quiet_stmt(stmt: &Stmt, held: &[LocalId]) -> bool {
    let all = |stmts: &[Stmt]| stmts.iter().all(|s| quiet_stmt(s, held));
    match stmt {
        Stmt::Set(local, value) => !held.contains(local) && quiet_expr(value),
        Stmt::Expr(e) => quiet_expr(e),
        Stmt::If {
            cond,
            then,
            otherwise,
        } => quiet_expr(cond) && all(then) && all(otherwise),
        Stmt::Loop { cond, body, step } => {
            cond.as_ref().is_none_or(quiet_expr) && all(body) && all(step)
        }
        Stmt::Block(body) => all(body),
        Stmt::Leave(_) | Stmt::Break | Stmt::Continue => true,
        Stmt::Return(value) => value.as_ref().is_none_or(quiet_expr),
        Stmt::ReturnMany(values) => values.iter().all(quiet_expr),
        Stmt::Buf(op, args) => {
            matches!(
                op,
                BufStmt::Set(_) | BufStmt::CheckIndex | BufStmt::CheckOverflow
            ) && args.iter().all(quiet_expr)
        }
        // A module variable written may be a bound: kept as written.
        Stmt::Release(_) | Stmt::CallMany { .. } | Stmt::SetGlobal(..) => false,
    }
}

/// Whether evaluating `e` calls nothing and moves nothing.
fn quiet_expr(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Const(_) | ExprKind::Local(_) | ExprKind::Borrow(_) | ExprKind::Global(_) => true,
        ExprKind::Call(..) | ExprKind::Move(_) => false,
        ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Math(_, x)
        | ExprKind::Alloc(x) => quiet_expr(x),
        ExprKind::Binary(_, a, b) => quiet_expr(a) && quiet_expr(b),
        ExprKind::Buf(_, args) => args.iter().all(quiet_expr),
    }
}

/// Whether `stmts` assign `local` anywhere.
pub(crate) fn assigns(stmts: &[Stmt], local: LocalId) -> bool {
    stmts.iter().any(|stmt| match stmt {
        Stmt::Set(l, _) => *l == local,
        Stmt::CallMany { targets, .. } => targets.contains(&local),
        Stmt::If {
            then, otherwise, ..
        } => assigns(then, local) || assigns(otherwise, local),
        Stmt::Loop { body, step, .. } => assigns(body, local) || assigns(step, local),
        Stmt::Block(body) => assigns(body, local),
        _ => false,
    })
}

/// The buffer `e` is the length of: `buf.len b`, lent or borrowed.
fn len_of(e: &Expr) -> Option<LocalId> {
    match &e.kind {
        ExprKind::Buf(BufOp::Len, args) => match args.as_slice() {
            [
                Expr {
                    kind: ExprKind::Local(b) | ExprKind::Borrow(b),
                    ..
                },
            ] => Some(*b),
            _ => None,
        },
        _ => None,
    }
}

fn is_len_of(e: &Expr, buffer: LocalId) -> bool {
    len_of(e) == Some(buffer)
}

fn binary(op: BinOp, a: Expr, b: Expr, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Binary(op, Box::new(a), Box::new(b)),
        ty,
    }
}
