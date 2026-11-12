//! Lowering: from a checked syntax tree to the IR.
//!
//! The checker has already accepted the module, so names resolve and types match;
//! this pass only reshapes. Structures become their scalars (see [`values`]).
//! What the backends cannot compile yet (arrays, strings, options, `for…of`,
//! `inout` and the other constructs listed in `E0900`) is reported, every
//! occurrence at once, instead of being miscompiled.

mod format;
mod functional;
mod heap;
mod maps;
mod matching;
mod parallel;
mod values;

use std::collections::{BTreeSet, HashMap};

use perasc_check::Analysis;
use perasc_check::ty::{FloatTy, IntTy, Ty as CheckTy};
use perasc_diag::{Diagnostic, Fix, Label, Message, Span, codes};
use perasc_syntax::ast::{
    AssignOp, Ast, BinaryOp, Convention, ExprId, ExprKind as AstExpr, FnDecl, ItemKind, StepOp,
    Stmt as AstStmt, StmtKind, Symbol, Type, UnaryOp,
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
    lower_with(ast, analysis, false)
}

/// [`lower`], and, with `overflow_checks`, `+`, `-` and `*` on integers stop
/// the program when the result does not fit, as a debug build does
/// ([decision 0011]); without, they wrap, as a production build does. A loop
/// counter `i++` under `i < bound`, which the body never assigns, is not
/// checked: `i + 1` is at most `bound`.
///
/// [decision 0011]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0011-what-to-build-next.md
///
/// # Errors
///
/// As [`lower`].
pub fn lower_with(
    ast: &Ast,
    analysis: &Analysis,
    overflow_checks: bool,
) -> Result<Module, Vec<Diagnostic>> {
    let unit = Unit {
        ast,
        analysis,
        imports: HashMap::new(),
    };
    lower_units(&[unit], overflow_checks)
        .map_err(|errors| errors.into_iter().flat_map(|(_, d)| d).collect())
}

/// One owned file of a build, checked, and the files its imports name: each
/// `from` as written, and the index of that file among the units
/// ([decision 0022]).
///
/// [decision 0022]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0022-owned-modules.md
pub struct Unit<'a> {
    /// Its syntax tree.
    pub ast: &'a Ast,
    /// Its checks.
    pub analysis: &'a Analysis,
    /// The unit each of its imports names, by the path the import writes.
    pub imports: HashMap<String, usize>,
}

/// The names of one unit, which only its own functions see.
#[derive(Default, Clone)]
struct Names {
    fns: HashMap<Symbol, FnId>,
    by_index: HashMap<usize, FnId>,
    constants: HashMap<Symbol, ExprId>,
    generic: HashMap<Symbol, usize>,
    instances: HashMap<(usize, Vec<CheckTy>), FnId>,
    /// Its exported functions that are not generic, as other units import
    /// them.
    exported: HashMap<String, FnId>,
}

