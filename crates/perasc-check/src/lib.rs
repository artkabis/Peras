//! Name, type, ownership, resource and stack-budget checks for the Peras compiler.
//!
//! Names and types are checked in one walk over the tree: each name is resolved
//! and typed where it is used. A name or type already reported as wrong becomes
//! [`ty::Ty::Error`], compatible with everything, so one mistake gives one error.
//! The walk runs only on a tree without syntax errors.

pub mod builtins;
mod errors;
mod expr;
mod generics;
mod matching;
mod methods;
mod owner;
mod scope;
mod stmt;
pub mod ty;

use std::collections::{HashMap, HashSet};

use perasc_diag::{Diagnostic, Span};
use perasc_syntax::ast::{
    Ast, Convention, EnumDecl, ExprId, ExprKind, FnDecl, ItemKind, Local, Stmt, StmtKind,
    StructDecl, Symbol, Type, TypeKind,
};

use crate::generics::GenericParam;
use crate::scope::{Binding, Declared, Kind, Scopes};
use crate::ty::{EnumId, FloatTy, FnId, IntTy, StructId, Ty};

/// Checks names and types of a parsed module, returning every error sorted by position.
#[must_use]
pub fn check(ast: &Ast, source: &str) -> Vec<Diagnostic> {
    analyze(ast, source).diagnostics
}

/// What the checker learned about a module.
#[derive(Debug, Clone)]
pub struct Analysis {
    /// Every error, sorted by position.
    pub diagnostics: Vec<Diagnostic>,
    /// The final type of each expression: literals have the type the context gave
    /// them, or `i32` and `f64` when nothing did.
    pub types: HashMap<ExprId, Ty>,
    /// Every structure, indexed by its [`StructId`]: its name
    /// and its fields in order.
    pub structs: Vec<(String, Vec<(String, Ty)>)>,
    /// The variables read where their heap value moves out of them: an
    /// assignment, a `return`, a `sink` argument… Any other read borrows.
    pub moves: HashSet<ExprId>,
    /// Every enumeration, indexed by its [`EnumId`]: its name, and its
    /// variants with the types of their data.
    pub enums: Vec<(String, Vec<Variant>)>,
    /// The number of type parameters of each function, by [`FnId`]: zero
    /// when it is not generic.
    pub fn_generics: Vec<usize>,
    /// Every instance of a generic function the module needs: the function
    /// and its type arguments, all concrete, in the order of the source.
    pub instances: Vec<(FnId, Vec<Ty>)>,
    /// The type arguments of each call to a generic function, in terms of
    /// the type parameters of the function that calls it.
    pub calls: HashMap<ExprId, (FnId, Vec<Ty>)>,
    /// The type of each type written in source, by its span.
    pub written: HashMap<(u32, u32), Ty>,
    /// For each structure made from a generic one: that one, and the type
    /// arguments.
    pub struct_origins: Vec<Option<(StructId, Vec<Ty>)>>,
    /// The structure made from a generic one for these type arguments.
    pub struct_instances: HashMap<(StructId, Vec<Ty>), StructId>,
    /// For each enumeration made from a generic one: that one, and the type
    /// arguments.
    pub enum_origins: Vec<Option<(EnumId, Vec<Ty>)>>,
    /// The enumeration made from a generic one for these type arguments.
    pub enum_instances: HashMap<(EnumId, Vec<Ty>), EnumId>,
    /// The function each method call reaches, by the call.
    pub method_calls: HashMap<ExprId, MethodCall>,
    /// For a call of an interface's method in a generic function: the
    /// method each instance of that function calls, by the call and the
    /// instance's type arguments, with the method's own type arguments.
    pub dyn_targets: HashMap<(ExprId, Vec<Ty>), (FnId, Vec<Ty>)>,
}

/// What a method call reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodCall {
    /// A method of the module: `p.norm()`, with `receiver`, or
    /// `Point.origin()`, without.
    Fn {
        /// The method.
        id: FnId,
        /// Whether the value before the dot is its `self`.
        receiver: bool,
    },
    /// A method of an interface, on a value of a type parameter: the
    /// method of each instance is in [`Analysis::dyn_targets`].
    Interface,
}

