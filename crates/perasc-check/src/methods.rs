//! Methods, interfaces, type aliases, tuples and module constants.
//!
//! An `impl` gives a structure or an enumeration of the module its methods:
//! functions whose first parameter, `self`, is the value before the dot. A
//! method without `self` is called on the type, `Point.origin()`. Any method
//! can also be called that way with its `self` written first.
//!
//! An interface lists methods; `impl Shape for Circle` provides them, checked
//! against the interface (`E0235`, `E0236`). An interface is a constraint:
//! in `fn total<T: Shape>(…)`, a value of type `T` has the interface's
//! methods, and each instance of the function calls those of its type.
//!
//! A type alias is another name for a type, interchangeable with it; a type
//! that must not mix with its representation is a structure, which costs
//! nothing since structures are kept as their scalars. A tuple is an
//! instance of a generic structure whose fields are `0`, `1`, …

use perasc_diag::Span;
use perasc_syntax::ast::{
    AliasDecl, Convention, ExprId, ExprKind, FnDecl, Ident, ImplDecl, InterfaceDecl,
};

use crate::generics::{GenericParam, GenericUse};
use crate::scope::Kind;
use crate::ty::{EnumId, FnId, StructId, Ty};
use crate::{Checker, MethodCall, StructDef, TypeDef, errors};

/// The index `Self` takes as a type parameter in an interface's methods.
const SELF: u32 = u32::MAX;

/// A type that can have methods: a structure or an enumeration of the
/// module, every instance of a generic one sharing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TypeKey {
    Struct(StructId),
    Enum(EnumId),
}

/// A type alias, resolved when first used.
pub(crate) struct AliasDef<'a> {
    pub decl: &'a AliasDecl,
    pub generics: Vec<GenericParam>,
    pub ty: Option<Ty>,
    /// Being resolved: met again, the alias contains itself.
    pub resolving: bool,
}

/// An interface and the methods it requires.
pub(crate) struct InterfaceDef {
    pub name: String,
    pub methods: Vec<Required>,
}

/// A method an interface requires, `Self` standing for the type.
#[derive(Clone)]
pub(crate) struct Required {
    pub name: String,
    pub span: Span,
    /// `self` first.
    pub params: Vec<Ty>,
    pub conventions: Vec<Convention>,
    pub ret: Ty,
}

/// A call being checked: the call, its arguments, the whole call, and the
/// type expected of it.
pub(crate) struct CallSite<'u> {
    pub call: ExprId,
    pub args: &'u [ExprId],
    pub span: Span,
    pub expected: Option<&'u Ty>,
}

/// What a method call reaches, as the checker records it.
#[derive(Debug, Clone)]
pub(crate) enum Target {
    Fn {
        id: FnId,
        receiver: bool,
    },
    /// A method of an interface on a value of a type parameter.
    Interface {
        interface: usize,
        method: String,
        receiver: Ty,
    },
}

impl Target {
    /// The record code generation reads.
    pub fn public(self) -> MethodCall {
        match self {
            Self::Fn { id, receiver } => MethodCall::Fn { id, receiver },
            Self::Interface { .. } => MethodCall::Interface,
        }
    }
}

/// `ty` with `Self` replaced by `with`.
fn replace_self(ty: &Ty, with: &Ty) -> Ty {
    match ty {
        Ty::Param(SELF, _) => with.clone(),
        Ty::Array(t) => Ty::Array(Box::new(replace_self(t, with))),
        Ty::Option(t) => Ty::Option(Box::new(replace_self(t, with))),
        Ty::Result(a, b) => Ty::Result(
            Box::new(replace_self(a, with)),
            Box::new(replace_self(b, with)),
        ),
        other => other.clone(),
    }
}

impl<'a> Checker<'a> {
    // --- Type aliases -------------------------------------------------------

    pub(crate) fn register_alias(&mut self, decl: &'a AliasDecl) {
        let name = self.name(decl.name.symbol).to_owned();
        let generics = self.generic_params(&decl.generics);
        let index = self.aliases.len();
        self.aliases.push(AliasDef {
            decl,
            generics,
            ty: None,
            resolving: false,
        });
        self.types.insert(name, TypeDef::Alias(index));
    }

