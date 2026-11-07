//! The release verifier, on IR written by hand: what it accepts runs without a
//! double release, a use after release or a leak in the interpreter, and each
//! kind of mistake is refused with its reason.

use crate::interp::{HeapReport, Trap, Value, run_with_heap};
use crate::{
    BinOp, Const, Expr, ExprKind, FnId, Function, Local, LocalId, Module, Stmt, Ty, verify,
};

pub fn int(v: i64) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(v)),
        ty: Ty::I32,
    }
}

pub fn get(l: u32) -> Expr {
    Expr {
        kind: ExprKind::Local(LocalId(l)),
        ty: Ty::I32,
    }
}

pub fn alloc(bytes: i64) -> Expr {
    Expr {
        kind: ExprKind::Alloc(Box::new(int(bytes))),
        ty: Ty::Owned,
    }
}

pub fn moved(l: u32) -> Expr {
    Expr {
        kind: ExprKind::Move(LocalId(l)),
        ty: Ty::Owned,
    }
}

pub fn borrowed(l: u32) -> Expr {
    Expr {
        kind: ExprKind::Borrow(LocalId(l)),
        ty: Ty::Borrowed,
    }
}

pub fn bin(op: BinOp, a: Expr, b: Expr, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Binary(op, Box::new(a), Box::new(b)),
        ty,
    }
}

pub fn call(f: u32, args: Vec<Expr>, ty: Ty) -> Expr {
    Expr {
        kind: ExprKind::Call(FnId(f), args),
        ty,
    }
}

/// `(n >> bit) & 1 == 1`: a condition that differs from one input to another.
pub fn bit(bit: i64) -> Expr {
    let shifted = bin(BinOp::Shr, get(0), int(bit), Ty::I32);
    bin(
        BinOp::Eq,
        bin(BinOp::BitAnd, shifted, int(1), Ty::I32),
        int(1),
        Ty::Bool,
    )
}

pub fn local(name: &str, ty: Ty) -> Local {
    Local {
        name: name.to_owned(),
        ty,
    }
}

/// Function 0 is `f(n: i32): ret` with the given extra locals and body; the
/// helpers follow:
///
/// - 1, `consume(x: owned): void`, releases `x`;
/// - 2, `peek(x: borrowed): i32`, returns 7;
/// - 3, `make(): owned`, returns a new block;
/// - 4, `relay(x: owned): owned`, returns `x`;
/// - 5, `spend(x: owned): bool`, releases `x` and returns `true`.
pub fn module(ret: Ty, locals: Vec<Local>, body: Vec<Stmt>) -> Module {
    let mut all = vec![local("n", Ty::I32)];
    all.extend(locals);
    let f = Function {
        name: "f".to_owned(),
        exported: true,
        param_count: 1,
        locals: all,
        ret,
        more: Vec::new(),
        shape: None,
        body,
    };
    let helper = |name: &str, param: Option<Ty>, ret: Ty, body: Vec<Stmt>| Function {
        name: name.to_owned(),
        exported: false,
        param_count: u32::from(param.is_some()),
        locals: param.map(|ty| local("x", ty)).into_iter().collect(),
        ret,
        more: Vec::new(),
        shape: None,
        body,
    };
    Module {
        functions: vec![
            f,
            helper(
                "consume",
                Some(Ty::Owned),
                Ty::Void,
                vec![Stmt::Release(LocalId(0))],
            ),
            helper(
                "peek",
                Some(Ty::Borrowed),
                Ty::I32,
                vec![Stmt::Return(Some(int(7)))],
            ),
            helper("make", None, Ty::Owned, vec![Stmt::Return(Some(alloc(8)))]),
            helper(
                "relay",
                Some(Ty::Owned),
                Ty::Owned,
                vec![Stmt::Return(Some(moved(0)))],
            ),
            helper(
                "spend",
                Some(Ty::Owned),
                Ty::Bool,
                vec![
                    Stmt::Release(LocalId(0)),
                    Stmt::Return(Some(Expr {
                        kind: ExprKind::Const(Const::Bool(true)),
                        ty: Ty::Bool,
                    })),
                ],
            ),
        ],
        data: Vec::new(),
    }
}

