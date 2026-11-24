//! Decision 0043, prototype a2: arenas chosen by the compiler.
//!
//! A *class* is a set of `owned` locals whose blocks are one value's: a local
//! moved into another, a buffer pushed into an array of buffers, a key moved
//! into a map. A class may be taken when one of its blocks is allocated here,
//! none comes from elsewhere (a parameter, a call's result), and none leaves
//! (moved into a call, a function value or a result). Its *home* is the
//! innermost loop turn, or the function's call, that holds every mention of
//! it: since a turn leaves the locals as it found them (the release
//! verifier), the whole value is dead at its home's end.
//!
//! Each home that is a loop's turn gets its room back at the end of each
//! turn, and after the loop; the call's, at each return. A statement that
//! allocates or grows a taken block runs with the arena on, `arena_flag(1)`
//! before it and `arena_flag(0)` after, unless that is unsafe:
//!
//! - it calls a function, or makes a function value: what the callee
//!   allocates is not this function's to place;
//! - it grows a block that is not taken;
//! - it is inside a loop, below the block's home, that gives its room back
//!   each turn: the room would be given back while the block lives.
//!
//! Such a statement runs with the arena off, and TLSF gives its blocks, as
//! today. Every release is kept: releasing a block of the arena does nothing,
//! so a block may come from either, and the IR, its verifier and its
//! interpreter are unchanged.

use std::collections::BTreeMap;

use crate::{
    BufOp, BufStmt, Const, Expr, ExprKind, Function, Local, LocalId, Module, RuntimeFn, Stmt, Ty,
};

/// A region: 0 is the function's call, then each loop's turn, in the order a
/// walk of the body meets the loops.
type Region = usize;

/// What one function's analysis finds.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Taken {
    /// For each home: its depth of loops (0 for the call) and its locals.
    pub regions: BTreeMap<Region, (usize, Vec<LocalId>)>,
    /// Assignments of a fresh block to a local.
    pub allocations: usize,
    /// Those that run with the arena on.
    pub taken: usize,
}

/// The analysis of every function of a module, by name.
#[must_use]
pub fn analyse(module: &Module) -> Vec<(String, Taken)> {
    module
        .functions
        .iter()
        .filter(|f| !f.job && f.uses_heap())
        .map(|f| {
            let plan = Plan::of(f, false);
            let mut taken = Taken {
                allocations: plan.facts.allocations,
                taken: plan.wrapped_allocations,
                ..Taken::default()
            };
            for (l, home) in plan.homes.iter().enumerate() {
                if let Some(r) = *home {
                    let depth = plan.facts.depth(r);
                    taken
                        .regions
                        .entry(r)
                        .or_insert((depth, Vec::new()))
                        .1
                        .push(local(l));
                }
            }
            (f.name.clone(), taken)
        })
        .collect()
}

/// Rewrites every function an arena can serve: marks, resets, and the
/// arena turned on around the statements it takes.
pub fn apply(module: &mut Module) {
    for f in module.functions.iter_mut().filter(|f| !f.job) {
        let plan = Plan::of(f, false);
        if !f.uses_heap() || plan.wrapped_allocations == 0 {
            continue;
        }
        plan.rewrite(f);
    }
}

/// Prototype a1: the blocks written `arena { … }` are the only homes; their
/// marks become the arena's mark and reset. Each value made in one that
/// outlives it, by the block's start and the local's name: an error.
pub fn apply_blocks(module: &mut Module) -> Vec<(u32, String)> {
    let mut escapes = Vec::new();
    for f in module.functions.iter_mut().filter(|f| !f.job) {
        let plan = Plan::of(f, true);
        for &(at, l) in &plan.escapes {
            let name = f
                .locals
                .get(l.0 as usize)
                .map_or_else(String::new, |x| x.name.clone());
            escapes.push((at, name));
        }
        if plan.facts.arenas.is_empty() {
            continue;
        }
        plan.rewrite(f);
    }
    escapes
}

fn local(l: usize) -> LocalId {
    LocalId(u32::try_from(l).unwrap_or(u32::MAX))
}

fn runtime(f: RuntimeFn, args: Vec<Expr>) -> Expr {
    Expr {
        kind: ExprKind::Buf(BufOp::Runtime(f), args),
        ty: f.result(),
    }
}

fn int(n: i64) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(n)),
        ty: Ty::I32,
    }
}

fn flag(on: i64) -> Stmt {
    Stmt::Expr(runtime(RuntimeFn::ArenaFlag, vec![int(on)]))
}

