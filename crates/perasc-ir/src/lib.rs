//! Typed, desugared intermediate representation shared by both Peras backends.
//!
//! The IR is what the backends compile: every expression carries its final type,
//! names are resolved to function and local indices, compound assignments and
//! increments are expanded, and every loop is one [`Stmt::Loop`] with an optional
//! condition and a step that `continue` runs. [`lower`] builds it from a checked
//! module; constructs the backends cannot compile yet are reported with `E0900`.

use std::fmt::{self, Write as _};

mod lower;

pub use lower::lower;

/// A value type. `bool` is stored as a 32-bit integer by the backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ty {
    /// `i32`
    I32,
    /// `i64`
    I64,
    /// `f32`
    F32,
    /// `f64`
    F64,
    /// `bool`
    Bool,
    /// No value.
    Void,
}

impl Ty {
    /// Its name in Peras.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Void => "void",
        }
    }

    /// `i32` and `i64`.
    #[must_use]
    pub const fn is_int(self) -> bool {
        matches!(self, Self::I32 | Self::I64)
    }

    /// `f32` and `f64`.
    #[must_use]
    pub const fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}

/// Index of a function in [`Module::functions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FnId(pub u32);

/// Index of a local in [`Function::locals`]; parameters come first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

/// A compiled module.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    /// Functions, in source order.
    pub functions: Vec<Function>,
}

/// A function.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    /// Its name in source, also its export name.
    pub name: String,
    /// Whether JavaScript can call it.
    pub exported: bool,
    /// Number of parameters: the first locals.
    pub param_count: u32,
    /// Parameters, then variables, each with its source name.
    pub locals: Vec<Local>,
    /// The result type.
    pub ret: Ty,
    /// The body.
    pub body: Vec<Stmt>,
}

impl Function {
    /// The parameters.
    pub fn params(&self) -> impl Iterator<Item = &Local> {
        self.locals
            .iter()
            .take(usize::try_from(self.param_count).unwrap_or(usize::MAX))
    }
}

/// A parameter or variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    /// Its name in source.
    pub name: String,
    /// Its type.
    pub ty: Ty,
}

/// A statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `local = value`
    Set(LocalId, Expr),
    /// A value computed for its effect, then dropped.
    Expr(Expr),
    /// `if (cond) { then } else { otherwise }`
    If {
        /// The condition, a `bool`.
        cond: Expr,
        /// Run when it holds.
        then: Vec<Stmt>,
        /// Run otherwise; often empty.
        otherwise: Vec<Stmt>,
    },
    /// A loop: checks `cond` (none means forever), runs `body`, then `step`.
    /// `continue` jumps to `step`; `break` leaves the loop.
    Loop {
        /// Checked before each iteration.
        cond: Option<Expr>,
        /// The body.
        body: Vec<Stmt>,
        /// Run after each iteration, also after `continue`.
        step: Vec<Stmt>,
    },
    /// Leaves the innermost loop.
    Break,
    /// Goes to the step of the innermost loop.
    Continue,
    /// Returns from the function.
    Return(Option<Expr>),
}

/// An expression and its type.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// Its shape.
    pub kind: ExprKind,
    /// Its type.
    pub ty: Ty,
}

/// A constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Const {
    /// An integer, of the expression's type.
    Int(i64),
    /// A float, of the expression's type.
    Float(f64),
    /// A boolean.
    Bool(bool),
}

/// The shapes of expression.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A constant.
    Const(Const),
    /// A parameter or variable.
    Local(LocalId),
    /// A prefix operator.
    Unary(UnOp, Box<Expr>),
    /// A binary operator; both operands have the same type.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// A call to a function of the module.
    Call(FnId, Vec<Expr>),
    /// A numeric conversion to the expression's type: `toF64()`, `toI32()`…
    Convert(Box<Expr>),
    /// A math operation of the operand's type.
    Math(MathOp, Box<Expr>),
}

/// Prefix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `-x`; wraps on integers.
    Neg,
    /// `!x` on a `bool`.
    Not,
    /// `~x` on an integer.
    BitNot,
}

/// Binary operators. Integer arithmetic wraps around; division by zero traps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`; on integers, truncates toward zero and traps on zero or overflow.
    Div,
    /// `%` on integers; the sign follows the dividend.
    Rem,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`, shift count taken modulo the width.
    Shl,
    /// `>>`, arithmetic, shift count taken modulo the width.
    Shr,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`, short-circuit.
    And,
    /// `||`, short-circuit.
    Or,
}