/// A variant of an enumeration: its name, and the types of its data.
pub type Variant = (String, Vec<Ty>);

/// Checks a module and keeps the type of every expression, for code generation.
#[must_use]
pub fn analyze(ast: &Ast, source: &str) -> Analysis {
    let mut checker = Checker::new(ast, source);
    checker.module();
    // Ownership needs correct types: one wrong type would give false moves.
    if checker.diagnostics.is_empty() {
        owner::Owner::new(&mut checker).module();
    }
    let (instances, dyn_targets) = if checker.diagnostics.is_empty() {
        checker.monomorphize()
    } else {
        (Vec::new(), HashMap::new())
    };
    let fn_generics = checker.fns.iter().map(|f| f.generics.len()).collect();
    let struct_origins = checker.structs.iter().map(|s| s.origin.clone()).collect();
    let enum_origins = checker.enums.iter().map(|e| e.origin.clone()).collect();
    let mut diagnostics = checker.diagnostics;
    diagnostics.sort_by_key(|d| d.primary_span().map(|s| s.start));
    let types = checker
        .expr_types
        .into_iter()
        .map(|(id, ty)| (id, ty.defaulted()))
        .collect();
    let structs = checker
        .structs
        .into_iter()
        .map(|s| (s.name, s.fields))
        .collect();
    Analysis {
        diagnostics,
        types,
        structs,
        fn_generics,
        instances,
        calls: checker.calls,
        written: checker
            .written
            .into_iter()
            .map(|(span, ty)| (span, ty.defaulted()))
            .collect(),
        struct_origins,
        struct_instances: checker.struct_instances,
        enum_origins,
        enum_instances: checker.enum_instances,
        method_calls: checker
            .method_calls
            .into_iter()
            .map(|(id, call)| (id, call.public()))
            .collect(),
        dyn_targets,
        moves: checker.moves,
        enums: checker
            .enums
            .into_iter()
            .map(|e| (e.name, e.variants))
            .collect(),
    }
}

/// A structure's fields, in order.
struct StructDef {
    name: String,
    fields: Vec<(String, Ty)>,
    /// Its type parameters, when it is generic.
    generics: Vec<GenericParam>,
    /// The generic structure it is made from, and the type arguments.
    origin: Option<(StructId, Vec<Ty>)>,
    /// Its name, where it is declared.
    span: Span,
}

/// A function's signature.
struct FnSig {
    name: String,
    name_span: Span,
    generics: Vec<GenericParam>,
    params: Vec<Ty>,
    ret: Ty,
    /// How it takes each parameter.
    conventions: Vec<Convention>,
    /// For a method: the type of its `impl`, which `Self` names.
    self_ty: Option<Ty>,
    /// For a method: whether its first parameter is `self`.
    has_self: bool,
}

/// A generic structure or enumeration, which a type names with arguments.
#[derive(Clone, Copy)]
enum Template {
    Struct(StructId),
    Enum(EnumId),
}

/// What a type name refers to.
#[derive(Clone)]
enum TypeDef {
    Plain(Ty),
    Array,
    Option,
    Result,
    Generic(Template, usize),
    /// `type Name = …`, by its index in [`Checker::aliases`].
    Alias(usize),
    /// An interface, by its index in [`Checker::interfaces`]: a constraint,
    /// not a type.
    Interface(usize),
}

