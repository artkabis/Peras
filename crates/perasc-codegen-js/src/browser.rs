//! The browser's API in JavaScript, and the release of resources
//! ([decision 0009]).
//!
//! A resource is an object with `release()` and `[Symbol.dispose]()`. The
//! variable that owns it releases it when it goes out of scope: the rest of
//! its block runs in a `try`, and the `finally` releases it, unless it was
//! given away first, which a flag of the variable records. In debug, each
//! resource is registered with a `FinalizationRegistry`, which reports one
//! collected without being released, with the file and line that made it.
//!
//! [decision 0009]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0009-browser-and-resources.md

use perasc_check::browser::Host;
use perasc_diag::Span;
use perasc_syntax::ast::{ExprId, Ident};

use crate::Module;
use crate::expr::string_literal;

impl Module<'_> {
    /// Where a resource is made, for the leak detector: `app.apeiron:12`.
    pub(crate) fn place_of(&self, span: Span) -> String {
        string_literal(&self.options.locate(span))
    }

    /// A global of the browser read as a value.
    pub(crate) fn host_value(name: &str) -> Option<&'static str> {
        match name {
            "document" => Some("document"),
            "storage" => Some("localStorage"),
            _ => None,
        }
    }

    /// A call of a global of the browser.
    pub(crate) fn host_call(&mut self, id: ExprId, name: &str, args: &[ExprId]) -> Option<String> {
        perasc_check::browser::global(name)?;
        let span = self.ast.expr(id).span;
        let values = self.exprs(args);
        let arg = |i: usize| {
            values
                .get(i)
                .cloned()
                .unwrap_or_else(|| "undefined".to_owned())
        };
        Some(match name {
            "fetch" => format!("fetch({})", arg(0)),
            "sleep" => {
                let sleep = self.helper("$sleep");
                format!("{sleep}({})", arg(0))
            }
            "now" => "performance.now()".to_owned(),
            "log" => format!("console.log({})", arg(0)),
            "setTimeout" | "setInterval" => {
                let timer = self.helper("$timer");
                let repeat = name == "setInterval";
                let at = self.place_of(span);
                format!("{timer}({repeat}, {}, {}, {at})", arg(0), arg(1))
            }
            _ => return None,
        })
    }

    /// `Group.new()`.
    pub(crate) fn group_new(&mut self, id: ExprId, base: ExprId, name: &Ident) -> Option<String> {
        let perasc_syntax::ast::ExprKind::Name(symbol) = self.ast.expr(base).kind else {
            return None;
        };
        if self.ast.name(symbol) != "Group"
            || self.ast.name(name.symbol) != "new"
            || self.bound(base).is_some()
        {
            return None;
        }
        let group = self.helper("$group");
        let at = self.place_of(self.ast.expr(id).span);
        Some(format!("{group}({at})"))
    }

    /// A method of a browser type: `me` is the receiver, `rest` the
    /// arguments, each already written.
    pub(crate) fn host_method(
        &mut self,
        id: ExprId,
        of: Host,
        name: &str,
        me: &str,
        rest: &[String],
    ) -> String {
        let arg = |i: usize| {
            rest.get(i)
                .cloned()
                .unwrap_or_else(|| "undefined".to_owned())
        };
        let span = self.ast.expr(id).span;
        match (of, name) {
            (_, "on") => {
                let on = self.helper("$on");
                let at = self.place_of(span);
                format!("{on}({me}, {}, {}, {at})", arg(0), arg(1))
            }
            (Host::Listener | Host::Timer | Host::Group, "release") => {
                let release = self.helper("$release");
                format!("{release}({me})")
            }
            (Host::Group, "add") => format!("{me}.add({})", arg(0)),
            (Host::Group, "slot") => format!("{me}.slot()"),
            (Host::Slot, "set") => format!("{me}.set({})", arg(0)),
            (Host::Slot, "clear") => format!("{me}.clear()"),
            // The DOM gives `null` where Peras has `None`, which is `undefined`.
            (
                Host::Document | Host::Element,
                "getElementById" | "querySelector" | "getAttribute",
            ) => {
                format!("({me}.{name}({}) ?? undefined)", arg(0))
            }
            (Host::Element, "addClass") => format!("{me}.classList.add({})", arg(0)),
            (Host::Element, "removeClass") => format!("{me}.classList.remove({})", arg(0)),
            (Host::Element, "toggleClass") => format!("{me}.classList.toggle({})", arg(0)),
            (Host::Element, "asInput") => {
                let cast = self.helper("$as");
                format!("{cast}({me}, \"HTMLInputElement\")")
            }
            (Host::Element, "asCanvas") => {
                let cast = self.helper("$as");
                format!("{cast}({me}, \"HTMLCanvasElement\")")
            }
            (Host::Canvas, "context2d") => format!("({me}.getContext(\"2d\") ?? undefined)"),
            (Host::Storage, "get") => format!("({me}.getItem({}) ?? undefined)", arg(0)),
            (Host::Storage, "set") => format!("{me}.setItem({}, {})", arg(0), arg(1)),
            (Host::Storage, "remove") => format!("{me}.removeItem({})", arg(0)),
            _ => format!("{me}.{name}({})", rest.join(", ")),
        }
    }

    /// A property of a browser type, as JavaScript names it.
    pub(crate) const fn host_field(of: Host, name: &str) -> &str {
        match (of, name.as_bytes()) {
            // `type` is a keyword of Peras.
            (Host::Event, b"kind") => "type",
            _ => name,
        }
    }

    /// Whether the variable declared at `span` holds a resource: its block
    /// releases it.
    pub(crate) fn owns_resource(&self, span: Span) -> bool {
        self.declared
            .get(&(span.start, span.end))
            .and_then(|&b| self.analysis.bindings.get(b))
            .is_some_and(|b| perasc_check::browser::is_resource(&b.ty))
    }
}
