//! `parallel for` ([decision 0007]): a counted loop whose turns may run on
//! several threads at once.
//!
//! The loop is lowered as any other, then its body is moved into a function
//! of its own, a job, `f__par0(start, end, captures…)`, which runs the turns
//! from `start` to `end`. The loop becomes a call to it over every turn: a
//! module built without threads runs it as it is, on one thread; with
//! threads, the loader splits the turns into ranges, one per thread.
//!
//! Each turn may read anything the loop does not change, and write only the
//! elements at its own index, so that no turn sees another, whatever the
//! order: the results are those of the loop run in order, bit for bit. A turn
//! may not assign a variable declared outside the loop, read at another index
//! an array the loop writes, call a function, make, move or release a buffer,
//! or leave the loop; `E0241` says which.
//!
//! [decision 0007]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0007-threads-over-shared-memory.md

use std::collections::{BTreeMap, BTreeSet};

use perasc_check::ty::{IntTy, Ty as CheckTy};
use perasc_diag::{Diagnostic, Fix, Label, Message, Span, codes};
use perasc_syntax::ast::{
    AssignOp, BinaryOp, ExprKind as AstExpr, StepOp, Stmt as AstStmt, StmtKind, Symbol,
};

use super::{FnState, Lowering};
use crate::{
    BinOp, BufOp, BufStmt, Const, Expr, ExprKind, FnId, Function, Local, LocalId, Stmt, Ty,
};

/// Why a loop cannot run in parallel: the `reason` of `E0241`.
enum Refusal {
    Shape,
    Outer(LocalId),
    Index,
    Reads,
    Calls,
    Heap,
    Exits,
    Global,
}

impl Refusal {
    const fn reason(&self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Outer(_) => "outer",
            Self::Index => "index",
            Self::Reads => "reads",
            Self::Calls => "calls",
            Self::Heap => "heap",
            Self::Exits => "exits",
            Self::Global => "global",
        }
    }
}

/// The head of a `for` loop: `(init; cond; update)`.
#[derive(Clone, Copy)]
pub(super) struct Head<'s> {
    pub(super) init: Option<&'s AstStmt>,
    pub(super) cond: Option<perasc_syntax::ast::ExprId>,
    pub(super) update: Option<&'s AstStmt>,
}

/// The parts of `for (let i = start; i < end; i++)`.
struct Counted<'s> {
    counter: Symbol,
    start: perasc_syntax::ast::ExprId,
    end: perasc_syntax::ast::ExprId,
    body: &'s AstStmt,
}

