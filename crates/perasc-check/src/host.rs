//! Checking the browser's types, members and globals ([`crate::browser`]),
//! in a managed module; in an owned one, each is refused (`E0242`).

use perasc_diag::Span;
use perasc_syntax::ast::{ExprId, ExprKind, Ident};

use crate::browser::{self, Global, Host, Member};
use crate::ty::{IntTy, Ty};
use crate::{Checker, errors};

impl Checker<'_> {
    /// Refuses the browser in an owned module.
    fn browser_here(&mut self, span: Span) {
        if !self.managed {
            self.report(errors::managed_only(span, "browser"));
        }
    }

    /// A global of the browser read as a value: `document`, `storage`.
    pub(crate) fn host_value(&mut self, name: &str, span: Span) -> Option<Ty> {
        let Some(Global::Value(ty)) = browser::global(name) else {
            return None;
        };
        self.browser_here(span);
        Some(ty)
    }

    /// A call of a global of the browser: `fetch(url)`, `setTimeout(ms, f)`.
    pub(crate) fn host_call(&mut self, name: &str, args: &[ExprId], span: Span) -> Option<Ty> {
        let global = browser::global(name)?;
        self.browser_here(span);
        Some(match global {
            Global::Value(ty) => {
                self.check_args(args, &[]);
                self.report(errors::invalid_operator(span, "()", &ty));
                Ty::Error
            }
            Global::Function(params, ret) => {
                self.arity(span, name, params.len(), args.len(), None);
                self.check_args(args, &params);
                ret
            }
            Global::Timer => {
                self.arity(span, name, 2, args.len(), None);
                if let Some(&delay) = args.first() {
                    self.expect(delay, &Ty::Int(IntTy::I32));
                }
                if let Some(&f) = args.get(1) {
                    self.arrow(f, &[], None);
                }
                for &extra in args.get(2..).unwrap_or_default() {
                    self.infer(extra, None);
                }
                Ty::Host(Host::Timer)
            }
        })
    }

    /// `Group.new()`, when `Group` names nothing of the module.
    pub(crate) fn group_new(
        &mut self,
        base: ExprId,
        name: &Ident,
        args: &[ExprId],
        span: Span,
    ) -> Option<Ty> {
        let ExprKind::Name(symbol) = self.ast.expr(base).kind else {
            return None;
        };
        if self.name(symbol) != "Group" || self.scopes.lookup(symbol).is_some() {
            return None;
        }
        self.browser_here(span);
        let member = self.name(name.symbol).to_owned();
        if member != "new" {
            self.unknown_member(&Ty::Host(Host::Group), &member, name.span);
            self.check_args(args, &[]);
            return Some(Ty::Error);
        }
        self.arity(span, "new", 0, args.len(), None);
        self.check_args(args, &[]);
        Some(Ty::Host(Host::Group))
    }

    /// A method of a browser type.
    pub(crate) fn host_method(
        &mut self,
        of: Host,
        name: &str,
        name_span: Span,
        args: &[ExprId],
        span: Span,
    ) -> Ty {
        // `element.on("click", (e) => …)`: a listener.
        if name == "on" && of.has_events() {
            self.arity(span, name, 2, args.len(), None);
            if let Some(&event) = args.first() {
                self.expect(event, &Ty::String);
            }
            if let Some(&f) = args.get(1) {
                self.arrow(f, &[Ty::Host(Host::Event)], None);
            }
            for &extra in args.get(2..).unwrap_or_default() {
                self.infer(extra, None);
            }
            return Ty::Host(Host::Listener);
        }
        // `group.add(listener)`: any resource.
        if name == "add" && of == Host::Group {
            self.arity(span, name, 1, args.len(), None);
            for (i, &arg) in args.iter().enumerate() {
                let ty = self.infer(arg, None);
                if i == 0 && !browser::is_resource(&ty) && ty != Ty::Error {
                    let at = self.ast.expr(arg).span;
                    self.report(errors::mismatch(at, &Ty::Host(Host::Listener), &ty, None));
                }
            }
            return Ty::Void;
        }
        match browser::member(of, name) {
            Some(Member::Method(params, ret)) => {
                self.arity(span, name, params.len(), args.len(), None);
                self.check_args(args, &params);
                ret
            }
            Some(Member::Field(ty, _)) => {
                self.check_args(args, &[]);
                self.report(errors::invalid_operator(name_span, "()", &ty));
                Ty::Error
            }
            None => {
                self.unknown_member(&Ty::Host(of), name, name_span);
                self.check_args(args, &[]);
                Ty::Error
            }
        }
    }

    /// A property of a browser type, read.
    pub(crate) fn host_field(&mut self, of: Host, name: &str, span: Span) -> Ty {
        if let Some(Member::Field(ty, _)) = browser::member(of, name) {
            ty
        } else {
            self.unknown_member(&Ty::Host(of), name, span);
            Ty::Error
        }
    }

    /// A property of a browser type, assigned: refused when it can only be
    /// read (`E0246`).
    pub(crate) fn host_field_written(&mut self, of: Host, name: &str, span: Span) {
        if let Some(Member::Field(_, false)) = browser::member(of, name) {
            self.report(errors::read_only(span, of.name(), name));
        }
    }
}
