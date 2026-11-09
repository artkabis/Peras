//! Generics: type parameters, their constraints, and instantiation.
//!
//! A generic function, structure or enumeration is checked once, its type
//! parameters standing for any type that meets their constraints
//! ([`Ty::Param`]): an operation is accepted only when every such type
//! supports it (`E0228`). At each use, the type arguments are inferred from
//! the arguments and the type expected (`E0229` when they cannot be), and
//! checked against the constraints (`E0227`).
//!
//! Code generation then needs one copy per set of type arguments
//! (monomorphization): once the module is checked, every instance reachable
//! from the functions that are not generic is collected, and every
//! structure or enumeration they use is created, so that lowering only
//! substitutes types it finds already made.

use std::collections::{HashMap, HashSet};

use perasc_diag::Span;
use perasc_syntax::ast::{ExprId, TypeParam};

use crate::ty::{Bound, EnumId, FnId, StructId, Ty};
use crate::{Analysis, Checker, errors, scope};

/// The deepest type argument an instance may have: deeper means the generic
/// code instantiates itself with ever larger types, without end.
const MAX_DEPTH: usize = 12;

/// The most instances a module may need.
const MAX_INSTANCES: usize = 4096;

/// A type parameter in scope.
#[derive(Debug, Clone)]
pub(crate) struct GenericParam {
    pub name: String,
    /// Its name, where it is declared.
    pub span: Span,
    pub bounds: Vec<Bound>,
    /// The interfaces its types implement, by their index.
    pub interfaces: Vec<usize>,
    /// Whether one of its constraints was unknown, already reported: it then
    /// meets every constraint, so one mistake gives one error.
    pub unknown: bool,
    /// Where a missing constraint is written: after the name, or after the
    /// last constraint.
    pub insert_at: u32,
}

impl GenericParam {
    /// Whether every type it stands for meets `bound`.
    pub fn meets(&self, bound: Bound) -> bool {
        self.unknown || self.bounds.iter().any(|b| b.implies(bound))
    }
}