struct Checker<'a> {
    ast: &'a Ast,
    source: &'a str,
    diagnostics: Vec<Diagnostic>,
    scopes: Scopes,
    types: HashMap<String, TypeDef>,
    structs: Vec<StructDef>,
    fns: Vec<FnSig>,
    /// Return type of the function being checked; `None` when not annotated.
    ret: Option<(Ty, Option<Span>)>,
    /// Types of the values returned so far, to infer a missing return type.
    returned: Vec<Ty>,
    /// Type of each expression checked so far.
    expr_types: HashMap<ExprId, Ty>,
    /// The binding each name expression resolves to.
    resolved: HashMap<ExprId, usize>,
    /// The binding each variable or parameter declares, by the start of its name.
    declared: HashMap<u32, usize>,
    /// See [`Analysis::moves`].
    moves: HashSet<ExprId>,
    enums: Vec<EnumDef>,
    /// The type parameters of the declaration being checked.
    generics: Vec<GenericParam>,
    struct_instances: generics::Instances<StructId>,
    enum_instances: generics::Instances<EnumId>,
    /// See [`Analysis::calls`].
    calls: HashMap<ExprId, (FnId, Vec<Ty>)>,
    /// See [`Analysis::written`].
    written: HashMap<(u32, u32), Ty>,
    /// Whether every generic structure and enumeration has its fields.
    templates_ready: bool,
    /// Where an endless instantiation was reported, once each.
    endless_reported: HashSet<u32>,
    /// The type aliases, resolved when first used.
    aliases: Vec<methods::AliasDef<'a>>,
    /// The interfaces, with the methods they require.
    interfaces: Vec<methods::InterfaceDef>,
    /// The methods of each type, by its generic template and their name.
    methods: HashMap<(methods::TypeKey, String), FnId>,
    /// The interfaces each type implements.
    implements: HashSet<(methods::TypeKey, usize)>,
    /// See [`Analysis::method_calls`]; the receiver of an interface's method.
    method_calls: HashMap<ExprId, methods::Target>,
    /// The type `Self` names here: the type of the `impl`, or the type that
    /// meets the interface whose methods are being declared.
    self_ty: Option<Ty>,
    /// The generic structure of each size of tuple.
    tuple_templates: HashMap<usize, StructId>,
}

/// An enumeration's variants, in order, each with the types of its data.
struct EnumDef {
    name: String,
    variants: Vec<(String, Vec<Ty>)>,
    generics: Vec<GenericParam>,
    origin: Option<(EnumId, Vec<Ty>)>,
    span: Span,
}

impl<'a> Checker<'a> {
    fn new(ast: &'a Ast, source: &'a str) -> Self {
        let mut types = HashMap::new();
        for int in IntTy::ALL {
            types.insert(int.name().to_owned(), TypeDef::Plain(Ty::Int(int)));
        }
        for float in [FloatTy::F32, FloatTy::F64] {
            types.insert(float.name().to_owned(), TypeDef::Plain(Ty::Float(float)));
        }
        types.insert("bool".to_owned(), TypeDef::Plain(Ty::Bool));
        types.insert("String".to_owned(), TypeDef::Plain(Ty::String));
        types.insert("void".to_owned(), TypeDef::Plain(Ty::Void));
        types.insert("Array".to_owned(), TypeDef::Array);
        types.insert("Option".to_owned(), TypeDef::Option);
        types.insert("Result".to_owned(), TypeDef::Result);
        Self {
            ast,
            source,
            diagnostics: Vec::new(),
            scopes: Scopes::default(),
            types,
            structs: Vec::new(),
            fns: Vec::new(),
            ret: None,
            returned: Vec::new(),
            expr_types: HashMap::new(),
            resolved: HashMap::new(),
            declared: HashMap::new(),
            moves: HashSet::new(),
            enums: Vec::new(),
            generics: Vec::new(),
            struct_instances: HashMap::new(),
            enum_instances: HashMap::new(),
            calls: HashMap::new(),
            written: HashMap::new(),
            templates_ready: false,
            endless_reported: HashSet::new(),
            aliases: Vec::new(),
            interfaces: Vec::new(),
            methods: HashMap::new(),
            implements: HashSet::new(),
            method_calls: HashMap::new(),
            self_ty: None,
            tuple_templates: HashMap::new(),
        }
    }

    fn report(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    fn name(&self, symbol: Symbol) -> &'a str {
        self.ast.name(symbol)
    }

