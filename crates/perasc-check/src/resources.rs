//! Resources in a managed module ([decision 0009]): a `Listener`, a `Timer`
//! and a `Group` belong to one variable, and are released when it goes out
//! of scope. This pass checks that each has exactly one owner, and records
//! where one is given away, so that the generator releases only the others.
//!
//! A resource is given away when it is returned, added to a group, released
//! with `release()`, or moved into another variable. After that, its
//! variable holds nothing. A resource kept by nothing, kept by a structure,
//! an array, an option, a map or an arrow function, given away in a loop, or
//! given away by a function that only borrowed it is refused (`E0245`).
//!
//! [decision 0009]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0009-browser-and-resources.md

use std::collections::{HashMap, HashSet};

use perasc_diag::Span;
use perasc_syntax::ast::{ArmBody, ExprId, ExprKind, FnDecl, ItemKind, Stmt, StmtKind};

use crate::browser::{holds_resource, is_resource};
use crate::ty::Ty;
use crate::{Checker, errors};

/// What a variable holding a resource holds now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    /// Its resource.
    Live,
    /// Nothing: given away on every path.
    Given,
    /// Nothing on some paths.
    Maybe,
}

/// How an expression is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    /// Its value is read, or lent.
    Use,
    /// Its value is given to a new owner: returned, stored in a variable,
    /// added to a group, released.
    Owner,
}

pub struct Resources<'c, 'a> {
    checker: &'c mut Checker<'a>,
    /// The binding declared at each span.
    declared: HashMap<(u32, u32), usize>,
    /// Each variable holding a resource, in the function being checked:
    /// what it holds, and the depth of loops it is declared at.
    held: HashMap<usize, (Held, usize)>,
    /// Parameters holding a resource: lent, never given away.
    lent: HashSet<usize>,
    /// Variables already reported, so that one mistake gives one error.
    reported: HashSet<usize>,
    /// The depth of loops around the statement being checked.
    loops: usize,
    /// Inside an arrow function.
    in_arrow: bool,
    /// Inside a value already reported for holding a resource.
    in_holder: bool,
    /// Module variables holding a resource, already reported.
    globals: HashSet<usize>,
}