fn owned(n: usize) -> Vec<Local> {
    (0..n).map(|i| local(&format!("o{i}"), Ty::Owned)).collect()
}

/// Accepted, and every input runs without a memory bug or a leak.
fn accepted(m: &Module) {
    assert_eq!(verify(m), Ok(()), "\n{m}");
    for n in 0..16 {
        let outcome = run_with_heap(m, FnId(0), &[Value::I32(n)]);
        let Ok((value, HeapReport { live, .. })) = outcome else {
            unreachable!("n = {n}: {outcome:?}\n{m}");
        };
        match value {
            // The block returned to the caller is the only one left.
            Value::Handle(block) => assert_eq!(live, [block], "n = {n}\n{m}"),
            _ => assert_eq!(live, Vec::<u32>::new(), "n = {n}\n{m}"),
        }
    }
}

/// Refused, with a reason that contains `reason`.
fn refused(m: &Module, reason: &str) {
    let problems = verify(m).err().unwrap_or_default();
    assert!(
        problems.iter().any(|p| p.contains(reason)),
        "expected “{reason}”, got {problems:?}\n{m}"
    );
}

#[test]
fn every_path_that_releases_exactly_once_is_accepted() {
    let set = |l: u32, e: Expr| Stmt::Set(LocalId(l), e);
    let consume = |l: u32| Stmt::Expr(call(1, vec![moved(l)], Ty::Void));
    // Allocate, lend, give away.
    accepted(&module(
        Ty::Void,
        owned(1),
        vec![
            set(1, alloc(16)),
            Stmt::Expr(call(2, vec![borrowed(1)], Ty::I32)),
            consume(1),
        ],
    ));
    // Each branch releases its own way; an early return releases first.
    accepted(&module(
        Ty::Void,
        owned(2),
        vec![
            set(1, call(3, vec![], Ty::Owned)),
            Stmt::If {
                cond: bit(0),
                then: vec![Stmt::Release(LocalId(1)), Stmt::Return(None)],
                otherwise: vec![set(2, call(4, vec![moved(1)], Ty::Owned))],
            },
            Stmt::Release(LocalId(2)),
        ],
    ));
    // A loop that allocates and releases on every turn, and leaves by `break`.
    let mut locals = owned(1);
    locals.push(local("i", Ty::I32));
    accepted(&module(
        Ty::Void,
        locals,
        vec![
            set(2, int(0)),
            Stmt::Loop {
                cond: Some(bin(BinOp::Lt, get(2), get(0), Ty::Bool)),
                body: vec![
                    set(1, alloc(4)),
                    Stmt::If {
                        cond: bin(BinOp::Eq, get(2), int(9), Ty::Bool),
                        then: vec![Stmt::Release(LocalId(1)), Stmt::Break],
                        otherwise: vec![],
                    },
                    consume(1),
                ],
                step: vec![set(2, bin(BinOp::Add, get(2), int(1), Ty::I32))],
            },
        ],
    ));
    // A block held across a whole loop, a block left early, a result moved out.
    accepted(&module(
        Ty::Owned,
        owned(2),
        vec![
            set(1, alloc(32)),
            Stmt::Block(vec![
                set(2, alloc(1)),
                Stmt::If {
                    cond: bit(1),
                    then: vec![Stmt::Release(LocalId(2)), Stmt::Leave(0)],
                    otherwise: vec![],
                },
                Stmt::Release(LocalId(2)),
            ]),
            Stmt::Return(Some(moved(1))),
        ],
    ));
}

