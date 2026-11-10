//! Ownership: moves, `inout` and `sink`, aliasing (spec, "Memory model").
//!
//! It runs after names and types are checked, on the same tree. A value on the
//! heap, an array, a string, or a structure or option holding one, has one
//! owner. Reading it borrows it; it **moves** where it is consumed:
//!
//! - as the value of a `let`, an assignment or a `return`;
//! - as an argument to a `sink` parameter, a structure constructor or `Some`;
//! - as an element of an array literal, or the argument of `push`.
//!
//! A moved variable cannot be used until it is given a new value (`E0204`).
//! What is only borrowed cannot be moved: a parameter that is not `sink`, a
//! field, an element, a module-level value (`E0209`). An `inout` parameter
//! takes `&x`, and only it does (`E0206`); during the call, no other argument
//! may reach the same variable (`E0207`).
//!
//! The analysis follows every path: after an `if`, a value moved on one side
//! is moved; in a loop, a value moved in one turn is moved at the start of the
//! next one, unless the turn gives it a new value or leaves the loop.

use std::collections::{BTreeMap, HashSet};

use perasc_diag::Span;
use perasc_syntax::ast::{
    ArmBody, AssignOp, BinaryOp, Convention, ExprId, ExprKind, FnDecl, Stmt, StmtKind, UnaryOp,
};

use crate::methods::Target;
use crate::scope::Kind;
use crate::ty::Ty;
use crate::{Checker, errors};

/// The variables moved on a path, with where each was moved.
type Moved = BTreeMap<usize, Span>;

/// A path through the code: what it moved, and whether it still goes on.
#[derive(Clone, Default)]
struct Path {
    moved: Moved,
    /// Where each variable last received its value, for the history an
    /// error shows: given here, moved there, used again here.
    given: BTreeMap<usize, Span>,
    /// The path left by `return`, `break` or `continue`.
    gone: bool,
}

/// What reaches the exits of a loop.
#[derive(Default)]
struct Exits {
    breaks: Vec<Moved>,
    continues: Vec<Moved>,
}

/// What one statement reads and passes `inout`: a variable changed by a call
/// cannot also be read elsewhere in the same statement, whose order of
/// evaluation would then decide what it reads.
#[derive(Default)]
struct Uses {
    /// Each place read: its variable, the fields on the way, and where.
    reads: Vec<(usize, Vec<String>, Span)>,
    /// Each place passed `inout`, the call, and the function called. The
    /// call's own arguments are evaluated before it: only reads outside it
    /// depend on the order of evaluation.
    inouts: Vec<(usize, Vec<String>, Span, String)>,
    /// Expressions inside a place already recorded whole.
    inside: HashSet<ExprId>,
}

/// Whether two places can be the same memory: one contains the other.
fn overlap(a: &[String], b: &[String]) -> bool {
    a.iter().zip(b).all(|(x, y)| x == y)
}

/// How an expression is used.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    /// Read, or lent for the time of a call.
    Read,
    /// Taken: a heap value moves.
    Consume,
}

pub(crate) struct Owner<'c, 'a> {
    checker: &'c mut Checker<'a>,
    /// The parameter conventions of each function, by its position.
    conventions: Vec<Vec<Convention>>,
    loops: Vec<Exits>,
    /// While a loop is checked a first time, to learn what one turn moves.
    silent: u32,
    /// Spans already reported, so that the second pass over a loop does not
    /// repeat an error.
    reported: HashSet<u32>,
    /// The variables `for…of` loops are going through, with the span of each
    /// loop's array: they cannot change during the loop.
    iterated: Vec<(usize, Span)>,
    /// One entry per statement being checked, innermost last.
    uses: Vec<Uses>,
}

impl<'c, 'a> Owner<'c, 'a> {
    pub(crate) fn new(checker: &'c mut Checker<'a>) -> Self {
        let conventions = checker
            .ast
            .functions()
            .iter()
            .map(|decl| decl.params.iter().map(|p| p.convention).collect())
            .collect();
        Self {
            checker,
            conventions,
            loops: Vec::new(),
            silent: 0,
            reported: HashSet::new(),
            iterated: Vec::new(),
            uses: Vec::new(),
        }
    }

