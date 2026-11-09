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

    /// The symbol of a name, if the module uses it; never adds one.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<Symbol> {
        self.map.get(name).copied()
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

    /// The symbol of a name, if the module uses it.
    #[must_use]
    pub fn interner_symbol(&self, name: &str) -> Option<Symbol> {
        self.interner.lookup(name)
    }

    /// Every function of the module, the methods of each `impl` in place of
    /// it: the order every pass numbers them in.
    #[must_use]
    pub fn functions(&self) -> Vec<&FnDecl> {
        let mut out = Vec::new();
        for item in &self.items {
            match &item.kind {
                ItemKind::Fn(decl) => out.push(decl),
                ItemKind::Impl(decl) => out.extend(decl.methods.iter()),
                _ => {}
            }
        }
        out
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
    /// `struct Name { field: Type; … }`
    Struct(StructDecl),
    /// `enum Name { Variant, Variant(Type, …), … }`
    Enum(EnumDecl),
    /// `type Name<T> = Type;`
    Alias(AliasDecl),
    /// `impl Type { fn … }` or `impl Interface for Type { fn … }`
    Impl(ImplDecl),
    /// `interface Name { fn method(self, …): Type; … }`
    Interface(InterfaceDecl),
}

/// `type Meters = f64;`: another name for a type, interchangeable with it.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasDecl {
    /// Preceded by `export`.
    pub exported: bool,
    /// The new name.
    pub name: Ident,
    /// Its type parameters, `<T>`.
    pub generics: Vec<TypeParam>,
    /// The type it names.
    pub ty: Type,
}

/// The methods of a type, or its implementation of an interface.
#[derive(Debug, Clone, PartialEq)]
pub struct ImplDecl {
    /// `impl<T> Stack<T>`: the type parameters of the target.
    pub generics: Vec<TypeParam>,
    /// The interface implemented, after `impl` and before `for`.
    pub interface: Option<Ident>,
    /// The type the methods belong to.
    pub target: Type,
    /// Its methods: their `generics` start with those of the `impl`, their
    /// first parameter is `self` when they take one.
    pub methods: Vec<FnDecl>,
}

/// An interface: methods a type provides to meet it as a constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    /// Preceded by `export`.
    pub exported: bool,
    /// Its name.
    pub name: Ident,
    /// The methods required, without bodies; `self` has type `Self`.
    pub methods: Vec<MethodSig>,
}

/// A method an interface requires: `fn area(self): f64;`.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodSig {
    /// Its name.
    pub name: Ident,
    /// Its parameters, `self` first.
    pub params: Vec<Param>,
    /// Its return type, if written.
    pub ret: Option<Type>,
    /// The whole declaration.
    pub span: Span,
}

/// An enumeration: a value that is one of its variants, each with its data.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    /// Preceded by `export`.
    pub exported: bool,
    /// The enumeration name.
    pub name: Ident,
    /// Its type parameters, `<T>`; empty when it has none.
    pub generics: Vec<TypeParam>,
    /// Its variants, in order.
    pub variants: Vec<Variant>,
}

/// A variant: `Circle(f64)`, or `Empty` without data.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    /// Its name.
    pub name: Ident,
    /// The types of its data, in order.
    pub fields: Vec<Type>,
}

/// One arm of a `match`: `pattern => body`.
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    /// What the value must look like.
    pub pattern: Pattern,
    /// What the arm gives or does.
    pub body: ArmBody,
    /// The whole arm.
    pub span: Span,
}

/// The body of an arm: a value, or statements.
#[derive(Debug, Clone, PartialEq)]
pub enum ArmBody {
    /// `pattern => value`
    Expr(ExprId),
    /// `pattern => { statements }`
    Block(Block),
}

/// A pattern of a `match` arm.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    /// Its shape.
    pub kind: PatternKind,
    /// Where it is.
    pub span: Span,
}

/// The shapes of pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum PatternKind {
    /// `_`: anything.
    Wildcard,
    /// A name: a variant without data when the value's type has one by that
    /// name, otherwise anything, given that name.
    Name(Ident),
    /// A number or a boolean, possibly negated.
    Literal(ExprId),
    /// `Variant(p, …)` or `Enum.Variant(p, …)`.
    Variant {
        /// The enumeration, when written.
        qualifier: Option<Ident>,
        /// The variant.
        name: Ident,
        /// Patterns for its data.
        fields: Vec<Pattern>,
    },
}

/// A structure declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    /// Preceded by `export`.
    pub exported: bool,
    /// The structure name.
    pub name: Ident,
    /// Its type parameters, `<A, B>`; empty when it has none.
    pub generics: Vec<TypeParam>,
    /// Fields, in order.
    pub fields: Vec<Field>,
}

