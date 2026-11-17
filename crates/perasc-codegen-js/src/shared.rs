//! Shared values in a managed module (decision 0033, prototypes): `Rc<T>`
//! read only and copied on write, as an owned module's are (decision 0032),
//! by one of three options `PERAS_PROTO_RCM` names: m1, the object shared
//! and copied at every write; m2, copied at every holder; m3, a box
//! `{ v, n }` counted, never released.

use perasc_check::ty::Ty;
use perasc_syntax::ast::{ExprId, ExprKind};

use crate::Module;

impl Module<'_> {
    /// The option of decision 0033 this build follows.
    pub(crate) fn rcm() -> String {
        perasc_check::proto_rcm()
    }

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

    /// A shared value read to be held once more: under m2 a copy of it,
    /// under m3 its box counted once more.
    pub(crate) fn held(&mut self, js: String) -> String {
        match Self::rcm().as_str() {
            "m2" => format!("structuredClone({js})"),
            "m3" => format!("{}({js})", self.helper("$hold")),
            _ => js,
        }
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

    /// `rc.field`: under m3, through its box.
    pub(crate) fn through(value: &str) -> String {
        if Self::rcm() == "m3" {
            format!("{value}.v")
        } else {
            value.to_owned()
        }
    }

    /// Before a write through the shared value `id` names: under m1 a copy
    /// of it, under m3 its own box, put back where it is held.
    pub(crate) fn own(&mut self, id: ExprId) {
        let copy = match Self::rcm().as_str() {
            "m1" => "structuredClone",
            "m3" => self.helper("$own"),
            _ => return,
        };
        let place = self.place(id);
        let current = self.read(&place);
        let write = self.write(&place, &format!("{copy}({current})"));
        self.line(format!("{write};"));
    }

    /// `Rc.new(value)`: the value, or under m3 its box.
    pub(crate) fn shared_new(&mut self, value: Option<ExprId>) -> String {
        let value = value.map_or_else(|| "undefined".to_owned(), |v| self.expr(v));
        if Self::rcm() == "m3" {
            format!("{{ v: {value}, n: 1 }}")
        } else {
            value
        }
    }
}