/// Lowers the files of a build into one module ([decision 0022]): each file
/// imported before the files that import it, the last the root, whose
/// exported functions alone JavaScript calls. Every function of every file is
/// in the module, under its own number; a file's private names are its own.
///
/// [decision 0022]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0022-owned-modules.md
///
/// # Errors
///
/// The `E0900` diagnostics of each unit that has some, by its index.
pub fn lower_units(
    units: &[Unit<'_>],
    overflow_checks: bool,
) -> Result<Module, Vec<(usize, Vec<Diagnostic>)>> {
    let Some(first) = units.first() else {
        return Ok(Module {
            functions: Vec::new(),
            data: Vec::new(),
        });
    };
    let root = units.len() - 1;
    let mut lowering = Lowering {
        overflow_checks,
        ast: first.ast,
        analysis: first.analysis,
        root: true,
        fns: HashMap::new(),
        by_index: HashMap::new(),
        constants: HashMap::new(),
        generic: HashMap::new(),
        instances: HashMap::new(),
        args: Vec::new(),
        results: HashMap::new(),
        conventions: HashMap::new(),
        data: Vec::new(),
        statics: HashMap::new(),
        static_top: crate::STATIC_START,
        diagnostics: Vec::new(),
        current: String::new(),
        body_span: Span::new(0, 0),
        jobs: Vec::new(),
        job_base: 0,
    };
    let mut tables: Vec<Names> = Vec::with_capacity(units.len());
    let mut errors: Vec<Vec<Diagnostic>> = vec![Vec::new(); units.len()];
    // Every function of every unit, numbered in order: each unit's own, then
    // the instances of its generic ones.
    let mut compiled = Vec::new();
    for (u, unit) in units.iter().enumerate() {
        let table = lowering.register(u, unit, u == root, &tables, &mut compiled);
        if let Some(e) = errors.get_mut(u) {
            e.append(&mut lowering.diagnostics);
        }
        tables.push(table);
    }
    // The jobs of `parallel for` loops come after every other function.
    lowering.job_base = u32::try_from(compiled.len()).unwrap_or(u32::MAX);
    let functions = lowering.bodies(units, &tables, &compiled, &mut errors);
    let failed: Vec<(usize, Vec<Diagnostic>)> = errors
        .into_iter()
        .enumerate()
        .filter(|(_, d)| !d.is_empty())
        .map(|(u, mut d)| {
            d.sort_by_key(|d| d.primary_span().map(|s| s.start));
            (u, d)
        })
        .collect();
    if failed.is_empty() {
        Ok(Module {
            functions,
            data: lowering.data,
        })
    } else {
        Err(failed)
    }
}

/// A function to compile: its unit, its declaration, its type arguments.
type Compiled<'a> = (usize, &'a FnDecl, Vec<CheckTy>);

impl<'a> Lowering<'a> {
    /// Numbers the functions of unit `u` after those already in `compiled`,
    /// gives each its conventions and results, and names what the unit
    /// imports from the units before it, whose names are `tables`.
    fn register(
        &mut self,
        u: usize,
        unit: &Unit<'a>,
        root: bool,
        tables: &[Names],
        compiled: &mut Vec<Compiled<'a>>,
    ) -> Names {
        let mut table = Names::default();
        let decls: Vec<&FnDecl> = unit.ast.functions();
        let start = compiled.len();
        for (index, decl) in decls.iter().enumerate() {
            let checked = perasc_check::ty::FnId(index);
            // A method is reached through its type, never by its name alone.
            let by_name = decl.impl_of.is_none();
            if unit.analysis.is_generic(checked) {
                if by_name {
                    table.generic.insert(decl.name.symbol, index);
                }
                continue;
            }
            let id = FnId(u32::try_from(compiled.len()).unwrap_or(u32::MAX));
            if by_name {
                table.fns.insert(decl.name.symbol, id);
                if decl.exported {
                    table
                        .exported
                        .insert(unit.ast.name(decl.name.symbol).to_owned(), id);
                }
            }
            table.by_index.insert(index, id);
            compiled.push((u, decl, Vec::new()));
        }
        for (checked, args) in &unit.analysis.instances {
            let Some(&decl) = decls.get(checked.0) else {
                continue;
            };
            let id = FnId(u32::try_from(compiled.len()).unwrap_or(u32::MAX));
            table.instances.insert((checked.0, args.clone()), id);
            compiled.push((u, decl, args.clone()));
        }
        // The functions it imports, under the names it imports them by.
        for item in &unit.ast.items {
            let ItemKind::Import(decl) = &item.kind else {
                continue;
            };
            let Some(from) = unit.imports.get(&decl.from).and_then(|&i| tables.get(i)) else {
                continue;
            };
            for name in &decl.names {
                if let Some(&id) = from.exported.get(unit.ast.name(name.symbol)) {
                    table.fns.insert(name.symbol, id);
                }
            }
        }
        self.enter(unit, &table, root);
        for (index, (_, decl, args)) in compiled.iter().enumerate().skip(start) {
            let id = FnId(u32::try_from(index).unwrap_or(u32::MAX));
            self.args.clone_from(args);
            self.conventions
                .insert(id, decl.params.iter().map(|p| p.convention).collect());
            // Its results, for its callers; a type not compiled yet is
            // reported once, with the function itself.
            let mut results: Vec<Ty> = decl
                .ret
                .as_ref()
                .and_then(|t| self.resolve(t).ok())
                .and_then(|t| self.leaves(&t).ok())
                .map(|leaves| leaves.into_iter().map(|(_, ty)| ty).collect())
                .unwrap_or_default();
            results.extend(self.inout_numbers(decl));
            self.results.insert(id, results);
        }
        for item in &unit.ast.items {
            if let ItemKind::Global(local) = &item.kind {
                match local.init {
                    // A constant is written where it is used.
                    Some(init) if !local.mutable => {
                        self.constants.insert(local.name.symbol, init);
                    }
                    _ => self.unsupported(local.name.span, "global"),
                }
            }
        }
        table.constants = std::mem::take(&mut self.constants);
        table
    }