    /// The type an alias names, in terms of its own type parameters.
    pub(crate) fn alias_type(&mut self, index: usize) -> Ty {
        let Some(alias) = self.aliases.get_mut(index) else {
            return Ty::Error;
        };
        if let Some(ty) = &alias.ty {
            return ty.clone();
        }
        let decl = alias.decl;
        if alias.resolving {
            let name = self.name(decl.name.symbol);
            self.report(errors::alias_cycle(decl.name.span, name));
            return Ty::Error;
        }
        alias.resolving = true;
        let generics = alias.generics.clone();
        let outer = std::mem::replace(&mut self.generics, generics);
        let outer_self = self.self_ty.take();
        let ty = self.resolve_type(&decl.ty);
        self.generics = outer;
        self.self_ty = outer_self;
        if let Some(alias) = self.aliases.get_mut(index) {
            alias.ty = Some(ty.clone());
            alias.resolving = false;
        }
        ty
    }

    // --- Tuples -------------------------------------------------------------

    /// `(A, B)`: an instance of the generic structure of tuples of that size.
    pub(crate) fn tuple_type(&mut self, items: Vec<Ty>) -> Ty {
        let n = items.len();
        let template = if let Some(&id) = self.tuple_templates.get(&n) {
            id
        } else {
            let id = StructId(self.structs.len());
            let generics: Vec<GenericParam> = (0..n)
                .map(|i| GenericParam {
                    name: format!("T{i}"),
                    span: Span::new(0, 0),
                    bounds: Vec::new(),
                    interfaces: Vec::new(),
                    unknown: false,
                    insert_at: 0,
                })
                .collect();
            let fields = Self::identity(&generics)
                .into_iter()
                .enumerate()
                .map(|(i, t)| (i.to_string(), t))
                .collect();
            self.structs.push(StructDef {
                name: "()".to_owned(),
                fields,
                generics,
                origin: None,
                span: Span::new(0, 0),
            });
            self.tuple_templates.insert(n, id);
            id
        };
        self.struct_instance(template, items)
    }

    /// `(a, b)`, its elements guided by the tuple type expected.
    pub(crate) fn tuple_expr(&mut self, items: &[ExprId], expected: Option<&Ty>) -> Ty {
        let wanted: Option<Vec<Ty>> = match expected {
            Some(Ty::Struct(id, _)) => self
                .structs
                .get(id.0)
                .filter(|s| {
                    s.origin.as_ref().is_some_and(|(t, args)| {
                        args.len() == items.len()
                            && self.tuple_templates.get(&items.len()) == Some(t)
                    })
                })
                .map(|s| s.fields.iter().map(|(_, t)| t.clone()).collect()),
            _ => None,
        };
        let mut types = Vec::with_capacity(items.len());
        for (i, &item) in items.iter().enumerate() {
            match wanted.as_ref().and_then(|w| w.get(i)) {
                Some(want) => {
                    let want = want.clone();
                    self.expect(item, &want);
                    types.push(want);
                }
                None => types.push(self.infer(item, None).defaulted()),
            }
        }
        self.tuple_type(types)
    }

    // --- Interfaces ---------------------------------------------------------

    pub(crate) fn register_interface(&mut self, decl: &InterfaceDecl) {
        let name = self.name(decl.name.symbol).to_owned();
        let index = self.interfaces.len();
        self.interfaces.push(InterfaceDef {
            name: name.clone(),
            methods: Vec::new(),
        });
        self.types.insert(name, TypeDef::Interface(index));
    }