fn reset(mark: LocalId) -> Stmt {
    Stmt::Expr(runtime(
        RuntimeFn::ArenaReset,
        vec![Expr {
            kind: ExprKind::Local(mark),
            ty: Ty::I32,
        }],
    ))
}

/// What a walk of a function's body records.
#[derive(Default)]
struct Facts {
    /// The parent of each region; region 0 has none.
    parents: Vec<Option<Region>>,
    /// Union-find over locals.
    class: Vec<usize>,
    /// Per local: allocated here, filled from elsewhere, leaves.
    fresh: Vec<bool>,
    foreign: Vec<bool>,
    leaves: Vec<bool>,
    /// Per local, the regions it is mentioned in.
    mentions: Vec<Vec<Region>>,
    /// Per local, the regions a fresh block is assigned to it in.
    fresh_at: Vec<Vec<Region>>,
    /// The regions of `arena { … }` blocks (prototype a1), by their start.
    arenas: BTreeMap<Region, u32>,
    allocations: usize,
}

impl Facts {
    fn of(f: &Function) -> Self {
        let n = f.locals.len();
        let mut facts = Self {
            parents: vec![None],
            class: (0..n).collect(),
            fresh: vec![false; n],
            foreign: vec![false; n],
            leaves: vec![false; n],
            mentions: vec![Vec::new(); n],
            fresh_at: vec![Vec::new(); n],
            arenas: BTreeMap::new(),
            allocations: 0,
        };
        for p in 0..f.param_count as usize {
            if let Some(slot) = facts.foreign.get_mut(p) {
                *slot = true;
            }
        }
        facts.block(&f.body, 0);
        facts
    }

    fn find(&mut self, l: usize) -> usize {
        let mut r = l;
        while let Some(&p) = self.class.get(r)
            && p != r
        {
            r = p;
        }
        if let Some(slot) = self.class.get_mut(l) {
            *slot = r;
        }
        r
    }

    fn union(&mut self, a: LocalId, b: LocalId) {
        let (ra, rb) = (self.find(a.0 as usize), self.find(b.0 as usize));
        if let Some(slot) = self.class.get_mut(ra) {
            *slot = rb;
        }
    }

    fn mention(&mut self, l: LocalId, region: Region) {
        if let Some(m) = self.mentions.get_mut(l.0 as usize) {
            m.push(region);
        }
    }

    fn mark(v: &mut [bool], l: LocalId) {
        if let Some(slot) = v.get_mut(l.0 as usize) {
            *slot = true;
        }
    }

    fn block(&mut self, stmts: &[Stmt], region: Region) {
        let mut open: Vec<Region> = Vec::new();
        for s in stmts {
            let here = open.last().copied().unwrap_or(region);
            match marker(s) {
                Some((start, true)) => {
                    let r = self.parents.len();
                    self.parents.push(Some(here));
                    self.arenas.insert(r, start);
                    open.push(r);
                }
                Some((_, false)) => {
                    open.pop();
                }
                None => self.stmt(s, here),
            }
        }
    }