    pub(crate) fn module(&mut self) {
        let ast = self.checker.ast;
        for (index, decl) in ast.functions().into_iter().enumerate() {
            // Its type parameters: a value of one moves unless it is `Copy`.
            self.checker.generics = self
                .checker
                .fns
                .get(index)
                .map(|f| f.generics.clone())
                .unwrap_or_default();
            self.function(decl);
        }
        self.checker.generics.clear();
    }

    fn function(&mut self, decl: &FnDecl) {
        let mut path = Path::default();
        self.stmts(&decl.body.stmts, &mut path);
    }

    fn report(&mut self, diagnostic: perasc_diag::Diagnostic) {
        if self.silent > 0 {
            return;
        }
        let at = diagnostic.primary_span().map_or(0, |s| s.start);
        if self.reported.insert(at) {
            self.checker.report(diagnostic);
        }
    }

    // --- Statements ----------------------------------------------------------

    fn stmts(&mut self, stmts: &[Stmt], path: &mut Path) {
        for stmt in stmts {
            if path.gone {
                return;
            }
            self.stmt(stmt, path);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, path: &mut Path) {
        let simple = matches!(
            stmt.kind,
            StmtKind::Local(_)
                | StmtKind::Return(_)
                | StmtKind::Expr(_)
                | StmtKind::Assign { .. }
                | StmtKind::Step { .. }
        );
        if simple {
            self.uses.push(Uses::default());
        }
        self.stmt_inner(stmt, path);
        if simple {
            self.check_uses();
        }
    }

    /// An expression on its own: a condition, the array of a `for…of`.
    fn expr_alone(&mut self, id: ExprId, mode: Use, path: &mut Path) {
        self.uses.push(Uses::default());
        self.expr(id, mode, path);
        self.check_uses();
    }

    /// `E0207` for a variable passed `inout` and read elsewhere in the same
    /// statement.
    fn check_uses(&mut self) {
        let Some(uses) = self.uses.pop() else {
            return;
        };
        for (binding, place, arg, callee) in &uses.inouts {
            // Reads inside a call with an `inout` argument happen before it;
            // the call itself checked its arguments against each other.
            let within = |span: &Span| {
                uses.inouts
                    .iter()
                    .any(|(_, _, a, _)| a.start <= span.start && span.end <= a.end)
            };
            let other = uses
                .reads
                .iter()
                .find(|(b, path, span)| b == binding && overlap(place, path) && !within(span));
            if let Some((_, _, read)) = other {
                let name = self.binding_name(*binding);
                self.report(errors::alias(*read, &name, callee, *arg, None));
            }
        }
    }

    fn stmt_inner(&mut self, stmt: &Stmt, path: &mut Path) {
        match &stmt.kind {
            StmtKind::Local(local) => {
                if let Some(init) = local.init {
                    self.expr(init, Use::Consume, path);
                }
                // A new variable, also at each turn of a loop.
                if let Some(&binding) = self.checker.declared.get(&local.name.span.start) {
                    path.moved.remove(&binding);
                    path.given.insert(binding, local.name.span);
                }
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(*value, Use::Consume, path);
                }
                path.gone = true;
            }
            StmtKind::Expr(e) => self.expr(*e, Use::Read, path),
            StmtKind::Assign { target, op, value } => {
                let consume = if *op == AssignOp::Assign {
                    Use::Consume
                } else {
                    Use::Read
                };
                self.expr(*value, consume, path);
                self.target(*target, *op, path);
            }
            StmtKind::Step { target, .. } => self.expr(*target, Use::Read, path),
            StmtKind::Block(block) => self.stmts(&block.stmts, path),
            StmtKind::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr_alone(*cond, Use::Read, path);
                let mut other = path.clone();
                self.stmt(then, path);
                if let Some(otherwise) = otherwise {
                    self.stmt(otherwise, &mut other);
                }
                merge(path, &other);
            }
            StmtKind::While { cond, body } => self.loop_(Some(*cond), body, None, path),
            StmtKind::For {
                init,
                cond,
                update,
                body,
            } => {
                if let Some(init) = init {
                    self.stmt(init, path);
                }
                self.loop_(*cond, body, update.as_deref(), path);
            }
            StmtKind::ForOf {
                name,
                iterable,
                body,
                ..
            } => {
                self.expr_alone(*iterable, Use::Read, path);
                if let Some(&binding) = self.checker.declared.get(&name.span.start) {
                    path.moved.remove(&binding);
                }
                let root = self.root(*iterable);
                let span = self.checker.ast.expr(*iterable).span;
                if let Some(root) = root {
                    self.iterated.push((root, span));
                }
                self.loop_(None, body, None, path);
                if root.is_some() {
                    self.iterated.pop();
                }
            }
            StmtKind::Break | StmtKind::Continue => {
                if let Some(exits) = self.loops.last_mut() {
                    let moved = path.moved.clone();
                    if matches!(stmt.kind, StmtKind::Break) {
                        exits.breaks.push(moved);
                    } else {
                        exits.continues.push(moved);
                    }
                }
                path.gone = true;
            }
            StmtKind::Error => {}
        }
    }

    /// A loop. A first, silent pass learns what one turn moves; the second
    /// starts from it, so that a value moved in one turn and used in the next
    /// is reported.
    fn loop_(&mut self, cond: Option<ExprId>, body: &Stmt, update: Option<&Stmt>, path: &mut Path) {
        self.silent += 1;
        let mut first = path.clone();
        let back = self.turn(cond, body, update, &mut first);
        self.silent -= 1;
        let mut head = path.clone();
        for moved in back {
            head.moved.extend(moved);
        }
        let mut turn = head.clone();
        self.loops.push(Exits::default());
        if let Some(cond) = cond {
            self.expr_alone(cond, Use::Read, &mut turn);
        }
        // The loop ends at its condition, or forever loops end by `break`.
        let at_cond = turn.moved.clone();
        self.stmt(body, &mut turn);
        if let Some(update) = update
            && !turn.gone
        {
            self.stmt(update, &mut turn);
        }
        let exits = self.loops.pop().unwrap_or_default();
        let mut leaving: Vec<Moved> = exits.breaks;
        if cond.is_some() || leaving.is_empty() {
            leaving.push(at_cond);
        }
        path.moved = leaving.into_iter().flatten().collect();
    }

    /// One turn of a loop, and what reaches its next turn.
    fn turn(
        &mut self,
        cond: Option<ExprId>,
        body: &Stmt,
        update: Option<&Stmt>,
        path: &mut Path,
    ) -> Vec<Moved> {
        self.loops.push(Exits::default());
        if let Some(cond) = cond {
            self.expr_alone(cond, Use::Read, path);
        }
        self.stmt(body, path);
        let exits = self.loops.pop().unwrap_or_default();
        let mut back = exits.continues;
        if !path.gone {
            back.push(path.moved.clone());
        }
        if let Some(update) = update {
            let mut after = Path {
                moved: back.iter().flatten().map(|(k, v)| (*k, *v)).collect(),
                given: path.given.clone(),
                gone: false,
            };
            self.stmt(update, &mut after);
            return vec![after.moved];
        }
        back
    }

    /// The target of an assignment. Assigning a whole variable gives it a new
    /// value, moved or not; a compound assignment reads it first.
    fn target(&mut self, target: ExprId, op: AssignOp, path: &mut Path) {
        let expr = self.checker.ast.expr(target);
        match &expr.kind {
            ExprKind::Name(_) if op == AssignOp::Assign => {
                if let Some(&binding) = self.checker.resolved.get(&target) {
                    self.not_iterated(binding, expr.span);
                    path.moved.remove(&binding);
                    path.given.insert(binding, expr.span);
                }
            }
            ExprKind::Paren(inner) => self.target(*inner, op, path),
            _ => {
                let before = self.uses.last().map_or(0, |u| u.reads.len());
                self.expr(target, Use::Read, path);
                // `p.x = f(&p)` writes `p.x` after the call: only a compound
                // assignment reads the target first.
                if op == AssignOp::Assign
                    && let Some(uses) = self.uses.last_mut()
                {
                    uses.reads.truncate(before);
                }
            }
        }
    }

    // --- Expressions ---------------------------------------------------------

    fn expr(&mut self, id: ExprId, mode: Use, path: &mut Path) {
        let expr = self.checker.ast.expr(id);
        let span = expr.span;
        self.record_place(id, span);
        match &expr.kind {
            ExprKind::Name(_) => self.name(id, span, mode, path),
            ExprKind::Paren(inner) => self.expr(*inner, mode, path),
            ExprKind::Match { scrutinee, arms } => {
                // The value matched is lent to the arms; each arm is a path.
                self.expr(*scrutinee, Use::Read, path);
                let start = path.clone();
                let mut joined = Path {
                    gone: true,
                    ..Path::default()
                };
                for arm in arms {
                    let mut arm_path = start.clone();
                    match &arm.body {
                        ArmBody::Expr(value) => self.expr(*value, mode, &mut arm_path),
                        ArmBody::Block(block) => self.stmts(&block.stmts, &mut arm_path),
                    }
                    merge(&mut joined, &arm_path);
                }
                *path = joined;
            }
            ExprKind::Field { base, .. } => {
                self.expr(*base, Use::Read, path);
                if mode == Use::Consume && self.moves(id) {
                    self.cannot_move(id, span, "field");
                }
            }
            ExprKind::Index { base, index } => {
                self.expr(*base, Use::Read, path);
                self.expr(*index, Use::Read, path);
                if mode == Use::Consume && self.moves(id) {
                    self.cannot_move(id, span, "element");
                }
            }
            ExprKind::Unary { operand, .. } | ExprKind::Try(operand) => {
                self.expr(*operand, Use::Read, path);
            }
            // Its values are read, in order, into the new string.
            ExprKind::Template { exprs, .. } => {
                for &e in exprs {
                    self.expr(e, Use::Read, path);
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                self.expr(*lhs, Use::Read, path);
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    // The right side may not run.
                    let mut maybe = path.clone();
                    self.expr(*rhs, Use::Read, &mut maybe);
                    path.moved.extend(maybe.moved);
                } else {
                    self.expr(*rhs, Use::Read, path);
                }
            }
            // An arrow function is the body of a loop: run twice, so that a
            // value it moves is seen used again on the next turn.
            ExprKind::Lambda { body, .. } => {
                self.expr(*body, mode, path);
                self.expr(*body, mode, path);
            }
            ExprKind::Array(items) | ExprKind::Tuple(items) => {
                for &item in items {
                    self.expr(item, Use::Consume, path);
                }
            }
            ExprKind::Call { callee, args, .. } => self.call(id, *callee, args, span, path),
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Error => {}
        }
    }

    /// A variable, read or moved.
    fn name(&mut self, id: ExprId, span: Span, mode: Use, path: &mut Path) {
        let Some(&binding) = self.checker.resolved.get(&id) else {
            return;
        };
        if let Some(uses) = self.uses.last_mut()
            && !uses.inside.contains(&id)
        {
            uses.reads.push((binding, Vec::new(), span));
        }
        if let Some(&moved_at) = path.moved.get(&binding) {
            let name = self.binding_name(binding);
            let copyable = self.copyable(binding);
            let given = path.given.get(&binding).copied().or_else(|| {
                self.checker
                    .scopes
                    .bindings
                    .get(binding)
                    .and_then(|b| b.span)
            });
            self.report(errors::moved(span, &name, moved_at, given, copyable));
            return;
        }
        if mode == Use::Read || !self.moves(id) {
            return;
        }
        let kind = self
            .checker
            .scopes
            .bindings
            .get(binding)
            .map(|b| b.kind.clone());
        match kind {
            Some(Kind::Param {
                convention: convention @ (Convention::Let | Convention::Inout),
                type_start,
            }) => {
                let name = self.binding_name(binding);
                // `sink` fixes a read-only parameter; `inout` belongs to the caller.
                let sink_at = (convention == Convention::Let).then_some(type_start);
                self.report(errors::move_borrowed_parameter(span, &name, sink_at));
            }
            // A constant is written where it is used: each use is a new value.
            Some(Kind::Global { mutable: false, .. }) => {}
            Some(Kind::Global { .. }) => self.cannot_move(id, span, "global"),
            // A name a pattern gives only lends a part of the value matched.
            Some(Kind::Pattern) => self.cannot_move(id, span, "binding"),
            _ => {
                self.not_iterated(binding, span);
                path.moved.insert(binding, span);
                self.checker.moves.insert(id);
            }
        }
    }

    /// `E0207` when a variable that a `for…of` goes through changes size or
    /// is replaced; changing its elements is allowed.
    fn not_iterated(&mut self, binding: usize, span: Span) {
        if let Some(&(_, loop_span)) = self.iterated.iter().find(|(b, _)| *b == binding) {
            let name = self.binding_name(binding);
            self.report(errors::changed_while_iterated(span, &name, loop_span));
        }
    }

    fn call(&mut self, id: ExprId, callee: ExprId, args: &[ExprId], span: Span, path: &mut Path) {
        let callee_expr = self.checker.ast.expr(callee);
        if let ExprKind::Field { base, name } = &callee_expr.kind
            && let Some(target) = self.checker.method_calls.get(&id).cloned()
        {
            // A method: `self` is an argument, written before the dot.
            let (conventions, receiver) = match target {
                Target::Fn { id, receiver } => (
                    self.conventions.get(id.0).cloned().unwrap_or_default(),
                    receiver,
                ),
                Target::Interface {
                    interface, method, ..
                } => {
                    let conventions = self
                        .checker
                        .interfaces
                        .get(interface)
                        .and_then(|i| i.methods.iter().find(|m| m.name == method))
                        .map(|m| m.conventions.clone())
                        .unwrap_or_default();
                    (conventions, true)
                }
            };
            let mut all = Vec::with_capacity(args.len() + 1);
            if receiver {
                all.push(*base);
            }
            all.extend_from_slice(args);
            let text = self.checker.name(name.symbol).to_owned();
            self.arguments(&text, &all, &conventions, span, path, receiver);
            return;
        }
        match &callee_expr.kind {
            ExprKind::Name(symbol) => {
                let kind = self
                    .checker
                    .resolved
                    .get(&callee)
                    .and_then(|&i| self.checker.scopes.bindings.get(i))
                    .map(|b| b.kind.clone());
                match kind {
                    Some(Kind::Fn(id)) => {
                        let conventions = self.conventions.get(id.0).cloned().unwrap_or_default();
                        let name = self.checker.name(*symbol).to_owned();
                        self.arguments(&name, args, &conventions, span, path, false);
                    }
                    // A constructor and `Some` take their arguments.
                    _ => {
                        for &arg in args {
                            self.expr(arg, Use::Consume, path);
                        }
                    }
                }
            }
            ExprKind::Field { base, name } => {
                self.expr(*base, Use::Read, path);
                let method = self.checker.name(name.symbol);
                if matches!(
                    method,
                    "push" | "pop" | "reverse" | "sort" | "set" | "add" | "delete" | "clear"
                ) && let Some(root) = self.root(*base)
                {
                    let span = self.checker.ast.expr(*base).span;
                    self.not_iterated(root, span);
                }
                // `set` and `add` take a map's or a set's key; `get`, `has` and
                // `delete` only lend theirs.
                let consume = matches!(method, "push" | "map" | "reduce" | "set" | "add");
                // The array cannot change while a function goes through it.
                let lambdas = args
                    .iter()
                    .any(|&a| matches!(self.checker.ast.expr(a).kind, ExprKind::Lambda { .. }));
                let root = self.root(*base).filter(|_| lambdas);
                if let Some(root) = root {
                    let span = self.checker.ast.expr(*base).span;
                    self.iterated.push((root, span));
                }
                for &arg in args {
                    let mode = if consume { Use::Consume } else { Use::Read };
                    self.expr(arg, mode, path);
                }
                if root.is_some() {
                    self.iterated.pop();
                }
            }
            _ => {
                for &arg in args {
                    self.expr(arg, Use::Read, path);
                }
            }
        }
    }

    /// The arguments of a call to a function of the module.
    fn arguments(
        &mut self,
        callee: &str,
        args: &[ExprId],
        conventions: &[Convention],
        call: Span,
        path: &mut Path,
        receiver: bool,
    ) {
        // The variable each argument reaches, and whether it is passed `inout`.
        let mut roots: Vec<(usize, Vec<String>, Span, bool)> = Vec::new();
        for (i, &arg) in args.iter().enumerate() {
            let convention = conventions.get(i).copied().unwrap_or(Convention::Let);
            let (inner, marked) = self.unmark(arg);
            let span = self.checker.ast.expr(arg).span;
            // The receiver of a method is never marked: `p.move(1.0)`.
            let marked = if receiver && i == 0 {
                convention == Convention::Inout
            } else {
                marked
            };
            match (convention, marked) {
                (Convention::Inout, false) => self.report(errors::inout_marker(span, true)),
                (Convention::Let | Convention::Sink, true) => {
                    self.report(errors::inout_marker(span, false));
                }
                _ => {}
            }
            if convention == Convention::Inout {
                self.check_inout_place(inner);
                if let Some(root) = self.root(inner) {
                    let span = self.checker.ast.expr(inner).span;
                    self.not_iterated(root, span);
                    let place = self.place_path(inner).map(|(_, p)| p).unwrap_or_default();
                    if let Some(uses) = self.uses.last_mut() {
                        uses.inouts.push((root, place, call, callee.to_owned()));
                    }
                }
            }
            let mode = if convention == Convention::Sink {
                Use::Consume
            } else {
                Use::Read
            };
            self.expr(inner, mode, path);
            // A number or a boolean is copied: it cannot alias.
            let shared = convention == Convention::Inout || self.moves(inner);
            if shared && let Some((root, place)) = self.place_path(inner) {
                let span = self.checker.ast.expr(inner).span;
                roots.push((root, place, span, convention == Convention::Inout));
            }
        }
        // Each argument that reaches the place of an earlier one, when either is
        // `inout`, is reported once. Different fields of one structure do not
        // overlap.
        for (j, (root, place, span, inout)) in roots.iter().enumerate() {
            let (root, span, inout) = (*root, *span, *inout);
            let earlier = roots.iter().take(j).find(|(r, other, _, other_inout)| {
                *r == root && (inout || *other_inout) && overlap(place, other)
            });
            if let Some(&(_, _, other_span, other_inout)) = earlier {
                let name = self.binding_name(root);
                // A copy of the argument that is only read removes the alias.
                let copy_at = if !self.copyable(root) {
                    None
                } else if !inout {
                    Some(span.end)
                } else if !other_inout {
                    Some(other_span.end)
                } else {
                    None
                };
                self.report(errors::alias(span, &name, callee, other_span, copy_at));
            }
        }
    }

    /// An `inout` argument must be a variable or a part of one that can change.
    fn check_inout_place(&mut self, arg: ExprId) {
        let expr = self.checker.ast.expr(arg);
        let span = expr.span;
        match &expr.kind {
            ExprKind::Name(_) | ExprKind::Field { .. } | ExprKind::Index { .. } => {
                if let Some(root) = self.root(arg) {
                    self.checker.check_mutable(root, span);
                }
            }
            ExprKind::Paren(inner) => self.check_inout_place(*inner),
            _ => {}
        }
    }

    /// The argument without its `&`, and whether it had one.
    fn unmark(&self, arg: ExprId) -> (ExprId, bool) {
        match self.checker.ast.expr(arg).kind {
            ExprKind::Unary {
                op: UnaryOp::Borrow,
                operand,
            } => (operand, true),
            ExprKind::Paren(inner) => self.unmark(inner),
            _ => (arg, false),
        }
    }

    /// A field or an element read: recorded whole, `b.samples`, so that it
    /// does not conflict with `&b.peak` the way the whole of `b` would.
    fn record_place(&mut self, id: ExprId, span: Span) {
        let (ExprKind::Field { base, .. } | ExprKind::Index { base, .. }) =
            self.checker.ast.expr(id).kind
        else {
            return;
        };
        let outer = self.uses.last().is_some_and(|u| !u.inside.contains(&id));
        let found = if outer { self.place_path(id) } else { None };
        let Some(uses) = self.uses.last_mut() else {
            return;
        };
        if let Some((root, place)) = found {
            uses.reads.push((root, place, span));
        }
        // The base is part of this place, unless it was not recorded.
        if outer || uses.inside.contains(&id) {
            let mut base = base;
            loop {
                uses.inside.insert(base);
                match &self.checker.ast.expr(base).kind {
                    ExprKind::Field { base: next, .. } | ExprKind::Index { base: next, .. } => {
                        base = *next;
                    }
                    ExprKind::Paren(inner) => base = *inner,
                    _ => break,
                }
            }
        }
    }

    /// A place as its variable and the fields on the way; an element counts as
    /// all of its array.
    fn place_path(&self, id: ExprId) -> Option<(usize, Vec<String>)> {
        match &self.checker.ast.expr(id).kind {
            ExprKind::Name(_) => self.checker.resolved.get(&id).map(|&b| (b, Vec::new())),
            ExprKind::Paren(inner) => self.place_path(*inner),
            ExprKind::Field { base, name } => {
                let (root, mut path) = self.place_path(*base)?;
                path.push(self.checker.name(name.symbol).to_owned());
                Some((root, path))
            }
            ExprKind::Index { base, .. } => self.place_path(*base),
            _ => None,
        }
    }

    /// The variable an access starts from: `buf` in `buf.samples[0]`.
    fn root(&self, mut id: ExprId) -> Option<usize> {
        loop {
            match &self.checker.ast.expr(id).kind {
                ExprKind::Name(_) => return self.checker.resolved.get(&id).copied(),
                ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => id = *base,
                ExprKind::Paren(inner) => id = *inner,
                _ => return None,
            }
        }
    }

    // --- Helpers -------------------------------------------------------------

    /// Whether the value of an expression moves when it is consumed.
    fn moves(&self, id: ExprId) -> bool {
        self.checker
            .expr_types
            .get(&id)
            .is_some_and(|ty| self.checker.is_move_type(ty))
    }

    fn cannot_move(&mut self, id: ExprId, span: Span, what: &'static str) {
        let copyable = matches!(
            self.checker.expr_types.get(&id),
            Some(Ty::Array(_) | Ty::String)
        );
        self.report(errors::move_out(span, what, copyable));
    }

    fn binding_name(&self, binding: usize) -> String {
        self.checker
            .scopes
            .bindings
            .get(binding)
            .map(|b| b.name.clone())
            .unwrap_or_default()
    }

    /// Whether `.copy()` gives a copy of the binding's value.
    fn copyable(&self, binding: usize) -> bool {
        self.checker
            .scopes
            .bindings
            .get(binding)
            .is_some_and(|b| matches!(b.ty, Ty::Array(_) | Ty::String))
    }
}

/// Where two paths meet: what either moved is moved.
fn merge(path: &mut Path, other: &Path) {
    match (path.gone, other.gone) {
        (true, false) => *path = other.clone(),
        (false, false) => path.moved.extend(other.moved.iter().map(|(k, v)| (*k, *v))),
        // A path that left does not reach what follows.
        (_, true) => {}
    }
}

impl Checker<'_> {
    /// Whether values of a type live on the heap, so that they move.
    pub(crate) fn is_move_type(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Array(_) | Ty::String | Ty::Map(..) | Ty::Set(_) => true,
            // Any type, unless its constraints say it is copied.
            Ty::Param(i, _) => self
                .param_of(*i)
                .is_none_or(|p| !p.meets(crate::ty::Bound::Copy)),
            Ty::Option(inner) => self.is_move_type(inner),
            Ty::Result(ok, err) => self.is_move_type(ok) || self.is_move_type(err),
            Ty::Struct(id, _) => self
                .structs
                .get(id.0)
                .is_some_and(|s| s.fields.iter().any(|(_, f)| self.is_move_type(f))),
            _ => false,
        }
    }
}
