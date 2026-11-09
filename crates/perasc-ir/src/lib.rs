//! Typed, desugared intermediate representation shared by both Peras backends.
//!
//! The IR is what the backends compile: every expression carries its final type,
//! names are resolved to function and local indices, compound assignments and
//! increments are expanded, and every loop is one [`Stmt::Loop`] with an optional
//! condition and a step that `continue` runs. [`lower`] builds it from a checked
//! module; constructs the backends cannot compile yet are reported with `E0900`.
//! [`optimize`] rewrites it without changing any result, and [`verify`] re-checks
//! its types after each step.

use std::fmt::{self, Write as _};

mod bounds;
mod inline;
#[cfg(test)]
mod interp;
mod licm;
mod lower;
mod opt;
mod own;
#[cfg(test)]
mod text;
mod verify;

pub use lower::lower;
pub use opt::optimize;
pub use verify::verify;

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
    /// A heap block with a single owner, who must release it exactly once.
    Owned,
    /// Read-only access to a block someone else owns: only a parameter has this
    /// type, so a reference is never stored.
    Borrowed,
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
            Self::Owned => "owned",
            Self::Borrowed => "borrowed",
        }
    }

    /// `owned` and `borrowed`: handles to heap blocks.
    #[must_use]
    pub const fn is_handle(self) -> bool {
        matches!(self, Self::Owned | Self::Borrowed)
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
    /// The bytes of each string literal, which [`BufOp::Literal`] copies into a
    /// new buffer.
    pub data: Vec<Vec<u8>>,
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
    /// The result type; the first result when there are several.
    pub ret: Ty,
    /// The results after the first: a function returning a structure returns its
    /// fields. Such a function ends with [`Stmt::ReturnMany`], and is only called
    /// by [`Stmt::CallMany`].
    pub more: Vec<Ty>,
    /// For an exported function that takes or returns structures or options:
    /// how JavaScript sees them. `None` when every value is a scalar.
    pub shape: Option<JsShape>,
    /// The body.
    pub body: Vec<Stmt>,
}

/// How JavaScript sees the parameters and result of an exported function.
#[derive(Debug, Clone, PartialEq)]
pub struct JsShape {
    /// Each parameter, by name.
    pub params: Vec<(String, Shape)>,
    /// The result; `None` for `void`.
    pub result: Option<Shape>,
    /// The `inout` parameters whose numbers the function returns after its
    /// result, to be written back into the objects JavaScript passed.
    pub writeback: Vec<(String, Shape)>,
}

/// A value as JavaScript sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// A scalar: one parameter or result of the WebAssembly function.
    Value(Ty),
    /// A structure: its name and its fields, whose scalars follow each other.
    Object(String, Vec<(String, Shape)>),
    /// An `Option<T>`: a `bool` saying whether a value is there, then the
    /// value's scalars. JavaScript sees the value, or `undefined`.
    Option(Box<Shape>),
    /// An `Array<T>`: one buffer handle. JavaScript sees an array, copied in
    /// and, for a result or an `inout` parameter, copied back out.
    Array(Elem, Pass),
    /// A `String`: one buffer of UTF-8 bytes. JavaScript sees a string.
    String(Pass),
    /// An array of structures: one buffer per scalar of an element, its
    /// columns. JavaScript sees an array of objects.
    Records(Box<Shape>, Pass),
    /// An enumeration: its name, and each variant with its data. JavaScript
    /// sees `{ kind: "Variant", values: [...] }`.
    Enum(String, Vec<(String, Vec<Shape>)>),
}

/// How a heap value is passed between JavaScript and WebAssembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// A parameter the function only reads.
    Borrow,
    /// A parameter the function may change: the changes are copied back.
    Inout,
    /// A parameter the function takes and releases.
    Sink,
    /// A result: the caller takes it.
    Result,
}

impl Shape {
    /// The scalars of the value, in order.
    #[must_use]
    pub fn scalars(&self) -> Vec<Ty> {
        match self {
            Self::Value(ty) => vec![*ty],
            Self::Object(_, fields) => fields.iter().flat_map(|(_, f)| f.scalars()).collect(),
            Self::Option(inner) => std::iter::once(Ty::Bool).chain(inner.scalars()).collect(),
            Self::Enum(_, variants) => std::iter::once(Ty::I32)
                .chain(
                    variants
                        .iter()
                        .flat_map(|(_, f)| f.iter().flat_map(Self::scalars)),
                )
                .collect(),
            Self::Array(_, Pass::Sink | Pass::Result) | Self::String(Pass::Sink | Pass::Result) => {
                vec![Ty::Owned]
            }
            Self::Array(..) | Self::String(_) => vec![Ty::Borrowed],
            Self::Records(element, pass) => {
                let handle = if matches!(pass, Pass::Sink | Pass::Result) {
                    Ty::Owned
                } else {
                    Ty::Borrowed
                };
                element.scalars().iter().map(|_| handle).collect()
            }
        }
    }
}

