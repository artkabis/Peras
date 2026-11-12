//! A map lookup done once (decision 0016, finding F210): the entry a lookup
//! of a key found is kept, and a later lookup of the same key in the same
//! map reads it, as long as nothing can have changed the answer between.
//! This is the suppression of a redundant computation (Morel and Renvoise,
//! 1979) for one kind of computation, conservative where it cannot prove.
//!
//! `m.has(k)` then `m.get(k)`, or the same with `m.set(k, …)` between when the
//! key was missing, look the key up once: the `set` gives the entry it made,
//! which is the one the `get` would find.
//!
//! What keeps a lookup's answer, an entry number:
//! - the map unchanged: any `set`, `delete` or `clear` of the map forgets it,
//!   since a map that grows after a delete renumbers its entries; a `set` of
//!   the same key gives the answer again;
//! - the map and the key not assigned, nor released: a string key may be lent
//!   from a buffer that a release, a store into an array of strings or a
//!   truncation frees, so these forget every string key;
//! - no loop between: a loop forgets everything, its body too at each turn;
//! - after an `if`, only what both branches keep, a branch that leaves being
//!   no path there.
//!
//! It replaces a lookup by the read of an `i32` local and adds `i32` locals:
//! no buffer is moved, kept or released, so the releases stay what they were.

use crate::{BufOp, BufStmt, Elem, Expr, ExprKind, Function, Local, LocalId, RuntimeFn, Stmt, Ty};

/// The key of a lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    /// A string, lent from this local.
    Text(u32),
    /// A string literal only read, which never changes (`BufOp::Static`).
    Literal(u32),
    /// An integer held by this local.
    Number(u32),
    /// An integer constant.
    Constant(i64),
}

/// A lookup whose answer a local holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Known {
    map: u32,
    key: Key,
    holder: LocalId,
}

/// The local an operand names, lent or read.
fn named(e: &Expr) -> Option<u32> {
    match e.kind {
        ExprKind::Borrow(local) | ExprKind::Local(local) => Some(local.0),
        _ => None,
    }
}

/// The key a string operand names: a local, or a static literal.
fn text_key(e: &Expr) -> Option<Key> {
    match e.kind {
        ExprKind::Buf(BufOp::Static(segment, _), _) => Some(Key::Literal(segment)),
        _ => named(e).map(Key::Text),
    }
}

/// The key an integer operand names: a local, read or widened, or a
/// constant. A local holds one type, so it is always widened the same way.
fn number_key(e: &Expr) -> Option<Key> {
    match &e.kind {
        ExprKind::Local(local) => Some(Key::Number(local.0)),
        ExprKind::Convert(inner) if e.ty == Ty::I64 => match inner.kind {
            ExprKind::Local(local) if inner.ty == Ty::I32 => Some(Key::Number(local.0)),
            _ => None,
        },
        ExprKind::Const(crate::Const::Int(n)) => Some(Key::Constant(*n)),
        _ => None,
    }
}

/// The map and key of a lookup, if `e` is one.
fn lookup(e: &Expr) -> Option<(u32, Key)> {
    let ExprKind::Buf(BufOp::Runtime(f), args) = &e.kind else {
        return None;
    };
    let [map, key] = args.as_slice() else {
        return None;
    };
    let map = named(map)?;
    let key = match f {
        RuntimeFn::MapFindS => text_key(key)?,
        RuntimeFn::MapFindI => number_key(key)?,
        _ => return None,
    };
    Some((map, key))
}

/// The map and key of a `set` that gives the entry of its key, if `e` is one.
fn insertion(e: &Expr) -> Option<(u32, Key)> {
    let ExprKind::Buf(BufOp::Runtime(f), args) = &e.kind else {
        return None;
    };
    // A `set` given the entry: its key after it.
    let (f, map, key) = match (f, args.as_slice()) {
        (RuntimeFn::MapPutAtS, [map, _, key]) => (RuntimeFn::MapPutS, map, key),
        (RuntimeFn::MapPutAtI, [map, _, key]) => (RuntimeFn::MapPutI, map, key),
        (RuntimeFn::MapPutAtSLent, [map, _, key]) => (RuntimeFn::MapPutSLent, map, key),
        (f, [map, key]) => (*f, map, key),
        _ => return None,
    };
    let map = named(map)?;
    let key = match f {
        RuntimeFn::MapPutS | RuntimeFn::MapPutSLent => text_key(key)?,
        RuntimeFn::MapPutI => number_key(key)?,
        _ => return None,
    };
    Some((map, key))
}

