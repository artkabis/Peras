//! The abstract syntax tree.
//!
//! Expressions live in an arena ([`Ast::exprs`]) and refer to each other by
//! [`ExprId`]: cheap to copy, no lifetimes in the API, and dropping a deeply nested
//! tree cannot overflow the stack. Names are interned as [`Symbol`]s.

use std::collections::HashMap;
use std::fmt::Write as _;

use perasc_diag::Span;

/// An interned name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u32);

/// Stores each distinct name once.
#[derive(Debug, Default, Clone)]
pub struct Interner {
    names: Vec<String>,
    map: HashMap<String, Symbol>,
}

impl Interner {
    /// The symbol for `name`, creating it on first use.
    pub fn intern(&mut self, name: &str) -> Symbol {
        if let Some(&symbol) = self.map.get(name) {
            return symbol;
        }
        let symbol = Symbol(u32::try_from(self.names.len()).unwrap_or(u32::MAX));
        self.names.push(name.to_owned());
        self.map.insert(name.to_owned(), symbol);
        symbol
    }

    /// The name of a symbol.
    #[must_use]
    pub fn name(&self, symbol: Symbol) -> &str {
        usize::try_from(symbol.0)
            .ok()
            .and_then(|i| self.names.get(i))
            .map_or("", String::as_str)
    }
}

/// A name and where it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ident {
    /// The interned name.
    pub symbol: Symbol,
    /// Its position.
    pub span: Span,
}

/// Index of an expression in [`Ast::exprs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(u32);

/// A parsed module.
#[derive(Debug, Default, Clone)]
pub struct Ast {
    /// Top-level declarations, in source order.
    pub items: Vec<Item>,
    /// Every expression of the module.
    pub exprs: Vec<Expr>,
    /// Every name of the module.
    pub interner: Interner,
}

impl Ast {
    /// Adds an expression to the arena.
    pub fn alloc(&mut self, kind: ExprKind, span: Span) -> ExprId {
        let id = ExprId(u32::try_from(self.exprs.len()).unwrap_or(u32::MAX));
        self.exprs.push(Expr { kind, span });
        id
    }

    /// An expression. Ids always come from this AST; a foreign id yields an error node.
    #[must_use]
    pub fn expr(&self, id: ExprId) -> &Expr {
        const MISSING: Expr = Expr {
            kind: ExprKind::Error,
            span: Span::new(0, 0),
        };
        usize::try_from(id.0)
            .ok()
            .and_then(|i| self.exprs.get(i))
            .unwrap_or(&MISSING)
    }

    /// The name of a symbol.
    #[must_use]
    pub fn name(&self, symbol: Symbol) -> &str {
        self.interner.name(symbol)
    }
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// What it declares.
    pub kind: ItemKind,
    /// From its first attribute or keyword to its end.
    pub span: Span,
}

/// The kinds of declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    /// `fn name(…) { … }`
    Fn(FnDecl),
    /// A module-level `let` or `const`.
    Global(Local),
}

/// A function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    /// `@frame` and other attributes, without the `@`.
    pub attributes: Vec<Ident>,
    /// Preceded by `export`.
    pub exported: bool,
    /// Preceded by `async`.
    pub is_async: bool,
    /// The function name.
    pub name: Ident,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// The return type, if written.
    pub ret: Option<Type>,
    /// The body.
    pub body: Block,
}

/// How an argument is passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convention {
    /// Default: read-only, no copy.
    Let,
    /// `inout`: exclusive mutation.
    Inout,
    /// `sink`: the function consumes the value.
    Sink,
}

/// A function parameter: `name: inout Type`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Its name.
    pub name: Ident,
    /// Its passing convention.
    pub convention: Convention,
    /// Its type.
    pub ty: Type,
}

/// A type as written.
#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    /// Its shape.
    pub kind: TypeKind,
    /// Its position.
    pub span: Span,
}

/// The shapes of type.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// `i32`, `Array<f32>`, `Option<Array<u8>>`.
    Named {
        /// The type name.
        name: Ident,
        /// Generic arguments, empty without `<…>`.
        args: Vec<Type>,
    },
    /// A type that failed to parse, already reported.
    Error,
}

/// `{ statements }`
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Statements, in order.
    pub stmts: Vec<Stmt>,
    /// From `{` to `}`.
    pub span: Span,
}

/// A statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    /// What it does.
    pub kind: StmtKind,
    /// Its position, `;` included.
    pub span: Span,
}

