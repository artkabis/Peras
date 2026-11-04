//! The IR verifier: re-checks the types and structure of the IR.
//!
//! It runs after lowering and after every optimization pass. A failure is never
//! the user's fault: it is a bug in `perasc`, caught before any code is generated.
//! WebAssembly only knows four value types, so without this check a `bool` used as
//! an `i32`, or one integer type for another, could reach the module unnoticed.

use crate::{BinOp, Const, Expr, ExprKind, Function, MathOp, Module, Stmt, Ty, UnOp};

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
            problems: &mut problems,
        };
        verifier.stmts(&function.body);
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
    problems: &'a mut Vec<String>,
}

impl Verifier<'_> {
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

    fn stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Set(local, value) => {
                self.expr(value);
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
            Stmt::Break | Stmt::Continue => {
                if self.loops == 0 {
                    self.problem("`break` or `continue` outside a loop");
                }
            }
            Stmt::Return(value) => {
                let ret = self.function.ret;
                match value {
                    Some(value) => {
                        self.expr(value);
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
            }
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
            ExprKind::Call(id, args) => {
                for arg in args {
                    self.expr(arg);
                }
                let callee = usize::try_from(id.0)
                    .ok()
                    .and_then(|i| self.module.functions.get(i));
                let Some(callee) = callee else {
                    self.problem(&format!("calls missing function #{}", id.0));
                    return;
                };
                let params: Vec<Ty> = callee.params().map(|p| p.ty).collect();
                let given: Vec<Ty> = args.iter().map(|a| a.ty).collect();
                if params != given || callee.ret != e.ty {
                    self.problem(&format!(
                        "call to {} does not match its signature",
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
                BinOp::Eq | BinOp::Ne => operand != Ty::Void,
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