/// The maps `e` changes, in any of its parts.
fn changed_maps(e: &Expr, out: &mut Vec<u32>) {
    if let ExprKind::Buf(BufOp::Runtime(f), args) = &e.kind
        && matches!(
            f,
            RuntimeFn::MapPutI
                | RuntimeFn::MapPutS
                | RuntimeFn::MapPutSLent
                | RuntimeFn::MapPutAtI
                | RuntimeFn::MapPutAtS
                | RuntimeFn::MapPutAtSLent
                | RuntimeFn::MapDeleteI
                | RuntimeFn::MapDeleteS
                | RuntimeFn::MapClear
        )
        && let Some(map) = args.first().and_then(named)
    {
        out.push(map);
    }
    for child in children(e) {
        changed_maps(child, out);
    }
}

fn children(e: &Expr) -> Vec<&Expr> {
    match &e.kind {
        ExprKind::Unary(_, a)
        | ExprKind::Convert(a)
        | ExprKind::Math(_, a)
        | ExprKind::Alloc(a) => {
            vec![a]
        }
        ExprKind::Binary(_, a, b) => vec![a, b],
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => args.iter().collect(),
        ExprKind::Const(_) | ExprKind::Local(_) | ExprKind::Move(_) | ExprKind::Borrow(_) => {
            Vec::new()
        }
    }
}

/// The map and key of a `set` that takes its key, and the function that
/// does it with the entry known, if `e` is one.
fn put(e: &Expr) -> Option<(u32, Key, RuntimeFn)> {
    let ExprKind::Buf(BufOp::Runtime(f), args) = &e.kind else {
        return None;
    };
    let [map, key] = args.as_slice() else {
        return None;
    };
    let map = named(map)?;
    match f {
        RuntimeFn::MapPutS => {
            let key = match key.kind {
                ExprKind::Move(local) => Key::Text(local.0),
                _ => text_key(key)?,
            };
            Some((map, key, RuntimeFn::MapPutAtS))
        }
        RuntimeFn::MapPutSLent => Some((map, text_key(key)?, RuntimeFn::MapPutAtSLent)),
        RuntimeFn::MapPutI => Some((map, number_key(key)?, RuntimeFn::MapPutAtI)),
        _ => None,
    }
}

/// `e` with each lookup known replaced by the read of its holder, and each
/// `set` of a key known given the entry, so that it does not look the key
/// up again (decision 0017).
fn reuse(e: &mut Expr, known: &[Known]) {
    if let Some((map, key)) = lookup(e)
        && let Some(k) = known.iter().find(|k| k.map == map && k.key == key)
    {
        *e = Expr {
            kind: ExprKind::Local(k.holder),
            ty: Ty::I32,
        };
        return;
    }
    if let Some((map, key, at)) = put(e)
        && let Some(k) = known.iter().find(|k| k.map == map && k.key == key)
        && let ExprKind::Buf(_, args) = &mut e.kind
    {
        let mut args = std::mem::take(args);
        args.insert(
            1,
            Expr {
                kind: ExprKind::Local(k.holder),
                ty: Ty::I32,
            },
        );
        e.kind = ExprKind::Buf(BufOp::Runtime(at), args);
        return;
    }
    match &mut e.kind {
        ExprKind::Unary(_, a)
        | ExprKind::Convert(a)
        | ExprKind::Math(_, a)
        | ExprKind::Alloc(a) => {
            reuse(a, known);
        }
        ExprKind::Binary(_, a, b) => {
            reuse(a, known);
            reuse(b, known);
        }
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => {
            for arg in args {
                reuse(arg, known);
            }
        }
        ExprKind::Const(_) | ExprKind::Local(_) | ExprKind::Move(_) | ExprKind::Borrow(_) => {}
    }
}

/// Whether `e` holds a lookup.
fn contains_lookup(e: &Expr) -> bool {
    lookup(e).is_some() || children(e).into_iter().any(contains_lookup)
}