/// The kinds of statement.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `let` or `const`.
    Local(Local),
    /// `return value;`
    Return(Option<ExprId>),
    /// An expression evaluated for its effect, usually a call.
    Expr(ExprId),
    /// `target = value;` or a compound form such as `+=`.
    Assign {
        /// What is assigned.
        target: ExprId,
        /// `=`, `+=`…
        op: AssignOp,
        /// The new value.
        value: ExprId,
    },
    /// `target++;` or `target--;`
    Step {
        /// What is incremented or decremented.
        target: ExprId,
        /// `++` or `--`.
        op: StepOp,
    },
    /// A statement that failed to parse, already reported.
    Error,
}

/// `let name: Type = value` or `const name = value`.
#[derive(Debug, Clone, PartialEq)]
pub struct Local {
    /// `let` (true) or `const` (false).
    pub mutable: bool,
    /// The variable.
    pub name: Ident,
    /// Its annotation, if written.
    pub ty: Option<Type>,
    /// Its initial value, if written.
    pub init: Option<ExprId>,
}

/// Assignment operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    /// `=`
    Assign,
    /// `+=`
    Add,
    /// `-=`
    Sub,
    /// `*=`
    Mul,
    /// `/=`
    Div,
    /// `%=`
    Rem,
}

/// `++` and `--`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOp {
    /// `++`
    Increment,
    /// `--`
    Decrement,
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// Its shape.
    pub kind: ExprKind,
    /// Its position.
    pub span: Span,
}

/// The shapes of expression.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// An integer literal; its type is decided by the type checker.
    Int(u64),
    /// A floating-point literal.
    Float(f64),
    /// A string literal, escapes decoded.
    String(String),
    /// `true` or `false`.
    Bool(bool),
    /// A variable or function name.
    Name(Symbol),
    /// `( inner )`, kept so that later passes and fixes see the parentheses.
    Paren(ExprId),
    /// `-x`, `!x`, `~x`, `&x`.
    Unary {
        /// The operator.
        op: UnaryOp,
        /// Its operand.
        operand: ExprId,
    },
    /// `lhs op rhs`.
    Binary {
        /// The operator.
        op: BinaryOp,
        /// Left operand.
        lhs: ExprId,
        /// Right operand.
        rhs: ExprId,
    },
    /// `callee(args)`.
    Call {
        /// What is called.
        callee: ExprId,
        /// Arguments, in order.
        args: Vec<ExprId>,
    },
    /// `base.name`.
    Field {
        /// The value.
        base: ExprId,
        /// The field or method name.
        name: Ident,
    },
    /// `base[index]`.
    Index {
        /// The collection.
        base: ExprId,
        /// The index.
        index: ExprId,
    },
    /// `value?`: returns early on an error.
    Try(ExprId),
    /// An expression that failed to parse, already reported.
    Error,
}

/// Prefix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-`
    Neg,
    /// `!`
    Not,
    /// `~`
    BitNot,
    /// `&`: a visible exclusive borrow, for an `inout` argument.
    Borrow,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `||`
    Or,
    /// `&&`
    And,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `&`
    BitAnd,
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
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
}

impl BinaryOp {
    /// The operator as written.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Or => "||",
            Self::And => "&&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::BitAnd => "&",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
        }
    }

    /// `==` and `!=`.
    #[must_use]
    pub const fn is_equality(self) -> bool {
        matches!(self, Self::Eq | Self::Ne)
    }

    /// `<`, `<=`, `>` and `>=`.
    #[must_use]
    pub const fn is_ordering(self) -> bool {
        matches!(self, Self::Lt | Self::Le | Self::Gt | Self::Ge)
    }

    /// Equality or ordering.
    #[must_use]
    pub const fn is_comparison(self) -> bool {
        self.is_equality() || self.is_ordering()
    }

    /// `&`, `|` and `^`.
    #[must_use]
    pub const fn is_bitwise(self) -> bool {
        matches!(self, Self::BitAnd | Self::BitOr | Self::BitXor)
    }
}

// ---------------------------------------------------------------------------
// S-expression dump, for snapshot tests
// ---------------------------------------------------------------------------