impl Module {
    /// Whether a function allocates, releases or passes heap blocks. The backends
    /// do not compile such code yet: decision 0001 requires the debug allocator
    /// to be linked in first.
    #[must_use]
    pub fn uses_heap(&self) -> bool {
        self.functions.iter().any(Function::uses_heap)
    }
}

impl Function {
    /// Every result type, in order; empty for a `void` function.
    #[must_use]
    pub fn results(&self) -> Vec<Ty> {
        if self.ret == Ty::Void {
            Vec::new()
        } else {
            std::iter::once(self.ret)
                .chain(self.more.iter().copied())
                .collect()
        }
    }

    /// Whether the function has several results, or calls a function that has.
    #[must_use]
    pub fn uses_many(&self) -> bool {
        fn any(stmts: &[Stmt]) -> bool {
            stmts.iter().any(|stmt| match stmt {
                Stmt::ReturnMany(_) | Stmt::CallMany { .. } => true,
                Stmt::If {
                    then, otherwise, ..
                } => any(then) || any(otherwise),
                Stmt::Loop { body, step, .. } => any(body) || any(step),
                Stmt::Block(body) => any(body),
                _ => false,
            })
        }
        !self.more.is_empty() || any(&self.body)
    }

    /// Whether the function allocates, releases or passes heap blocks.
    #[must_use]
    pub fn uses_heap(&self) -> bool {
        self.ret.is_handle() || self.locals.iter().any(|l| l.ty.is_handle())
    }

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
    /// A block that [`Stmt::Leave`] can exit early; it has no other effect.
    Block(Vec<Stmt>),
    /// Jumps past the end of an enclosing [`Stmt::Block`]: `Leave(0)` the innermost
    /// one, `Leave(1)` the one around it. Loops in between are left as well.
    Leave(u32),
    /// Leaves the innermost loop.
    Break,
    /// Goes to the step of the innermost loop.
    Continue,
    /// Returns from the function.
    Return(Option<Expr>),
    /// Releases the block an `owned` local holds; the local is empty afterwards.
    Release(LocalId),
    /// Returns every result of a function with several, in order.
    ReturnMany(Vec<Expr>),
    /// An operation on a buffer: see [`BufStmt`].
    Buf(BufStmt, Vec<Expr>),
    /// Calls a function with several results and stores them, in order.
    CallMany {
        /// One local per result.
        targets: Vec<LocalId>,
        /// The function.
        callee: FnId,
        /// Its arguments.
        args: Vec<Expr>,
    },
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
    /// A new `owned` block of the given number of bytes (an `i32`).
    Alloc(Box<Expr>),
    /// Takes the block out of an `owned` local, which is empty afterwards: to
    /// another local, to a `sink` argument, or as the result.
    Move(LocalId),
    /// Lends the block of an `owned` local, as a `borrowed` argument.
    Borrow(LocalId),
    /// A value computed from a buffer: see [`BufOp`].
    Buf(BufOp, Vec<Expr>),
}

/// How an element is stored in a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Elem {
    /// 4 bytes.
    I32,
    /// 8 bytes.
    I64,
    /// 4 bytes.
    F32,
    /// 8 bytes.
    F64,
    /// 4 bytes, 0 or 1.
    Bool,
    /// 1 byte, read as an `i32` from 0 to 255: the bytes of a string.
    U8,
    /// 4 bytes: a buffer the buffer owns, an element of an array of strings
    /// or of arrays. Read, it is lent (`borrowed`); stored, it is taken
    /// (`owned`), and the element it replaces is released.
    Handle,
}

impl Elem {
    /// Its size in bytes.
    #[must_use]
    pub const fn size(self) -> u32 {
        match self {
            Self::U8 => 1,
            Self::I32 | Self::F32 | Self::Bool | Self::Handle => 4,
            Self::I64 | Self::F64 => 8,
        }
    }

