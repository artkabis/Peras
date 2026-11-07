//! Enumerations and `match`: variants, patterns, and exhaustiveness.
//!
//! A `match` must cover every value its scrutinee can have: every variant of
//! an enumeration or an option, `true` and `false`, or, for numbers and
//! strings, a final `_` (`E0222`). An arm that no value can reach, after a
//! catch-all or a variant already covered, is an error too (`E0224`): it is
//! dead code, or a mistake in the order of the arms.

use perasc_diag::Span;
use perasc_syntax::ast::{Arm, ArmBody, ExprId, ExprKind, Ident, Pattern, PatternKind};

use crate::scope::{self, Kind};
use crate::ty::Ty;
use crate::{Checker, errors};

/// What a pattern covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Covers {
    /// Every value: `_`, a name.
    All,
    /// One variant; all of it when its data patterns cover everything.
    Variant(usize, bool),
    /// One boolean.
    Bool(bool),
    /// Some values only: a number, a variant with a refutable part.
    Some,
}

impl Checker<'_> {
    /// `Enum.Variant` (`args` is `None`) or `Enum.Variant(args)`, when `base`
    /// names an enumeration; `None` otherwise.
    pub(crate) fn enum_variant(
        &mut self,
        base: ExprId,
        name: &Ident,
        args: Option<&[ExprId]>,
        span: Span,
    ) -> Option<Ty> {
        let ExprKind::Name(symbol) = self.ast.expr(base).kind else {
            return None;
        };
        let index = self.scopes.lookup(symbol)?;
        let binding = self.scopes.bindings.get_mut(index)?;
        let Kind::Enum(id) = binding.kind else {
            return None;
        };
        binding.used = true;
        self.resolved.insert(base, index);
        let ty = Ty::Enum(id, binding.name.clone());
        let variants = self.variants_of(&ty).unwrap_or_default();
        let text = self.name(name.symbol).to_owned();
        let Some((_, fields)) = variants.iter().find(|(v, _)| *v == text) else {
            self.unknown_variant(&ty, &text, name.span);
            for &arg in args.unwrap_or(&[]) {
                self.infer(arg, None);
            }
            return Some(Ty::Error);
        };
        let given = args.map_or(0, <[ExprId]>::len);
        if given != fields.len() {
            self.report(errors::argument_count(
                span,
                &text,
                fields.len(),
                given,
                None,
            ));
        }
        for (i, &arg) in args.unwrap_or(&[]).iter().enumerate() {
            match fields.get(i) {
                Some(field) => self.expect(arg, field),
                None => {
                    self.infer(arg, None);
                }
            }
        }
        Some(ty)
    }

    /// `E0223`, with the closest variant name.
    fn unknown_variant(&mut self, ty: &Ty, name: &str, span: Span) {
        let variants: Vec<String> = self
            .variants_of(ty)
            .unwrap_or_default()
            .into_iter()
            .map(|(v, _)| v)
            .collect();
        let suggestion =
            scope::suggest(name, variants.iter().map(String::as_str)).map(str::to_owned);
        self.report(errors::unknown_variant(
            span,
            ty,
            name,
            suggestion.as_deref(),
        ));
    }

    /// `match (scrutinee) { arms }`: its type is the arms' common type when
    /// they are all values, `void` when one is a block.
    pub(crate) fn match_expr(
        &mut self,
        scrutinee: ExprId,
        arms: &[Arm],
        span: Span,
        expected: Option<&Ty>,
    ) -> Ty {
        let ty = self.infer(scrutinee, None).defaulted();
        let variants = self.variants_of(&ty);
        let mut covered = vec![false; variants.as_ref().map_or(0, Vec::len)];
        let mut bools = [false, false];
        let mut catch_all = false;
        let mut result: Option<Ty> = expected.filter(|t| **t != Ty::Void).cloned();
        let mut values = true;
        for arm in arms {
            self.scopes.push();
            let covers = self.pattern(&arm.pattern, &ty);
            let reachable = !catch_all
                && match covers {
                    Covers::Variant(i, _) => !covered.get(i).copied().unwrap_or(false),
                    Covers::Bool(b) => !bools[usize::from(b)],
                    Covers::All | Covers::Some => true,
                };
            if !reachable && ty != Ty::Error {
                self.report(errors::unreachable_arm(arm.pattern.span));
            }
            match covers {
                Covers::All => catch_all = true,
                Covers::Variant(i, true) => {
                    if let Some(slot) = covered.get_mut(i) {
                        *slot = true;
                    }
                }
                Covers::Bool(b) => bools[usize::from(b)] = true,
                Covers::Variant(_, false) | Covers::Some => {}
            }
            match &arm.body {
                ArmBody::Expr(value) => match &result {
                    Some(t) => self.expect(*value, t),
                    None => result = Some(self.infer(*value, None).defaulted()),
                },
                ArmBody::Block(block) => {
                    values = false;
                    self.block(&block.stmts);
                }
            }
            self.pop_scope();
        }
        if !catch_all && ty != Ty::Error {
            let missing: Vec<String> = match &variants {
                Some(variants) => variants
                    .iter()
                    .zip(&covered)
                    .filter(|(_, c)| !**c)
                    .map(|((name, fields), _)| pattern_text(name, fields.len()))
                    .collect(),
                None if ty == Ty::Bool => [false, true]
                    .into_iter()
                    .filter(|b| !bools[usize::from(*b)])
                    .map(|b| b.to_string())
                    .collect(),
                None => vec!["_".to_owned()],
            };
            if !missing.is_empty() {
                let close = span.end.saturating_sub(1);
                self.report(errors::non_exhaustive(span, &ty, &missing, close));
            }
        }
        if values {
            result.unwrap_or(Ty::Void)
        } else {
            Ty::Void
        }
    }

    /// Checks a pattern against the type it matches, declaring the names it
    /// gives; what it covers.
    fn pattern(&mut self, pattern: &Pattern, ty: &Ty) -> Covers {
        let variants = self.variants_of(ty);
        match &pattern.kind {
            PatternKind::Wildcard => Covers::All,
            PatternKind::Name(ident) => {
                let text = self.name(ident.symbol).to_owned();
                let found = variants
                    .as_ref()
                    .and_then(|v| v.iter().position(|(n, _)| *n == text));
                if let (Some(index), Some(variants)) = (found, &variants) {
                    let fields = variants.get(index).map_or(0, |(_, f)| f.len());
                    if fields > 0 {
                        self.report(errors::argument_count(pattern.span, &text, fields, 0, None));
                    }
                    return Covers::Variant(index, true);
                }
                self.declare(ident.symbol, Some(ident.span), Kind::Pattern, ty.clone());
                Covers::All
            }
            PatternKind::Literal(value) => {
                if !(ty.is_integer() || *ty == Ty::Bool || *ty == Ty::Error) {
                    let span = self.ast.expr(*value).span;
                    self.report(errors::invalid_operator(span, "match", ty));
                    return Covers::Some;
                }
                self.expect(*value, ty);
                match self.ast.expr(*value).kind {
                    ExprKind::Bool(b) => Covers::Bool(b),
                    _ => Covers::Some,
                }
            }
            PatternKind::Variant {
                qualifier,
                name,
                fields,
            } => {
                let Some(variants) = variants else {
                    if *ty != Ty::Error {
                        let text = self.name(name.symbol).to_owned();
                        self.report(errors::unknown_variant(name.span, ty, &text, None));
                    }
                    return Covers::Some;
                };
                if let Some(qualifier) = qualifier {
                    let written = self.name(qualifier.symbol);
                    if written != ty.to_string() {
                        let found = Ty::Struct(crate::ty::StructId(usize::MAX), written.to_owned());
                        self.report(errors::mismatch(qualifier.span, ty, &found, None));
                    }
                }
                let text = self.name(name.symbol).to_owned();
                let Some(index) = variants.iter().position(|(n, _)| *n == text) else {
                    self.unknown_variant(ty, &text, name.span);
                    return Covers::Some;
                };
                let types = variants
                    .get(index)
                    .map(|(_, f)| f.clone())
                    .unwrap_or_default();
                if types.len() != fields.len() {
                    self.report(errors::argument_count(
                        pattern.span,
                        &text,
                        types.len(),
                        fields.len(),
                        None,
                    ));
                }
                let mut full = true;
                for (field, field_ty) in fields.iter().zip(&types) {
                    full &= self.pattern(field, field_ty) == Covers::All;
                }
                Covers::Variant(index, full)
            }
        }
    }
}

/// How a missing variant is written in an arm: `Circle(_)`, `Empty`.
fn pattern_text(name: &str, fields: usize) -> String {
    if fields == 0 {
        name.to_owned()
    } else {
        format!("{name}({})", vec!["_"; fields].join(", "))
    }
}
