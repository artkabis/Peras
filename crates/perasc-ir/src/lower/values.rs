//! Structures, as values: every structure is replaced by its scalars.
//!
//! A structure is a fixed-size value, so it lives in locals, never on the heap
//! (spec, "Where values live"). Lowering replaces it by its *leaves*: the scalar
//! fields, nested structures flattened, in declaration order. A `Point { x, y }`
//! local becomes two locals, a parameter two parameters, and a function returning
//! a `Point` returns two results ([`Stmt::ReturnMany`], called by
//! [`Stmt::CallMany`]).
//!
//! **Evaluation order.** A call returning several results cannot sit inside an
//! expression: it runs first, as a statement, into temporaries. To keep the
//! order of evaluation, any operand already lowered to its left that could call
//! or trap is evaluated into a temporary before it; on the right of `&&` and
//! `||` it runs only when the operand would; in a loop condition, at every turn.

use perasc_check::ty::{FloatTy, IntTy, StructId, Ty as CheckTy};
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, Type, TypeKind};

use super::{FnState, Lowering};
use crate::opt::is_simple;
use crate::{Expr, ExprKind, LocalId, Shape, Stmt, Ty};

impl Lowering<'_> {
    /// The scalars of a type with their path (`inner_x`), or the feature that is
    /// not compiled yet. A scalar has the empty path.
    pub(super) fn leaves(&self, ty: &CheckTy) -> Result<Vec<(String, Ty)>, &'static str> {
        match ty {
            CheckTy::Struct(id, _) => {
                let (_, fields) = self.analysis.structs.get(id.0).ok_or("struct")?;
                let mut out = Vec::new();
                for (name, field) in fields {
                    for (path, leaf) in self.leaves(field)? {
                        let path = if path.is_empty() {
                            name.clone()
                        } else {
                            format!("{name}_{path}")
                        };
                        out.push((path, leaf));
                    }
                }
                Ok(out)
            }
            other => match Self::ir_type(other) {
                Some(Ty::Void) | None => Err(super::feature_of_check_type(other)),
                Some(leaf) => Ok(vec![(String::new(), leaf)]),
            },
        }
    }

    /// The checker type of a type written in source, or the feature that is not
    /// compiled yet.
    pub(super) fn resolve(&self, ty: &Type) -> Result<CheckTy, &'static str> {
        let TypeKind::Named { name, args } = &ty.kind else {
            return Err("struct");
        };
        let text = self.ast.name(name.symbol);
        if let Some(index) = self.analysis.structs.iter().position(|(n, _)| n == text) {
            return Ok(CheckTy::Struct(StructId(index), text.to_owned()));
        }
        Ok(match (text, args.is_empty()) {
            ("i32", true) => CheckTy::Int(IntTy::I32),
            ("i64", true) => CheckTy::Int(IntTy::I64),
            ("f32", true) => CheckTy::Float(FloatTy::F32),
            ("f64", true) => CheckTy::Float(FloatTy::F64),
            ("bool", true) => CheckTy::Bool,
            ("void", true) => CheckTy::Void,
            _ => return Err(super::feature_of_type(text)),
        })
    }

    /// How JavaScript sees a value of this type.
    pub(super) fn shape(&self, ty: &CheckTy) -> Shape {
        match ty {
            CheckTy::Struct(id, name) => {
                let fields = self
                    .analysis
                    .structs
                    .get(id.0)
                    .map(|(_, fields)| {
                        fields
                            .iter()
                            .map(|(n, f)| (n.clone(), self.shape(f)))
                            .collect()
                    })
                    .unwrap_or_default();
                Shape::Object(name.clone(), fields)
            }
            other => Shape::Value(Self::ir_type(other).unwrap_or(Ty::Void)),
        }
    }

    /// Where a field's scalars sit among the structure's, and the field's type.
    pub(super) fn field(&self, ty: &CheckTy, name: &str) -> Option<(usize, usize, CheckTy)> {
        let CheckTy::Struct(id, _) = ty else {
            return None;
        };
        let (_, fields) = self.analysis.structs.get(id.0)?;
        let mut offset = 0;
        for (field, field_ty) in fields {
            let len = self.leaves(field_ty).ok()?.len();
            if field == name {
                return Some((offset, len, field_ty.clone()));
            }
            offset += len;
        }
        None
    }

    /// The type the checker gave an expression.
    pub(super) fn check_type(&self, id: ExprId) -> Option<&CheckTy> {
        self.analysis.types.get(&id)
    }

    /// Whether the checker typed an expression as a structure.
    pub(super) fn is_struct(&self, id: ExprId) -> bool {
        matches!(self.check_type(id), Some(CheckTy::Struct(..)))
    }

    /// The scalars of any expression: one for a scalar, the leaves of a
    /// structure. Statements it needs first go to `state.pre`.
    pub(super) fn value(&mut self, state: &mut FnState, id: ExprId) -> Vec<Expr> {
        if !self.is_struct(id) {
            return vec![self.expr(state, id)];
        }
        let ast_expr = self.ast.expr(id);
        let span = ast_expr.span;
        match &ast_expr.kind {
            AstExpr::Paren(inner) => self.value(state, *inner),
            AstExpr::Name(symbol) => {
                if let Some(super::Binding::Struct(locals)) = state.lookup(*symbol) {
                    locals
                        .iter()
                        .map(|l| super::local_expr(state, *l))
                        .collect()
                } else {
                    self.unsupported(span, "global");
                    Vec::new()
                }
            }
            AstExpr::Call { callee, args } => {
                let AstExpr::Name(symbol) = self.ast.expr(*callee).kind else {
                    self.unsupported(span, "method");
                    return Vec::new();
                };
                let args = self.arguments(state, args);
                if let Some(&id) = self.fns.get(&symbol) {
                    self.call_results(state, id, args)
                } else {
                    // A constructor: its scalars are its arguments', in order.
                    args
                }
            }
            AstExpr::Field { base, name } => {
                let fields = self.value(state, *base);
                let base_ty = self.check_type(*base).cloned();
                let picked = base_ty.and_then(|t| self.field(&t, self.ast.name(name.symbol)));
                match picked {
                    Some((offset, len, _)) => Self::pick(state, fields, offset, len),
                    None => Vec::new(),
                }
            }
            _ => {
                self.unsupported(span, "struct");
                Vec::new()
            }
        }
    }

    /// `len` scalars from `offset`. The others are dropped, so any of them that
    /// could call or trap is evaluated first, with all the others, in order.
    pub(super) fn pick(
        state: &mut FnState,
        fields: Vec<Expr>,
        offset: usize,
        len: usize,
    ) -> Vec<Expr> {
        let fields = if fields.iter().all(is_simple) {
            fields
        } else {
            fields.into_iter().map(|e| state.spill(e)).collect()
        };
        fields.into_iter().skip(offset).take(len).collect()
    }

    /// Call arguments, flattened, in order (see the module documentation).
    pub(super) fn arguments(&mut self, state: &mut FnState, args: &[ExprId]) -> Vec<Expr> {
        let mut out: Vec<Expr> = Vec::new();
        for &arg in args {
            let mark = state.pre.len();
            let value = self.value(state, arg);
            state.keep_before(mark, &mut out);
            out.extend(value);
        }
        out
    }

    /// The results of a call to a function of the module, as scalars: a call
    /// expression for one result, temporaries filled by a statement for several.
    pub(super) fn call_results(
        &mut self,
        state: &mut FnState,
        id: crate::FnId,
        args: Vec<Expr>,
    ) -> Vec<Expr> {
        let results = self.results.get(&id).cloned().unwrap_or_default();
        match results.as_slice() {
            [] => {
                state.pre.push(Stmt::Expr(Expr {
                    kind: ExprKind::Call(id, args),
                    ty: Ty::Void,
                }));
                Vec::new()
            }
            [one] => vec![Expr {
                kind: ExprKind::Call(id, args),
                ty: *one,
            }],
            many => {
                let targets: Vec<LocalId> = many.iter().map(|&ty| state.temp(ty)).collect();
                state.pre.push(Stmt::CallMany {
                    targets: targets.clone(),
                    callee: id,
                    args,
                });
                targets
                    .iter()
                    .map(|&t| super::local_expr(state, t))
                    .collect()
            }
        }
    }

    /// `targets = values`, scalar by scalar. When a value reads a target written
    /// before it, every value is first evaluated into a temporary, so that
    /// `p = Point(p.y, p.x)` swaps.
    pub(super) fn assign(
        state: &mut FnState,
        targets: &[LocalId],
        values: Vec<Expr>,
        out: &mut Vec<Stmt>,
    ) {
        let overlaps = values
            .iter()
            .enumerate()
            .any(|(i, v)| targets.iter().take(i).any(|t| reads(v, *t)));
        let values: Vec<Expr> = if overlaps {
            values
                .into_iter()
                .map(|v| {
                    let temp = state.temp(v.ty);
                    out.push(Stmt::Set(temp, v));
                    super::local_expr(state, temp)
                })
                .collect()
        } else {
            values
        };
        for (target, value) in targets.iter().zip(values) {
            out.push(Stmt::Set(*target, value));
        }
    }
}

