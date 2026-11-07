//! The IR verifier: re-checks the types and structure of the IR.
//!
//! It runs after lowering and after every optimization pass. A failure is never
//! the user's fault: it is a bug in `perasc`, caught before any code is generated.
//! WebAssembly only knows four value types, so without this check a `bool` used as
//! an `i32`, or one integer type for another, could reach the module unnoticed.

use crate::{
    BinOp, BufOp, BufStmt, Const, Expr, ExprKind, Function, LocalId, MathOp, Module, Stmt, Ty, UnOp,
};

/// Checks a module; every problem is described in one line.
///
/// # Errors
///
/// The list of problems found.
pub fn verify(module: &Module) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    for function in &module.functions {
        let mut verifier = Verifier {
            module,
            function,
            loops: 0,
            blocks: 0,
            problems: &mut problems,
        };
        verifier.locals();
        verifier.stmts(&function.body);
    }
    // Releases are checked on well-typed code only.
    if problems.is_empty() {
        crate::own::check(module, &mut problems);
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

struct Verifier<'a> {
    module: &'a Module,
    function: &'a Function,
    /// Loops around the current statement, for `break` and `continue`.
    loops: usize,
    /// Blocks around the current statement, for `leave`.
    blocks: u32,
    problems: &'a mut Vec<String>,
}

impl<'a> Verifier<'a> {
    fn problem(&mut self, text: &str) {
        self.problems
            .push(format!("fn {}: {text}", self.function.name));
    }

