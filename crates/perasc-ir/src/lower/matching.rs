//! Enumerations and `match`.
//!
//! An enumeration is a tag, the index of its variant, followed by the scalars
//! of every variant's data; those of the other variants are zero, and their
//! buffers empty, so that the value always owns the same buffers whatever its
//! variant: the release verifier needs no flag to follow it.
//!
//! A `match` is a chain of `if`: each arm tests its pattern on the scalars of
//! the value, gives names to the parts it binds, and runs its body. The last
//! arm needs no test: the checker proved the arms cover every value. Where
//! the arms meet, what one of them moved is released on the others, as after
//! an `if`.

use std::collections::BTreeSet;

use perasc_check::ty::Ty as CheckTy;
use perasc_syntax::ast::{Arm, ArmBody, ExprId, ExprKind as AstExpr, Pattern, PatternKind};

use super::heap::Mode;
use super::{Binding, FnState, Lowering};
use crate::{BinOp, Const, Expr, ExprKind, LocalId, Stmt, Ty};

/// An arm lowered: its test (none for the last), its statements, and what is
/// live at its end.
struct Branch {
    cond: Option<Expr>,
    stmts: Vec<Stmt>,
    live: BTreeSet<u32>,
    gone: bool,
}

impl Lowering<'_> {
    /// The variants of a type that has some, with the types of their data.
    fn variants(&self, ty: &CheckTy) -> Option<Vec<(String, Vec<CheckTy>)>> {
        match ty {
            CheckTy::Enum(id, _) => self.analysis.enums.get(id.0).map(|(_, v)| v.clone()),
            CheckTy::Option(inner) => Some(vec![
                ("None".to_owned(), Vec::new()),
                ("Some".to_owned(), vec![(**inner).clone()]),
            ]),
            CheckTy::Result(ok, err) => Some(vec![
                ("Ok".to_owned(), vec![(**ok).clone()]),
                ("Err".to_owned(), vec![(**err).clone()]),
            ]),
            _ => None,
        }
    }

    /// Where the data of variant `index` starts among the scalars of `ty`, and
    /// the number of scalars of each of its fields.
    fn variant_layout(&self, ty: &CheckTy, index: usize) -> (usize, Vec<usize>) {
        let variants = self.variants(ty).unwrap_or_default();
        let width = |t: &CheckTy| self.leaves(t).map_or(0, |l| l.len());
        let start = 1 + variants
            .iter()
            .take(index)
            .flat_map(|(_, fields)| fields.iter().map(width))
            .sum::<usize>();
        let fields = variants
            .get(index)
            .map(|(_, fields)| fields.iter().map(width).collect())
            .unwrap_or_default();
        (start, fields)
    }

    /// `Enum.Variant(args)` or `Enum.Variant`, when `base` names an
    /// enumeration: its tag, its data, zero for the other variants.
    pub(super) fn enum_value(
        &mut self,
        state: &mut FnState,
        id: ExprId,
        base: ExprId,
        variant: &str,
        args: &[ExprId],
    ) -> Option<Vec<Expr>> {
        let AstExpr::Name(symbol) = self.ast.expr(base).kind else {
            return None;
        };
        if state.lookup(symbol).is_some() {
            return None;
        }
        let ty = self.check_type(id)?.clone();
        let CheckTy::Enum(..) = ty else {
            return None;
        };
        let variants = self.variants(&ty)?;
        let index = variants.iter().position(|(v, _)| v == variant)?;
        let data = self.arguments(state, None, args);
        let mut out = vec![int(i64::try_from(index).unwrap_or(0))];
        let mut data = data.into_iter();
        for (k, (_, fields)) in variants.iter().enumerate() {
            for field in fields {
                if k == index {
                    let width = self.leaves(field).map_or(0, |l| l.len());
                    out.extend(data.by_ref().take(width));
                } else {
                    out.extend(self.zero_value(field));
                }
            }
        }
        Some(out)
    }

    /// Whether `base` names an enumeration, not a variable.
    pub(super) fn enum_value_named(&self, state: &FnState, base: ExprId) -> bool {
        let AstExpr::Name(symbol) = self.ast.expr(base).kind else {
            return false;
        };
        let text = self.ast.name(symbol);
        state.lookup(symbol).is_none() && self.analysis.enums.iter().any(|(n, _)| n == text)
    }

    /// `match (scrutinee) { arms }`, as the scalars of its value; none when
    /// its arms are blocks.
    pub(super) fn match_values(
        &mut self,
        state: &mut FnState,
        id: ExprId,
        scrutinee: ExprId,
        arms: &[Arm],
        mode: Mode,
    ) -> Vec<Expr> {
        let Some(ty) = self.check_type(scrutinee).cloned() else {
            return Vec::new();
        };
        // The value matched, lent; its numbers copied, so that an arm
        // changing the variable matched does not change what it tests.
        let values: Vec<Expr> = self
            .value_as(state, scrutinee, Mode::Borrow)
            .into_iter()
            .map(|v| {
                if v.ty.is_handle() || matches!(v.kind, ExprKind::Const(_)) {
                    v
                } else {
                    let temp = state.temp(v.ty);
                    state.pre.push(Stmt::Set(temp, v));
                    super::local_expr(state, temp)
                }
            })
            .collect();
        // The result, in temporaries every arm fills.
        let result_ty = self.check_type(id).cloned().unwrap_or(CheckTy::Void);
        let results: Vec<LocalId> = if result_ty == CheckTy::Void {
            Vec::new()
        } else {
            self.leaves(&result_ty)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, t)| state.temp(t))
                .collect()
        };
        let outer_pre = std::mem::take(&mut state.pre);
        let outer_drops = std::mem::take(&mut state.drops);
        let start = state.live.clone();
        let mut branches = Vec::with_capacity(arms.len());
        for (n, arm) in arms.iter().enumerate() {
            state.live.clone_from(&start);
            state.gone = false;
            state.open();
            let cond = self.pattern(state, &arm.pattern, &ty, &values);
            let mut stmts = std::mem::take(&mut state.pre);
            match &arm.body {
                ArmBody::Expr(value) => {
                    let values = self.value(state, *value);
                    state.flush(&mut stmts);
                    if results.is_empty() {
                        for value in values {
                            if !super::heap::pure(&value) {
                                stmts.push(Stmt::Expr(value));
                            }
                        }
                    } else {
                        Self::assign(state, &results, values, &mut stmts);
                    }
                    state.end_statement(&mut stmts);
                }
                ArmBody::Block(block) => self.block(state, &block.stmts, &mut stmts),
            }
            state.close(&mut stmts);
            // The last arm runs when no other did: the checker proved the arms
            // cover every value.
            let cond = if n + 1 == arms.len() { None } else { cond };
            branches.push(Branch {
                cond,
                stmts,
                live: state.live.clone(),
                gone: state.gone,
            });
            if branches.last().is_some_and(|b| b.cond.is_none()) {
                break;
            }
        }
        state.pre = outer_pre;
        state.drops = outer_drops;
        Self::join(state, &mut branches);
        let mut chain: Vec<Stmt> = Vec::new();
        for branch in branches.into_iter().rev() {
            chain = match branch.cond {
                None => branch.stmts,
                Some(cond) => vec![Stmt::If {
                    cond,
                    then: branch.stmts,
                    otherwise: chain,
                }],
            };
        }
        state.pre.extend(chain);
        let filled: Vec<Expr> = results
            .iter()
            .map(|&r| super::local_expr(state, r))
            .collect();
        Self::fresh(state, filled, mode)
    }

    /// Where the arms meet: a local live at the end of one arm and not of
    /// another is released at the end of the one that holds it.
    fn join(state: &mut FnState, branches: &mut [Branch]) {
        let ends: Vec<&BTreeSet<u32>> = branches
            .iter()
            .filter(|b| !b.gone)
            .map(|b| &b.live)
            .collect();
        let Some((first, rest)) = ends.split_first() else {
            state.gone = true;
            return;
        };
        let target: BTreeSet<u32> = rest.iter().fold((*first).clone(), |acc, s| {
            acc.intersection(s).copied().collect()
        });
        for branch in branches.iter_mut().filter(|b| !b.gone) {
            for &local in branch.live.difference(&target) {
                branch.stmts.push(super::heap::release_stmt(LocalId(local)));
            }
        }
        state.live = target;
        state.gone = false;
    }

    /// The test of a pattern on the scalars `values` of a value of type `ty`,
    /// none when it always matches; the names it binds are declared in the
    /// current scope.
    fn pattern(
        &mut self,
        state: &mut FnState,
        pattern: &Pattern,
        ty: &CheckTy,
        values: &[Expr],
    ) -> Option<Expr> {
        match &pattern.kind {
            PatternKind::Wildcard => None,
            PatternKind::Name(ident) => {
                let text = self.ast.name(ident.symbol);
                let variants = self.variants(ty).unwrap_or_default();
                if let Some(index) = variants.iter().position(|(v, _)| v == text) {
                    return Some(tag_is(values, index));
                }
                bind(state, ident.symbol, values);
                None
            }
            PatternKind::Literal(literal) => {
                let value = self.expr(state, *literal);
                let first = values.first()?.clone();
                Some(super::binary(BinOp::Eq, first, value))
            }
            PatternKind::Variant { name, fields, .. } => {
                let variant = self.ast.name(name.symbol);
                let variants = self.variants(ty)?;
                let index = variants.iter().position(|(v, _)| v == variant)?;
                let (start, widths) = self.variant_layout(ty, index);
                let types = variants
                    .get(index)
                    .map(|(_, t)| t.clone())
                    .unwrap_or_default();
                let mut test = tag_is(values, index);
                let mut offset = start;
                for ((field, field_ty), width) in fields.iter().zip(&types).zip(widths) {
                    let part = values.get(offset..offset + width).unwrap_or_default();
                    if let Some(inner) = self.pattern(state, field, field_ty, part) {
                        test = super::binary(BinOp::And, test, inner);
                    }
                    offset += width;
                }
                Some(test)
            }
        }
    }
}

