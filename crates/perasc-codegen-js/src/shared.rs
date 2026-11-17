//! Shared values in a managed module (decision 0033, option m3): `Rc<T>`
//! read only and copied on write, as an owned module's are (decision 0032).
//! An `Rc` is a box `{ v, n }`: a copy holds the same box and counts one
//! more; a write through a holder of a box others hold gives that holder its
//! own. Nothing is ever taken off a count, so a count can only be too high:
//! a copy may be made that was not needed, never one that was missed.

use perasc_check::ty::Ty;
use perasc_syntax::ast::{ExprId, ExprKind};

use crate::Module;

impl Module<'_> {
    /// Whether `id` reads a shared value, or an option of one, from a place:
    /// a variable, a field or an element.
    pub(crate) fn is_rc_place(&self, id: ExprId) -> bool {
        let shared = match self.ty(id) {
            Ty::Rc(_) => true,
            Ty::Option(inner) => matches!(*inner, Ty::Rc(_)),
            _ => false,
        };
        shared
            && match &self.ast.expr(id).kind {
                ExprKind::Name(_) | ExprKind::Field { .. } | ExprKind::Index { .. } => true,
                ExprKind::Paren(inner) => self.is_rc_place(*inner),
                _ => false,
            }
    }

    /// A shared value read to be held once more: its box counted once more.
    pub(crate) fn held(&mut self, js: &str) -> String {
        format!("{}({js})", self.helper("$hold"))
    }

    /// An expression read where it is only lent: the base of a field, the
    /// receiver of a method. A shared value read so is not held once more.
    pub(crate) fn lent(&mut self, id: ExprId) -> String {
        if self.is_rc_place(id) {
            self.expr_as_written(id)
        } else {
            self.expr(id)
        }
    }

    /// `rc.field`: through its box.
    pub(crate) fn through(value: &str) -> String {
        format!("{value}.v")
    }

    /// Before a write through the shared value `id` names: its own box, put
    /// back where it is held.
    pub(crate) fn own(&mut self, id: ExprId) {
        let own = self.helper("$own");
        let place = self.place(id);
        let current = self.read(&place);
        let write = self.write(&place, &format!("{own}({current})"));
        self.line(format!("{write};"));
    }

    /// `Rc.new(value)`: its box.
    pub(crate) fn shared_new(&mut self, value: Option<ExprId>) -> String {
        let value = value.map_or_else(|| "undefined".to_owned(), |v| self.expr(v));
        format!("{{ v: {value}, n: 1 }}")
    }
}