    fn local_type(&self, index: u32) -> Option<Ty> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.function.locals.get(i))
            .map(|l| l.ty)
    }

    /// Only a parameter may be `borrowed`: a reference is never stored.
    fn locals(&mut self) {
        let params = usize::try_from(self.function.param_count).unwrap_or(usize::MAX);
        for (i, local) in self.function.locals.iter().enumerate() {
            if local.ty == Ty::Borrowed && i >= params {
                self.problem(&format!(
                    "%{i} is a borrowed variable: only a parameter can borrow"
                ));
            }
        }
        if self.function.ret == Ty::Borrowed {
            self.problem("returns a borrowed block, which would outlive its owner");
        }
    }

    /// A value given to an `owned` local or returned: a new block, a moved one,
    /// or the result of a call.
    fn handed_over(&mut self, value: &Expr) {
        match &value.kind {
            ExprKind::Move(local) => self.handle(*local, Ty::Owned, value.ty),
            ExprKind::Alloc(_)
            | ExprKind::Call(..)
            | ExprKind::Buf(BufOp::New(_) | BufOp::Literal(_) | BufOp::Clone | BufOp::Take, _) => {
                self.expr(value);
            }
            _ => self.problem(&format!(
                "an owned value must be allocated, moved or returned by a call, not {:?}",
                value.kind
            )),
        }
    }

    /// A call argument: a block can be moved (`sink`) or lent (`borrow`), and a
    /// borrowed parameter passed on.
    fn argument(&mut self, arg: &Expr) {
        match &arg.kind {
            ExprKind::Move(local) => self.handle(*local, Ty::Owned, arg.ty),
            ExprKind::Borrow(local) => {
                self.handle(*local, Ty::Owned, Ty::Owned);
                if arg.ty != Ty::Borrowed {
                    self.problem(&format!("borrow of %{} typed {}", local.0, arg.ty.name()));
                }
            }
            ExprKind::Local(local) if self.local_type(local.0) == Some(Ty::Borrowed) => {
                if arg.ty != Ty::Borrowed {
                    self.problem(&format!("%{} read as {}", local.0, arg.ty.name()));
                }
            }
            _ => self.expr(arg),
        }
    }

    /// A buffer operand: a `borrow` of an `owned` local, or a `borrowed` parameter.
    fn buffer(&mut self, arg: Option<&Expr>) {
        match arg.map(|a| (&a.kind, a.ty)) {
            Some((ExprKind::Borrow(local), Ty::Borrowed))
                if self.local_type(local.0) == Some(Ty::Owned) => {}
            Some((ExprKind::Local(local), Ty::Borrowed))
                if self.local_type(local.0) == Some(Ty::Borrowed) => {}
            // An element of an array of buffers, lent.
            Some((ExprKind::Buf(BufOp::Get(crate::Elem::Handle), args), Ty::Borrowed)) => {
                self.buf_expr(BufOp::Get(crate::Elem::Handle), args, Ty::Borrowed);
            }
            other => self.problem(&format!("a buffer operand must be borrowed, not {other:?}")),
        }
    }

    /// Operands after the buffer, with the types they must have.
    fn operands(&mut self, what: &str, args: &[Expr], types: &[Ty]) {
        let given: Vec<Ty> = args.iter().map(|a| a.ty).collect();
        if given != types {
            self.problem(&format!("{what} on {given:?}, expected {types:?}"));
        }
        for arg in args {
            // A buffer stored into an element is handed over to the array.
            if arg.ty == Ty::Owned {
                self.handed_over(arg);
            } else {
                self.expr(arg);
            }
        }
    }

    fn buf_expr(&mut self, op: BufOp, args: &[Expr], ty: Ty) {
        let rest = args.get(1..).unwrap_or(&[]);
        let expected = match op {
            BufOp::New(_) => {
                self.operands("buf.new", args, &[Ty::I32]);
                Ty::Owned
            }
            BufOp::Take => {
                self.buffer(args.first());
                self.operands("buf.take", rest, &[Ty::I32]);
                Ty::Owned
            }
            BufOp::Clone => {
                self.buffer(args.first());
                self.operands("buf.clone", rest, &[]);
                Ty::Owned
            }
            BufOp::Literal(index) => {
                self.operands("buf.literal", args, &[]);
                if usize::try_from(index).map_or(true, |i| i >= self.module.data.len()) {
                    self.problem(&format!("buf.literal #{index} has no data"));
                }
                Ty::Owned
            }
            BufOp::Len => {
                self.buffer(args.first());
                self.operands("buf.len", rest, &[]);
                Ty::I32
            }
            BufOp::Get(elem) => {
                self.buffer(args.first());
                self.operands("buf.get", rest, &[Ty::I32]);
                elem.ty()
            }
        };
        if ty != expected {
            self.problem(&format!("{} typed {}", op.name(), ty.name()));
        }
    }

    fn buf_stmt(&mut self, op: BufStmt, args: &[Expr]) {
        let rest = args.get(1..).unwrap_or(&[]);
        match op {
            BufStmt::Set(elem) => {
                self.buffer(args.first());
                self.operands("buf.set", rest, &[Ty::I32, elem.stored()]);
            }
            BufStmt::Push(elem) => {
                self.buffer(args.first());
                self.operands("buf.push", rest, &[elem.stored()]);
            }
            BufStmt::Truncate => {
                self.buffer(args.first());
                self.operands("buf.truncate", rest, &[Ty::I32]);
            }
            BufStmt::CheckIndex => self.operands("check_index", args, &[Ty::I32, Ty::I32]),
            BufStmt::Append(_) => {
                self.buffer(args.first());
                self.buffer(args.get(1));
                if args.len() != 2 {
                    self.problem("buf.append takes two buffers");
                }
            }
            BufStmt::Release => match args {
                [
                    Expr {
                        kind: ExprKind::Move(local),
                        ty,
                    },
                ] => self.handle(*local, Ty::Owned, *ty),
                _ => self.problem("buf.release takes one moved buffer"),
            },
        }
    }

    /// `local` must have type `expected`, and the expression `ty` must be owned.
    fn handle(&mut self, local: LocalId, expected: Ty, ty: Ty) {
        if self.local_type(local.0) != Some(expected) {
            self.problem(&format!("%{} is not {}", local.0, expected.name()));
        }
        if ty != Ty::Owned {
            self.problem(&format!("move of %{} typed {}", local.0, ty.name()));
        }
    }

    fn stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Set(local, value) => {
                match self.local_type(local.0) {
                    Some(Ty::Owned) => self.handed_over(value),
                    Some(Ty::Borrowed) => {
                        self.problem(&format!("%{} is borrowed and cannot be assigned", local.0));
                    }
                    _ => self.expr(value),
                }
                match self.local_type(local.0) {
                    Some(ty) if ty == value.ty => {}
                    Some(ty) => self.problem(&format!(
                        "%{} is {} but receives {}",
                        local.0,
                        ty.name(),
                        value.ty.name()
                    )),
                    None => self.problem(&format!("%{} does not exist", local.0)),
                }
            }
            Stmt::Expr(e) => self.expr(e),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.condition(cond);
                self.stmts(then);
                self.stmts(otherwise);
            }
            Stmt::Loop { cond, body, step } => {
                if let Some(cond) = cond {
                    self.condition(cond);
                }
                self.loops += 1;
                self.stmts(body);
                self.stmts(step);
                self.loops -= 1;
            }
            Stmt::Block(body) => {
                self.blocks += 1;
                self.stmts(body);
                self.blocks -= 1;
            }
            Stmt::Leave(depth) => {
                if *depth >= self.blocks {
                    self.problem(&format!("`leave {depth}` inside {} blocks", self.blocks));
                }
            }
            Stmt::Break | Stmt::Continue => {
                if self.loops == 0 {
                    self.problem("`break` or `continue` outside a loop");
                }
            }
            Stmt::CallMany {
                targets,
                callee,
                args,
            } => self.call_many(targets, *callee, args),
            Stmt::ReturnMany(values) => self.return_many(values),
            Stmt::Buf(op, args) => self.buf_stmt(*op, args),
            Stmt::Release(local) => {
                if self.local_type(local.0) != Some(Ty::Owned) {
                    self.problem(&format!("release of %{}, which is not owned", local.0));
                }
            }
            Stmt::Return(_) if !self.function.more.is_empty() => {
                self.problem("has several results: it returns them all at once");
            }
            Stmt::Return(value) => {
                let ret = self.function.ret;
                match value {
                    Some(value) => {
                        if ret == Ty::Owned {
                            self.handed_over(value);
                        } else {
                            self.expr(value);
                        }
                        if value.ty != ret || ret == Ty::Void {
                            self.problem(&format!(
                                "returns {} from a function returning {}",
                                value.ty.name(),
                                ret.name()
                            ));
                        }
                    }
                    None if ret != Ty::Void => {
                        self.problem(&format!("returns nothing but must return {}", ret.name()));
                    }
                    None => {}
                }
            }
        }
    }

    /// `targets = call callee(args)`, for a function with several results.
    fn call_many(&mut self, targets: &[LocalId], callee: crate::FnId, args: &[Expr]) {
        let mut types = Vec::with_capacity(targets.len());
        for target in targets {
            match self.local_type(target.0) {
                Some(Ty::Borrowed) => {
                    self.problem(&format!("%{} is borrowed and cannot be assigned", target.0));
                }
                Some(ty) => types.push(ty),
                None => self.problem(&format!("%{} does not exist", target.0)),
            }
        }
        if let Some(called) = self.call(callee, args, &types)
            && called.more.is_empty()
        {
            self.problem(&format!(
                "{} has one result: it is called by an expression",
                called.name
            ));
        }
    }

    /// `return a, b, …`, in a function with several results.
    fn return_many(&mut self, values: &[Expr]) {
        for value in values {
            if value.ty == Ty::Owned {
                self.handed_over(value);
            } else {
                self.expr(value);
            }
        }
        let given: Vec<Ty> = values.iter().map(|v| v.ty).collect();
        if self.function.more.is_empty() || given != self.function.results() {
            self.problem("returns several values that do not match its results");
        }
    }

    /// A call with `args` whose results are `results`; the callee, if it exists.
    fn call(&mut self, id: crate::FnId, args: &[Expr], results: &[Ty]) -> Option<&'a Function> {
        for arg in args {
            self.argument(arg);
        }
        let module = self.module;
        let Some(callee) = usize::try_from(id.0)
            .ok()
            .and_then(|i| module.functions.get(i))
        else {
            self.problem(&format!("calls missing function #{}", id.0));
            return None;
        };
        let params: Vec<Ty> = callee.params().map(|p| p.ty).collect();
        let given: Vec<Ty> = args.iter().map(|a| a.ty).collect();
        if params != given || callee.results() != results {
            self.problem(&format!(
                "call to {} does not match its signature",
                callee.name
            ));
        }
        Some(callee)
    }

    fn condition(&mut self, cond: &Expr) {
        self.expr(cond);
        if cond.ty != Ty::Bool {
            self.problem(&format!("condition of type {}", cond.ty.name()));
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Const(c) => {
                let fits = match c {
                    Const::Int(_) => e.ty.is_int(),
                    Const::Float(_) => e.ty.is_float(),
                    Const::Bool(_) => e.ty == Ty::Bool,
                };
                if !fits {
                    self.problem(&format!("constant {c:?} typed {}", e.ty.name()));
                }
            }
            ExprKind::Local(local) => {
                if self.local_type(local.0) != Some(e.ty) {
                    self.problem(&format!("%{} read as {}", local.0, e.ty.name()));
                }
                if e.ty.is_handle() {
                    self.problem(&format!(
                        "%{} holds a block: it can only be moved, borrowed or passed on",
                        local.0
                    ));
                }
            }
            ExprKind::Alloc(size) => {
                self.expr(size);
                if size.ty != Ty::I32 || e.ty != Ty::Owned {
                    self.problem(&format!("alloc of {} bytes typed {}", size.ty.name(), e.ty.name()));
                }
            }
            ExprKind::Move(local) | ExprKind::Borrow(local) => self.problem(&format!(
                "%{} is moved or borrowed where only an assignment, a return or an argument can take it",
                local.0
            )),
            ExprKind::Unary(op, operand) => {
                self.expr(operand);
                let valid = match op {
                    UnOp::Neg => operand.ty.is_int() || operand.ty.is_float(),
                    UnOp::Not => operand.ty == Ty::Bool,
                    UnOp::BitNot => operand.ty.is_int(),
                };
                if !valid || e.ty != operand.ty {
                    self.problem(&format!(
                        "{op:?} on {} typed {}",
                        operand.ty.name(),
                        e.ty.name()
                    ));
                }
            }
            ExprKind::Binary(op, lhs, rhs) => self.binary(*op, lhs, rhs, e.ty),
            ExprKind::Buf(op, args) => self.buf_expr(*op, args, e.ty),
            ExprKind::Call(id, args) => {
                let results = if e.ty == Ty::Void { vec![] } else { vec![e.ty] };
                if let Some(callee) = self.call(*id, args, &results)
                    && !callee.more.is_empty()
                {
                    self.problem(&format!(
                        "{} has several results: it is called by `call many` only",
                        callee.name
                    ));
                }
            }
            ExprKind::Convert(operand) => {
                self.expr(operand);
                let numeric = |t: Ty| t.is_int() || t.is_float();
                if !numeric(operand.ty) || !numeric(e.ty) {
                    self.problem(&format!(
                        "conversion from {} to {}",
                        operand.ty.name(),
                        e.ty.name()
                    ));
                }
            }
            ExprKind::Math(op, operand) => {
                self.expr(operand);
                let valid = match op {
                    MathOp::Abs => operand.ty.is_int() || operand.ty.is_float(),
                    MathOp::Sqrt | MathOp::Floor | MathOp::Ceil => operand.ty.is_float(),
                };
                if !valid || e.ty != operand.ty {
                    self.problem(&format!("{} on {}", op.name(), operand.ty.name()));
                }
            }
        }
    }

    fn binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr, ty: Ty) {
        self.expr(lhs);
        self.expr(rhs);
        let operand = lhs.ty;
        let valid_operands = lhs.ty == rhs.ty
            && match op {
                BinOp::And | BinOp::Or => operand == Ty::Bool,
                BinOp::Eq | BinOp::Ne => operand != Ty::Void && !operand.is_handle(),
                BinOp::Lt
                | BinOp::Le
                | BinOp::Gt
                | BinOp::Ge
                | BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div => operand.is_int() || operand.is_float(),
                BinOp::Rem
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor
                | BinOp::Shl
                | BinOp::Shr => operand.is_int(),
            };
        let expected = if op.is_comparison() || matches!(op, BinOp::And | BinOp::Or) {
            Ty::Bool
        } else {
            operand
        };
        if !valid_operands || ty != expected {
            self.problem(&format!(
                "{} on {} and {} typed {}",
                op.symbol(),
                lhs.ty.name(),
                rhs.ty.name(),
                ty.name()
            ));
        }
    }
}