    /// The bodies of the functions `compiled` numbers, then the jobs of
    /// their `parallel for` loops. Another unit's function is JavaScript's
    /// to call only in the root, and has a name of its own in the module
    /// (decision 0022).
    fn bodies(
        &mut self,
        units: &[Unit<'a>],
        tables: &[Names],
        compiled: &[Compiled<'a>],
        errors: &mut [Vec<Diagnostic>],
    ) -> Vec<Function> {
        let root = units.len().saturating_sub(1);
        let mut functions: Vec<Function> = Vec::with_capacity(compiled.len());
        let mut current = usize::MAX;
        let mut job_units: Vec<usize> = Vec::new();
        for (u, decl, args) in compiled {
            if *u != current {
                current = *u;
                if let (Some(unit), Some(table)) = (units.get(current), tables.get(current)) {
                    self.enter(unit, table, current == root);
                }
            }
            self.args.clone_from(args);
            let jobs = self.jobs.len();
            let mut function = self.function(decl);
            job_units.extend(std::iter::repeat_n(*u, self.jobs.len() - jobs));
            if *u != root {
                function.exported = false;
                function.shape = None;
                function.name = format!("{}__m{u}", function.name);
            }
            functions.push(function);
            if let Some(e) = errors.get_mut(*u) {
                e.append(&mut self.diagnostics);
            }
        }
        let mut jobs = std::mem::take(&mut self.jobs);
        for (job, &u) in jobs.iter_mut().zip(&job_units) {
            if u != root {
                job.name = format!("{}__m{u}", job.name);
            }
        }
        functions.append(&mut jobs);
        functions
    }

    /// Makes `unit` the one being lowered, with its names.
    fn enter(&mut self, unit: &Unit<'a>, names: &Names, root: bool) {
        self.ast = unit.ast;
        self.analysis = unit.analysis;
        self.root = root;
        self.fns.clone_from(&names.fns);
        self.by_index.clone_from(&names.by_index);
        self.constants.clone_from(&names.constants);
        self.generic.clone_from(&names.generic);
        self.instances.clone_from(&names.instances);
    }
}

struct Lowering<'a> {
    /// `+`, `-` and `*` on integers stop the program on an overflow.
    overflow_checks: bool,
    /// The unit being lowered is the root of the build: its exported
    /// functions are JavaScript's (decision 0022).
    root: bool,
    ast: &'a Ast,
    analysis: &'a Analysis,
    /// The name of the function being lowered, which its jobs carry.
    current: String,
    /// The span of its body.
    body_span: Span,
    /// The jobs of the `parallel for` loops lowered so far.
    jobs: Vec<Function>,
    /// The id of the first job: the number of other functions.
    job_base: u32,
    /// The functions that are not generic, by name.
    fns: HashMap<Symbol, FnId>,
    /// The functions and methods that are not generic, by their index in
    /// the checker's table.
    by_index: HashMap<usize, FnId>,
    /// The module constants: their value, lowered where they are used.
    constants: HashMap<Symbol, ExprId>,
    /// The generic functions, by name: their index in the checker's table.
    generic: HashMap<Symbol, usize>,
    /// The instance compiled for each generic function and type arguments.
    instances: HashMap<(usize, Vec<CheckTy>), FnId>,
    /// The type arguments of the function being lowered: empty unless it is
    /// an instance of a generic one.
    args: Vec<CheckTy>,
    /// The scalar results of each function: several for a structure.
    results: HashMap<FnId, Vec<Ty>>,
    /// How each function takes its parameters.
    conventions: HashMap<FnId, Vec<Convention>>,
    /// The bytes of the string literals, each once.
    data: Vec<Vec<u8>>,
    /// The literals only read, placed below the heap: each one's header
    /// address, by its index in `data` ([`crate::BufOp::Static`]).
    statics: HashMap<usize, u32>,
    /// The first free address below the heap for the next one.
    static_top: u32,
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
    /// The temporaries that hold an array of views ([decision 0004]), with
    /// the local their text comes from, if it comes from one.
    ///
    /// [decision 0004]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0004-strings-without-allocation.md
    views: HashMap<LocalId, Option<Symbol>>,
    /// The text whose `split`, lowered next, may give views: set by the
    /// consumer that reads the array at once.
    views_for: Option<ExprId>,
    /// `const f = t.split(…)` is being lowered with `t` unable to change and
    /// `f` only read: its `split` gives views it owns ([`Lowering::local`]).
    views_owned: bool,
    /// The locals that cannot change while they live: parameters lent, not
    /// `inout`, and `const` variables.
    fixed: std::collections::HashSet<LocalId>,
    /// The array whose `filter`, lowered next, only counts the elements it
    /// keeps: set by `.length` read on it at once ([decision 0004], fusion).
    ///
    /// [decision 0004]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0004-strings-without-allocation.md
    counted: Option<ExprId>,
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
            views: HashMap::new(),
            views_for: None,
            views_owned: false,
            fixed: std::collections::HashSet::new(),
            counted: None,
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
        let found = self
            .resolve(ty)
            .and_then(|t| Self::ir_type(&t).ok_or_else(|| feature_of_check_type(&t)));
        found.unwrap_or_else(|feature| {
            self.unsupported(ty.span, feature);
            Ty::Void
        })
    }