impl Checker<'_> {
    /// How deep a type is nested: `i32` is 1, `Array<Array<i32>>` is 3, and
    /// `Pair<Pair<i32, i32>, i32>` is 3.
    fn depth(&self, ty: &Ty) -> usize {
        let args = |origin: Option<&Vec<Ty>>| {
            origin.map_or(0, |args| {
                args.iter().map(|a| self.depth(a)).max().unwrap_or(0)
            })
        };
        match ty {
            Ty::Array(t) | Ty::Option(t) => 1 + self.depth(t),
            Ty::Result(a, b) => 1 + self.depth(a).max(self.depth(b)),
            Ty::Struct(id, _) => {
                1 + args(
                    self.structs
                        .get(id.0)
                        .and_then(|s| s.origin.as_ref())
                        .map(|(_, a)| a),
                )
            }
            Ty::Enum(id, _) => {
                1 + args(
                    self.enums
                        .get(id.0)
                        .and_then(|e| e.origin.as_ref())
                        .map(|(_, a)| a),
                )
            }
            _ => 1,
        }
    }

    /// The type parameters of a declaration, their constraints resolved.
    pub(crate) fn generic_params(&mut self, list: &[TypeParam]) -> Vec<GenericParam> {
        let mut out: Vec<GenericParam> = Vec::new();
        for param in list {
            let name = self.name(param.name.symbol).to_owned();
            if let Some(previous) = out.iter().find(|p| p.name == name) {
                let d = errors::duplicate(param.name.span, &name, previous.span);
                self.report(d);
                continue;
            }
            let mut bounds = Vec::new();
            let mut interfaces = Vec::new();
            let mut unknown = false;
            for bound in &param.bounds {
                let text = self.name(bound.symbol);
                if let Some(b) = Bound::named(text) {
                    bounds.push(b);
                } else if let Some(i) = self.interface_named(text) {
                    interfaces.push(i);
                } else {
                    unknown = true;
                    let names: Vec<String> = Bound::ALL
                        .iter()
                        .map(|b| b.name().to_owned())
                        .chain(self.interfaces.iter().map(|i| i.name.clone()))
                        .collect();
                    let suggestion =
                        scope::suggest(text, names.iter().map(String::as_str)).map(str::to_owned);
                    self.report(errors::unknown_constraint(
                        bound.span,
                        text,
                        suggestion.as_deref(),
                    ));
                }
            }
            let insert_at = param
                .bounds
                .last()
                .map_or(param.name.span.end, |b| b.span.end);
            out.push(GenericParam {
                name,
                span: param.name.span,
                bounds,
                interfaces,
                unknown,
                insert_at,
            });
        }
        out
    }

    /// The type parameter of this name, in the declaration being checked.
    pub(crate) fn type_param(&self, name: &str) -> Option<Ty> {
        let index = self.generics.iter().position(|p| p.name == name)?;
        Some(Ty::Param(
            u32::try_from(index).unwrap_or(u32::MAX),
            name.to_owned(),
        ))
    }

    /// The parameter a [`Ty::Param`] stands for.
    pub(crate) fn param_of(&self, index: u32) -> Option<&GenericParam> {
        self.generics.get(usize::try_from(index).ok()?)
    }

    /// Whether `ty` meets a constraint: a type parameter when its own
    /// constraints imply it, any other type by what it is.
    pub(crate) fn satisfies(&self, ty: &Ty, bound: Bound) -> bool {
        match ty {
            Ty::Error => true,
            Ty::Param(i, _) => self.param_of(*i).is_some_and(|p| p.meets(bound)),
            _ => match bound {
                Bound::Eq => !matches!(ty, Ty::Void | Ty::Fn(..)),
                Bound::Ord | Bound::Numeric => ty.is_numeric(),
                Bound::Integer => ty.is_integer(),
                Bound::Float => ty.is_float(),
                Bound::Copy => !self.holds_heap(ty) && !matches!(ty, Ty::Void | Ty::Fn(..)),
            },
        }
    }

    /// Whether `ty` is a type parameter that meets `bound`.
    pub(crate) fn satisfies_param(&self, ty: &Ty, bound: Bound) -> bool {
        matches!(ty, Ty::Param(..)) && self.satisfies(ty, bound)
    }

    /// Whether a value of this type holds a buffer on the heap, which a copy
    /// would share: a string or an array, at any depth, or a type parameter
    /// that is not `Copy`.
    fn holds_heap(&self, ty: &Ty) -> bool {
        match ty {
            Ty::String | Ty::Array(_) => true,
            Ty::Param(..) => !self.satisfies(ty, Bound::Copy),
            Ty::Option(t) => self.holds_heap(t),
            Ty::Result(a, b) => self.holds_heap(a) || self.holds_heap(b),
            Ty::Struct(id, _) => self
                .structs
                .get(id.0)
                .is_some_and(|s| s.fields.iter().any(|(_, f)| self.holds_heap(f))),
            Ty::Enum(id, _) => self.enums.get(id.0).is_some_and(|e| {
                e.variants
                    .iter()
                    .any(|(_, fields)| fields.iter().any(|f| self.holds_heap(f)))
            }),
            _ => false,
        }
    }

    /// Whether a type mentions a type parameter, at any depth.
    pub(crate) fn mentions_params(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Param(..) => true,
            Ty::Array(t) | Ty::Option(t) => self.mentions_params(t),
            Ty::Result(a, b) => self.mentions_params(a) || self.mentions_params(b),
            Ty::Struct(id, _) => self
                .structs
                .get(id.0)
                .and_then(|s| s.origin.as_ref())
                .is_some_and(|(_, args)| args.iter().any(|a| self.mentions_params(a))),
            Ty::Enum(id, _) => self
                .enums
                .get(id.0)
                .and_then(|e| e.origin.as_ref())
                .is_some_and(|(_, args)| args.iter().any(|a| self.mentions_params(a))),
            _ => false,
        }
    }

    /// `ty` with each type parameter replaced by its argument; the instances
    /// of structures and enumerations it needs are created.
    pub(crate) fn subst(&mut self, ty: &Ty, args: &[Ty]) -> Ty {
        match ty {
            Ty::Param(i, _) => usize::try_from(*i)
                .ok()
                .and_then(|i| args.get(i))
                .cloned()
                .unwrap_or(Ty::Error),
            Ty::Array(t) => Ty::Array(Box::new(self.subst(t, args))),
            Ty::Option(t) => Ty::Option(Box::new(self.subst(t, args))),
            Ty::Result(a, b) => {
                let a = self.subst(a, args);
                let b = self.subst(b, args);
                Ty::Result(Box::new(a), Box::new(b))
            }
            Ty::Struct(id, _) if self.mentions_params(ty) => {
                let Some((template, inner)) = self.structs.get(id.0).and_then(|s| s.origin.clone())
                else {
                    return ty.clone();
                };
                let inner = inner.iter().map(|t| self.subst(t, args)).collect();
                self.struct_instance(template, inner)
            }
            Ty::Enum(id, _) if self.mentions_params(ty) => {
                let Some((template, inner)) = self.enums.get(id.0).and_then(|e| e.origin.clone())
                else {
                    return ty.clone();
                };
                let inner = inner.iter().map(|t| self.subst(t, args)).collect();
                self.enum_instance(template, inner)
            }
            other => other.clone(),
        }
    }

    /// The instance of a generic structure for these type arguments, made
    /// the first time it is needed.
    pub(crate) fn struct_instance(&mut self, template: StructId, args: Vec<Ty>) -> Ty {
        let Some(def) = self.structs.get(template.0) else {
            return Ty::Error;
        };
        if args.is_empty() {
            return Ty::Struct(template, def.name.clone());
        }
        if let Some(&id) = self.struct_instances.get(&(template, args.clone())) {
            let name = self.structs.get(id.0).map(|s| s.name.clone());
            return Ty::Struct(id, name.unwrap_or_default());
        }
        let span = def.span;
        let name = instance_name(&def.name, &args);
        if args.iter().any(|a| self.depth(a) > MAX_DEPTH) {
            self.endless(span, &def.name.clone());
            return Ty::Error;
        }
        let id = StructId(self.structs.len());
        self.structs.push(crate::StructDef {
            name: name.clone(),
            fields: Vec::new(),
            generics: Vec::new(),
            origin: Some((template, args.clone())),
            span,
        });
        self.struct_instances.insert((template, args), id);
        if self.templates_ready {
            self.fill_struct(id);
        }
        Ty::Struct(id, name)
    }

    /// The instance of a generic enumeration for these type arguments.
    pub(crate) fn enum_instance(&mut self, template: EnumId, args: Vec<Ty>) -> Ty {
        let Some(def) = self.enums.get(template.0) else {
            return Ty::Error;
        };
        if args.is_empty() {
            return Ty::Enum(template, def.name.clone());
        }
        if let Some(&id) = self.enum_instances.get(&(template, args.clone())) {
            let name = self.enums.get(id.0).map(|e| e.name.clone());
            return Ty::Enum(id, name.unwrap_or_default());
        }
        let span = def.span;
        let name = instance_name(&def.name, &args);
        if args.iter().any(|a| self.depth(a) > MAX_DEPTH) {
            self.endless(span, &def.name.clone());
            return Ty::Error;
        }
        let id = EnumId(self.enums.len());
        self.enums.push(crate::EnumDef {
            name: name.clone(),
            variants: Vec::new(),
            generics: Vec::new(),
            origin: Some((template, args.clone())),
            span,
        });
        self.enum_instances.insert((template, args), id);
        if self.templates_ready {
            self.fill_enum(id);
        }
        Ty::Enum(id, name)
    }

    /// The fields of a structure instance: its template's, substituted.
    fn fill_struct(&mut self, id: StructId) {
        let Some((template, args)) = self.structs.get(id.0).and_then(|s| s.origin.clone()) else {
            return;
        };
        let fields = self
            .structs
            .get(template.0)
            .map(|s| s.fields.clone())
            .unwrap_or_default();
        let fields = fields
            .into_iter()
            .map(|(name, ty)| (name, self.subst(&ty, &args)))
            .collect();
        if let Some(def) = self.structs.get_mut(id.0) {
            def.fields = fields;
        }
    }

    /// The variants of an enumeration instance: its template's, substituted.
    fn fill_enum(&mut self, id: EnumId) {
        let Some((template, args)) = self.enums.get(id.0).and_then(|e| e.origin.clone()) else {
            return;
        };
        let variants = self
            .enums
            .get(template.0)
            .map(|e| e.variants.clone())
            .unwrap_or_default();
        let variants = variants
            .into_iter()
            .map(|(name, fields)| {
                let fields = fields.iter().map(|t| self.subst(t, &args)).collect();
                (name, fields)
            })
            .collect();
        if let Some(def) = self.enums.get_mut(id.0) {
            def.variants = variants;
        }
    }

    /// Once every template is known: the fields and variants of the
    /// instances that signatures and fields needed before.
    pub(crate) fn templates_done(&mut self) {
        self.templates_ready = true;
        let mut i = 0;
        while i < self.structs.len() {
            self.fill_struct(StructId(i));
            i += 1;
        }
        let mut i = 0;
        while i < self.enums.len() {
            self.fill_enum(EnumId(i));
            i += 1;
        }
    }

    /// `E0231`, once per template.
    fn endless(&mut self, span: Span, name: &str) {
        if self.endless_reported.insert(span.start) {
            self.report(errors::endless_instances(span, name));
        }
    }

    /// The identity instance of a template, `Pair<A, B>` in its own terms:
    /// the pattern its uses are matched against.
    pub(crate) fn identity(generics: &[GenericParam]) -> Vec<Ty> {
        generics
            .iter()
            .enumerate()
            .map(|(i, p)| Ty::Param(u32::try_from(i).unwrap_or(u32::MAX), p.name.clone()))
            .collect()
    }

    /// Matches a type of a generic declaration against the type of a value,
    /// recording what each of its type parameters stands for.
    pub(crate) fn bind(&self, pattern: &Ty, actual: &Ty, out: &mut [Option<Ty>]) {
        match (pattern, actual) {
            (_, Ty::Error) => {}
            (Ty::Param(i, _), _) => {
                let Some(slot) = usize::try_from(*i).ok().and_then(|i| out.get_mut(i)) else {
                    return;
                };
                match slot {
                    None => *slot = Some(actual.clone()),
                    // A literal gives way to a concrete number of its kind.
                    Some(Ty::IntLit) if matches!(actual, Ty::Int(_)) => {
                        *slot = Some(actual.clone());
                    }
                    Some(Ty::FloatLit) if matches!(actual, Ty::Float(_)) => {
                        *slot = Some(actual.clone());
                    }
                    Some(Ty::IntLit) if *actual == Ty::FloatLit => *slot = Some(Ty::FloatLit),
                    Some(_) => {}
                }
            }
            (Ty::Array(p), Ty::Array(a)) | (Ty::Option(p), Ty::Option(a)) => self.bind(p, a, out),
            (Ty::Result(p, q), Ty::Result(a, b)) => {
                self.bind(p, a, out);
                self.bind(q, b, out);
            }
            (Ty::Struct(p, _), Ty::Struct(a, _)) => {
                let origin = |id: StructId| self.structs.get(id.0).and_then(|s| s.origin.as_ref());
                if let (Some((pt, pargs)), Some((at, aargs))) = (origin(*p), origin(*a))
                    && pt == at
                {
                    for (p, a) in pargs.iter().zip(aargs) {
                        self.bind(p, a, out);
                    }
                }
            }
            (Ty::Enum(p, _), Ty::Enum(a, _)) => {
                let origin = |id: EnumId| self.enums.get(id.0).and_then(|e| e.origin.as_ref());
                if let (Some((pt, pargs)), Some((at, aargs))) = (origin(*p), origin(*a))
                    && pt == at
                {
                    for (p, a) in pargs.iter().zip(aargs) {
                        self.bind(p, a, out);
                    }
                }
            }
            _ => {}
        }
    }

    /// Checks the arguments of a use of a generic declaration, and infers
    /// its type arguments from them and from the type expected; returns
    /// them, and the type of the use.
    pub(crate) fn infer_generic(
        &mut self,
        use_: &GenericUse<'_>,
        written: Option<&[Ty]>,
        args: &[ExprId],
        expected: Option<&Ty>,
    ) -> (Vec<Ty>, Ty) {
        let mut bound: Vec<Option<Ty>> = match written {
            Some(types) => types.iter().cloned().map(Some).collect(),
            None => vec![None; use_.generics.len()],
        };
        // A method's `self` is the value before the dot, already typed.
        let params = if let Some((receiver, _)) = use_.receiver {
            if let Some(own) = use_.params.first() {
                self.bind(own, receiver, &mut bound);
            }
            use_.params.get(1..).unwrap_or_default()
        } else {
            use_.params
        };
        let mut actual: Vec<Option<Ty>> = Vec::with_capacity(args.len());
        for (i, &arg) in args.iter().enumerate() {
            match params.get(i) {
                Some(param) if self.mentions_params(param) => {
                    // A parameter already known guides the argument, as in
                    // `push(xs, 1)` with `xs: Array<i64>`.
                    let known: Option<Vec<Ty>> = bound.iter().cloned().collect();
                    let hint = known
                        .map(|k| self.subst(param, &k))
                        .filter(|t| !matches!(t, Ty::IntLit | Ty::FloatLit));
                    let ty = self.infer(arg, hint.as_ref());
                    self.bind(param, &ty, &mut bound);
                    actual.push(Some(ty));
                }
                Some(param) => {
                    let param = param.clone();
                    self.expect(arg, &param);
                    actual.push(None);
                }
                None => {
                    self.infer(arg, None);
                    actual.push(None);
                }
            }
        }
        if let Some(expected) = expected {
            self.bind(use_.ret, expected, &mut bound);
        }
        let mut types = Vec::with_capacity(bound.len());
        for (i, slot) in bound.into_iter().enumerate() {
            let param = use_.generics.get(i).cloned();
            let ty = if let Some(ty) = slot {
                ty.defaulted()
            } else {
                if let Some(param) = &param {
                    self.report(errors::cannot_infer(
                        use_.span,
                        &param.name,
                        use_.name,
                        param.span,
                    ));
                }
                Ty::Error
            };
            if let Some(param) = &param {
                let at = self.argument_of(use_.span, params, args, i);
                self.check_param(&ty, param, at);
            }
            types.push(ty);
        }
        // Each argument against its parameter, now known: literals take
        // their type, `None` and `[]` what they hold.
        for ((&arg, param), given) in args.iter().zip(params).zip(actual) {
            if let Some(given) = given {
                let want = self.subst(param, &types);
                self.coerce(arg, &given, &want);
            }
        }
        if let (Some((receiver, at)), Some(own)) = (use_.receiver, use_.params.first()) {
            let want = self.subst(own, &types);
            if want != *receiver && *receiver != Ty::Error && want != Ty::Error {
                self.report(errors::mismatch(at, &want, receiver, None));
            }
        }
        let ret = self.subst(use_.ret, &types);
        (types, ret)
    }

    /// `E0227` for each constraint of `param` that `ty` does not meet.
    pub(crate) fn check_param(&mut self, ty: &Ty, param: &GenericParam, at: Span) {
        for &b in &param.bounds {
            if !self.satisfies(ty, b) {
                self.report(errors::constraint_not_met(
                    at,
                    ty,
                    &param.name,
                    b.name(),
                    param.span,
                ));
            }
        }
        for &i in &param.interfaces {
            if !self.implements(ty, i) {
                let name = self
                    .interfaces
                    .get(i)
                    .map(|d| d.name.clone())
                    .unwrap_or_default();
                self.report(errors::constraint_not_met(
                    at,
                    ty,
                    &param.name,
                    &name,
                    param.span,
                ));
            }
        }
    }

    /// Where to report a type argument that misses a constraint: the first
    /// argument whose parameter mentions it, or the whole use.
    fn argument_of(&self, span: Span, params: &[Ty], args: &[ExprId], index: usize) -> Span {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        args.iter()
            .zip(params)
            .find(|(_, p)| mentions(p, index))
            .map_or(span, |(&a, _)| self.ast.expr(a).span)
    }

    /// Every instance of a generic function the module needs, from the
    /// functions that are not generic; every structure and enumeration
    /// those instances use; and the method each interface call of an
    /// instance reaches.
    pub(crate) fn monomorphize(&mut self) -> (Vec<(FnId, Vec<Ty>)>, DynTargets) {
        // The function each expression and written type belongs to.
        let ast = self.ast;
        let spans: Vec<Span> = ast
            .functions()
            .iter()
            .map(|f| Span::new(f.name.span.start, f.body.span.end))
            .collect();
        let owner = |span: Span| -> Option<usize> {
            let i = spans.partition_point(|s| s.start <= span.start);
            let i = i.checked_sub(1)?;
            spans.get(i).filter(|s| span.end <= s.end).map(|_| i)
        };
        let mut exprs: Vec<Vec<Ty>> = vec![Vec::new(); spans.len()];
        for (id, ty) in &self.expr_types {
            if let Some(list) = owner(ast.expr(*id).span).and_then(|f| exprs.get_mut(f)) {
                list.push(ty.clone());
            }
        }
        for (&(start, end), ty) in &self.written {
            if let Some(list) = owner(Span::new(start, end)).and_then(|f| exprs.get_mut(f)) {
                list.push(ty.clone());
            }
        }
        let mut calls: Vec<Vec<(ExprId, Callee)>> = vec![Vec::new(); spans.len()];
        for (id, (callee, args)) in &self.calls {
            if let Some(list) = owner(ast.expr(*id).span).and_then(|f| calls.get_mut(f)) {
                list.push((*id, Callee::Fn(*callee, args.clone())));
            }
        }
        for (id, target) in &self.method_calls {
            if let crate::methods::Target::Interface {
                method, receiver, ..
            } = target
                && let Some(list) = owner(ast.expr(*id).span).and_then(|f| calls.get_mut(f))
            {
                list.push((*id, Callee::Interface(method.clone(), receiver.clone())));
            }
        }
        // Sorted, so that instances come in the order of the source.
        for list in &mut calls {
            list.sort_by_key(|(id, _)| ast.expr(*id).span.start);
        }

        let mut instances: Vec<(FnId, Vec<Ty>)> = Vec::new();
        let mut targets: DynTargets = HashMap::new();
        let mut seen: HashSet<(FnId, Vec<Ty>)> = HashSet::new();
        let mut queue: Vec<(usize, Vec<Ty>)> = (0..spans.len())
            .filter(|&f| self.fns.get(f).is_some_and(|s| s.generics.is_empty()))
            .map(|f| (f, Vec::new()))
            .collect();
        queue.reverse();
        while let Some((f, args)) = queue.pop() {
            if !args.is_empty() {
                // Its types, so that every instance of a structure or an
                // enumeration they need exists.
                let mut types = exprs.get(f).cloned().unwrap_or_default();
                if let Some(sig) = self.fns.get(f) {
                    types.extend(sig.params.iter().cloned());
                    types.push(sig.ret.clone());
                }
                for ty in types {
                    self.subst(&ty, &args);
                }
            }
            for (call, callee) in calls.get(f).cloned().unwrap_or_default() {
                let (callee, cargs) = match callee {
                    Callee::Fn(callee, cargs) => {
                        (callee, cargs.iter().map(|t| self.subst(t, &args)).collect())
                    }
                    Callee::Interface(method, receiver) => {
                        let receiver = self.subst(&receiver, &args);
                        let Some(target) = self.method_for(&receiver, &method) else {
                            continue;
                        };
                        targets.insert((call, args.clone()), target.clone());
                        target
                    }
                };
                if cargs.is_empty() {
                    continue;
                }
                if cargs.iter().any(|t| self.depth(t) > MAX_DEPTH)
                    || instances.len() >= MAX_INSTANCES
                {
                    let span = ast.expr(call).span;
                    let name = self
                        .fns
                        .get(callee.0)
                        .map(|s| s.name.clone())
                        .unwrap_or_default();
                    if self.endless_reported.insert(span.start) {
                        self.report(errors::endless_instances(span, &name));
                    }
                    // Its copies are not made: the next mistake is reported too.
                    continue;
                }
                let key = (callee, cargs);
                if seen.insert(key.clone()) {
                    instances.push(key.clone());
                    queue.insert(0, (key.0.0, key.1));
                }
            }
        }
        (instances, targets)
    }

    /// The method of a concrete type an interface's method reaches, with the
    /// type arguments of its `impl`.
    fn method_for(&mut self, receiver: &Ty, method: &str) -> Option<(FnId, Vec<Ty>)> {
        let key = self.type_key(receiver)?;
        let &id = self.methods.get(&(key, method.to_owned()))?;
        let sig = self.fns.get(id.0)?;
        let mut bound: Vec<Option<Ty>> = vec![None; sig.generics.len()];
        if let Some(own) = sig.params.first() {
            self.bind(own, receiver, &mut bound);
        }
        let args = bound.into_iter().map(|t| t.unwrap_or(Ty::Error)).collect();
        Some((id, args))
    }
}