#[test]
fn each_kind_of_mistake_is_refused_with_its_reason() {
    let set = |l: u32, e: Expr| Stmt::Set(LocalId(l), e);
    let release = |l: u32| Stmt::Release(LocalId(l));
    for (body, reason) in [
        (vec![set(1, alloc(8))], "still holds a block at the end"),
        (
            vec![set(1, alloc(8)), Stmt::Return(None)],
            "still holds a block when the function returns",
        ),
        (
            vec![set(1, alloc(8)), release(1), release(1)],
            "released but holds no block",
        ),
        (vec![release(1)], "released but holds no block"),
        (
            vec![
                set(1, alloc(8)),
                Stmt::Expr(call(1, vec![moved(1)], Ty::Void)),
                Stmt::Expr(call(2, vec![borrowed(1)], Ty::I32)),
            ],
            "used after its block was released or moved",
        ),
        (
            vec![set(1, alloc(8)), set(1, alloc(8)), release(1)],
            "overwritten while it holds a block",
        ),
        (vec![Stmt::Expr(alloc(8))], "dropped at once"),
        (
            vec![
                set(1, alloc(8)),
                Stmt::If {
                    cond: bit(0),
                    then: vec![release(1)],
                    otherwise: vec![],
                },
            ],
            "holds a block on one path and not on another, after an if",
        ),
        (
            vec![Stmt::Loop {
                cond: Some(bit(0)),
                body: vec![set(1, alloc(8)), Stmt::Break],
                step: vec![],
            }],
            "holds a block on one path and not on another, after a loop",
        ),
        (
            vec![
                set(1, alloc(8)),
                Stmt::Loop {
                    cond: Some(bit(0)),
                    body: vec![release(1)],
                    step: vec![],
                },
                release(1),
            ],
            "a turn of a loop empties %1",
        ),
        (
            // `spend` runs only when the bit is clear: who holds the block then?
            vec![
                set(1, alloc(8)),
                Stmt::If {
                    cond: bin(
                        BinOp::Or,
                        bit(0),
                        call(5, vec![moved(1)], Ty::Bool),
                        Ty::Bool,
                    ),
                    then: vec![],
                    otherwise: vec![],
                },
            ],
            "moved on one side of `&&` or `||` only",
        ),
        (
            // Two blocks are never compared: a handle is not a value.
            vec![
                set(1, alloc(8)),
                set(2, alloc(8)),
                Stmt::If {
                    cond: bin(
                        BinOp::Eq,
                        call(4, vec![moved(1)], Ty::Owned),
                        call(4, vec![moved(2)], Ty::Owned),
                        Ty::Bool,
                    ),
                    then: vec![],
                    otherwise: vec![],
                },
            ],
            "== on owned and owned",
        ),
    ] {
        refused(&module(Ty::Void, owned(2), body), reason);
    }
}

#[test]
fn handles_appear_only_where_ownership_is_clear() {
    let set = |l: u32, e: Expr| Stmt::Set(LocalId(l), e);
    // Only a parameter may borrow; nothing returns a borrow.
    refused(
        &module(Ty::Void, vec![local("b", Ty::Borrowed)], vec![]),
        "only a parameter can borrow",
    );
    refused(
        &module(Ty::Borrowed, vec![], vec![]),
        "returns a borrowed block",
    );
    // An owned local is moved or borrowed, never read as is: that would copy it.
    let mut locals = owned(2);
    locals.push(local("i", Ty::I32));
    for (body, reason) in [
        (
            vec![
                set(1, alloc(8)),
                set(
                    2,
                    Expr {
                        kind: ExprKind::Local(LocalId(1)),
                        ty: Ty::Owned,
                    },
                ),
            ],
            "an owned value must be allocated, moved or returned by a call",
        ),
        (
            vec![set(3, call(2, vec![moved(1)], Ty::I32))],
            "call to peek does not match its signature",
        ),
        (
            vec![set(
                3,
                call(
                    2,
                    vec![Expr {
                        kind: ExprKind::Move(LocalId(1)),
                        ty: Ty::Borrowed,
                    }],
                    Ty::I32,
                ),
            )],
            "move of %1 typed borrowed",
        ),
        (
            vec![Stmt::Release(LocalId(3))],
            "release of %3, which is not owned",
        ),
        (
            vec![
                set(1, alloc(8)),
                set(3, bin(BinOp::Add, get(3), int(1), Ty::I32)),
                Stmt::Expr(moved(1)),
            ],
            "where only an assignment, a return or an argument can take it",
        ),
    ] {
        refused(&module(Ty::Void, locals.clone(), body), reason);
    }
}

