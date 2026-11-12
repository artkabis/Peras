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

use std::collections::BTreeSet;

use perasc_check::ty::{IntTy, Ty as CheckTy};
use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, Type};

use super::heap::{self, Mode};
use super::{FnState, Lowering};
use crate::{BinOp, Const, Elem, Expr, ExprKind, LocalId, Pass, Shape, Stmt, Ty, UnOp};

impl Lowering<'_> {
    /// The scalars of a type with their path (`inner_x`), or the feature that is
    /// not compiled yet. A scalar has the empty path.
    pub(super) fn leaves(&self, ty: &CheckTy) -> Result<Vec<(String, Ty)>, &'static str> {
        self.leaves_in(ty, 0)
    }

    /// The shape of each leaf of `ty`, in the order of [`Self::leaves`], as
    /// [`crate::RuntimeFn::BufEqual`] compares a column of it: a number, a
    /// string, an array of them, a structure of them. `None` for what it does
    /// not describe: an option, an enumeration, a map, nesting deeper than
    /// seven buffers (decision 0021, finding F305).
    pub(super) fn shapes(&self, ty: &CheckTy) -> Option<Vec<u32>> {
        const HANDLE: u32 = 7;
        const DEEPEST: u32 = 1 << 24;
        let nested = |inner: Vec<u32>| -> Option<Vec<u32>> {
            inner
                .into_iter()
                .map(|s| (s < DEEPEST).then_some(HANDLE | (s << 4)))
                .collect()
        };
        match ty {
            CheckTy::Struct(id, _) => {
                let (_, fields) = self.analysis.structs.get(id.0)?;
                let mut out = Vec::new();
                for (_, field) in fields {
                    out.extend(self.shapes(field)?);
                }
                Some(out)
            }
            CheckTy::Array(inner) => nested(self.shapes(inner)?),
            CheckTy::String => Some(vec![HANDLE | (Elem::U8.shape() << 4)]),
            other => {
                let elem = Elem::of(Self::ir_type(other)?)?;
                (elem != Elem::Handle).then_some(vec![elem.shape()])
            }
        }
    }

    /// [`Self::leaves`], `depth` types deep: a type that contains itself,
    /// through an array as in `struct Tree { children: Array<Tree> }`, has no
    /// end, and is not compiled yet.
    fn leaves_in(&self, ty: &CheckTy, depth: usize) -> Result<Vec<(String, Ty)>, &'static str> {
        const DEEPEST: usize = 64;
        if depth > DEEPEST {
            return Err("recursive-type");
        }
        let deeper = depth + 1;
        match ty {
            CheckTy::Struct(id, _) => {
                let (_, fields) = self.analysis.structs.get(id.0).ok_or("struct")?;
                let mut out = Vec::new();
                for (name, field) in fields {
                    for (path, leaf) in self.leaves_in(field, deeper)? {
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
            // One buffer per scalar of an element: its columns.
            CheckTy::Array(inner) => {
                let leaves = self.leaves_in(inner, deeper).map_err(|e| {
                    if e == "recursive-type" {
                        e
                    } else {
                        "array-element"
                    }
                })?;
                if leaves.iter().any(|(_, t)| Elem::of(*t).is_none()) {
                    return Err("array-element");
                }
                Ok(leaves
                    .into_iter()
                    .map(|(path, _)| (path, Ty::Owned))
                    .collect())
            }
            // A string, a map or a set is one handle, wherever it is held.
            CheckTy::String | CheckTy::Map(..) | CheckTy::Set(_) => {
                Ok(vec![(String::new(), Ty::Owned)])
            }
            // A tag, 0 for `Ok`, then the data of `Ok`, then that of `Err`.
            CheckTy::Result(ok, err) => {
                let mut out = vec![("tag".to_owned(), Ty::I32)];
                for (side, ty) in [("ok", ok), ("err", err)] {
                    for (path, leaf) in self.leaves_in(ty, deeper)? {
                        let path = if path.is_empty() {
                            side.to_owned()
                        } else {
                            format!("{side}_{path}")
                        };
                        out.push((path, leaf));
                    }
                }
                Ok(out)
            }
            // A tag, then the scalars of every variant's data, in order.
            CheckTy::Enum(id, _) => {
                let mut out = vec![("tag".to_owned(), Ty::I32)];
                let (_, variants) = self.analysis.enums.get(id.0).ok_or("enum")?;
                for (variant, fields) in variants {
                    for (k, field) in fields.iter().enumerate() {
                        for (path, leaf) in self.leaves_in(field, deeper)? {
                            let path = if path.is_empty() {
                                format!("{variant}_{k}")
                            } else {
                                format!("{variant}_{k}_{path}")
                            };
                            out.push((path, leaf));
                        }
                    }
                }
                Ok(out)
            }
            CheckTy::Option(inner) => {
                let mut out = vec![("some".to_owned(), Ty::Bool)];
                for (path, leaf) in self.leaves_in(inner, deeper)? {
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
        let written = self
            .analysis
            .written
            .get(&(ty.span.start, ty.span.end))
            .ok_or("struct")?;
        let resolved = self.analysis.subst(written, &self.args);
        compiled(&resolved)?;
        Ok(resolved)
    }

    /// How JavaScript sees a value of this type, as a result.
    pub(super) fn shape(&self, ty: &CheckTy) -> Shape {
        self.shape_passed(ty, Pass::Result)
    }

    /// How JavaScript sees a value of this type, passed this way.
    pub(super) fn shape_passed(&self, ty: &CheckTy, pass: Pass) -> Shape {
        self.shape_in(ty, pass, 0)
    }

    /// [`Self::shape_passed`], `depth` types deep; a type that contains
    /// itself stops, its leaves being reported as not compiled.
    fn shape_in(&self, ty: &CheckTy, pass: Pass, depth: usize) -> Shape {
        if depth > 64 {
            return Shape::Value(Ty::Void);
        }
        let deeper = depth + 1;
        match ty {
            CheckTy::Struct(id, name) => {
                let fields = self
                    .analysis
                    .structs
                    .get(id.0)
                    .map(|(_, fields)| {
                        fields
                            .iter()
                            .map(|(n, f)| (n.clone(), self.shape_in(f, pass, deeper)))
                            .collect()
                    })
                    .unwrap_or_default();
                // A tuple has no name: JavaScript sees an array.
                let name = if name.starts_with('(') {
                    String::new()
                } else {
                    super::identifier(name)
                };
                Shape::Object(name, fields)
            }
            CheckTy::Option(inner) => Shape::Option(Box::new(self.shape_in(inner, pass, deeper))),
            CheckTy::Result(ok, err) => Shape::Enum(
                String::new(),
                vec![
                    ("Ok".to_owned(), vec![self.shape_in(ok, pass, deeper)]),
                    ("Err".to_owned(), vec![self.shape_in(err, pass, deeper)]),
                ],
            ),
            CheckTy::Enum(id, name) => Shape::Enum(
                super::identifier(name),
                self.analysis
                    .enums
                    .get(id.0)
                    .map(|(_, variants)| {
                        variants
                            .iter()
                            .map(|(v, fields)| {
                                (
                                    v.clone(),
                                    fields
                                        .iter()
                                        .map(|f| self.shape_in(f, pass, deeper))
                                        .collect(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            CheckTy::String => Shape::String(pass),
            CheckTy::Map(key, value) => {
                let slots = self.map_slots(ty);
                Shape::Map(Box::new(crate::MapShape {
                    key: self.shape_in(key, pass, deeper),
                    value: Some(self.shape_in(value, pass, deeper)),
                    slots,
                    pass,
                }))
            }
            CheckTy::Set(key) => {
                let slots = self.map_slots(ty);
                Shape::Map(Box::new(crate::MapShape {
                    key: self.shape_in(key, pass, deeper),
                    value: None,
                    slots,
                    pass,
                }))
            }
            CheckTy::Array(inner) => match Self::ir_type(inner).and_then(Elem::of) {
                Some(elem) => Shape::Array(elem, pass),
                None => Shape::Records(Box::new(self.shape_in(inner, pass, deeper)), pass),
            },
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
    pub(super) fn check_type(&self, id: ExprId) -> Option<CheckTy> {
        self.analysis
            .types
            .get(&id)
            .map(|t| self.analysis.subst(t, &self.args))
    }

    /// Whether the checker typed an expression as a structure or an option.
    pub(super) fn is_aggregate(&self, id: ExprId) -> bool {
        self.check_type(id).as_ref().is_some_and(is_aggregate)
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
                } else if let Some(super::Binding::Lent(values)) = state.lookup(*symbol) {
                    values
                } else if let Some(globals) = self.globals.get(symbol).cloned() {
                    // A module variable: its scalars, read (decision 0023).
                    globals.into_iter().map(|g| self.global_expr(g)).collect()
                } else if let Some(&init) = self.constants.get(symbol) {
                    // A constant: its value, made here.
                    self.value_as(state, init, mode)
                } else if self.ast.name(*symbol) == "None" {
                    self.none(id)
                } else {
                    self.unsupported(span, "global");
                    Vec::new()
                }
            }
            AstExpr::Call { callee, args, .. } => self.call_values(state, id, *callee, args, mode),
            AstExpr::Try(inner) => self.try_values(state, *inner, span),
            AstExpr::String(text) => self.string_literal(state, text, mode),
            AstExpr::Template { parts, exprs } => self.template(state, parts, exprs, mode),
            AstExpr::Binary {
                op: perasc_syntax::ast::BinaryOp::Add,
                lhs,
                rhs,
            } => self.concat(state, *lhs, *rhs, mode),
            AstExpr::Array(items) => {
                let built = self.array_literal(state, id, items);
                Self::fresh(state, built, mode)
            }
            // A tuple is a structure: its scalars are its elements'.
            AstExpr::Tuple(items) => {
                let values = self.arguments(state, None, items);
                Self::fresh(state, values, mode)
            }
            AstExpr::Index { base, index } => self.index_values(state, *base, *index, span),
            AstExpr::Field { base, name } if self.enum_value_named(state, *base) => {
                let variant = self.ast.name(name.symbol);
                let values = self
                    .enum_value(state, id, *base, variant, &[])
                    .unwrap_or_default();
                Self::fresh(state, values, mode)
            }
            AstExpr::Match { scrutinee, arms } => {
                self.match_values(state, id, *scrutinee, arms, mode)
            }
            AstExpr::Field { base, name } => {
                // A field is read from its structure, which keeps it.
                let fields = self.value_as(state, *base, Mode::Borrow);
                let base_ty = self.check_type(*base);
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

    /// A call, as the scalars of its value: a function's results, `Some`,
    /// `Ok`, `Err`, a constructor, a variant or a method.
    fn call_values(
        &mut self,
        state: &mut FnState,
        id: ExprId,
        callee: ExprId,
        args: &[ExprId],
        mode: Mode,
    ) -> Vec<Expr> {
        let span = self.ast.expr(id).span;
        match self.ast.expr(callee).kind {
            AstExpr::Name(symbol) => {
                let function = self.callee(id, symbol);
                let back = function
                    .map(|f| self.writeback_places(state, f, args))
                    .unwrap_or_default();
                let args = self.arguments(state, function, args);
                let values = if let Some(function) = function {
                    self.call_results(state, function, args, &back)
                } else if self.ast.name(symbol) == "Some" {
                    let mut out = vec![bool_const(true)];
                    out.extend(args);
                    out
                } else if let (name @ ("Ok" | "Err"), Some(CheckTy::Result(ok, err))) =
                    (self.ast.name(symbol), self.check_type(id))
                {
                    // The tag, then `Ok`'s data, then `Err`'s.
                    let is_ok = name == "Ok";
                    let mut out = vec![Expr {
                        kind: ExprKind::Const(Const::Int(i64::from(!is_ok))),
                        ty: Ty::I32,
                    }];
                    if is_ok {
                        out.extend(args);
                        out.extend(self.zero_value(&err));
                    } else {
                        out.extend(self.zero_value(&ok));
                        out.extend(args);
                    }
                    out
                } else {
                    // A constructor: its scalars are its arguments', in order.
                    args
                };
                Self::fresh(state, values, mode)
            }
            AstExpr::Field { base, .. } if self.method_callee(id).is_some() => {
                let (function, receiver) =
                    self.method_callee(id).unwrap_or((crate::FnId(0), false));
                let all = Self::method_arguments(base, receiver, args);
                let back = self.writeback_places(state, function, &all);
                let args = self.arguments(state, Some(function), &all);
                let values = self.call_results(state, function, args, &back);
                Self::fresh(state, values, mode)
            }
            AstExpr::Field { base, name } => {
                let method = self.ast.name(name.symbol);
                if let Some(values) = self.map_new(state, id, base, method, mode) {
                    return values;
                }
                if let Some(values) = self.enum_value(state, id, base, method, args) {
                    return Self::fresh(state, values, mode);
                }
                self.method_values(state, base, method, args, mode, span)
            }
            _ => {
                self.unsupported(span, "function-value");
                Vec::new()
            }
        }
    }

    /// `None`, of the type the checker gave it: `false`, then zeros.
    fn none(&self, id: ExprId) -> Vec<Expr> {
        self.check_type(id)
            .map(|t| self.zero_value(&t))
            .unwrap_or_default()
    }

    /// The value every scalar of a type holds when nothing is there: zero,
    /// `false`, and an empty buffer for each string or array, so that an
    /// option or an enumeration always owns its buffers, whatever it holds.
    pub(super) fn zero_value(&self, ty: &CheckTy) -> Vec<Expr> {
        match ty {
            CheckTy::String | CheckTy::Array(_) => self
                .columns(Some(ty))
                .unwrap_or_default()
                .into_iter()
                .map(|elem| Expr {
                    kind: ExprKind::Buf(
                        crate::BufOp::New(elem),
                        vec![Expr {
                            kind: ExprKind::Const(Const::Int(0)),
                            ty: Ty::I32,
                        }],
                    ),
                    ty: Ty::Owned,
                })
                .collect(),
            CheckTy::Map(..) | CheckTy::Set(_) => self.empty_map(ty).into_iter().collect(),
            CheckTy::Struct(id, _) => self
                .analysis
                .structs
                .get(id.0)
                .map(|(_, fields)| {
                    fields
                        .iter()
                        .flat_map(|(_, f)| self.zero_value(f))
                        .collect()
                })
                .unwrap_or_default(),
            CheckTy::Option(inner) => {
                let mut out = vec![bool_const(false)];
                out.extend(self.zero_value(inner));
                out
            }
            CheckTy::Result(ok, err) => {
                let mut out = vec![Expr {
                    kind: ExprKind::Const(Const::Int(0)),
                    ty: Ty::I32,
                }];
                out.extend(self.zero_value(ok));
                out.extend(self.zero_value(err));
                out
            }
            CheckTy::Enum(id, _) => {
                let mut out = vec![Expr {
                    kind: ExprKind::Const(Const::Int(0)),
                    ty: Ty::I32,
                }];
                if let Some((_, variants)) = self.analysis.enums.get(id.0) {
                    for (_, fields) in variants {
                        for field in fields {
                            out.extend(self.zero_value(field));
                        }
                    }
                }
                out
            }
            other => vec![super::zero(Self::ir_type(other).unwrap_or(Ty::I32))],
        }
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
        if self.is_map(base) {
            return self.map_method(state, base, method, args, mode, span);
        }
        if !self.is_aggregate(base) {
            if method == "toString" {
                return self.stringified(state, base, mode);
            }
            if method == "toFixed" {
                return self.fixed(state, base, args, mode);
            }
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
        // A structure's copy: each buffer copied, with those it owns
        // (decision 0021, finding F102).
        if method == "copy" {
            let copies = self
                .value_as(state, base, Mode::Borrow)
                .into_iter()
                .map(|value| {
                    if value.ty == Ty::Borrowed {
                        Expr {
                            kind: ExprKind::Buf(crate::BufOp::Clone, vec![value]),
                            ty: Ty::Owned,
                        }
                    } else {
                        value
                    }
                })
                .collect();
            return Self::fresh(state, copies, mode);
        }
        if method == "unwrapOr"
            && let Some(values) = self.map_get_or(state, base, args, mode)
        {
            return values;
        }
        let fields = self.value_as(state, base, Mode::Borrow);
        // A result is read as an option of its `Ok` data: present when the tag
        // is 0.
        let fields = match self.check_type(base) {
            Some(CheckTy::Result(ok, _)) => {
                let ok_width = self.leaves(&ok).map_or(0, |l| l.len());
                let fields: Vec<Expr> = fields.into_iter().map(|e| state.spill(e)).collect();
                let tag = fields.first().cloned().unwrap_or_else(|| bool_const(false));
                let mut view = vec![super::binary(
                    BinOp::Eq,
                    tag,
                    Expr {
                        kind: ExprKind::Const(Const::Int(0)),
                        ty: Ty::I32,
                    },
                )];
                view.extend(fields.into_iter().skip(1).take(ok_width));
                view
            }
            _ => fields,
        };
        match method {
            "isSome" | "isNone" | "isOk" | "isErr" => {
                let some = Self::pick(state, fields, 0, 1);
                let Some(some) = some.into_iter().next() else {
                    return Vec::new();
                };
                if matches!(method, "isSome" | "isOk") {
                    vec![some]
                } else {
                    vec![Expr {
                        kind: ExprKind::Unary(UnOp::Not, Box::new(some)),
                        ty: Ty::Bool,
                    }]
                }
            }
            "unwrapOr" => self.unwrap_or_values(state, fields, args, mode),
            _ => {
                self.unsupported(span, "method");
                Vec::new()
            }
        }
    }

    /// `option.unwrapOr(fallback)` on the scalars `fields` of the option.
    fn unwrap_or_values(
        &mut self,
        state: &mut FnState,
        fields: Vec<Expr>,
        args: &[ExprId],
        mode: Mode,
    ) -> Vec<Expr> {
        // The receiver, then the fallback; then the value is chosen.
        let mark = state.pre.len();
        let drops = state.drops.len();
        let fallback = self.arguments(state, None, args);
        if Self::quiet_since(state, mark, drops, &fallback) {
            let making = state.pre.split_off(mark);
            return Self::unwrap_or_lazily(state, fields, fallback, making, true, mode);
        }
        Self::unwrap_or_eagerly(state, fields, fallback, mark, mode)
    }

    /// Whether the fallback lowered since `mark`, with no drop registered
    /// since `drops`, is quiet: a fallback that only makes literals and
    /// copies, and reads values, cannot be seen to run, so it is made only
    /// when the option is empty (decisions 0016 and 0017, finding F209).
    /// Anything else runs first, as written.
    pub(super) fn quiet_since(
        state: &FnState,
        mark: usize,
        drops: usize,
        fallback: &[Expr],
    ) -> bool {
        state.drops.len() == drops
            && state.pre[mark..]
                .iter()
                .all(|s| matches!(s, Stmt::Set(_, e) if quiet_value(e)))
            && fallback.iter().all(quiet_value)
    }

    /// `unwrapOr` whose fallback, lowered since `mark`, runs first: the
    /// value when there is one, else the fallback.
    pub(super) fn unwrap_or_eagerly(
        state: &mut FnState,
        mut fields: Vec<Expr>,
        fallback: Vec<Expr>,
        mark: usize,
        mode: Mode,
    ) -> Vec<Expr> {
        state.keep_before(mark, &mut fields);
        let fields: Vec<Expr> = fields.into_iter().map(|e| state.spill(e)).collect();
        let Some((some, payload)) = fields.split_first() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut then = Vec::new();
        for (value, fallback) in payload.iter().zip(fallback) {
            // A fallback read from a variable that keeps it: a copy.
            let fallback = if fallback.ty == Ty::Borrowed {
                Expr {
                    kind: ExprKind::Buf(crate::BufOp::Clone, vec![fallback]),
                    ty: Ty::Owned,
                }
            } else {
                fallback
            };
            if fallback.ty == Ty::Owned {
                // A buffer: the fallback, or a copy of the one inside.
                let temp = state.owned_temp(fallback);
                then.push(heap::release_stmt(temp));
                then.push(Stmt::Set(
                    temp,
                    Expr {
                        kind: ExprKind::Buf(crate::BufOp::Clone, vec![value.clone()]),
                        ty: Ty::Owned,
                    },
                ));
                out.push(Expr {
                    kind: ExprKind::Local(temp),
                    ty: Ty::Owned,
                });
                continue;
            }
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
        Self::fresh(state, out, mode)
    }

    /// `unwrapOr` whose fallback, made by the statements `making`, is quiet:
    /// the value when there is one, else the fallback, made only then. With
    /// `spill`, the value is evaluated before the choice; without, it is read
    /// only when there is one, as the entry of a map that `get` found.
    pub(super) fn unwrap_or_lazily(
        state: &mut FnState,
        fields: Vec<Expr>,
        fallback: Vec<Expr>,
        making: Vec<Stmt>,
        spill: bool,
        mode: Mode,
    ) -> Vec<Expr> {
        let fields: Vec<Expr> = if spill {
            fields.into_iter().map(|e| state.spill(e)).collect()
        } else {
            fields
        };
        let Some((some, payload)) = fields.split_first() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut then = Vec::new();
        let mut otherwise = making;
        for (value, fallback) in payload.iter().zip(fallback) {
            // A fallback read from a variable that keeps it: a copy.
            let fallback = if fallback.ty == Ty::Borrowed {
                Expr {
                    kind: ExprKind::Buf(crate::BufOp::Clone, vec![fallback]),
                    ty: Ty::Owned,
                }
            } else {
                fallback
            };
            if fallback.ty == Ty::Owned {
                // A buffer: a copy of the one inside, or the fallback.
                let temp = state.temp(Ty::Owned);
                state.live.insert(temp.0);
                then.push(Stmt::Set(
                    temp,
                    Expr {
                        kind: ExprKind::Buf(crate::BufOp::Clone, vec![value.clone()]),
                        ty: Ty::Owned,
                    },
                ));
                otherwise.push(Stmt::Set(temp, fallback));
                out.push(Expr {
                    kind: ExprKind::Local(temp),
                    ty: Ty::Owned,
                });
                continue;
            }
            let temp = state.temp(fallback.ty);
            then.push(Stmt::Set(temp, value.clone()));
            otherwise.push(Stmt::Set(temp, fallback));
            out.push(super::local_expr(state, temp));
        }
        state.pre.push(Stmt::If {
            cond: some.clone(),
            then,
            otherwise,
        });
        Self::fresh(state, out, mode)
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
        // Buffers go to temporaries: the `None` path releases them, the other
        // takes them.
        let fields: Vec<Expr> = self
            .value(state, inner)
            .into_iter()
            .map(|e| {
                if e.ty == Ty::Owned {
                    let temp = state.owned_temp(e);
                    Expr {
                        kind: ExprKind::Local(temp),
                        ty: Ty::Owned,
                    }
                } else {
                    state.spill(e)
                }
            })
            .collect();
        if let Some(CheckTy::Result(ok, _)) = self.check_type(inner) {
            return self.try_result(state, &ret, &ok, &fields);
        }
        let Some((some, payload)) = fields.split_first() else {
            return Vec::new();
        };
        // `None`, and the `inout` numbers as they are; what the function holds
        // is released first.
        let mut none: Vec<Expr> = self.zero_value(&ret);
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
        Self::fresh(state, payload.to_vec(), Mode::Consume)
    }

    /// `x?` on a result: its `Ok` data, or a return of its error, moved into
    /// the function's own result.
    fn try_result(
        &self,
        state: &mut FnState,
        ret: &CheckTy,
        ok: &CheckTy,
        fields: &[Expr],
    ) -> Vec<Expr> {
        let ok_width = self.leaves(ok).map_or(0, |l| l.len());
        let Some((tag, data)) = fields.split_first() else {
            return Vec::new();
        };
        let payload: Vec<Expr> = data.iter().take(ok_width).cloned().collect();
        let error: Vec<Expr> = data.iter().skip(ok_width).cloned().collect();
        // The error's buffers move into the return; everything else is released.
        let moved: BTreeSet<u32> = error
            .iter()
            .filter_map(|e| match e.kind {
                ExprKind::Local(l) if e.ty == Ty::Owned => Some(l.0),
                _ => None,
            })
            .collect();
        let CheckTy::Result(ret_ok, _) = ret else {
            return Vec::new();
        };
        let mut early = vec![Expr {
            kind: ExprKind::Const(Const::Int(1)),
            ty: Ty::I32,
        }];
        early.extend(self.zero_value(ret_ok));
        early.extend(error.iter().map(|e| match e.kind {
            ExprKind::Local(l) if e.ty == Ty::Owned => Expr {
                kind: ExprKind::Move(l),
                ty: Ty::Owned,
            },
            _ => e.clone(),
        }));
        early.extend(state.writeback_values());
        let mut then: Vec<Stmt> = state
            .live
            .iter()
            .filter(|l| !moved.contains(l))
            .map(|&l| heap::release_stmt(LocalId(l)))
            .collect();
        then.push(super::return_of(early));
        state.pre.push(Stmt::If {
            cond: super::binary(
                BinOp::Ne,
                tag.clone(),
                Expr {
                    kind: ExprKind::Const(Const::Int(0)),
                    ty: Ty::I32,
                },
            ),
            then,
            otherwise: Vec::new(),
        });
        // On the `Ok` path, the error's empty buffers are released.
        for &local in &moved {
            let release = state.release(LocalId(local));
            state.pre.push(release);
        }
        Self::fresh(state, payload, Mode::Consume)
    }

    /// `len` scalars from `offset`. The others are dropped, so any of them that
    /// could call or trap is evaluated first, with all the others, in order;
    /// a block among them, the field of a temporary, is released when the
    /// statement ends.
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
        let mut picked = Vec::with_capacity(len);
        for (i, field) in fields.into_iter().enumerate() {
            if (offset..offset + len).contains(&i) {
                picked.push(field);
            } else if field.ty == Ty::Owned {
                let _ = Self::fresh(state, vec![field], Mode::Borrow);
            }
        }
        picked
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

/// A value whose making cannot be seen: pure, a literal or an empty
/// buffer made, or a block moved out of a temporary.
/// A value whose making cannot be seen: a read, a literal, a new buffer, or a
/// copy of a value read (decision 0017, step 1). Each only allocates, which a
/// program cannot observe, and reads what is not changed before the choice.
fn quiet_value(e: &Expr) -> bool {
    heap::pure(e)
        || matches!(
            &e.kind,
            ExprKind::Buf(
                crate::BufOp::Literal(_) | crate::BufOp::New(_) | crate::BufOp::Clone,
                args
            ) if args.iter().all(heap::pure)
        )
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
                // A block made before the statements that follow is moved
                // into its place when the statement completes.
                let read = if operand.ty == Ty::Owned {
                    Expr {
                        kind: ExprKind::Move(temp),
                        ty: Ty::Owned,
                    }
                } else {
                    super::local_expr(self, temp)
                };
                let value = std::mem::replace(operand, read);
                self.pre.insert(at, Stmt::Set(temp, value));
                at += 1;
            }
        }
        // Until then, the blocks the operands hold are released on the exits
        // of the statements that follow, as the return of a `?`.
        for operand in operands.iter() {
            if let ExprKind::Move(temp) = operand.kind {
                release_before_returns(&mut self.pre, at, temp);
            }
        }
    }
}

/// Structures, options and arrays: values lowered through [`Lowering::value`].
pub(super) const fn is_aggregate(ty: &CheckTy) -> bool {
    matches!(
        ty,
        CheckTy::Struct(..)
            | CheckTy::Option(_)
            | CheckTy::Array(_)
            | CheckTy::String
            | CheckTy::Enum(..)
            | CheckTy::Result(..)
            | CheckTy::Map(..)
            | CheckTy::Set(_)
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
pub(super) fn checked(state: &mut FnState, op: BinOp, a: Expr, b: Expr) -> Vec<Expr> {
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
        ExprKind::Const(_) | ExprKind::Global(_) => false,
        ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Math(_, x)
        | ExprKind::Alloc(x) => reads(x, local),
        ExprKind::Binary(_, a, b) => reads(a, local) || reads(b, local),
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => args.iter().any(|a| reads(a, local)),
    }
}

/// Whether the backends compile values of this type; the feature to report
/// when they do not.
fn compiled(ty: &CheckTy) -> Result<(), &'static str> {
    match ty {
        CheckTy::Int(IntTy::I32 | IntTy::I64)
        | CheckTy::Float(_)
        | CheckTy::Bool
        | CheckTy::String
        | CheckTy::Void
        | CheckTy::Struct(..)
        | CheckTy::Enum(..)
        | CheckTy::Map(..)
        | CheckTy::Set(_) => Ok(()),
        CheckTy::Array(inner) | CheckTy::Option(inner) => compiled(inner),
        CheckTy::Result(ok, err) => compiled(ok).and(compiled(err)),
        CheckTy::Int(_) => Err("integer-type"),
        _ => Err("struct"),
    }
}

/// Releases `temp` before each `return` of `stmts` from `from` on, nested ones
/// included: the early exits, as `?` makes, of a statement that holds a block
/// it has not yet handed over.
fn release_before_returns(stmts: &mut Vec<Stmt>, from: usize, temp: LocalId) {
    let mut i = from;
    while i < stmts.len() {
        match &mut stmts[i] {
            Stmt::Return(_) | Stmt::ReturnMany(_) => {
                stmts.insert(i, heap::release_stmt(temp));
                i += 1;
            }
            Stmt::If {
                then, otherwise, ..
            } => {
                release_before_returns(then, 0, temp);
                release_before_returns(otherwise, 0, temp);
            }
            Stmt::Loop { body, step, .. } => {
                release_before_returns(body, 0, temp);
                release_before_returns(step, 0, temp);
            }
            Stmt::Block(inner) => release_before_returns(inner, 0, temp),
            _ => {}
        }
        i += 1;
    }
}