impl Ast {
    /// The tree as indented S-expressions, one declaration or statement per line.
    #[must_use]
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            match &item.kind {
                ItemKind::Fn(f) => self.dump_fn(&mut out, f),
                ItemKind::Global(local) => {
                    let _ = writeln!(out, "{}", self.local(local));
                }
            }
        }
        out
    }

    fn dump_fn(&self, out: &mut String, f: &FnDecl) {
        let mut head = String::from("(fn ");
        head.push_str(self.name(f.name.symbol));
        for attribute in &f.attributes {
            let _ = write!(head, " @{}", self.name(attribute.symbol));
        }
        if f.exported {
            head.push_str(" export");
        }
        if f.is_async {
            head.push_str(" async");
        }
        head.push_str(" (params");
        for p in &f.params {
            let convention = match p.convention {
                Convention::Let => "",
                Convention::Inout => "inout ",
                Convention::Sink => "sink ",
            };
            let _ = write!(
                head,
                " ({} {convention}{})",
                self.name(p.name.symbol),
                self.ty(&p.ty)
            );
        }
        head.push(')');
        if let Some(ret) = &f.ret {
            let _ = write!(head, " -> {}", self.ty(ret));
        }
        let _ = writeln!(out, "{head}");
        for stmt in &f.body.stmts {
            let _ = writeln!(out, "  {}", self.stmt(stmt));
        }
        let _ = writeln!(out, ")");
    }

    fn ty(&self, ty: &Type) -> String {
        match &ty.kind {
            TypeKind::Named { name, args } if args.is_empty() => self.name(name.symbol).to_owned(),
            TypeKind::Named { name, args } => {
                let args: Vec<String> = args.iter().map(|a| self.ty(a)).collect();
                format!("{}<{}>", self.name(name.symbol), args.join(", "))
            }
            TypeKind::Error => "<error>".to_owned(),
        }
    }

    fn local(&self, local: &Local) -> String {
        let keyword = if local.mutable { "let" } else { "const" };
        let mut s = format!("({keyword} {}", self.name(local.name.symbol));
        if let Some(ty) = &local.ty {
            let _ = write!(s, ": {}", self.ty(ty));
        }
        if let Some(init) = local.init {
            let _ = write!(s, " {}", self.sexpr(init));
        }
        s.push(')');
        s
    }

    fn stmt(&self, stmt: &Stmt) -> String {
        match &stmt.kind {
            StmtKind::Local(local) => self.local(local),
            StmtKind::Return(None) => "(return)".to_owned(),
            StmtKind::Return(Some(value)) => format!("(return {})", self.sexpr(*value)),
            StmtKind::Expr(e) => self.sexpr(*e),
            StmtKind::Assign { target, op, value } => {
                let op = match op {
                    AssignOp::Assign => "=",
                    AssignOp::Add => "+=",
                    AssignOp::Sub => "-=",
                    AssignOp::Mul => "*=",
                    AssignOp::Div => "/=",
                    AssignOp::Rem => "%=",
                };
                format!("({op} {} {})", self.sexpr(*target), self.sexpr(*value))
            }
            StmtKind::Step { target, op } => {
                let op = match op {
                    StepOp::Increment => "++",
                    StepOp::Decrement => "--",
                };
                format!("({op} {})", self.sexpr(*target))
            }
            StmtKind::Error => "<error>".to_owned(),
        }
    }

    /// One expression as an S-expression.
    #[must_use]
    pub fn sexpr(&self, id: ExprId) -> String {
        match &self.expr(id).kind {
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Float(x) => format!("{x:?}"),
            ExprKind::String(s) => format!("{s:?}"),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Name(s) => self.name(*s).to_owned(),
            ExprKind::Paren(inner) => format!("(paren {})", self.sexpr(*inner)),
            ExprKind::Unary { op, operand } => {
                let op = match op {
                    UnaryOp::Neg => "neg",
                    UnaryOp::Not => "!",
                    UnaryOp::BitNot => "~",
                    UnaryOp::Borrow => "&",
                };
                format!("({op} {})", self.sexpr(*operand))
            }
            ExprKind::Binary { op, lhs, rhs } => {
                format!(
                    "({} {} {})",
                    op.symbol(),
                    self.sexpr(*lhs),
                    self.sexpr(*rhs)
                )
            }
            ExprKind::Call { callee, args } => {
                let mut s = format!("(call {}", self.sexpr(*callee));
                for arg in args {
                    let _ = write!(s, " {}", self.sexpr(*arg));
                }
                s.push(')');
                s
            }
            ExprKind::Field { base, name } => {
                format!("(. {} {})", self.sexpr(*base), self.name(name.symbol))
            }
            ExprKind::Index { base, index } => {
                format!("(index {} {})", self.sexpr(*base), self.sexpr(*index))
            }
            ExprKind::Try(inner) => format!("(? {})", self.sexpr(*inner)),
            ExprKind::Error => "<error>".to_owned(),
        }
    }
}
