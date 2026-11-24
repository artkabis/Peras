//! Decision 0039, t1: `Duration` and `Instant`, numbers of
//! milliseconds the checker keeps apart, compiled as `f64`.

use perasc_syntax::ast::{BinaryOp, ExprId, ExprKind, Ident};

use crate::methods::CallSite;
use crate::ty::{FloatTy, Ty};
use crate::{Checker, TimeCall, errors};

const DURATION: Ty = Ty::Float(FloatTy::Duration);
const INSTANT: Ty = Ty::Float(FloatTy::Instant);

impl Checker<'_> {
    /// An operator with a `Duration` or an `Instant` on either side: what
    /// it gives, or an error that says why it gives nothing.
    pub(crate) fn time_binary(
        &mut self,
        op: BinaryOp,
        (lhs, lt): (ExprId, &Ty),
        (rhs, rt): (ExprId, &Ty),
        span: perasc_diag::Span,
    ) -> Option<Ty> {
        let time = |t: &Ty| matches!(t, Ty::Float(f) if f.is_time());
        if !time(lt) && !time(rt) {
            return None;
        }
        if matches!(lt, Ty::Error) || matches!(rt, Ty::Error) {
            return Some(Ty::Error);
        }
        let number = |t: &Ty| matches!(t, Ty::IntLit | Ty::FloatLit | Ty::Float(FloatTy::F64));
        let f64_ty = Ty::Float(FloatTy::F64);
        let result = match (op, lt, rt) {
            (BinaryOp::Add | BinaryOp::Sub, a, b) if *a == DURATION && *b == DURATION => DURATION,
            (BinaryOp::Sub, a, b) if *a == INSTANT && *b == INSTANT => DURATION,
            (BinaryOp::Add | BinaryOp::Sub, a, b) if *a == INSTANT && *b == DURATION => INSTANT,
            (BinaryOp::Add, a, b) if *a == DURATION && *b == INSTANT => INSTANT,
            (BinaryOp::Mul | BinaryOp::Div, a, b) if *a == DURATION && number(b) => {
                self.coerce(rhs, b, &f64_ty);
                DURATION
            }
            (BinaryOp::Mul, a, b) if number(a) && *b == DURATION => {
                self.coerce(lhs, a, &f64_ty);
                DURATION
            }
            (BinaryOp::Div, a, b) if *a == DURATION && *b == DURATION => f64_ty,
            (op, a, b) if op.is_comparison() && a == b => Ty::Bool,
            (op, a, b) if op.is_comparison() || matches!(op, BinaryOp::Add | BinaryOp::Sub) => {
                // A number against a time: its unit is missing.
                if *a == INSTANT && op == BinaryOp::Sub && number(b) {
                    // An elapsed time, or an instant of another clock: the
                    // program knows which (finding F225).
                    let r = self.ast.expr(rhs).span;
                    self.report(errors::time_either(r));
                } else if time(a) && number(b) {
                    let r = self.ast.expr(rhs).span;
                    let written = self.slice(r).to_owned();
                    let name = if *a == INSTANT {
                        "Duration"
                    } else {
                        name_of(a)
                    };
                    self.report(errors::time_number(r, name, &written));
                } else if number(a) && time(b) {
                    let l = self.ast.expr(lhs).span;
                    let written = self.slice(l).to_owned();
                    let name = if *b == INSTANT {
                        "Duration"
                    } else {
                        name_of(b)
                    };
                    self.report(errors::time_number(l, name, &written));
                } else {
                    self.report(errors::time_operator(span, op.symbol(), a, b));
                }
                Ty::Error
            }
            (op, a, b) => {
                self.report(errors::time_operator(span, op.symbol(), a, b));
                Ty::Error
            }
        };
        Some(result)
    }

    /// `Duration.ms(x)`, `Duration.seconds(x)`, `Duration.minutes(x)`,
    /// `Duration.hours(x)` and `Instant.now()`.
    pub(crate) fn time_static(
        &mut self,
        at: &CallSite<'_>,
        base: ExprId,
        name: &Ident,
    ) -> Option<Ty> {
        let ExprKind::Name(symbol) = self.ast.expr(base).kind else {
            return None;
        };
        let owner = self.name(symbol).to_owned();
        if !matches!(owner.as_str(), "Duration" | "Instant") || self.scopes.lookup(symbol).is_some()
        {
            return None;
        }
        let method = self.name(name.symbol).to_owned();
        let (call, ty, args): (TimeCall, Ty, Vec<Ty>) = match (owner.as_str(), method.as_str()) {
            ("Duration", "ms") => (TimeCall::Make(1.0), DURATION, vec![Ty::Float(FloatTy::F64)]),
            ("Duration", "seconds") => (
                TimeCall::Make(1000.0),
                DURATION,
                vec![Ty::Float(FloatTy::F64)],
            ),
            ("Duration", "minutes") => (
                TimeCall::Make(60_000.0),
                DURATION,
                vec![Ty::Float(FloatTy::F64)],
            ),
            ("Duration", "hours") => (
                TimeCall::Make(3_600_000.0),
                DURATION,
                vec![Ty::Float(FloatTy::F64)],
            ),
            ("Instant", "now") => (TimeCall::Now, INSTANT, Vec::new()),
            _ => {
                let ty = if owner == "Duration" {
                    DURATION
                } else {
                    INSTANT
                };
                self.unknown_member(&ty, &method, name.span);
                self.check_args(at.args, &[]);
                return Some(Ty::Error);
            }
        };
        if at.args.len() != args.len() {
            self.report(errors::argument_count(
                at.span,
                &method,
                args.len(),
                at.args.len(),
                None,
            ));
        }
        self.check_args(at.args, &args);
        self.time_calls.insert(at.call, call);
        Some(ty)
    }

    /// `d.toMs()`, `d.toSeconds()`, `d.toMinutes()`, `d.toHours()` of a
    /// `Duration`; nothing else of a number is a method of a time.
    pub(crate) fn time_method(
        &mut self,
        at: &CallSite<'_>,
        receiver: &Ty,
        name: &Ident,
    ) -> Option<Ty> {
        let Ty::Float(f) = receiver else {
            return None;
        };
        if !f.is_time() {
            return None;
        }
        let method = self.name(name.symbol).to_owned();
        let divisor = match (*f, method.as_str()) {
            (FloatTy::Duration, "toMs") => 1.0,
            (FloatTy::Duration, "toSeconds") => 1000.0,
            (FloatTy::Duration, "toMinutes") => 60_000.0,
            (FloatTy::Duration, "toHours") => 3_600_000.0,
            _ => {
                self.unknown_member(receiver, &method, name.span);
                self.check_args(at.args, &[]);
                return Some(Ty::Error);
            }
        };
        self.check_args(at.args, &[]);
        self.time_calls.insert(at.call, TimeCall::Read(divisor));
        Some(Ty::Float(FloatTy::F64))
    }
}

const fn name_of(t: &Ty) -> &'static str {
    match t {
        Ty::Float(f) => f.name(),
        _ => "Duration",
    }
}
