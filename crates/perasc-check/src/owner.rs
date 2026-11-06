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
    AssignOp, BinaryOp, Convention, ExprId, ExprKind, FnDecl, ItemKind, Stmt, StmtKind, UnaryOp,
};

use crate::scope::Kind;
use crate::ty::Ty;
use crate::{Checker, errors};

/// The variables moved on a path, with where each was moved.
type Moved = BTreeMap<usize, Span>;

/// A path through the code: what it moved, and whether it still goes on.
#[derive(Clone, Default)]
struct Path {
    moved: Moved,
    /// The path left by `return`, `break` or `continue`.
    gone: bool,
}

/// What reaches the exits of a loop.
#[derive(Default)]
struct Exits {
    breaks: Vec<Moved>,
    continues: Vec<Moved>,
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
}

impl<'c, 'a> Owner<'c, 'a> {
    pub(crate) fn new(checker: &'c mut Checker<'a>) -> Self {
        let conventions = checker
            .ast
            .items
            .iter()
            .filter_map(|item| match &item.kind {
                ItemKind::Fn(decl) => Some(decl.params.iter().map(|p| p.convention).collect()),
                _ => None,
            })
            .collect();
        Self {
            checker,
            conventions,
            loops: Vec::new(),
            silent: 0,
            reported: HashSet::new(),
            iterated: Vec::new(),
        }
    }

    pub(crate) fn module(&mut self) {
        let ast = self.checker.ast;
        for item in &ast.items {
            if let ItemKind::Fn(decl) = &item.kind {
                self.function(decl);
            }
        }
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
        match &stmt.kind {
            StmtKind::Local(local) => {
                if let Some(init) = local.init {
                    self.expr(init, Use::Consume, path);
                }
                // A new variable, also at each turn of a loop.
                if let Some(&binding) = self.checker.declared.get(&local.name.span.start) {
                    path.moved.remove(&binding);
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
                self.expr(*cond, Use::Read, path);
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
                self.expr(*iterable, Use::Read, path);
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
            self.expr(cond, Use::Read, &mut turn);
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
            self.expr(cond, Use::Read, path);
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
                }
            }
            ExprKind::Paren(inner) => self.target(*inner, op, path),
            _ => self.expr(target, Use::Read, path),
        }
    }

    // --- Expressions ---------------------------------------------------------

    fn expr(&mut self, id: ExprId, mode: Use, path: &mut Path) {
        let expr = self.checker.ast.expr(id);
        let span = expr.span;
        match &expr.kind {
            ExprKind::Name(_) => self.name(id, span, mode, path),
            ExprKind::Paren(inner) => self.expr(*inner, mode, path),
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
            ExprKind::Array(items) => {
                for &item in items {
                    self.expr(item, Use::Consume, path);
                }
            }
            ExprKind::Call { callee, args } => self.call(*callee, args, path),
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
        if let Some(&moved_at) = path.moved.get(&binding) {
            let name = self.binding_name(binding);
            let copyable = self.copyable(binding);
            self.report(errors::moved(span, &name, moved_at, copyable));
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
            Some(Kind::Global { .. }) => self.cannot_move(id, span, "global"),
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

    fn call(&mut self, callee: ExprId, args: &[ExprId], path: &mut Path) {
        let callee_expr = self.checker.ast.expr(callee);
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
                        self.arguments(&name, args, &conventions, path);
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
                if matches!(method, "push" | "pop")
                    && let Some(root) = self.root(*base)
                {
                    let span = self.checker.ast.expr(*base).span;
                    self.not_iterated(root, span);
                }
                let consume = method == "push";
                for &arg in args {
                    let mode = if consume { Use::Consume } else { Use::Read };
                    self.expr(arg, mode, path);
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
        path: &mut Path,
    ) {
        // The variable each argument reaches, and whether it is passed `inout`.
        let mut roots: Vec<(usize, Span, bool)> = Vec::new();
        for (i, &arg) in args.iter().enumerate() {
            let convention = conventions.get(i).copied().unwrap_or(Convention::Let);
            let (inner, marked) = self.unmark(arg);
            let span = self.checker.ast.expr(arg).span;
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
            if shared && let Some(root) = self.root(inner) {
                roots.push((
                    root,
                    self.checker.ast.expr(inner).span,
                    convention == Convention::Inout,
                ));
            }
        }
        // Each argument that reaches the variable of an earlier one, when
        // either is `inout`, is reported once.
        for (j, &(root, span, inout)) in roots.iter().enumerate() {
            let earlier = roots
                .iter()
                .take(j)
                .find(|&&(r, _, other_inout)| r == root && (inout || other_inout));
            if let Some(&(_, other_span, other_inout)) = earlier {
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
            Ty::Array(_) | Ty::String => true,
            Ty::Option(inner) => self.is_move_type(inner),
            Ty::Struct(id, _) => self
                .structs
                .get(id.0)
                .is_some_and(|s| s.fields.iter().any(|(_, f)| self.is_move_type(f))),
            _ => false,
        }
    }
}
