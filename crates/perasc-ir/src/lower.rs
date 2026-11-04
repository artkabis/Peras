//! Lowering: from a checked syntax tree to the IR.
//!
//! The checker has already accepted the module, so names resolve and types match;
//! this pass only reshapes. What the backends cannot compile yet (structures,
//! arrays, strings, options, `for…of`, `inout` and the other constructs listed in
//! `E0900`) is reported, every occurrence at once, instead of being miscompiled.

use std::collections::HashMap;

use perasc_check::Analysis;
use perasc_check::ty::{FloatTy, IntTy, Ty as CheckTy};
use perasc_diag::{Diagnostic, Fix, Label, Message, Span, codes};
use perasc_syntax::ast::{
    AssignOp, Ast, BinaryOp, Convention, ExprId, ExprKind as AstExpr, FnDecl, ItemKind, StepOp,
    Stmt as AstStmt, StmtKind, Symbol, Type, TypeKind, UnaryOp,
};

use crate::{
    BinOp, Const, Expr, ExprKind, FnId, Function, Local, LocalId, MathOp, Module, Stmt, Ty, UnOp,
};

/// Lowers a checked module to the IR, or reports what code generation does not support yet.
///
/// # Errors
///
/// One `E0900` diagnostic per unsupported construct, sorted by position.
pub fn lower(ast: &Ast, analysis: &Analysis) -> Result<Module, Vec<Diagnostic>> {
    let mut lowering = Lowering {
        ast,
        analysis,
        fns: HashMap::new(),
        diagnostics: Vec::new(),
    };
    let mut index = 0u32;
    for item in &ast.items {
        match &item.kind {
            ItemKind::Fn(decl) => {
                lowering.fns.insert(decl.name.symbol, FnId(index));
                index += 1;
            }
            ItemKind::Struct(decl) => lowering.unsupported(decl.name.span, "struct"),
            ItemKind::Global(local) => lowering.unsupported(local.name.span, "global"),
        }
    }
    let functions = ast
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Fn(decl) => Some(lowering.function(decl)),
            _ => None,
        })
        .collect();
    if lowering.diagnostics.is_empty() {
        Ok(Module { functions })
    } else {
        let mut diagnostics = lowering.diagnostics;
        diagnostics.sort_by_key(|d| d.primary_span().map(|s| s.start));
        Err(diagnostics)
    }
}

struct Lowering<'a> {
    ast: &'a Ast,
    analysis: &'a Analysis,
    fns: HashMap<Symbol, FnId>,
    diagnostics: Vec<Diagnostic>,
}

/// The state of one function being lowered.
struct FnState {
    locals: Vec<Local>,
    scopes: Vec<HashMap<Symbol, LocalId>>,
}

impl FnState {
    fn add(&mut self, symbol: Symbol, name: &str, ty: Ty) -> LocalId {
        let id = LocalId(u32::try_from(self.locals.len()).unwrap_or(u32::MAX));
        self.locals.push(Local {
            name: name.to_owned(),
            ty,
        });
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(symbol, id);
        }
        id
    }

    fn lookup(&self, symbol: Symbol) -> Option<LocalId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(&symbol).copied())
    }
}

