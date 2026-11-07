//! Structures and options, as values: each is replaced by its scalars.
//!
//! A structure is a fixed-size value, so it lives in locals, never on the heap
//! (spec, "Where values live"). Lowering replaces it by its *leaves*: the scalar
//! fields, nested structures flattened, in declaration order. A `Point { x, y }`
//! local becomes two locals, a parameter two parameters, and a function returning
//! a `Point` returns two results ([`Stmt::ReturnMany`], called by
//! [`Stmt::CallMany`]).
//!
//! An `Option<T>` is a `bool`, whether a value is there, followed by the
//! scalars of `T`, which are zero when it is not.
//!
//! **Evaluation order.** A call returning several results cannot sit inside an
//! expression: it runs first, as a statement, into temporaries. To keep the
//! order of evaluation, any operand already lowered to its left that could call
//! or trap is evaluated into a temporary before it; on the right of `&&` and
//! `||` it runs only when the operand would; in a loop condition, at every turn.

use perasc_check::ty::{FloatTy, IntTy, StructId, Ty as CheckTy};
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, Type, TypeKind};

use super::heap::{self, Mode};
use super::{FnState, Lowering};
use crate::{BinOp, Const, Elem, Expr, ExprKind, LocalId, Pass, Shape, Stmt, Ty, UnOp};

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
            CheckTy::Array(inner) => match Self::ir_type(inner).and_then(Elem::of) {
                Some(_) => Ok(vec![(String::new(), Ty::Owned)]),
                None => Err("array-element"),
            },
            CheckTy::String => Ok(vec![(String::new(), Ty::Owned)]),
            CheckTy::Option(inner) => {
                let mut out = vec![("some".to_owned(), Ty::Bool)];
                let inner_leaves = self.leaves(inner)?;
                if inner_leaves.iter().any(|(_, t)| t.is_handle()) {
                    return Err("heap-option");
                }
                for (path, leaf) in inner_leaves {
                    let path = if path.is_empty() {
                        "value".to_owned()
                    } else {
                        format!("value_{path}")
                    };
                    out.push((path, leaf));
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
        match (text, args.as_slice()) {
            ("Option", [inner]) => return Ok(CheckTy::Option(Box::new(self.resolve(inner)?))),
            ("Array", [inner]) => return Ok(CheckTy::Array(Box::new(self.resolve(inner)?))),
            _ => {}
        }
        Ok(match (text, args.is_empty()) {
            ("i32", true) => CheckTy::Int(IntTy::I32),
            ("i64", true) => CheckTy::Int(IntTy::I64),
            ("f32", true) => CheckTy::Float(FloatTy::F32),
            ("f64", true) => CheckTy::Float(FloatTy::F64),
            ("bool", true) => CheckTy::Bool,
            ("String", true) => CheckTy::String,
            ("void", true) => CheckTy::Void,
            _ => return Err(super::feature_of_type(text)),
        })
    }

    /// How JavaScript sees a value of this type, as a result.
    pub(super) fn shape(&self, ty: &CheckTy) -> Shape {
        self.shape_passed(ty, Pass::Result)
    }

    /// How JavaScript sees a value of this type, passed this way.
    pub(super) fn shape_passed(&self, ty: &CheckTy, pass: Pass) -> Shape {
        match ty {
            CheckTy::Struct(id, name) => {
                let fields = self
                    .analysis
                    .structs
                    .get(id.0)
                    .map(|(_, fields)| {
                        fields
                            .iter()
                            .map(|(n, f)| (n.clone(), self.shape_passed(f, pass)))
                            .collect()
                    })
                    .unwrap_or_default();
                Shape::Object(name.clone(), fields)
            }
            CheckTy::Option(inner) => Shape::Option(Box::new(self.shape_passed(inner, pass))),
            CheckTy::String => Shape::String(pass),
            CheckTy::Array(inner) => Shape::Array(
                Self::ir_type(inner).and_then(Elem::of).unwrap_or(Elem::I32),
                pass,
            ),
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

    /// Whether the checker typed an expression as a structure or an option.
    pub(super) fn is_aggregate(&self, id: ExprId) -> bool {
        self.check_type(id).is_some_and(is_aggregate)
    }

    /// The scalars of any expression, taken by a new owner: one for a scalar,
    /// the leaves of a structure, an option or an array. Statements it needs
    /// first go to `state.pre`.
    pub(super) fn value(&mut self, state: &mut FnState, id: ExprId) -> Vec<Expr> {
        self.value_as(state, id, Mode::Consume)
    }

    /// The scalars of any expression, taken or only read: its buffers then
    /// move, or are lent.
    pub(super) fn value_as(&mut self, state: &mut FnState, id: ExprId, mode: Mode) -> Vec<Expr> {
        if !self.is_aggregate(id) {
            return vec![self.expr(state, id)];
        }
        let ast_expr = self.ast.expr(id);
        let span = ast_expr.span;
        match &ast_expr.kind {
            AstExpr::Paren(inner) => self.value_as(state, *inner, mode),
            AstExpr::Name(symbol) => {
                if let Some(super::Binding::Struct(locals)) = state.lookup(*symbol) {
                    let moves = mode == Mode::Consume && self.analysis.moves.contains(&id);
                    locals
                        .iter()
                        .map(|&l| {
                            if !state.is_owned(l) {
                                super::local_expr(state, l)
                            } else if moves {
                                heap::moved(state, l)
                            } else {
                                heap::borrowed(l)
                            }
                        })
                        .collect()
                } else if self.ast.name(*symbol) == "None" {
                    self.none(id)
                } else {
                    self.unsupported(span, "global");
                    Vec::new()
                }
            }
            AstExpr::Call { callee, args } => match self.ast.expr(*callee).kind {
                AstExpr::Name(symbol) => {
                    let id = self.fns.get(&symbol).copied();
                    let back = id
                        .map(|id| self.writeback_places(state, id, args))
                        .unwrap_or_default();
                    let args = self.arguments(state, id, args);
                    let values = if let Some(id) = id {
                        self.call_results(state, id, args, &back)
                    } else if self.ast.name(symbol) == "Some" {
                        let mut out = vec![bool_const(true)];
                        out.extend(args);
                        out
                    } else {
                        // A constructor: its scalars are its arguments', in order.
                        args
                    };
                    Self::fresh(state, values, mode)
                }
                AstExpr::Field { base, name } => {
                    let method = self.ast.name(name.symbol);
                    self.method_values(state, base, method, args, mode, span)
                }
                _ => {
                    self.unsupported(span, "function-value");
                    Vec::new()
                }
            },
            AstExpr::Try(inner) => self.try_values(state, *inner, span),
            AstExpr::String(text) => self.string_literal(state, text, mode),
            AstExpr::Binary {
                op: perasc_syntax::ast::BinaryOp::Add,
                lhs,
                rhs,
            } => self.concat(state, *lhs, *rhs, mode),
            AstExpr::Array(items) => {
                let built = self.array_literal(state, id, items);
                Self::fresh(state, built.into_iter().collect(), mode)
            }
            AstExpr::Field { base, name } => {
                // A field is read from its structure, which keeps it.
                let fields = self.value_as(state, *base, Mode::Borrow);
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

    /// `None`, of the type the checker gave it: `false`, then zeros.
    fn none(&self, id: ExprId) -> Vec<Expr> {
        let leaves = self
            .check_type(id)
            .and_then(|t| self.leaves(t).ok())
            .unwrap_or_default();
        leaves.iter().map(|(_, t)| super::zero(*t)).collect()
    }

    /// A method whose receiver or result is an option, as scalars.
    pub(super) fn method_values(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        method: &str,
        args: &[ExprId],
        mode: Mode,
        span: perasc_diag::Span,
    ) -> Vec<Expr> {
        if self.is_buffer(base) {
            return self.array_method(state, base, method, args, mode, span);
        }
        if !self.is_aggregate(base) {
            let op = match method {
                "checkedAdd" => BinOp::Add,
                "checkedSub" => BinOp::Sub,
                "checkedMul" => BinOp::Mul,
                _ => {
                    self.unsupported(span, "method");
                    return Vec::new();
                }
            };
            let receiver = self.expr(state, base);
            let mark = state.pre.len();
            let arg = args.first().map(|&a| self.expr(state, a));
            let mut operands = [receiver];
            state.keep_before(mark, &mut operands);
            let [receiver] = operands;
            return match arg {
                Some(arg) => checked(state, op, receiver, arg),
                None => Vec::new(),
            };
        }
        let mut fields = self.value_as(state, base, Mode::Borrow);
        match method {
            "isSome" | "isNone" => {
                let some = Self::pick(state, fields, 0, 1);
                let Some(some) = some.into_iter().next() else {
                    return Vec::new();
                };
                if method == "isSome" {
                    vec![some]
                } else {
                    vec![Expr {
                        kind: ExprKind::Unary(UnOp::Not, Box::new(some)),
                        ty: Ty::Bool,
                    }]
                }
            }
            "unwrapOr" => {
                // The receiver, then the fallback; then the value is chosen.
                let mark = state.pre.len();
                let fallback = self.arguments(state, None, args);
                state.keep_before(mark, &mut fields);
                let fields: Vec<Expr> = fields.into_iter().map(|e| state.spill(e)).collect();
                let Some((some, payload)) = fields.split_first() else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                let mut then = Vec::new();
                for (value, fallback) in payload.iter().zip(fallback) {
                    let temp = state.temp(fallback.ty);
                    state.pre.push(Stmt::Set(temp, fallback));
                    then.push(Stmt::Set(temp, value.clone()));
                    out.push(super::local_expr(state, temp));
                }
                state.pre.push(Stmt::If {
                    cond: some.clone(),
                    then,
                    otherwise: Vec::new(),
                });
                out
            }
            _ => {
                self.unsupported(span, "method");
                Vec::new()
            }
        }
    }

    /// `x?`: the value of an option, or a return of `None` when it has none.
    pub(super) fn try_values(
        &mut self,
        state: &mut FnState,
        inner: ExprId,
        span: perasc_diag::Span,
    ) -> Vec<Expr> {
        let Some(ret) = state.ret_option.clone() else {
            self.unsupported(span, "try");
            return Vec::new();
        };
        let fields: Vec<Expr> = self
            .value(state, inner)
            .into_iter()
            .map(|e| state.spill(e))
            .collect();
        let Some((some, payload)) = fields.split_first() else {
            return Vec::new();
        };
        // `None`, and the `inout` numbers as they are; what the function holds
        // is released first.
        let mut none: Vec<Expr> = ret.iter().map(|t| super::zero(*t)).collect();
        none.extend(state.writeback_values());
        let mut then = state.exit_releases();
        then.push(super::return_of(none));
        state.pre.push(Stmt::If {
            cond: Expr {
                kind: ExprKind::Unary(UnOp::Not, Box::new(some.clone())),
                ty: Ty::Bool,
            },
            then,
            otherwise: Vec::new(),
        });
        payload.to_vec()
    }

    /// `len` scalars from `offset`. The others are dropped, so any of them that
    /// could call or trap is evaluated first, with all the others, in order.
    pub(super) fn pick(
        state: &mut FnState,
        fields: Vec<Expr>,
        offset: usize,
        len: usize,
    ) -> Vec<Expr> {
        let fields = if fields.iter().all(heap::pure) {
            fields
        } else {
            fields.into_iter().map(|e| state.spill(e)).collect()
        };
        fields.into_iter().skip(offset).take(len).collect()
    }

    /// Call arguments, flattened, in order (see the module documentation). A
    /// `sink` argument, or one of a constructor, is taken; any other is lent.
    pub(super) fn arguments(
        &mut self,
        state: &mut FnState,
        callee: Option<crate::FnId>,
        args: &[ExprId],
    ) -> Vec<Expr> {
        let conventions = callee
            .and_then(|id| self.conventions.get(&id))
            .cloned()
            .unwrap_or_default();
        let mut out: Vec<Expr> = Vec::new();
        for (i, &arg) in args.iter().enumerate() {
            let (arg, mode) = match (callee, conventions.get(i)) {
                (None, _) | (Some(_), Some(perasc_syntax::ast::Convention::Sink)) => {
                    (arg, Mode::Consume)
                }
                _ => (self.unmarked(arg), Mode::Borrow),
            };
            let mark = state.pre.len();
            let value = self.value_as(state, arg, mode);
            state.keep_before(mark, &mut out);
            out.extend(value);
        }
        out
    }

    /// The numbers of the `inout` arguments of a call, in order: the locals the
    /// function's extra results are written back into.
    pub(super) fn writeback_places(
        &mut self,
        state: &FnState,
        callee: crate::FnId,
        args: &[ExprId],
    ) -> Vec<LocalId> {
        let conventions = self.conventions.get(&callee).cloned().unwrap_or_default();
        let mut out = Vec::new();
        for (&arg, convention) in args.iter().zip(conventions) {
            if convention != perasc_syntax::ast::Convention::Inout {
                continue;
            }
            let place = self.unmarked(arg);
            for local in self.place(state, place).unwrap_or_default() {
                if !state.is_owned(local) && !state.is_borrowed(local) {
                    out.push(local);
                }
            }
        }
        out
    }

    /// An argument without its `&`.
    fn unmarked(&self, arg: ExprId) -> ExprId {
        match self.ast.expr(arg).kind {
            AstExpr::Unary {
                op: perasc_syntax::ast::UnaryOp::Borrow,
                operand,
            } => operand,
            AstExpr::Paren(inner) => self.unmarked(inner),
            _ => arg,
        }
    }

    /// The results of a call to a function of the module, as scalars: a call
    /// expression for one result, temporaries filled by a statement for several.
    pub(super) fn call_results(
        &mut self,
        state: &mut FnState,
        id: crate::FnId,
        args: Vec<Expr>,
        back: &[LocalId],
    ) -> Vec<Expr> {
        let results = self.results.get(&id).cloned().unwrap_or_default();
        if !back.is_empty() {
            // The function also returns its `inout` numbers, last: they go
            // straight back into the caller's variables.
            let declared = results.len().saturating_sub(back.len());
            if results.len() == 1 {
                let target = back.first().copied().unwrap_or(LocalId(0));
                state.pre.push(Stmt::Set(
                    target,
                    Expr {
                        kind: ExprKind::Call(id, args),
                        ty: results.first().copied().unwrap_or(Ty::Void),
                    },
                ));
                return Vec::new();
            }
            let temps: Vec<LocalId> = results
                .iter()
                .take(declared)
                .map(|&ty| state.temp(ty))
                .collect();
            let mut targets = temps.clone();
            targets.extend_from_slice(back);
            state.pre.push(Stmt::CallMany {
                targets,
                callee: id,
                args,
            });
            return temps.iter().map(|&t| super::local_expr(state, t)).collect();
        }
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
    /// `p = Point(p.y, p.x)` swaps. A buffer a target still holds is released
    /// just before it receives the new one.
    pub(super) fn assign(
        state: &mut FnState,
        targets: &[LocalId],
        values: Vec<Expr>,
        out: &mut Vec<Stmt>,
    ) {
        let overlaps = values.iter().enumerate().any(|(i, v)| {
            targets.iter().take(i).any(|t| reads(v, *t))
                || targets
                    .get(i)
                    .is_some_and(|t| state.live.contains(&t.0) && reads(v, *t))
        });
        let values: Vec<Expr> = if overlaps {
            values
                .into_iter()
                .map(|v| {
                    let owned = v.ty == Ty::Owned;
                    let temp = state.temp(v.ty);
                    out.push(Stmt::Set(temp, v));
                    if owned {
                        heap::moved(state, temp)
                    } else {
                        super::local_expr(state, temp)
                    }
                })
                .collect()
        } else {
            values
        };
        for (target, value) in targets.iter().zip(values) {
            if state.live.contains(&target.0) {
                out.push(state.release(*target));
            }
            out.push(Stmt::Set(*target, value));
            if state.is_owned(*target) {
                state.live.insert(target.0);
            }
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
        if heap::pure(&value) {
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
            if !heap::pure(operand) {
                let temp = self.temp(operand.ty);
                let value = std::mem::replace(operand, super::local_expr(self, temp));
                self.pre.insert(at, Stmt::Set(temp, value));
                at += 1;
            }
        }
    }
}

/// Structures, options and arrays: values lowered through [`Lowering::value`].
pub(super) const fn is_aggregate(ty: &CheckTy) -> bool {
    matches!(
        ty,
        CheckTy::Struct(..) | CheckTy::Option(_) | CheckTy::Array(_) | CheckTy::String
    )
}

fn bool_const(value: bool) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Bool(value)),
        ty: Ty::Bool,
    }
}

fn convert(e: Expr, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Convert(Box::new(e)),
        ty,
    }
}

/// `a.checkedAdd(b)` and the others: whether the exact result fits, and the
/// wrapped result. An `i32` is computed in 64 bits; an `i64` is checked on its
/// sign bits, and a product by dividing it again.
fn checked(state: &mut FnState, op: BinOp, a: Expr, b: Expr) -> Vec<Expr> {
    use super::binary;
    let a = state.spill(a);
    let b = state.spill(b);
    let ty = a.ty;
    let int = |v: i64| Expr {
        kind: ExprKind::Const(Const::Int(v)),
        ty,
    };
    if ty == Ty::I32 {
        let wide = binary(op, convert(a, Ty::I64), convert(b, Ty::I64));
        let wide = force(state, wide);
        let narrow = convert(wide.clone(), Ty::I32);
        let fits = binary(BinOp::Eq, convert(narrow.clone(), Ty::I64), wide);
        return vec![fits, narrow];
    }
    let r = force(state, binary(op, a.clone(), b.clone()));
    let xor = |x: &Expr, y: &Expr| binary(BinOp::BitXor, x.clone(), y.clone());
    let fits = match op {
        BinOp::Add => binary(
            BinOp::Ge,
            binary(BinOp::BitAnd, xor(&a, &r), xor(&b, &r)),
            int(0),
        ),
        BinOp::Sub => binary(
            BinOp::Ge,
            binary(BinOp::BitAnd, xor(&a, &b), xor(&a, &r)),
            int(0),
        ),
        _ => {
            // a == 0 || (!(a == -1 && b == MIN) && r / a == b)
            let min = binary(
                BinOp::And,
                binary(BinOp::Eq, a.clone(), int(-1)),
                binary(BinOp::Eq, b.clone(), int(i64::MIN)),
            );
            let not_min = Expr {
                kind: ExprKind::Unary(UnOp::Not, Box::new(min)),
                ty: Ty::Bool,
            };
            let back = binary(BinOp::Eq, binary(BinOp::Div, r.clone(), a.clone()), b);
            binary(
                BinOp::Or,
                binary(BinOp::Eq, a, int(0)),
                binary(BinOp::And, not_min, back),
            )
        }
    };
    let fits = force(state, fits);
    vec![fits, r]
}

/// The value, always evaluated now into a temporary.
fn force(state: &mut FnState, value: Expr) -> Expr {
    let temp = state.temp(value.ty);
    state.pre.push(Stmt::Set(temp, value));
    super::local_expr(state, temp)
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
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => args.iter().any(|a| reads(a, local)),
    }
}