impl Lowering<'_> {
    /// `E0241` at `span`.
    fn refuse(&mut self, span: Span, refusal: &Refusal, state: &FnState) {
        let name = match refusal {
            Refusal::Outer(local) => usize::try_from(local.0)
                .ok()
                .and_then(|i| state.locals.get(i))
                .map_or_else(String::new, |l| l.name.clone()),
            _ => String::new(),
        };
        self.diagnostics
            .push(refused(span, refusal.reason(), &name));
    }

    /// `E0241` for a loop that is not counted: `parallel for…of`.
    pub(super) fn refuse_shape(&mut self, state: &FnState, span: Span) {
        self.refuse(span, &Refusal::Shape, state);
    }

    /// The counter, bounds and body of a loop written
    /// `for (let i = start; i < end; i++)` over an `i32`, or `None`.
    fn counted<'s>(
        &self,
        init: Option<&'s AstStmt>,
        cond: Option<perasc_syntax::ast::ExprId>,
        update: Option<&'s AstStmt>,
        body: &'s AstStmt,
    ) -> Option<Counted<'s>> {
        let StmtKind::Local(local) = &init?.kind else {
            return None;
        };
        let counter = local.name.symbol;
        let start = local.init?;
        if !local.mutable || self.check_type(start) != Some(CheckTy::Int(IntTy::I32)) {
            return None;
        }
        let is_counter = |e| matches!(self.ast.expr(e).kind, AstExpr::Name(s) if s == counter);
        let AstExpr::Binary {
            op: BinaryOp::Lt,
            lhs,
            rhs: end,
        } = self.ast.expr(cond?).kind
        else {
            return None;
        };
        if !is_counter(lhs) {
            return None;
        }
        let steps = match &update?.kind {
            StmtKind::Step {
                target,
                op: StepOp::Increment,
            } => is_counter(*target),
            StmtKind::Assign {
                target,
                op: AssignOp::Add,
                value,
            } => is_counter(*target) && matches!(self.ast.expr(*value).kind, AstExpr::Int(1)),
            _ => false,
        };
        steps.then_some(Counted {
            counter,
            start,
            end,
            body,
        })
    }

    /// `parallel for (let i = start; i < end; i++) body`: the body moved into
    /// a job, and a call to it over every turn.
    pub(super) fn parallel_for(
        &mut self,
        state: &mut FnState,
        span: Span,
        head: Head<'_>,
        body: &AstStmt,
        out: &mut Vec<Stmt>,
    ) {
        let Some(counted) = self.counted(head.init, head.cond, head.update, body) else {
            self.refuse(span, &Refusal::Shape, state);
            return;
        };
        state.open();
        // The bounds, computed once; the outer function's locals end here.
        let start = self.expr(state, counted.start);
        let end = self.expr(state, counted.end);
        let mark = state.locals.len();
        let start_local = state.temp(Ty::I32);
        let end_local = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(start_local, start));
        state.pre.push(Stmt::Set(end_local, end));
        state.flush(out);
        let text = self.ast.name(counted.counter).to_owned();
        let i = state.add(counted.counter, &text, Ty::I32);
        let read = |local| Expr {
            kind: ExprKind::Local(local),
            ty: Ty::I32,
        };
        let cond = super::binary(BinOp::Lt, read(i), read(end_local));
        let one = Expr {
            kind: ExprKind::Const(Const::Int(1)),
            ty: Ty::I32,
        };
        let step = vec![Stmt::Set(i, super::binary(BinOp::Add, read(i), one))];
        let mut job_body = vec![Stmt::Set(i, read(start_local))];
        // The turn's parts (decision 0031, option p2, prototype).
        let outer_turn = state.turn.replace((counted.counter, mark));
        let outer_parts = std::mem::take(&mut state.turn_parts);
        job_body.extend(self.loop_body(
            state,
            Some(cond),
            Vec::new(),
            counted.body,
            step,
            &mut Vec::new(),
        ));
        // What the loop leaves to close belongs to the outer function.
        state.close(out);
        state.turn = outer_turn;
        let parts = std::mem::replace(&mut state.turn_parts, outer_parts);
        match check(&job_body, i, mark, &parts) {
            Err(refusal) => {
                self.refuse(span, &refusal, state);
                return;
            }
            Ok(called) if !called.is_empty() => {
                self.pure_pending.push((span, self.unit, called));
            }
            Ok(_) => {}
        }
        let (job, args) = outline(
            &state.locals,
            &job_body,
            mark,
            (start_local, end_local),
            &self.current,
        );
        let id = FnId(self.job_base + u32::try_from(self.jobs.len()).unwrap_or(u32::MAX));
        let mut job = job;
        job.name = format!("{}__par{}", self.current, self.jobs.len());
        self.jobs.push(job);
        out.push(Stmt::Expr(Expr {
            kind: ExprKind::Call(id, args),
            ty: Ty::Void,
        }));
    }
}

/// `E0241` at `span`, for `reason`.
pub(super) fn refused(span: Span, reason: &str, name: &str) -> Diagnostic {
    let message = Message::new("check-parallel")
        .text("reason", reason)
        .text("name", name);
    Diagnostic {
        code: codes::PARALLEL,
        message,
        labels: vec![Label {
            span,
            message: Message::new("check-parallel-label"),
            primary: true,
        }],
        cause: Message::new("check-parallel-why"),
        fix: Fix {
            message: Message::new("check-parallel-fix"),
            edits: Vec::new(),
        },
    }
}

/// The functions of a module that a turn of `parallel for` may call
/// (decision 0031, option p1, prototype): with every function they call,
/// they make, move and release no buffer, write no element, read and write
/// no module variable, and call no function value. Found by a fixed point
/// over the call graph, every function pure until shown otherwise.
pub(super) fn pure_functions(functions: &[Function]) -> Vec<bool> {
    let mut callees: Vec<BTreeSet<usize>> = Vec::with_capacity(functions.len());
    let mut pure: Vec<bool> = Vec::with_capacity(functions.len());
    for f in functions {
        let mut calls = BTreeSet::new();
        let mut alone = true;
        let _ = walk(&f.body, 0, 0, &mut |stmt, _, _| {
            match stmt {
                Stmt::Buf(BufStmt::CheckIndex | BufStmt::CheckOverflow, _) => {}
                Stmt::Buf(..)
                | Stmt::Release(_)
                | Stmt::SetGlobal(..)
                | Stmt::CallManyIndirect { .. } => alone = false,
                Stmt::CallMany { callee, .. } => {
                    calls.insert(callee.0 as usize);
                }
                _ => {}
            }
            Ok(())
        });
        crate::each_expr(&f.body, &mut |e| match &e.kind {
            ExprKind::Call(id, _) => {
                calls.insert(id.0 as usize);
            }
            ExprKind::Buf(BufOp::Get(_) | BufOp::Len, _) | ExprKind::Borrow(_) => {}
            ExprKind::CallIndirect(..)
            | ExprKind::Global(_)
            | ExprKind::Buf(..)
            | ExprKind::Move(_)
            | ExprKind::Alloc(_)
            | ExprKind::Env(..) => alone = false,
            _ => {}
        });
        callees.push(calls);
        pure.push(alone);
    }
    loop {
        let mut changed = false;
        for (i, calls) in callees.iter().enumerate() {
            if pure[i]
                && calls
                    .iter()
                    .any(|&c| !pure.get(c).copied().unwrap_or(false))
            {
                pure[i] = false;
                changed = true;
            }
        }
        if !changed {
            return pure;
        }
    }
}