/// The first lookup in `e`, taken out into the new local `holder`.
fn take_lookup(e: &mut Expr, holder: LocalId) -> Option<(Expr, u32, Key)> {
    if let Some((map, key)) = lookup(e) {
        let found = std::mem::replace(
            e,
            Expr {
                kind: ExprKind::Local(holder),
                ty: Ty::I32,
            },
        );
        return Some((found, map, key));
    }
    match &mut e.kind {
        ExprKind::Unary(_, a)
        | ExprKind::Convert(a)
        | ExprKind::Math(_, a)
        | ExprKind::Alloc(a) => take_lookup(a, holder),
        ExprKind::Binary(_, a, b) => take_lookup(a, holder).or_else(|| take_lookup(b, holder)),
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => {
            args.iter_mut().find_map(|arg| take_lookup(arg, holder))
        }
        ExprKind::Const(_) | ExprKind::Local(_) | ExprKind::Move(_) | ExprKind::Borrow(_) => None,
    }
}

/// Forgets what local `x` being assigned, or released when `owned`, can
/// change.
fn forget_local(known: &mut Vec<Known>, x: u32, owned: bool) {
    known.retain(|k| {
        let names = k.map == x
            || k.holder.0 == x
            || matches!(k.key, Key::Text(t) | Key::Number(t) if t == x);
        // A buffer released may be the one a string key is lent from.
        !(names || owned && matches!(k.key, Key::Text(_)))
    });
}

fn forget_maps(known: &mut Vec<Known>, e: &Expr) {
    let mut maps = Vec::new();
    changed_maps(e, &mut maps);
    known.retain(|k| !maps.contains(&k.map));
}

/// Whether the statements end by leaving: what follows them is not reached
/// from them.
fn leaves(stmts: &[Stmt]) -> bool {
    matches!(
        stmts.last(),
        Some(Stmt::Return(_) | Stmt::ReturnMany(_) | Stmt::Break | Stmt::Continue | Stmt::Leave(_))
    )
}

/// `x = value`: a lookup known read, then what it changes forgotten, and
/// what it finds or makes known.
fn set(x: LocalId, mut value: Expr, known: &mut Vec<Known>) -> Stmt {
    reuse(&mut value, known);
    forget_maps(known, &value);
    let found = lookup(&value).or_else(|| insertion(&value));
    // An assignment frees nothing: a buffer is freed by a release, which
    // comes before.
    forget_local(known, x.0, false);
    if let Some((map, key)) = found
        && map != x.0
        && !matches!(key, Key::Text(t) | Key::Number(t) if t == x.0)
    {
        known.push(Known {
            map,
            key,
            holder: x,
        });
    }
    Stmt::Set(x, value)
}

/// An operation on a buffer, and what it can change forgotten.
fn buf(op: BufStmt, mut args: Vec<Expr>, known: &mut Vec<Known>) -> Stmt {
    for arg in &mut args {
        reuse(arg, known);
        forget_maps(known, arg);
    }
    match op {
        // A string's bytes, or an array of strings changed: a string key may
        // be lent from it.
        BufStmt::Set(Elem::Handle | Elem::U8)
        | BufStmt::Truncate
        | BufStmt::Release
        | BufStmt::Append(_)
        | BufStmt::Push(Elem::U8) => {
            known.retain(|k| !matches!(k.key, Key::Text(_)));
        }
        _ => {}
    }
    if let Some(t) = args.first().and_then(named) {
        forget_local(known, t, false);
    }
    Stmt::Buf(op, args)
}

struct Pass<'a> {
    locals: &'a mut Vec<Local>,
}

