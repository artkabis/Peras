//! Modules that use the heap, written in IR by hand: no Peras construct
//! allocates yet. Shared by `tests/heap.rs` and `examples/heap_fixture.rs`.

use perasc_ir::{BinOp, Const, Expr, ExprKind, FnId, Function, Local, LocalId, Module, Stmt, Ty};

fn int(v: i64) -> Expr {
    Expr {
        kind: ExprKind::Const(Const::Int(v)),
        ty: Ty::I32,
    }
}

fn get(l: u32) -> Expr {
    Expr {
        kind: ExprKind::Local(LocalId(l)),
        ty: Ty::I32,
    }
}

fn set(l: u32, value: Expr) -> Stmt {
    Stmt::Set(LocalId(l), value)
}

fn alloc(bytes: Expr) -> Expr {
    Expr {
        kind: ExprKind::Alloc(Box::new(bytes)),
        ty: Ty::Owned,
    }
}

fn local(name: &str, ty: Ty) -> Local {
    Local {
        name: name.to_owned(),
        ty,
    }
}

/// `work(n: i32): i32`: allocates and releases a block `n` times, lending each
/// to `peek(x: borrowed): i32`, and returns `n`. Releases exactly once.
pub fn work() -> Module {
    let peek = Function {
        name: "peek".to_owned(),
        exported: false,
        job: false,
        param_count: 1,
        locals: vec![local("x", Ty::Borrowed)],
        ret: Ty::I32,
        more: Vec::new(),
        shape: None,
        body: vec![Stmt::Return(Some(int(7)))],
    };
    let work = Function {
        name: "work".to_owned(),
        exported: true,
        job: false,
        param_count: 1,
        locals: vec![
            local("n", Ty::I32),
            local("block", Ty::Owned),
            local("i", Ty::I32),
        ],
        ret: Ty::I32,
        more: Vec::new(),
        shape: None,
        body: vec![
            set(2, int(0)),
            Stmt::Loop {
                cond: Some(Expr {
                    kind: ExprKind::Binary(BinOp::Lt, Box::new(get(2)), Box::new(get(0))),
                    ty: Ty::Bool,
                }),
                body: vec![
                    set(1, alloc(int(24))),
                    Stmt::Expr(Expr {
                        kind: ExprKind::Call(
                            FnId(1),
                            vec![Expr {
                                kind: ExprKind::Borrow(LocalId(1)),
                                ty: Ty::Borrowed,
                            }],
                        ),
                        ty: Ty::I32,
                    }),
                    Stmt::Release(LocalId(1)),
                ],
                step: vec![set(
                    2,
                    Expr {
                        kind: ExprKind::Binary(BinOp::Add, Box::new(get(2)), Box::new(int(1))),
                        ty: Ty::I32,
                    },
                )],
            },
            Stmt::Return(Some(get(0))),
        ],
    };
    Module {
        globals: Vec::new(),
        functions: vec![work, peek],
        data: Vec::new(),
    }
}

/// `twice(n: i32): void` releases its block twice: the verifier refuses it, and
/// the debug allocator catches it at run time.
pub fn twice() -> Module {
    one(
        "twice",
        vec![
            set(1, alloc(int(8))),
            Stmt::Release(LocalId(1)),
            Stmt::Release(LocalId(1)),
        ],
    )
}

/// `keep(n: i32): void` never releases its block: a leak.
pub fn keep() -> Module {
    one("keep", vec![set(1, alloc(int(8)))])
}

fn one(name: &str, body: Vec<Stmt>) -> Module {
    Module {
        globals: Vec::new(),
        functions: vec![Function {
            name: name.to_owned(),
            exported: true,
            job: false,
            param_count: 1,
            locals: vec![local("n", Ty::I32), local("block", Ty::Owned)],
            ret: Ty::Void,
            more: Vec::new(),
            shape: None,
            body,
        }],
        data: Vec::new(),
    }
}