/// Whether the turns of a loop, counted by `counter`, are independent: the
/// locals before `mark` are the outer function's. The functions a turn
/// calls, when calls are allowed (decision 0031, option p1), are given back,
/// for [`pure_functions`] to judge once the module is lowered.
fn check(
    body: &[Stmt],
    counter: LocalId,
    mark: usize,
    parts: &[(LocalId, LocalId)],
) -> Result<Vec<FnId>, Refusal> {
    let calls_allowed = perasc_check::proto_par() != "p0";
    let mut called = Vec::new();
    // `b * k + j` of a part taken at the counter: its own elements.
    let in_part = |index: &Expr| match &index.kind {
        ExprKind::Binary(BinOp::Add, offset, _) => {
            matches!(offset.kind, ExprKind::Local(l) if parts.iter().any(|(o, _)| *o == l))
        }
        _ => false,
    };
    let mut by_parts = BTreeSet::new();
    let mut written = BTreeSet::new();
    walk(body, 0, 0, &mut |stmt, loops, blocks| {
        match stmt {
            Stmt::Set(local, _) if !inner(*local, mark) => return Err(Refusal::Outer(*local)),
            Stmt::Buf(BufStmt::Set(_), args) => {
                let [buffer, index, _] = args.as_slice() else {
                    return Err(Refusal::Heap);
                };
                let set = if in_part(index) {
                    &mut by_parts
                } else if is_local(index, counter) {
                    &mut written
                } else {
                    return Err(Refusal::Index);
                };
                if let Some(local) = handle(buffer) {
                    set.insert(local.0);
                }
            }
            // A bounds check reads the length only.
            Stmt::Buf(BufStmt::CheckIndex | BufStmt::CheckOverflow, _) => {}
            Stmt::Buf(..) | Stmt::Release(_) => return Err(Refusal::Heap),
            Stmt::CallMany { callee, .. } if calls_allowed => called.push(*callee),
            Stmt::CallMany { .. } | Stmt::CallManyIndirect { .. } => return Err(Refusal::Calls),
            // Each thread has its own module variables (decision 0023).
            Stmt::SetGlobal(..) => return Err(Refusal::Global),
            Stmt::Return(_) | Stmt::ReturnMany(_) => return Err(Refusal::Exits),
            // The parallel loop is the outermost one in the job.
            Stmt::Break if loops <= 1 => return Err(Refusal::Exits),
            Stmt::Leave(n) if *n >= blocks => return Err(Refusal::Exits),
            _ => {}
        }
        Ok(())
    })?;
    // A buffer written both at the counter and through parts: their elements
    // could meet.
    if written.iter().any(|b| by_parts.contains(b)) {
        return Err(Refusal::Index);
    }
    walk_exprs(body, &mut |e| match &e.kind {
        ExprKind::Call(id, _) if calls_allowed => {
            called.push(*id);
            Ok(())
        }
        ExprKind::Call(..) | ExprKind::CallIndirect(..) => Err(Refusal::Calls),
        ExprKind::Global(_) => Err(Refusal::Global),
        ExprKind::Buf(BufOp::Get(_), args) => {
            let buffer = args.first().and_then(handle).map(|l| l.0);
            let index = args.get(1);
            let at_counter = index.is_some_and(|i| is_local(i, counter));
            let at_part = index.is_some_and(in_part);
            if buffer.is_some_and(|b| written.contains(&b)) && !at_counter
                || buffer.is_some_and(|b| by_parts.contains(&b)) && !at_part
            {
                Err(Refusal::Reads)
            } else {
                Ok(())
            }
        }
        ExprKind::Buf(BufOp::Len, _) => Ok(()),
        ExprKind::Buf(..) | ExprKind::Move(_) | ExprKind::Alloc(_) => Err(Refusal::Heap),
        _ => Ok(()),
    })?;
    Ok(called)
}