    /// The methods an interface requires, `Self` standing for the type.
    pub(crate) fn interface_methods(&mut self, decl: &InterfaceDecl) {
        let name = self.name(decl.name.symbol);
        let Some(index) = self.interfaces.iter().position(|i| i.name == name) else {
            return;
        };
        self.self_ty = Some(Ty::Param(SELF, "Self".to_owned()));
        let mut methods: Vec<Required> = Vec::new();
        for method in &decl.methods {
            let text = self.name(method.name.symbol).to_owned();
            if let Some(previous) = methods.iter().find(|m| m.name == text) {
                let d = errors::duplicate(method.name.span, &text, previous.span);
                self.report(d);
                continue;
            }
            let params = method
                .params
                .iter()
                .map(|p| self.resolve_type(&p.ty))
                .collect();
            let ret = method
                .ret
                .as_ref()
                .map_or(Ty::Void, |t| self.resolve_type(t));
            if !method.params.first().is_some_and(|p| p.is_self) {
                self.report(errors::interface_without_self(method.name.span, &text));
            }
            methods.push(Required {
                name: text,
                span: method.name.span,
                params,
                conventions: method.params.iter().map(|p| p.convention).collect(),
                ret,
            });
        }
        self.self_ty = None;
        if let Some(def) = self.interfaces.get_mut(index) {
            def.methods = methods;
        }
    }

    /// The interface of this name.
    pub(crate) fn interface_named(&self, name: &str) -> Option<usize> {
        match self.types.get(name) {
            Some(TypeDef::Interface(index)) => Some(*index),
            _ => None,
        }
    }

    /// Whether a type implements an interface: a type parameter when its
    /// constraints say so, a structure or an enumeration by an `impl`.
    pub(crate) fn implements(&self, ty: &Ty, interface: usize) -> bool {
        match ty {
            Ty::Error => true,
            Ty::Param(i, _) => self
                .param_of(*i)
                .is_some_and(|p| p.unknown || p.interfaces.contains(&interface)),
            _ => self
                .type_key(ty)
                .is_some_and(|k| self.implements.contains(&(k, interface))),
        }
    }

    // --- Implementations ----------------------------------------------------

    /// The template a type's methods belong to.
    pub(crate) fn type_key(&self, ty: &Ty) -> Option<TypeKey> {
        match ty {
            Ty::Struct(id, _) => {
                let template = self
                    .structs
                    .get(id.0)
                    .and_then(|s| s.origin.as_ref())
                    .map_or(*id, |(t, _)| *t);
                Some(TypeKey::Struct(template))
            }
            Ty::Enum(id, _) => {
                let template = self
                    .enums
                    .get(id.0)
                    .and_then(|e| e.origin.as_ref())
                    .map_or(*id, |(t, _)| *t);
                Some(TypeKey::Enum(template))
            }
            _ => None,
        }
    }

    /// The methods of an `impl`, registered as functions of its type.
    pub(crate) fn register_impl(&mut self, decl: &ImplDecl) {
        self.generics = self.generic_params(&decl.generics);
        let target = self.resolve_type(&decl.target);
        let key = self.type_key(&target);
        if key.is_none() && target != Ty::Error {
            self.report(errors::no_methods(decl.target.span, &target));
        }
        // Every type parameter of the `impl` must come from its type: a call
        // gives the type, never the parameter.
        let mut found: Vec<Option<Ty>> = vec![None; self.generics.len()];
        self.bind(&target, &target, &mut found);
        for (param, slot) in self.generics.clone().iter().zip(found) {
            if slot.is_none() && target != Ty::Error {
                let name = target.to_string();
                self.report(errors::cannot_infer(
                    decl.target.span,
                    &param.name,
                    &name,
                    param.span,
                ));
            }
        }
        self.generics.clear();
        for method in &decl.methods {
            let id = self.register_fn(method, Some(&target));
            let Some(key) = key else { continue };
            let name = self.name(method.name.symbol).to_owned();
            if let Some(&previous) = self.methods.get(&(key, name.clone())) {
                let at = self.fns.get(previous.0).map(|f| f.name_span);
                if let Some(at) = at {
                    self.report(errors::duplicate(method.name.span, &name, at));
                }
                continue;
            }
            self.methods.insert((key, name), id);
        }
    }

