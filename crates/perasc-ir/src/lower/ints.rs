//! The integer types other than `i32` and `i64` (decision 0024): `i8`,
//! `i16`, `u8`, `u16` and `u32` kept in an `i32`, `u64` in an `i64`.
//!
//! A value is always in its canonical form: an `i8` or an `i16`
//! sign-extended, a `u8` or a `u16` zero-extended, so that `==` and the
//! bitwise operators need nothing of their own. The operations carry the
//! signedness: unsigned division, remainder, right shift and comparisons,
//! and conversions that read or make unsigned values.

use perasc_check::ty::{FloatTy, IntTy, Ty as CheckTy};
use perasc_syntax::ast::ExprId;

use super::{FnState, Lowering, binary};
use crate::{BinOp, BufStmt, Const, Expr, ExprKind, Stmt, Ty, UnOp};

/// Whether an integer type is one of decision 0024's: not `i32` nor `i64`.
pub(super) const fn is_other(kind: IntTy) -> bool {
    !matches!(kind, IntTy::I32 | IntTy::I64)
}

pub(super) const fn is_unsigned(kind: IntTy) -> bool {
    matches!(kind, IntTy::U8 | IntTy::U16 | IntTy::U32 | IntTy::U64)
}

/// The IR's name of one of decision 0024's types, for the loader.
pub(super) const fn int_kind(kind: IntTy) -> crate::IntKind {
    match kind {
        IntTy::I8 => crate::IntKind::I8,
        IntTy::I16 => crate::IntKind::I16,
        IntTy::U8 => crate::IntKind::U8,
        IntTy::U16 => crate::IntKind::U16,
        IntTy::U32 | IntTy::I32 => crate::IntKind::U32,
        IntTy::U64 | IntTy::I64 => crate::IntKind::U64,
    }
}

/// Whether a shape holds one of decision 0024's integers, which the loader
/// must convert: a function that takes or gives one is wrapped.
pub(super) fn shape_has_int(shape: &crate::Shape) -> bool {
    use crate::Shape;
    match shape {
        Shape::Int(_) => true,
        Shape::Object(_, fields) => fields.iter().any(|(_, f)| shape_has_int(f)),
        Shape::Option(inner) | Shape::Records(inner, _) => shape_has_int(inner),
        Shape::Enum(_, variants) => variants.iter().any(|(_, f)| f.iter().any(shape_has_int)),
        Shape::Map(map) => shape_has_int(&map.key) || map.value.as_ref().is_some_and(shape_has_int),
        Shape::Value(_) | Shape::Array(..) | Shape::String(_) => false,
    }
}

/// The element type of an `Array<T>` of one of decision 0024's integers,
/// stored at its own width: `None` for any other element.
pub(super) const fn array_elem(element: &CheckTy) -> Option<crate::Elem> {
    match element {
        CheckTy::Int(kind) if is_other(*kind) => Some(crate::Elem::of_int(int_kind(*kind))),
        _ => None,
    }
}

/// The number of bits of an integer type.
const fn bits(kind: IntTy) -> i64 {
    match kind {
        IntTy::I8 | IntTy::U8 => 8,
        IntTy::I16 | IntTy::U16 => 16,
        IntTy::I32 | IntTy::U32 => 32,
        IntTy::I64 | IntTy::U64 => 64,
    }
}

/// The range of a type narrower than its storage, `i8` to `u16`.
const fn narrow_range(kind: IntTy) -> Option<(i64, i64)> {
    match kind {
        IntTy::I8 => Some((-128, 127)),
        IntTy::I16 => Some((-32_768, 32_767)),
        IntTy::U8 => Some((0, 255)),
        IntTy::U16 => Some((0, 65_535)),
        _ => None,
    }
}

/// The IR type that keeps an integer type.
pub(super) const fn storage(kind: IntTy) -> Ty {
    if matches!(kind, IntTy::I64 | IntTy::U64) {
        Ty::I64
    } else {
        Ty::I32
    }
}

fn int(v: i64, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(v)),
        ty,
    }
}