impl BinOp {
    /// The operator in source.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::And => "&&",
            Self::Or => "||",
        }
    }

    /// Comparisons, which produce a `bool`.
    #[must_use]
    pub const fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge
        )
    }
}

/// Math operations available on every target without a library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathOp {
    /// Absolute value; wraps for the smallest integer.
    Abs,
    /// Square root.
    Sqrt,
    /// Rounds down.
    Floor,
    /// Rounds up.
    Ceil,
}

impl MathOp {
    /// The method name in Peras.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Abs => "abs",
            Self::Sqrt => "sqrt",
            Self::Floor => "floor",
            Self::Ceil => "ceil",
        }
    }
}

// ---------------------------------------------------------------------------
// Text form, for snapshot tests
// ---------------------------------------------------------------------------

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for function in &self.functions {
            let params: Vec<String> = function
                .params()
                .enumerate()
                .map(|(i, l)| format!("%{i} {}: {}", l.name, l.ty.name()))
                .collect();
            let export = if function.exported { "export " } else { "" };
            writeln!(
                f,
                "{export}fn {}({}) -> {}",
                function.name,
                params.join(", "),
                function.ret.name()
            )?;
            for (i, local) in function.locals.iter().enumerate().skip(params.len()) {
                writeln!(f, "  local %{i} {}: {}", local.name, local.ty.name())?;
            }
            let mut out = String::new();
            write_stmts(&mut out, &function.body, 1);
            f.write_str(&out)?;
        }
        Ok(())
    }
}

fn write_stmts(out: &mut String, stmts: &[Stmt], level: usize) {
    let pad = "  ".repeat(level);
    for stmt in stmts {
        match stmt {
            Stmt::Set(local, value) => {
                let _ = writeln!(out, "{pad}%{} = {}", local.0, expr(value));
            }
            Stmt::Expr(e) => {
                let _ = writeln!(out, "{pad}{}", expr(e));
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                let _ = writeln!(out, "{pad}if {}", expr(cond));
                write_stmts(out, then, level + 1);
                if !otherwise.is_empty() {
                    let _ = writeln!(out, "{pad}else");
                    write_stmts(out, otherwise, level + 1);
                }
            }
            Stmt::Loop { cond, body, step } => {
                let cond = cond.as_ref().map_or_else(|| "forever".to_owned(), expr);
                let _ = writeln!(out, "{pad}loop while {cond}");
                write_stmts(out, body, level + 1);
                if !step.is_empty() {
                    let _ = writeln!(out, "{pad}step");
                    write_stmts(out, step, level + 1);
                }
            }
            Stmt::Break => {
                let _ = writeln!(out, "{pad}break");
            }
            Stmt::Continue => {
                let _ = writeln!(out, "{pad}continue");
            }
            Stmt::Return(value) => {
                let value = value.as_ref().map(expr).unwrap_or_default();
                let _ = writeln!(out, "{pad}return {value}");
            }
        }
    }
}

/// An expression as text: `(i32.add %0 1)`.
fn expr(e: &Expr) -> String {
    let ty = e.ty.name();
    match &e.kind {
        ExprKind::Const(Const::Int(v)) => format!("{v}:{ty}"),
        ExprKind::Const(Const::Float(v)) => format!("{v:?}:{ty}"),
        ExprKind::Const(Const::Bool(v)) => v.to_string(),
        ExprKind::Local(l) => format!("%{}", l.0),
        ExprKind::Unary(op, operand) => {
            let op = match op {
                UnOp::Neg => "neg",
                UnOp::Not => "not",
                UnOp::BitNot => "bitnot",
            };
            format!("({ty}.{op} {})", expr(operand))
        }
        ExprKind::Binary(op, lhs, rhs) => {
            format!(
                "({}.{} {} {})",
                lhs.ty.name(),
                op.symbol(),
                expr(lhs),
                expr(rhs)
            )
        }
        ExprKind::Call(id, args) => {
            let args: Vec<String> = args.iter().map(expr).collect();
            format!("(call #{} {})", id.0, args.join(" "))
        }
        ExprKind::Convert(operand) => {
            format!("({ty}.from_{} {})", operand.ty.name(), expr(operand))
        }
        ExprKind::Math(op, operand) => format!("({ty}.{} {})", op.name(), expr(operand)),
    }
}