/// What a call inside a function reaches, to collect instances.
#[derive(Clone)]
enum Callee {
    /// A generic function, with its type arguments.
    Fn(FnId, Vec<Ty>),
    /// A method of an interface, by name, on a value of this type.
    Interface(String, Ty),
}

/// See [`Analysis::dyn_targets`].
pub(crate) type DynTargets = HashMap<(ExprId, Vec<Ty>), (FnId, Vec<Ty>)>;

/// A use of a generic declaration: a call, a constructor, a variant.
pub(crate) struct GenericUse<'u> {
    /// Its name, for messages.
    pub name: &'u str,
    /// The whole use.
    pub span: Span,
    pub generics: &'u [GenericParam],
    /// The types its arguments take, in its own terms.
    pub params: &'u [Ty],
    /// The type of the use, in its own terms.
    pub ret: &'u Ty,
    /// For a method called on a value: its type, and where it is. It is the
    /// first of `params`.
    pub receiver: Option<(&'u Ty, Span)>,
}

/// Whether a type mentions the type parameter of this index, directly or in
/// the arguments of a generic structure written in it.
fn mentions(ty: &Ty, index: u32) -> bool {
    match ty {
        Ty::Param(i, _) => *i == index,
        Ty::Array(t) | Ty::Option(t) => mentions(t, index),
        Ty::Result(a, b) => mentions(a, index) || mentions(b, index),
        // An instance's arguments are not visible from here: assume it does.
        Ty::Struct(..) | Ty::Enum(..) => true,
        _ => false,
    }
}