fn inner(local: LocalId, mark: usize) -> bool {
    usize::try_from(local.0).is_ok_and(|i| i >= mark)
}

fn is_local(e: &Expr, local: LocalId) -> bool {
    matches!(e.kind, ExprKind::Local(l) if l == local)
}

/// The local a buffer operand lends.
const fn handle(e: &Expr) -> Option<LocalId> {
    match e.kind {
        ExprKind::Local(l) | ExprKind::Borrow(l) => Some(l),
        _ => None,
    }
}

/// Calls `visit` on every statement, with the loops and blocks around it in
/// the job.
fn walk(
    stmts: &[Stmt],
    loops: u32,
    blocks: u32,
    visit: &mut impl FnMut(&Stmt, u32, u32) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    for stmt in stmts {
        visit(stmt, loops, blocks)?;
        match stmt {
            Stmt::If {
                then, otherwise, ..
            } => {
                walk(then, loops, blocks, visit)?;
                walk(otherwise, loops, blocks, visit)?;
            }
            Stmt::Loop { body, step, .. } => {
                walk(body, loops + 1, blocks, visit)?;
                walk(step, loops + 1, blocks, visit)?;
            }
            Stmt::Block(body) => walk(body, loops, blocks + 1, visit)?,
            _ => {}
        }
    }
    Ok(())
}

/// Calls `visit` on every expression, and every one inside it.
fn walk_exprs(
    stmts: &[Stmt],
    visit: &mut impl FnMut(&Expr) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    let mut result = Ok(());
    crate::each_expr(stmts, &mut |e| {
        if result.is_ok() {
            result = visit(e);
        }
    });
    result
}

/// The job: its parameters `start`, `end`, then the outer locals the body
/// reads, lent; its locals, those of the body. And the arguments of its call.
fn outline(
    outer: &[Local],
    body: &[Stmt],
    mark: usize,
    bounds: (LocalId, LocalId),
    current: &str,
) -> (Function, Vec<Expr>) {
    let mut captured = BTreeSet::new();
    let mut used = BTreeSet::new();
    crate::each_expr(body, &mut |e| {
        if let ExprKind::Local(l) | ExprKind::Borrow(l) | ExprKind::Move(l) = e.kind {
            used.insert(l.0);
        }
    });
    collect_set_targets(body, &mut used);
    for &l in &used {
        if !inner(LocalId(l), mark) {
            captured.insert(l);
        }
    }
    let ty_of = |l: u32| {
        usize::try_from(l)
            .ok()
            .and_then(|i| outer.get(i))
            .map_or(Ty::I32, |local| local.ty)
    };
    let name_of = |l: u32| {
        usize::try_from(l)
            .ok()
            .and_then(|i| outer.get(i))
            .map_or_else(String::new, |local| local.name.clone())
    };
    let mut map: BTreeMap<u32, u32> = BTreeMap::new();
    let mut locals = vec![
        Local {
            name: "start".to_owned(),
            ty: Ty::I32,
        },
        Local {
            name: "end".to_owned(),
            ty: Ty::I32,
        },
    ];
    map.insert(bounds.0.0, 0);
    map.insert(bounds.1.0, 1);
    let mut args = vec![
        Expr {
            kind: ExprKind::Local(bounds.0),
            ty: Ty::I32,
        },
        Expr {
            kind: ExprKind::Local(bounds.1),
            ty: Ty::I32,
        },
    ];
    // A buffer the outer function owns is lent to the job.
    let mut lent = BTreeSet::new();
    for &l in &captured {
        let ty = ty_of(l);
        let (param, arg) = match ty {
            Ty::Owned => {
                lent.insert(l);
                (Ty::Borrowed, ExprKind::Borrow(LocalId(l)))
            }
            other => (other, ExprKind::Local(LocalId(l))),
        };
        map.insert(l, u32::try_from(locals.len()).unwrap_or(u32::MAX));
        locals.push(Local {
            name: name_of(l),
            ty: param,
        });
        args.push(Expr {
            kind: arg,
            ty: if ty == Ty::Owned { Ty::Borrowed } else { ty },
        });
    }
    let param_count = u32::try_from(locals.len()).unwrap_or(u32::MAX);
    for &l in &used {
        if let std::collections::btree_map::Entry::Vacant(slot) = map.entry(l) {
            slot.insert(u32::try_from(locals.len()).unwrap_or(u32::MAX));
            locals.push(Local {
                name: name_of(l),
                ty: ty_of(l),
            });
        }
    }
    let rename = Rename { map, lent };
    let body = rename.stmts(body);
    (
        Function {
            name: format!("{current}__par"),
            exported: false,
            job: true,
            param_count,
            locals,
            ret: Ty::Void,
            more: Vec::new(),
            shape: None,
            body,
        },
        args,
    )
}

