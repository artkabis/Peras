//! The standard methods of arrays that take an arrow function: `map`,
//! `filter`, `reduce`, `find`, `findIndex`, `some` and `every`, and the
//! methods of numbers and arrays that need no function.
//!
//! An arrow function is not a value yet: it is written as an argument of one
//! of these methods, and compiled in place, as the body of a loop. Its
//! parameters lend the elements, as `for…of` does; they are read-only.

use perasc_diag::Span;
use perasc_syntax::ast::{Block, ExprId, ExprKind, LambdaBody};

use crate::scope::Kind;
use crate::ty::{Bound, IntTy, Ty};
use crate::{Checker, errors};

/// The array methods that take an arrow function.
pub const FUNCTION_METHODS: [&str; 9] = [
    "map",
    "filter",
    "reduce",
    "find",
    "findIndex",
    "some",
    "every",
    "sortBy",
    "sortByDescending",
];

/// Whether `sortBy` orders by values of this type: `i32`, `i64`, `f32`,
/// `f64` and strings.
#[must_use]
pub fn is_sort_key(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Int(IntTy::I32 | IntTy::I64) | Ty::Float(_) | Ty::IntLit | Ty::FloatLit | Ty::String
    )
}

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
            // `xs.sortBy((o) => o.amount)`: a key per element, a number or a
            // string; the array is sorted in place, stably.
            "sortBy" | "sortByDescending" => {
                let key = self.arrow(f, std::slice::from_ref(element), None);
                if !is_sort_key(&key) && key != Ty::Error {
                    let at = match &self.ast.expr(f).kind {
                        ExprKind::Lambda {
                            body: LambdaBody::Expr(body),
                            ..
                        } => self.ast.expr(*body).span,
                        _ => self.ast.expr(f).span,
                    };
                    self.report(errors::sort_key(at, &key));
                }
                // Code generation reads the key's type on the function.
                let key = key.defaulted();
                self.expr_types
                    .insert(f, Ty::Callback(vec![element.clone()], Box::new(key)));
                Ty::Void
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
            ret: ret_written,
            body,
        } = &expr.kind
        else {
            // In a managed module, a function already made: a variable, a
            // parameter, or a function of the module by its name.
            if self.managed {
                let value = ret.cloned().unwrap_or(Ty::Void);
                let wanted = Ty::Callback(params.to_vec(), Box::new(value.clone()));
                let actual = self.infer(id, Some(&wanted));
                return if self.coerce(id, &actual, &wanted) {
                    value
                } else {
                    Ty::Error
                };
            }
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
                let given = self.value_type(annotation);
                if given != ty && given != Ty::Error && ty != Ty::Error {
                    self.report(errors::mismatch(annotation.span, &ty, &given, None));
                }
                // The body is checked with the type written: one mistake, one error.
                ty = given;
            }
            self.declare(name.symbol, Some(name.span), Kind::Pattern, ty);
        }
        // `(x): R => …`: the type written is the one checked.
        let given = ret_written.as_ref().map(|t| (self.value_type(t), t.span));
        let ret = match (ret, given) {
            (Some(ret), Some((given, at))) => {
                if given != *ret && given != Ty::Error && *ret != Ty::Error {
                    self.report(errors::mismatch(at, ret, &given, None));
                }
                Some((given, Some(at)))
            }
            (None, Some((given, at))) => Some((given, Some(at))),
            (ret, None) => ret.map(|r| (r.clone(), None)),
        };
        let value = match body {
            LambdaBody::Expr(body) => match ret {
                Some((ret, _)) => {
                    self.expect(*body, &ret);
                    ret
                }
                None => self.infer(*body, None).defaulted(),
            },
            LambdaBody::Block(block) if self.managed => {
                let (ret, at) = ret.unwrap_or((Ty::Void, None));
                self.arrow_block(block, ret, at)
            }
            LambdaBody::Block(block) => {
                self.report(errors::managed_only(block.span, "arrow-block"));
                Ty::Error
            }
        };
        self.pop_scope();
        value
    }

    /// The statements of an arrow function, which give a `ret` by `return`.
    fn arrow_block(&mut self, block: &Block, ret: Ty, ret_span: Option<Span>) -> Ty {
        let outer_ret = self.ret.replace((ret.clone(), ret_span));
        let outer_returned = std::mem::take(&mut self.returned);
        // It runs later, when called: it cannot wait for the function around it.
        let outer_async = std::mem::replace(&mut self.flags.in_async, false);
        self.block(&block.stmts);
        if !matches!(ret, Ty::Void | Ty::Error) && !crate::always_returns(self.ast, &block.stmts) {
            let end = block.span.end;
            let closing = Span::new(end.saturating_sub(1), end);
            let at = ret_span.unwrap_or(block.span);
            self.report(errors::missing_return(closing, "=>", &ret, at));
        }
        self.flags.in_async = outer_async;
        self.returned = outer_returned;
        self.ret = outer_ret;
        ret
    }

    /// An arrow function used as a value, in a managed module: its parameters'
    /// types written, its value's written or inferred.
    pub(crate) fn arrow_value(&mut self, id: ExprId) -> Ty {
        let ExprKind::Lambda { params, ret, body } = &self.ast.expr(id).kind else {
            return Ty::Error;
        };
        let mut types = Vec::with_capacity(params.len());
        for (name, annotation) in params {
            let ty = if let Some(t) = annotation {
                self.value_type(t)
            } else {
                let text = self.name(name.symbol).to_owned();
                self.report(errors::arrow_param_type(name.span, &text));
                Ty::Error
            };
            types.push(ty);
        }
        // With no type written, a block gives nothing and a value its own type.
        let wanted = match (ret, body) {
            (Some(t), _) => Some(self.value_type(t)),
            (None, LambdaBody::Block(_)) => Some(Ty::Void),
            (None, LambdaBody::Expr(_)) => None,
        };
        let value = self.arrow(id, &types, wanted.as_ref());
        Ty::Callback(types, Box::new(value))
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
