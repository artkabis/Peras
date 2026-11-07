//! The release verifier: every heap block is released exactly once, on every
//! path (decision 0001, precondition 1).
//!
//! It runs after the type checks of [`crate::verify`], on each function that
//! uses `owned` or `borrowed` values. For every `owned` local it tracks one bit
//! along each path of the structured IR: the local holds a block, or it is
//! empty. Locals start empty; an `owned` parameter starts full, since the caller
//! gave it up (`sink`). Then:
//!
//! - `l = alloc`, `l = move m`, `l = call` fill `l`, which must be empty:
//!   otherwise its block would be lost;
//! - `move l` and `release l` empty `l`, which must be full; `borrow l` needs it
//!   full and leaves it so;
//! - at `return`, every local must be empty, after the result moved its block
//!   out; so must a `void` function that ends;
//! - where paths meet (after an `if`, a loop, a block, `&&` and `||`), every
//!   local must be full on all of them or empty on all of them;
//! - each turn of a loop must leave the locals as it found them, at the bottom
//!   and at each `continue`: a turn that allocates releases, or moves out.
//!
//! A trap ends the whole module instance, its memory included: it needs no
//! release, and the debug allocator's leak check is not run after one.
//!
//! The verifier proves the property, or refuses: it never guesses. The first
//! problem of a function is reported, since the states after it mean nothing.

use crate::{BinOp, Expr, ExprKind, Function, LocalId, Module, Stmt, Ty};

/// Checks every function that uses the heap; one line per refused function.
pub fn check(module: &Module, problems: &mut Vec<String>) {
    for function in module.functions.iter().filter(|f| f.uses_heap()) {
        if let Err(problem) = Checker::new(function).function() {
            problems.push(format!("fn {}: {problem}", function.name));
        }
    }
}

/// For each local: whether it holds a block. Only `owned` locals ever do.
type State = Vec<bool>;

/// Where an exit leads, and the states that reach it.
#[derive(Default)]
struct Exits {
    breaks: Vec<State>,
    continues: Vec<State>,
}

struct Checker<'a> {
    function: &'a Function,
    /// One entry per enclosing loop, innermost last.
    loops: Vec<Exits>,
    /// The states reaching the end of each enclosing block, innermost last.
    blocks: Vec<Vec<State>>,
}

impl<'a> Checker<'a> {
    fn new(function: &'a Function) -> Self {
        Self {
            function,
            loops: Vec::new(),
            blocks: Vec::new(),
        }
    }

    fn function(&mut self) -> Result<(), String> {
        let params = usize::try_from(self.function.param_count).unwrap_or(usize::MAX);
        let start: State = self
            .function
            .locals
            .iter()
            .enumerate()
            .map(|(i, l)| i < params && l.ty == Ty::Owned)
            .collect();
        let end = self.stmts(&self.function.body, start)?;
        // Falling off the end of a function with a result traps.
        if let Some(state) = end
            && self.function.ret == Ty::Void
        {
            all_empty(&state, "at the end of the function")?;
        }
        Ok(())
    }

    /// The state after `stmts`, or `None` if control never falls through.
    fn stmts(&mut self, stmts: &[Stmt], mut state: State) -> Result<Option<State>, String> {
        for stmt in stmts {
            match self.stmt(stmt, state)? {
                Some(next) => state = next,
                None => return Ok(None),
            }
        }
        Ok(Some(state))
    }

    fn stmt(&mut self, stmt: &Stmt, mut state: State) -> Result<Option<State>, String> {
        match stmt {
            Stmt::Set(local, value) => {
                expr(value, &mut state)?;
                self.fill(&mut state, *local)?;
            }
            Stmt::CallMany { targets, args, .. } => {
                for arg in args {
                    expr(arg, &mut state)?;
                }
                for target in targets {
                    self.fill(&mut state, *target)?;
                }
            }
            Stmt::ReturnMany(values) => {
                for value in values {
                    expr(value, &mut state)?;
                }
                all_empty(&state, "when the function returns")?;
                return Ok(None);
            }
            Stmt::Expr(e) => {
                expr(e, &mut state)?;
                if e.ty == Ty::Owned {
                    return Err("a new block is dropped at once: it leaks".to_owned());
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                expr(cond, &mut state)?;
                let a = self.stmts(then, state.clone())?;
                let b = self.stmts(otherwise, state)?;
                return join(
                    &[a, b].into_iter().flatten().collect::<Vec<_>>(),
                    "after an if",
                );
            }
            Stmt::Loop { cond, body, step } => return self.loop_(cond.as_ref(), body, step, state),
            Stmt::Block(body) => {
                self.blocks.push(Vec::new());
                let end = self.stmts(body, state);
                let mut reaching = self.blocks.pop().unwrap_or_default();
                reaching.extend(end?);
                return join(&reaching, "after a block");
            }
            Stmt::Leave(depth) => {
                let index = self
                    .blocks
                    .len()
                    .checked_sub(1 + usize::try_from(*depth).unwrap_or(usize::MAX));
                match index.and_then(|i| self.blocks.get_mut(i)) {
                    Some(reaching) => reaching.push(state),
                    None => return Err(format!("`leave {depth}` outside its block")),
                }
                return Ok(None);
            }
            Stmt::Break | Stmt::Continue => {
                let Some(exits) = self.loops.last_mut() else {
                    return Err("`break` or `continue` outside a loop".to_owned());
                };
                if matches!(stmt, Stmt::Break) {
                    exits.breaks.push(state);
                } else {
                    exits.continues.push(state);
                }
                return Ok(None);
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    expr(value, &mut state)?;
                }
                all_empty(&state, "when the function returns")?;
                return Ok(None);
            }
            // A buffer statement reads its operands; `buf.release` moves its
            // buffer out, which `verify` requires to be a `move`.
            Stmt::Buf(_, args) => {
                for arg in args {
                    expr(arg, &mut state)?;
                }
            }
            Stmt::Release(local) => {
                if !holds(&state, *local) {
                    return Err(format!(
                        "%{} is released but holds no block: released twice, moved, or never set",
                        local.0
                    ));
                }
                set(&mut state, *local, false);
            }
        }
        Ok(Some(state))
    }

