//! Bounds checks proven away, as for WebAssembly: in
//! `for (let i = start; i < xs.length; i++)` with `start` a literal at least
//! zero, whose body calls nothing, waits for nothing and changes neither `i`
//! nor `xs`, every `xs[i]` is in bounds. A call could change the length of
//! `xs` through another name, since a managed module shares its arrays: a
//! body with one keeps its checks.

use perasc_check::ty::Ty;
use perasc_syntax::ast::{ArmBody, ExprId, ExprKind, StepOp, Stmt, StmtKind, UnaryOp};

use crate::Module;

/// The variables a proof is about: the counter and the array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Proof {
    pub counter: usize,
    pub array: usize,
}

impl Module<'_> {
    /// The proof a counted loop gives, if it gives one.
    pub(crate) fn loop_proof(
        &self,
        init: Option<&Stmt>,
        cond: Option<ExprId>,
        update: Option<&Stmt>,
        body: &Stmt,
    ) -> Option<Proof> {
        // `let i = <literal ≥ 0>`.
        let StmtKind::Local(local) = &init?.kind else {
            return None;
        };
        if !matches!(self.ast.expr(local.init?).kind, ExprKind::Int(_)) {
            return None;
        }
        let counter = *self
            .declared
            .get(&(local.name.span.start, local.name.span.end))?;
        // `i < xs.length`.
        let ExprKind::Binary {
            op: perasc_syntax::ast::BinaryOp::Lt,
            lhs,
            rhs,
        } = &self.ast.expr(cond?).kind
        else {
            return None;
        };
        if self.analysis.names.get(lhs) != Some(&counter) {
            return None;
        }
        let ExprKind::Field { base, name } = &self.ast.expr(*rhs).kind else {
            return None;
        };
        if self.ast.name(name.symbol) != "length" || !matches!(self.ty(*base), Ty::Array(_)) {
            return None;
        }
        let ExprKind::Name(_) = self.ast.expr(*base).kind else {
            return None;
        };
        let array = *self.analysis.names.get(base)?;
        // `i++`.
        let StmtKind::Step {
            target,
            op: StepOp::Increment,
        } = &update?.kind
        else {
            return None;
        };
        if self.analysis.names.get(target) != Some(&counter) {
            return None;
        }
        let proof = Proof { counter, array };
        self.quiet_stmt(body, proof).then_some(proof)
    }

    /// Whether a statement leaves the counter and the array as they are,
    /// calling and waiting for nothing.
    fn quiet_stmt(&self, stmt: &Stmt, proof: Proof) -> bool {
        match &stmt.kind {
            StmtKind::Local(local) => local.init.is_none_or(|e| self.quiet(e, proof)),
            StmtKind::Return(value) => value.is_none_or(|e| self.quiet(e, proof)),
            StmtKind::Expr(e) => self.quiet(*e, proof),
            StmtKind::Assign { target, value, .. } => {
                self.writable(*target, proof) && self.quiet(*value, proof)
            }
            StmtKind::Step { target, .. } => self.writable(*target, proof),
            StmtKind::Block(block) => block.stmts.iter().all(|s| self.quiet_stmt(s, proof)),
            StmtKind::If {
                cond,
                then,
                otherwise,
            } => {
                self.quiet(*cond, proof)
                    && self.quiet_stmt(then, proof)
                    && otherwise
                        .as_deref()
                        .is_none_or(|s| self.quiet_stmt(s, proof))
            }
            StmtKind::While { cond, body } => {
                self.quiet(*cond, proof) && self.quiet_stmt(body, proof)
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
                ..
            } => {
                init.as_deref().is_none_or(|s| self.quiet_stmt(s, proof))
                    && cond.is_none_or(|e| self.quiet(e, proof))
                    && update.as_deref().is_none_or(|s| self.quiet_stmt(s, proof))
                    && self.quiet_stmt(body, proof)
            }
            StmtKind::ForOf { iterable, body, .. } => {
                self.quiet(*iterable, proof) && self.quiet_stmt(body, proof)
            }
            StmtKind::Break | StmtKind::Continue => true,
            StmtKind::Error => false,
        }
    }

    /// Whether a target may be written: neither the counter nor the array
    /// itself, and an element only of an array.
    fn writable(&self, target: ExprId, proof: Proof) -> bool {
        match &self.ast.expr(target).kind {
            ExprKind::Name(_) => {
                let bound = self.analysis.names.get(&target);
                bound != Some(&proof.counter) && bound != Some(&proof.array)
            }
            ExprKind::Index { base, index } => {
                self.quiet(*base, proof) && self.quiet(*index, proof)
            }
            ExprKind::Field { base, .. } => self.quiet(*base, proof),
            ExprKind::Paren(inner) => self.writable(*inner, proof),
            _ => false,
        }
    }

    /// Whether an expression calls and waits for nothing: only a number's
    /// methods, which change nothing, are allowed.
    fn quiet(&self, id: ExprId, proof: Proof) -> bool {
        match &self.ast.expr(id).kind {
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Name(_) => true,
            ExprKind::Array(items) | ExprKind::Tuple(items) => {
                items.iter().all(|&e| self.quiet(e, proof))
            }
            ExprKind::Paren(e) | ExprKind::Try(e) => self.quiet(*e, proof),
            ExprKind::Unary { op, operand } => *op != UnaryOp::Await && self.quiet(*operand, proof),
            ExprKind::Binary { lhs, rhs, .. } => self.quiet(*lhs, proof) && self.quiet(*rhs, proof),
            ExprKind::Template { exprs, .. } => exprs.iter().all(|&e| self.quiet(e, proof)),
            ExprKind::Field { base, .. } => self.quiet(*base, proof),
            ExprKind::Index { base, index } => {
                self.quiet(*base, proof) && self.quiet(*index, proof)
            }
            ExprKind::Match { scrutinee, arms } => {
                self.quiet(*scrutinee, proof)
                    && arms.iter().all(|arm| match &arm.body {
                        ArmBody::Expr(e) => self.quiet(*e, proof),
                        ArmBody::Block(block) => {
                            block.stmts.iter().all(|s| self.quiet_stmt(s, proof))
                        }
                    })
            }
            ExprKind::Call { callee, args, .. } => {
                let ExprKind::Field { base, .. } = &self.ast.expr(*callee).kind else {
                    return false;
                };
                matches!(self.ty(*base), Ty::Int(_) | Ty::Float(_))
                    && self.quiet(*base, proof)
                    && args.iter().all(|&e| self.quiet(e, proof))
            }
            ExprKind::Lambda { .. } | ExprKind::Error => false,
        }
    }

    /// Whether `base[index]` is proven in bounds by an enclosing loop.
    pub(crate) fn proven(&self, base: ExprId, index: ExprId) -> bool {
        let (Some(array), Some(counter)) = (
            self.analysis.names.get(&base),
            self.analysis.names.get(&index),
        ) else {
            return false;
        };
        self.current
            .proofs
            .iter()
            .any(|p| p.array == *array && p.counter == *counter)
    }
}