impl Pass<'_> {
    fn temp(&mut self) -> LocalId {
        let id = LocalId(u32::try_from(self.locals.len()).unwrap_or(u32::MAX));
        self.locals.push(Local {
            name: "found".to_owned(),
            ty: Ty::I32,
        });
        id
    }

    /// What is known after an `if`: what both branches know. An answer the
    /// two branches hold in different locals goes into a new one, set at the
    /// end of each: a local of the program is never set where it was not.
    fn merge(
        &mut self,
        (then, after_then): (&mut Vec<Stmt>, Vec<Known>),
        (otherwise, after_otherwise): (&mut Vec<Stmt>, Vec<Known>),
    ) -> Vec<Known> {
        if leaves(then) {
            return after_otherwise;
        }
        if leaves(otherwise) {
            return after_then;
        }
        let mut known = Vec::new();
        for t in after_then {
            let Some(o) = after_otherwise
                .iter()
                .find(|o| o.map == t.map && o.key == t.key)
            else {
                continue;
            };
            if o.holder == t.holder {
                known.push(t);
                continue;
            }
            let holder = self.temp();
            for (branch, from) in [(&mut *then, t.holder), (&mut *otherwise, o.holder)] {
                branch.push(Stmt::Set(
                    holder,
                    Expr {
                        kind: ExprKind::Local(from),
                        ty: Ty::I32,
                    },
                ));
            }
            known.push(Known { holder, ..t });
        }
        known
    }

    /// An `if`: a lookup in its condition taken out before it, then each
    /// branch, then what both know.
    fn if_stmt(
        &mut self,
        mut cond: Expr,
        then: Vec<Stmt>,
        otherwise: Vec<Stmt>,
        known: &mut Vec<Known>,
        out: &mut Vec<Stmt>,
    ) {
        reuse(&mut cond, known);
        // A lookup in the condition: taken out, so that a branch and what
        // follows can read its answer.
        if contains_lookup(&cond) {
            let holder = self.temp();
            if let Some((found, map, key)) = take_lookup(&mut cond, holder) {
                out.push(Stmt::Set(holder, found));
                known.push(Known { map, key, holder });
            }
        }
        forget_maps(known, &cond);
        let (mut then, after_then) = self.block(then, known.clone());
        let (mut otherwise, after_otherwise) = self.block(otherwise, known.clone());
        *known = self.merge((&mut then, after_then), (&mut otherwise, after_otherwise));
        out.push(Stmt::If {
            cond,
            then,
            otherwise,
        });
    }

    /// The statements rewritten, with what is known after them.
    fn block(&mut self, stmts: Vec<Stmt>, mut known: Vec<Known>) -> (Vec<Stmt>, Vec<Known>) {
        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            match stmt {
                Stmt::Set(x, value) => out.push(set(x, value, &mut known)),
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                } => self.if_stmt(cond, then, otherwise, &mut known, &mut out),
                Stmt::Loop { cond, body, step } => {
                    // Nothing is known in a loop, nor after it.
                    let (body, _) = self.block(body, Vec::new());
                    let (step, _) = self.block(step, Vec::new());
                    known.clear();
                    out.push(Stmt::Loop { cond, body, step });
                }
                Stmt::Block(body) => {
                    let (body, _) = self.block(body, known.clone());
                    known.clear();
                    out.push(Stmt::Block(body));
                }
                Stmt::Release(x) => {
                    forget_local(&mut known, x.0, true);
                    out.push(Stmt::Release(x));
                }
                Stmt::Buf(op, args) => out.push(buf(op, args, &mut known)),
                Stmt::Expr(mut e) => {
                    reuse(&mut e, &known);
                    forget_maps(&mut known, &e);
                    out.push(Stmt::Expr(e));
                }
                Stmt::CallMany {
                    targets,
                    callee,
                    mut args,
                } => {
                    for arg in &mut args {
                        reuse(arg, &known);
                        forget_maps(&mut known, arg);
                    }
                    for t in &targets {
                        forget_local(&mut known, t.0, false);
                    }
                    out.push(Stmt::CallMany {
                        targets,
                        callee,
                        args,
                    });
                }
                Stmt::Return(Some(mut e)) => {
                    reuse(&mut e, &known);
                    known.clear();
                    out.push(Stmt::Return(Some(e)));
                }
                Stmt::ReturnMany(mut values) => {
                    for v in &mut values {
                        reuse(v, &known);
                    }
                    known.clear();
                    out.push(Stmt::ReturnMany(values));
                }
                jump @ (Stmt::Return(None) | Stmt::Break | Stmt::Continue | Stmt::Leave(_)) => {
                    known.clear();
                    out.push(jump);
                }
            }
        }
        (out, known)
    }
}

/// Lookups done once in `function`.
pub fn reuse_lookups(function: &mut Function) {
    let body = std::mem::take(&mut function.body);
    let mut pass = Pass {
        locals: &mut function.locals,
    };
    function.body = pass.block(body, Vec::new()).0;
}