/// A structure field: `name: Type`.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Its name.
    pub name: Ident,
    /// Its type.
    pub ty: Type,
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
    /// Its type parameters, `<T: Ord>`; empty when it has none.
    pub generics: Vec<TypeParam>,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// From `(` to `)`: a missing return type is inserted after it.
    pub params_span: Span,
    /// The return type, if written.
    pub ret: Option<Type>,
    /// The body.
    pub body: Block,
    /// For a method, the name of the type of its `impl`.
    pub impl_of: Option<Ident>,
}

impl FnDecl {
    /// Whether it is a method that takes `self`.
    #[must_use]
    pub fn has_self(&self) -> bool {
        self.impl_of.is_some() && self.params.first().is_some_and(|p| p.is_self)
    }
}

/// A type parameter: `T`, or `T: Ord + Copy` with its constraints.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    /// Its name.
    pub name: Ident,
    /// The constraints its types must meet, in order.
    pub bounds: Vec<Ident>,
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
    /// Whether it is the `self` of a method.
    pub is_self: bool,
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
    /// `(i32, String)`: two types or more.
    Tuple(Vec<Type>),
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
    /// `{ statements }`
    Block(Block),
    /// `if (cond) then else otherwise`
    If {
        /// The condition.
        cond: ExprId,
        /// Run when it holds; a block or a single statement.
        then: Box<Stmt>,
        /// Run otherwise, if written; another `if` for `else if`.
        otherwise: Option<Box<Stmt>>,
    },
    /// `while (cond) body`
    While {
        /// The condition, checked before each iteration.
        cond: ExprId,
        /// The loop body.
        body: Box<Stmt>,
    },
    /// `for (init; cond; update) body`
    For {
        /// Run once before the loop: `let i = 0` or an assignment.
        init: Option<Box<Stmt>>,
        /// Checked before each iteration; none means forever.
        cond: Option<ExprId>,
        /// Run after each iteration: `i++`.
        update: Option<Box<Stmt>>,
        /// The loop body.
        body: Box<Stmt>,
    },
    /// `for (const item of items) body`
    ForOf {
        /// `let` (true) or `const` (false).
        mutable: bool,
        /// The `let` or `const` keyword.
        keyword: Span,
        /// The loop variable.
        name: Ident,
        /// Its annotation, if written.
        ty: Option<Type>,
        /// What is iterated.
        iterable: ExprId,
        /// The loop body.
        body: Box<Stmt>,
    },
    /// `break;`
    Break,
    /// `continue;`
    Continue,
    /// A statement that failed to parse, already reported.
    Error,
}