    /// The type the checker gave an expression.
    fn type_of(&mut self, id: ExprId) -> Ty {
        let span = self.ast.expr(id).span;
        match self.check_type(id) {
            Some(ty) => Self::ir_type(&ty).unwrap_or_else(|| {
                self.unsupported(span, feature_of_check_type(&ty));
                Ty::Void
            }),
            None => Ty::Void,
        }
    }

    /// A name that is not a local: a module constant, or `E0900` for a
    /// function used as a value or a module variable.
    fn not_a_local(&mut self, state: &mut FnState, symbol: Symbol, span: Span, ty: Ty) -> Expr {
        // A constant: its value, lowered here.
        if let Some(&init) = self.constants.get(&symbol) {
            return self.expr(state, init);
        }
        let function = self.fns.contains_key(&symbol) || self.generic.contains_key(&symbol);
        let feature = if function { "function-value" } else { "global" };
        self.unsupported(span, feature);
        zero(ty)
    }

    /// The function a call by name reaches: the function itself, or the
    /// instance of a generic one for the type arguments of this call.
    fn callee(&self, call: ExprId, symbol: Symbol) -> Option<FnId> {
        if let Some((checked, args)) = self.analysis.calls.get(&call) {
            let args: Vec<CheckTy> = args
                .iter()
                .map(|t| self.analysis.subst(t, &self.args))
                .collect();
            return self.instances.get(&(checked.0, args)).copied();
        }
        self.fns.get(&symbol).copied()
    }

    /// For a method call, the function it reaches and whether the value
    /// before the dot is its first argument.
    pub(super) fn method_callee(&self, call: ExprId) -> Option<(FnId, bool)> {
        match self.analysis.method_calls.get(&call)? {
            perasc_check::MethodCall::Fn { id, receiver } => {
                let target = match self.analysis.calls.get(&call) {
                    Some((checked, args)) => {
                        let args: Vec<CheckTy> = args
                            .iter()
                            .map(|t| self.analysis.subst(t, &self.args))
                            .collect();
                        self.instances.get(&(checked.0, args)).copied()
                    }
                    None => self.by_index.get(&id.0).copied(),
                }?;
                Some((target, *receiver))
            }
            perasc_check::MethodCall::Interface => {
                let (id, args) = self.analysis.dyn_targets.get(&(call, self.args.clone()))?;
                let target = if args.is_empty() {
                    self.by_index.get(&id.0).copied()
                } else {
                    self.instances.get(&(id.0, args.clone())).copied()
                }?;
                Some((target, true))
            }
        }
    }