impl FnState {
    /// A new local that no name refers to.
    pub(super) fn temp(&mut self, ty: Ty) -> LocalId {
        let id = LocalId(u32::try_from(self.locals.len()).unwrap_or(u32::MAX));
        self.locals.push(crate::Local {
            name: "tmp".to_owned(),
            ty,
        });
        id
    }

    /// The value, evaluated now into a temporary unless it is simple.
    pub(super) fn spill(&mut self, value: Expr) -> Expr {
        if is_simple(&value) {
            return value;
        }
        let temp = self.temp(value.ty);
        self.pre.push(Stmt::Set(temp, value));
        super::local_expr(self, temp)
    }

    /// If statements were added to `pre` since `mark`, the operands already
    /// lowered that could call or trap are evaluated before them, in order.
    pub(super) fn keep_before(&mut self, mark: usize, operands: &mut [Expr]) {
        if self.pre.len() == mark {
            return;
        }
        let mut at = mark;
        for operand in operands.iter_mut() {
            if !is_simple(operand) {
                let temp = self.temp(operand.ty);
                let value = std::mem::replace(operand, super::local_expr(self, temp));
                self.pre.insert(at, Stmt::Set(temp, value));
                at += 1;
            }
        }
    }
}

/// Whether an expression reads a local.
fn reads(e: &Expr, local: LocalId) -> bool {
    match &e.kind {
        ExprKind::Local(l) | ExprKind::Move(l) | ExprKind::Borrow(l) => *l == local,
        ExprKind::Const(_) => false,
        ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Math(_, x)
        | ExprKind::Alloc(x) => reads(x, local),
        ExprKind::Binary(_, a, b) => reads(a, local) || reads(b, local),
        ExprKind::Call(_, args) => args.iter().any(|a| reads(a, local)),
    }
}