    fn stmt(&mut self, s: &Stmt, region: Region) {
        match s {
            Stmt::Set(l, e) => {
                self.mention(*l, region);
                match &e.kind {
                    ExprKind::Move(m) => self.union(*l, *m),
                    ExprKind::Buf(BufOp::Take, args) => {
                        if let Some(c) = args.first().and_then(root) {
                            self.union(*l, c);
                        }
                    }
                    ExprKind::Call(..)
                    | ExprKind::CallIndirect(..)
                    | ExprKind::Global(_)
                    | ExprKind::Env(..)
                    | ExprKind::Local(_)
                    | ExprKind::Borrow(_)
                        if e.ty == Ty::Owned =>
                    {
                        Self::mark(&mut self.foreign, *l);
                    }
                    _ if e.ty == Ty::Owned => {
                        self.allocations += 1;
                        Self::mark(&mut self.fresh, *l);
                        if let Some(at) = self.fresh_at.get_mut(l.0 as usize) {
                            at.push(region);
                        }
                    }
                    _ => {}
                }
                self.expr(e, region, Use::Value);
            }
            Stmt::SetGlobal(_, e) | Stmt::Expr(e) => self.expr(e, region, Use::Leaves),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond, region, Use::Value);
                self.block(then, region);
                self.block(otherwise, region);
            }
            Stmt::Loop { cond, body, step } => {
                let turn = self.parents.len();
                self.parents.push(Some(region));
                if let Some(c) = cond {
                    self.expr(c, turn, Use::Value);
                }
                self.block(body, turn);
                self.block(step, turn);
            }
            Stmt::Block(b) => self.block(b, region),
            Stmt::Leave(_) | Stmt::Break | Stmt::Continue => {}
            Stmt::Return(e) => {
                if let Some(e) = e {
                    self.expr(e, region, Use::Leaves);
                }
            }
            Stmt::ReturnMany(es) => {
                for e in es {
                    self.expr(e, region, Use::Leaves);
                }
            }
            Stmt::Release(l) => self.mention(*l, region),
            Stmt::Buf(op, args) => self.buf_stmt(*op, args, region),
            Stmt::CallMany { targets, args, .. } => {
                for t in targets {
                    self.mention(*t, region);
                    Self::mark(&mut self.foreign, *t);
                }
                for a in args {
                    self.expr(a, region, Use::Leaves);
                }
            }
            Stmt::CallManyIndirect {
                targets,
                callee,
                args,
            } => {
                for t in targets {
                    self.mention(*t, region);
                    Self::mark(&mut self.foreign, *t);
                }
                self.expr(callee, region, Use::Leaves);
                for a in args {
                    self.expr(a, region, Use::Leaves);
                }
            }
        }
    }

    fn buf_stmt(&mut self, op: BufStmt, args: &[Expr], region: Region) {
        if op == BufStmt::Release {
            for a in args {
                self.expr(a, region, Use::Value);
            }
            return;
        }
        let container = args.first().and_then(root);
        for a in args {
            if let (ExprKind::Move(m), Some(c)) = (&a.kind, container) {
                self.mention(*m, region);
                self.union(*m, c);
            } else {
                self.expr(a, region, Use::Value);
            }
        }
    }

    fn expr(&mut self, e: &Expr, region: Region, how: Use) {
        match &e.kind {
            ExprKind::Local(l) | ExprKind::Borrow(l) => self.mention(*l, region),
            ExprKind::Move(l) => {
                self.mention(*l, region);
                if how == Use::Leaves {
                    Self::mark(&mut self.leaves, *l);
                }
            }
            ExprKind::Const(_) | ExprKind::Global(_) => {}
            ExprKind::Unary(_, x)
            | ExprKind::Convert(x)
            | ExprKind::Math(_, x)
            | ExprKind::Alloc(x) => {
                self.expr(x, region, Use::Value);
            }
            ExprKind::Binary(_, a, b) => {
                self.expr(a, region, Use::Value);
                self.expr(b, region, Use::Value);
            }
            ExprKind::Call(_, args) | ExprKind::Env(_, args) => {
                for a in args {
                    self.expr(a, region, Use::Leaves);
                }
            }
            ExprKind::CallIndirect(callee, args) => {
                self.expr(callee, region, Use::Leaves);
                for a in args {
                    self.expr(a, region, Use::Leaves);
                }
            }
            ExprKind::Buf(_, args) => {
                // A runtime function that takes a block, a key moved into a
                // map, keeps it in its first argument.
                let container = args.first().and_then(root);
                for (i, a) in args.iter().enumerate() {
                    match (&a.kind, container) {
                        (ExprKind::Move(m), Some(c)) if i > 0 => {
                            self.mention(*m, region);
                            self.union(*m, c);
                        }
                        _ => self.expr(a, region, how),
                    }
                }
            }
        }
    }

    /// Whether `inner` is `outer` or inside it.
    fn within(&self, mut inner: Region, outer: Region) -> bool {
        loop {
            if inner == outer {
                return true;
            }
            match self.parents.get(inner).copied().flatten() {
                Some(p) => inner = p,
                None => return false,
            }
        }
    }

    fn depth(&self, mut r: Region) -> usize {
        let mut d = 0;
        while let Some(p) = self.parents.get(r).copied().flatten() {
            r = p;
            d += 1;
        }
        d
    }

    /// The innermost region holding both.
    fn meet(&self, a: Region, b: Region) -> Region {
        let mut r = a;
        while !self.within(b, r) {
            r = self.parents.get(r).copied().flatten().unwrap_or(0);
        }
        r
    }

    /// The home of each local an arena may take, `None` for the others.
    fn homes(&mut self) -> Vec<Option<Region>> {
        let n = self.class.len();
        let mut classes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for l in 0..n {
            let c = self.find(l);
            classes.entry(c).or_default().push(l);
        }
        let mut homes = vec![None; n];
        let flag = |v: &[bool], l: usize| v.get(l).copied().unwrap_or(true);
        for locals in classes.values() {
            if locals
                .iter()
                .any(|&l| flag(&self.foreign, l) || flag(&self.leaves, l))
                || !locals
                    .iter()
                    .any(|&l| self.fresh.get(l).copied().unwrap_or(false))
            {
                continue;
            }
            let mut r: Option<Region> = None;
            for &l in locals {
                for &m in self.mentions.get(l).map_or(&[][..], Vec::as_slice) {
                    r = Some(r.map_or(m, |x| self.meet(x, m)));
                }
            }
            for &l in locals {
                if let Some(slot) = homes.get_mut(l) {
                    *slot = r;
                }
            }
        }
        homes
    }
}

