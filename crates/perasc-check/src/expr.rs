//! Expressions: literals, names, operators, calls, fields and indexing.
//!
//! Typing is bidirectional: the expected type, when known, flows into the
//! expression, so `let x: f32 = 0.5` types the literal as `f32`. A literal whose
//! type nothing decides is `{integer}` or `{float}` until it meets one, and
//! defaults to `i32` or `f64`. No conversion is ever implicit: mixing `i32` and
//! `f64` is an error whose fix inserts the explicit `.toF64()`.

use perasc_diag::Span;
use perasc_syntax::ast::{BinaryOp, ExprId, ExprKind, Symbol, Type, UnaryOp};

use crate::errors::{self, Conversion};
use crate::generics::GenericUse;
use crate::scope::{self, Kind};
use crate::ty::{Bound, IntTy, Ty};
use crate::{Checker, builtins};

impl Checker<'_> {
    /// Checks an expression against an expected type, reporting a mismatch.
    pub(crate) fn expect(&mut self, id: ExprId, expected: &Ty) {
        let actual = self.infer(id, Some(expected));
        self.coerce(id, &actual, expected);
    }

    /// Whether `actual` can be used where `expected` is, reporting why not.
    pub(crate) fn coerce(&mut self, id: ExprId, actual: &Ty, expected: &Ty) -> bool {
        if actual == expected || matches!(actual, Ty::Error) || matches!(expected, Ty::Error) {
            return true;
        }
        let span = self.ast.expr(id).span;
        match (actual, expected) {
            (Ty::IntLit, Ty::Int(int)) => {
                self.range_check(id, *int);
                self.settle(id, expected);
                true
            }
            (Ty::FloatLit, Ty::Float(_)) => {
                self.settle(id, expected);
                true
            }
            (Ty::IntLit, Ty::Float(_) | Ty::FloatLit) if self.literal(id).is_some() => {
                let literal = self.slice(span).to_owned();
                self.report(errors::integer_for_float(span, &literal, expected));
                false
            }
            // `None` and `[]` take the type the context expects: code generation
            // needs to know what they hold.
            (Ty::Option(inner), Ty::Option(_)) | (Ty::Array(inner), Ty::Array(_))
                if **inner == Ty::Error =>
            {
                self.fill_in(id, expected);
                true
            }
            // `Ok(x)` and `Err(e)` know only one side: the context gives the other.
            (Ty::Result(ok, err), Ty::Result(want_ok, want_err))
                if (**ok == Ty::Error || ok == want_ok)
                    && (**err == Ty::Error || err == want_err) =>
            {
                self.fill_in(id, expected);
                true
            }
            _ => {
                let conversion = self.conversion(id, actual, expected);
                self.report(errors::mismatch(span, expected, actual, conversion));
                false
            }
        }
    }

    /// The explicit conversion turning a number of type `from` into `to`.
    fn conversion(&self, id: ExprId, from: &Ty, to: &Ty) -> Option<Conversion> {
        let method = to.conversion()?;
        if !matches!(from, Ty::Int(_) | Ty::Float(_)) {
            return None;
        }
        let expr = self.ast.expr(id);
        let parenthesize = matches!(expr.kind, ExprKind::Binary { .. } | ExprKind::Unary { .. });
        Some(Conversion {
            span: expr.span,
            method,
            parenthesize,
        })
    }

    /// The value of an integer literal, possibly negated and parenthesized.
    fn literal(&self, id: ExprId) -> Option<(u64, bool)> {
        match &self.ast.expr(id).kind {
            ExprKind::Int(value) => Some((*value, false)),
            ExprKind::Paren(inner) => self.literal(*inner),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } => match self.ast.expr(*operand).kind {
                ExprKind::Int(value) => Some((value, true)),
                _ => None,
            },
            _ => None,
        }
    }

    /// `E0212` when an integer literal does not fit the type it takes.
    fn range_check(&mut self, id: ExprId, int: IntTy) {
        let Some((value, negative)) = self.literal(id) else {
            return;
        };
        let fits = if negative {
            value <= int.min_magnitude()
        } else {
            value <= int.max()
        };
        if fits {
            return;
        }
        let span = self.ast.expr(id).span;
        let literal = self.slice(span).to_owned();
        let min = if int.signed() {
            format!("-{}", int.min_magnitude())
        } else {
            "0".to_owned()
        };
        let max = int.max().to_string();
        self.report(errors::out_of_range(
            span,
            &literal,
            &Ty::Int(int),
            &min,
            &max,
        ));
    }

    /// The type of an expression; `expected` guides literals, arrays and `Some`.
    pub(crate) fn infer(&mut self, id: ExprId, expected: Option<&Ty>) -> Ty {
        let ty = self.infer_inner(id, expected);
        self.expr_types.insert(id, ty.clone());
        ty
    }

    /// Records the type the context expects for `None` or `[]`, through parentheses.
    fn fill_in(&mut self, id: ExprId, ty: &Ty) {
        self.expr_types.insert(id, ty.clone());
        if let ExprKind::Paren(inner) = self.ast.expr(id).kind {
            self.fill_in(inner, ty);
        }
    }

    /// Gives a literal expression the concrete type the context chose for it, down
    /// to the literals it is made of: in `x + (1 + 2)` with `x: i64`, all are `i64`.
    fn settle(&mut self, id: ExprId, ty: &Ty) {
        if !matches!(self.expr_types.get(&id), Some(Ty::IntLit | Ty::FloatLit)) {
            return;
        }
        self.expr_types.insert(id, ty.clone());
        match self.ast.expr(id).kind {
            ExprKind::Paren(inner) => self.settle(inner, ty),
            ExprKind::Unary { op, operand } if op != UnaryOp::Not => self.settle(operand, ty),
            ExprKind::Binary { op, lhs, rhs }
                if !op.is_comparison() && !matches!(op, BinaryOp::And | BinaryOp::Or) =>
            {
                self.settle(lhs, ty);
                self.settle(rhs, ty);
            }
            _ => {}
        }
    }

    fn infer_inner(&mut self, id: ExprId, expected: Option<&Ty>) -> Ty {
        let expr = self.ast.expr(id);
        let span = expr.span;
        match &expr.kind {
            ExprKind::Int(_) => Ty::IntLit,
            ExprKind::Float(_) => Ty::FloatLit,
            ExprKind::String(_) => Ty::String,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Array(items) => self.array(items, expected),
            ExprKind::Tuple(items) => self.tuple_expr(items, expected),
            ExprKind::Name(symbol) => {
                if let Some(index) = self.scopes.lookup(*symbol) {
                    self.resolved.insert(id, index);
                }
                self.read_name(*symbol, span)
            }
            ExprKind::Paren(inner) => self.infer(*inner, expected),
            ExprKind::Unary { op, operand } => self.unary(*op, *operand, span, expected),
            ExprKind::Binary { op, lhs, rhs } => self.binary(*op, *lhs, *rhs, span, expected),
            ExprKind::Call {
                callee,
                types,
                args,
            } => self.call(id, *callee, types, args, span, expected),
            ExprKind::Field { base, name } => {
                if let Some(ty) = self.enum_variant(*base, name, None, span, expected) {
                    return ty;
                }
                let base_ty = self.infer(*base, None);
                self.field(&base_ty, name.symbol, name.span)
            }
            ExprKind::Match { scrutinee, arms } => {
                self.match_expr(*scrutinee, arms, span, expected)
            }
            ExprKind::Index { base, index } => {
                let base_ty = self.infer(*base, None);
                self.expect_integer(*index);
                match base_ty {
                    Ty::Array(element) => *element,
                    Ty::Error => Ty::Error,
                    other => {
                        self.report(errors::invalid_operator(span, "[]", &other));
                        Ty::Error
                    }
                }
            }
            ExprKind::Try(inner) => match self.infer(*inner, None) {
                Ty::Option(value) => {
                    self.check_try(span, &Ty::Option(Box::new(Ty::Error)));
                    *value
                }
                Ty::Result(value, error) => {
                    self.check_try(span, &Ty::Result(Box::new(Ty::Error), error));
                    *value
                }
                Ty::Error => Ty::Error,
                other => {
                    self.report(errors::invalid_operator(span, "?", &other));
                    Ty::Error
                }
            },
            ExprKind::Error => Ty::Error,
        }
    }

    /// `E0225` unless the function returns what `?` returns early: an option
    /// for an option, a result with the same error for a result.
    fn check_try(&mut self, span: Span, early: &Ty) {
        let ret = self.ret.as_ref().map(|(t, _)| t.clone());
        let fits = match (&ret, early) {
            (Some(Ty::Option(_)), Ty::Option(_)) | (Some(Ty::Error) | None, _) => true,
            (Some(Ty::Result(_, have)), Ty::Result(_, want)) => {
                have == want || **have == Ty::Error || **want == Ty::Error
            }
            _ => false,
        };
        if !fits {
            self.report(errors::try_return(span, early, ret.as_ref()));
        }
    }

    /// An index must be an integer.
    fn expect_integer(&mut self, index: ExprId) {
        let ty = self.infer(index, Some(&Ty::Int(IntTy::I32)));
        if !ty.is_integer() && ty != Ty::Error {
            let span = self.ast.expr(index).span;
            let conversion = self.conversion(index, &ty, &Ty::Int(IntTy::I32));
            self.report(errors::mismatch(
                span,
                &Ty::Int(IntTy::I32),
                &ty,
                conversion,
            ));
        }
    }

    /// A name read as a value: marks it used.
    fn read_name(&mut self, symbol: Symbol, span: Span) -> Ty {
        let Some(index) = self.scopes.lookup(symbol) else {
            let name = self.name(symbol);
            let suggestion = scope::suggest(name, self.scopes.visible_names()).map(str::to_owned);
            // The suggested name was probably meant: count it as read, so that the
            // typo does not also make it "never read".
            if let Some(meant) = suggestion
                .as_deref()
                .and_then(|s| self.scopes.lookup_name(s))
                && let Some(binding) = self.scopes.bindings.get_mut(meant)
            {
                binding.used = true;
            }
            self.report(errors::undefined(span, name, suggestion.as_deref()));
            return Ty::Error;
        };
        let Some(binding) = self.scopes.bindings.get_mut(index) else {
            return Ty::Error;
        };
        binding.used = true;
        match binding.kind {
            // A structure or `Some` alone is not a value: it must be called.
            Kind::Struct(_) | Kind::Some | Kind::Ok | Kind::Err => Ty::Error,
            _ => binding.ty.clone(),
        }
    }

    fn array(&mut self, items: &[ExprId], expected: Option<&Ty>) -> Ty {
        let expected_element = match expected {
            Some(Ty::Array(element)) => Some((**element).clone()),
            _ => None,
        };
        let mut element: Option<Ty> = expected_element.clone();
        for &item in items {
            match element.clone() {
                Some(e) => {
                    let actual = self.infer(item, Some(&e));
                    let merged = self.merge(item, &actual, &e);
                    element = Some(merged);
                }
                None => element = Some(self.infer(item, None)),
            }
        }
        Ty::Array(Box::new(element.map_or(Ty::Error, Ty::defaulted)))
    }

    /// The common type of an element and the array so far.
    fn merge(&mut self, item: ExprId, actual: &Ty, so_far: &Ty) -> Ty {
        match (actual, so_far) {
            (Ty::Int(_) | Ty::Float(_), Ty::IntLit | Ty::FloatLit)
                if actual.is_float() == so_far.is_float() =>
            {
                actual.clone()
            }
            _ => {
                self.coerce(item, actual, so_far);
                so_far.clone()
            }
        }
    }

    fn unary(&mut self, op: UnaryOp, operand: ExprId, span: Span, expected: Option<&Ty>) -> Ty {
        match op {
            UnaryOp::Neg => {
                let ty = self.infer(operand, expected);
                let unsigned = matches!(ty, Ty::Int(int) if !int.signed());
                // A parameter may stand for an unsigned integer: only a
                // `Float` one can be negated.
                let negatable = match &ty {
                    Ty::Param(..) => self.satisfies(&ty, Bound::Float),
                    _ => ty.is_numeric() && !unsigned,
                };
                if negatable || ty == Ty::Error {
                    ty
                } else {
                    self.invalid_operator(span, "-", &ty, Bound::Float);
                    Ty::Error
                }
            }
            UnaryOp::Not => {
                let ty = self.infer(operand, Some(&Ty::Bool));
                if !matches!(ty, Ty::Bool | Ty::Error) {
                    self.report(errors::invalid_operator(span, "!", &ty));
                }
                Ty::Bool
            }
            UnaryOp::BitNot => {
                let ty = self.infer(operand, expected);
                if ty.is_integer() || ty == Ty::Error || self.satisfies_param(&ty, Bound::Integer) {
                    ty
                } else {
                    self.invalid_operator(span, "~", &ty, Bound::Integer);
                    Ty::Error
                }
            }
            UnaryOp::Borrow => self.infer(operand, expected),
        }
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
        span: Span,
        expected: Option<&Ty>,
    ) -> Ty {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            for side in [lhs, rhs] {
                let ty = self.infer(side, Some(&Ty::Bool));
                if !matches!(ty, Ty::Bool | Ty::Error) {
                    let side_span = self.ast.expr(side).span;
                    self.report(errors::invalid_operator(side_span, op.symbol(), &ty));
                }
            }
            return Ty::Bool;
        }
        // Operands of a comparison are guided by each other, not by the `bool` result.
        let operand_hint = if op.is_comparison() { None } else { expected };
        let lt = self.infer(lhs, operand_hint);
        let rt = self.infer(rhs, operand_hint.or(Some(&lt)));
        // An operand the operator cannot take at all: say so, rather than
        // "mismatched types" against the other operand.
        for (side, ty) in [(lhs, &lt), (rhs, &rt)] {
            if !self.operand_allowed(op, ty) {
                let side_span = self.ast.expr(side).span;
                self.invalid_operator(side_span, op.symbol(), ty, needed_bound(op));
                return if op.is_comparison() {
                    Ty::Bool
                } else {
                    Ty::Error
                };
            }
        }
        let Some(common) = self.unify(lhs, &lt, rhs, &rt) else {
            return Ty::Error;
        };
        let allowed = self.operand_allowed(op, &common);
        if !allowed && common != Ty::Error {
            self.invalid_operator(span, op.symbol(), &common, needed_bound(op));
            return Ty::Error;
        }
        if op.is_comparison() { Ty::Bool } else { common }
    }

    /// The common type of two operands, or `None` after reporting a mismatch.
    /// Mixed numbers are never converted silently: the fix converts toward the float,
    /// or the wider integer, as in `x.toF64() + y`.
    fn unify(&mut self, lhs: ExprId, lt: &Ty, rhs: ExprId, rt: &Ty) -> Option<Ty> {
        // An operand already reported makes the whole expression an error: nothing
        // computed from it is reported again.
        if matches!(lt, Ty::Error) || matches!(rt, Ty::Error) {
            return Some(Ty::Error);
        }
        if lt == rt {
            return Some(lt.clone());
        }
        match (lt, rt) {
            (Ty::IntLit | Ty::FloatLit, Ty::Int(_) | Ty::Float(_) | Ty::FloatLit) => {
                self.coerce(lhs, lt, rt).then(|| rt.clone())
            }
            (Ty::Int(_) | Ty::Float(_), Ty::IntLit | Ty::FloatLit) => {
                self.coerce(rhs, rt, lt).then(|| lt.clone())
            }
            (Ty::FloatLit, Ty::IntLit) => self.coerce(rhs, rt, lt).then(|| lt.clone()),
            (Ty::Int(a), Ty::Int(b)) => {
                // Convert the narrower one.
                let (narrow, narrow_ty, wide) = if a.max() < b.max() {
                    (lhs, lt, rt)
                } else {
                    (rhs, rt, lt)
                };
                self.coerce(narrow, narrow_ty, wide);
                None
            }
            (Ty::Int(_), Ty::Float(_)) => {
                self.coerce(lhs, lt, rt);
                None
            }
            // A float on the left, or anything else: convert the right operand.
            _ => {
                self.coerce(rhs, rt, lt);
                None
            }
        }
    }

    // --- Calls, fields and methods ------------------------------------------

    fn call(
        &mut self,
        id: ExprId,
        callee: ExprId,
        types: &[Type],
        args: &[ExprId],
        span: Span,
        expected: Option<&Ty>,
    ) -> Ty {
        let callee_expr = self.ast.expr(callee);
        match &callee_expr.kind {
            ExprKind::Name(symbol) => {
                if let Some(index) = self.scopes.lookup(*symbol) {
                    self.resolved.insert(callee, index);
                }
                let written: Vec<Ty> = types.iter().map(|t| self.resolve_type(t)).collect();
                let call = Call {
                    id,
                    name_span: callee_expr.span,
                    span,
                    types: (!types.is_empty()).then_some(written),
                };
                self.call_name(*symbol, &call, args, expected)
            }
            ExprKind::Field { base, name } => {
                let site = crate::methods::CallSite {
                    call: id,
                    args,
                    span,
                    expected,
                };
                if let Some(ty) = self.static_method(&site, *base, name) {
                    return ty;
                }
                if let Some(ty) = self.enum_variant(*base, name, Some(args), span, expected) {
                    return ty;
                }
                let receiver = self.infer(*base, None);
                if let Some(ty) = self.user_method(&site, *base, &receiver, name) {
                    return ty;
                }
                // `push` and `pop` change the array: it must be one that can change.
                if matches!(receiver, Ty::Array(_))
                    && matches!(self.name(name.symbol), "push" | "pop")
                    && let Some(root) = self.root(*base)
                {
                    let base_span = self.ast.expr(*base).span;
                    self.check_mutable(root, base_span);
                }
                self.method_call(&receiver, name.symbol, name.span, args, span)
            }
            _ => {
                let ty = self.infer(callee, None);
                self.check_args(args, &[]);
                if ty != Ty::Error {
                    self.report(errors::invalid_operator(callee_expr.span, "()", &ty));
                }
                Ty::Error
            }
        }
    }

    fn call_name(
        &mut self,
        symbol: Symbol,
        call: &Call,
        args: &[ExprId],
        expected: Option<&Ty>,
    ) -> Ty {
        let (name_span, span) = (call.name_span, call.span);
        let Some(index) = self.scopes.lookup(symbol) else {
            self.read_name(symbol, name_span);
            self.check_args(args, &[]);
            return Ty::Error;
        };
        let kind = match self.scopes.bindings.get_mut(index) {
            Some(binding) => {
                binding.used = true;
                binding.kind.clone()
            }
            None => return Ty::Error,
        };
        let name = self.name(symbol);
        match kind {
            Kind::Fn(id) => self.call_fn(id, name, call, args, expected),
            Kind::Struct(id) => {
                let declared = self.scopes.bindings.get(index).and_then(|b| b.span);
                self.construct(id, name, declared, call, args, expected)
            }
            Kind::Some => {
                self.arity(span, name, 1, args.len(), None);
                let inner = match expected {
                    Some(Ty::Option(inner)) => Some((**inner).clone()),
                    _ => None,
                };
                let ty = match (args.first(), inner) {
                    (Some(&arg), Some(inner)) => {
                        self.expect(arg, &inner);
                        inner
                    }
                    (Some(&arg), None) => self.infer(arg, None).defaulted(),
                    (None, _) => Ty::Error,
                };
                self.check_args(args.get(1..).unwrap_or(&[]), &[]);
                Ty::Option(Box::new(ty))
            }
            Kind::Ok | Kind::Err => {
                self.arity(span, name, 1, args.len(), None);
                let ok = kind == Kind::Ok;
                let (want_ok, want_err) = match expected {
                    Some(Ty::Result(o, e)) => (Some((**o).clone()), Some((**e).clone())),
                    _ => (None, None),
                };
                let side = if ok {
                    want_ok.clone()
                } else {
                    want_err.clone()
                };
                let given = match (args.first(), side) {
                    (Some(&arg), Some(side)) => {
                        self.expect(arg, &side);
                        side
                    }
                    (Some(&arg), None) => self.infer(arg, None).defaulted(),
                    (None, _) => Ty::Error,
                };
                self.check_args(args.get(1..).unwrap_or(&[]), &[]);
                if ok {
                    Ty::Result(Box::new(given), Box::new(want_err.unwrap_or(Ty::Error)))
                } else {
                    Ty::Result(Box::new(want_ok.unwrap_or(Ty::Error)), Box::new(given))
                }
            }
            _ => {
                let ty = self
                    .scopes
                    .bindings
                    .get(index)
                    .map_or(Ty::Error, |b| b.ty.clone());
                self.check_args(args, &[]);
                if ty != Ty::Error {
                    self.report(errors::invalid_operator(name_span, "()", &ty));
                }
                Ty::Error
            }
        }
    }

    /// A call to a function of the module, generic or not.
    fn call_fn(
        &mut self,
        id: crate::ty::FnId,
        name: &str,
        call: &Call,
        args: &[ExprId],
        expected: Option<&Ty>,
    ) -> Ty {
        let (params, ret, declared, generics) = match self.fns.get(id.0) {
            Some(sig) => (
                sig.params.clone(),
                sig.ret.clone(),
                Some(sig.name_span),
                sig.generics.clone(),
            ),
            None => return Ty::Error,
        };
        self.arity(call.span, name, params.len(), args.len(), declared);
        if !self.type_argument_count(call, name, generics.len()) {
            self.check_args(args, &[]);
            return Ty::Error;
        }
        if generics.is_empty() {
            self.check_args(args, &params);
            return ret;
        }
        let use_ = GenericUse {
            name,
            span: call.span,
            generics: &generics,
            params: &params,
            ret: &ret,
            receiver: None,
        };
        let (types, ret) = self.infer_generic(&use_, call.types.as_deref(), args, expected);
        self.calls.insert(call.id, (id, types));
        ret
    }

    /// A structure built from its fields: `Point(1.0, 2.0)`, or `Pair(1, "a")`,
    /// whose type arguments come from the fields.
    fn construct(
        &mut self,
        id: crate::ty::StructId,
        name: &str,
        declared: Option<Span>,
        call: &Call,
        args: &[ExprId],
        expected: Option<&Ty>,
    ) -> Ty {
        let (fields, generics): (Vec<Ty>, _) = self
            .structs
            .get(id.0)
            .map(|s| {
                (
                    s.fields.iter().map(|(_, t)| t.clone()).collect(),
                    s.generics.clone(),
                )
            })
            .unwrap_or_default();
        self.arity(call.span, name, fields.len(), args.len(), declared);
        if !self.type_argument_count(call, name, generics.len()) {
            self.check_args(args, &[]);
            return Ty::Error;
        }
        if generics.is_empty() {
            self.check_args(args, &fields);
            return Ty::Struct(id, name.to_owned());
        }
        let own = Self::identity(&generics);
        let ret = self.struct_instance(id, own);
        let use_ = GenericUse {
            name,
            span: call.span,
            generics: &generics,
            params: &fields,
            ret: &ret,
            receiver: None,
        };
        self.infer_generic(&use_, call.types.as_deref(), args, expected)
            .1
    }

    /// `E0216` when the type arguments written are not as many as the type
    /// parameters; whether they are.
    fn type_argument_count(&mut self, call: &Call, name: &str, expected: usize) -> bool {
        let Some(types) = &call.types else {
            return true;
        };
        if types.len() == expected {
            return true;
        }
        self.report(errors::type_argument_count(
            call.span,
            name,
            expected,
            types.len(),
        ));
        false
    }

    /// `E0214` when the number of arguments is wrong.
    pub(crate) fn arity(
        &mut self,
        span: Span,
        name: &str,
        expected: usize,
        found: usize,
        declared: Option<Span>,
    ) {
        if expected != found {
            self.report(errors::argument_count(
                span, name, expected, found, declared,
            ));
        }
    }

    /// Checks arguments against parameter types; extra arguments are still checked.
    pub(crate) fn check_args(&mut self, args: &[ExprId], params: &[Ty]) {
        for (i, &arg) in args.iter().enumerate() {
            match params.get(i) {
                Some(ty) => self.expect(arg, ty),
                None => {
                    self.infer(arg, None);
                }
            }
        }
    }

    fn method_call(
        &mut self,
        receiver: &Ty,
        name: Symbol,
        name_span: Span,
        args: &[ExprId],
        span: Span,
    ) -> Ty {
        if *receiver == Ty::Error {
            self.check_args(args, &[]);
            return Ty::Error;
        }
        let text = self.name(name);
        // A `Numeric` parameter converts to any number, as numbers do.
        let as_number = if self.satisfies_param(receiver, Bound::Numeric) {
            &Ty::Float(crate::ty::FloatTy::F64)
        } else {
            receiver
        };
        let method = builtins::method(as_number, text)
            .filter(|_| as_number == receiver || builtins::is_conversion(text));
        // A conversion on a parameter that may not be a number.
        if method.is_none() && matches!(receiver, Ty::Param(..)) && builtins::is_conversion(text) {
            let op = format!(".{text}()");
            self.invalid_operator(name_span, &op, receiver, Bound::Numeric);
            self.check_args(args, &[]);
            return Ty::Error;
        }
        let Some(method) = method else {
            self.unknown_member(receiver, text, name_span);
            self.check_args(args, &[]);
            return Ty::Error;
        };
        self.arity(span, text, method.params.len(), args.len(), None);
        self.check_args(args, &method.params);
        method.ret
    }

    fn field(&mut self, base: &Ty, name: Symbol, span: Span) -> Ty {
        let text = self.name(name);
        match base {
            Ty::Error => Ty::Error,
            Ty::Struct(id, _) => {
                let field = self
                    .structs
                    .get(id.0)
                    .and_then(|s| s.fields.iter().find(|(n, _)| n == text))
                    .map(|(_, t)| t.clone());
                field.unwrap_or_else(|| {
                    self.unknown_member(base, text, span);
                    Ty::Error
                })
            }
            other => builtins::field(other, text).unwrap_or_else(|| {
                self.unknown_member(other, text, span);
                Ty::Error
            }),
        }
    }

    pub(crate) fn unknown_member(&mut self, ty: &Ty, member: &str, span: Span) {
        let names = self.member_names(ty);
        let suggestion =
            scope::suggest(member, names.iter().map(String::as_str)).map(str::to_owned);
        self.report(errors::unknown_member(
            span,
            ty,
            member,
            suggestion.as_deref(),
        ));
    }
}

