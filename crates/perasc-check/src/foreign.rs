//! `extern from "lib" { … }`: JavaScript a managed module calls, declared
//! with Peras types ([decision 0010]). The checker trusts the declarations;
//! the generated code checks, at each call, every value JavaScript gives
//! back against them.
//!
//! [decision 0010]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0010-calling-javascript.md

use perasc_diag::Span;
use perasc_syntax::ast::{ExternDecl, ExternFn};

use crate::browser::holds_resource;
use crate::methods::TypeKey;
use crate::scope::Kind;
use crate::ty::{FnId, Ty};
use crate::{Checker, FnSig, errors};

/// A function of an `extern` block, as code generation needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternInfo {
    /// Its id among the module's functions.
    pub id: FnId,
    /// The module it comes from: `"date-fns"`, `"./chart.js"`.
    pub from: String,
    /// Its name, as JavaScript exports it, or as the method is called.
    pub name: String,
    /// For a method, the name of its type.
    pub method_of: Option<String>,
    /// Its parameters, `self` included for a method.
    pub params: Vec<Ty>,
    /// Its result.
    pub ret: Ty,
}

impl Checker<'_> {
    /// The types of an `extern` block: each a new opaque type.
    pub(crate) fn extern_types(&mut self, decl: &ExternDecl) {
        if !self.managed {
            return;
        }
        for name in &decl.types {
            let text = self.name(name.symbol).to_owned();
            let id = u32::try_from(self.opaque_count).unwrap_or(u32::MAX);
            self.opaque_count += 1;
            self.types
                .insert(text.clone(), crate::TypeDef::Plain(Ty::Opaque(id, text)));
        }
    }

    /// The functions of an `extern` block.
    pub(crate) fn extern_fns(&mut self, decl: &ExternDecl, span: Span) {
        if !self.managed {
            self.report(errors::managed_only(span, "extern"));
            return;
        }
        for f in &decl.fns {
            self.extern_fn(decl, f);
        }
    }

    fn extern_fn(&mut self, decl: &ExternDecl, f: &ExternFn) {
        self.flags.in_extern = true;
        self.extern_fn_inner(decl, f);
        self.flags.in_extern = false;
    }

    fn extern_fn_inner(&mut self, decl: &ExternDecl, f: &ExternFn) {
        let mut params = Vec::with_capacity(f.params.len());
        for p in &f.params {
            // A function type is allowed here only, at the top of a parameter.
            self.flags.callback_ok = true;
            let ty = self.resolve_type(&p.ty);
            self.flags.callback_ok = false;
            let inside = match &ty {
                Ty::Callback(ps, r) => ps.iter().chain(std::iter::once(&**r)).any(holds_resource),
                other => holds_resource(other),
            };
            if inside {
                let name = self.name(p.name.symbol).to_owned();
                self.report(errors::extern_decl(p.ty.span, "resource", &name));
            }
            params.push(ty);
        }
        let ret = f.ret.as_ref().map_or(Ty::Void, |t| self.resolve_type(t));
        let name = self.name(f.name.symbol).to_owned();
        if holds_resource(&ret) {
            let at = f.ret.as_ref().map_or(f.name.span, |t| t.span);
            self.report(errors::extern_decl(at, "resource", &name));
        }
        if f.is_async && !matches!(ret, Ty::Promise(_) | Ty::Error) {
            let at = f.ret.as_ref().map_or(f.name.span, |t| t.span);
            self.report(errors::extern_decl(at, "async", &name));
        }
        // `fn update(self: Chart, …)`: a method of the block's type `Chart`.
        let receiver = f
            .params
            .first()
            .filter(|p| self.name(p.name.symbol) == "self")
            .map(|p| (p.ty.span, params.first().cloned()));
        let method_of = match receiver {
            Some((_, Some(Ty::Opaque(id, type_name))))
                if decl.types.iter().any(|t| self.name(t.symbol) == type_name) =>
            {
                Some((id, type_name))
            }
            Some((at, _)) => {
                self.report(errors::extern_decl(at, "self", &name));
                None
            }
            None => None,
        };
        let id = FnId(self.fns.len());
        self.fns.push(FnSig {
            name: name.clone(),
            name_span: f.name.span,
            generics: Vec::new(),
            params: params.clone(),
            ret: ret.clone(),
            conventions: f.params.iter().map(|p| p.convention).collect(),
            self_ty: method_of
                .as_ref()
                .map(|(id, type_name)| Ty::Opaque(*id, type_name.clone())),
            has_self: method_of.is_some(),
        });
        match &method_of {
            Some((type_id, _)) => {
                self.methods
                    .insert((TypeKey::Opaque(*type_id), name.clone()), id);
            }
            None => self.declare(
                f.name.symbol,
                Some(f.name.span),
                Kind::Fn(id),
                Ty::Fn(id, name.clone()),
            ),
        }
        self.externs.push(ExternInfo {
            id,
            from: decl.from.clone(),
            name,
            method_of: method_of.map(|(_, type_name)| type_name),
            params,
            ret,
        });
    }
}