impl Facts {
    /// The innermost `arena { … }` region holding `r`.
    fn arena_of(&self, mut r: Region) -> Option<Region> {
        loop {
            if self.arenas.contains_key(&r) {
                return Some(r);
            }
            r = self.parents.get(r).copied().flatten()?;
        }
    }

    /// Prototype a1: the home of each local made in an `arena { … }` block,
    /// that block; and each value made in one that outlives it, by the
    /// block's start and one of its locals.
    fn homes_a1(&mut self) -> (Vec<Option<Region>>, Vec<(u32, LocalId)>) {
        let n = self.class.len();
        let mut classes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for l in 0..n {
            let c = self.find(l);
            classes.entry(c).or_default().push(l);
        }
        let mut homes = vec![None; n];
        let mut escapes = Vec::new();
        let flag = |v: &[bool], l: usize| v.get(l).copied().unwrap_or(true);
        for locals in classes.values() {
            let made: Vec<(usize, Region)> = locals
                .iter()
                .flat_map(|&l| {
                    self.fresh_at
                        .get(l)
                        .into_iter()
                        .flatten()
                        .map(move |&r| (l, r))
                })
                .collect();
            let Some(&(first, first_at)) = made.iter().find(|&&(_, r)| self.arena_of(r).is_some())
            else {
                continue;
            };
            let mut meet_made = first_at;
            for &(_, r) in &made {
                meet_made = self.meet(meet_made, r);
            }
            let mut all = meet_made;
            for &l in locals {
                for &m in self.mentions.get(l).map_or(&[][..], Vec::as_slice) {
                    all = self.meet(all, m);
                }
            }
            let block = self.arena_of(meet_made);
            let leaves = locals
                .iter()
                .any(|&l| flag(&self.foreign, l) || flag(&self.leaves, l));
            match block {
                Some(b) if !leaves && self.within(all, b) => {
                    for &l in locals {
                        if let Some(slot) = homes.get_mut(l) {
                            *slot = Some(b);
                        }
                    }
                }
                _ => {
                    let at = self
                        .arena_of(first_at)
                        .and_then(|b| self.arenas.get(&b))
                        .copied()
                        .unwrap_or(0);
                    escapes.push((at, local(first)));
                }
            }
        }
        (homes, escapes)
    }
}