    /// `impl Interface for Type`: every method the interface requires, with
    /// its signature, and no other.
    pub(crate) fn check_implementation(&mut self, decl: &ImplDecl) {
        let Some(interface) = decl.interface else {
            return;
        };
        let text = self.name(interface.symbol);
        let Some(index) = self.interface_named(text) else {
            let names: Vec<String> = self.interfaces.iter().map(|i| i.name.clone()).collect();
            let suggestion =
                crate::scope::suggest(text, names.iter().map(String::as_str)).map(str::to_owned);
            self.report(errors::unknown_constraint(
                interface.span,
                text,
                suggestion.as_deref(),
            ));
            return;
        };
        // The target again, in the terms of the `impl`'s parameters.
        self.generics = self.generic_params(&decl.generics);
        let target = self.resolve_type(&decl.target);
        self.generics.clear();
        let Some(key) = self.type_key(&target) else {
            return;
        };
        let required = self
            .interfaces
            .get(index)
            .map(|i| i.methods.clone())
            .unwrap_or_default();
        let interface_name = self
            .interfaces
            .get(index)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let mut missing = Vec::new();
        for want in &required {
            let Some(method) = decl
                .methods
                .iter()
                .find(|m| self.name(m.name.symbol) == want.name)
            else {
                missing.push(want.name.clone());
                continue;
            };
            let Some(&id) = self.methods.get(&(key, want.name.clone())) else {
                continue;
            };
            self.compare_signature(method, id, want, &target, &interface_name);
        }
        if !missing.is_empty() {
            self.report(errors::missing_methods(
                decl.target.span,
                &interface_name,
                &target,
                &missing,
            ));
        }
        for method in &decl.methods {
            let name = self.name(method.name.symbol);
            if !required.iter().any(|r| r.name == name) {
                self.report(errors::extra_method(
                    method.name.span,
                    name,
                    &interface_name,
                ));
            }
        }
        self.implements.insert((key, index));
    }

    /// `E0236` when a method differs from what the interface requires.
    fn compare_signature(
        &mut self,
        method: &FnDecl,
        id: FnId,
        want: &Required,
        target: &Ty,
        interface: &str,
    ) {
        let Some(sig) = self.fns.get(id.0) else {
            return;
        };
        let params: Vec<Ty> = want
            .params
            .iter()
            .map(|t| replace_self(t, target))
            .collect();
        let ret = replace_self(&want.ret, target);
        let same = sig.params == params && sig.ret == ret && sig.conventions == want.conventions;
        if !same {
            let expected = self.signature_text(&want.name, &params, &want.conventions, &ret);
            self.report(errors::signature_mismatch(
                method.name.span,
                &want.name,
                interface,
                &expected,
                want.span,
            ));
        }
    }

    /// `fn area(self): f64`, for messages.
    fn signature_text(
        &self,
        name: &str,
        params: &[Ty],
        conventions: &[Convention],
        ret: &Ty,
    ) -> String {
        let _ = self;
        let parts: Vec<String> = params
            .iter()
            .zip(conventions)
            .enumerate()
            .map(|(i, (t, c))| {
                let convention = match c {
                    Convention::Let => "",
                    Convention::Inout => "inout ",
                    Convention::Sink => "sink ",
                };
                if i == 0 {
                    format!("{convention}self")
                } else {
                    format!("p{i}: {convention}{t}")
                }
            })
            .collect();
        let ret = if *ret == Ty::Void {
            String::new()
        } else {
            format!(": {ret}")
        };
        format!("fn {name}({}){ret}", parts.join(", "))
    }

    // --- Calls ----------------------------------------------------------------