impl<'c, 'a> Resources<'c, 'a> {
    pub fn new(checker: &'c mut Checker<'a>) -> Self {
        let declared = checker
            .scopes
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.span.map(|s| ((s.start, s.end), i)))
            .collect();
        Self {
            checker,
            declared,
            held: HashMap::new(),
            lent: HashSet::new(),
            reported: HashSet::new(),
            loops: 0,
            in_arrow: false,
            in_holder: false,
            globals: HashSet::new(),
        }
    }

    pub fn module(mut self) {
        let ast = self.checker.ast;
        // A structure cannot hold a resource: nothing would release it.
        for item in &ast.items {
            if let ItemKind::Struct(decl) = &item.kind {
                for field in &decl.fields {
                    let key = (field.ty.span.start, field.ty.span.end);
                    if self.checker.written.get(&key).is_some_and(holds_resource) {
                        let name = ast.name(field.name.symbol).to_owned();
                        self.checker
                            .report(errors::resource(field.ty.span, "holder", &name));
                    }
                }
            }
        }
        // A module variable lives as long as the page: nothing would release
        // the resource it holds.
        for item in &ast.items {
            if let ItemKind::Global(local) = &item.kind {
                let key = (local.name.span.start, local.name.span.end);
                if let Some(&b) = self.declared.get(&key)
                    && self.binding_ty(b).is_some_and(|t| is_resource(&t))
                {
                    self.globals.insert(b);
                    let name = self.binding_name(b);
                    self.report(local.name.span, "global", &name);
                }
            }
        }
        for decl in ast.functions() {
            self.function(decl);
        }
    }

    fn function(&mut self, decl: &FnDecl) {
        self.held.clear();
        self.lent.clear();
        self.loops = 0;
        for param in &decl.params {
            if let Some(&b) = self
                .declared
                .get(&(param.name.span.start, param.name.span.end))
                && self.binding_ty(b).is_some_and(|t| is_resource(&t))
            {
                self.lent.insert(b);
            }
        }
        for stmt in &decl.body.stmts {
            self.stmt(stmt);
        }
    }

    fn binding_ty(&self, binding: usize) -> Option<Ty> {
        self.checker
            .scopes
            .bindings
            .get(binding)
            .map(|b| b.ty.clone())
    }

    fn ty(&self, id: ExprId) -> Ty {
        self.checker
            .expr_types
            .get(&id)
            .cloned()
            .unwrap_or(Ty::Error)
    }

    fn report(&mut self, span: Span, reason: &str, name: &str) {
        self.checker.report(errors::resource(span, reason, name));
    }

    // --- Statements -----------------------------------------------------------

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Local(local) => {
                let Some(init) = local.init else {
                    return;
                };
                let key = (local.name.span.start, local.name.span.end);
                let binding = self.declared.get(&key).copied();
                let resource = binding
                    .and_then(|b| self.binding_ty(b))
                    .is_some_and(|t| is_resource(&t));
                let position = if resource {
                    Position::Owner
                } else {
                    Position::Use
                };
                self.expr(init, position);
                if let (true, Some(b)) = (resource, binding) {
                    self.held.insert(b, (Held::Live, self.loops));
                }
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(*value, Position::Owner);
                }
            }
            StmtKind::Expr(id) => {
                if is_resource(&self.ty(*id)) && !self.is_variable(*id) {
                    let span = self.checker.ast.expr(*id).span;
                    self.report(span, "dropped", "");
                    self.children(*id);
                } else {
                    self.expr(*id, Position::Use);
                }
            }
            StmtKind::Assign { target, value, .. } => {
                let mut position = Position::Use;
                // A module variable's resource is reported where it is declared.
                if let Some(&b) = self.checker.resolved.get(target)
                    && self.globals.contains(&b)
                {
                    position = Position::Owner;
                } else if let Some(&b) = self.checker.resolved.get(target)
                    && self.held.contains_key(&b)
                {
                    let span = self.checker.ast.expr(*target).span;
                    let name = self.binding_name(b);
                    self.report(span, "reassign", &name);
                    // The new resource is reported once, with the variable.
                    position = Position::Owner;
                }
                self.expr(*target, Position::Use);
                self.expr(*value, position);
            }
            StmtKind::Step { target, .. } => self.expr(*target, Position::Use),
            StmtKind::Block(block) => {
                for s in &block.stmts {
                    self.stmt(s);
                }
            }
            StmtKind::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(*cond, Position::Use);
                let before = self.held.clone();
                self.stmt(then);
                let after_then = std::mem::replace(&mut self.held, before);
                if let Some(otherwise) = otherwise {
                    self.stmt(otherwise);
                }
                self.join(&after_then);
            }
            StmtKind::While { cond, body } => {
                self.expr(*cond, Position::Use);
                self.in_loop(body);
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
                ..
            } => {
                if let Some(init) = init {
                    self.stmt(init);
                }
                if let Some(cond) = cond {
                    self.expr(*cond, Position::Use);
                }
                self.in_loop(body);
                if let Some(update) = update {
                    self.stmt(update);
                }
            }
            StmtKind::ForOf { iterable, body, .. } => {
                self.expr(*iterable, Position::Use);
                self.in_loop(body);
            }
            StmtKind::Break | StmtKind::Continue | StmtKind::Error => {}
        }
    }

    /// A loop's body: run once, what it may give away given away maybe.
    fn in_loop(&mut self, body: &Stmt) {
        let before = self.held.clone();
        self.loops += 1;
        self.stmt(body);
        self.loops -= 1;
        self.join(&before);
    }

    /// After two paths: a resource given away on one is given away maybe.
    fn join(&mut self, other: &HashMap<usize, (Held, usize)>) {
        for (binding, (held, depth)) in other {
            let mine = self.held.get(binding).map(|(h, _)| *h);
            let joined = match (mine, *held) {
                (Some(a), b) if a == b => a,
                (None, b) => b,
                _ => Held::Maybe,
            };
            self.held.insert(*binding, (joined, *depth));
        }
    }

    fn binding_name(&self, binding: usize) -> String {
        self.checker
            .scopes
            .bindings
            .get(binding)
            .map(|b| b.name.clone())
            .unwrap_or_default()
    }

    /// Whether an expression is a variable, in parentheses or not.
    fn is_variable(&self, id: ExprId) -> bool {
        match &self.checker.ast.expr(id).kind {
            ExprKind::Name(_) => true,
            ExprKind::Paren(inner) => self.is_variable(*inner),
            _ => false,
        }
    }

    // --- Expressions ----------------------------------------------------------

    fn expr(&mut self, id: ExprId, position: Position) {
        let ast = self.checker.ast;
        let expr = ast.expr(id);
        let ty = self.ty(id);
        // A resource inside another value has no owner to release it. A
        // promise of one is given by an `async` function, and `await` takes it.
        // Reported where the value is made, once: its parts are not reported.
        let holder = holds_resource(&ty)
            && !is_resource(&ty)
            && !matches!(ty, Ty::Promise(_) | Ty::Error)
            && !matches!(expr.kind, ExprKind::Name(_) | ExprKind::Paren(_));
        if holder && !self.in_holder {
            self.report(expr.span, "holder", "");
            let outer = std::mem::replace(&mut self.in_holder, true);
            self.children(id);
            self.in_holder = outer;
            return;
        }
        match &expr.kind {
            ExprKind::Name(_) => self.name(id, position),
            ExprKind::Paren(inner) => self.expr(*inner, position),
            ExprKind::Lambda { body, .. } => {
                let outer = std::mem::replace(&mut self.in_arrow, true);
                self.expr(*body, Position::Use);
                self.in_arrow = outer;
            }
            ExprKind::Call { callee, args, .. } => {
                if let ExprKind::Field { base, name } = &ast.expr(*callee).kind {
                    let method = ast.name(name.symbol);
                    let receiver = self.ty(*base);
                    // `r.release()` gives `r` away; `group.add(r)` gives `r` to the group.
                    if method == "release" && is_resource(&receiver) {
                        self.expr(*base, Position::Owner);
                        return;
                    }
                    if method == "add" && matches!(receiver, Ty::Host(crate::browser::Host::Group))
                    {
                        self.expr(*base, Position::Use);
                        for &arg in args {
                            self.expr(arg, Position::Owner);
                        }
                        return;
                    }
                    self.expr(*base, Position::Use);
                } else {
                    self.expr(*callee, Position::Use);
                }
                for &arg in args {
                    self.expr(arg, Position::Use);
                }
                self.made(id, position);
            }
            ExprKind::Unary { operand, .. } => {
                // `await f()`: the resource the promise gives.
                self.expr(*operand, Position::Use);
                self.made(id, position);
            }
            _ => {
                self.children(id);
                self.made(id, position);
            }
        }
    }

    /// A resource made by an expression must go to an owner.
    fn made(&mut self, id: ExprId, position: Position) {
        if position == Position::Use && !self.in_holder && is_resource(&self.ty(id)) {
            let span = self.checker.ast.expr(id).span;
            self.report(span, "dropped", "");
        }
    }

    /// The sub-expressions of an expression, each used.
    fn children(&mut self, id: ExprId) {
        let ast = self.checker.ast;
        let mut subs: Vec<ExprId> = Vec::new();
        match &ast.expr(id).kind {
            ExprKind::Array(items) | ExprKind::Tuple(items) => subs.extend(items),
            ExprKind::Unary { operand, .. } => subs.push(*operand),
            ExprKind::Binary { lhs, rhs, .. } => subs.extend([*lhs, *rhs]),
            ExprKind::Call { callee, args, .. } => {
                subs.push(*callee);
                subs.extend(args);
            }
            ExprKind::Template { exprs, .. } => subs.extend(exprs),
            ExprKind::Field { base, .. } => subs.push(*base),
            ExprKind::Index { base, index } => subs.extend([*base, *index]),
            ExprKind::Try(inner) | ExprKind::Paren(inner) => subs.push(*inner),
            ExprKind::Lambda { body, .. } => subs.push(*body),
            ExprKind::Match { scrutinee, arms } => {
                subs.push(*scrutinee);
                for arm in arms {
                    match &arm.body {
                        ArmBody::Expr(e) => subs.push(*e),
                        ArmBody::Block(block) => {
                            for s in &block.stmts {
                                self.stmt(s);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        for sub in subs {
            self.expr(sub, Position::Use);
        }
    }

    /// A variable: read, lent, or given away.
    fn name(&mut self, id: ExprId, position: Position) {
        let Some(&binding) = self.checker.resolved.get(&id) else {
            return;
        };
        let span = self.checker.ast.expr(id).span;
        let lent = self.lent.contains(&binding);
        let Some(&(held, depth)) = self.held.get(&binding) else {
            // A parameter: lent by the caller.
            if lent {
                if self.in_arrow {
                    let name = self.binding_name(binding);
                    self.report(span, "capture", &name);
                } else if position == Position::Owner {
                    let name = self.binding_name(binding);
                    self.report(span, "lent", &name);
                }
            }
            return;
        };
        let name = self.binding_name(binding);
        if self.in_arrow {
            self.report(span, "capture", &name);
            return;
        }
        if held != Held::Live {
            if self.reported.insert(binding) {
                self.report(span, "moved", &name);
            }
            return;
        }
        if position == Position::Owner {
            if depth < self.loops {
                self.report(span, "loop", &name);
            }
            self.held.insert(binding, (Held::Given, depth));
            self.checker.moves.insert(id);
        }
    }
}