/// A mark of an `arena { … }` block (prototype a1): its start, and whether
/// it opens the block.
fn marker(s: &Stmt) -> Option<(u32, bool)> {
    if let Stmt::Expr(Expr {
        kind: ExprKind::Buf(BufOp::Runtime(f), args),
        ..
    }) = s
        && let [
            Expr {
                kind: ExprKind::Const(Const::Int(at)),
                ..
            },
        ] = args.as_slice()
        && *at < 0
    {
        let start = u32::try_from(-1 - *at).ok()?;
        return match f {
            RuntimeFn::ArenaReset => Some((start, true)),
            RuntimeFn::ArenaFlag => Some((start, false)),
            _ => None,
        };
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    /// Read, or consumed where it is.
    Value,
    /// Given away: to a call, a result, a function value.
    Leaves,
}

/// The local a buffer expression reads from: `borrow l`, or an element of one.
fn root(e: &Expr) -> Option<LocalId> {
    match &e.kind {
        ExprKind::Borrow(l) | ExprKind::Local(l) => Some(*l),
        ExprKind::Buf(BufOp::Get(_), args) => args.first().and_then(root),
        _ => None,
    }
}

/// Whether a runtime function may change, and so grow, its first argument.
fn mutates(f: RuntimeFn) -> bool {
    use RuntimeFn as F;
    matches!(
        f,
        F::ArrReverse
            | F::ArrSort
            | F::ArrPermute
            | F::ArrKeep
            | F::FmtIntAppend
            | F::FmtUintAppend
            | F::MapPutI
            | F::MapPutS
            | F::MapPutSLent
            | F::MapPutAtI
            | F::MapPutAtS
            | F::MapPutAtSLent
            | F::MapStore
            | F::MapStoreF
            | F::MapStoreH
            | F::MapDeleteI
            | F::MapDeleteS
            | F::MapClear
            | F::SharedUnique
            | F::SharedUniqueSlot
            | F::SharedUniqueElem
    )
}

/// What one statement touches, for the choice of running it in the arena.
#[derive(Default)]
struct Touch {
    /// Locals it allocates into, or grows.
    blocks: Vec<LocalId>,
    /// It calls a function, makes a function value, or reads a module
    /// variable that holds a block.
    calls: bool,
}

impl Touch {
    fn stmt(s: &Stmt) -> Self {
        let mut t = Self::default();
        match s {
            Stmt::Set(l, e) => {
                if e.ty == Ty::Owned && !matches!(e.kind, ExprKind::Move(_) | ExprKind::Local(_)) {
                    t.blocks.push(*l);
                }
                t.expr(e);
            }
            Stmt::Expr(e) | Stmt::SetGlobal(_, e) => t.expr(e),
            Stmt::Buf(op, args) => {
                if matches!(op, BufStmt::Push(_) | BufStmt::Append(_))
                    && let Some(c) = args.first().and_then(root)
                {
                    t.blocks.push(c);
                }
                for a in args {
                    t.expr(a);
                }
            }
            Stmt::Release(_) => {}
            _ => t.calls = true,
        }
        t
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Call(..) | ExprKind::CallIndirect(..) | ExprKind::Env(..) => {
                self.calls = true;
            }
            ExprKind::Unary(_, x)
            | ExprKind::Convert(x)
            | ExprKind::Math(_, x)
            | ExprKind::Alloc(x) => self.expr(x),
            ExprKind::Binary(_, a, b) => {
                self.expr(a);
                self.expr(b);
            }
            ExprKind::Buf(op, args) => {
                if let BufOp::Runtime(f) = op
                    && mutates(*f)
                    && let Some(c) = args.first().and_then(root)
                {
                    self.blocks.push(c);
                }
                for a in args {
                    self.expr(a);
                }
            }
            ExprKind::Const(_)
            | ExprKind::Local(_)
            | ExprKind::Move(_)
            | ExprKind::Borrow(_)
            | ExprKind::Global(_) => {}
        }
    }
}

/// The choices for one function.
struct Plan {
    facts: Facts,
    homes: Vec<Option<Region>>,
    /// The homes that are a loop's turn, given their room back each turn.
    resetting: Vec<Region>,
    /// Prototype a1: values made in a block that outlive it.
    escapes: Vec<(u32, LocalId)>,
    wrapped_allocations: usize,
}

impl Plan {
    fn of(f: &Function, blocks: bool) -> Self {
        let mut facts = Facts::of(f);
        let (homes, escapes) = if blocks {
            facts.homes_a1()
        } else {
            (facts.homes(), Vec::new())
        };
        let mut resetting: Vec<Region> = homes
            .iter()
            .flatten()
            .copied()
            .filter(|&r| r != 0)
            .collect();
        resetting.sort_unstable();
        resetting.dedup();
        let mut plan = Self {
            facts,
            homes,
            resetting,
            escapes,
            wrapped_allocations: 0,
        };
        let mut counter = Counter::default();
        plan.count(&f.body, 0, &mut counter);
        plan.wrapped_allocations = counter.allocations;
        plan
    }

    fn count(&self, stmts: &[Stmt], region: Region, c: &mut Counter) {
        let mut open: Vec<Region> = Vec::new();
        for s in stmts {
            let region = open.last().copied().unwrap_or(region);
            match marker(s) {
                Some((_, true)) => {
                    c.loops += 1;
                    open.push(c.loops);
                    continue;
                }
                Some((_, false)) => {
                    open.pop();
                    continue;
                }
                None => {}
            }
            match s {
                Stmt::If {
                    then, otherwise, ..
                } => {
                    self.count(then, region, c);
                    self.count(otherwise, region, c);
                }
                Stmt::Loop { body, step, .. } => {
                    c.loops += 1;
                    let turn = c.loops;
                    self.count(body, turn, c);
                    self.count(step, turn, c);
                }
                Stmt::Block(b) => self.count(b, region, c),
                _ => {
                    if self.wraps(s, region) && matches!(s, Stmt::Set(..)) {
                        c.allocations += 1;
                    }
                }
            }
        }
    }