/// `e`, of an `i32` that may hold any bits, in the canonical form of `kind`:
/// the low bits kept, then extended by the sign or by zeros.
pub(super) fn narrow(e: Expr, kind: IntTy) -> Expr {
    match kind {
        IntTy::U8 => binary(BinOp::BitAnd, e, int(0xFF, Ty::I32)),
        IntTy::U16 => binary(BinOp::BitAnd, e, int(0xFFFF, Ty::I32)),
        IntTy::I8 | IntTy::I16 => {
            let shift = 32 - bits(kind);
            binary(
                BinOp::Shr,
                binary(BinOp::Shl, e, int(shift, Ty::I32)),
                int(shift, Ty::I32),
            )
        }
        _ => e,
    }
}

/// An unsigned conversion, `UnOp::ConvertU`, of `e` to `ty`.
fn convert_u(e: Expr, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Unary(UnOp::ConvertU, Box::new(e)),
        ty,
    }
}

/// A signed conversion of `e` to `ty`.
fn convert(e: Expr, ty: Ty) -> Expr {
    if e.ty == ty {
        return e;
    }
    Expr {
        kind: ExprKind::Convert(Box::new(e)),
        ty,
    }
}

/// The literal `value` of `kind`, as its storage holds it: a `u32` or a
/// `u64` above the signed range keeps its bits.
pub(super) fn literal(value: u64, kind: IntTy) -> Const {
    match kind {
        #[allow(clippy::cast_possible_truncation)]
        IntTy::U32 => Const::Int(i64::from((value as u32).cast_signed())),
        #[allow(clippy::cast_possible_wrap)]
        IntTy::U64 => Const::Int(value as i64),
        _ => Const::Int(i64::try_from(value).unwrap_or(i64::MAX)),
    }
}

/// `a op b` with `+`, `-` or `*` on `kind`: the value, wrapped to `kind`,
/// and whether the exact result fits it.
pub(super) fn checked_parts(
    state: &mut FnState,
    op: BinOp,
    a: Expr,
    b: Expr,
    kind: IntTy,
) -> (Expr, Expr) {
    let a = state.spill(a);
    let b = state.spill(b);
    let ty = a.ty;
    let raw = state.spill(binary(op, a.clone(), b.clone()));
    if let Some((lo, hi)) = narrow_range(kind) {
        // Two operands of 16 bits never overflow an `i32` but by `*` of two
        // `u16`, whose result then reads negative: out of range all the same.
        let fits = binary(
            BinOp::And,
            binary(BinOp::Ge, raw.clone(), int(lo, Ty::I32)),
            binary(BinOp::Le, raw.clone(), int(hi, Ty::I32)),
        );
        return (narrow(raw, kind), fits);
    }
    let fits = match op {
        // Unsigned: the sum wrapped when it is below an operand, the
        // difference when the subtrahend is the larger.
        BinOp::Add => binary(BinOp::GeU, raw.clone(), a),
        BinOp::Sub => binary(BinOp::GeU, a, b),
        _ if ty == Ty::I32 => {
            let wide = binary(BinOp::Mul, convert_u(a, Ty::I64), convert_u(b, Ty::I64));
            binary(BinOp::LeU, wide, int(0xFFFF_FFFF, Ty::I64))
        }
        // A `u64` product: exact when dividing it by one operand gives the
        // other, or when that operand is zero.
        _ => binary(
            BinOp::Or,
            binary(BinOp::Eq, a.clone(), int(0, Ty::I64)),
            binary(BinOp::Eq, binary(BinOp::DivU, raw.clone(), a), b),
        ),
    };
    (raw, fits)
}

/// The count of a shift, taken modulo the width of `kind`: WebAssembly
/// takes it modulo 32 or 64 only.
fn shift_count(b: Expr, kind: IntTy) -> Expr {
    match kind {
        IntTy::I8 | IntTy::U8 | IntTy::I16 | IntTy::U16 => {
            binary(BinOp::BitAnd, b, int(bits(kind) - 1, Ty::I32))
        }
        _ => b,
    }
}

const fn unsigned_op(op: BinOp) -> BinOp {
    match op {
        BinOp::Div => BinOp::DivU,
        BinOp::Rem => BinOp::RemU,
        BinOp::Shr => BinOp::ShrU,
        BinOp::Lt => BinOp::LtU,
        BinOp::Le => BinOp::LeU,
        BinOp::Gt => BinOp::GtU,
        BinOp::Ge => BinOp::GeU,
        other => other,
    }
}