impl Lowering<'_> {
    /// `E0900`: a construct the backends cannot compile yet.
    fn unsupported(&mut self, span: Span, feature: &'static str) {
        self.diagnostics.push(Diagnostic {
            code: codes::NOT_SUPPORTED_YET,
            message: Message::new("gen-unsupported").text("feature", feature),
            labels: vec![Label {
                span,
                message: Message::new("gen-unsupported-label").text("feature", feature),
                primary: true,
            }],
            cause: Message::new("gen-unsupported-why"),
            fix: Fix {
                message: Message::new("gen-unsupported-fix"),
                edits: Vec::new(),
            },
        });
    }

    /// The IR type of a checker type, or `None` for types not compiled yet.
    fn ir_type(ty: &CheckTy) -> Option<Ty> {
        Some(match ty {
            CheckTy::Int(IntTy::I32) | CheckTy::IntLit => Ty::I32,
            CheckTy::Int(IntTy::I64) => Ty::I64,
            CheckTy::Float(FloatTy::F32) => Ty::F32,
            CheckTy::Float(FloatTy::F64) | CheckTy::FloatLit => Ty::F64,
            CheckTy::Bool => Ty::Bool,
            CheckTy::Void => Ty::Void,
            _ => return None,
        })
    }

    /// The IR type of a type written in source.
    fn written_type(&mut self, ty: &Type) -> Ty {
        let TypeKind::Named { name, args } = &ty.kind else {
            return Ty::Void;
        };
        let text = self.ast.name(name.symbol);
        let found = match (text, args.is_empty()) {
            ("i32", true) => Some(Ty::I32),
            ("i64", true) => Some(Ty::I64),
            ("f32", true) => Some(Ty::F32),
            ("f64", true) => Some(Ty::F64),
            ("bool", true) => Some(Ty::Bool),
            ("void", true) => Some(Ty::Void),
            _ => None,
        };
        found.unwrap_or_else(|| {
            self.unsupported(ty.span, feature_of_type(text));
            Ty::Void
        })
    }

    /// The type the checker gave an expression.
    fn type_of(&mut self, id: ExprId) -> Ty {
        let span = self.ast.expr(id).span;
        match self.analysis.types.get(&id) {
            Some(ty) => Self::ir_type(ty).unwrap_or_else(|| {
                self.unsupported(span, feature_of_check_type(ty));
                Ty::Void
            }),
            None => Ty::Void,
        }
    }

    fn function(&mut self, decl: &FnDecl) -> Function {
        let mut state = FnState {
            locals: Vec::new(),
            scopes: vec![HashMap::new()],
        };
        let errors_before = self.diagnostics.len();
        for param in &decl.params {
            if param.convention == Convention::Inout {
                self.unsupported(param.name.span, "inout");
            }
            let ty = self.written_type(&param.ty);
            state.add(param.name.symbol, self.ast.name(param.name.symbol), ty);
        }
        let ret = decl.ret.as_ref().map_or(Ty::Void, |t| self.written_type(t));
        let mut body = Vec::new();
        // A signature already reported: its body would only repeat the same cause
        // (`p.x` for an unsupported `p: Point`), so it is not lowered.
        if self.diagnostics.len() == errors_before {
            self.block(&mut state, &decl.body.stmts, &mut body);
        }
        Function {
            name: self.ast.name(decl.name.symbol).to_owned(),
            exported: decl.exported,
            param_count: u32::try_from(decl.params.len()).unwrap_or(u32::MAX),
            locals: state.locals,
            ret,
            body,
        }
    }

    fn block(&mut self, state: &mut FnState, stmts: &[AstStmt], out: &mut Vec<Stmt>) {
        state.scopes.push(HashMap::new());
        for stmt in stmts {
            self.stmt(state, stmt, out);
        }
        state.scopes.pop();
    }

    /// A body: a block or a single statement, in its own scope.
    fn body(&mut self, state: &mut FnState, stmt: &AstStmt) -> Vec<Stmt> {
        let mut out = Vec::new();
        self.block(state, std::slice::from_ref(stmt), &mut out);
        out
    }

    fn stmt(&mut self, state: &mut FnState, stmt: &AstStmt, out: &mut Vec<Stmt>) {
        match &stmt.kind {
            StmtKind::Local(local) => {
                let ty = match (&local.ty, local.init) {
                    (Some(t), _) => self.written_type(t),
                    (None, Some(init)) => self.type_of(init),
                    (None, None) => Ty::I32,
                };
                let value = match local.init {
                    Some(init) => self.expr(state, init),
                    None => zero(ty),
                };
                let id = state.add(local.name.symbol, self.ast.name(local.name.symbol), ty);
                out.push(Stmt::Set(id, value));
            }
            StmtKind::Return(value) => {
                let value = value.map(|v| self.expr(state, v));
                out.push(Stmt::Return(value));
            }
            StmtKind::Expr(e) => {
                let e = self.expr(state, *e);
                out.push(Stmt::Expr(e));
            }
            StmtKind::Assign { target, op, value } => {
                let Some(local) = self.target(state, *target) else {
                    return;
                };
                let value = self.expr(state, *value);
                let value = match assign_op(*op) {
                    None => value,
                    Some(op) => binary(op, local_expr(state, local), value),
                };
                out.push(Stmt::Set(local, value));
            }
            StmtKind::Step { target, op } => {
                let Some(local) = self.target(state, *target) else {
                    return;
                };
                let current = local_expr(state, local);
                let one = one(current.ty);
                let op = if *op == StepOp::Increment {
                    BinOp::Add
                } else {
                    BinOp::Sub
                };
                out.push(Stmt::Set(local, binary(op, current, one)));
            }
            StmtKind::Block(block) => self.block(state, &block.stmts, out),
            StmtKind::If {
                cond,
                then,
                otherwise,
            } => {
                let cond = self.expr(state, *cond);
                let then = self.body(state, then);
                let otherwise = otherwise
                    .as_ref()
                    .map(|o| self.body(state, o))
                    .unwrap_or_default();
                out.push(Stmt::If {
                    cond,
                    then,
                    otherwise,
                });
            }
            StmtKind::While { cond, body } => {
                let cond = self.loop_condition(state, *cond);
                let body = self.body(state, body);
                out.push(Stmt::Loop {
                    cond,
                    body,
                    step: Vec::new(),
                });
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
            } => {
                state.scopes.push(HashMap::new());
                if let Some(init) = init {
                    self.stmt(state, init, out);
                }
                let cond = cond.and_then(|c| self.loop_condition(state, c));
                let mut step = Vec::new();
                if let Some(update) = update {
                    self.stmt(state, update, &mut step);
                }
                let body = self.body(state, body);
                state.scopes.pop();
                out.push(Stmt::Loop { cond, body, step });
            }
            StmtKind::ForOf { name, .. } => self.unsupported(name.span, "for-of"),
            StmtKind::Break => out.push(Stmt::Break),
            StmtKind::Continue => out.push(Stmt::Continue),
            StmtKind::Error => {}
        }
    }

    /// A loop condition; `true` means no condition.
    fn loop_condition(&mut self, state: &mut FnState, cond: ExprId) -> Option<Expr> {
        let cond = self.expr(state, cond);
        (cond.kind != ExprKind::Const(Const::Bool(true))).then_some(cond)
    }

    /// The local an assignment or increment writes; only variables for now.
    fn target(&mut self, state: &FnState, target: ExprId) -> Option<LocalId> {
        let expr = self.ast.expr(target);
        match &expr.kind {
            AstExpr::Name(symbol) => state.lookup(*symbol),
            AstExpr::Paren(inner) => self.target(state, *inner),
            _ => {
                self.unsupported(expr.span, "place");
                None
            }
        }
    }

    fn expr(&mut self, state: &mut FnState, id: ExprId) -> Expr {
        let ty = self.type_of(id);
        let ast_expr = self.ast.expr(id);
        let span = ast_expr.span;
        let kind = match &ast_expr.kind {
            AstExpr::Int(value) => ExprKind::Const(int_const(*value, false)),
            AstExpr::Float(value) => ExprKind::Const(Const::Float(*value)),
            AstExpr::Bool(value) => ExprKind::Const(Const::Bool(*value)),
            AstExpr::Name(symbol) => {
                let Some(local) = state.lookup(*symbol) else {
                    // Not a local: a function used as a value, or a module-level constant.
                    let feature = if self.fns.contains_key(symbol) {
                        "function-value"
                    } else {
                        "global"
                    };
                    self.unsupported(span, feature);
                    return zero(ty);
                };
                ExprKind::Local(local)
            }
            AstExpr::Paren(inner) => return self.expr(state, *inner),
            AstExpr::Unary { op, operand } => {
                if let (UnaryOp::Neg, AstExpr::Int(value)) = (op, &self.ast.expr(*operand).kind) {
                    ExprKind::Const(int_const(*value, true))
                } else {
                    let op = match op {
                        UnaryOp::Neg => UnOp::Neg,
                        UnaryOp::Not => UnOp::Not,
                        UnaryOp::BitNot => UnOp::BitNot,
                        UnaryOp::Borrow => {
                            self.unsupported(span, "borrow");
                            return zero(ty);
                        }
                    };
                    ExprKind::Unary(op, Box::new(self.expr(state, *operand)))
                }
            }
            AstExpr::Binary { op, lhs, rhs } => {
                let lhs = self.expr(state, *lhs);
                let rhs = self.expr(state, *rhs);
                if *op == BinaryOp::Rem && lhs.ty.is_float() {
                    self.unsupported(span, "float-remainder");
                }
                ExprKind::Binary(bin_op(*op), Box::new(lhs), Box::new(rhs))
            }
            AstExpr::Call { callee, args } => return self.call(state, *callee, args, ty, span),
            AstExpr::String(_) => return self.not_yet(span, "string", ty),
            AstExpr::Array(_) => return self.not_yet(span, "array", ty),
            AstExpr::Field { .. } => return self.not_yet(span, "field", ty),
            AstExpr::Index { .. } => return self.not_yet(span, "index", ty),
            AstExpr::Try(_) => return self.not_yet(span, "try", ty),
            AstExpr::Error => return zero(ty),
        };
        Expr { kind, ty }
    }

    fn not_yet(&mut self, span: Span, feature: &'static str, ty: Ty) -> Expr {
        self.unsupported(span, feature);
        zero(ty)
    }

    fn call(
        &mut self,
        state: &mut FnState,
        callee: ExprId,
        args: &[ExprId],
        ty: Ty,
        span: Span,
    ) -> Expr {
        match &self.ast.expr(callee).kind {
            AstExpr::Name(symbol) => {
                let Some(&id) = self.fns.get(symbol) else {
                    // A structure constructor or `Some`.
                    let feature = if self.ast.name(*symbol) == "Some" {
                        "option"
                    } else {
                        "struct"
                    };
                    return self.not_yet(span, feature, ty);
                };
                let args = args.iter().map(|&a| self.expr(state, a)).collect();
                Expr {
                    kind: ExprKind::Call(id, args),
                    ty,
                }
            }
            AstExpr::Field { base, name } => {
                let receiver = self.expr(state, *base);
                let method = self.ast.name(name.symbol);
                self.method(state, receiver, method, args, ty, span)
            }
            _ => self.not_yet(span, "function-value", ty),
        }
    }

    /// Built-in methods on numbers.
    fn method(
        &mut self,
        state: &mut FnState,
        receiver: Expr,
        method: &str,
        args: &[ExprId],
        ty: Ty,
        span: Span,
    ) -> Expr {
        let math = match method {
            "abs" => Some(MathOp::Abs),
            "sqrt" => Some(MathOp::Sqrt),
            "floor" => Some(MathOp::Floor),
            "ceil" => Some(MathOp::Ceil),
            _ => None,
        };
        if let Some(op) = math {
            return Expr {
                kind: ExprKind::Math(op, Box::new(receiver)),
                ty,
            };
        }
        if matches!(method, "toI32" | "toI64" | "toF32" | "toF64") {
            return Expr {
                kind: ExprKind::Convert(Box::new(receiver)),
                ty,
            };
        }
        let wrapping = match method {
            "wrappingAdd" => Some(BinOp::Add),
            "wrappingSub" => Some(BinOp::Sub),
            "wrappingMul" => Some(BinOp::Mul),
            _ => None,
        };
        if let (Some(op), Some(&arg)) = (wrapping, args.first()) {
            let arg = self.expr(state, arg);
            return binary(op, receiver, arg);
        }
        self.not_yet(span, "method", ty)
    }
}