    /// Whether this statement, in this region, runs with the arena on.
    fn wraps(&self, s: &Stmt, at: Region) -> bool {
        let touch = Touch::stmt(s);
        if touch.blocks.is_empty() || touch.calls {
            return false;
        }
        touch.blocks.iter().all(|l| {
            self.homes
                .get(l.0 as usize)
                .copied()
                .flatten()
                .is_some_and(|home| {
                    !self.resetting.iter().any(|&reset| {
                        reset != home
                            && self.facts.within(reset, home)
                            && self.facts.within(at, reset)
                    })
                })
        })
    }

    fn rewrite(&self, f: &mut Function) {
        let add = |locals: &mut Vec<Local>, name: &str, ty: Ty| {
            locals.push(Local {
                name: name.to_owned(),
                ty,
            });
            local(locals.len() - 1)
        };
        let call = add(&mut f.locals, "arena", Ty::I32);
        let mut marks = BTreeMap::new();
        for &r in &self.resetting {
            marks.insert(r, add(&mut f.locals, "arena", Ty::I32));
        }
        let mut results = Vec::new();
        let ret = f.ret;
        if ret != Ty::Void {
            results.push(add(&mut f.locals, "result", ret));
        }
        for &ty in &f.more.clone() {
            results.push(add(&mut f.locals, "result", ty));
        }
        let mut w = Writer {
            plan: self,
            marks,
            call,
            results,
            loops: 0,
        };
        let body = std::mem::take(&mut f.body);
        let mut out = vec![Stmt::Set(call, runtime(RuntimeFn::ArenaMark, vec![int(0)]))];
        out.extend(w.block(body, 0));
        if ret == Ty::Void && !matches!(out.last(), Some(Stmt::Return(_))) {
            out.push(reset(call));
        }
        f.body = out;
    }
}

#[derive(Default)]
struct Counter {
    loops: usize,
    allocations: usize,
}

struct Writer<'a> {
    plan: &'a Plan,
    marks: BTreeMap<Region, LocalId>,
    call: LocalId,
    results: Vec<LocalId>,
    loops: usize,
}

impl Writer<'_> {
    fn block(&mut self, stmts: Vec<Stmt>, region: Region) -> Vec<Stmt> {
        let mut out = Vec::with_capacity(stmts.len());
        let mut open: Vec<Region> = Vec::new();
        for s in stmts {
            let here = open.last().copied().unwrap_or(region);
            match marker(&s) {
                Some((_, true)) => {
                    self.loops += 1;
                    let r = self.loops;
                    open.push(r);
                    if let Some(&mark) = self.marks.get(&r) {
                        out.push(Stmt::Set(mark, runtime(RuntimeFn::ArenaMark, vec![int(0)])));
                    }
                }
                Some((_, false)) => {
                    if let Some(mark) = open.pop().and_then(|r| self.marks.get(&r).copied()) {
                        out.push(reset(mark));
                    }
                }
                None => self.stmt(s, here, &mut out),
            }
        }
        out
    }

    fn take(l: LocalId, ty: Ty) -> Expr {
        let kind = if ty == Ty::Owned {
            ExprKind::Move(l)
        } else {
            ExprKind::Local(l)
        };
        Expr { kind, ty }
    }

    fn stmt(&mut self, s: Stmt, region: Region, out: &mut Vec<Stmt>) {
        match s {
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                let then = self.block(then, region);
                let otherwise = self.block(otherwise, region);
                out.push(Stmt::If {
                    cond,
                    then,
                    otherwise,
                });
            }
            Stmt::Loop { cond, body, step } => {
                self.loops += 1;
                let turn = self.loops;
                let body = self.block(body, turn);
                let mut step = self.block(step, turn);
                match self.marks.get(&turn).copied() {
                    Some(mark) => {
                        out.push(Stmt::Set(mark, runtime(RuntimeFn::ArenaMark, vec![int(0)])));
                        step.push(reset(mark));
                        out.push(Stmt::Loop { cond, body, step });
                        out.push(reset(mark));
                    }
                    None => out.push(Stmt::Loop { cond, body, step }),
                }
            }
            Stmt::Block(b) => {
                let b = self.block(b, region);
                out.push(Stmt::Block(b));
            }
            Stmt::Return(None) => {
                out.push(reset(self.call));
                out.push(Stmt::Return(None));
            }
            Stmt::Return(Some(e)) => {
                let Some(&tmp) = self.results.first() else {
                    out.push(Stmt::Return(Some(e)));
                    return;
                };
                let ty = e.ty;
                out.push(Stmt::Set(tmp, e));
                out.push(reset(self.call));
                out.push(Stmt::Return(Some(Self::take(tmp, ty))));
            }
            Stmt::ReturnMany(es) => {
                let mut values = Vec::with_capacity(es.len());
                for (e, &tmp) in es.into_iter().zip(self.results.clone().iter()) {
                    let ty = e.ty;
                    out.push(Stmt::Set(tmp, e));
                    values.push(Self::take(tmp, ty));
                }
                out.push(reset(self.call));
                out.push(Stmt::ReturnMany(values));
            }
            other => {
                if self.plan.wraps(&other, region) {
                    out.push(flag(1));
                    out.push(other);
                    out.push(flag(0));
                } else {
                    out.push(other);
                }
            }
        }
    }
}

