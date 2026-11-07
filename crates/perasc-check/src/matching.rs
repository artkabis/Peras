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
        let mut result: Option<Ty> = expected.filter(|t| **t != Ty::Void).cloned();
        let mut values = true;
        let mut rows: Vec<Vec<Pat>> = Vec::new();
        for arm in arms {
            self.scopes.push();
            let pat = self.pattern(&arm.pattern, &ty);
            if ty != Ty::Error
                && !self.useful(&rows, std::slice::from_ref(&pat), std::slice::from_ref(&ty))
            {
                self.report(errors::unreachable_arm(arm.pattern.span));
            }
            rows.push(vec![pat]);
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
        if ty != Ty::Error {
            let missing = self.missing(&rows, &ty);
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

    /// The constructors of a type that has finitely many: its variants with
    /// the types of their data, or `false` and `true`.
    fn constructors(&self, ty: &Ty) -> Option<Vec<(String, Vec<Ty>)>> {
        if *ty == Ty::Bool {
            return Some(vec![
                ("false".to_owned(), Vec::new()),
                ("true".to_owned(), Vec::new()),
            ]);
        }
        self.variants_of(ty)
    }

    /// The values of `ty` no row covers, as patterns to write: `Empty`,
    /// `Err(_)`, `false`, or `_`.
    fn missing(&self, rows: &[Vec<Pat>], ty: &Ty) -> Vec<String> {
        let types = std::slice::from_ref(ty);
        match self.constructors(ty) {
            Some(constructors) => constructors
                .iter()
                .enumerate()
                .filter(|(c, (_, fields))| {
                    let row = vec![Pat::Any; fields.len()];
                    self.useful(&specialize(rows, *c, fields.len()), &row, fields)
                })
                .map(|(_, (name, fields))| pattern_text(name, fields.len()))
                .collect(),
            None if self.useful(rows, &[Pat::Any], types) => vec!["_".to_owned()],
            None => Vec::new(),
        }
    }

    /// Whether a row of patterns matches a value no row of `matrix` matches:
    /// the algorithm of Maranget, "Warnings for pattern matching" (2007).
    fn useful(&self, matrix: &[Vec<Pat>], row: &[Pat], types: &[Ty]) -> bool {
        let (Some(first), Some(ty)) = (row.first(), types.first()) else {
            return matrix.is_empty();
        };
        let rest = row.get(1..).unwrap_or_default();
        let rest_types = types.get(1..).unwrap_or_default();
        let constructors = self.constructors(ty);
        let fields_of = |c: usize| -> Vec<Ty> {
            constructors
                .as_ref()
                .and_then(|cs| cs.get(c))
                .map(|(_, f)| f.clone())
                .unwrap_or_default()
        };
        let with =
            |head: Vec<Pat>| -> Vec<Pat> { head.into_iter().chain(rest.iter().cloned()).collect() };
        let with_types = |head: Vec<Ty>| -> Vec<Ty> {
            head.into_iter().chain(rest_types.iter().cloned()).collect()
        };
        match first {
            Pat::Ctor(c, args) => {
                let fields = fields_of(*c);
                self.useful(
                    &specialize(matrix, *c, args.len()),
                    &with(args.clone()),
                    &with_types(fields),
                )
            }
            Pat::Lit(value) => {
                let kept: Vec<Vec<Pat>> = matrix
                    .iter()
                    .filter(|r| {
                        matches!(r.first(), Some(Pat::Any)) || r.first() == Some(&Pat::Lit(*value))
                    })
                    .map(|r| r.get(1..).unwrap_or_default().to_vec())
                    .collect();
                self.useful(&kept, rest, rest_types)
            }
            Pat::Any => {
                if let Some(cs) = &constructors {
                    return cs.iter().enumerate().any(|(c, (_, fields))| {
                        let row = with(vec![Pat::Any; fields.len()]);
                        self.useful(
                            &specialize(matrix, c, fields.len()),
                            &row,
                            &with_types(fields.clone()),
                        )
                    });
                }
                // Infinitely many values: only the rows matching anything remain.
                let defaults: Vec<Vec<Pat>> = matrix
                    .iter()
                    .filter(|r| matches!(r.first(), Some(Pat::Any)))
                    .map(|r| r.get(1..).unwrap_or_default().to_vec())
                    .collect();
                self.useful(&defaults, rest, rest_types)
            }
        }
    }

    /// Checks a pattern against the type it matches, declaring the names it
    /// gives; the pattern, for the exhaustiveness check.
    fn pattern(&mut self, pattern: &Pattern, ty: &Ty) -> Pat {
        let variants = self.variants_of(ty);
        match &pattern.kind {
            PatternKind::Wildcard => Pat::Any,
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
                    return Pat::Ctor(index, vec![Pat::Any; fields]);
                }
                self.declare(ident.symbol, Some(ident.span), Kind::Pattern, ty.clone());
                Pat::Any
            }
            PatternKind::Literal(value) => {
                if !(ty.is_integer() || *ty == Ty::Bool || *ty == Ty::Error) {
                    let span = self.ast.expr(*value).span;
                    self.report(errors::invalid_operator(span, "match", ty));
                    return Pat::Any;
                }
                self.expect(*value, ty);
                if let ExprKind::Bool(b) = self.ast.expr(*value).kind {
                    return Pat::Ctor(usize::from(b), Vec::new());
                }
                match self.literal_value(*value) {
                    Some(n) => Pat::Lit(n),
                    None => Pat::Any,
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
                    return Pat::Any;
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
                    // Matches nothing: the arms after it stay reachable.
                    return Pat::Ctor(usize::MAX, Vec::new());
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
                    return Pat::Any;
                }
                let args = fields
                    .iter()
                    .zip(&types)
                    .map(|(field, field_ty)| self.pattern(field, field_ty))
                    .collect();
                Pat::Ctor(index, args)
            }
        }
    }

    /// The value of an integer literal pattern, negated if written `-n`.
    fn literal_value(&self, id: ExprId) -> Option<i128> {
        match self.ast.expr(id).kind {
            ExprKind::Int(n) => Some(i128::from(n)),
            ExprKind::Paren(inner) => self.literal_value(inner),
            ExprKind::Unary { operand, .. } => self.literal_value(operand).map(|n| -n),
            _ => None,
        }
    }
}

/// A pattern as the exhaustiveness check sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pat {
    /// `_`, a name: anything.
    Any,
    /// The constructor of this index (a variant, `false` or `true`), with
    /// patterns for its data.
    Ctor(usize, Vec<Pat>),
    /// One integer.
    Lit(i128),
}

/// The rows of `matrix` that can match constructor `c`, with its data in
/// place of their first pattern.
fn specialize(matrix: &[Vec<Pat>], c: usize, arity: usize) -> Vec<Vec<Pat>> {
    matrix
        .iter()
        .filter_map(|row| {
            let rest = row.get(1..).unwrap_or_default();
            match row.first() {
                Some(Pat::Any) => Some(
                    vec![Pat::Any; arity]
                        .into_iter()
                        .chain(rest.iter().cloned())
                        .collect(),
                ),
                Some(Pat::Ctor(other, args)) if *other == c => {
                    Some(args.iter().cloned().chain(rest.iter().cloned()).collect())
                }
                _ => None,
            }
        })
        .collect()
}

/// How a missing variant is written in an arm: `Circle(_)`, `Empty`.
fn pattern_text(name: &str, fields: usize) -> String {
    if fields == 0 {
        name.to_owned()
    } else {
        format!("{name}({})", vec!["_"; fields].join(", "))
    }
}