    /// The value type of an element.
    #[must_use]
    pub const fn ty(self) -> Ty {
        match self {
            Self::I32 | Self::U8 => Ty::I32,
            Self::Handle => Ty::Borrowed,
            Self::I64 => Ty::I64,
            Self::F32 => Ty::F32,
            Self::F64 => Ty::F64,
            Self::Bool => Ty::Bool,
        }
    }

    /// The element type holding values of a type, if any.
    #[must_use]
    pub const fn of(ty: Ty) -> Option<Self> {
        Some(match ty {
            Ty::I32 => Self::I32,
            Ty::I64 => Self::I64,
            Ty::F32 => Self::F32,
            Ty::F64 => Self::F64,
            Ty::Bool => Self::Bool,
            Ty::Owned => Self::Handle,
            _ => return None,
        })
    }

    /// The type of a value stored into an element: a handle is taken.
    #[must_use]
    pub const fn stored(self) -> Ty {
        match self {
            Self::Handle => Ty::Owned,
            other => other.ty(),
        }
    }

    /// Its name in the text form.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::Handle => "handle",
            other => other.ty().name(),
        }
    }
}

/// A buffer is a heap value with a length and a capacity, counted in elements,
/// and its elements, contiguous: an `Array<T>` or the bytes of a `String`. Its
/// handle stays the same when it grows. Operands that are buffers are `borrow`
/// expressions or `borrowed` locals, except where [`BufStmt::Release`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufOp {
    /// `[capacity]`: a new, empty `owned` buffer.
    New(Elem),
    /// `[buffer]`: its length, an `i32`.
    Len,
    /// `[buffer, index]`: the element at `index`, which the code has checked is
    /// below the length ([`BufStmt::CheckIndex`]).
    Get(Elem),
    /// `[]`: a new `owned` buffer of bytes holding [`Module::data`] at this index.
    Literal(u32),
    /// `[buffer]`: a new `owned` copy, with copies of the buffers it owns.
    Clone,
    /// `[buffer, index]`: the buffer element at a checked index, taken: the
    /// code then shortens the buffer below it ([`BufStmt::Truncate`]), so
    /// that the buffer no longer owns it. This is `pop` on an array of strings.
    Take,
    /// A function of the runtime on strings, its operands lent; see
    /// [`RuntimeFn`].
    Runtime(RuntimeFn),
}

/// A function of the runtime on strings and arrays of strings, written once
/// in `perasc-runtime/runtime/buffers.wat` and imported by every module that
/// uses the heap. Indices and lengths count bytes, as `length` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeFn {
    /// `(s, needle, from) -> i32`: the first byte index at or after `from`
    /// where `needle` occurs, or -1.
    IndexOf,
    /// `(s, prefix) -> bool`
    StartsWith,
    /// `(s, suffix) -> bool`
    EndsWith,
    /// `(s, start, end) -> owned`: the bytes between, negative counting from
    /// the end, clamped, and moved back to the start of a character.
    Slice,
    /// `(s) -> owned`: without the spaces, tabs and line breaks at both ends.
    Trim,
    /// `(s) -> owned`: ASCII letters in upper case.
    Upper,
    /// `(s) -> owned`: ASCII letters in lower case.
    Lower,
    /// `(s, n) -> owned`: `s` repeated `n` times.
    Repeat,
    /// `(s, from, to) -> owned`: the first `from` replaced by `to`.
    Replace,
    /// `(s, from, to) -> owned`: every `from` replaced by `to`.
    ReplaceAll,
    /// `(s, separator) -> owned`: an array of the parts, which it owns; an
    /// empty separator splits into characters.
    Split,
    /// `(strings, separator) -> owned`: the strings of an array, joined.
    Join,
    /// `(column, start, end) -> owned`: a new column of the elements
    /// between, negative counting from the end, clamped; buffers copied.
    ArrSlice,
    /// `(a, b) -> owned`: a new column of the elements of `a`, then `b`.
    ArrConcat,
    /// `(column)`: reverses its elements in place.
    ArrReverse,
    /// `(column, kind)`: sorts its numbers in increasing order, in place;
    /// `kind` is 0 for `i32`, 1 `i64`, 2 `f32`, 3 `f64`.
    ArrSort,
    /// `(strings, s) -> i32`: the index of the first string equal to `s`, or -1.
    ArrIndexOfStr,
    /// `(n: i64) -> owned`: the decimal digits of `n`, after `-` if negative.
    FmtInt,
    /// `(b: bool) -> owned`: `true` or `false`.
    FmtBool,
    /// `(s, bits) -> bool`: whether `s` is an optional sign then decimal
    /// digits, nothing else, in the range of a signed integer of `bits` bits.
    IsInt,
    /// `(s) -> i64`: the integer `s` writes, which [`Self::IsInt`] accepted.
    ParseInt,
    /// `(column, flags)`: keeps, in place and in order, the elements whose flag
    /// (an array of `bool`) is set, and releases the buffers among the others.
    /// `filter` on a temporary array, which it then gives as its result.
    ArrKeep,
    /// `(s, separator) -> owned`: the parts `Split` gives, as views of the
    /// bytes of `s` in one block. Given only where `s` outlives the array and
    /// does not change meanwhile ([decision 0004]); a copy of it, or of a
    /// part, is an ordinary string.
    ///
    /// [decision 0004]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0004-strings-without-allocation.md
    SplitViews,
}