/// Prototype a1b: [`apply_blocks`], with the arena on for each block written
/// instead of around each statement it takes: turned off only around a
/// statement or a condition it cannot take, and before every jump out.
/// Only the functions that hold a block are analysed.
pub fn apply_blocks_on(module: &mut Module) -> Vec<(u32, String)> {
    let mut escapes = Vec::new();
    for f in module.functions.iter_mut().filter(|f| !f.job) {
        if !holds_marker(&f.body) {
            continue;
        }
        let plan = Plan::of(f, true);
        for &(at, l) in &plan.escapes {
            let name = f
                .locals
                .get(l.0 as usize)
                .map_or_else(String::new, |x| x.name.clone());
            escapes.push((at, name));
        }
        plan.rewrite_on(f);
    }
    escapes
}

/// Whether a body holds the mark of an `arena { … }` block.
fn holds_marker(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::If {
            then, otherwise, ..
        } => holds_marker(then) || holds_marker(otherwise),
        Stmt::Loop { body, step, .. } => holds_marker(body) || holds_marker(step),
        Stmt::Block(b) => holds_marker(b),
        other => marker(other).is_some(),
    })
}

impl Plan {
    /// Whether a statement may run with the arena on, in this region: it
    /// calls nothing, and every block it allocates or grows is taken, with
    /// no arena below its home giving its room back around it.
    fn safe(&self, s: &Stmt, at: Region) -> bool {
        let touch = Touch::stmt(s);
        !touch.calls && self.blocks_safe(&touch, at)
    }

    fn safe_expr(&self, e: &Expr, at: Region) -> bool {
        let mut touch = Touch::default();
        touch.expr(e);
        !touch.calls && self.blocks_safe(&touch, at)
    }

    fn blocks_safe(&self, touch: &Touch, at: Region) -> bool {
        touch.blocks.iter().all(|l| {
            self.homes
                .get(l.0 as usize)
                .copied()
                .flatten()
                .is_some_and(|home| {
                    !self.resetting.iter().any(|&reset| {
                        reset != home
                            && self.facts.within(reset, home)
                            && self.facts.within(at, reset)
                    })
                })
        })
    }

    fn rewrite_on(&self, f: &mut Function) {
        let add = |locals: &mut Vec<Local>, name: &str, ty: Ty| {
            locals.push(Local {
                name: name.to_owned(),
                ty,
            });
            local(locals.len() - 1)
        };
        let call = add(&mut f.locals, "arena", Ty::I32);
        let mut marks = BTreeMap::new();
        for &r in &self.resetting {
            marks.insert(r, add(&mut f.locals, "arena", Ty::I32));
        }
        let mut results = Vec::new();
        let ret = f.ret;
        if ret != Ty::Void {
            results.push(add(&mut f.locals, "result", ret));
        }
        for &ty in &f.more.clone() {
            results.push(add(&mut f.locals, "result", ty));
        }
        let mut w = OnWriter {
            plan: self,
            marks,
            call,
            results,
            loops: 0,
            depth: 0,
            on: false,
        };
        let body = std::mem::take(&mut f.body);
        let mut out = vec![Stmt::Set(call, runtime(RuntimeFn::ArenaMark, vec![int(0)]))];
        out.extend(w.block(body, 0));
        if ret == Ty::Void && !matches!(out.last(), Some(Stmt::Return(_))) {
            w.set(false, &mut out);
            out.push(reset(call));
        }
        f.body = out;
    }
}

