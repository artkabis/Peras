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
/// The interpreter of the IR, the oracle of the tests: what a module gives,
/// computed without any backend.
#[doc(hidden)]
pub mod interp;
mod licm;
mod lookups;
mod lower;
mod opt;
mod own;
pub mod text;
mod verify;

pub use lower::{Unit, lower, lower_units, lower_with};
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

/// The static environment of no function ([`EnvOp::Zero`]): address 0 is
/// never a valid pointer, and the 16 bytes after are this header.
pub const ZERO_ENV: u32 = 16;
/// The first address of the static literals' headers and static
/// environments, after [`ZERO_ENV`].
pub const STATIC_START: u32 = 32;
/// The end of the static literals: both allocators leave the memory below
/// 1024 unused.
pub const STATIC_END: u32 = 1024;

/// A compiled module.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    /// Functions, in source order.
    pub functions: Vec<Function>,
    /// The module's variables, by their [`GlobalId`]: one per scalar of each
    /// module-level `let` (decision 0023).
    pub globals: Vec<Global>,
    /// The bytes of each string literal, which [`BufOp::Literal`] copies into a
    /// new buffer.
    pub data: Vec<Vec<u8>>,
}

/// A module variable: one scalar of a module-level `let`, kept between
/// calls, its initial value a constant (decision 0023).
#[derive(Debug, Clone, PartialEq)]
pub struct Global {
    /// Its name in source, a field's path after it: `origin_x`.
    pub name: String,
    /// Its type: a number or a boolean.
    pub ty: Ty,
    /// Its value before any call.
    pub init: Const,
}

/// The number of a module variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlobalId(pub u32);

/// A function.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    /// Its name in source, also its export name.
    pub name: String,
    /// Whether JavaScript can call it.
    pub exported: bool,
    /// Whether it is the body of a `parallel for`, `(start, end, captures…)`,
    /// which a thread may run on its own range of turns.
    pub job: bool,
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

/// An integer type other than `i32` and `i64`, as JavaScript sees it: the
/// loader wraps a value given to its type, and gives a `u32` or a `u64` back
/// non-negative (decision 0024).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntKind {
    /// `i8`
    I8,
    /// `i16`
    I16,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `u64`, a `BigInt` in JavaScript.
    U64,
}

impl IntKind {
    /// The IR type that keeps it.
    #[must_use]
    pub const fn ty(self) -> Ty {
        if matches!(self, Self::U64) {
            Ty::I64
        } else {
            Ty::I32
        }
    }
}

/// A value as JavaScript sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// A scalar: one parameter or result of the WebAssembly function.
    Value(Ty),
    /// An integer of another type than `i32` and `i64`: one scalar, which
    /// the loader converts each way.
    Int(IntKind),
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
    /// A `Map<K, V>` or a `Set<K>`: one handle. JavaScript sees a `Map` or a
    /// `Set`, copied in insertion order, in and out.
    Map(Box<MapShape>),
}

/// A map as JavaScript sees it: what the loader needs to copy it.
#[derive(Debug, Clone, PartialEq)]
pub struct MapShape {
    /// Its key: an integer, a `bool`, or a string.
    pub key: Shape,
    /// Its value; `None` for a set.
    pub value: Option<Shape>,
    /// The shape the runtime makes it with: its value slots, and those that
    /// own a buffer.
    pub slots: i64,
    /// How it is passed.
    pub pass: Pass,
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
            Self::Int(kind) => vec![kind.ty()],
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
            Self::Map(map) => vec![if matches!(map.pass, Pass::Sink | Pass::Result) {
                Ty::Owned
            } else {
                Ty::Borrowed
            }],
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
    /// to be linked in first. Static environments alone are not the heap:
    /// see [`Module::static_envs_only`].
    #[must_use]
    pub fn uses_heap(&self) -> bool {
        (self.functions.iter().any(Function::uses_heap) || !self.statics().is_empty())
            && !self.static_envs_only()
    }

    /// The static literals the module lends ([`BufOp::Static`]): each one's
    /// index in [`Module::data`] and header address, once, by address.
    #[must_use]
    pub fn statics(&self) -> Vec<(u32, u32)> {
        let mut found = std::collections::BTreeMap::new();
        for f in &self.functions {
            each_expr(&f.body, &mut |e| {
                if let ExprKind::Buf(BufOp::Static(index, at), _) = &e.kind {
                    found.insert(*at, *index);
                }
            });
        }
        found.into_iter().map(|(at, index)| (index, at)).collect()
    }

    /// Whether the module is linked to the runtime: it uses the heap, or, in
    /// a debug build, it reports an integer overflow through the runtime's
    /// imports, as an index out of bounds is. Its declarations for TypeScript
    /// follow [`Module::uses_heap`] alone, the same in every build.
    #[must_use]
    pub fn needs_runtime(&self) -> bool {
        self.uses_heap()
            || self.functions.iter().any(Function::checks_overflow)
            || self.calls_runtime()
            || self.checks_bounds()
    }