    /// `Point.origin()`, or `Point.norm(p)`: a method called on its type.
    pub(crate) fn static_method(
        &mut self,
        at: &CallSite<'_>,
        base: ExprId,
        name: &Ident,
    ) -> Option<Ty> {
        let (call, args) = (at.call, at.args);
        let ExprKind::Name(symbol) = self.ast.expr(base).kind else {
            return None;
        };
        let index = self.scopes.lookup(symbol)?;
        let key = match self.scopes.bindings.get(index)?.kind {
            Kind::Struct(id) => TypeKey::Struct(id),
            Kind::Enum(id) => TypeKey::Enum(id),
            _ => return None,
        };
        let text = self.name(name.symbol).to_owned();
        let Some(&id) = self.methods.get(&(key, text.clone())) else {
            if let TypeKey::Struct(_) = key {
                // A structure has no variants: the member must be a method.
                if let Some(binding) = self.scopes.bindings.get_mut(index) {
                    binding.used = true;
                }
                self.resolved.insert(base, index);
                let ty = self
                    .scopes
                    .bindings
                    .get(index)
                    .map_or(Ty::Error, |b| b.ty.clone());
                self.unknown_member(&ty, &text, name.span);
                self.check_args(args, &[]);
                return Some(Ty::Error);
            }
            return None;
        };
        if let Some(binding) = self.scopes.bindings.get_mut(index) {
            binding.used = true;
        }
        self.resolved.insert(base, index);
        self.method_calls.insert(
            call,
            Target::Fn {
                id,
                receiver: false,
            },
        );
        Some(self.call_method(at, id, &text, None))
    }

    /// `value.method(args)` for a method of the module, or of an interface
    /// when the value is of a type parameter.
    pub(crate) fn user_method(
        &mut self,
        at: &CallSite<'_>,
        base: ExprId,
        receiver: &Ty,
        name: &Ident,
    ) -> Option<Ty> {
        let (call, args) = (at.call, at.args);
        let text = self.name(name.symbol).to_owned();
        if let Ty::Param(i, _) = receiver {
            return self.interface_call(at, base, *i, receiver, &text);
        }
        let key = self.type_key(receiver)?;
        let &id = self.methods.get(&(key, text.clone()))?;
        let has_self = self.fns.get(id.0).is_some_and(|f| f.has_self);
        if !has_self {
            self.unknown_member(receiver, &text, name.span);
            self.check_args(args, &[]);
            return Some(Ty::Error);
        }
        self.method_calls
            .insert(call, Target::Fn { id, receiver: true });
        self.mutable_receiver(
            base,
            self.fns
                .get(id.0)
                .and_then(|f| f.conventions.first().copied()),
        );
        let base_span = self.ast.expr(base).span;
        Some(self.call_method(at, id, &text, Some((receiver, base_span))))
    }

    /// A method of an interface on a value of a type parameter.
    fn interface_call(
        &mut self,
        at: &CallSite<'_>,
        base: ExprId,
        param: u32,
        receiver: &Ty,
        text: &str,
    ) -> Option<Ty> {
        let (call, args, span) = (at.call, at.args, at.span);
        let interfaces = self.param_of(param)?.interfaces.clone();
        let (interface, want) = interfaces.iter().find_map(|&i| {
            let def = self.interfaces.get(i)?;
            def.methods
                .iter()
                .find(|m| m.name == text)
                .map(|m| (i, m.clone()))
        })?;
        let params: Vec<Ty> = want
            .params
            .iter()
            .map(|t| replace_self(t, receiver))
            .collect();
        self.arity(
            span,
            text,
            params.len().saturating_sub(1),
            args.len(),
            Some(want.span),
        );
        self.mutable_receiver(base, want.conventions.first().copied());
        self.check_args(args, params.get(1..).unwrap_or(&[]));
        self.method_calls.insert(
            call,
            Target::Interface {
                interface,
                method: text.to_owned(),
                receiver: receiver.clone(),
            },
        );
        Some(replace_self(&want.ret, receiver))
    }

    /// A method taking `inout self` changes its receiver: it must be a
    /// variable that can change.
    fn mutable_receiver(&mut self, base: ExprId, convention: Option<Convention>) {
        if convention == Some(Convention::Inout)
            && let Some(root) = self.root(base)
        {
            let span = self.ast.expr(base).span;
            self.check_mutable(root, span);
        }
    }

