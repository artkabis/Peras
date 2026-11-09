//! Statements: declarations, assignments, control flow and returns.

use perasc_diag::Span;
use perasc_syntax::ast::{AssignOp, Convention, ExprId, ExprKind, Stmt, StmtKind};

use crate::errors::{self, ConditionFix, Immutable};
use crate::scope::Kind;
use crate::ty::{Bound, Ty};
use crate::{Checker, builtins};

impl Checker<'_> {
    /// The statements of a block, in a scope of their own.
    pub(crate) fn block(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    /// A statement used as a body: in a scope of its own, even without braces.
    fn scoped(&mut self, stmt: &Stmt) {
        self.scopes.push();
        self.stmt(stmt);
        self.pop_scope();
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Local(local) => {
                let ty = self.local_type(local);
                let kind = Kind::Local {
                    mutable: local.mutable,
                    keyword: local.keyword,
                };
                self.declare(local.name.symbol, Some(local.name.span), kind, ty);
            }
            StmtKind::Return(value) => self.return_stmt(stmt.span, *value),
            StmtKind::Expr(e) => {
                self.infer(*e, None);
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
            } => {
                self.condition(*cond);
                self.scoped(then);
                if let Some(otherwise) = otherwise {
                    self.scoped(otherwise);
                }
            }
            StmtKind::While { cond, body } => {
                self.condition(*cond);
                self.scoped(body);
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
            } => {
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

    /// The element type of what a `for…of` walks.
    fn iterable(&mut self, iterable: ExprId) -> Ty {
        let ty = self.infer(iterable, None);
        match ty {
            Ty::Array(element) => *element,
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
                self.check_mutable(index, expr.span);
                self.scopes
                    .bindings
                    .get(index)
                    .map_or(Ty::Error, |b| b.ty.clone())
            }
            ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => {
                if let Some(root) = self.root(*base) {
                    let span = self.ast.expr(*base).span;
                    self.check_mutable(root, span);
                }
                self.infer(target, None)
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

    /// `E0205` when the binding cannot be modified.
    pub(crate) fn check_mutable(&mut self, index: usize, span: Span) {
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