/// A call by name: the expression, the name, the whole call.
struct Call {
    id: ExprId,
    name_span: Span,
    span: Span,
    /// The type arguments written, when they are.
    types: Option<Vec<Ty>>,
}

/// The constraint a type parameter needs for `op`.
const fn needed_bound(op: BinaryOp) -> Bound {
    match op {
        BinaryOp::Eq | BinaryOp::Ne => Bound::Eq,
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => Bound::Ord,
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            Bound::Integer
        }
        _ => Bound::Numeric,
    }
}

impl Checker<'_> {
    /// Whether one operand of this type can ever be valid for `op`: for a
    /// type parameter, whether its constraints allow it.
    fn operand_allowed(&self, op: BinaryOp, ty: &Ty) -> bool {
        match ty {
            Ty::Param(..) if matches!(op, BinaryOp::And | BinaryOp::Or) => false,
            // `+` joins strings too, but a parameter that is `Numeric` is not one.
            Ty::Param(..) => self.satisfies(ty, needed_bound(op)),
            _ => operand_allowed(op, ty),
        }
    }

    /// `E0221`, or `E0228` with the constraint to add when the operand is a
    /// type parameter.
    pub(crate) fn invalid_operator(&mut self, span: Span, op: &str, ty: &Ty, bound: Bound) {
        if let Ty::Param(i, name) = ty
            && let Some(param) = self.param_of(*i)
        {
            let d = errors::missing_constraint(
                span,
                op,
                name,
                bound.name(),
                param.insert_at,
                !param.bounds.is_empty(),
            );
            self.report(d);
            return;
        }
        self.report(errors::invalid_operator(span, op, ty));
    }
}

/// Whether one operand of this type can ever be valid for `op`.
fn operand_allowed(op: BinaryOp, ty: &Ty) -> bool {
    if matches!(ty, Ty::Error) {
        return true;
    }
    match op {
        BinaryOp::Eq | BinaryOp::Ne => !matches!(ty, Ty::Void | Ty::Fn(..)),
        BinaryOp::Add => ty.is_numeric() || *ty == Ty::String,
        BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Rem
        | BinaryOp::Lt
        | BinaryOp::Le
        | BinaryOp::Gt
        | BinaryOp::Ge => ty.is_numeric(),
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
            ty.is_integer()
        }
        BinaryOp::And | BinaryOp::Or => *ty == Ty::Bool,
    }
}