impl RuntimeFn {
    /// Every function, in the order a module imports them.
    pub const ALL: [Self; 23] = [
        Self::IndexOf,
        Self::StartsWith,
        Self::EndsWith,
        Self::Slice,
        Self::Trim,
        Self::Upper,
        Self::Lower,
        Self::Repeat,
        Self::Replace,
        Self::ReplaceAll,
        Self::Split,
        Self::Join,
        Self::ArrSlice,
        Self::ArrConcat,
        Self::ArrReverse,
        Self::ArrSort,
        Self::ArrIndexOfStr,
        Self::FmtInt,
        Self::FmtBool,
        Self::IsInt,
        Self::ParseInt,
        Self::ArrKeep,
        Self::SplitViews,
    ];

    /// Its name, in the runtime's exports and in the text form.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::IndexOf => "str_index_of",
            Self::StartsWith => "str_starts_with",
            Self::EndsWith => "str_ends_with",
            Self::Slice => "str_slice",
            Self::Trim => "str_trim",
            Self::Upper => "str_upper",
            Self::Lower => "str_lower",
            Self::Repeat => "str_repeat",
            Self::Replace => "str_replace",
            Self::ReplaceAll => "str_replace_all",
            Self::Split => "str_split",
            Self::Join => "arr_join",
            Self::ArrSlice => "arr_slice",
            Self::ArrConcat => "arr_concat",
            Self::ArrReverse => "arr_reverse",
            Self::ArrSort => "arr_sort",
            Self::ArrIndexOfStr => "arr_index_of_str",
            Self::FmtInt => "fmt_int",
            Self::FmtBool => "fmt_bool",
            Self::IsInt => "str_is_int",
            Self::ParseInt => "str_parse_int",
            Self::ArrKeep => "arr_keep",
            Self::SplitViews => "str_split_views",
        }
    }

    /// The type of its first operand: a lent buffer, or the number or
    /// boolean it formats.
    #[must_use]
    pub const fn receiver(self) -> Ty {
        match self {
            Self::FmtInt => Ty::I64,
            Self::FmtBool => Ty::Bool,
            _ => Ty::Borrowed,
        }
    }

    /// The types of its operands after the first.
    #[must_use]
    pub const fn operands(self) -> &'static [Ty] {
        match self {
            Self::IndexOf => &[Ty::Borrowed, Ty::I32],
            Self::StartsWith
            | Self::EndsWith
            | Self::Split
            | Self::SplitViews
            | Self::Join
            | Self::ArrConcat
            | Self::ArrIndexOfStr
            | Self::ArrKeep => &[Ty::Borrowed],
            Self::Slice | Self::ArrSlice => &[Ty::I32, Ty::I32],
            Self::Trim
            | Self::Upper
            | Self::Lower
            | Self::ArrReverse
            | Self::FmtInt
            | Self::FmtBool
            | Self::ParseInt => &[],
            Self::Repeat | Self::ArrSort | Self::IsInt => &[Ty::I32],
            Self::Replace | Self::ReplaceAll => &[Ty::Borrowed, Ty::Borrowed],
        }
    }

    /// The type of its result.
    #[must_use]
    pub const fn result(self) -> Ty {
        match self {
            Self::IndexOf | Self::ArrIndexOfStr => Ty::I32,
            Self::StartsWith | Self::EndsWith | Self::IsInt => Ty::Bool,
            Self::ParseInt => Ty::I64,
            Self::ArrReverse | Self::ArrSort | Self::ArrKeep => Ty::Void,
            _ => Ty::Owned,
        }
    }

    /// Its position among the runtime's imports, after the allocator's.
    #[must_use]
    pub fn position(self) -> u32 {
        Self::ALL
            .iter()
            .position(|&f| f == self)
            .and_then(|i| u32::try_from(i).ok())
            .unwrap_or(0)
    }
}