/// The feature name for a type written in source that is not compiled yet.
fn feature_of_type(name: &str) -> &'static str {
    match name {
        "Array" => "array",
        "Option" => "option",
        "String" => "string",
        "i8" | "i16" | "u8" | "u16" | "u32" | "u64" => "integer-type",
        _ => "struct",
    }
}

fn feature_of_check_type(ty: &CheckTy) -> &'static str {
    match ty {
        CheckTy::Array(_) => "array",
        CheckTy::Option(_) => "option",
        CheckTy::String => "string",
        CheckTy::Int(_) => "integer-type",
        CheckTy::Fn(..) => "function-value",
        _ => "struct",
    }
}

/// An integer literal, negated when written `-literal`. The checker has already
/// checked the range, so the value fits the target type.
fn int_const(value: u64, negative: bool) -> Const {
    let wide = i128::from(value);
    let signed = if negative { -wide } else { wide };
    Const::Int(i64::try_from(signed).unwrap_or(i64::MIN))
}

fn zero(ty: Ty) -> Expr {
    let value = match ty {
        Ty::F32 | Ty::F64 => Const::Float(0.0),
        Ty::Bool => Const::Bool(false),
        _ => Const::Int(0),
    };
    Expr {
        kind: ExprKind::Const(value),
        ty,
    }
}