    /// A loop: each turn must leave the locals as it found them.
    fn loop_(
        &mut self,
        cond: Option<&Expr>,
        body: &[Stmt],
        step: &[Stmt],
        head: State,
    ) -> Result<Option<State>, String> {
        if let Some(cond) = cond {
            let mut after = head.clone();
            expr(cond, &mut after)?;
            if after != head {
                return Err("a loop condition moves a block".to_owned());
            }
        }
        self.loops.push(Exits::default());
        let end = self.stmts(body, head.clone());
        let exits = self.loops.pop().unwrap_or_default();
        let mut into_step = exits.continues;
        into_step.extend(end?);
        if !into_step.is_empty() {
            let at_step = join(&into_step, "before the next turn of a loop")?;
            // `break` and `continue` in a step are never lowered: refuse them.
            let depth = self.loops.len();
            self.loops.push(Exits::default());
            let after_step = self.stmts(step, at_step.unwrap_or_default());
            let step_exits = self.loops.pop().unwrap_or_default();
            if self.loops.len() != depth
                || !step_exits.breaks.is_empty()
                || !step_exits.continues.is_empty()
            {
                return Err("`break` or `continue` in a loop step".to_owned());
            }
            if let Some(back) = after_step?
                && let Some(local) = first_difference(&back, &head)
            {
                return Err(format!(
                    "a turn of a loop {} %{}: every turn must leave it as the loop found it",
                    if back[local] { "fills" } else { "empties" },
                    local
                ));
            }
        }
        // The loop ends when its condition is false, at the top, or by `break`.
        let mut leaving = exits.breaks;
        if cond.is_some() {
            leaving.push(head);
        }
        join(&leaving, "after a loop")
    }

    /// `local` receives a value; an `owned` local must be empty first.
    fn fill(&self, state: &mut State, local: LocalId) -> Result<(), String> {
        if self.is_owned(local) {
            if holds(state, local) {
                return Err(format!(
                    "%{} is overwritten while it holds a block: that block leaks",
                    local.0
                ));
            }
            set(state, local, true);
        }
        Ok(())
    }

    fn is_owned(&self, local: LocalId) -> bool {
        usize::try_from(local.0)
            .ok()
            .and_then(|i| self.function.locals.get(i))
            .is_some_and(|l| l.ty == Ty::Owned)
    }
}

/// The state after an expression, evaluated left to right.
fn expr(e: &Expr, state: &mut State) -> Result<(), String> {
    match &e.kind {
        ExprKind::Const(_) | ExprKind::Local(_) => {}
        ExprKind::Move(local) | ExprKind::Borrow(local) => {
            if !holds(state, *local) {
                return Err(format!(
                    "%{} is used after its block was released or moved",
                    local.0
                ));
            }
            if matches!(e.kind, ExprKind::Move(_)) {
                set(state, *local, false);
            }
        }
        ExprKind::Binary(BinOp::And | BinOp::Or, a, b) => {
            expr(a, state)?;
            // `b` may not run: it must not change who holds what.
            let mut after = state.clone();
            expr(b, &mut after)?;
            if after != *state {
                return Err("a block is moved on one side of `&&` or `||` only".to_owned());
            }
        }
        ExprKind::Binary(_, a, b) => {
            expr(a, state)?;
            expr(b, state)?;
        }
        ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Math(_, x)
        | ExprKind::Alloc(x) => {
            expr(x, state)?;
        }
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => {
            for arg in args {
                expr(arg, state)?;
            }
        }
    }
    Ok(())
}

/// The common state of the paths that meet; `None` if none does.
fn join(states: &[State], place: &str) -> Result<Option<State>, String> {
    let Some((first, rest)) = states.split_first() else {
        return Ok(None);
    };
    for other in rest {
        if let Some(local) = first_difference(first, other) {
            return Err(format!(
                "%{local} holds a block on one path and not on another, {place}"
            ));
        }
    }
    Ok(Some(first.clone()))
}

fn all_empty(state: &State, place: &str) -> Result<(), String> {
    match state.iter().position(|&full| full) {
        Some(local) => Err(format!("%{local} still holds a block {place}: it leaks")),
        None => Ok(()),
    }
}

fn holds(state: &State, local: LocalId) -> bool {
    usize::try_from(local.0)
        .ok()
        .and_then(|i| state.get(i))
        .copied()
        .unwrap_or(false)
}

fn set(state: &mut State, local: LocalId, full: bool) {
    if let Some(slot) = usize::try_from(local.0).ok().and_then(|i| state.get_mut(i)) {
        *slot = full;
    }
}

fn first_difference(a: &State, b: &State) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y)
}
