//! Name, type, ownership, resource and stack-budget checks for the Peras compiler.
//!
//! Names and types are checked in one walk over the tree: each name is resolved
//! and typed where it is used. A name or type already reported as wrong becomes
//! [`ty::Ty::Error`], compatible with everything, so one mistake gives one error.
//! The walk runs only on a tree without syntax errors.

pub mod builtins;
mod errors;
mod expr;
mod matching;
mod owner;
mod scope;
mod stmt;
pub mod ty;

use std::collections::{HashMap, HashSet};

use perasc_diag::{Diagnostic, Span};
use perasc_syntax::ast::{
    Ast, EnumDecl, ExprId, ExprKind, FnDecl, ItemKind, Local, Stmt, StmtKind, StructDecl, Symbol,
    Type, TypeKind,
};

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
}

/// A function's signature.
struct FnSig {
    name_span: Span,
    params: Vec<Ty>,
    ret: Ty,
}

/// What a type name refers to.
#[derive(Clone)]
enum TypeDef {
    Plain(Ty),
    Array,
    Option,
    Result,
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
}

/// An enumeration's variants, in order, each with the types of its data.
struct EnumDef {
    name: String,
    variants: Vec<(String, Vec<Ty>)>,
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

        // Structures and enumerations first, so that any signature can name them.
        let items = &self.ast.items;
        for item in items {
            match &item.kind {
                ItemKind::Struct(decl) => self.register_struct(decl),
                ItemKind::Enum(decl) => self.register_enum(decl),
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
        for item in items {
            if let ItemKind::Fn(decl) = &item.kind {
                self.register_fn(decl);
            }
        }
        for item in items {
            if let ItemKind::Global(local) = &item.kind {
                self.global(local);
            }
        }
        let mut fn_index = 0;
        for item in items {
            if let ItemKind::Fn(decl) = &item.kind {
                self.fn_body(decl, FnId(fn_index));
                fn_index += 1;
            }
        }
        self.pop_scope();
    }

    fn register_struct(&mut self, decl: &StructDecl) {
        let name = self.name(decl.name.symbol).to_owned();
        let id = StructId(self.structs.len());
        self.structs.push(StructDef {
            name: name.clone(),
            fields: Vec::new(),
        });
        // A type of the module takes the place of a built-in one of that name.
        self.types
            .insert(name.clone(), TypeDef::Plain(Ty::Struct(id, name.clone())));
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
        self.enums.push(EnumDef {
            name: name.clone(),
            variants: Vec::new(),
        });
        self.types
            .insert(name.clone(), TypeDef::Plain(Ty::Enum(id, name.clone())));
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
    }

    fn register_fn(&mut self, decl: &FnDecl) {
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
            name_span: decl.name.span,
            params,
            ret,
        });
        self.declare(
            decl.name.symbol,
            Some(decl.name.span),
            Kind::Fn(id),
            Ty::Fn(id, name),
        );
    }

    fn global(&mut self, local: &Local) {
        let ty = self.local_type(local);
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

        self.scopes.push();
        let param_types: Vec<Ty> = self
            .fns
            .get(id.0)
            .map(|s| s.params.clone())
            .unwrap_or_default();
        for (param, ty) in decl.params.iter().zip(param_types) {
            let kind = Kind::Param {
                convention: param.convention,
                type_start: param.ty.span.start,
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

    fn resolve_type(&mut self, ty: &Type) -> Ty {
        let TypeKind::Named { name, args } = &ty.kind else {
            return Ty::Error;
        };
        let text = self.name(name.symbol);
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
