//! Numbers and booleans as text, and text as integers: functions of the
//! runtime.
//!
//! `n.toString()` gives the decimal digits of an integer, `x.toString()` a
//! float as JavaScript writes it, `b.toString()` gives `true` or `false`. A template literal, `` `${n} items` ``, writes its
//! values the same way into one new string, sized once. `s.parseI32()` and `s.parseI64()` give an option: the
//! runtime first says whether `s` is an integer in range, then, only then,
//! reads it.

use perasc_syntax::ast::ExprId;

use super::heap::Mode;
use super::{FnState, Lowering};
use crate::{BinOp, BufOp, BufStmt, Const, Elem, Expr, ExprKind, RuntimeFn, Stmt, Ty};

impl Lowering<'_> {
    /// `x.toString()` on a number or a boolean: a new string.
    pub(super) fn stringified(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        mode: Mode,
    ) -> Vec<Expr> {
        let receiver = self.expr(state, base);
        let flag = |single: bool| Expr {
            kind: ExprKind::Const(Const::Int(i64::from(single))),
            ty: Ty::I32,
        };
        let widened = |receiver: Expr, ty: Ty| Expr {
            kind: ExprKind::Convert(Box::new(receiver)),
            ty,
        };
        let operands = match receiver.ty {
            Ty::Bool => (RuntimeFn::FmtBool, vec![receiver]),
            Ty::I64 => (RuntimeFn::FmtInt, vec![receiver]),
            Ty::F64 => (RuntimeFn::FmtFloat, vec![receiver, flag(false)]),
            // Exactly, then written with the digits of an `f32`.
            Ty::F32 => (
                RuntimeFn::FmtFloat,
                vec![widened(receiver, Ty::F64), flag(true)],
            ),
            _ => (RuntimeFn::FmtInt, vec![widened(receiver, Ty::I64)]),
        };
        let text = runtime(operands.0, operands.1);
        Self::fresh(state, vec![text], mode)
    }

    /// `x.toFixed(d)`: written by the host, as JavaScript writes it, after a
    /// check that `d` is from 0 to 100, which throws the `RangeError` that
    /// JavaScript's would.
    pub(super) fn fixed(
        &mut self,
        state: &mut FnState,
        base: ExprId,
        args: &[ExprId],
        mode: Mode,
    ) -> Vec<Expr> {
        let receiver = self.expr(state, base);
        let x = state.temp(Ty::F64);
        let value = if receiver.ty == Ty::F64 {
            receiver
        } else {
            // Every integer of 32 bits and every `f32` is exact as an `f64`.
            Expr {
                kind: ExprKind::Convert(Box::new(receiver)),
                ty: Ty::F64,
            }
        };
        state.pre.push(Stmt::Set(x, value));
        let digits = args.first().map_or(
            Expr {
                kind: ExprKind::Const(Const::Int(0)),
                ty: Ty::I32,
            },
            |&a| self.expr(state, a),
        );
        let d = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(d, digits));
        let d_expr = super::local_expr(state, d);
        state.pre.push(Stmt::Buf(
            BufStmt::CheckIndex,
            vec![
                d_expr.clone(),
                Expr {
                    kind: ExprKind::Const(Const::Int(101)),
                    ty: Ty::I32,
                },
            ],
        ));
        let mode_value = super::binary(
            BinOp::Add,
            d_expr,
            Expr {
                kind: ExprKind::Const(Const::Int(2)),
                ty: Ty::I32,
            },
        );
        let text = runtime(
            RuntimeFn::FmtFloat,
            vec![super::local_expr(state, x), mode_value],
        );
        Self::fresh(state, vec![text], mode)
    }

    /// `` `text${a}text` ``: each piece of text and each value as text, in
    /// order, lent; then one new string with room for them all, each
    /// appended.
    pub(super) fn template(
        &mut self,
        state: &mut FnState,
        parts: &[String],
        exprs: &[ExprId],
        mode: Mode,
    ) -> Vec<Expr> {
        let mut pieces = Vec::new();
        for (k, part) in parts.iter().enumerate() {
            if !part.is_empty() {
                pieces.extend(self.string_literal(state, part, Mode::Borrow));
            }
            let Some(&value) = exprs.get(k) else {
                continue;
            };
            if self.is_aggregate(value) {
                pieces.extend(self.buffer(state, value));
            } else {
                pieces.extend(self.stringified(state, value, Mode::Borrow));
            }
        }
        let room = pieces
            .iter()
            .map(|p| Expr {
                kind: ExprKind::Buf(BufOp::Len, vec![p.clone()]),
                ty: Ty::I32,
            })
            .reduce(|a, b| super::binary(BinOp::Add, a, b))
            .unwrap_or(Expr {
                kind: ExprKind::Const(Const::Int(0)),
                ty: Ty::I32,
            });
        let joined = state.owned_temp(Expr {
            kind: ExprKind::Buf(BufOp::New(Elem::U8), vec![room]),
            ty: Ty::Owned,
        });
        for piece in pieces {
            state.pre.push(Stmt::Buf(
                BufStmt::Append(Elem::U8),
                vec![super::heap::borrowed(joined), piece],
            ));
        }
        let joined = Expr {
            kind: ExprKind::Local(joined),
            ty: Ty::Owned,
        };
        Self::fresh(state, vec![joined], mode)
    }

    /// `s.parseF64()` on the lent string `s`: whether it is a decimal number,
    /// and its value, correctly rounded; 0 when it is not. The runtime gives
    /// NaN for a text that is not one, which no number accepted can be.
    pub(super) fn parse_float(state: &mut FnState, s: Expr) -> Vec<Expr> {
        let read = state.temp(Ty::F64);
        state
            .pre
            .push(Stmt::Set(read, runtime(RuntimeFn::ParseFloat, vec![s])));
        let read = super::local_expr(state, read);
        let some = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(
            some,
            super::binary(BinOp::Eq, read.clone(), read.clone()),
        ));
        let value = state.temp(Ty::F64);
        state.pre.push(Stmt::Set(value, super::zero(Ty::F64)));
        state.pre.push(Stmt::If {
            cond: super::local_expr(state, some),
            then: vec![Stmt::Set(value, read)],
            otherwise: Vec::new(),
        });
        vec![
            super::local_expr(state, some),
            super::local_expr(state, value),
        ]
    }

    /// `s.parseI32()` or `s.parseI64()` on the lent string `s`: whether it is
    /// an integer in range, and its value, 0 when it is not.
    pub(super) fn parse_int(state: &mut FnState, s: Expr, narrow: bool) -> Vec<Expr> {
        let bits = if narrow { 32 } else { 64 };
        let some = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(
            some,
            runtime(
                RuntimeFn::IsInt,
                vec![
                    s.clone(),
                    Expr {
                        kind: ExprKind::Const(Const::Int(bits)),
                        ty: Ty::I32,
                    },
                ],
            ),
        ));
        let ty = if narrow { Ty::I32 } else { Ty::I64 };
        let value = state.temp(ty);
        state.pre.push(Stmt::Set(value, super::zero(ty)));
        let parsed = runtime(RuntimeFn::ParseInt, vec![s]);
        let parsed = if narrow {
            Expr {
                kind: ExprKind::Convert(Box::new(parsed)),
                ty,
            }
        } else {
            parsed
        };
        state.pre.push(Stmt::If {
            cond: super::local_expr(state, some),
            then: vec![Stmt::Set(value, parsed)],
            otherwise: Vec::new(),
        });
        vec![
            super::local_expr(state, some),
            super::local_expr(state, value),
        ]
    }
}

/// A call to a function of the runtime.
fn runtime(f: RuntimeFn, operands: Vec<Expr>) -> Expr {
    Expr {
        kind: ExprKind::Buf(BufOp::Runtime(f), operands),
        ty: f.result(),
    }
}