/// A statement on a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufStmt {
    /// `[buffer, index, value]`: stores the element at a checked index.
    Set(Elem),
    /// `[buffer, value]`: adds an element at the end, growing the buffer.
    Push(Elem),
    /// `[buffer, length]`: sets a length no larger than the current one.
    Truncate,
    /// `[index, length]`: stops the program with an error unless
    /// `0 <= index < length`.
    CheckIndex,
    /// `[move buffer]`: releases the buffer and its elements.
    Release,
    /// `[buffer, other]`: adds the elements of `other` at the end of `buffer`.
    Append(Elem),
}

impl BufOp {
    /// Its name in the text form.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::New(elem) => format!("buf.new.{}", elem.name()),
            Self::Len => "buf.len".to_owned(),
            Self::Get(elem) => format!("buf.get.{}", elem.name()),
            Self::Literal(index) => format!("buf.literal #{index}"),
            Self::Clone => "buf.clone".to_owned(),
            Self::Take => "buf.take".to_owned(),
            Self::Runtime(f) => f.name().to_owned(),
        }
    }
}

impl BufStmt {
    /// Its name in the text form.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Set(elem) => format!("buf.set.{}", elem.name()),
            Self::Push(elem) => format!("buf.push.{}", elem.name()),
            Self::Truncate => "buf.truncate".to_owned(),
            Self::CheckIndex => "check_index".to_owned(),
            Self::Release => "buf.release".to_owned(),
            Self::Append(elem) => format!("buf.append.{}", elem.name()),
        }
    }
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
    /// `a` to the power `b`, on floats.
    Pow,
    /// The angle of the point `(b, a)`, `a.atan2(b)`, on floats.
    Atan2,
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
            Self::Pow => "pow",
            Self::Atan2 => "atan2",
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
    /// `e` to the power of the operand.
    Exp,
    /// The natural logarithm.
    Log,
    /// Sine, in radians.
    Sin,
    /// Cosine, in radians.
    Cos,
    /// Tangent, in radians.
    Tan,
    /// Rounds to the nearest; halves up, as JavaScript's `Math.round`.
    Round,
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
            Self::Exp => "exp",
            Self::Log => "log",
            Self::Sin => "sin",
            Self::Cos => "cos",
            Self::Tan => "tan",
            Self::Round => "round",
        }
    }

    /// The function of JavaScript's `Math` that computes it, for those
    /// WebAssembly has no instruction for.
    #[must_use]
    pub const fn host(self) -> Option<HostMath> {
        Some(match self {
            Self::Exp => HostMath::Exp,
            Self::Log => HostMath::Log,
            Self::Sin => HostMath::Sin,
            Self::Cos => HostMath::Cos,
            Self::Tan => HostMath::Tan,
            Self::Round => HostMath::Round,
            _ => return None,
        })
    }
}

/// A function of JavaScript's `Math` that a module imports, on `f64`: the
/// functions of numbers WebAssembly has no instruction for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HostMath {
    /// `Math.exp`
    Exp,
    /// `Math.log`
    Log,
    /// `Math.sin`
    Sin,
    /// `Math.cos`
    Cos,
    /// `Math.tan`
    Tan,
    /// `Math.round`
    Round,
    /// `Math.pow`, with two operands.
    Pow,
    /// `Math.atan2`, with two operands.
    Atan2,
}

impl HostMath {
    /// Its name in `Math`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exp => "exp",
            Self::Log => "log",
            Self::Sin => "sin",
            Self::Cos => "cos",
            Self::Tan => "tan",
            Self::Round => "round",
            Self::Pow => "pow",
            Self::Atan2 => "atan2",
        }
    }

    /// Whether it takes two operands.
    #[must_use]
    pub const fn binary(self) -> bool {
        matches!(self, Self::Pow | Self::Atan2)
    }
}

impl Module {
    /// The functions of `Math` the module calls, each once, in order.
    #[must_use]
    pub fn host_math(&self) -> Vec<HostMath> {
        let mut found = std::collections::BTreeSet::new();
        for f in &self.functions {
            host_in_stmts(&f.body, &mut found);
        }
        found.into_iter().collect()
    }
}