/// `let name: Type = value` or `const name = value`.
#[derive(Debug, Clone, PartialEq)]
pub struct Local {
    /// `let` (true) or `const` (false).
    pub mutable: bool,
    /// The `let` or `const` keyword.
    pub keyword: Span,
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
    /// `[a, b, c]`.
    Array(Vec<ExprId>),
    /// `(a, b)`: two values or more.
    Tuple(Vec<ExprId>),
    /// `(x) => body` or `x => body`: an arrow function, its body a value.
    Lambda {
        /// Its parameters, their types when written.
        params: Vec<(Ident, Option<Type>)>,
        /// Its value.
        body: ExprId,
    },
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
        /// Type arguments written, `larger<i64>(…)`; usually inferred.
        types: Vec<Type>,
        /// Arguments, in order.
        args: Vec<ExprId>,
    },
    /// `` `text${value}text` ``: one more piece of text than values, the
    /// pieces decoded, escapes included.
    Template {
        /// The text before, between and after the values.
        parts: Vec<String>,
        /// The values written into it.
        exprs: Vec<ExprId>,
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
    /// `match (value) { pattern => body, … }`
    Match {
        /// The value matched.
        scrutinee: ExprId,
        /// The arms, tried in order.
        arms: Vec<Arm>,
    },
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
                ItemKind::Struct(st) => {
                    let mut line = format!("(struct {}", self.name(st.name.symbol));
                    if st.exported {
                        line.push_str(" export");
                    }
                    for field in &st.fields {
                        let _ = write!(
                            line,
                            " ({} {})",
                            self.name(field.name.symbol),
                            self.ty(&field.ty)
                        );
                    }
                    line.push(')');
                    let _ = writeln!(out, "{line}");
                }
                ItemKind::Enum(en) => {
                    let mut line = format!("(enum {}", self.name(en.name.symbol));
                    if en.exported {
                        line.push_str(" export");
                    }
                    for variant in &en.variants {
                        let fields: Vec<String> =
                            variant.fields.iter().map(|t| self.ty(t)).collect();
                        let _ = write!(
                            line,
                            " ({}{}{})",
                            self.name(variant.name.symbol),
                            if fields.is_empty() { "" } else { " " },
                            fields.join(" ")
                        );
                    }
                    line.push(')');
                    let _ = writeln!(out, "{line}");
                }
                ItemKind::Alias(alias) => {
                    let _ = writeln!(
                        out,
                        "(type {}{} {})",
                        self.name(alias.name.symbol),
                        self.generics(&alias.generics),
                        self.ty(&alias.ty)
                    );
                }
                ItemKind::Interface(interface) => {
                    let _ = writeln!(out, "(interface {}", self.name(interface.name.symbol));
                    for method in &interface.methods {
                        let params: Vec<String> =
                            method.params.iter().map(|p| self.param(p)).collect();
                        let ret = method.ret.as_ref().map(|t| self.ty(t));
                        let _ = writeln!(
                            out,
                            "  (fn {} ({}){})",
                            self.name(method.name.symbol),
                            params.join(" "),
                            ret.map(|r| format!(" -> {r}")).unwrap_or_default()
                        );
                    }
                    let _ = writeln!(out, ")");
                }
                ItemKind::Impl(decl) => {
                    let interface = decl
                        .interface
                        .map(|i| format!("{} for ", self.name(i.symbol)))
                        .unwrap_or_default();
                    let _ = writeln!(
                        out,
                        "(impl{} {interface}{}",
                        self.generics(&decl.generics),
                        self.ty(&decl.target)
                    );
                    for method in &decl.methods {
                        self.dump_fn(&mut out, method);
                    }
                    let _ = writeln!(out, ")");
                }
            }
        }
        out
    }

    /// `<T: Ord, U>`, or nothing.
    fn generics(&self, generics: &[TypeParam]) -> String {
        if generics.is_empty() {
            return String::new();
        }
        let params: Vec<String> = generics
            .iter()
            .map(|g| {
                let mut text = self.name(g.name.symbol).to_owned();
                let bounds: Vec<&str> = g.bounds.iter().map(|b| self.name(b.symbol)).collect();
                if !bounds.is_empty() {
                    let _ = write!(text, ": {}", bounds.join(" + "));
                }
                text
            })
            .collect();
        format!("<{}>", params.join(", "))
    }

    /// A pattern as an S-expression.
    #[must_use]
    pub fn pattern(&self, pattern: &Pattern) -> String {
        match &pattern.kind {
            PatternKind::Wildcard => "_".to_owned(),
            PatternKind::Name(name) => self.name(name.symbol).to_owned(),
            PatternKind::Literal(e) => self.sexpr(*e),
            PatternKind::Variant {
                qualifier,
                name,
                fields,
            } => {
                let mut out = String::from("(");
                if let Some(q) = qualifier {
                    let _ = write!(out, "{}.", self.name(q.symbol));
                }
                out.push_str(self.name(name.symbol));
                for field in fields {
                    let _ = write!(out, " {}", self.pattern(field));
                }
                out.push(')');
                out
            }
        }
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
        head.push_str(&self.generics(&f.generics));
        head.push_str(" (params");
        for p in &f.params {
            let _ = write!(head, " {}", self.param(p));
        }
        head.push(')');
        if let Some(ret) = &f.ret {
            let _ = write!(head, " -> {}", self.ty(ret));
        }
        let _ = writeln!(out, "{head}");
        for stmt in &f.body.stmts {
            self.dump_stmt(out, stmt, 1);
        }
        let _ = writeln!(out, ")");
    }

    /// `(name inout Type)`, or `self` for the receiver of a method.
    fn param(&self, p: &Param) -> String {
        let convention = match p.convention {
            Convention::Let => "",
            Convention::Inout => "inout ",
            Convention::Sink => "sink ",
        };
        if p.is_self {
            return format!("({convention}self)");
        }
        format!(
            "({} {convention}{})",
            self.name(p.name.symbol),
            self.ty(&p.ty)
        )
    }

    fn ty(&self, ty: &Type) -> String {
        match &ty.kind {
            TypeKind::Named { name, args } if args.is_empty() => self.name(name.symbol).to_owned(),
            TypeKind::Named { name, args } => {
                let args: Vec<String> = args.iter().map(|a| self.ty(a)).collect();
                format!("{}<{}>", self.name(name.symbol), args.join(", "))
            }
            TypeKind::Tuple(items) => {
                let items: Vec<String> = items.iter().map(|a| self.ty(a)).collect();
                format!("({})", items.join(", "))
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

    /// One statement per line; nested statements are indented one more level.
    fn dump_stmt(&self, out: &mut String, stmt: &Stmt, level: usize) {
        let pad = "  ".repeat(level);
        let nested = |out: &mut String, head: String, children: &[&Stmt]| {
            let _ = writeln!(out, "{pad}{head}");
            for child in children {
                self.dump_stmt(out, child, level + 1);
            }
            let _ = writeln!(out, "{pad})");
        };
        match &stmt.kind {
            StmtKind::Block(block) => {
                let children: Vec<&Stmt> = block.stmts.iter().collect();
                nested(out, "(block".to_owned(), &children);
            }
            StmtKind::If {
                cond,
                then,
                otherwise,
            } => {
                let mut children = vec![then.as_ref()];
                let head = if let Some(other) = otherwise {
                    children.push(other.as_ref());
                    format!("(if-else {}", self.sexpr(*cond))
                } else {
                    format!("(if {}", self.sexpr(*cond))
                };
                nested(out, head, &children);
            }
            StmtKind::While { cond, body } => {
                nested(
                    out,
                    format!("(while {}", self.sexpr(*cond)),
                    &[body.as_ref()],
                );
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
            } => {
                let part =
                    |s: &Option<Box<Stmt>>| s.as_ref().map_or("_".to_owned(), |s| self.stmt(s));
                let cond = cond.map_or("_".to_owned(), |c| self.sexpr(c));
                let head = format!("(for {} {cond} {}", part(init), part(update));
                nested(out, head, &[body.as_ref()]);
            }
            StmtKind::ForOf {
                mutable,
                keyword: _,
                name,
                ty,
                iterable,
                body,
            } => {
                let keyword = if *mutable { "let" } else { "const" };
                let ty = ty
                    .as_ref()
                    .map(|t| format!(": {}", self.ty(t)))
                    .unwrap_or_default();
                let head = format!(
                    "(for-of ({keyword} {}{ty}) {}",
                    self.name(name.symbol),
                    self.sexpr(*iterable)
                );
                nested(out, head, &[body.as_ref()]);
            }
            _ => {
                let _ = writeln!(out, "{pad}{}", self.stmt(stmt));
            }
        }
    }

    /// A simple statement on one line.
    fn stmt(&self, stmt: &Stmt) -> String {
        match &stmt.kind {
            StmtKind::Break => "(break)".to_owned(),
            StmtKind::Continue => "(continue)".to_owned(),
            StmtKind::Block(_)
            | StmtKind::If { .. }
            | StmtKind::While { .. }
            | StmtKind::For { .. }
            | StmtKind::ForOf { .. } => "(…)".to_owned(),
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
            ExprKind::Array(items) => {
                let items: Vec<String> = items.iter().map(|i| self.sexpr(*i)).collect();
                format!("[{}]", items.join(" "))
            }
            ExprKind::Tuple(items) => {
                let items: Vec<String> = items.iter().map(|i| self.sexpr(*i)).collect();
                format!("(tuple {})", items.join(" "))
            }
            ExprKind::Lambda { params, body } => {
                let params: Vec<String> = params
                    .iter()
                    .map(|(p, t)| match t {
                        Some(t) => format!("{}: {}", self.name(p.symbol), self.ty(t)),
                        None => self.name(p.symbol).to_owned(),
                    })
                    .collect();
                format!("(=> ({}) {})", params.join(" "), self.sexpr(*body))
            }
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
            ExprKind::Call { callee, args, .. } => {
                let mut s = format!("(call {}", self.sexpr(*callee));
                for arg in args {
                    let _ = write!(s, " {}", self.sexpr(*arg));
                }
                s.push(')');
                s
            }
            ExprKind::Template { parts, exprs } => {
                let mut s = String::from("(template");
                for (k, part) in parts.iter().enumerate() {
                    let _ = write!(s, " {part:?}");
                    if let Some(e) = exprs.get(k) {
                        let _ = write!(s, " {}", self.sexpr(*e));
                    }
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
            ExprKind::Match { scrutinee, arms } => {
                let mut out = format!("(match {}", self.sexpr(*scrutinee));
                for arm in arms {
                    let body = match &arm.body {
                        ArmBody::Expr(e) => self.sexpr(*e),
                        ArmBody::Block(block) => {
                            format!("(block {} statements)", block.stmts.len())
                        }
                    };
                    let _ = write!(out, " ({} => {body})", self.pattern(&arm.pattern));
                }
                out.push(')');
                out
            }
            ExprKind::Error => "<error>".to_owned(),
        }
    }
}
