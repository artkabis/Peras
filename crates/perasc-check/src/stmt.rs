//! Statements: declarations, assignments, control flow and returns.

use perasc_diag::Span;
use perasc_syntax::ast::{
    AssignOp, BinaryOp, Convention, ExprId, ExprKind, Stmt, StmtKind, Symbol, UnaryOp,
};

use crate::errors::{self, ConditionFix, Immutable};
use crate::scope::Kind;
use crate::ty::{Bound, Ty};
use crate::{Checker, builtins};

impl Checker<'_> {
    /// The statements of a block, in a scope of their own.
    pub(crate) fn block(&mut self, stmts: &[Stmt]) {
        // Decision 0028's option 2e: after `if (x.isNone())`
        // with a block that leaves, `x` has the type of its value, until
        // the end of this block.
        let mut narrowed: Vec<Undo> = Vec::new();
        for stmt in stmts {
            self.stmt(stmt);
            let after = self.narrowing(stmt);
            narrowed.extend(self.narrow_all(after));
        }
        for undo in narrowed.into_iter().rev() {
            self.undo(undo);
        }
    }

    /// Narrows each of them, in order: what gives them back, in order.
    pub(crate) fn narrow_all(&mut self, narrows: Vec<Narrow>) -> Vec<Undo> {
        narrows.into_iter().filter_map(|n| self.narrow(n)).collect()
    }

    /// Gives back what [`Self::narrow_all`] changed.
    pub(crate) fn undo_all(&mut self, undos: Vec<Undo>) {
        for undo in undos.into_iter().rev() {
            self.undo(undo);
        }
    }

    /// Gives a name or a field the type of its option's value.
    fn narrow(&mut self, narrow: Narrow) -> Option<Undo> {
        match narrow {
            Narrow::Name(index, inner) => {
                let old = std::mem::replace(&mut self.scopes.bindings.get_mut(index)?.ty, inner);
                let before = self.narrowed_now.insert(index, old.clone());
                Some(Undo::Name(index, old, before))
            }
            Narrow::Field(index, path, inner) => {
                self.narrowed_fields.push((index, path, inner));
                Some(Undo::Field(self.narrowed_fields.len() - 1))
            }
        }
    }

    /// Gives back what [`Self::narrow`] changed.
    fn undo(&mut self, undo: Undo) {
        match undo {
            Undo::Name(index, old, before) => {
                if let Some(binding) = self.scopes.bindings.get_mut(index) {
                    binding.ty = old;
                }
                match before {
                    Some(ty) => self.narrowed_now.insert(index, ty),
                    None => self.narrowed_now.remove(&index),
                };
            }
            Undo::Field(at) => self.narrowed_fields.truncate(at),
        }
    }

    /// What an `if` whose block leaves narrows after it, and the type of
    /// each value: a `const` or a parameter not `inout`, or a field of one
    /// (decision 0028, option 2e). Under decision 0045's n1, each option of
    /// a chain of `||` tested `isNone()`; under n2, the facts of its
    /// condition when false, or when true after an `else` that leaves.
    fn narrowing(&self, stmt: &Stmt) -> Vec<Narrow> {
        if crate::proto("PERAS_PROTO_TAKE") != "e" {
            return Vec::new();
        }
        let StmtKind::If {
            cond,
            then,
            otherwise,
        } = &stmt.kind
        else {
            return Vec::new();
        };
        let leaves = |s: &Stmt| crate::never_ends(self.ast, std::slice::from_ref(s));
        match otherwise {
            None if leaves(then) => self.facts(*cond).when_false,
            Some(otherwise) if crate::narrow_proto() == "n2" => {
                if leaves(then) {
                    self.facts(*cond).when_false
                } else if leaves(otherwise) {
                    self.facts(*cond).when_true
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /// What a condition proves, under the option of decision 0045 a build
    /// follows: under n0, `x.isSome()` its value when true, `x.isNone()`
    /// when false; under n1, each such test of a chain of `&&` when true,
    /// of `||` when false; under n2, through `!`, `&&` and `||`.
    pub(crate) fn facts(&self, cond: ExprId) -> Facts {
        match crate::narrow_proto() {
            "n2" => self.facts_n2(cond),
            "n1" => Facts {
                when_true: self.chain(cond, BinaryOp::And, "isSome"),
                when_false: self.chain(cond, BinaryOp::Or, "isNone"),
            },
            _ => Facts {
                when_true: self.option_test(cond, "isSome").into_iter().collect(),
                when_false: self.option_test(cond, "isNone").into_iter().collect(),
            },
        }
    }

    /// What the left operand of `&&` (`true`) or `||` (`false`) proves to
    /// the right one, under decision 0045's prototypes; nothing under n0.
    pub(crate) fn operand_facts(&self, lhs: ExprId, and: bool) -> Vec<Narrow> {
        if crate::narrow_proto() == "n0" {
            return Vec::new();
        }
        let facts = self.facts(lhs);
        if and {
            facts.when_true
        } else {
            facts.when_false
        }
    }

    /// Decision 0045's n1: the tests `x.<test>()` of a chain of `op`.
    fn chain(&self, cond: ExprId, op: BinaryOp, test: &str) -> Vec<Narrow> {
        match &self.ast.expr(cond).kind {
            ExprKind::Paren(inner) => self.chain(*inner, op, test),
            ExprKind::Binary { op: o, lhs, rhs } if *o == op => {
                let mut all = self.chain(*lhs, op, test);
                all.extend(self.chain(*rhs, op, test));
                all
            }
            _ => self.option_test(cond, test).into_iter().collect(),
        }
    }

    /// Decision 0045's n2: the facts of a condition, through `!`, `&&` and
    /// `||`, as TypeScript keeps them.
    fn facts_n2(&self, cond: ExprId) -> Facts {
        match &self.ast.expr(cond).kind {
            ExprKind::Paren(inner) => self.facts_n2(*inner),
            ExprKind::Unary {
                op: UnaryOp::Not,
                operand,
            } => {
                let inner = self.facts_n2(*operand);
                Facts {
                    when_true: inner.when_false,
                    when_false: inner.when_true,
                }
            }
            ExprKind::Binary {
                op: op @ (BinaryOp::And | BinaryOp::Or),
                lhs,
                rhs,
            } => {
                let (a, b) = (self.facts_n2(*lhs), self.facts_n2(*rhs));
                if *op == BinaryOp::And {
                    Facts {
                        when_true: union(a.when_true, b.when_true),
                        when_false: common(a.when_false, &b.when_false),
                    }
                } else {
                    Facts {
                        when_true: common(a.when_true, &b.when_true),
                        when_false: union(a.when_false, b.when_false),
                    }
                }
            }
            _ => Facts {
                when_true: self.option_test(cond, "isSome").into_iter().collect(),
                when_false: self.option_test(cond, "isNone").into_iter().collect(),
            },
        }
    }

    /// `x.isSome()` or `x.isNone()`, `x` an option that cannot change: its
    /// binding and the type of its value (finding F115).
    fn option_test(&self, cond: ExprId, test: &str) -> Option<Narrow> {
        if crate::proto("PERAS_PROTO_TAKE") != "e" {
            return None;
        }
        let ExprKind::Call { callee, args, .. } = &self.ast.expr(cond).kind else {
            return None;
        };
        let ExprKind::Field { base, name } = &self.ast.expr(*callee).kind else {
            return None;
        };
        if !args.is_empty() || self.name(name.symbol) != test {
            return None;
        }
        self.fixed_option(*base)
    }

    /// An option no one can change: a `const` or a parameter not `inout`,
    /// or a field of one, `child.parent` (finding F115); what it narrows to.
    fn fixed_option(&self, base: ExprId) -> Option<Narrow> {
        if let Some((index, path)) = self.field_path(base) {
            return match self.expr_types.get(&base) {
                Some(Ty::Option(inner)) => Some(Narrow::Field(index, path, (**inner).clone())),
                _ => None,
            };
        }
        let index = *self.resolved.get(&base)?;
        let binding = self.scopes.bindings.get(index)?;
        match &binding.ty {
            Ty::Option(inner) if self.is_fixed(index) => {
                Some(Narrow::Name(index, (**inner).clone()))
            }
            _ => None,
        }
    }

    /// Whether a binding cannot change: a `const`, or a parameter not
    /// `inout`.
    fn is_fixed(&self, index: usize) -> bool {
        self.scopes
            .bindings
            .get(index)
            .is_some_and(|binding| match binding.kind {
                Kind::Local { mutable, .. } => !mutable,
                Kind::Param { convention, .. } => convention != Convention::Inout,
                _ => false,
            })
    }

    /// `a.b.c`, fields down to a binding that cannot change: the binding
    /// and the fields' names, from it.
    pub(crate) fn field_path(&self, id: ExprId) -> Option<(usize, Vec<Symbol>)> {
        let mut path = Vec::new();
        let mut at = id;
        loop {
            match &self.ast.expr(at).kind {
                ExprKind::Field { base, name } => {
                    path.push(name.symbol);
                    at = *base;
                }
                ExprKind::Paren(inner) => at = *inner,
                ExprKind::Name(_) => break,
                _ => return None,
            }
        }
        let index = *self.resolved.get(&at)?;
        if path.is_empty() || !self.is_fixed(index) {
            return None;
        }
        path.reverse();
        Some((index, path))
    }

    /// The type of the value of a field narrowed here, `child.parent` after
    /// `if (child.parent.isSome())`.
    pub(crate) fn narrowed_field(&self, id: ExprId) -> Option<Ty> {
        let (index, path) = self.field_path(id)?;
        self.narrowed_fields
            .iter()
            .rev()
            .find(|(i, p, _)| *i == index && *p == path)
            .map(|(_, _, inner)| inner.clone())
    }

    /// A statement used as a body: in a scope of its own, even without braces.
    fn scoped(&mut self, stmt: &Stmt) {
        self.scopes.push();
        self.stmt(stmt);
        self.pop_scope();
    }

    /// A declaration, and under decision 0028's option 2c, prototyped, its
    /// `else` block.
    fn local(&mut self, local: &perasc_syntax::ast::Local) {
        let mut ty = self.local_type(local);
        // Decision 0028's option 2c, prototyped: the value of `Some`
        // or `Ok`, or a block that leaves.
        if let Some(block) = &local.otherwise {
            ty = match ty {
                Ty::Option(inner) | Ty::Result(inner, _) => *inner,
                Ty::Error => Ty::Error,
                other => {
                    let span = local
                        .init
                        .map_or(local.name.span, |i| self.ast.expr(i).span);
                    let wanted = Ty::Option(Box::new(other.clone()));
                    self.report(errors::mismatch(span, &wanted, &other, None));
                    Ty::Error
                }
            };
            self.scopes.push();
            self.block(&block.stmts);
            self.pop_scope();
            if !crate::never_ends(self.ast, &block.stmts) {
                let end = block.span.end;
                let closing = perasc_diag::Span::new(end.saturating_sub(1), end);
                self.report(errors::missing_return(
                    closing,
                    "else",
                    &Ty::Void,
                    block.span,
                ));
            }
        }
        let kind = Kind::Local {
            mutable: local.mutable,
            keyword: local.keyword,
        };
        self.declare(local.name.symbol, Some(local.name.span), kind, ty);
        // A `const` an integer division initialised (decision 0034, d1).
        if !local.mutable
            && let Some(division) = local.init.and_then(|init| self.int_division(init))
            && let Some(index) = self.scopes.lookup(local.name.symbol)
        {
            self.int_divisions.insert(index, division);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Local(local) => self.local(local),
            StmtKind::Return(value) => self.return_stmt(stmt.span, *value),
            StmtKind::Expr(e) => {
                // A promise nobody awaits: the work it stands for runs later, and
                // whatever follows does not wait for it.
                if let Ty::Promise(_) = self.infer(*e, None) {
                    let span = self.ast.expr(*e).span;
                    self.report(errors::floating_promise(span));
                }
            }
            StmtKind::Assign { target, op, value } => self.assign(*target, *op, *value),
            StmtKind::Step { target, .. } => {
                let ty = self.place(*target);
                let numeric = ty.is_numeric() || self.satisfies_param(&ty, Bound::Numeric);
                if !numeric && ty != Ty::Error {
                    let span = self.ast.expr(*target).span;
                    self.invalid_operator(span, "++", &ty, Bound::Numeric);
                }
            }
            StmtKind::Block(block) => {
                self.scopes.push();
                self.block(&block.stmts);
                self.pop_scope();
            }
            StmtKind::If {
                cond,
                then,
                otherwise,
            } => self.if_stmt(*cond, then, otherwise.as_deref()),
            StmtKind::While { cond, body } => self.while_stmt(*cond, body),
            StmtKind::For {
                parallel,
                init,
                cond,
                update,
                body,
            } => {
                if *parallel && self.managed {
                    self.report(errors::owned_only(stmt.span, "parallel"));
                }
                self.scopes.push();
                if let Some(init) = init {
                    self.stmt(init);
                }
                if let Some(cond) = cond {
                    self.condition(*cond);
                }
                if let Some(update) = update {
                    self.stmt(update);
                }
                self.scoped(body);
                self.pop_scope();
            }
            StmtKind::ForOf {
                parallel: _,
                mutable,
                keyword,
                name,
                ty,
                iterable,
                body,
            } => {
                let element = self.iterable(*iterable);
                let element = match ty {
                    Some(annotation) => {
                        let declared = self.resolve_type(annotation);
                        self.compatible(annotation.span, &element, &declared);
                        declared
                    }
                    None => element,
                };
                self.scopes.push();
                let kind = Kind::Local {
                    mutable: *mutable,
                    keyword: *keyword,
                };
                self.declare(name.symbol, Some(name.span), kind, element);
                self.scoped(body);
                self.pop_scope();
            }
            StmtKind::Break | StmtKind::Continue | StmtKind::Error => {}
        }
    }

    /// An `if`: `if (x.isSome()) { … }`, `x` is its value in the block;
    /// under decision 0045's n2, `if (x.isNone()) … else { … }` in the
    /// `else`.
    fn if_stmt(&mut self, cond: ExprId, then: &Stmt, otherwise: Option<&Stmt>) {
        self.condition(cond);
        let facts = self.facts(cond);
        let narrowed = self.narrow_all(facts.when_true);
        self.scoped(then);
        self.undo_all(narrowed);
        if let Some(otherwise) = otherwise {
            let narrowed = if crate::narrow_proto() == "n2" {
                self.narrow_all(facts.when_false)
            } else {
                Vec::new()
            };
            self.scoped(otherwise);
            self.undo_all(narrowed);
        }
    }

    /// A `while`; under decision 0045's n2, its body with what its condition
    /// proves.
    fn while_stmt(&mut self, cond: ExprId, body: &Stmt) {
        self.condition(cond);
        let narrowed = if crate::narrow_proto() == "n2" {
            let facts = self.facts(cond);
            self.narrow_all(facts.when_true)
        } else {
            Vec::new()
        };
        self.scoped(body);
        self.undo_all(narrowed);
    }

    /// The element type of what a `for…of` walks: an array's elements, a
    /// set's keys, a map's entries as tuples.
    fn iterable(&mut self, iterable: ExprId) -> Ty {
        let ty = self.infer(iterable, None);
        match ty {
            Ty::Array(element) | Ty::Set(element) => *element,
            // A map's entries, in insertion order: `(key, value)`.
            Ty::Map(key, value) => self.tuple_type(vec![*key, *value]),
            Ty::Error => Ty::Error,
            other => {
                let span = self.ast.expr(iterable).span;
                self.report(errors::mismatch(
                    span,
                    &Ty::Array(Box::new(Ty::Error)),
                    &other,
                    None,
                ));
                Ty::Error
            }
        }
    }

    /// A `for…of` annotation must match the elements.
    fn compatible(&mut self, span: Span, element: &Ty, declared: &Ty) {
        if element != declared && !matches!(element, Ty::Error) && !matches!(declared, Ty::Error) {
            self.report(errors::mismatch(span, declared, element, None));
        }
    }

    fn return_stmt(&mut self, span: Span, value: Option<ExprId>) {
        match (self.ret.clone(), value) {
            // Unannotated: record what is returned, to infer the missing type.
            (None, Some(value)) => {
                let ty = self.infer(value, None);
                self.returned.push(ty);
            }
            (None, None) => {}
            (Some((ret, ret_span)), Some(value)) => {
                if ret == Ty::Void {
                    self.infer(value, None);
                    let at = self.ast.expr(value).span;
                    self.report(errors::return_mismatch(at, &ret, true, ret_span));
                } else {
                    self.expect(value, &ret);
                }
            }
            (Some((ret, ret_span)), None) => {
                if !matches!(ret, Ty::Void | Ty::Error) {
                    let keyword = Span::new(span.start, span.start + 6);
                    self.report(errors::return_mismatch(keyword, &ret, false, ret_span));
                }
            }
        }
    }

    /// A condition must be a `bool`: Peras has no truthiness.
    fn condition(&mut self, cond: ExprId) {
        let ty = self.infer(cond, Some(&Ty::Bool));
        if matches!(ty, Ty::Bool | Ty::Error) {
            return;
        }
        let expr = self.ast.expr(cond);
        let parenthesize = matches!(expr.kind, ExprKind::Binary { .. } | ExprKind::Unary { .. });
        let fix = if ty.is_numeric() {
            ConditionFix::Append(" != 0", parenthesize)
        } else if matches!(ty, Ty::Option(_)) {
            ConditionFix::Append(".isSome()", parenthesize)
        } else if ty == Ty::String {
            ConditionFix::Append(".length != 0", parenthesize)
        } else {
            ConditionFix::None
        };
        self.report(errors::condition_not_bool(expr.span, &ty, &fix));
    }

    fn assign(&mut self, target: ExprId, op: AssignOp, value: ExprId) {
        let ty = self.place(target);
        if op == AssignOp::Assign {
            self.expect(value, &ty);
            return;
        }
        // Compound: numbers, and `+=` on strings.
        let string_append = op == AssignOp::Add && ty == Ty::String;
        let numeric = ty.is_numeric() || self.satisfies_param(&ty, Bound::Numeric);
        if !numeric && !string_append && ty != Ty::Error {
            let span = self.ast.expr(target).span;
            let symbol = match op {
                AssignOp::Add => "+=",
                AssignOp::Sub => "-=",
                AssignOp::Mul => "*=",
                AssignOp::Div => "/=",
                AssignOp::Rem => "%=",
                AssignOp::Assign => "=",
            };
            self.invalid_operator(span, symbol, &ty, Bound::Numeric);
            self.infer(value, None);
            return;
        }
        self.expect(value, &ty);
    }

    /// The type of an assignment target, checking its root variable can be modified.
    /// Assigning a variable is not reading it: a variable only ever written is unused.
    fn place(&mut self, target: ExprId) -> Ty {
        let expr = self.ast.expr(target);
        match &expr.kind {
            ExprKind::Name(symbol) => {
                let Some(index) = self.scopes.lookup(*symbol) else {
                    return self.infer(target, None);
                };
                self.resolved.insert(target, index);
                self.capture(index, expr.span);
                self.check_mutable(index, expr.span);
                // A managed `inout` parameter shares its caller's value: it
                // can be changed, not replaced. One JavaScript copies is
                // already refused where it is declared.
                let inout = self.scopes.bindings.get(index).is_some_and(|b| {
                    matches!(
                        b.kind,
                        Kind::Param {
                            convention: Convention::Inout,
                            ..
                        }
                    ) && matches!(
                        b.ty,
                        Ty::Array(_) | Ty::Map(..) | Ty::Set(_) | Ty::Struct(..)
                    )
                });
                if self.managed && inout {
                    self.report(errors::owned_only(expr.span, "inout-assign"));
                }
                let ty = self
                    .scopes
                    .bindings
                    .get(index)
                    .map_or(Ty::Error, |b| b.ty.clone());
                // Kept for code generation, which computes `x += 1` by its type.
                self.expr_types.insert(target, ty.clone());
                ty
            }
            ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => {
                let ty = self.infer(target, None);
                // A property of the browser belongs to the page, not to the
                // variable: `const button = …; button.textContent = …` is fine.
                let host = matches!(self.expr_types.get(base), Some(Ty::Host(_)));
                // A member that does not exist is reported once, as such.
                if !host
                    && ty != Ty::Error
                    && let Some(root) = self.root(*base)
                {
                    let span = self.ast.expr(*base).span;
                    self.check_mutable(root, span);
                    self.mark_changed(*base);
                }
                // A property of the browser that can only be read.
                if let ExprKind::Field { base, name } = &expr.kind
                    && let Some(Ty::Host(h)) = self.expr_types.get(base).cloned()
                {
                    let text = self.name(name.symbol).to_owned();
                    self.host_field_written(h, &text, name.span);
                }
                ty
            }
            ExprKind::Paren(inner) => self.place(*inner),
            _ => self.infer(target, None),
        }
    }

    /// The binding a field or element access starts from: `buf` in `buf.samples[0]`.
    pub(crate) fn root(&self, mut expr: ExprId) -> Option<usize> {
        loop {
            match &self.ast.expr(expr).kind {
                ExprKind::Name(symbol) => return self.scopes.lookup(*symbol),
                ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => expr = *base,
                ExprKind::Paren(inner) => expr = *inner,
                _ => return None,
            }
        }
    }

    /// Marks every field and element from `expr` down to its binding as on
    /// the way to a place changed (see [`crate::Analysis::changed`]).
    pub(crate) fn mark_changed(&mut self, mut expr: ExprId) {
        loop {
            match &self.ast.expr(expr).kind {
                ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => {
                    self.changed.insert(expr);
                    expr = *base;
                }
                ExprKind::Paren(inner) => expr = *inner,
                _ => return,
            }
        }
    }

    /// `E0205` when the binding cannot be modified.
    pub(crate) fn check_mutable(&mut self, index: usize, span: Span) {
        self.mutations.push((index, span));
        let Some(binding) = self.scopes.bindings.get(index) else {
            return;
        };
        let why = match binding.kind {
            Kind::Local {
                mutable: false,
                keyword,
            }
            | Kind::Global {
                mutable: false,
                keyword,
            } => Immutable::Const(keyword),
            Kind::Param {
                convention: Convention::Let,
                type_start,
            } => Immutable::Parameter(type_start),
            _ => return,
        };
        let Some(declared) = binding.span else { return };
        let name = binding.name.clone();
        self.report(errors::assign_to_immutable(span, &name, declared, &why));
    }

    /// Member names of a type, for suggestions.
    pub(crate) fn member_names(&self, ty: &Ty) -> Vec<String> {
        match ty {
            Ty::Struct(id, _) => {
                let mut names: Vec<String> = self
                    .structs
                    .get(id.0)
                    .map(|s| s.fields.iter().map(|(n, _)| n.clone()).collect())
                    .unwrap_or_default();
                names.extend(self.method_names(ty));
                names
            }
            Ty::Enum(..) => self.method_names(ty),
            other => builtins::members(other)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

/// What a condition proves when it is true, and when it is false (decision
/// 0045).
pub(crate) struct Facts {
    pub(crate) when_true: Vec<Narrow>,
    pub(crate) when_false: Vec<Narrow>,
}

/// The narrowings of both, each once.
fn union(mut a: Vec<Narrow>, b: Vec<Narrow>) -> Vec<Narrow> {
    for n in b {
        if !a.iter().any(|m| m.same_place(&n)) {
            a.push(n);
        }
    }
    a
}

/// The narrowings of `a` that `b` has too.
fn common(a: Vec<Narrow>, b: &[Narrow]) -> Vec<Narrow> {
    a.into_iter()
        .filter(|n| b.iter().any(|m| m.same_place(n)))
        .collect()
}

impl Narrow {
    /// Whether two narrowings are of one binding or one field.
    fn same_place(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Name(a, _), Self::Name(b, _)) => a == b,
            (Self::Field(a, p, _), Self::Field(b, q, _)) => a == b && p == q,
            _ => false,
        }
    }
}

/// What a test narrows (decision 0028, finding F115): a binding, or a field
/// path from one, to the type of its option's value.
pub(crate) enum Narrow {
    Name(usize, Ty),
    Field(usize, Vec<Symbol>, Ty),
}

/// How to give back what a narrowing changed.
pub(crate) enum Undo {
    Name(usize, Ty, Option<Ty>),
    Field(usize),
}