fn host_in_stmts(stmts: &[Stmt], found: &mut std::collections::BTreeSet<HostMath>) {
    for stmt in stmts {
        match stmt {
            Stmt::Set(_, e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => host_in_expr(e, found),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                host_in_expr(cond, found);
                host_in_stmts(then, found);
                host_in_stmts(otherwise, found);
            }
            Stmt::Loop { cond, body, step } => {
                if let Some(cond) = cond {
                    host_in_expr(cond, found);
                }
                host_in_stmts(body, found);
                host_in_stmts(step, found);
            }
            Stmt::Block(body) => host_in_stmts(body, found),
            Stmt::ReturnMany(values)
            | Stmt::Buf(_, values)
            | Stmt::CallMany { args: values, .. } => {
                for e in values {
                    host_in_expr(e, found);
                }
            }
            Stmt::Leave(_)
            | Stmt::Break
            | Stmt::Continue
            | Stmt::Return(None)
            | Stmt::Release(_) => {}
        }
    }
}

fn host_in_expr(e: &Expr, found: &mut std::collections::BTreeSet<HostMath>) {
    match &e.kind {
        ExprKind::Math(op, x) => {
            if let Some(h) = op.host() {
                found.insert(h);
            }
            host_in_expr(x, found);
        }
        ExprKind::Binary(op, a, b) => {
            match op {
                BinOp::Pow => {
                    found.insert(HostMath::Pow);
                }
                BinOp::Atan2 => {
                    found.insert(HostMath::Atan2);
                }
                _ => {}
            }
            host_in_expr(a, found);
            host_in_expr(b, found);
        }
        ExprKind::Unary(_, x) | ExprKind::Convert(x) | ExprKind::Alloc(x) => host_in_expr(x, found),
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) => {
            for a in args {
                host_in_expr(a, found);
            }
        }
        ExprKind::Const(_) | ExprKind::Local(_) | ExprKind::Move(_) | ExprKind::Borrow(_) => {}
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
            let ret = if function.more.is_empty() {
                function.ret.name().to_owned()
            } else {
                let all: Vec<&str> = function.results().iter().map(|t| t.name()).collect();
                format!("({})", all.join(", "))
            };
            writeln!(
                f,
                "{export}fn {}({}) -> {ret}",
                function.name,
                params.join(", "),
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
            Stmt::Block(body) => {
                let _ = writeln!(out, "{pad}block");
                write_stmts(out, body, level + 1);
            }
            Stmt::Leave(depth) => {
                let _ = writeln!(out, "{pad}leave {depth}");
            }
            Stmt::Release(local) => {
                let _ = writeln!(out, "{pad}release %{}", local.0);
            }
            Stmt::Buf(op, args) => {
                let args: Vec<String> = args.iter().map(expr).collect();
                let _ = writeln!(out, "{pad}{} {}", op.name(), args.join(" "));
            }
            Stmt::ReturnMany(values) => {
                let values: Vec<String> = values.iter().map(expr).collect();
                let _ = writeln!(out, "{pad}return {}", values.join(", "));
            }
            Stmt::CallMany {
                targets,
                callee,
                args,
            } => {
                let targets: Vec<String> = targets.iter().map(|t| format!("%{}", t.0)).collect();
                let args: Vec<String> = args.iter().map(expr).collect();
                let _ = writeln!(
                    out,
                    "{pad}{} = call #{} {}",
                    targets.join(", "),
                    callee.0,
                    args.join(" ")
                );
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
        ExprKind::Alloc(size) => format!("(alloc {})", expr(size)),
        ExprKind::Move(l) => format!("(move %{})", l.0),
        ExprKind::Borrow(l) => format!("(borrow %{})", l.0),
        ExprKind::Buf(op, args) => {
            let args: Vec<String> = args.iter().map(expr).collect();
            format!("({} {})", op.name(), args.join(" "))
        }
    }
}

#[cfg(test)]
mod array_tests;
#[cfg(test)]
mod functional_tests;
#[cfg(test)]
mod generic_tests;
#[cfg(test)]
mod match_tests;
#[cfg(test)]
mod method_tests;
#[cfg(test)]
mod opt_tests;
#[cfg(test)]
mod option_tests;
#[cfg(test)]
mod own_tests;
#[cfg(test)]
mod string_tests;
#[cfg(test)]
mod struct_tests;