/// `tag == index`, the tag being the first scalar.
fn tag_is(values: &[Expr], index: usize) -> Expr {
    let tag = values.first().cloned().unwrap_or_else(|| int(0));
    // An option's tag is its `bool`.
    if tag.ty == Ty::Bool {
        return if index == 1 {
            tag
        } else {
            Expr {
                kind: ExprKind::Unary(crate::UnOp::Not, Box::new(tag)),
                ty: Ty::Bool,
            }
        };
    }
    super::binary(BinOp::Eq, tag, int(i64::try_from(index).unwrap_or(0)))
}

/// Gives a name to a part of the value matched: a number is a local, any
/// other part is read where it is used, lent.
fn bind(state: &mut FnState, symbol: perasc_syntax::ast::Symbol, values: &[Expr]) {
    let binding = match values {
        [
            Expr {
                kind: ExprKind::Local(local),
                ty,
            },
        ] if !ty.is_handle() => Binding::Scalar(*local),
        [value] if !value.ty.is_handle() => {
            let temp = state.temp(value.ty);
            state.pre.push(Stmt::Set(temp, value.clone()));
            Binding::Scalar(temp)
        }
        _ => Binding::Lent(values.to_vec()),
    };
    if let Some(scope) = state.scopes.last_mut() {
        scope.insert(symbol, binding);
    }
}

fn int(value: i64) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(value)),
        ty: Ty::I32,
    }
}