    /// A call to a method of the module, its receiver given or not.
    fn call_method(
        &mut self,
        at: &CallSite<'_>,
        id: FnId,
        name: &str,
        receiver: Option<(&Ty, Span)>,
    ) -> Ty {
        let (call, args, span, expected) = (at.call, at.args, at.span, at.expected);
        let Some(sig) = self.fns.get(id.0) else {
            return Ty::Error;
        };
        let (params, ret, generics, declared) = (
            sig.params.clone(),
            sig.ret.clone(),
            sig.generics.clone(),
            sig.name_span,
        );
        let skip = usize::from(receiver.is_some());
        self.arity(
            span,
            name,
            params.len().saturating_sub(skip),
            args.len(),
            Some(declared),
        );
        if generics.is_empty() {
            if let (Some((given, at)), Some(want)) = (receiver, params.first())
                && given != want
                && *given != Ty::Error
            {
                self.report(errors::mismatch(at, want, given, None));
            }
            self.check_args(args, params.get(skip..).unwrap_or(&[]));
            return ret;
        }
        let use_ = GenericUse {
            name,
            span,
            generics: &generics,
            params: &params,
            ret: &ret,
            receiver,
        };
        let (types, ret) = self.infer_generic(&use_, None, args, expected);
        self.calls.insert(call, (id, types));
        ret
    }

    // --- Module constants -----------------------------------------------------

    /// `E0233` unless the value of a module `const` is known when compiling:
    /// literals, other constants, operators, arrays, tuples, structures,
    /// options and variants of them.
    pub(crate) fn check_constant(&mut self, id: ExprId) {
        let expr = self.ast.expr(id);
        match &expr.kind {
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Error => {}
            ExprKind::Paren(inner) => self.check_constant(*inner),
            ExprKind::Unary { operand, .. } => self.check_constant(*operand),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.check_constant(*lhs);
                self.check_constant(*rhs);
            }
            ExprKind::Array(items) | ExprKind::Tuple(items) => {
                for &item in items {
                    self.check_constant(item);
                }
            }
            ExprKind::Name(_) => {
                let kind = self
                    .resolved
                    .get(&id)
                    .and_then(|&b| self.scopes.bindings.get(b))
                    .map(|b| b.kind.clone());
                if !matches!(
                    kind,
                    Some(Kind::Global { mutable: false, .. } | Kind::None) | None
                ) {
                    self.report(errors::not_constant(expr.span));
                }
            }
            // `Shape.Empty`
            ExprKind::Field { base, .. } if self.names_enum(*base) => {}
            ExprKind::Call { callee, args, .. } => {
                let constructor = match &self.ast.expr(*callee).kind {
                    ExprKind::Name(_) => matches!(
                        self.resolved
                            .get(callee)
                            .and_then(|&b| self.scopes.bindings.get(b))
                            .map(|b| &b.kind),
                        Some(Kind::Struct(_) | Kind::Some | Kind::Ok | Kind::Err)
                    ),
                    ExprKind::Field { base, .. } => {
                        self.names_enum(*base) && !self.method_calls.contains_key(&id)
                    }
                    _ => false,
                };
                if constructor {
                    for &arg in args {
                        self.check_constant(arg);
                    }
                } else {
                    self.report(errors::not_constant(expr.span));
                }
            }
            _ => self.report(errors::not_constant(expr.span)),
        }
    }

    /// Whether an expression names an enumeration.
    fn names_enum(&self, base: ExprId) -> bool {
        matches!(
            self.resolved
                .get(&base)
                .and_then(|&b| self.scopes.bindings.get(b))
                .map(|b| &b.kind),
            Some(Kind::Enum(_))
        )
    }

    /// The method names of a type, for suggestions.
    pub(crate) fn method_names(&self, ty: &Ty) -> Vec<String> {
        let Some(key) = self.type_key(ty) else {
            return Vec::new();
        };
        self.methods
            .keys()
            .filter(|(k, _)| *k == key)
            .map(|(_, n)| n.clone())
            .collect()
    }
}