    fn slice(&self, span: Span) -> &'a str {
        let start = usize::try_from(span.start).unwrap_or(0);
        let end = usize::try_from(span.end).unwrap_or(0);
        self.source.get(start..end).unwrap_or("")
    }

    /// Declares a binding, reporting duplicates and shadowing.
    fn declare(&mut self, symbol: Symbol, span: Option<Span>, kind: Kind, ty: Ty) {
        let name = self.name(symbol).to_owned();
        let binding = Binding {
            symbol,
            name: name.clone(),
            span,
            kind,
            ty,
            used: false,
        };
        let outcome = self.scopes.declare(binding);
        let Some(span) = span else { return };
        self.declared
            .insert(span.start, self.scopes.bindings.len().saturating_sub(1));
        match outcome {
            Declared::Ok => {}
            Declared::Duplicate(previous) => {
                // The first one is now unreachable: do not also report it unused.
                if let Some(first) = self.scopes.bindings.get_mut(previous) {
                    first.used = true;
                }
                let previous = self.scopes.bindings.get(previous).and_then(|b| b.span);
                self.report(match previous {
                    Some(previous) => errors::duplicate(span, &name, previous),
                    None => errors::shadowing(span, &name, None),
                });
            }
            Declared::Shadows(previous) => {
                let previous = self.scopes.bindings.get(previous).and_then(|b| b.span);
                self.report(errors::shadowing(span, &name, previous));
            }
        }
    }

    /// Closes a scope, reporting every variable and parameter never read.
    fn pop_scope(&mut self) {
        for index in self.scopes.pop() {
            let Some(binding) = self.scopes.bindings.get(index) else {
                continue;
            };
            if binding.used || !binding.must_be_used() {
                continue;
            }
            if let Some(span) = binding.span {
                let parameter = matches!(binding.kind, Kind::Param { .. });
                let d = errors::unused(span, &binding.name, parameter);
                self.report(d);
            }
        }
    }

    // --- Module -------------------------------------------------------------

    fn module(&mut self) {
        self.scopes.push();
        let some = self.ast.interner_symbol("Some");
        let none = self.ast.interner_symbol("None");
        if let Some(symbol) = some {
            self.declare(symbol, None, Kind::Some, Ty::Error);
        }
        if let Some(symbol) = none {
            self.declare(symbol, None, Kind::None, Ty::Option(Box::new(Ty::Error)));
        }
        for (text, kind) in [("Ok", Kind::Ok), ("Err", Kind::Err)] {
            if let Some(symbol) = self.ast.interner_symbol(text) {
                self.declare(symbol, None, kind, Ty::Error);
            }
        }

        // Structures, enumerations, aliases and interfaces first, so that any
        // signature can name them.
        let items = &self.ast.items;
        // Interfaces before all, as constraints of the type parameters below.
        for item in items {
            if let ItemKind::Interface(decl) = &item.kind {
                self.register_interface(decl);
            }
        }
        for item in items {
            match &item.kind {
                ItemKind::Struct(decl) => self.register_struct(decl),
                ItemKind::Enum(decl) => self.register_enum(decl),
                ItemKind::Alias(decl) => self.register_alias(decl),
                _ => {}
            }
        }
        for item in items {
            match &item.kind {
                ItemKind::Struct(decl) => self.struct_fields(decl),
                ItemKind::Enum(decl) => self.enum_variants(decl),
                _ => {}
            }
        }
        self.templates_done();
        // Every alias, used or not, so that one containing itself is reported.
        for index in 0..self.aliases.len() {
            self.alias_type(index);
        }
        for item in items {
            if let ItemKind::Interface(decl) = &item.kind {
                self.interface_methods(decl);
            }
        }
        // Functions and methods, in the order of [`Ast::functions`].
        for item in items {
            match &item.kind {
                ItemKind::Fn(decl) => {
                    self.register_fn(decl, None);
                }
                ItemKind::Impl(decl) => self.register_impl(decl),
                _ => {}
            }
        }
        for item in items {
            if let ItemKind::Impl(decl) = &item.kind {
                self.check_implementation(decl);
            }
        }
        for item in items {
            if let ItemKind::Global(local) = &item.kind {
                self.global(local);
            }
        }
        for (index, decl) in self.ast.functions().into_iter().enumerate() {
            self.fn_body(decl, FnId(index));
        }
        self.pop_scope();
    }

    fn register_struct(&mut self, decl: &StructDecl) {
        let name = self.name(decl.name.symbol).to_owned();
        let id = StructId(self.structs.len());
        let generics = self.generic_params(&decl.generics);
        let def = if generics.is_empty() {
            TypeDef::Plain(Ty::Struct(id, name.clone()))
        } else {
            TypeDef::Generic(Template::Struct(id), generics.len())
        };
        self.structs.push(StructDef {
            name: name.clone(),
            fields: Vec::new(),
            generics,
            origin: None,
            span: decl.name.span,
        });
        // A type of the module takes the place of a built-in one of that name.
        self.types.insert(name.clone(), def);
        self.declare(
            decl.name.symbol,
            Some(decl.name.span),
            Kind::Struct(id),
            Ty::Struct(id, name),
        );
    }

    fn register_enum(&mut self, decl: &EnumDecl) {
        let name = self.name(decl.name.symbol).to_owned();
        let id = EnumId(self.enums.len());
        let generics = self.generic_params(&decl.generics);
        let def = if generics.is_empty() {
            TypeDef::Plain(Ty::Enum(id, name.clone()))
        } else {
            TypeDef::Generic(Template::Enum(id), generics.len())
        };
        self.enums.push(EnumDef {
            name: name.clone(),
            variants: Vec::new(),
            generics,
            origin: None,
            span: decl.name.span,
        });
        self.types.insert(name.clone(), def);
        self.declare(
            decl.name.symbol,
            Some(decl.name.span),
            Kind::Enum(id),
            Ty::Enum(id, name),
        );
    }

    /// The variants of an enumeration, each name once.
    fn enum_variants(&mut self, decl: &EnumDecl) {
        let name = self.name(decl.name.symbol);
        let Some(index) = self.enums.iter().position(|e| e.name == name) else {
            return;
        };
        self.generics = self
            .enums
            .get(index)
            .map(|e| e.generics.clone())
            .unwrap_or_default();
        let mut seen: Vec<(Symbol, Span)> = Vec::new();
        let mut variants = Vec::new();
        for variant in &decl.variants {
            if let Some((_, previous)) = seen.iter().find(|(s, _)| *s == variant.name.symbol) {
                let d =
                    errors::duplicate(variant.name.span, self.name(variant.name.symbol), *previous);
                self.report(d);
                continue;
            }
            seen.push((variant.name.symbol, variant.name.span));
            let fields = variant
                .fields
                .iter()
                .map(|t| self.resolve_type(t))
                .collect();
            variants.push((self.name(variant.name.symbol).to_owned(), fields));
        }
        if let Some(def) = self.enums.get_mut(index) {
            def.variants = variants;
        }
        self.generics.clear();
    }

    /// The variants of a type that has some: an enumeration, or an option
    /// (`None`, `Some(value)`).
    pub(crate) fn variants_of(&self, ty: &Ty) -> Option<Vec<(String, Vec<Ty>)>> {
        match ty {
            Ty::Enum(id, _) => self.enums.get(id.0).map(|e| e.variants.clone()),
            Ty::Option(inner) => Some(vec![
                ("None".to_owned(), Vec::new()),
                ("Some".to_owned(), vec![(**inner).clone()]),
            ]),
            Ty::Result(ok, err) => Some(vec![
                ("Ok".to_owned(), vec![(**ok).clone()]),
                ("Err".to_owned(), vec![(**err).clone()]),
            ]),
            _ => None,
        }
    }

    fn struct_fields(&mut self, decl: &StructDecl) {
        let name = self.name(decl.name.symbol);
        let Some(index) = self.structs.iter().position(|s| s.name == name) else {
            return;
        };
        self.generics = self
            .structs
            .get(index)
            .map(|s| s.generics.clone())
            .unwrap_or_default();
        let mut seen: Vec<(Symbol, Span)> = Vec::new();
        let mut fields = Vec::new();
        for field in &decl.fields {
            if let Some((_, previous)) = seen.iter().find(|(s, _)| *s == field.name.symbol) {
                let d = errors::duplicate(field.name.span, self.name(field.name.symbol), *previous);
                self.report(d);
                continue;
            }
            seen.push((field.name.symbol, field.name.span));
            let ty = self.resolve_type(&field.ty);
            fields.push((self.name(field.name.symbol).to_owned(), ty));
        }
        if let Some(def) = self.structs.get_mut(index) {
            def.fields = fields;
        }
        self.generics.clear();
    }

    /// A function's signature; a method's, with the type of its `impl`, is
    /// not a name of the module but a member of that type.
    fn register_fn(&mut self, decl: &FnDecl, self_ty: Option<&Ty>) -> FnId {
        self.generics = self.generic_params(&decl.generics);
        self.self_ty = self_ty.cloned();
        if decl.exported && !self.generics.is_empty() {
            let name = self.name(decl.name.symbol);
            self.report(errors::generic_export(decl.name.span, name));
        }
        let params = decl
            .params
            .iter()
            .map(|p| self.resolve_type(&p.ty))
            .collect();
        let ret = match &decl.ret {
            Some(ty) => self.resolve_type(ty),
            // Unannotated: `void`, unless it returns a value (reported in the body).
            None if returns_value(&decl.body.stmts) => Ty::Error,
            None => Ty::Void,
        };
        let id = FnId(self.fns.len());
        let name = self.name(decl.name.symbol).to_owned();
        self.fns.push(FnSig {
            name: name.clone(),
            name_span: decl.name.span,
            generics: std::mem::take(&mut self.generics),
            params,
            ret,
            conventions: decl.params.iter().map(|p| p.convention).collect(),
            self_ty: self.self_ty.take(),
            has_self: decl.has_self(),
        });
        if decl.impl_of.is_none() {
            self.declare(
                decl.name.symbol,
                Some(decl.name.span),
                Kind::Fn(id),
                Ty::Fn(id, name),
            );
        }
        id
    }

    fn global(&mut self, local: &Local) {
        let ty = self.local_type(local);
        if !local.mutable
            && let Some(init) = local.init
        {
            self.check_constant(init);
        }
        self.declare(
            local.name.symbol,
            Some(local.name.span),
            Kind::Global {
                mutable: local.mutable,
                keyword: local.keyword,
            },
            ty,
        );
    }

    fn fn_body(&mut self, decl: &FnDecl, id: FnId) {
        let (ret, ret_span) = match (&decl.ret, self.fns.get(id.0)) {
            (Some(ty), Some(sig)) => (Some(sig.ret.clone()), Some(ty.span)),
            _ => (None, None),
        };
        self.ret = ret.clone().map(|t| (t, ret_span));
        self.returned.clear();
        self.generics = self
            .fns
            .get(id.0)
            .map(|s| s.generics.clone())
            .unwrap_or_default();
        self.self_ty = self.fns.get(id.0).and_then(|s| s.self_ty.clone());

        self.scopes.push();
        let param_types: Vec<Ty> = self
            .fns
            .get(id.0)
            .map(|s| s.params.clone())
            .unwrap_or_default();
        for (param, ty) in decl.params.iter().zip(param_types) {
            // `inout` is written before `self`, and before the type of the others.
            let type_start = if param.is_self {
                param.name.span.start
            } else {
                param.ty.span.start
            };
            let kind = Kind::Param {
                convention: param.convention,
                type_start,
            };
            self.declare(param.name.symbol, Some(param.name.span), kind, ty);
        }
        self.block(&decl.body.stmts);
        self.pop_scope();

        let name = self.name(decl.name.symbol);
        match (ret, ret_span) {
            (Some(ret), Some(ret_span)) => {
                let returns = !matches!(ret, Ty::Void | Ty::Error);
                if returns && !always_returns(self.ast, &decl.body.stmts) {
                    let end = decl.body.span.end;
                    let closing = Span::new(end.saturating_sub(1), end);
                    self.report(errors::missing_return(closing, name, &ret, ret_span));
                }
            }
            _ => {
                if decl.exported || !self.returned.is_empty() {
                    let inferred = self
                        .returned
                        .iter()
                        .find(|t| !matches!(t, Ty::Error))
                        .cloned()
                        .map_or(Ty::Void, Ty::defaulted);
                    let at = decl.params_span.end;
                    let d = errors::missing_return_type(
                        decl.name.span,
                        name,
                        decl.exported,
                        at,
                        &inferred,
                    );
                    self.report(d);
                }
            }
        }
        self.ret = None;
        self.generics.clear();
        self.self_ty = None;
    }

    /// The declared or inferred type of a `let` or `const`, its value checked.
    fn local_type(&mut self, local: &Local) -> Ty {
        let annotated = local.ty.as_ref().map(|t| self.resolve_type(t));
        match (annotated, local.init) {
            (Some(ty), Some(init)) => {
                self.expect(init, &ty);
                ty
            }
            (Some(ty), None) => ty,
            (None, Some(init)) => self.infer(init, None).defaulted(),
            (None, None) => Ty::Error,
        }
    }

    // --- Types written in source -------------------------------------------

    pub(crate) fn resolve_type(&mut self, ty: &Type) -> Ty {
        let resolved = self.resolve_type_inner(ty);
        self.written
            .insert((ty.span.start, ty.span.end), resolved.clone());
        resolved
    }

    fn resolve_type_inner(&mut self, ty: &Type) -> Ty {
        let (name, args) = match &ty.kind {
            TypeKind::Named { name, args } => (name, args),
            TypeKind::Tuple(items) => {
                let items: Vec<Ty> = items.iter().map(|t| self.resolve_type(t)).collect();
                return self.tuple_type(items);
            }
            TypeKind::Error => return Ty::Error,
        };
        let text = self.name(name.symbol);
        if text == "Self"
            && args.is_empty()
            && let Some(own) = self.self_ty.clone()
        {
            return own;
        }
        if let Some(param) = self.type_param(text) {
            if !args.is_empty() {
                self.report(errors::type_argument_count(ty.span, text, 0, args.len()));
                return Ty::Error;
            }
            return param;
        }
        let Some(def) = self.types.get(text).cloned() else {
            let suggestion =
                scope::suggest(text, self.types.keys().map(String::as_str)).map(str::to_owned);
            self.report(errors::unknown_type(name.span, text, suggestion.as_deref()));
            return Ty::Error;
        };
        let expected = match def {
            TypeDef::Plain(_) => 0,
            TypeDef::Array | TypeDef::Option => 1,
            TypeDef::Result => 2,
            TypeDef::Generic(_, count) => count,
            TypeDef::Alias(index) => self.aliases.get(index).map_or(0, |a| a.generics.len()),
            TypeDef::Interface(index) => {
                let name = self.interfaces.get(index).map(|i| i.name.clone());
                self.report(errors::interface_as_type(
                    ty.span,
                    &name.unwrap_or_default(),
                ));
                return Ty::Error;
            }
        };
        if args.len() != expected {
            self.report(errors::type_argument_count(
                ty.span,
                text,
                expected,
                args.len(),
            ));
            return Ty::Error;
        }
        match def {
            TypeDef::Plain(t) => t,
            TypeDef::Array => Ty::Array(Box::new(self.type_arg(args))),
            TypeDef::Option => Ty::Option(Box::new(self.type_arg(args))),
            TypeDef::Result => {
                let ok = self.type_arg(args);
                let err = args.get(1).map_or(Ty::Error, |a| self.resolve_type(a));
                Ty::Result(Box::new(ok), Box::new(err))
            }
            TypeDef::Alias(index) => {
                let args: Vec<Ty> = args.iter().map(|a| self.resolve_type(a)).collect();
                let aliased = self.alias_type(index);
                self.subst(&aliased, &args)
            }
            TypeDef::Interface(_) => Ty::Error,
            TypeDef::Generic(template, _) => {
                let args: Vec<Ty> = args.iter().map(|a| self.resolve_type(a)).collect();
                self.check_bounds(template, &args, ty);
                match template {
                    Template::Struct(id) => self.struct_instance(id, args),
                    Template::Enum(id) => self.enum_instance(id, args),
                }
            }
        }
    }

    /// `E0227` for each type argument written that misses a constraint of
    /// its parameter, as in `Sorted<String>` with `struct Sorted<T: Ord>`.
    fn check_bounds(&mut self, template: Template, args: &[Ty], ty: &Type) {
        let generics = match template {
            Template::Struct(id) => self.structs.get(id.0).map(|s| s.generics.clone()),
            Template::Enum(id) => self.enums.get(id.0).map(|e| e.generics.clone()),
        }
        .unwrap_or_default();
        let TypeKind::Named { args: written, .. } = &ty.kind else {
            return;
        };
        for ((param, arg), at) in generics.iter().zip(args).zip(written) {
            for &bound in &param.bounds {
                if !self.satisfies(arg, bound) {
                    self.report(errors::constraint_not_met(
                        at.span,
                        arg,
                        &param.name,
                        bound.name(),
                        param.span,
                    ));
                }
            }
        }
    }

    fn type_arg(&mut self, args: &[Type]) -> Ty {
        args.first().map_or(Ty::Error, |a| self.resolve_type(a))
    }
}