fn one(ty: Ty) -> Expr {
    let value = if ty.is_float() {
        Const::Float(1.0)
    } else {
        Const::Int(1)
    };
    Expr {
        kind: ExprKind::Const(value),
        ty,
    }
}

fn local_expr(state: &FnState, local: LocalId) -> Expr {
    let ty = usize::try_from(local.0)
        .ok()
        .and_then(|i| state.locals.get(i))
        .map_or(Ty::Void, |l| l.ty);
    Expr {
        kind: ExprKind::Local(local),
        ty,
    }
}

fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let ty = if op.is_comparison() || matches!(op, BinOp::And | BinOp::Or) {
        Ty::Bool
    } else {
        lhs.ty
    };
    Expr {
        kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
        ty,
    }
}

const fn assign_op(op: AssignOp) -> Option<BinOp> {
    match op {
        AssignOp::Assign => None,
        AssignOp::Add => Some(BinOp::Add),
        AssignOp::Sub => Some(BinOp::Sub),
        AssignOp::Mul => Some(BinOp::Mul),
        AssignOp::Div => Some(BinOp::Div),
        AssignOp::Rem => Some(BinOp::Rem),
    }
}

const fn bin_op(op: BinaryOp) -> BinOp {
    match op {
        BinaryOp::Add => BinOp::Add,
        BinaryOp::Sub => BinOp::Sub,
        BinaryOp::Mul => BinOp::Mul,
        BinaryOp::Div => BinOp::Div,
        BinaryOp::Rem => BinOp::Rem,
        BinaryOp::BitAnd => BinOp::BitAnd,
        BinaryOp::BitOr => BinOp::BitOr,
        BinaryOp::BitXor => BinOp::BitXor,
        BinaryOp::Shl => BinOp::Shl,
        BinaryOp::Shr => BinOp::Shr,
        BinaryOp::Eq => BinOp::Eq,
        BinaryOp::Ne => BinOp::Ne,
        BinaryOp::Lt => BinOp::Lt,
        BinaryOp::Le => BinOp::Le,
        BinaryOp::Gt => BinOp::Gt,
        BinaryOp::Ge => BinOp::Ge,
        BinaryOp::And => BinOp::And,
        BinaryOp::Or => BinOp::Or,
    }
}

#[cfg(test)]
mod tests;