    /// Whether a function checks an index without any buffer, which reports
    /// through the runtime's `bounds`: `unwrap()` of decision 0028's option
    /// 2d, prototyped.
    #[must_use]
    pub fn checks_bounds(&self) -> bool {
        let mut found = false;
        for f in &self.functions {
            each_stmt(&f.body, &mut |s| {
                if matches!(s, Stmt::Buf(BufStmt::CheckIndex, _)) {
                    found = true;
                }
            });
        }
        found
    }

    /// Whether a function calls the runtime, as `%` on floats does with no
    /// buffer at all (decision 0021).
    #[must_use]
    pub fn calls_runtime(&self) -> bool {
        let mut found = false;
        for f in &self.functions {
            each_expr(&f.body, &mut |e| {
                if matches!(e.kind, ExprKind::Buf(BufOp::Runtime(_), _)) {
                    found = true;
                }
            });
        }
        found
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

    /// Whether the function checks an integer for overflow, in a debug build.
    #[must_use]
    pub fn checks_overflow(&self) -> bool {
        fn checks(stmts: &[Stmt]) -> bool {
            stmts.iter().any(|stmt| match stmt {
                Stmt::Buf(BufStmt::CheckOverflow, _) => true,
                Stmt::If {
                    then, otherwise, ..
                } => checks(then) || checks(otherwise),
                Stmt::Loop { body, step, .. } => checks(body) || checks(step),
                Stmt::Block(body) => checks(body),
                _ => false,
            })
        }
        checks(&self.body)
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
    /// `variable = value`, for a module variable.
    SetGlobal(GlobalId, Expr),
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
    /// Calls the function a value names (decision 0025) and stores its
    /// results, in order: [`Self::CallMany`] through a function value.
    CallManyIndirect {
        /// One local per result.
        targets: Vec<LocalId>,
        /// The function value, an environment lent: see [`ExprKind::Env`].
        callee: Expr,
        /// Its arguments, lent: a function type has no `sink` parameter.
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

/// The environments of function values (decision 0026). A function value is
/// one handle, its environment: a block that holds the position of its
/// function in [`Module::fn_table`], how many hold it, and the values it
/// captured, one `i64` slot each, written when it is made and never after.
/// A copy of the value adds to the count ([`BufOp::Clone`]), a release takes
/// from it ([`BufStmt::Release`]), and the last one frees the block. A
/// static environment, written when the module is instantiated, has no
/// count: neither changes it, and it is never freed. Every function in the
/// table takes its environment as its last parameter, lent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvOp {
    /// `[]`: the static environment of a function that captures nothing, an
    /// `owned` value that costs no allocation, at this address below the
    /// heap ([`Module::static_envs`]).
    Static(FnId, u32),
    /// `[slots…]`: a new environment of this function, counted once,
    /// holding the slots in order: each an `i64`, or a buffer's handle. The
    /// first slots, as many as the number given, are `owned` buffers the
    /// environment releases with its last holder; a `borrowed` one is lent
    /// to it (decision 0027, prototypes).
    New(FnId, u32),
    /// `[slots…]`: a shared value, `Rc.new(v)` (decision 0032, prototype):
    /// the same counted block as [`Self::New`], of no function, its slots
    /// the value's, the `owned` buffers and shared values first.
    Shared(u32),
    /// `[]`: the environment of no function, held by a `None` or an empty
    /// variant; it is static, at [`ZERO_ENV`], and is never called.
    Zero,
    /// `[env]`, lent: the slot at this index, an `i64`.
    Load(u32),
    /// `[env]`, lent: the buffer at this slot, lent in turn.
    LoadHandle(u32),
    /// `[env, value]`, the environment lent: stores the `i64` at this slot,
    /// a field written through a shared value (decision 0032, prototype).
    Store(u32),
    /// `[env, value]`, the environment lent, the value `owned`: stores the
    /// buffer at this slot and releases the one it held.
    StoreHandle(u32),
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
    /// Reads a module variable, which any call may have changed.
    Global(GlobalId),
    /// A value computed from a buffer: see [`BufOp`].
    Buf(BufOp, Vec<Expr>),
    /// A function value, its environment (decision 0026): see [`EnvOp`].
    Env(EnvOp, Vec<Expr>),
    /// A call to the function a value names, with one result or none: the
    /// arguments given here, then its environment, lent, the function's last
    /// argument, evaluated after the others. The signature is that of the
    /// arguments, the environment and the expression; WebAssembly checks it
    /// when it calls.
    CallIndirect(Box<Expr>, Vec<Expr>),
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
    /// 1 byte, read as an `i32` from 0 to 255: the bytes of a string, or a
    /// `u8`.
    U8,
    /// 1 byte, read as an `i32` from -128 to 127: an `i8` (decision 0024).
    I8,
    /// 2 bytes, read sign-extended: an `i16`.
    I16,
    /// 2 bytes, read zero-extended: a `u16`.
    U16,
    /// 4 bytes, as [`Self::I32`], that JavaScript reads unsigned: a `u32`.
    U32,
    /// 8 bytes, as [`Self::I64`], that JavaScript reads unsigned: a `u64`.
    U64,
    /// 4 bytes: a buffer the buffer owns, an element of an array of strings
    /// or of arrays. Read, it is lent (`borrowed`); stored, it is taken
    /// (`owned`), and the element it replaces is released.
    Handle,
}

impl Elem {
    /// Its kind in a shape of [`RuntimeFn::BufEqual`].
    #[must_use]
    pub const fn shape(self) -> u32 {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::I32 | Self::U32 => 2,
            Self::I64 | Self::U64 => 3,
            Self::F32 => 4,
            Self::F64 => 5,
            Self::Bool => 6,
            Self::Handle => 7,
            Self::I16 | Self::U16 => 8,
        }
    }

    /// Its size in bytes.
    #[must_use]
    pub const fn size(self) -> u32 {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 | Self::Bool | Self::Handle => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }

    /// The value type of an element.
    #[must_use]
    pub const fn ty(self) -> Ty {
        match self {
            Self::I32 | Self::U8 | Self::I8 | Self::I16 | Self::U16 | Self::U32 => Ty::I32,
            Self::Handle => Ty::Borrowed,
            Self::I64 | Self::U64 => Ty::I64,
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

    /// The element type of an array of one of decision 0024's integers.
    #[must_use]
    pub const fn of_int(kind: IntKind) -> Self {
        match kind {
            IntKind::I8 => Self::I8,
            IntKind::I16 => Self::I16,
            IntKind::U8 => Self::U8,
            IntKind::U16 => Self::U16,
            IntKind::U32 => Self::U32,
            IntKind::U64 => Self::U64,
        }
    }

    /// The kind of the runtime's sorts (`arr_sort`, `arr_sort_index`): the
    /// low two bits 0 for `i32`, 1 `i64`, 2 `f32`, 3 `f64`; 4 added for an
    /// unsigned integer; 8 for elements of one byte, 16 for two, which the
    /// runtime widens to an `i32` to sort them. Strings are 32.
    #[must_use]
    pub const fn sort_kind(self) -> i64 {
        match self {
            Self::I32 | Self::Bool => 0,
            Self::I64 => 1,
            Self::F32 => 2,
            Self::F64 => 3,
            Self::U32 => 4,
            Self::U64 => 5,
            Self::I8 => 8,
            Self::U8 => 8 | 4,
            Self::I16 => 16,
            Self::U16 => 16 | 4,
            Self::Handle => 32,
        }
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
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
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
    /// `[]`: a `borrowed` buffer holding [`Module::data`] at the first index,
    /// whose 16-byte header is at the address given, in memory below the
    /// allocators' heap ([`STATIC_START`] to [`STATIC_END`]), written once
    /// when the module is instantiated: a literal only read, neither copied
    /// nor released (decision 0016, finding F211). Releasing it is an invalid
    /// release, which both allocators catch.
    Static(u32, u32),
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
    /// `(keys, kind, desc) -> owned`: the indices of the keys in the order
    /// of `sortBy`, an array of `i32`, stable; `kind` is 0 for `i32`, 1
    /// `i64`, 2 `f32`, 3 `f64`, 4 strings; `desc` reverses the order of the
    /// keys, not that of equal ones.
    ArrSortIndex,
    /// `(column, order)`: puts the element at `order[i]` at `i`, in place.
    ArrPermute,
    /// `(n: i64) -> owned`: the decimal digits of `n`, after `-` if negative.
    FmtInt,
    /// `(b: bool) -> owned`: `true` or `false`.
    FmtBool,
    /// `(n: i64) -> i32`: the number of bytes [`Self::FmtInt`] writes for
    /// `n`, its sign included (decision 0017).
    FmtIntLen,
    /// `(x: f64, y: f64) -> f64`: the remainder of `x / y` with the sign of
    /// `x`, exactly, as JavaScript's `%` and C's `fmod` give it; WebAssembly
    /// has no instruction for it (decision 0021, finding F304).
    FloatRem,
    /// `(a, b, shape: i32) -> bool`: whether two buffers hold the same
    /// elements, by `==` on each: bytes and integers by value, floats as
    /// floats compare, a handle's buffer by the same rule, one level down.
    /// `shape` says the kind of the elements in its low four bits, then the
    /// kind of a handle's elements in the next four, and so on: 1 a byte,
    /// 2 an `i32`, 3 an `i64`, 4 an `f32`, 5 an `f64`, 6 a `bool`, 7 a
    /// handle (decision 0021, finding F305).
    BufEqual,
    /// `(s, n: i64)`: the digits of `n` appended to the string `s`, as
    /// [`Self::FmtInt`] writes them, without a string of their own: a
    /// template's integers (decision 0017).
    FmtIntAppend,
    /// `(n: i64) -> owned`: the decimal digits of `n` read as unsigned, a
    /// `u64` (decision 0024).
    FmtUint,
    /// `(n: i64) -> i32`: the number of bytes [`Self::FmtUint`] writes.
    FmtUintLen,
    /// `(s, n: i64)`: the digits of `n`, read as unsigned, appended to `s`.
    FmtUintAppend,
    /// `(strings: i32, shape: i32) -> owned`: a new, empty map or set
    /// ([decision 0005]); `strings` when its keys are strings it owns;
    /// `shape` the number of value slots in its low byte, and above it one
    /// bit per slot that holds a buffer the map owns.
    ///
    /// [decision 0005]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0005-maps-and-sets.md
    MapNew,
    /// `(map, key: i64) -> i32`: the entry of an integer key, or -1.
    MapFindI,
    /// `(map, key) -> i32`: the entry of a string key, lent, or -1.
    MapFindS,
    /// `(map, key: i64) -> i32`: the entry of an integer key, made when it
    /// is not there; the buffers its old value owned are released, for
    /// [`Self::MapStore`] to store the new one.
    MapPutI,
    /// `(map, key: owned) -> i32`: [`Self::MapPutI`] for a string key; the
    /// map takes the key, and releases it when the key was already there.
    MapPutS,
    /// `(map, key) -> i32`: [`Self::MapPutS`] with the key lent: the map
    /// keeps a copy of it only when the key is new.
    MapPutSLent,
    /// `(map, entry, key: i64) -> i32`: [`Self::MapPutI`] when a lookup of
    /// the same key in the same map, with nothing changed since, gave
    /// `entry`: that entry's old value released without looking the key up
    /// again, or the key put as usual when `entry` is -1 (decision 0017).
    /// The runtime checks that `entry` holds the key, and looks it up when
    /// it does not.
    MapPutAtI,
    /// `(map, entry, key: owned) -> i32`: [`Self::MapPutAtI`] for a string
    /// key, which the map takes, as [`Self::MapPutS`] does.
    MapPutAtS,
    /// `(map, entry, key) -> i32`: [`Self::MapPutAtS`] with the key lent, as
    /// [`Self::MapPutSLent`]: the map keeps a copy of it only when it is new.
    MapPutAtSLent,
    /// `(map, entry, slot, value: i64)`: stores a scalar of an entry's value.
    MapStore,
    /// `(map, entry, slot, value: f64)`: [`Self::MapStore`] for a float.
    MapStoreF,
    /// `(map, entry, slot, value: owned)`: [`Self::MapStore`] for a buffer,
    /// which the map then owns.
    MapStoreH,
    /// `(map, entry, slot) -> i64`: a scalar of the value of an entry
    /// [`Self::MapFindI`] or [`Self::MapFindS`] gave.
    MapValue,
    /// `(map, entry, slot) -> f64`: [`Self::MapValue`] for a float.
    MapValueF,
    /// `(map, entry, slot) -> borrowed`: [`Self::MapValue`] for a buffer, lent.
    MapValueH,
    /// `(map, key: i64) -> bool`: removes an integer key, whether it was there.
    MapDeleteI,
    /// `(map, key) -> bool`: removes a string key, lent; the map's own copy
    /// of it is released.
    MapDeleteS,
    /// `(map)`: removes every entry.
    MapClear,
    /// `(map, entry) -> i32`: the first live entry from `entry` on, or -1.
    MapNext,
    /// `(map, entry) -> i64`: the integer key of an entry.
    MapKeyI,
    /// `(map, entry) -> borrowed`: the string key of an entry, lent.
    MapKeyS,
    /// `(x: f64, single) -> owned`: `x` as JavaScript's `String()` writes it;
    /// with `single` set, `x` is an `f32`, written with the fewest digits
    /// that read back as it ([`text::number_text`]).
    FmtFloat,
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
    /// `(s) -> owned`: a view of `s` to walk its parts one at a time, empty
    /// at first: one header whose bytes are `s`'s own, released alone
    /// (decision 0014, `for (const x of s.split(sep))` without an array).
    StrCursor,
    /// `(s, separator, cursor, from) -> i32`: points `cursor` at the part of
    /// `s` from `from` to the next `separator`, or to its end; gives where
    /// the next part starts, or -1 when this one was the last. The parts are
    /// those of `Split`, the last empty one included.
    NextPart,
    /// `(s, separator, cursor, from) -> i32`: [`Self::NextPart`] when the
    /// separator is the empty literal, each part a character: a function
    /// small enough for the engine to inline (decision 0016).
    NextChar,
    /// `(s) -> f64`: the decimal number `s` writes, correctly rounded, as
    /// `parseF64` reads it ([`text::is_decimal`]); NaN when it is not one,
    /// which no text it accepts can write.
    ParseFloat,
    /// `(bytes, shared: owned) -> owned`: a shared value of `bytes` bytes,
    /// header included, that only this holder holds: the same block when it
    /// is the only one, otherwise a copy, the buffers it owns copied and the
    /// shared values it holds held once more, and one fewer holding the
    /// original (decision 0032, prototype r1).
    SharedUnique,
    /// `(bytes, shared, slot)`: the shared value held at this slot of
    /// another, [`Self::SharedUnique`] where it is held (decision 0032).
    SharedUniqueSlot,
    /// `(bytes, column, index)`: the shared value held at this element of a
    /// column of handles, [`Self::SharedUnique`] where it is held.
    SharedUniqueElem,
}

impl RuntimeFn {
    /// Every function, in the order a module imports them.
    pub const ALL: [Self; 61] = [
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
        Self::FmtFloat,
        Self::MapNew,
        Self::MapFindI,
        Self::MapFindS,
        Self::MapPutI,
        Self::MapPutS,
        Self::MapPutSLent,
        Self::MapStore,
        Self::MapStoreF,
        Self::MapStoreH,
        Self::MapValue,
        Self::MapValueF,
        Self::MapValueH,
        Self::MapDeleteI,
        Self::MapDeleteS,
        Self::MapClear,
        Self::MapNext,
        Self::MapKeyI,
        Self::MapKeyS,
        Self::ArrSortIndex,
        Self::ArrPermute,
        Self::StrCursor,
        Self::NextPart,
        Self::NextChar,
        Self::ParseFloat,
        Self::MapPutAtI,
        Self::MapPutAtS,
        Self::MapPutAtSLent,
        Self::FmtIntLen,
        Self::FmtIntAppend,
        Self::FloatRem,
        Self::BufEqual,
        Self::FmtUint,
        Self::FmtUintLen,
        Self::FmtUintAppend,
        Self::SharedUnique,
        Self::SharedUniqueSlot,
        Self::SharedUniqueElem,
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
            Self::ArrSortIndex => "arr_sort_index",
            Self::ArrPermute => "arr_permute",
            Self::FmtInt => "fmt_int",
            Self::FmtBool => "fmt_bool",
            Self::IsInt => "str_is_int",
            Self::ParseInt => "str_parse_int",
            Self::ArrKeep => "arr_keep",
            Self::SplitViews => "str_split_views",
            Self::StrCursor => "str_cursor",
            Self::NextPart => "str_next_part",
            Self::NextChar => "str_next_char",
            Self::ParseFloat => "str_parse_float",
            Self::FmtFloat => "fmt_float",
            Self::MapNew => "map_new",
            Self::MapFindI => "map_find_i",
            Self::MapFindS => "map_find_s",
            Self::MapPutI => "map_put_i",
            Self::MapPutS => "map_put_s",
            Self::MapPutSLent => "map_put_s_lent",
            Self::MapPutAtI => "map_put_at_i",
            Self::MapPutAtS => "map_put_at_s",
            Self::MapPutAtSLent => "map_put_at_s_lent",
            Self::FmtIntLen => "fmt_int_len",
            Self::FmtIntAppend => "fmt_int_append",
            Self::FmtUint => "fmt_uint",
            Self::FmtUintLen => "fmt_uint_len",
            Self::FmtUintAppend => "fmt_uint_append",
            Self::FloatRem => "f64_rem",
            Self::BufEqual => "buf_equal",
            Self::MapStore => "map_store",
            Self::MapStoreF => "map_store_f",
            Self::MapStoreH => "map_store_h",
            Self::MapValue => "map_value",
            Self::MapValueF => "map_value_f",
            Self::MapValueH => "map_value_h",
            Self::MapDeleteI => "map_delete_i",
            Self::MapDeleteS => "map_delete_s",
            Self::MapClear => "map_clear",
            Self::MapNext => "map_next",
            Self::MapKeyI => "map_key_i",
            Self::MapKeyS => "map_key_s",
            Self::SharedUnique => "shared_unique",
            Self::SharedUniqueSlot => "shared_unique_slot",
            Self::SharedUniqueElem => "shared_unique_elem",
        }
    }

    /// The type of its first operand: a lent buffer, or the number or
    /// boolean it formats.
    #[must_use]
    pub const fn receiver(self) -> Ty {
        match self {
            Self::FmtInt | Self::FmtIntLen | Self::FmtUint | Self::FmtUintLen => Ty::I64,
            Self::FmtBool => Ty::Bool,
            Self::FmtFloat | Self::FloatRem => Ty::F64,
            Self::MapNew | Self::SharedUnique | Self::SharedUniqueSlot | Self::SharedUniqueElem => {
                Ty::I32
            }
            _ => Ty::Borrowed,
        }
    }

    /// The types of its operands after the first.
    #[must_use]
    pub const fn operands(self) -> &'static [Ty] {
        match self {
            Self::NextPart | Self::NextChar => &[Ty::Borrowed, Ty::Borrowed, Ty::I32],
            Self::IndexOf | Self::BufEqual | Self::SharedUniqueSlot | Self::SharedUniqueElem => {
                &[Ty::Borrowed, Ty::I32]
            }
            Self::StartsWith
            | Self::EndsWith
            | Self::Split
            | Self::SplitViews
            | Self::Join
            | Self::ArrConcat
            | Self::ArrIndexOfStr
            | Self::ArrKeep
            | Self::MapFindS
            | Self::MapDeleteS
            | Self::MapPutSLent
            | Self::ArrPermute => &[Ty::Borrowed],
            Self::Slice
            | Self::ArrSlice
            | Self::ArrSortIndex
            | Self::MapValue
            | Self::MapValueF
            | Self::MapValueH => &[Ty::I32, Ty::I32],
            Self::Trim
            | Self::FmtIntLen
            | Self::FmtUintLen
            | Self::FmtUint
            | Self::StrCursor
            | Self::ParseFloat
            | Self::Upper
            | Self::Lower
            | Self::ArrReverse
            | Self::FmtInt
            | Self::FmtBool
            | Self::ParseInt
            | Self::MapClear => &[],
            Self::Repeat
            | Self::ArrSort
            | Self::IsInt
            | Self::FmtFloat
            | Self::MapNew
            | Self::MapNext
            | Self::MapKeyI
            | Self::MapKeyS => &[Ty::I32],
            Self::Replace | Self::ReplaceAll => &[Ty::Borrowed, Ty::Borrowed],
            Self::MapFindI
            | Self::MapDeleteI
            | Self::MapPutI
            | Self::FmtIntAppend
            | Self::FmtUintAppend => &[Ty::I64],
            Self::MapPutS | Self::SharedUnique => &[Ty::Owned],
            Self::MapPutAtI => &[Ty::I32, Ty::I64],
            Self::MapPutAtS => &[Ty::I32, Ty::Owned],
            Self::MapPutAtSLent => &[Ty::I32, Ty::Borrowed],
            Self::FloatRem => &[Ty::F64],
            Self::MapStore => &[Ty::I32, Ty::I32, Ty::I64],
            Self::MapStoreF => &[Ty::I32, Ty::I32, Ty::F64],
            Self::MapStoreH => &[Ty::I32, Ty::I32, Ty::Owned],
        }
    }

    /// The type of its result.
    #[must_use]
    pub const fn result(self) -> Ty {
        match self {
            Self::IndexOf
            | Self::NextPart
            | Self::NextChar
            | Self::ArrIndexOfStr
            | Self::MapFindI
            | Self::MapFindS
            | Self::MapNext
            | Self::MapPutI
            | Self::MapPutS
            | Self::MapPutSLent
            | Self::MapPutAtI
            | Self::MapPutAtS
            | Self::MapPutAtSLent
            | Self::FmtIntLen
            | Self::FmtUintLen => Ty::I32,
            Self::StartsWith
            | Self::EndsWith
            | Self::IsInt
            | Self::MapDeleteI
            | Self::MapDeleteS
            | Self::BufEqual => Ty::Bool,
            Self::ParseInt | Self::MapValue | Self::MapKeyI => Ty::I64,
            Self::MapKeyS | Self::MapValueH => Ty::Borrowed,
            Self::MapValueF | Self::ParseFloat | Self::FloatRem => Ty::F64,
            Self::ArrReverse
            | Self::ArrSort
            | Self::ArrPermute
            | Self::ArrKeep
            | Self::MapStore
            | Self::MapStoreF
            | Self::MapStoreH
            | Self::MapClear
            | Self::FmtIntAppend
            | Self::FmtUintAppend
            | Self::SharedUniqueSlot
            | Self::SharedUniqueElem => Ty::Void,
            _ => Ty::Owned,
        }
    }

    /// Whether it makes a shared value its holder's own (decision 0032).
    #[must_use]
    pub const fn on_shared(self) -> bool {
        matches!(
            self,
            Self::SharedUnique | Self::SharedUniqueSlot | Self::SharedUniqueElem
        )
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
    /// `[fits]`: stops the program with an integer overflow unless `fits`,
    /// a `bool`: `+`, `-` and `*` on integers in a debug build.
    CheckOverflow,
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
            Self::Static(index, at) => format!("buf.static #{index} @{at}"),
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
            Self::CheckOverflow => "check_overflow".to_owned(),
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
    /// A conversion that reads its integer operand, or makes its integer
    /// result, as unsigned: an unsigned integer to a float, a `u32` widened to
    /// 64 bits with zeros, a float to an unsigned integer, saturating
    /// (decision 0024).
    ConvertU,
    /// The same bits, read as the expression's type: an `f64` and an `i64`,
    /// an `f32` and an `i32`, either way (decision 0026).
    Bits,
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
    /// `/` on unsigned integers; traps on zero (decision 0024).
    DivU,
    /// `%` on unsigned integers; traps on zero.
    RemU,
    /// `>>` on unsigned integers, logical.
    ShrU,
    /// `<` on unsigned integers.
    LtU,
    /// `<=` on unsigned integers.
    LeU,
    /// `>` on unsigned integers.
    GtU,
    /// `>=` on unsigned integers.
    GeU,
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
            Self::DivU => "/u",
            Self::RemU => "%u",
            Self::ShrU => ">>u",
            Self::LtU => "<u",
            Self::LeU => "<=u",
            Self::GtU => ">u",
            Self::GeU => ">=u",
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
            Self::Eq
                | Self::Ne
                | Self::Lt
                | Self::Le
                | Self::Gt
                | Self::Ge
                | Self::LtU
                | Self::LeU
                | Self::GtU
                | Self::GeU
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
    /// Rounds to the nearest; halves to the even, as a `Uint8ClampedArray`
    /// stores a number (decision 0047, f3). One instruction, `f64.nearest`.
    RoundEven,
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
            Self::RoundEven => "roundEven",
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
            each_expr(&f.body, &mut |e| match &e.kind {
                ExprKind::Math(op, _) => {
                    if let Some(h) = op.host() {
                        found.insert(h);
                    }
                }
                ExprKind::Binary(BinOp::Pow, ..) => {
                    found.insert(HostMath::Pow);
                }
                ExprKind::Binary(BinOp::Atan2, ..) => {
                    found.insert(HostMath::Atan2);
                }
                _ => {}
            });
        }
        found.into_iter().collect()
    }

    /// The functions used as values, in the order of their positions
    /// (decision 0025): an environment of `f` holds the position of `f` here.
    #[must_use]
    pub fn fn_table(&self) -> Vec<FnId> {
        let mut found = std::collections::BTreeSet::new();
        for f in &self.functions {
            each_expr(&f.body, &mut |e| {
                if let ExprKind::Env(EnvOp::Static(id, _) | EnvOp::New(id, _), _) = e.kind {
                    found.insert(id.0);
                }
            });
        }
        found.into_iter().map(FnId).collect()
    }

    /// The static environments the module uses, each once, by address:
    /// the function each names, `None` for [`EnvOp::Zero`].
    #[must_use]
    pub fn static_envs(&self) -> Vec<(u32, Option<FnId>)> {
        let mut found = std::collections::BTreeMap::new();
        for f in &self.functions {
            each_expr(&f.body, &mut |e| match e.kind {
                ExprKind::Env(EnvOp::Static(id, at), _) => {
                    found.insert(at, Some(id));
                }
                ExprKind::Env(EnvOp::Zero, _) => {
                    found.insert(ZERO_ENV, None);
                }
                _ => {}
            });
        }
        found.into_iter().collect()
    }

    /// Whether every block the module holds is a static environment: its
    /// function values capture nothing, and it uses no other heap value.
    /// Its environments are then only the positions of their functions, and
    /// no allocator is needed (decision 0026).
    #[must_use]
    pub fn static_envs_only(&self) -> bool {
        let mut only = self.statics().is_empty();
        for f in &self.functions {
            each_expr(&f.body, &mut |e| match &e.kind {
                ExprKind::Env(EnvOp::Static(..) | EnvOp::Zero, _)
                | ExprKind::Buf(BufOp::Clone, _) => {}
                ExprKind::Env(..) | ExprKind::Buf(..) | ExprKind::Alloc(_) => only = false,
                _ => {}
            });
            each_stmt(&f.body, &mut |s| {
                if matches!(s, Stmt::Buf(op, _) if *op != BufStmt::Release && *op != BufStmt::CheckIndex && *op != BufStmt::CheckOverflow)
                    || matches!(s, Stmt::Release(_))
                {
                    only = false;
                }
            });
        }
        only
    }

    /// The signatures of the calls through function values, each once: the
    /// types of the arguments, then of the results (decision 0025).
    #[must_use]
    pub fn indirect_signatures(&self) -> Vec<(Vec<Ty>, Vec<Ty>)> {
        fn stmts(f: &Function, body: &[Stmt], out: &mut Vec<(Vec<Ty>, Vec<Ty>)>) {
            for stmt in body {
                match stmt {
                    Stmt::CallManyIndirect { targets, args, .. } => {
                        let results = targets
                            .iter()
                            .map(|t| {
                                usize::try_from(t.0)
                                    .ok()
                                    .and_then(|i| f.locals.get(i))
                                    .map_or(Ty::Void, |l| l.ty)
                            })
                            .collect();
                        let sig = (
                            args.iter()
                                .map(|a| a.ty)
                                .chain(std::iter::once(Ty::Borrowed))
                                .collect(),
                            results,
                        );
                        if !out.contains(&sig) {
                            out.push(sig);
                        }
                    }
                    Stmt::If {
                        then, otherwise, ..
                    } => {
                        stmts(f, then, out);
                        stmts(f, otherwise, out);
                    }
                    Stmt::Loop { body, step, .. } => {
                        stmts(f, body, out);
                        stmts(f, step, out);
                    }
                    Stmt::Block(body) => stmts(f, body, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for f in &self.functions {
            each_expr(&f.body, &mut |e| {
                if let ExprKind::CallIndirect(_, args) = &e.kind {
                    let results = if e.ty == Ty::Void {
                        Vec::new()
                    } else {
                        vec![e.ty]
                    };
                    let sig = (
                        args.iter()
                            .map(|a| a.ty)
                            .chain(std::iter::once(Ty::Borrowed))
                            .collect(),
                        results,
                    );
                    if !out.contains(&sig) {
                        out.push(sig);
                    }
                }
            });
            stmts(f, &f.body, &mut out);
        }
        out
    }

    /// The functions of the runtime the module calls, in the order of
    /// [`RuntimeFn::ALL`]: a module imports these only.
    #[must_use]
    pub fn runtime_fns(&self) -> Vec<RuntimeFn> {
        let mut found = Vec::new();
        for f in &self.functions {
            each_expr(&f.body, &mut |e| {
                if let ExprKind::Buf(BufOp::Runtime(r), _) = &e.kind
                    && !found.contains(r)
                {
                    found.push(*r);
                }
            });
        }
        RuntimeFn::ALL
            .into_iter()
            .filter(|f| found.contains(f))
            .collect()
    }
}

/// Calls `visit` on every expression of `stmts`, nested ones included.
pub fn each_expr(stmts: &[Stmt], visit: &mut impl FnMut(&Expr)) {
    for stmt in stmts {
        match stmt {
            Stmt::Set(_, e) | Stmt::SetGlobal(_, e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => {
                each_in(e, visit);
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                each_in(cond, visit);
                each_expr(then, visit);
                each_expr(otherwise, visit);
            }
            Stmt::Loop { cond, body, step } => {
                if let Some(cond) = cond {
                    each_in(cond, visit);
                }
                each_expr(body, visit);
                each_expr(step, visit);
            }
            Stmt::Block(body) => each_expr(body, visit),
            Stmt::ReturnMany(values)
            | Stmt::Buf(_, values)
            | Stmt::CallMany { args: values, .. } => {
                for e in values {
                    each_in(e, visit);
                }
            }
            Stmt::CallManyIndirect { callee, args, .. } => {
                each_in(callee, visit);
                for e in args {
                    each_in(e, visit);
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

/// Calls `visit` on `e`, then on every expression inside it.
fn each_in(e: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(e);
    match &e.kind {
        ExprKind::Math(_, x)
        | ExprKind::Unary(_, x)
        | ExprKind::Convert(x)
        | ExprKind::Alloc(x) => {
            each_in(x, visit);
        }
        ExprKind::Binary(_, a, b) => {
            each_in(a, visit);
            each_in(b, visit);
        }
        ExprKind::Call(_, args) | ExprKind::Buf(_, args) | ExprKind::Env(_, args) => {
            for a in args {
                each_in(a, visit);
            }
        }
        ExprKind::CallIndirect(callee, args) => {
            each_in(callee, visit);
            for a in args {
                each_in(a, visit);
            }
        }
        ExprKind::Const(_)
        | ExprKind::Local(_)
        | ExprKind::Move(_)
        | ExprKind::Borrow(_)
        | ExprKind::Global(_) => {}
    }
}

/// Calls `visit` on every statement of `stmts`, nested ones included.
fn each_stmt(stmts: &[Stmt], visit: &mut impl FnMut(&Stmt)) {
    for stmt in stmts {
        visit(stmt);
        match stmt {
            Stmt::If {
                then, otherwise, ..
            } => {
                each_stmt(then, visit);
                each_stmt(otherwise, visit);
            }
            Stmt::Loop { body, step, .. } => {
                each_stmt(body, visit);
                each_stmt(step, visit);
            }
            Stmt::Block(body) => each_stmt(body, visit),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Text form, for snapshot tests
// ---------------------------------------------------------------------------

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, global) in self.globals.iter().enumerate() {
            writeln!(
                f,
                "global @{i} {}: {} = {:?}",
                global.name,
                global.ty.name(),
                global.init
            )?;
        }
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
            Stmt::SetGlobal(global, value) => {
                let _ = writeln!(out, "{pad}@{} = {}", global.0, expr(value));
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
            Stmt::CallManyIndirect {
                targets,
                callee,
                args,
            } => {
                let targets: Vec<String> = targets.iter().map(|t| format!("%{}", t.0)).collect();
                let args: Vec<String> = args.iter().map(expr).collect();
                let _ = writeln!(
                    out,
                    "{pad}{} = call_indirect {} {}",
                    targets.join(", "),
                    expr(callee),
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
        ExprKind::Global(g) => format!("@{}", g.0),
        ExprKind::Unary(op, operand) => {
            let op = match op {
                UnOp::Neg => "neg",
                UnOp::Not => "not",
                UnOp::BitNot => "bitnot",
                UnOp::ConvertU => "convert_u",
                UnOp::Bits => "bits",
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
        ExprKind::Env(op, args) => {
            let args: Vec<String> = args.iter().map(expr).collect();
            let op = match op {
                EnvOp::Static(id, at) => format!("env.static #{} @{at}", id.0),
                EnvOp::New(id, 0) => format!("env.new #{}", id.0),
                EnvOp::New(id, owned) => format!("env.new #{} owns {owned}", id.0),
                EnvOp::Shared(owned) => format!("rc.new owns {owned}"),
                EnvOp::Zero => "env.zero".to_owned(),
                EnvOp::Load(slot) => format!("env.load {slot}"),
                EnvOp::Store(slot) => format!("env.store {slot}"),
                EnvOp::StoreHandle(slot) => format!("env.store_handle {slot}"),
                EnvOp::LoadHandle(slot) => format!("env.handle {slot}"),
            };
            format!("({op} {})", args.join(" "))
        }
        ExprKind::CallIndirect(callee, args) => {
            let args: Vec<String> = args.iter().map(expr).collect();
            format!("(call_indirect {} {})", expr(callee), args.join(" "))
        }
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
mod map_tests;
#[cfg(test)]
mod match_tests;
#[cfg(test)]
mod method_tests;
#[cfg(test)]
mod opt_tests;
#[cfg(test)]
mod option_tests;
#[cfg(test)]
mod overflow_tests;
#[cfg(test)]
mod own_tests;
#[cfg(test)]
mod parallel_tests;
#[cfg(test)]
mod sort_tests;
#[cfg(test)]
mod string_tests;
#[cfg(test)]
mod struct_tests;