#[test]
fn the_interpreter_catches_what_the_verifier_refuses() {
    // The verifier is the guard; the interpreter shows what it guards against.
    let set = |l: u32, e: Expr| Stmt::Set(LocalId(l), e);
    let twice = module(
        Ty::Void,
        owned(1),
        vec![
            set(1, alloc(8)),
            Stmt::Release(LocalId(1)),
            Stmt::Release(LocalId(1)),
        ],
    );
    assert_eq!(
        run_with_heap(&twice, FnId(0), &[Value::I32(0)]).err(),
        Some(Trap::DoubleRelease)
    );
    let leak = module(Ty::Void, owned(1), vec![set(1, alloc(8))]);
    assert_eq!(
        run_with_heap(&leak, FnId(0), &[Value::I32(0)]).map(|(_, heap)| heap.live),
        Ok(vec![1])
    );
}

/// Owned locals of the generated functions: %1 to %3. Counters follow.
const OWNED: usize = 3;

/// A small deterministic generator, so that a failure can be replayed.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 33) % n
    }
}

/// Writes random valid code: it tracks which locals hold a block, and before
/// every exit (end of a branch, of a turn, `break`, `continue`, `return`) it
/// releases or refills what must be.
struct Generator {
    rng: Rng,
    /// Counters already used by loops: locals after the owned ones.
    counters: u32,
}

/// Where control goes after a list of statements: `None` if it never falls through.
type Full = Option<[bool; OWNED]>;

impl Generator {
    fn owned(index: usize) -> u32 {
        u32::try_from(index + 1).unwrap_or(0)
    }

    /// Statements that bring the locals from `now` to `wanted`.
    fn reconcile(now: [bool; OWNED], wanted: [bool; OWNED]) -> Vec<Stmt> {
        let mut out = Vec::new();
        for i in 0..OWNED {
            match (now[i], wanted[i]) {
                (true, false) => out.push(Stmt::Release(LocalId(Self::owned(i)))),
                (false, true) => out.push(Stmt::Set(LocalId(Self::owned(i)), alloc(4))),
                _ => {}
            }
        }
        out
    }

    /// A list of statements from `full`; `head` is the state its loop expects at
    /// every exit, if inside one.
    fn block(
        &mut self,
        depth: u32,
        mut full: [bool; OWNED],
        head: Option<[bool; OWNED]>,
    ) -> (Vec<Stmt>, Full) {
        let mut out = Vec::new();
        for _ in 0..=self.rng.below(5) {
            let i = usize::try_from(self.rng.below(OWNED as u64)).unwrap_or(0);
            let l = Self::owned(i);
            match self.rng.below(12) {
                0 | 1 if !full[i] => {
                    let value = match self.rng.below(3) {
                        0 => alloc(16),
                        1 => call(3, vec![], Ty::Owned),
                        _ => match (0..OWNED).find(|&j| full[j]) {
                            Some(j) => {
                                full[j] = false;
                                Expr {
                                    kind: ExprKind::Move(LocalId(Self::owned(j))),
                                    ty: Ty::Owned,
                                }
                            }
                            None => alloc(1),
                        },
                    };
                    out.push(Stmt::Set(LocalId(l), value));
                    full[i] = true;
                }
                2 if full[i] => {
                    out.push(Stmt::Release(LocalId(l)));
                    full[i] = false;
                }
                3 if full[i] => {
                    out.push(Stmt::Expr(call(1, vec![moved(l)], Ty::Void)));
                    full[i] = false;
                }
                4 if full[i] => out.push(Stmt::Expr(call(2, vec![borrowed(l)], Ty::I32))),
                5 if full[i] => out.push(Stmt::Set(LocalId(l), call(4, vec![moved(l)], Ty::Owned))),
                6 | 7 if depth < 3 => {
                    let cond = bit(i64::try_from(self.rng.below(4)).unwrap_or(0));
                    let (mut then, a) = self.block(depth + 1, full, head);
                    let (mut otherwise, b) = self.block(depth + 1, full, head);
                    // Where the branches disagree, the one holding the block releases it.
                    let joined = match (a, b) {
                        (Some(a), Some(b)) => {
                            let both = std::array::from_fn(|k| a[k] && b[k]);
                            then.extend(Self::reconcile(a, both));
                            otherwise.extend(Self::reconcile(b, both));
                            Some(both)
                        }
                        (Some(a), None) => Some(a),
                        (None, Some(b)) => Some(b),
                        (None, None) => None,
                    };
                    out.push(Stmt::If {
                        cond,
                        then,
                        otherwise,
                    });
                    match joined {
                        Some(next) => full = next,
                        None => return (out, None),
                    }
                }
                8 if depth < 3 => {
                    self.counters += 1;
                    let counter = u32::try_from(OWNED).unwrap_or(0) + self.counters;
                    let bound = bin(BinOp::BitAnd, get(0), int(3), Ty::I32);
                    let (mut body, end) = self.block(depth + 1, full, Some(full));
                    if let Some(end) = end {
                        body.extend(Self::reconcile(end, full));
                    }
                    out.push(Stmt::Set(LocalId(counter), int(0)));
                    out.push(Stmt::Loop {
                        cond: Some(bin(BinOp::Lt, get(counter), bound, Ty::Bool)),
                        body,
                        step: vec![Stmt::Set(
                            LocalId(counter),
                            bin(BinOp::Add, get(counter), int(1), Ty::I32),
                        )],
                    });
                }
                9 if head.is_some() => {
                    out.extend(Self::reconcile(full, head.unwrap_or(full)));
                    out.push(if self.rng.below(2) == 0 {
                        Stmt::Break
                    } else {
                        Stmt::Continue
                    });
                    return (out, None);
                }
                10 if depth > 0 => {
                    out.extend(Self::reconcile(full, [false; OWNED]));
                    out.push(Stmt::Return(None));
                    return (out, None);
                }
                _ => {}
            }
        }
        (out, Some(full))
    }