impl Lowering<'_> {
    /// The integer type of an expression, when it is one of decision 0024's.
    pub(super) fn int_kind(&self, id: ExprId) -> Option<IntTy> {
        match self.check_type(id)? {
            CheckTy::Int(kind) if is_other(kind) => Some(kind),
            _ => None,
        }
    }

    /// `a op b` on `kind`: with overflow checks, `+`, `-` and `*` stop the
    /// program when the exact result leaves the type, and wrap otherwise;
    /// `/` of the smallest `i8` or `i16` by -1 stops it in every build, as it
    /// does for `i32`.
    pub(super) fn other_arith(
        &mut self,
        state: &mut FnState,
        op: BinOp,
        a: Expr,
        b: Expr,
        kind: IntTy,
    ) -> Expr {
        let unsigned = is_unsigned(kind);
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul => {
                let (value, fits) = checked_parts(state, op, a, b, kind);
                if self.overflow_checks {
                    state
                        .pre
                        .push(Stmt::Buf(BufStmt::CheckOverflow, vec![fits]));
                }
                value
            }
            BinOp::Div if !unsigned => {
                let quotient = state.spill(binary(BinOp::Div, a, b));
                if let Some((_, hi)) = narrow_range(kind) {
                    let fits = binary(BinOp::Le, quotient.clone(), int(hi, Ty::I32));
                    state
                        .pre
                        .push(Stmt::Buf(BufStmt::CheckOverflow, vec![fits]));
                }
                quotient
            }
            BinOp::Shl => narrow(binary(BinOp::Shl, a, shift_count(b, kind)), kind),
            BinOp::Shr => {
                let op = if unsigned { BinOp::ShrU } else { BinOp::Shr };
                binary(op, a, shift_count(b, kind))
            }
            _ if unsigned => binary(unsigned_op(op), a, b),
            _ => binary(op, a, b),
        }
    }

    /// `x.toT()` from one number type to another, as `as` converts in Rust:
    /// between integers, the bits wrapped to the target; from a float, toward
    /// zero and saturating, `NaN` to 0; to a float, to the nearest.
    pub(super) fn number_conversion(
        state: &mut FnState,
        receiver: Expr,
        from: &CheckTy,
        to: &CheckTy,
    ) -> Expr {
        let target = match to {
            CheckTy::Int(kind) => storage(*kind),
            CheckTy::Float(FloatTy::F32) => Ty::F32,
            _ => Ty::F64,
        };
        match (from, to) {
            (CheckTy::Int(a), CheckTy::Int(b)) => {
                let widened = match (receiver.ty, target) {
                    // The canonical form of an unsigned type is zero-extended.
                    (Ty::I32, Ty::I64) if is_unsigned(*a) => convert_u(receiver, Ty::I64),
                    _ => convert(receiver, target),
                };
                narrow(widened, *b)
            }
            (CheckTy::Int(a), CheckTy::Float(_)) => {
                if matches!(a, IntTy::U32 | IntTy::U64) {
                    convert_u(receiver, target)
                } else {
                    convert(receiver, target)
                }
            }
            (CheckTy::Float(_), CheckTy::Int(b)) => match narrow_range(*b) {
                // Toward zero, saturating to an `i32`, then to the type: its
                // bounds are integers, so the result is the same.
                Some((lo, hi)) => {
                    let t = state.temp(Ty::I32);
                    state.pre.push(Stmt::Set(t, convert(receiver, Ty::I32)));
                    let read = super::local_expr(state, t);
                    for (op, bound) in [(BinOp::Lt, lo), (BinOp::Gt, hi)] {
                        state.pre.push(Stmt::If {
                            cond: binary(op, read.clone(), int(bound, Ty::I32)),
                            then: vec![Stmt::Set(t, int(bound, Ty::I32))],
                            otherwise: Vec::new(),
                        });
                    }
                    read
                }
                None if is_unsigned(*b) => convert_u(receiver, target),
                None => convert(receiver, target),
            },
            _ => convert(receiver, target),
        }
    }
}
