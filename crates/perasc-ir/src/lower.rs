//! Lowering: from a checked syntax tree to the IR.
//!
//! The checker has already accepted the module, so names resolve and types match;
//! this pass only reshapes. Structures become their scalars (see [`values`]).
//! What the backends cannot compile yet (arrays, strings, options, `for…of`,
//! `inout` and the other constructs listed in `E0900`) is reported, every
//! occurrence at once, instead of being miscompiled.

mod heap;
mod matching;
mod values;

use std::collections::{BTreeSet, HashMap};

use perasc_check::Analysis;
use perasc_check::ty::{FloatTy, IntTy, Ty as CheckTy};
use perasc_diag::{Diagnostic, Fix, Label, Message, Span, codes};
use perasc_syntax::ast::{
    AssignOp, Ast, BinaryOp, Convention, ExprId, ExprKind as AstExpr, FnDecl, ItemKind, StepOp,
    Stmt as AstStmt, StmtKind, Symbol, Type, TypeKind, UnaryOp,
};

use crate::{
    BinOp, Const, Expr, ExprKind, FnId, Function, JsShape, Local, LocalId, MathOp, Module, Stmt,
    Ty, UnOp,
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
        results: HashMap::new(),
        conventions: HashMap::new(),
        data: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut index = 0u32;
    for item in &ast.items {
        match &item.kind {
            ItemKind::Fn(decl) => {
                let id = FnId(index);
                lowering.fns.insert(decl.name.symbol, id);
                lowering
                    .conventions
                    .insert(id, decl.params.iter().map(|p| p.convention).collect());
                // Its results, for its callers; a type not compiled yet is
                // reported once, with the function itself.
                let mut results: Vec<Ty> = decl
                    .ret
                    .as_ref()
                    .and_then(|t| lowering.resolve(t).ok())
                    .and_then(|t| lowering.leaves(&t).ok())
                    .map(|leaves| leaves.into_iter().map(|(_, ty)| ty).collect())
                    .unwrap_or_default();
                results.extend(lowering.inout_numbers(decl));
                lowering.results.insert(id, results);
                index += 1;
            }
            // A structure or an enumeration is a type: its uses are lowered
            // where they appear.
            ItemKind::Struct(_) | ItemKind::Enum(_) => {}
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
        Ok(Module {
            functions,
            data: lowering.data,
        })
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
    /// The scalar results of each function: several for a structure.
    results: HashMap<FnId, Vec<Ty>>,
    /// How each function takes its parameters.
    conventions: HashMap<FnId, Vec<Convention>>,
    /// The bytes of the string literals, each once.
    data: Vec<Vec<u8>>,
    diagnostics: Vec<Diagnostic>,
}

/// What a name stands for.
#[derive(Debug, Clone)]
enum Binding {
    /// A scalar local.
    Scalar(LocalId),
    /// A structure: one local per scalar, in order.
    Struct(Vec<LocalId>),
    /// The element a `for…of` reaches, when it holds buffers: its scalars,
    /// read from the array at each use, since a lent buffer is never stored.
    Lent(Vec<Expr>),
}

/// The state of one function being lowered.
struct FnState {
    locals: Vec<Local>,
    scopes: Vec<HashMap<Symbol, Binding>>,
    /// Statements that must run before the statement being lowered: calls
    /// returning several results, and what keeps the order of evaluation.
    pre: Vec<Stmt>,
    /// When the function returns an option: its type, for the `None` of `?`.
    ret_option: Option<CheckTy>,
    /// The numbers of the `inout` parameters, returned after the result so
    /// that the caller writes them back.
    writeback: Vec<LocalId>,
    /// The `owned` locals declared in each open scope, parallel to `scopes`.
    owned: Vec<Vec<LocalId>>,
    /// The `owned` locals that hold a buffer here (see [`heap`]).
    live: BTreeSet<u32>,
    /// Whether the code here is unreachable: after `return`, `break`, `continue`.
    gone: bool,
    /// Temporaries holding buffers until the end of the statement.
    drops: Vec<LocalId>,
    /// The loops around the code being lowered, innermost last.
    loops: Vec<heap::LoopFrame>,
}

impl FnState {
    fn new() -> Self {
        Self {
            locals: Vec::new(),
            scopes: vec![HashMap::new()],
            pre: Vec::new(),
            ret_option: None,
            writeback: Vec::new(),
            owned: vec![Vec::new()],
            live: BTreeSet::new(),
            gone: false,
            drops: Vec::new(),
            loops: Vec::new(),
        }
    }

    /// Opens a scope.
    fn open(&mut self) {
        self.scopes.push(HashMap::new());
        self.owned.push(Vec::new());
    }

    /// Closes the innermost scope, releasing what it still holds.
    fn close(&mut self, out: &mut Vec<Stmt>) {
        if !self.gone {
            let depth = self.owned.len().saturating_sub(1);
            self.release_scopes(depth, out);
        }
        self.scopes.pop();
        self.owned.pop();
    }

    fn add(&mut self, symbol: Symbol, name: &str, ty: Ty) -> LocalId {
        let id = self.local(name, ty);
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(symbol, Binding::Scalar(id));
        }
        id
    }

    /// A structure's locals, named `p_x`, `p_inner_y`.
    fn add_struct(&mut self, symbol: Symbol, name: &str, leaves: &[(String, Ty)]) -> Vec<LocalId> {
        let ids: Vec<LocalId> = leaves
            .iter()
            .map(|(path, ty)| {
                let local = if path.is_empty() {
                    name.to_owned()
                } else {
                    format!("{name}_{path}")
                };
                self.local(&local, *ty)
            })
            .collect();
        for (&id, (_, ty)) in ids.iter().zip(leaves) {
            if *ty == Ty::Owned
                && let Some(scope) = self.owned.last_mut()
            {
                scope.push(id);
            }
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(symbol, Binding::Struct(ids.clone()));
        }
        ids
    }

    fn local(&mut self, name: &str, ty: Ty) -> LocalId {
        let id = LocalId(u32::try_from(self.locals.len()).unwrap_or(u32::MAX));
        self.locals.push(Local {
            name: name.to_owned(),
            ty,
        });
        id
    }

    fn lookup(&self, symbol: Symbol) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(&symbol).cloned())
    }

    /// Moves the statements waiting in `pre` to `out`.
    fn flush(&mut self, out: &mut Vec<Stmt>) {
        out.append(&mut self.pre);
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

    /// A parameter: its locals, how JavaScript passes it, and whether its
    /// numbers go back to the caller (`inout`).
    fn param(
        &mut self,
        state: &mut FnState,
        decl: &FnDecl,
        param: &perasc_syntax::ast::Param,
        shapes: &mut Vec<(String, crate::Shape)>,
        writeback: &mut Vec<(String, crate::Shape)>,
        has_struct: &mut bool,
    ) {
        let name = self.ast.name(param.name.symbol);
        let Some(ty) = self.typed(&param.ty) else {
            return;
        };
        *has_struct |= values::is_aggregate(&ty);
        let pass = match param.convention {
            Convention::Let => crate::Pass::Borrow,
            Convention::Inout => crate::Pass::Inout,
            Convention::Sink => crate::Pass::Sink,
        };
        shapes.push((name.to_owned(), self.shape_passed(&ty, pass)));
        match self.leaves(&ty) {
            Ok(leaves) if values::is_aggregate(&ty) => {
                // A parameter that is not `sink` only borrows its buffers;
                // `inout` changes them in place, and returns its numbers.
                let inout = param.convention == Convention::Inout;
                // A JavaScript string cannot change, nor be given back to an
                // option: there is nothing to copy back into.
                if inout && decl.exported && self.holds_string(&ty) {
                    self.unsupported(param.name.span, "inout-string");
                }
                if inout && decl.exported && matches!(ty, CheckTy::Option(_) | CheckTy::Enum(..)) {
                    self.unsupported(param.name.span, "inout-export");
                }
                let numbers = leaves.iter().any(|(_, t)| !t.is_handle());
                if inout && numbers {
                    writeback.push((name.to_owned(), self.shape_passed(&ty, crate::Pass::Inout)));
                }
                let leaves: Vec<(String, Ty)> = leaves
                    .into_iter()
                    .map(|(path, t)| match (t, param.convention) {
                        (Ty::Owned, Convention::Let | Convention::Inout) => (path, Ty::Borrowed),
                        other => (path, other.0),
                    })
                    .collect();
                let ids = state.add_struct(param.name.symbol, name, &leaves);
                for id in ids {
                    if state.is_owned(id) {
                        state.live.insert(id.0);
                    } else if inout && !state.is_borrowed(id) {
                        state.writeback.push(id);
                    }
                }
            }
            Ok(leaves) => {
                // A number passed `inout` from JavaScript has nowhere to go back.
                if param.convention == Convention::Inout && decl.exported {
                    self.unsupported(param.name.span, "inout-export");
                }
                let leaf = leaves.first().map_or(Ty::Void, |(_, t)| *t);
                let id = state.add(param.name.symbol, name, leaf);
                if param.convention == Convention::Inout {
                    state.writeback.push(id);
                }
            }
            Err(feature) => self.unsupported(param.ty.span, feature),
        }
    }

    fn function(&mut self, decl: &FnDecl) -> Function {
        let mut state = FnState::new();
        let errors_before = self.diagnostics.len();
        let mut shapes = Vec::new();
        let mut writeback = Vec::new();
        let mut has_struct = false;
        for param in &decl.params {
            self.param(
                &mut state,
                decl,
                param,
                &mut shapes,
                &mut writeback,
                &mut has_struct,
            );
        }
        let param_count = u32::try_from(state.locals.len()).unwrap_or(u32::MAX);
        let ret_ty = decl.ret.as_ref().and_then(|t| self.typed(t));
        has_struct |= ret_ty.as_ref().is_some_and(values::is_aggregate);
        let mut results: Vec<Ty> = match &ret_ty {
            Some(ty) if *ty != CheckTy::Void => match self.leaves(ty) {
                Ok(leaves) => leaves.into_iter().map(|(_, t)| t).collect(),
                Err(_) => Vec::new(),
            },
            _ => Vec::new(),
        };
        if matches!(ret_ty, Some(CheckTy::Option(_))) {
            state.ret_option.clone_from(&ret_ty);
        }
        // The `inout` numbers come back after the result.
        let inout_types: Vec<Ty> = state
            .writeback
            .iter()
            .map(|&l| local_expr(&state, l).ty)
            .collect();
        results.extend(inout_types);
        let mut body = Vec::new();
        // A signature already reported: its body would only repeat the same cause
        // (`xs.length` for an unsupported `xs: Array<i32>`), so it is not lowered.
        if self.diagnostics.len() == errors_before {
            self.block(&mut state, &decl.body.stmts, &mut body);
            // A `void` function that ends releases its `sink` parameters, and
            // gives back its `inout` numbers.
            if !state.gone {
                state.release_scopes(0, &mut body);
                if !state.writeback.is_empty() {
                    let values = state.writeback_values();
                    body.push(return_of(values));
                }
            }
        }
        let shape = (decl.exported && has_struct).then(|| JsShape {
            params: shapes,
            result: ret_ty
                .filter(|t| *t != CheckTy::Void)
                .map(|t| self.shape(&t)),
            writeback,
        });
        Function {
            name: self.ast.name(decl.name.symbol).to_owned(),
            exported: decl.exported,
            param_count,
            locals: state.locals,
            ret: results.first().copied().unwrap_or(Ty::Void),
            more: results.get(1..).map(<[Ty]>::to_vec).unwrap_or_default(),
            shape,
            body,
        }
    }

    /// The checker type of a type written in source; what is not compiled yet
    /// is reported.
    fn typed(&mut self, ty: &Type) -> Option<CheckTy> {
        match self.resolve(ty) {
            Ok(resolved) => Some(resolved),
            Err(feature) => {
                self.unsupported(ty.span, feature);
                None
            }
        }
    }

    fn block(&mut self, state: &mut FnState, stmts: &[AstStmt], out: &mut Vec<Stmt>) {
        state.open();
        for stmt in stmts {
            // What follows `return`, `break` or `continue` never runs.
            if state.gone {
                break;
            }
            self.stmt(state, stmt, out);
        }
        state.close(out);
    }

    /// A body: a block or a single statement, in its own scope.
    fn body(&mut self, state: &mut FnState, stmt: &AstStmt) -> Vec<Stmt> {
        let mut out = Vec::new();
        self.block(state, std::slice::from_ref(stmt), &mut out);
        out
    }

    fn stmt(&mut self, state: &mut FnState, stmt: &AstStmt, out: &mut Vec<Stmt>) {
        match &stmt.kind {
            StmtKind::Local(local) => self.local(state, local, out),
            StmtKind::Return(value) => self.return_stmt(state, *value, out),
            StmtKind::Expr(e) => {
                let values = self.value_as(state, *e, heap::Mode::Borrow);
                state.flush(out);
                // A structure's scalars are kept only for their effects.
                for value in values {
                    if !heap::pure(&value) {
                        out.push(Stmt::Expr(value));
                    }
                }
                state.end_statement(out);
            }
            StmtKind::Assign { target, op, value } => {
                if self.is_element_place(*target) {
                    self.index_assign(state, *target, assign_op(*op), Some(*value), out);
                } else {
                    self.assign_stmt(state, *target, *op, *value, out);
                    state.end_statement(out);
                }
            }
            StmtKind::Step { target, op } if self.is_element_place(*target) => {
                let op = if *op == StepOp::Increment {
                    BinOp::Add
                } else {
                    BinOp::Sub
                };
                self.index_assign(state, *target, Some(op), None, out);
            }
            StmtKind::Step { target, op } => {
                let Some(locals) = self.place(state, *target) else {
                    return;
                };
                let Some(&local) = locals.first() else {
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
            } => self.if_stmt(state, *cond, then, otherwise.as_deref(), out),
            StmtKind::While { cond, body } => {
                let (cond, before) = self.loop_condition(state, *cond);
                let lowered =
                    self.loop_body(state, cond, before, body, Vec::new(), &mut Vec::new());
                out.extend(lowered);
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
            } => {
                state.open();
                if let Some(init) = init {
                    self.stmt(state, init, out);
                }
                let (cond, before) = match cond {
                    Some(c) => self.loop_condition(state, *c),
                    None => (None, Vec::new()),
                };
                let mut step = Vec::new();
                if let Some(update) = update {
                    self.stmt(state, update, &mut step);
                }
                let lowered = self.loop_body(state, cond, before, body, step, &mut Vec::new());
                out.extend(lowered);
                state.close(out);
            }
            StmtKind::ForOf {
                name,
                iterable,
                body,
                ..
            } => self.for_of(state, name, *iterable, body, out),
            StmtKind::Break => Self::break_stmt(state, out),
            StmtKind::Continue => Self::continue_stmt(state, out),
            StmtKind::Error => {}
        }
    }

    /// `if (cond) then else otherwise`; where the branches meet, a buffer one
    /// of them moved is released on the other (see [`heap`]).
    fn if_stmt(
        &mut self,
        state: &mut FnState,
        cond: ExprId,
        then: &AstStmt,
        otherwise: Option<&AstStmt>,
        out: &mut Vec<Stmt>,
    ) {
        let cond = self.expr(state, cond);
        let cond = state
            .settle(vec![cond])
            .pop()
            .unwrap_or_else(|| zero(Ty::Bool));
        state.flush(out);
        let start = state.live.clone();
        let mut then = self.body(state, then);
        let after_then = (std::mem::replace(&mut state.live, start), state.gone);
        state.gone = false;
        let mut otherwise = otherwise.map(|o| self.body(state, o)).unwrap_or_default();
        let after_else = (state.live.clone(), state.gone);
        Self::merge(state, after_then, &mut then, after_else, &mut otherwise);
        out.push(Stmt::If {
            cond,
            then,
            otherwise,
        });
    }

    /// `break`: releases what the loop's body holds; the rest is decided when
    /// the whole loop is lowered.
    fn break_stmt(state: &mut FnState, out: &mut Vec<Stmt>) {
        if let Some(depth) = state.loops.last().map(|l| l.depth) {
            state.release_scopes(depth, out);
        }
        let live = state.live.clone();
        if let Some(frame) = state.loops.last_mut() {
            frame.breaks.push(live);
        }
        // A block of its own, where the releases of this way out go.
        out.push(Stmt::Block(vec![Stmt::Break]));
        state.gone = true;
    }

    /// `continue`: the next turn must find the locals as the loop started.
    fn continue_stmt(state: &mut FnState, out: &mut Vec<Stmt>) {
        if let Some((depth, head)) = state.loops.last().map(|l| (l.depth, l.head.clone())) {
            state.release_scopes(depth, out);
            let extra: BTreeSet<u32> = state.live.difference(&head).copied().collect();
            state.release_all(&extra, out);
        }
        out.push(Stmt::Continue);
        state.gone = true;
    }

    /// `return value;`: the value first, then every buffer still held is
    /// released, then the function returns.
    fn return_stmt(&mut self, state: &mut FnState, value: Option<ExprId>, out: &mut Vec<Stmt>) {
        let values = value.map(|v| self.value(state, v)).unwrap_or_default();
        let values = state.settle(values);
        let holding = !state.live.is_empty();
        let values = if holding {
            values
                .into_iter()
                .map(|v| {
                    if heap::pure(&v) || matches!(v.kind, ExprKind::Const(_)) {
                        v
                    } else if v.ty == Ty::Owned {
                        let temp = state.owned_temp(v);
                        heap::moved(state, temp)
                    } else {
                        state.spill(v)
                    }
                })
                .collect()
        } else {
            values
        };
        state.flush(out);
        state.release_scopes(0, out);
        let mut values = values;
        values.extend(state.writeback_values());
        out.push(return_of(values));
        state.gone = true;
    }

    /// `let name = value;`, a scalar or a structure.
    fn local(
        &mut self,
        state: &mut FnState,
        local: &perasc_syntax::ast::Local,
        out: &mut Vec<Stmt>,
    ) {
        let name = self.ast.name(local.name.symbol);
        let ty = match (&local.ty, local.init) {
            (Some(t), _) => self.typed(t),
            (None, Some(init)) => self.check_type(init).cloned(),
            (None, None) => None,
        };
        if let Some(ty) = ty.filter(values::is_aggregate) {
            let Ok(leaves) = self.leaves(&ty) else {
                self.unsupported(local.name.span, "struct");
                return;
            };
            let values = match local.init {
                Some(init) => self.value(state, init),
                None => leaves.iter().map(|(_, t)| zero(*t)).collect(),
            };
            state.flush(out);
            let ids = state.add_struct(local.name.symbol, name, &leaves);
            Self::assign(state, &ids, values, out);
            state.end_statement(out);
            return;
        }
        let ty = match (&local.ty, local.init) {
            (Some(t), _) => self.written_type(t),
            (None, Some(init)) => self.type_of(init),
            (None, None) => Ty::I32,
        };
        let value = match local.init {
            Some(init) => self.expr(state, init),
            None => zero(ty),
        };
        state.flush(out);
        let id = state.add(local.name.symbol, name, ty);
        out.push(Stmt::Set(id, value));
        state.end_statement(out);
    }

    /// `target = value`, or `target op= value`, on a variable or a field.
    fn assign_stmt(
        &mut self,
        state: &mut FnState,
        target: ExprId,
        op: AssignOp,
        value: ExprId,
        out: &mut Vec<Stmt>,
    ) {
        if op == AssignOp::Add && self.is_buffer(value) {
            self.append(state, target, value, out);
            return;
        }
        let Some(locals) = self.place(state, target) else {
            return;
        };
        let values = self.value(state, value);
        state.flush(out);
        match (assign_op(op), locals.as_slice(), values.as_slice()) {
            (Some(op), [local], [value]) => {
                let value = binary(op, local_expr(state, *local), value.clone());
                out.push(Stmt::Set(*local, value));
            }
            _ => Self::assign(state, &locals, values, out),
        }
    }

    /// A loop condition, `None` for `true`, and the statements it needs first.
    fn loop_condition(&mut self, state: &mut FnState, cond: ExprId) -> (Option<Expr>, Vec<Stmt>) {
        let cond = self.expr(state, cond);
        let cond = state
            .settle(vec![cond])
            .pop()
            .unwrap_or_else(|| zero(Ty::Bool));
        let before = std::mem::take(&mut state.pre);
        let cond = (cond.kind != ExprKind::Const(Const::Bool(true))).then_some(cond);
        (cond, before)
    }

    /// A loop whose condition needs statements first runs them at every turn:
    /// `loop { before; if (!cond) break; body } step`.
    fn loop_with(cond: Option<Expr>, before: Vec<Stmt>, body: Vec<Stmt>, step: Vec<Stmt>) -> Stmt {
        if before.is_empty() {
            return Stmt::Loop { cond, body, step };
        }
        let mut turn = before;
        if let Some(cond) = cond {
            turn.push(Stmt::If {
                cond: Expr {
                    kind: ExprKind::Unary(UnOp::Not, Box::new(cond)),
                    ty: Ty::Bool,
                },
                then: vec![Stmt::Break],
                otherwise: Vec::new(),
            });
        }
        turn.extend(body);
        Stmt::Loop {
            cond: None,
            body: turn,
            step,
        }
    }

    /// The locals an assignment or increment writes: a variable, or a field of
    /// a structure variable, as many scalars as it has.
    pub(super) fn place(&mut self, state: &FnState, target: ExprId) -> Option<Vec<LocalId>> {
        let expr = self.ast.expr(target);
        match &expr.kind {
            AstExpr::Name(symbol) => match state.lookup(*symbol)? {
                Binding::Scalar(local) => Some(vec![local]),
                Binding::Struct(locals) => Some(locals),
                Binding::Lent(_) => None,
            },
            AstExpr::Paren(inner) => self.place(state, *inner),
            AstExpr::Field { base, name } => {
                let locals = self.place(state, *base)?;
                let base_ty = self.check_type(*base).cloned()?;
                let (offset, len, _) = self.field(&base_ty, self.ast.name(name.symbol))?;
                Some(locals.into_iter().skip(offset).take(len).collect())
            }
            _ => {
                self.unsupported(expr.span, "place");
                None
            }
        }
    }

    fn expr(&mut self, state: &mut FnState, id: ExprId) -> Expr {
        if self.is_aggregate(id) {
            // Structures and options go through `value`: never a single scalar.
            let span = self.ast.expr(id).span;
            self.unsupported(span, "struct");
            return zero(Ty::Void);
        }
        let ty = self.type_of(id);
        let ast_expr = self.ast.expr(id);
        let span = ast_expr.span;
        let kind = match &ast_expr.kind {
            AstExpr::Int(value) => ExprKind::Const(int_const(*value, false)),
            AstExpr::Float(value) => ExprKind::Const(Const::Float(*value)),
            AstExpr::Bool(value) => ExprKind::Const(Const::Bool(*value)),
            AstExpr::Name(symbol) => {
                let Some(Binding::Scalar(local)) = state.lookup(*symbol) else {
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
            AstExpr::Binary { op, lhs, rhs }
                if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && self.is_buffer(*lhs) =>
            {
                return self.buffer_eq(state, *lhs, *rhs, *op == BinaryOp::Eq, span);
            }
            AstExpr::Binary { op, lhs, rhs } => {
                let lhs = self.expr(state, *lhs);
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return self.short_circuit(state, *op, lhs, *rhs);
                }
                let mark = state.pre.len();
                let rhs = self.expr(state, *rhs);
                let mut operands = [lhs];
                state.keep_before(mark, &mut operands);
                let [lhs] = operands;
                if *op == BinaryOp::Rem && lhs.ty.is_float() {
                    self.unsupported(span, "float-remainder");
                }
                ExprKind::Binary(bin_op(*op), Box::new(lhs), Box::new(rhs))
            }
            AstExpr::Call { callee, args } => return self.call(state, *callee, args, ty, span),
            AstExpr::String(_) => return self.not_yet(span, "string", ty),
            AstExpr::Array(_) => return self.not_yet(span, "array", ty),
            AstExpr::Field { base, .. } if self.is_buffer(*base) => {
                return self.array_length(state, *base);
            }
            AstExpr::Field { base, name } => {
                if !self.is_aggregate(*base) {
                    return self.not_yet(span, "field", ty);
                }
                let fields = self.value(state, *base);
                let base_ty = self.check_type(*base).cloned();
                let picked = base_ty.and_then(|t| self.field(&t, self.ast.name(name.symbol)));
                let Some((offset, len, _)) = picked else {
                    return zero(ty);
                };
                return Self::pick(state, fields, offset, len)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| zero(ty));
            }
            AstExpr::Index { base, index } => return self.index_get(state, *base, *index, span),
            AstExpr::Match { scrutinee, arms } => {
                return self
                    .match_values(state, id, *scrutinee, arms, heap::Mode::Borrow)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| zero(ty));
            }
            AstExpr::Try(inner) => {
                return self
                    .try_values(state, *inner, span)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| zero(ty));
            }
            AstExpr::Error => return zero(ty),
        };
        Expr { kind, ty }
    }

    /// `a && b`, `a || b`. When `b` needs statements first, they run only when
    /// `b` would: `t = a; if (t) { …; t = b }` for `&&`.
    fn short_circuit(&mut self, state: &mut FnState, op: BinaryOp, lhs: Expr, rhs: ExprId) -> Expr {
        let outer = std::mem::take(&mut state.pre);
        let live = state.live.clone();
        let rhs = self.expr(state, rhs);
        let inner = std::mem::replace(&mut state.pre, outer);
        let op = bin_op(op);
        // Buffers moved by the right side, which may not run: released when it
        // does not.
        let moved: Vec<Stmt> = live
            .difference(&state.live)
            .map(|&l| heap::release_stmt(LocalId(l)))
            .collect();
        if inner.is_empty() && moved.is_empty() {
            return binary(op, lhs, rhs);
        }
        let t = state.temp(Ty::Bool);
        state.pre.push(Stmt::Set(t, lhs));
        let t_expr = local_expr(state, t);
        let cond = if op == BinOp::And {
            t_expr.clone()
        } else {
            Expr {
                kind: ExprKind::Unary(UnOp::Not, Box::new(t_expr.clone())),
                ty: Ty::Bool,
            }
        };
        let mut then = inner;
        then.push(Stmt::Set(t, rhs));
        state.pre.push(Stmt::If {
            cond,
            then,
            otherwise: moved,
        });
        t_expr
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
                let back = self.writeback_places(state, id, args);
                let args = self.arguments(state, Some(id), args);
                self.call_results(state, id, args, &back)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| zero(ty))
            }
            AstExpr::Field { base, name } if self.is_aggregate(*base) => {
                let method = self.ast.name(name.symbol);
                self.method_values(state, *base, method, args, heap::Mode::Borrow, span)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| zero(ty))
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
            let mark = state.pre.len();
            let arg = self.expr(state, arg);
            let mut operands = [receiver];
            state.keep_before(mark, &mut operands);
            let [receiver] = operands;
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

/// `return`, with none, one or several values.
fn return_of(mut values: Vec<Expr>) -> Stmt {
    match values.len() {
        0 => Stmt::Return(None),
        1 => Stmt::Return(values.pop()),
        _ => Stmt::ReturnMany(values),
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