/// Whether a function body contains `return <value>`, at any depth.
fn returns_value(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|stmt| match &stmt.kind {
        StmtKind::Return(Some(_)) => true,
        StmtKind::Block(block) => returns_value(&block.stmts),
        StmtKind::If {
            then, otherwise, ..
        } => {
            returns_value(std::slice::from_ref(then))
                || otherwise
                    .as_ref()
                    .is_some_and(|o| returns_value(std::slice::from_ref(o)))
        }
        StmtKind::While { body, .. }
        | StmtKind::For { body, .. }
        | StmtKind::ForOf { body, .. } => returns_value(std::slice::from_ref(body)),
        _ => false,
    })
}

/// Whether every path through these statements ends with `return` or loops forever.
fn always_returns(ast: &Ast, stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| stmt_always_returns(ast, s))
}

fn stmt_always_returns(ast: &Ast, stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Return(_) => true,
        // A `match` whose arms all return; one that misses a value is E0222.
        StmtKind::Expr(e) => match_arms(ast, *e).is_some_and(|arms| {
            !arms.is_empty() && arms.iter().all(|stmts| always_returns(ast, stmts))
        }),
        StmtKind::Block(block) => always_returns(ast, &block.stmts),
        StmtKind::If {
            then,
            otherwise: Some(otherwise),
            ..
        } => stmt_always_returns(ast, then) && stmt_always_returns(ast, otherwise),
        // `for (;;)` and `while (true)` without `break` never end normally.
        StmtKind::For {
            cond: None, body, ..
        } => !breaks(ast, body),
        StmtKind::While { cond, body } => {
            matches!(ast.expr(*cond).kind, ExprKind::Bool(true)) && !breaks(ast, body)
        }
        _ => false,
    }
}

/// Whether a loop body contains a `break` for this loop (not a nested one).
fn breaks(ast: &Ast, stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Break => true,
        StmtKind::Block(block) => block.stmts.iter().any(|s| breaks(ast, s)),
        StmtKind::If {
            then, otherwise, ..
        } => breaks(ast, then) || otherwise.as_ref().is_some_and(|o| breaks(ast, o)),
        StmtKind::Expr(e) => match_arms(ast, *e).is_some_and(|arms| {
            arms.iter()
                .any(|stmts| stmts.iter().any(|s| breaks(ast, s)))
        }),
        _ => false,
    }
}

/// The statements of each block arm of a `match` used as a statement.
fn match_arms(ast: &Ast, e: ExprId) -> Option<Vec<&[Stmt]>> {
    let ExprKind::Match { arms, .. } = &ast.expr(e).kind else {
        return None;
    };
    Some(
        arms.iter()
            .map(|arm| match &arm.body {
                perasc_syntax::ast::ArmBody::Block(block) => block.stmts.as_slice(),
                perasc_syntax::ast::ArmBody::Expr(_) => &[],
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