/// `Pair<i32, String>`.
fn instance_name(template: &str, args: &[Ty]) -> String {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    // A tuple: `(i32, String)`.
    if template == "()" {
        return format!("({})", args.join(", "));
    }
    format!("{template}<{}>", args.join(", "))
}

impl Analysis {
    /// `ty` with each type parameter replaced by its argument, for the
    /// instance of a generic function being compiled. Every structure and
    /// enumeration it needs was made by the checker.
    #[must_use]
    pub fn subst(&self, ty: &Ty, args: &[Ty]) -> Ty {
        if args.is_empty() {
            return ty.clone();
        }
        match ty {
            Ty::Param(i, _) => usize::try_from(*i)
                .ok()
                .and_then(|i| args.get(i))
                .cloned()
                .unwrap_or(Ty::Error),
            Ty::Array(t) => Ty::Array(Box::new(self.subst(t, args))),
            Ty::Option(t) => Ty::Option(Box::new(self.subst(t, args))),
            Ty::Result(a, b) => {
                Ty::Result(Box::new(self.subst(a, args)), Box::new(self.subst(b, args)))
            }
            Ty::Struct(id, _) => {
                let Some(Some((template, inner))) = self.struct_origins.get(id.0) else {
                    return ty.clone();
                };
                let inner: Vec<Ty> = inner.iter().map(|t| self.subst(t, args)).collect();
                self.struct_instances
                    .get(&(*template, inner))
                    .and_then(|i| {
                        self.structs
                            .get(i.0)
                            .map(|(n, _)| Ty::Struct(*i, n.clone()))
                    })
                    .unwrap_or_else(|| ty.clone())
            }
            Ty::Enum(id, _) => {
                let Some(Some((template, inner))) = self.enum_origins.get(id.0) else {
                    return ty.clone();
                };
                let inner: Vec<Ty> = inner.iter().map(|t| self.subst(t, args)).collect();
                self.enum_instances
                    .get(&(*template, inner))
                    .and_then(|i| self.enums.get(i.0).map(|(n, _)| Ty::Enum(*i, n.clone())))
                    .unwrap_or_else(|| ty.clone())
            }
            other => other.clone(),
        }
    }

    /// Whether the function of this id is generic: compiled once per
    /// instance, not on its own.
    #[must_use]
    pub fn is_generic(&self, id: FnId) -> bool {
        self.fn_generics.get(id.0).is_some_and(|&n| n > 0)
    }
}

/// The instances of each generic template, for the tables of [`Analysis`].
pub(crate) type Instances<K> = HashMap<(K, Vec<Ty>), K>;
