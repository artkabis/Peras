//! Parts (decision 0031, option p2, prototype): in a turn of `parallel for`
//! counted by `b`, `let band = xs.part(b, k)` is the elements `b * k` to
//! `(b + 1) * k` of `xs`, which the turn reads and writes. A part is no
//! buffer: its name stands for its source, an offset and a length, each a
//! local of the turn; `band[j]` is `xs[b * k + j]`, with `j < k` checked.
//! Taken at the counter with a `k` the turn does not compute, the parts of
//! two turns never overlap, which `parallel::check` relies on.

use perasc_syntax::ast::{ExprId, ExprKind as AstExpr, Local as AstLocal};

use super::{FnState, Lowering};
use crate::{BinOp, BufStmt, Expr, ExprKind, LocalId, Stmt, Ty};

/// What a part's name stands for.
#[derive(Clone, Copy)]
pub(super) struct Part {
    pub(super) source: ExprId,
    pub(super) offset: LocalId,
    pub(super) len: LocalId,
}

impl Lowering<'_> {
    /// `let band = xs.part(b, k)` in a turn: the part bound, `true`; anything
    /// else, `false`.
    pub(super) fn part_local(
        &mut self,
        state: &mut FnState,
        local: &AstLocal,
        out: &mut Vec<Stmt>,
    ) -> bool {
        if perasc_check::proto_par() != "p2" {
            return false;
        }
        let Some(init) = local.init else {
            return false;
        };
        let AstExpr::Call { callee, args, .. } = &self.ast.expr(init).kind else {
            return false;
        };
        let AstExpr::Field { base, name } = self.ast.expr(*callee).kind else {
            return false;
        };
        let ([at, size], "part") = (args.as_slice(), self.ast.name(name.symbol)) else {
            return false;
        };
        let (at, size) = (*at, *size);
        let span = self.ast.expr(init).span;
        // Outside a turn, refused below; still bound, so that its uses are
        // not refused again.
        let (counter, mark) = state.turn.map_or((None, 0), |(c, m)| (Some(c), m));
        // Taken at the counter, of a size the turn does not compute.
        let at_counter = matches!(self.ast.expr(at).kind, AstExpr::Name(s) if Some(s) == counter);
        let size = self.expr(state, size);
        let fixed = reads_before(&size, mark);
        let len = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(len, size));
        let at = self.expr(state, at);
        let offset = state.temp(Ty::I32);
        state.pre.push(Stmt::Set(
            offset,
            super::binary(BinOp::Mul, at, local_i32(len)),
        ));
        state.flush(out);
        state.parts.insert(
            local.name.symbol,
            Part {
                source: base,
                offset,
                len,
            },
        );
        // A part refused here stays known, so that its elements are not
        // refused a second time as written elsewhere than the counter.
        state.turn_parts.push((offset, len));
        if !(at_counter && fixed) {
            self.diagnostics
                .push(super::parallel::refused(span, "part", ""));
        }
        true
    }

    /// The part a name stands for, if it is one.
    pub(super) fn part_of(
        state: &FnState,
        base: ExprId,
        ast: &perasc_syntax::ast::Ast,
    ) -> Option<Part> {
        match ast.expr(base).kind {
            AstExpr::Name(symbol) => state.parts.get(&symbol).copied(),
            _ => None,
        }
    }

    /// The index of an element of a part, in its source: `j < k` checked,
    /// then `b * k + j`.
    pub(super) fn part_index(state: &mut FnState, part: Part, index: Expr) -> Expr {
        state.pre.push(Stmt::Buf(
            BufStmt::CheckIndex,
            vec![index.clone(), local_i32(part.len)],
        ));
        // Left as written, for `parallel::check` to see a part's index.
        super::binary(BinOp::Add, local_i32(part.offset), index)
    }
}

fn local_i32(local: LocalId) -> Expr {
    Expr {
        kind: ExprKind::Local(local),
        ty: Ty::I32,
    }
}

/// Whether an expression reads only locals declared before `mark`: the
/// outer function's, which no turn changes.
fn reads_before(e: &Expr, mark: usize) -> bool {
    let mut only = true;
    crate::each_expr(&[Stmt::Expr(e.clone())], &mut |x| {
        if let ExprKind::Local(l) | ExprKind::Borrow(l) = x.kind
            && usize::try_from(l.0).is_ok_and(|i| i >= mark)
        {
            only = false;
        }
    });
    only
}
