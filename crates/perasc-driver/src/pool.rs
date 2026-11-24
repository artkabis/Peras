//! Decision 0044, prototypes p1 and p2: `Pool<T>` added to a program that
//! names it, its source in `pool/p1.peras` or `pool/p2.peras`, and each
//! `pool[h]` written again as `pool.values[pool.slotOrFault(h)]`, which the
//! compiler already builds everywhere a place is read or written.

use std::borrow::Cow;

use perasc_syntax::ast::{ExprId, ExprKind};

/// The source of the pool of a prototype, if `PERAS_PROTO_POOL` names one.
fn prelude() -> Option<&'static str> {
    match std::env::var("PERAS_PROTO_POOL").as_deref() {
        Ok("p1") => Some(include_str!("pool/p1.peras")),
        Ok("p2") => Some(include_str!("pool/p2.peras")),
        _ => None,
    }
}

/// Whether an expression is a name or a field of one: written twice, it
/// reads the same place twice.
fn is_place(ast: &perasc_syntax::ast::Ast, id: ExprId) -> bool {
    match &ast.expr(id).kind {
        ExprKind::Name(_) => true,
        ExprKind::Field { base, .. } => is_place(ast, *base),
        ExprKind::Paren(inner) => is_place(ast, *inner),
        _ => false,
    }
}

/// The source as the prototype builds it: unchanged unless a prototype is
/// set and the source names `Pool`; the pool's source after it, so that the
/// places of the program stay where they are; each `pool[h]` written again.
#[must_use]
pub fn prepare(source: &str) -> Cow<'_, str> {
    let Some(pool) = prelude() else {
        return Cow::Borrowed(source);
    };
    if !source.contains("Pool") {
        return Cow::Borrowed(source);
    }
    let mut text = format!("{source}\n{pool}");
    // One at a time, the last first, so that one inside another's index is
    // written before it; each pass analyses the text again.
    for _ in 0..10_000 {
        let parsed = perasc_syntax::parse(&text);
        if !parsed.diagnostics.is_empty() {
            break;
        }
        let analysis = perasc_check::analyze(&parsed.ast, &text);
        let Some(&at) = analysis
            .pool_indexes
            .iter()
            .max_by_key(|&&id| parsed.ast.expr(id).span.start)
        else {
            break;
        };
        let ExprKind::Index { base, index } = parsed.ast.expr(at).kind else {
            break;
        };
        if !is_place(&parsed.ast, base) {
            break;
        }
        let slice = |id: ExprId| {
            let s = parsed.ast.expr(id).span;
            text.get(s.start as usize..s.end as usize)
                .unwrap_or("")
                .to_owned()
        };
        let (b, i) = (slice(base), slice(index));
        let whole = parsed.ast.expr(at).span;
        text.replace_range(
            whole.start as usize..whole.end as usize,
            &format!("{b}.values[{b}.slotOrFault({i})]"),
        );
    }
    Cow::Owned(text)
}