    fn function(&mut self) -> Module {
        self.counters = 0;
        let (mut body, end) = self.block(0, [false; OWNED], None);
        if let Some(end) = end {
            body.extend(Self::reconcile(end, [false; OWNED]));
        }
        let mut locals = owned(OWNED);
        for c in 0..self.counters {
            locals.push(local(&format!("i{c}"), Ty::I32));
        }
        module(Ty::Void, locals, body)
    }
}

/// Every `release` of function 0, as paths of statement indices.
fn releases(stmts: &[Stmt], path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    for (i, stmt) in stmts.iter().enumerate() {
        path.push(i);
        match stmt {
            Stmt::Release(_) => out.push(path.clone()),
            Stmt::If {
                then, otherwise, ..
            } => {
                path.push(0);
                releases(then, path, out);
                path.pop();
                path.push(1);
                releases(otherwise, path, out);
                path.pop();
            }
            Stmt::Loop { body, .. } => releases(body, path, out),
            _ => {}
        }
        path.pop();
    }
}

/// The statement list that holds the statement at `path`, and its index there.
fn list_at<'a>(stmts: &'a mut Vec<Stmt>, path: &[usize]) -> Option<(&'a mut Vec<Stmt>, usize)> {
    let (&index, rest) = path.split_first()?;
    if rest.is_empty() {
        return Some((stmts, index));
    }
    match stmts.get_mut(index)? {
        Stmt::If {
            then, otherwise, ..
        } => {
            let (&side, rest) = rest.split_first()?;
            list_at(if side == 0 { then } else { otherwise }, rest)
        }
        Stmt::Loop { body, .. } => list_at(body, rest),
        _ => None,
    }
}

/// Random programs that release exactly once run cleanly, and every one with a
/// `release` removed or doubled is refused.
#[test]
fn random_programs_and_their_mutants() {
    let mut generator = Generator {
        rng: Rng(0x2545_F491_4F6C_DD1D),
        counters: 0,
    };
    let (mut programs, mut mutants) = (0, 0);
    for _ in 0..1_000 {
        let m = generator.function();
        accepted(&m);
        programs += 1;
        let mut paths = Vec::new();
        releases(&m.functions[0].body, &mut Vec::new(), &mut paths);
        for path in paths {
            for double in [false, true] {
                let mut mutant = m.clone();
                if let Some((list, index)) = list_at(&mut mutant.functions[0].body, &path) {
                    if double {
                        let copy = list[index].clone();
                        list.insert(index, copy);
                    } else {
                        list.remove(index);
                    }
                }
                assert!(verify(&mutant).is_err(), "mutant accepted:\n{mutant}");
                mutants += 1;
            }
        }
    }
    assert!(
        programs == 1_000 && mutants > 1_000,
        "{programs} programs, {mutants} mutants"
    );
}