    /// The arguments of a method call: the receiver first when it has one.
    pub(super) fn method_arguments(base: ExprId, receiver: bool, args: &[ExprId]) -> Vec<ExprId> {
        let mut all = Vec::with_capacity(args.len() + 1);
        if receiver {
            all.push(base);
        }
        all.extend_from_slice(args);
        all
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
                if inout && decl.exported && self.root && self.holds_string(&ty) {
                    self.unsupported(param.name.span, "inout-string");
                }
                if inout
                    && decl.exported
                    && self.root
                    && matches!(ty, CheckTy::Option(_) | CheckTy::Enum(..))
                {
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
                if param.convention == Convention::Let {
                    state.fixed.extend(ids.iter().copied());
                }
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
                if param.convention == Convention::Inout && decl.exported && self.root {
                    self.unsupported(param.name.span, "inout-export");
                }
                let leaf = leaves.first().map_or(Ty::Void, |(_, t)| *t);
                let id = state.add(param.name.symbol, name, leaf);
                if param.convention == Convention::Let {
                    state.fixed.insert(id);
                }
                if param.convention == Convention::Inout {
                    state.writeback.push(id);
                }
            }
            Err(feature) => self.unsupported(param.ty.span, feature),
        }
    }

    fn function(&mut self, decl: &FnDecl) -> Function {
        self.current = self.function_name(decl);
        self.body_span = decl.body.span;
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
        if matches!(ret_ty, Some(CheckTy::Option(_) | CheckTy::Result(..))) {
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
        let shape = (decl.exported && self.root && has_struct).then(|| JsShape {
            params: shapes,
            result: ret_ty
                .filter(|t| *t != CheckTy::Void)
                .map(|t| self.shape(&t)),
            writeback,
        });
        Function {
            name: self.function_name(decl),
            exported: decl.exported && self.root,
            job: false,
            param_count,
            locals: state.locals,
            ret: results.first().copied().unwrap_or(Ty::Void),
            more: results.get(1..).map(<[Ty]>::to_vec).unwrap_or_default(),
            shape,
            body,
        }
    }

    /// The name of the function being lowered: an instance of a generic
    /// function is named after its type arguments, `larger__i64`.
    fn function_name(&self, decl: &FnDecl) -> String {
        let name = match decl.impl_of {
            Some(owner) => format!(
                "{}_{}",
                self.ast.name(owner.symbol),
                self.ast.name(decl.name.symbol)
            ),
            None => self.ast.name(decl.name.symbol).to_owned(),
        };
        if self.args.is_empty() {
            return name;
        }
        let args: Vec<String> = self
            .args
            .iter()
            .map(|t| identifier(&t.to_string()))
            .collect();
        format!("{name}__{}", args.join("_"))
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

    /// `for (init; cond; update) body`, or `parallel for`.
    fn for_loop(&mut self, state: &mut FnState, stmt: &AstStmt, out: &mut Vec<Stmt>) {
        let StmtKind::For {
            parallel,
            init,
            cond,
            update,
            body,
        } = &stmt.kind
        else {
            return;
        };
        if *parallel {
            let head = parallel::Head {
                init: init.as_deref(),
                cond: *cond,
                update: update.as_deref(),
            };
            self.parallel_for(state, stmt.span, head, body, out);
            return;
        }
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
        let counter = self.counter(state, cond.as_ref(), update.as_deref());
        let mut lowered = self.loop_body(state, cond, before, body, step, &mut Vec::new());
        if let Some(i) = counter {
            unchecked_counter(&mut lowered, i);
        }
        out.extend(lowered);
        state.close(out);
    }

    /// `a op b`; with overflow checks, `+`, `-` and `*` on integers computed
    /// as `checkedAdd` and the others are, the program stopped when the
    /// result does not fit.
    fn arith(&mut self, state: &mut FnState, op: BinOp, a: Expr, b: Expr) -> Expr {
        if !self.overflow_checks
            || !matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul)
            || !matches!(a.ty, Ty::I32 | Ty::I64)
        {
            return binary(op, a, b);
        }
        let mut parts = values::checked(state, op, a, b).into_iter();
        let (Some(fits), Some(value)) = (parts.next(), parts.next()) else {
            return zero(Ty::I32);
        };
        state
            .pre
            .push(Stmt::Buf(crate::BufStmt::CheckOverflow, vec![fits]));
        value
    }

    /// The local of a loop counter whose step cannot overflow: the condition
    /// is `i < bound` and the step `i++`, on the same integer variable.
    fn counter(
        &mut self,
        state: &mut FnState,
        cond: Option<&Expr>,
        update: Option<&AstStmt>,
    ) -> Option<LocalId> {
        if !self.overflow_checks {
            return None;
        }
        let StmtKind::Step {
            target,
            op: StepOp::Increment,
        } = &update?.kind
        else {
            return None;
        };
        if !matches!(self.ast.expr(*target).kind, AstExpr::Name(_)) {
            return None;
        }
        let locals = self.place(state, *target)?;
        let [local] = locals.as_slice() else {
            return None;
        };
        let local = *local;
        let ExprKind::Binary(BinOp::Lt, i, _) = &cond?.kind else {
            return None;
        };
        (i.kind == ExprKind::Local(local) && matches!(i.ty, Ty::I32 | Ty::I64)).then_some(local)
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
                let value = self.arith(state, op, current, one);
                state.flush(out);
                out.push(Stmt::Set(local, value));
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
            StmtKind::For { .. } => self.for_loop(state, stmt, out),
            StmtKind::ForOf { parallel: true, .. } => {
                self.refuse_shape(state, stmt.span);
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
            (None, Some(init)) => self.check_type(init),
            (None, None) => None,
        };
        // `const f = t.split(",")`, `t` unable to change and `f` only read:
        // `f` owns an array of views of `t`, not a copy of every part
        // (decision 0014, finding F201).
        if !local.mutable
            && local
                .init
                .is_some_and(|init| self.split_views_local(state, local.name.symbol, init))
        {
            state.views_for = local.init.and_then(|init| self.split_source(init));
            state.views_owned = true;
        }
        if let Some(ty) = ty.filter(values::is_aggregate) {
            let leaves = match self.leaves(&ty) {
                Ok(leaves) => leaves,
                // The reason itself: a type that contains itself, an element
                // not compiled yet…
                Err(feature) => {
                    self.unsupported(local.name.span, feature);
                    return;
                }
            };
            let values = match local.init {
                Some(init) => self.value(state, init),
                None => leaves.iter().map(|(_, t)| zero(*t)).collect(),
            };
            state.views_for = None;
            state.views_owned = false;
            state.flush(out);
            let ids = state.add_struct(local.name.symbol, name, &leaves);
            if !local.mutable {
                state.fixed.extend(ids.iter().copied());
            }
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
        state.views_for = None;
        state.views_owned = false;
        state.flush(out);
        let id = state.add(local.name.symbol, name, ty);
        if !local.mutable {
            state.fixed.insert(id);
        }
        out.push(Stmt::Set(id, value));
        state.end_statement(out);
    }

    /// Whether `const name = init` may hold views: `init` is `t.split(…)`
    /// with `t` a variable that cannot change while `name` lives, and every
    /// use of `name` in the function is `name.length` or `name[i]`, which the
    /// checker lets neither change the array nor move an element out of it.
    fn split_views_local(&self, state: &FnState, name: Symbol, init: ExprId) -> bool {
        let Some(text) = self.split_source(init) else {
            return false;
        };
        let fixed = self
            .root(text)
            .is_some_and(|root| match state.lookup(root) {
                Some(Binding::Lent(_)) => true,
                Some(Binding::Scalar(id)) => state.fixed.contains(&id),
                Some(Binding::Struct(ids)) => ids.iter().all(|id| state.fixed.contains(id)),
                None => false,
            });
        if !fixed {
            return false;
        }
        let body = self.body_span;
        let inside = |span: Span| body.start <= span.start && span.end <= body.end;
        let is_name = |id: ExprId| matches!(self.ast.expr(id).kind, AstExpr::Name(s) if s == name);
        let uses = self
            .ast
            .exprs
            .iter()
            .filter(|e| inside(e.span) && matches!(e.kind, AstExpr::Name(s) if s == name))
            .count();
        let reads = self
            .ast
            .exprs
            .iter()
            .filter(|e| inside(e.span))
            .filter(|e| match &e.kind {
                AstExpr::Field { base, name: field } => {
                    is_name(*base) && self.ast.name(field.symbol) == "length"
                }
                AstExpr::Index { base, .. } => is_name(*base),
                _ => false,
            })
            .count();
        uses > 0 && uses == reads
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
                let current = local_expr(state, *local);
                let value = self.arith(state, op, current, value.clone());
                state.flush(out);
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
                let base_ty = self.check_type(*base)?;
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
            AstExpr::Name(symbol) => match state.lookup(*symbol) {
                Some(Binding::Scalar(local)) => ExprKind::Local(local),
                _ => return self.not_a_local(state, *symbol, span, ty),
            },
            AstExpr::Paren(inner) => return self.expr(state, *inner),
            AstExpr::Unary { op, operand } => {
                if let (UnaryOp::Neg, AstExpr::Int(value)) = (op, &self.ast.expr(*operand).kind) {
                    ExprKind::Const(int_const(*value, true))
                } else {
                    let op = match op {
                        UnaryOp::Neg => UnOp::Neg,
                        UnaryOp::Not => UnOp::Not,
                        UnaryOp::BitNot => UnOp::BitNot,
                        // `await` is refused in an owned module by the checker.
                        UnaryOp::Borrow | UnaryOp::Await => {
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
                    return float_rem(lhs, rhs);
                }
                return self.arith(state, bin_op(*op), lhs, rhs);
            }
            AstExpr::Call { callee, args, .. } => {
                return self.call(state, id, *callee, args, ty, span);
            }
            AstExpr::String(_) | AstExpr::Template { .. } => {
                return self.not_yet(span, "string", ty);
            }
            AstExpr::Array(_) => return self.not_yet(span, "array", ty),
            AstExpr::Tuple(_) => return self.not_yet(span, "struct", ty),
            AstExpr::Lambda { .. } => return self.not_yet(span, "function-value", ty),
            AstExpr::Field { base, .. } if self.is_buffer(*base) || self.is_map(*base) => {
                return self.array_length(state, *base);
            }
            AstExpr::Field { base, name } => {
                if !self.is_aggregate(*base) {
                    return self.not_yet(span, "field", ty);
                }
                let fields = self.value(state, *base);
                let base_ty = self.check_type(*base);
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
        let outer_drops = std::mem::take(&mut state.drops);
        let live = state.live.clone();
        let rhs = self.expr(state, rhs);
        // The right side's temporaries, as `s.endsWith("1")` makes: its value
        // first, then they are released in its branch, the only path that
        // made them.
        let rhs = if state.drops.is_empty() {
            rhs
        } else {
            let value = state.temp(Ty::Bool);
            state.pre.push(Stmt::Set(value, rhs));
            let mut released = Vec::new();
            state.end_statement(&mut released);
            state.pre.extend(released);
            local_expr(state, value)
        };
        state.drops = outer_drops;
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
        call: ExprId,
        callee: ExprId,
        args: &[ExprId],
        ty: Ty,
        span: Span,
    ) -> Expr {
        match &self.ast.expr(callee).kind {
            AstExpr::Name(symbol) => {
                let Some(id) = self.callee(call, *symbol) else {
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
            AstExpr::Field { base, .. } if self.method_callee(call).is_some() => {
                let (id, receiver) = self.method_callee(call).unwrap_or((FnId(0), false));
                let all = Self::method_arguments(*base, receiver, args);
                let back = self.writeback_places(state, id, &all);
                let args = self.arguments(state, Some(id), &all);
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
        if let Some(chosen) = self.ordering(state, receiver.clone(), method, args) {
            return chosen;
        }
        let math = match method {
            "abs" => Some(MathOp::Abs),
            "sqrt" => Some(MathOp::Sqrt),
            "floor" => Some(MathOp::Floor),
            "ceil" => Some(MathOp::Ceil),
            "exp" => Some(MathOp::Exp),
            "log" => Some(MathOp::Log),
            "sin" => Some(MathOp::Sin),
            "cos" => Some(MathOp::Cos),
            "tan" => Some(MathOp::Tan),
            "round" => Some(MathOp::Round),
            _ => None,
        };
        if let (Some(op), Some(&arg)) = (
            match method {
                "pow" => Some(BinOp::Pow),
                "atan2" => Some(BinOp::Atan2),
                "hypot" => Some(BinOp::Add),
                _ => None,
            },
            args.first(),
        ) {
            let mark = state.pre.len();
            let other = self.expr(state, arg);
            let mut operands = [receiver];
            state.keep_before(mark, &mut operands);
            let [receiver] = operands;
            if method != "hypot" {
                return binary(op, receiver, other);
            }
            // `sqrt(a * a + b * b)`, each operand computed once.
            let (a, b) = (state.temp(ty), state.temp(ty));
            state.pre.push(Stmt::Set(a, receiver));
            state.pre.push(Stmt::Set(b, other));
            let (a, b) = (local_expr(state, a), local_expr(state, b));
            let sum = binary(
                BinOp::Add,
                binary(BinOp::Mul, a.clone(), a),
                binary(BinOp::Mul, b.clone(), b),
            );
            return Expr {
                kind: ExprKind::Math(MathOp::Sqrt, Box::new(sum)),
                ty,
            };
        }
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
/// A name made of letters, digits and `_`: `Pair<i32, f64>` gives
/// `Pair_i32_f64`, for the names of instances in the backends.
pub(crate) fn identifier(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_owned()
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

/// `x % y` on floats: a function of the runtime, exact as JavaScript's `%`;
/// an `f32` computed as an `f64`, whose remainder an `f32` holds exactly
/// (decision 0021, finding F304).
fn float_rem(x: Expr, y: Expr) -> Expr {
    let ty = x.ty;
    let wide = |e: Expr| {
        if e.ty == Ty::F64 {
            e
        } else {
            Expr {
                kind: ExprKind::Convert(Box::new(e)),
                ty: Ty::F64,
            }
        }
    };
    let rem = Expr {
        kind: ExprKind::Buf(
            crate::BufOp::Runtime(crate::RuntimeFn::FloatRem),
            vec![wide(x), wide(y)],
        ),
        ty: Ty::F64,
    };
    if ty == Ty::F64 {
        rem
    } else {
        Expr {
            kind: ExprKind::Convert(Box::new(rem)),
            ty,
        }
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

/// In the loop just lowered, a step `i++` checked for overflow becomes a plain
/// `i = i + 1` when the body never assigns `i`: the condition `i < bound`,
/// checked before, keeps `i + 1` at most `bound`.
fn unchecked_counter(lowered: &mut [Stmt], i: LocalId) {
    let Some(Stmt::Loop { body, step, .. }) = lowered.last_mut() else {
        return;
    };
    if crate::bounds::assigns(body, i) {
        return;
    }
    let checked = step
        .iter()
        .any(|s| matches!(s, Stmt::Buf(crate::BufStmt::CheckOverflow, _)));
    if !checked {
        return;
    }
    let Some(ty) = step.iter().find_map(|s| match s {
        Stmt::Set(l, e) if *l == i => Some(e.ty),
        _ => None,
    }) else {
        return;
    };
    let current = Expr {
        kind: ExprKind::Local(i),
        ty,
    };
    *step = vec![Stmt::Set(i, binary(BinOp::Add, current, one(ty)))];
}
