//! Expressions. Each gives a JavaScript expression; one that needs
//! statements first (`match` as a value, `?`) writes them before, in order.

use std::fmt::Write as _;

use perasc_check::ty::{FloatTy, IntTy, Ty};
use perasc_check::{MethodCall, NameKind};
use perasc_syntax::ast::{BinaryOp, ExprId, ExprKind, Ident, LambdaBody, UnaryOp};

use crate::{Module, is_big, round, wrap};

/// A string as a JavaScript literal.
pub fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A float as a JavaScript literal, the shortest that reads back the same.
fn float_literal(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_owned()
    } else if value.is_infinite() {
        if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned()
    } else {
        format!("{value:?}")
    }
}

/// Whether a JavaScript expression reads nothing that statements could
/// change: a literal or a temporary.
fn settled(js: &str) -> bool {
    js.starts_with("$t")
        || js.starts_with('"')
        || js == "undefined"
        || js == "true"
        || js == "false"
        || js.chars().next().is_some_and(|c| c.is_ascii_digit())
}

impl Module<'_> {
    /// An expression.
    pub(crate) fn expr(&mut self, id: ExprId) -> String {
        let expr = self.ast.expr(id);
        let span = expr.span;
        match &expr.kind {
            ExprKind::Int(value) => match self.ty(id) {
                Ty::Int(int) if is_big(int) => format!("{value}n"),
                Ty::Float(_) => {
                    #[allow(clippy::cast_precision_loss)]
                    let v = *value as f64;
                    float_literal(v)
                }
                _ => value.to_string(),
            },
            ExprKind::Float(value) => {
                if self.ty(id) == Ty::Float(FloatTy::F32) {
                    #[allow(clippy::cast_possible_truncation)]
                    let single = *value as f32;
                    float_literal(f64::from(single))
                } else {
                    float_literal(*value)
                }
            }
            ExprKind::String(text) => string_literal(text),
            ExprKind::Bool(value) => value.to_string(),
            ExprKind::Array(items) | ExprKind::Tuple(items) => {
                format!("[{}]", self.exprs(items).join(", "))
            }
            ExprKind::Lambda { params, body, .. } => self.lambda(params, body),
            ExprKind::Name(_) => self.name(id),
            ExprKind::Paren(inner) => self.expr(*inner),
            ExprKind::Unary { op, operand } => self.unary(id, *op, *operand),
            ExprKind::Binary { op, lhs, rhs } => self.binary(*op, *lhs, *rhs),
            ExprKind::Call { callee, args, .. } => self.call(id, *callee, args),
            ExprKind::Template { parts, exprs } => self.template(parts, exprs),
            ExprKind::Field { base, name } => self.field(id, *base, name),
            ExprKind::Index { base, index } => {
                let proven = self.proven(*base, *index);
                let values = self.exprs(&[*base, *index]);
                if proven {
                    format!("{}[{}]", values[0], values[1])
                } else {
                    let at = self.helper("$at");
                    format!("{at}({}, {})", values[0], values[1])
                }
            }
            ExprKind::Try(inner) => self.try_(id, *inner),
            ExprKind::Match { scrutinee, arms } => self.match_value(id, *scrutinee, arms),
            ExprKind::Error => {
                self.unsupported(span, "other");
                "undefined".to_owned()
            }
        }
    }

    /// Several expressions, evaluated in order: when a later one needs
    /// statements first, the earlier ones are kept in temporaries before them.
    pub(crate) fn exprs(&mut self, ids: &[ExprId]) -> Vec<String> {
        let mut out: Vec<String> = Vec::with_capacity(ids.len());
        for &id in ids {
            let saved = std::mem::take(&mut self.current.lines);
            let value = self.expr(id);
            let lines = std::mem::replace(&mut self.current.lines, saved);
            if !lines.is_empty() {
                for earlier in &mut out {
                    if !settled(earlier) {
                        let t = self.temp();
                        self.line(format!("const {t} = {earlier};"));
                        *earlier = t;
                    }
                }
                self.current.lines.extend(lines);
            }
            out.push(value);
        }
        out
    }

    /// An expression kept in a temporary unless it is already settled.
    pub(crate) fn spill(&mut self, js: String) -> String {
        if settled(&js) {
            return js;
        }
        let t = self.temp();
        self.line(format!("const {t} = {js};"));
        t
    }

    fn name(&mut self, id: ExprId) -> String {
        let span = self.ast.expr(id).span;
        match self.bound(id) {
            // A resource given away: its owner no longer releases it.
            Some((binding, NameKind::Local)) if self.analysis.moves.contains(&id) => {
                let name = self.variable(binding);
                format!("({name}$live = false, {name})")
            }
            Some((binding, NameKind::Local | NameKind::Global)) => self.variable(binding),
            Some((_, NameKind::None)) => "undefined".to_owned(),
            None => {
                let ExprKind::Name(symbol) = self.ast.expr(id).kind else {
                    return "undefined".to_owned();
                };
                if let Some(js) = Self::host_value(self.ast.name(symbol)) {
                    js.to_owned()
                } else {
                    self.unsupported(span, "other");
                    "undefined".to_owned()
                }
            }
            // `render`, given where a function is expected.
            Some((_, NameKind::Fn(f))) => self
                .fns
                .get(&f.0)
                .cloned()
                .unwrap_or_else(|| "$unnamed".to_owned()),
            _ => {
                self.unsupported(span, "function-value");
                "undefined".to_owned()
            }
        }
    }

    fn lambda(
        &mut self,
        params: &[(Ident, Option<perasc_syntax::ast::Type>)],
        body: &LambdaBody,
    ) -> String {
        let names: Vec<String> = params
            .iter()
            .map(|(ident, _)| self.declare(ident.span, self.ast.name(ident.symbol)))
            .collect();
        let body = match body {
            LambdaBody::Expr(body) => *body,
            LambdaBody::Block(block) => {
                // Its own `return`: `?` returns from it too.
                let outer = std::mem::replace(&mut self.current.in_lambda, false);
                let ((), lines) = self.apart(|m| m.block_stmts(block));
                self.current.in_lambda = outer;
                let close = "  ".repeat(self.current.depth);
                return if lines.is_empty() {
                    format!("({}) => {{}}", names.join(", "))
                } else {
                    format!(
                        "({}) => {{\n{}\n{close}}}",
                        names.join(", "),
                        lines.join("\n")
                    )
                };
            }
        };
        let outer = std::mem::replace(&mut self.current.in_lambda, true);
        let (value, lines) = self.apart(|m| m.expr(body));
        self.current.in_lambda = outer;
        let params = format!("({})", names.join(", "));
        if lines.is_empty() {
            format!("{params} => {value}")
        } else {
            let indent = "  ".repeat(self.current.depth + 1);
            let close = "  ".repeat(self.current.depth);
            format!(
                "{params} => {{\n{}\n{indent}return {value};\n{close}}}",
                lines.join("\n")
            )
        }
    }

    fn unary(&mut self, id: ExprId, op: UnaryOp, operand: ExprId) -> String {
        let ty = self.ty(id);
        // `-5` is a literal: no wrapping needed.
        if op == UnaryOp::Neg
            && matches!(
                self.ast.expr(operand).kind,
                ExprKind::Int(_) | ExprKind::Float(_)
            )
        {
            let value = self.expr(operand);
            return format!("-{value}");
        }
        let value = self.expr(operand);
        match op {
            UnaryOp::Neg => match ty {
                Ty::Int(int) => wrap(int, &format!("-{value}")),
                _ => format!("(-{value})"),
            },
            UnaryOp::Not => format!("(!{value})"),
            UnaryOp::BitNot => match ty {
                Ty::Int(IntTy::U32) => format!("((~{value}) >>> 0)"),
                Ty::Int(int @ (IntTy::U8 | IntTy::U16 | IntTy::U64)) => {
                    wrap(int, &format!("~{value}"))
                }
                _ => format!("(~{value})"),
            },
            UnaryOp::Borrow => value,
            UnaryOp::Await => format!("(await {value})"),
        }
    }

    fn binary(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> String {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            return self.logical(op == BinaryOp::And, lhs, rhs);
        }
        let ty = self.ty(lhs);
        let values = self.exprs(&[lhs, rhs]);
        let (a, b) = (&values[0], &values[1]);
        self.operation(op, &ty, a, b)
    }

    /// `a op b` on values of type `ty`, as the owned backends compute it.
    pub(crate) fn operation(&mut self, op: BinaryOp, ty: &Ty, a: &str, b: &str) -> String {
        let symbol = match op {
            BinaryOp::Eq | BinaryOp::Ne => {
                let negate = if op == BinaryOp::Ne { "!" } else { "" };
                return if matches!(
                    ty,
                    Ty::Int(_) | Ty::Float(_) | Ty::Bool | Ty::String | Ty::Host(_)
                ) {
                    let strict = if negate.is_empty() { "===" } else { "!==" };
                    format!("({a} {strict} {b})")
                } else {
                    let eq = self.helper("$eq");
                    format!("{negate}{eq}({a}, {b})")
                };
            }
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        };
        if matches!(
            op,
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
        ) {
            return format!("({a} {symbol} {b})");
        }
        match ty {
            Ty::Int(int) => self.integer(op, *int, a, b),
            Ty::Bool => format!("(({a} {symbol} {b}) !== 0)"),
            Ty::Float(_) => round(ty, format!("({a} {symbol} {b})")),
            _ => format!("({a} {symbol} {b})"),
        }
    }

    /// Integer arithmetic: wrapped, and a division by zero throws.
    fn integer(&mut self, op: BinaryOp, int: IntTy, a: &str, b: &str) -> String {
        let big = is_big(int);
        match op {
            BinaryOp::Add => wrap(int, &format!("{a} + {b}")),
            BinaryOp::Sub => wrap(int, &format!("{a} - {b}")),
            BinaryOp::Mul if big => wrap(int, &format!("{a} * {b}")),
            BinaryOp::Mul if int == IntTy::I32 => format!("Math.imul({a}, {b})"),
            BinaryOp::Mul => wrap(int, &format!("Math.imul({a}, {b})")),
            BinaryOp::Div | BinaryOp::Rem => {
                let helper = match (op == BinaryOp::Div, big) {
                    (true, true) => self.helper("$div64"),
                    (true, false) => self.helper("$div"),
                    (false, true) => self.helper("$rem64"),
                    (false, false) => self.helper("$rem"),
                };
                let signed = !matches!(int, IntTy::U8 | IntTy::U16 | IntTy::U32 | IntTy::U64);
                wrap(int, &format!("{helper}({a}, {b}, {signed})"))
            }
            BinaryOp::Shl if big => wrap(int, &format!("{a} << ({b} & 63n)")),
            BinaryOp::Shr if big => format!("({a} >> ({b} & 63n))"),
            BinaryOp::Shl => wrap(int, &format!("{a} << {b}")),
            BinaryOp::Shr => match int {
                IntTy::U8 | IntTy::U16 | IntTy::U32 => format!("({a} >>> {b})"),
                _ => format!("({a} >> {b})"),
            },
            BinaryOp::BitAnd => self.bits(int, a, "&", b),
            BinaryOp::BitOr => self.bits(int, a, "|", b),
            BinaryOp::BitXor => self.bits(int, a, "^", b),
            _ => format!("({a} ? {b} : {b})"),
        }
    }

    /// `&`, `|`, `^`: exact on `BigInt` and on 32-bit numbers, unsigned
    /// ones put back above zero.
    #[allow(clippy::unused_self)]
    fn bits(&self, int: IntTy, a: &str, symbol: &str, b: &str) -> String {
        if int == IntTy::U32 {
            format!("(({a} {symbol} {b}) >>> 0)")
        } else {
            format!("({a} {symbol} {b})")
        }
    }

    /// `a && b`, `a || b`: `b` evaluated only when needed, its statements too.
    fn logical(&mut self, and: bool, lhs: ExprId, rhs: ExprId) -> String {
        let a = self.expr(lhs);
        let (b, lines) = self.apart(|m| m.expr(rhs));
        let symbol = if and { "&&" } else { "||" };
        if lines.is_empty() {
            return format!("({a} {symbol} {b})");
        }
        let t = self.temp();
        self.line(format!("let {t} = {a};"));
        let test = if and { t.clone() } else { format!("!{t}") };
        self.line(format!("if ({test}) {{"));
        self.current.lines.extend(lines);
        let indent = "  ".repeat(self.current.depth + 1);
        self.current.lines.push(format!("{indent}{t} = {b};"));
        self.line("}");
        t
    }

    fn template(&mut self, parts: &[String], exprs: &[ExprId]) -> String {
        let values = self.exprs(exprs);
        let mut out = String::from("`");
        for (i, part) in parts.iter().enumerate() {
            for c in part.chars() {
                match c {
                    '`' => out.push_str("\\`"),
                    '\\' => out.push_str("\\\\"),
                    '$' => out.push_str("\\$"),
                    c => out.push(c),
                }
            }
            if let (Some(value), Some(&id)) = (values.get(i), exprs.get(i)) {
                // `${x}` converts as `String(x)` does: only an `f32` needs more.
                let text = if self.ty(id) == Ty::Float(FloatTy::F32) {
                    self.text_of(id, value)
                } else {
                    value.clone()
                };
                out.push_str("${");
                out.push_str(&text);
                out.push('}');
            }
        }
        out.push('`');
        out
    }

    /// A value as text, as `toString()` writes it.
    pub(crate) fn text_of(&mut self, id: ExprId, value: &str) -> String {
        if self.ty(id) == Ty::Float(FloatTy::F32) {
            let helper = self.helper("$f32text");
            format!("{helper}({value})")
        } else {
            format!("String({value})")
        }
    }

    fn field(&mut self, id: ExprId, base: ExprId, name: &Ident) -> String {
        let text = self.ast.name(name.symbol);
        // `Shape.Empty`: a variant without data.
        if let Some((_, NameKind::Enum(_))) = self.bound(base) {
            return format!("{{ kind: {}, values: [] }}", string_literal(text));
        }
        let ty = self.ty(base);
        let value = self.expr(base);
        match (&ty, text) {
            (Ty::Array(_) | Ty::String, "length") => format!("{value}.length"),
            (Ty::Map(..) | Ty::Set(_), "size") => format!("{value}.size"),
            (Ty::Host(h), _) => format!("{value}.{}", Self::host_field(*h, text)),
            _ if self.is_tuple(&ty) => format!("{value}[{text}]"),
            _ => {
                let _ = id;
                format!("{value}.{text}")
            }
        }
    }

    /// `value?`: returns `None` or the `Err` from the function.
    fn try_(&mut self, id: ExprId, inner: ExprId) -> String {
        let span = self.ast.expr(id).span;
        if self.current.in_lambda {
            self.unsupported(span, "try");
            return "undefined".to_owned();
        }
        let ty = self.ty(inner);
        let value = self.expr(inner);
        let t = self.temp();
        self.line(format!("const {t} = {value};"));
        if matches!(ty, Ty::Result(..)) {
            self.line(format!("if ({t}.kind === \"Err\") return {t};"));
            format!("{t}.values[0]")
        } else {
            self.line(format!("if ({t} === undefined) return undefined;"));
            t
        }
    }

    // --- Calls --------------------------------------------------------------

    fn call(&mut self, id: ExprId, callee: ExprId, args: &[ExprId]) -> String {
        let span = self.ast.expr(id).span;
        match &self.ast.expr(callee).kind {
            ExprKind::Name(symbol) => match self.bound(callee) {
                None => {
                    let name = self.ast.name(*symbol).to_owned();
                    self.host_call(id, &name, args).unwrap_or_else(|| {
                        self.unsupported(span, "function-value");
                        "undefined".to_owned()
                    })
                }
                Some((_, NameKind::Fn(f))) => {
                    let name = self.callee_name(id, f.0);
                    let values = self.exprs(args);
                    format!("{name}({})", values.join(", "))
                }
                Some((_, NameKind::Struct(_))) => {
                    let ty = self.ty(id);
                    let values = self.exprs(args);
                    self.construct(&ty, &values)
                }
                Some((_, NameKind::Some)) => {
                    let values = self.exprs(args);
                    values
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| "undefined".to_owned())
                }
                Some((_, kind @ (NameKind::Ok | NameKind::Err))) => {
                    let values = self.exprs(args);
                    let variant = if kind == NameKind::Ok { "Ok" } else { "Err" };
                    format!("{{ kind: \"{variant}\", values: [{}] }}", values.join(", "))
                }
                // A function kept in a variable or a parameter.
                Some((binding, NameKind::Local | NameKind::Global)) => {
                    let name = self.variable(binding);
                    let values = self.exprs(args);
                    format!("{name}({})", values.join(", "))
                }
                _ => {
                    self.unsupported(span, "function-value");
                    "undefined".to_owned()
                }
            },
            ExprKind::Field { base, name } => self.call_field(id, *base, name, args),
            _ if matches!(self.ty(callee), Ty::Callback(..)) => {
                let function = self.expr(callee);
                let values = self.exprs(args);
                format!("({function})({})", values.join(", "))
            }
            _ => {
                self.unsupported(span, "function-value");
                "undefined".to_owned()
            }
        }
    }

    /// The function a call reaches: an instance for a generic one.
    fn callee_name(&self, call: ExprId, index: usize) -> String {
        if let Some((checked, args)) = self.analysis.calls.get(&call) {
            let args: Vec<Ty> = args
                .iter()
                .map(|t| self.analysis.subst(t, &self.current.args))
                .collect();
            if let Some(name) = self.instances.get(&(checked.0, args)) {
                return name.clone();
            }
        }
        self.fns
            .get(&index)
            .cloned()
            .unwrap_or_else(|| "$unnamed".to_owned())
    }

    /// A structure's value: an object, or an array for a tuple.
    pub(crate) fn construct(&self, ty: &Ty, values: &[String]) -> String {
        if self.is_tuple(ty) {
            return format!("[{}]", values.join(", "));
        }
        let fields: Vec<String> = self
            .fields(ty)
            .iter()
            .zip(values)
            .map(|(name, value)| format!("{name}: {value}"))
            .collect();
        format!("{{ {} }}", fields.join(", "))
    }

    fn call_field(&mut self, id: ExprId, base: ExprId, name: &Ident, args: &[ExprId]) -> String {
        let text = self.ast.name(name.symbol);
        let span = self.ast.expr(id).span;
        // `Map.new()`, `Set.new()`.
        if text == "new" && self.bound(base).is_none() {
            match self.ty(id) {
                Ty::Map(..) => return "new Map()".to_owned(),
                Ty::Set(_) => return "new Set()".to_owned(),
                _ => {}
            }
        }
        if let Some(js) = self.group_new(id, base, name) {
            return js;
        }
        // A method of the browser.
        if let Ty::Host(h) = self.ty(base) {
            let mut all = Vec::with_capacity(args.len() + 1);
            all.push(base);
            all.extend_from_slice(args);
            let values = self.exprs(&all);
            return self.host_method(id, h, text, &values[0], &values[1..]);
        }
        // `Shape.Circle(r)`.
        if let Some((_, NameKind::Enum(_))) = self.bound(base) {
            let values = self.exprs(args);
            return format!(
                "{{ kind: {}, values: [{}] }}",
                string_literal(text),
                values.join(", ")
            );
        }
        // A method of the module: `p.norm()`, `Point.origin()`.
        if let Some(call) = self.analysis.method_calls.get(&id) {
            let target = match call {
                MethodCall::Fn { id: f, receiver } => Some((self.callee_name(id, f.0), *receiver)),
                MethodCall::Interface => self
                    .analysis
                    .dyn_targets
                    .get(&(id, self.current.args.clone()))
                    .map(|(f, args)| {
                        let name = if args.is_empty() {
                            self.fns.get(&f.0).cloned()
                        } else {
                            self.instances.get(&(f.0, args.clone())).cloned()
                        };
                        (name.unwrap_or_else(|| "$unnamed".to_owned()), true)
                    }),
            };
            let Some((function, receiver)) = target else {
                self.unsupported(span, "method");
                return "undefined".to_owned();
            };
            let mut all = Vec::with_capacity(args.len() + 1);
            if receiver {
                all.push(base);
            }
            all.extend_from_slice(args);
            let values = self.exprs(&all);
            return format!("{function}({})", values.join(", "));
        }
        self.builtin(id, base, text, args)
    }
}