/// Adds the locals that statements assign.
fn collect_set_targets(stmts: &[Stmt], used: &mut BTreeSet<u32>) {
    for stmt in stmts {
        match stmt {
            Stmt::Set(l, _) => {
                used.insert(l.0);
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                collect_set_targets(then, used);
                collect_set_targets(otherwise, used);
            }
            Stmt::Loop { body, step, .. } => {
                collect_set_targets(body, used);
                collect_set_targets(step, used);
            }
            Stmt::Block(body) => collect_set_targets(body, used),
            Stmt::CallMany { targets, .. } => {
                used.extend(targets.iter().map(|l| l.0));
            }
            _ => {}
        }
    }
}

/// The job's statements: each local renumbered, and a buffer the outer
/// function owned read through the parameter that lends it.
struct Rename {
    map: BTreeMap<u32, u32>,
    lent: BTreeSet<u32>,
}

impl Rename {
    fn local(&self, l: LocalId) -> LocalId {
        LocalId(self.map.get(&l.0).copied().unwrap_or(l.0))
    }

    fn stmts(&self, stmts: &[Stmt]) -> Vec<Stmt> {
        stmts
            .iter()
            .map(|stmt| match stmt {
                Stmt::Set(l, e) => Stmt::Set(self.local(*l), self.expr(e)),
                Stmt::Expr(e) => Stmt::Expr(self.expr(e)),
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                } => Stmt::If {
                    cond: self.expr(cond),
                    then: self.stmts(then),
                    otherwise: self.stmts(otherwise),
                },
                Stmt::Loop { cond, body, step } => Stmt::Loop {
                    cond: cond.as_ref().map(|c| self.expr(c)),
                    body: self.stmts(body),
                    step: self.stmts(step),
                },
                Stmt::Block(body) => Stmt::Block(self.stmts(body)),
                Stmt::Buf(op, args) => Stmt::Buf(*op, args.iter().map(|a| self.expr(a)).collect()),
                // A pure call (decision 0031, option p1, prototype).
                Stmt::CallMany {
                    targets,
                    callee,
                    args,
                } => Stmt::CallMany {
                    targets: targets.iter().map(|l| self.local(*l)).collect(),
                    callee: *callee,
                    args: args.iter().map(|a| self.expr(a)).collect(),
                },
                // `check` refused the others.
                other => other.clone(),
            })
            .collect()
    }

    fn expr(&self, e: &Expr) -> Expr {
        let kind = match &e.kind {
            ExprKind::Local(l) => ExprKind::Local(self.local(*l)),
            // Refused in a turn when lowered (decision 0023): never here.
            ExprKind::Global(g) => ExprKind::Global(*g),
            ExprKind::Borrow(l) if self.lent.contains(&l.0) => ExprKind::Local(self.local(*l)),
            ExprKind::Borrow(l) => ExprKind::Borrow(self.local(*l)),
            ExprKind::Move(l) => ExprKind::Move(self.local(*l)),
            ExprKind::Unary(op, x) => ExprKind::Unary(*op, Box::new(self.expr(x))),
            ExprKind::Binary(op, a, b) => {
                ExprKind::Binary(*op, Box::new(self.expr(a)), Box::new(self.expr(b)))
            }
            ExprKind::Call(id, args) => {
                ExprKind::Call(*id, args.iter().map(|a| self.expr(a)).collect())
            }
            ExprKind::Env(op, args) => {
                ExprKind::Env(*op, args.iter().map(|a| self.expr(a)).collect())
            }
            ExprKind::CallIndirect(callee, args) => ExprKind::CallIndirect(
                Box::new(self.expr(callee)),
                args.iter().map(|a| self.expr(a)).collect(),
            ),
            ExprKind::Convert(x) => ExprKind::Convert(Box::new(self.expr(x))),
            ExprKind::Math(op, x) => ExprKind::Math(*op, Box::new(self.expr(x))),
            ExprKind::Alloc(x) => ExprKind::Alloc(Box::new(self.expr(x))),
            ExprKind::Buf(op, args) => {
                ExprKind::Buf(*op, args.iter().map(|a| self.expr(a)).collect())
            }
            ExprKind::Const(c) => ExprKind::Const(*c),
        };
        Expr { kind, ty: e.ty }
    }
}
