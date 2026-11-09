//! The standard methods of arrays that take an arrow function: `map`,
//! `filter`, `reduce`, `find`, `findIndex`, `some` and `every`, and the
//! methods of numbers and arrays that need no function.
//!
//! An arrow function is not a value yet: it is written as an argument of one
//! of these methods, and compiled in place, as the body of a loop. Its
//! parameters lend the elements, as `for…of` does; they are read-only.

use perasc_diag::Span;
use perasc_syntax::ast::{ExprId, ExprKind};

use crate::scope::Kind;
use crate::ty::{Bound, IntTy, Ty};
use crate::{Checker, errors};

/// The array methods that take an arrow function.
pub const FUNCTION_METHODS: [&str; 7] = [
    "map",
    "filter",
    "reduce",
    "find",
    "findIndex",
    "some",
    "every",
];

impl Checker<'_> {
    /// A method of `Array<element>` that takes an arrow function, or `None`
    /// when `name` is not one.
    pub(crate) fn array_function_method(
        &mut self,
        element: &Ty,
        name: &str,
        args: &[ExprId],
        span: Span,
    ) -> Option<Ty> {
        if !FUNCTION_METHODS.contains(&name) {
            return None;
        }
        let expected = if name == "reduce" { 2 } else { 1 };
        self.arity(span, name, expected, args.len(), None);
        let Some(&f) = args.first() else {
            return Some(Ty::Error);
        };
        let ty = match name {
            "map" => {
                let value = self.arrow(f, std::slice::from_ref(element), None);
                Ty::Array(Box::new(value))
            }
            "filter" => {
                self.arrow(f, std::slice::from_ref(element), Some(&Ty::Bool));
                Ty::Array(Box::new(element.clone()))
            }
            "find" => {
                self.arrow(f, std::slice::from_ref(element), Some(&Ty::Bool));
                Ty::Option(Box::new(element.clone()))
            }
            "findIndex" => {
                self.arrow(f, std::slice::from_ref(element), Some(&Ty::Bool));
                Ty::Int(IntTy::I32)
            }
            "some" | "every" => {
                self.arrow(f, std::slice::from_ref(element), Some(&Ty::Bool));
                Ty::Bool
            }
            // `reduce((acc, x) => …, start)`: the start gives the type.
            _ => {
                let start = args
                    .get(1)
                    .map_or(Ty::Error, |&s| self.infer(s, None).defaulted());
                if let Some(&s) = args.get(1) {
                    self.coerce(
                        s,
                        &self.expr_types.get(&s).cloned().unwrap_or(Ty::Error),
                        &start,
                    );
                }
                self.arrow(f, &[start.clone(), element.clone()], Some(&start));
                start
            }
        };
        for &extra in args.get(expected..).unwrap_or_default() {
            self.infer(extra, None);
        }
        Some(ty)
    }

    /// Checks an arrow function given parameters of these types, and a value
    /// of type `ret` when one is required; returns the type of its value.
    pub(crate) fn arrow(&mut self, id: ExprId, params: &[Ty], ret: Option<&Ty>) -> Ty {
        let expr = self.ast.expr(id);
        let ExprKind::Lambda {
            params: written,
            body,
        } = &expr.kind
        else {
            self.infer(id, None);
            self.report(errors::arrow_expected(expr.span));
            return Ty::Error;
        };
        if written.len() != params.len() {
            self.report(errors::argument_count(
                expr.span,
                "=>",
                params.len(),
                written.len(),
                None,
            ));
        }
        self.expr_types.insert(id, Ty::Void);
        self.scopes.push();
        for ((name, annotation), ty) in written.iter().zip(params) {
            let mut ty = ty.clone();
            if let Some(annotation) = annotation {
                let given = self.resolve_type(annotation);
                if given != ty && given != Ty::Error && ty != Ty::Error {
                    self.report(errors::mismatch(annotation.span, &ty, &given, None));
                }
                // The body is checked with the type written: one mistake, one error.
                ty = given;
            }
            self.declare(name.symbol, Some(name.span), Kind::Pattern, ty);
        }
        let value = match ret {
            Some(ret) => {
                self.expect(*body, ret);
                ret.clone()
            }
            None => self.infer(*body, None).defaulted(),
        };
        self.pop_scope();
        value
    }

    /// `min(b)`, `max(b)` and `clamp(lo, hi)` on a number, or on a type
    /// parameter that is `Ord`; `None` for another method.
    pub(crate) fn ordering_method(
        &mut self,
        receiver: &Ty,
        name: &str,
        args: &[ExprId],
        span: Span,
    ) -> Option<Ty> {
        let count = match name {
            "min" | "max" => 1,
            "clamp" => 2,
            _ => return None,
        };
        let ordered = receiver.is_numeric() || self.satisfies_param(receiver, Bound::Ord);
        if !ordered {
            return None;
        }
        let me = receiver.clone().defaulted();
        self.arity(span, name, count, args.len(), None);
        self.check_args(args, &vec![me.clone(); count]);
        Some(me)
    }
}