/// The writer of a1b: it knows whether the arena is on where it writes.
/// Where two paths meet, each leaves it as it was where they parted; a jump
/// turns it off, which is always safe: a statement run with the arena off
/// takes its blocks from TLSF.
struct OnWriter<'a> {
    plan: &'a Plan,
    marks: BTreeMap<Region, LocalId>,
    call: LocalId,
    results: Vec<LocalId>,
    loops: usize,
    /// How many blocks written hold the place written.
    depth: usize,
    /// Whether the arena is on there.
    on: bool,
}

impl OnWriter<'_> {
    fn set(&mut self, on: bool, out: &mut Vec<Stmt>) {
        if self.on != on {
            out.push(flag(i64::from(on)));
            self.on = on;
        }
    }

    fn block(&mut self, stmts: Vec<Stmt>, region: Region) -> Vec<Stmt> {
        let mut out = Vec::with_capacity(stmts.len());
        let mut open: Vec<Region> = Vec::new();
        for s in stmts {
            let here = open.last().copied().unwrap_or(region);
            match marker(&s) {
                Some((_, true)) => {
                    self.loops += 1;
                    let r = self.loops;
                    open.push(r);
                    self.depth += 1;
                    if let Some(&mark) = self.marks.get(&r) {
                        out.push(Stmt::Set(mark, runtime(RuntimeFn::ArenaMark, vec![int(0)])));
                    }
                }
                Some((_, false)) => {
                    self.depth = self.depth.saturating_sub(1);
                    if self.depth == 0 {
                        self.set(false, &mut out);
                    }
                    if let Some(mark) = open.pop().and_then(|r| self.marks.get(&r).copied()) {
                        out.push(reset(mark));
                    }
                }
                None => self.stmt(s, here, &mut out),
            }
        }
        out
    }

    fn stmt(&mut self, s: Stmt, region: Region, out: &mut Vec<Stmt>) {
        let inside = self.depth > 0;
        match s {
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                let wanted = inside && self.plan.safe_expr(&cond, region);
                self.set(wanted, out);
                let then = self.branch(then, region, wanted);
                let otherwise = self.branch(otherwise, region, wanted);
                out.push(Stmt::If {
                    cond,
                    then,
                    otherwise,
                });
            }
            Stmt::Loop { cond, body, step } => {
                self.loops += 1;
                let turn = self.loops;
                let wanted = inside && cond.as_ref().is_none_or(|c| self.plan.safe_expr(c, turn));
                self.set(wanted, out);
                let body = self.block(body, turn);
                let mut step = self.block(step, turn);
                if self.on != wanted {
                    step.push(flag(i64::from(wanted)));
                }
                self.on = wanted;
                out.push(Stmt::Loop { cond, body, step });
            }
            Stmt::Block(b) => {
                let b = self.block(b, region);
                out.push(Stmt::Block(b));
            }
            Stmt::Break | Stmt::Continue | Stmt::Leave(_) => {
                let was = self.on;
                self.set(false, out);
                out.push(s);
                self.on = was;
            }
            Stmt::Return(None) => {
                self.set(false, out);
                out.push(reset(self.call));
                out.push(Stmt::Return(None));
            }
            Stmt::Return(Some(e)) => {
                // A result is computed with the arena off: it leaves.
                self.set(false, out);
                let Some(&tmp) = self.results.first() else {
                    out.push(Stmt::Return(Some(e)));
                    return;
                };
                let ty = e.ty;
                out.push(Stmt::Set(tmp, e));
                out.push(reset(self.call));
                out.push(Stmt::Return(Some(Writer::take(tmp, ty))));
            }
            Stmt::ReturnMany(es) => {
                self.set(false, out);
                let mut values = Vec::with_capacity(es.len());
                for (e, &tmp) in es.into_iter().zip(self.results.clone().iter()) {
                    let ty = e.ty;
                    out.push(Stmt::Set(tmp, e));
                    values.push(Writer::take(tmp, ty));
                }
                out.push(reset(self.call));
                out.push(Stmt::ReturnMany(values));
            }
            other => {
                let wanted = inside && self.plan.safe(&other, region);
                self.set(wanted, out);
                out.push(other);
            }
        }
    }

    /// A branch, which leaves the arena as it found it.
    fn branch(&mut self, stmts: Vec<Stmt>, region: Region, at_start: bool) -> Vec<Stmt> {
        self.on = at_start;
        let mut out = self.block(stmts, region);
        self.set(at_start, &mut out);
        out
    }
}
