//! Statements, and `match`.

use perasc_check::ty::{IntTy, Ty};
use perasc_syntax::ast::{
    Arm, ArmBody, AssignOp, BinaryOp, ExprId, ExprKind, Pattern, PatternKind, StepOp, Stmt,
    StmtKind,
};

use crate::expr::string_literal;
use crate::{Module, NameKind};

/// A place a value is written to.
enum Place {
    /// A variable, or a field: written as it reads.
    Plain(String),
    /// An element of an array, its bounds checked.
    Element(String, String),
}

impl Module<'_> {
    pub(crate) fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Local(local) => {
                let value = local.init.map(|init| self.expr(init));
                let name = self.declare(local.name.span, self.ast.name(local.name.symbol));
                let keyword = if local.mutable { "let" } else { "const" };
                match value {
                    Some(value) => self.line(format!("{keyword} {name} = {value};")),
                    None => self.line(format!("let {name};")),
                }
            }
            StmtKind::Return(value) => match value {
                Some(value) => {
                    let value = self.expr(*value);
                    self.line(format!("return {value};"));
                }
                None => self.line("return;"),
            },
            StmtKind::Expr(id) => self.effect(*id),
            StmtKind::Assign { target, op, value } => {
                if self.replace_resource(*target, *value) {
                    return;
                }
                let text = self.assignment(*target, *op, *value);
                self.line(format!("{text};"));
            }
            StmtKind::Step { target, op } => {
                let text = self.step(*target, *op);
                self.line(format!("{text};"));
            }
            StmtKind::Block(block) => {
                self.line("{");
                self.current.depth += 1;
                self.block_stmts(block);
                self.current.depth -= 1;
                self.line("}");
            }
            StmtKind::If {
                cond,
                then,
                otherwise,
            } => self.if_(*cond, then, otherwise.as_deref()),
            StmtKind::While { cond, body } => {
                let (test, lines) = self.apart(|m| m.expr(*cond));
                if lines.is_empty() {
                    self.line(format!("while ({test}) {{"));
                } else {
                    self.line("while (true) {");
                    self.current.lines.extend(lines);
                    self.inner(&format!("if (!{test}) break;"));
                }
                self.body(body);
                self.line("}");
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
                ..
            } => self.for_(stmt, init.as_deref(), *cond, update.as_deref(), body),
            StmtKind::ForOf {
                mutable,
                name,
                iterable,
                body,
                ..
            } => {
                let items = self.expr(*iterable);
                let name = self.declare(name.span, self.ast.name(name.symbol));
                let keyword = if *mutable { "let" } else { "const" };
                self.line(format!("for ({keyword} {name} of {items}) {{"));
                self.body(body);
                self.line("}");
            }
            StmtKind::Break => self.line("break;"),
            StmtKind::Continue => self.line("continue;"),
            StmtKind::Error => self.unsupported(stmt.span, "other"),
        }
    }

    /// A line one level deeper than the current one.
    fn inner(&mut self, text: &str) {
        self.current.depth += 1;
        self.line(text);
        self.current.depth -= 1;
    }

    /// The statements of a loop's or a branch's body, one level deeper.
    fn body(&mut self, body: &Stmt) {
        self.current.depth += 1;
        match &body.kind {
            StmtKind::Block(block) => self.block_stmts(block),
            _ => self.stmt(body),
        }
        self.current.depth -= 1;
    }

    /// An expression for its effect: a call, or a `match` whose arms are
    /// statements.
    fn effect(&mut self, id: ExprId) {
        if let ExprKind::Match { scrutinee, arms } = &self.ast.expr(id).kind {
            self.match_(*scrutinee, arms, None);
            return;
        }
        let value = self.expr(id);
        self.line(format!("{value};"));
    }

    fn if_(&mut self, cond: ExprId, then: &Stmt, otherwise: Option<&Stmt>) {
        let test = self.expr(cond);
        self.line(format!("if ({test}) {{"));
        self.body(then);
        let mut otherwise = otherwise;
        while let Some(next) = otherwise {
            if let StmtKind::If {
                cond,
                then,
                otherwise: further,
            } = &next.kind
            {
                let (test, lines) = self.apart(|m| m.expr(*cond));
                if lines.is_empty() {
                    self.line(format!("}} else if ({test}) {{"));
                    self.body(then);
                    otherwise = further.as_deref();
                    continue;
                }
                // Its condition needs statements: in the `else`, before it.
                self.line("} else {");
                self.current.lines.extend(lines);
                self.current.depth += 1;
                self.line(format!("if ({test}) {{"));
                self.body(then);
                if let Some(further) = further.as_deref() {
                    self.line("} else {");
                    self.body(further);
                }
                self.line("}");
                self.current.depth -= 1;
                self.line("}");
                return;
            }
            self.line("} else {");
            self.body(next);
            otherwise = None;
        }
        self.line("}");
    }

    fn for_(
        &mut self,
        stmt: &Stmt,
        init: Option<&Stmt>,
        cond: Option<ExprId>,
        update: Option<&Stmt>,
        body: &Stmt,
    ) {
        let proof = self.loop_proof(init, cond, update, body);
        // The loop has a scope of its own, for its counter.
        self.line("{");
        self.current.depth += 1;
        if let Some(init) = init {
            self.stmt(init);
        }
        let (step, step_lines) = match update {
            Some(update) => self.apart(|m| match &update.kind {
                StmtKind::Step { target, op } => Some(m.step(*target, *op)),
                StmtKind::Assign { target, op, value } => Some(m.assignment(*target, *op, *value)),
                _ => None,
            }),
            None => (Some(String::new()), Vec::new()),
        };
        let step = match step {
            Some(step) if step_lines.is_empty() => step,
            _ => {
                self.unsupported(stmt.span, "other");
                String::new()
            }
        };
        let (test, lines) = match cond {
            Some(cond) => self.apart(|m| m.expr(cond)),
            None => (String::new(), Vec::new()),
        };
        if lines.is_empty() {
            self.line(format!("for (; {test}; {step}) {{"));
        } else {
            self.line(format!("for (; ; {step}) {{"));
            self.current.lines.extend(lines);
            self.inner(&format!("if (!{test}) break;"));
        }
        self.current.proofs.extend(proof);
        self.body(body);
        if proof.is_some() {
            self.current.proofs.pop();
        }
        self.line("}");
        self.current.depth -= 1;
        self.line("}");
    }

    // --- Assignments ----------------------------------------------------------

    /// Where `target` is, each part of it computed once.
    fn place(&mut self, target: ExprId) -> Place {
        match &self.ast.expr(target).kind {
            ExprKind::Index { base, index } => {
                let proven = self.proven(*base, *index);
                let values = self.exprs(&[*base, *index]);
                let base = self.keep(*base, values[0].clone());
                let index = self.keep(*index, values[1].clone());
                if proven {
                    Place::Plain(format!("{base}[{index}]"))
                } else {
                    Place::Element(base, index)
                }
            }
            ExprKind::Field { base, name } => {
                let ty = self.ty(*base);
                let value = self.expr(*base);
                let value = self.keep(*base, value);
                let text = self.ast.name(name.symbol);
                if self.is_tuple(&ty) {
                    Place::Plain(format!("{value}[{text}]"))
                } else {
                    Place::Plain(format!("{value}.{text}"))
                }
            }
            ExprKind::Paren(inner) => self.place(*inner),
            _ => Place::Plain(self.expr(target)),
        }
    }

    /// A part of a place, computed once: a temporary, unless it is a local
    /// variable, which only an assignment of it can change.
    fn keep(&mut self, id: ExprId, js: String) -> String {
        let local = matches!(self.ast.expr(id).kind, ExprKind::Name(_))
            && matches!(self.bound(id), Some((_, perasc_check::NameKind::Local)));
        if local { js } else { self.spill(js) }
    }

    fn read(&mut self, place: &Place) -> String {
        match place {
            Place::Plain(text) => text.clone(),
            Place::Element(base, index) => {
                let at = self.helper("$at");
                format!("{at}({base}, {index})")
            }
        }
    }

    fn write(&mut self, place: &Place, value: &str) -> String {
        match place {
            Place::Plain(text) => format!("{text} = {value}"),
            Place::Element(base, index) => {
                let put = self.helper("$put");
                format!("{put}({base}, {index}, {value})")
            }
        }
    }

    /// `target = value`, or `target op= value`, as an expression.
    /// `timer = setTimeout(…)`: the new resource is made first, then the one
    /// the variable held is released, if it still holds it.
    fn replace_resource(&mut self, target: ExprId, value: ExprId) -> bool {
        let Some((binding, NameKind::Local)) = self.bound(target) else {
            return false;
        };
        let owns = self
            .analysis
            .bindings
            .get(binding)
            .is_some_and(|b| perasc_check::browser::is_resource(&b.ty));
        if !owns {
            return false;
        }
        let name = self.variable(binding);
        let value = self.expr(value);
        let t = self.temp();
        let release = self.helper("$release");
        self.line(format!("const {t} = {value};"));
        self.line(format!("if ({name}$live) {release}({name});"));
        self.line(format!("{name} = {t};"));
        self.line(format!("{name}$live = true;"));
        true
    }

    fn assignment(&mut self, target: ExprId, op: AssignOp, value: ExprId) -> String {
        let ty = self.ty(target);
        let place = self.place(target);
        let value = self.expr(value);
        let op = match op {
            AssignOp::Assign => return self.write(&place, &value),
            AssignOp::Add => BinaryOp::Add,
            AssignOp::Sub => BinaryOp::Sub,
            AssignOp::Mul => BinaryOp::Mul,
            AssignOp::Div => BinaryOp::Div,
            AssignOp::Rem => BinaryOp::Rem,
        };
        let current = self.read(&place);
        let new = self.operation(op, &ty, &current, &value);
        self.write(&place, &new)
    }

    /// `target++`, `target--`, as an expression.
    fn step(&mut self, target: ExprId, op: StepOp) -> String {
        let ty = self.ty(target);
        let one = match ty {
            Ty::Int(IntTy::I64 | IntTy::U64) => "1n",
            _ => "1",
        };
        let place = self.place(target);
        let current = self.read(&place);
        let op = if op == StepOp::Increment {
            BinaryOp::Add
        } else {
            BinaryOp::Sub
        };
        let new = self.operation(op, &ty, &current, one);
        self.write(&place, &new)
    }

    // --- Match --------------------------------------------------------------

    /// `match` as a value: its arms assign a temporary.
    pub(crate) fn match_value(&mut self, _id: ExprId, scrutinee: ExprId, arms: &[Arm]) -> String {
        let result = self.temp();
        self.line(format!("let {result};"));
        self.match_(scrutinee, arms, Some(&result));
        result
    }

    /// The arms of a `match`, tried in order; with `result`, each arm's
    /// value is assigned to it.
    fn match_(&mut self, scrutinee: ExprId, arms: &[Arm], result: Option<&str>) {
        let ty = self.ty(scrutinee);
        let value = self.expr(scrutinee);
        let subject = self.temp();
        self.line(format!("const {subject} = {value};"));
        let mut first = true;
        for arm in arms {
            let mut tests = Vec::new();
            let mut binds = Vec::new();
            self.pattern(&arm.pattern, &subject, &ty, &mut tests, &mut binds);
            let always = tests.is_empty();
            let head = match (first, always) {
                (true, true) => "{".to_owned(),
                (true, false) => format!("if ({}) {{", tests.join(" && ")),
                (false, true) => "} else {".to_owned(),
                (false, false) => format!("}} else if ({}) {{", tests.join(" && ")),
            };
            self.line(head);
            self.current.depth += 1;
            for (name, value) in binds {
                self.line(format!("const {name} = {value};"));
            }
            match &arm.body {
                ArmBody::Expr(body) => {
                    if let Some(result) = result {
                        let value = self.expr(*body);
                        self.line(format!("{result} = {value};"));
                    } else {
                        self.effect(*body);
                    }
                }
                ArmBody::Block(block) => self.block_stmts(block),
            }
            self.current.depth -= 1;
            first = false;
            if always {
                break;
            }
        }
        if !first {
            self.line("}");
        }
    }

    /// The tests a pattern makes on `subject`, of type `ty`, and the names
    /// it gives.
    fn pattern(
        &mut self,
        pattern: &Pattern,
        subject: &str,
        ty: &Ty,
        tests: &mut Vec<String>,
        binds: &mut Vec<(String, String)>,
    ) {
        match &pattern.kind {
            PatternKind::Wildcard => {}
            PatternKind::Name(ident) => {
                let span = ident.span;
                if self.declared.contains_key(&(span.start, span.end)) {
                    let name = self.declare(span, self.ast.name(ident.symbol));
                    binds.push((name, subject.to_owned()));
                } else {
                    let name = self.ast.name(ident.symbol).to_owned();
                    tests.push(Self::variant_test(subject, ty, &name));
                }
            }
            PatternKind::Literal(id) => {
                let value = self.expr(*id);
                tests.push(format!("{subject} === {value}"));
            }
            PatternKind::Variant { name, fields, .. } => {
                let text = self.ast.name(name.symbol).to_owned();
                tests.push(Self::variant_test(subject, ty, &text));
                let types = self.variant_fields(ty, &text);
                for (i, field) in fields.iter().enumerate() {
                    let field_ty = types.get(i).cloned().unwrap_or(Ty::Error);
                    let part = match ty {
                        Ty::Option(_) => subject.to_owned(),
                        _ => format!("{subject}.values[{i}]"),
                    };
                    self.pattern(field, &part, &field_ty, tests, binds);
                }
            }
        }
    }

    /// The test that `subject` is the variant `name`.
    fn variant_test(subject: &str, ty: &Ty, name: &str) -> String {
        match (ty, name) {
            (Ty::Option(_), "None") => format!("{subject} === undefined"),
            (Ty::Option(_), _) => format!("{subject} !== undefined"),
            _ => format!("{subject}.kind === {}", string_literal(name)),
        }
    }

    /// The types of a variant's data.
    fn variant_fields(&self, ty: &Ty, name: &str) -> Vec<Ty> {
        match ty {
            Ty::Option(inner) => vec![(**inner).clone()],
            Ty::Result(ok, err) => {
                if name == "Ok" {
                    vec![(**ok).clone()]
                } else {
                    vec![(**err).clone()]
                }
            }
            Ty::Enum(id, _) => self
                .analysis
                .enums
                .get(id.0)
                .and_then(|(_, variants)| variants.iter().find(|(n, _)| n == name))
                .map(|(_, types)| types.clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}
