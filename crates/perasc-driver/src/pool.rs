//! Decision 0044, prototypes p1 and p2: `Pool<T>` added to a program that
//! names it, its source in `pool/p1.peras` or `pool/p2.peras`, and each
//! `pool[h]` written again as `pool.values[pool.slotOrFault(h)]`, which the
//! compiler already builds everywhere a place is read or written.

use std::borrow::Cow;

use perasc_syntax::ast::{ExprId, ExprKind};

/// The source of the pool of a prototype, if `PERAS_PROTO_POOL` names one.
fn prelude() -> Option<&'static str> {
    match std::env::var("PERAS_PROTO_POOL").as_deref() {
        Ok("p1" | "p4") => Some(include_str!("pool/p1.peras")),
        Ok("p2") => Some(include_str!("pool/p2.peras")),
        Ok("p3") => Some(include_str!("pool/p3.peras")),
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
    // p3 is imported: its import becomes blanks of the same length, so that
    // every place of the program stays where it was.
    let import = "import { Pool, Handle } from \"peras:pool\";";
    let mut text = if std::env::var("PERAS_PROTO_POOL").is_ok_and(|v| v == "p3") {
        if !source.contains(import) {
            return Cow::Borrowed(source);
        }
        format!(
            "{}\n{pool}",
            source.replace(import, &" ".repeat(import.len()))
        )
    } else {
        format!("{source}\n{pool}")
    };
    // p4: each walk over a pool, the last first, written as a walk over its
    // slots, its handle's generation checked once in its body.
    if std::env::var("PERAS_PROTO_POOL").is_ok_and(|v| v == "p4") {
        for n in 0..10_000 {
            let Some(next) = walk(&text, n) else {
                break;
            };
            text = next;
        }
    }
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

/// The text with its last walk over a pool written again, or `None` when
/// none is left (decision 0044, prototype p4): `for (const h of pool) body`
/// becomes a walk over the pool's slots, the handle of each live one made
/// as it is reached; in the body, `pool[h]` reads the slot found once,
/// before the first statement that reads through `h`, and found again after
/// a statement that may insert or remove.
fn walk(text: &str, n: usize) -> Option<String> {
    let parsed = perasc_syntax::parse(text);
    if !parsed.diagnostics.is_empty() {
        return None;
    }
    let analysis = perasc_check::analyze(&parsed.ast, text);
    let at = analysis.pool_loops.iter().max_by_key(|l| l.stmt.start)?;
    let slice = |s: perasc_diag::Span| text.get(s.start as usize..s.end as usize).unwrap_or("");
    let base = slice(parsed.ast.expr(at.iterable).span).to_owned();
    let name = slice(at.name).to_owned();
    let slot = format!("__ps{n}");
    // An edit: the range it replaces, and what it writes there.
    let mut edits: Vec<(u32, u32, String)> = Vec::new();
    let keyword = if at.mutable { "let" } else { "const" };
    edits.push((
        at.stmt.start,
        at.body.start,
        format!(
            "for (let {slot} = 0; {slot} < {base}.live.length; {slot}++) {{ if (!{base}.live[{slot}]) {{ continue; }} {keyword} {name} = Handle({slot}, {base}.gens[{slot}]); "
        ),
    ));
    edits.push((at.stmt.end, at.stmt.end, " }".to_owned()));
    if !at.mutable {
        // The reads `base[name]` of the body, by position.
        let reads: Vec<perasc_diag::Span> = analysis
            .pool_indexes
            .iter()
            .filter_map(|&id| {
                let ExprKind::Index { base: b, index } = parsed.ast.expr(id).kind else {
                    return None;
                };
                let is_name = matches!(parsed.ast.expr(index).kind, ExprKind::Name(_));
                let span = parsed.ast.expr(id).span;
                (is_name
                    && slice(parsed.ast.expr(b).span) == base
                    && slice(parsed.ast.expr(index).span) == name
                    && span.start >= at.body.start
                    && span.end <= at.body.end)
                    .then_some(span)
            })
            .collect();
        let mut checked: Option<String> = None;
        for (k, &(stmt, cond)) in at.stmts.iter().enumerate() {
            let changes = changes_pool(slice(stmt), &base);
            // Where the slot found holds: the whole statement when it may
            // not insert or remove; the condition of an `if` that does so
            // only in its blocks; nowhere otherwise.
            let region = if changes {
                cond.filter(|c| !changes_pool(slice(*c), &base))
            } else {
                Some(stmt)
            };
            if let Some(region) = region {
                let mine: Vec<_> = reads
                    .iter()
                    .filter(|r| r.start >= region.start && r.end <= region.end)
                    .collect();
                if !mine.is_empty() {
                    let found = checked.get_or_insert_with(|| {
                        let found = format!("__ph{n}_{k}");
                        edits.push((
                            stmt.start,
                            stmt.start,
                            format!("const {found} = {base}.slotOrFault({name}); "),
                        ));
                        found
                    });
                    for r in mine {
                        edits.push((r.start, r.end, format!("{base}.values[{found}]")));
                    }
                }
            }
            if changes {
                checked = None;
            }
        }
    }
    // The last first; at one place, a replaced range before what is
    // inserted there, so that the insertion comes first in the text.
    edits.sort_by(|a, b| b.0.cmp(&a.0).then((b.1 - b.0).cmp(&(a.1 - a.0))));
    let mut out = text.to_owned();
    for (start, end, with) in edits {
        out.replace_range(start as usize..end as usize, &with);
    }
    Some(out)
}

/// Whether a statement may insert into a pool or remove from it: it calls
/// one of its methods other than those that read, or names it whole, to pass
/// it to a function or give it another value.
fn changes_pool(stmt: &str, base: &str) -> bool {
    const READS: [&str; 7] = [
        "values",
        "gens",
        "live",
        "has",
        "get",
        "size",
        "slotOrFault",
    ];
    let bytes = stmt.as_bytes();
    let mut from = 0;
    while let Some(i) = stmt[from..].find(base) {
        let start = from + i;
        let end = start + base.len();
        from = end;
        let before = start.checked_sub(1).and_then(|p| bytes.get(p)).copied();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.') {
            continue;
        }
        match bytes.get(end) {
            Some(c) if c.is_ascii_alphanumeric() || *c == b'_' => {}
            Some(b'[') => {}
            Some(b'.') => {
                let method: String = stmt[end + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !READS.contains(&method.as_str()) {
                    return true;
                }
            }
            _ => return true,
        }
    }
    false
}
