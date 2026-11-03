//! Types as the checker sees them.

use std::fmt;

/// Integer types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntTy {
    /// `i8`
    I8,
    /// `i16`
    I16,
    /// `i32`, the default for an integer literal.
    I32,
    /// `i64`
    I64,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `u64`
    U64,
}

impl IntTy {
    /// Every integer type, for name lookup.
    pub const ALL: [Self; 8] = [
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
    ];

    /// Its name in source.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
        }
    }

    /// Whether it can hold negative values.
    #[must_use]
    pub const fn signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64)
    }

    /// The largest value, as an unsigned 64-bit number.
    #[must_use]
    pub const fn max(self) -> u64 {
        match self {
            Self::I8 => 127,
            Self::I16 => 32_767,
            Self::I32 => 2_147_483_647,
            Self::I64 => 9_223_372_036_854_775_807,
            Self::U8 => 255,
            Self::U16 => 65_535,
            Self::U32 => 4_294_967_295,
            Self::U64 => u64::MAX,
        }
    }

    /// The magnitude of the smallest value: `-min`, or 0 when unsigned.
    #[must_use]
    pub const fn min_magnitude(self) -> u64 {
        if self.signed() { self.max() + 1 } else { 0 }
    }

    /// The conversion method to this type, e.g. `toI64`.
    #[must_use]
    pub const fn conversion(self) -> &'static str {
        match self {
            Self::I8 => "toI8",
            Self::I16 => "toI16",
            Self::I32 => "toI32",
            Self::I64 => "toI64",
            Self::U8 => "toU8",
            Self::U16 => "toU16",
            Self::U32 => "toU32",
            Self::U64 => "toU64",
        }
    }
}

/// Floating-point types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatTy {
    /// `f32`
    F32,
    /// `f64`, the default for a float literal.
    F64,
}

impl FloatTy {
    /// Its name in source.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    /// The conversion method to this type.
    #[must_use]
    pub const fn conversion(self) -> &'static str {
        match self {
            Self::F32 => "toF32",
            Self::F64 => "toF64",
        }
    }
}

/// Index of a structure in the checker's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId(pub usize);

/// Index of a function in the checker's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FnId(pub usize);

/// A type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// A sized integer.
    Int(IntTy),
    /// A float.
    Float(FloatTy),
    /// An integer literal whose type the context has not fixed yet.
    IntLit,
    /// A float literal whose type the context has not fixed yet.
    FloatLit,
    /// `bool`
    Bool,
    /// `String`
    String,
    /// `void`: no value.
    Void,
    /// `Array<T>`
    Array(Box<Ty>),
    /// `Option<T>`
    Option(Box<Ty>),
    /// A structure, with its name for messages.
    Struct(StructId, String),
    /// A function used as a value, with its name for messages.
    Fn(FnId, String),
    /// A type already reported as wrong: compatible with everything, so one
    /// mistake gives one error.
    Error,
}

impl Ty {
    /// Integers, floats and their literals.
    #[must_use]
    pub const fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::Int(_) | Self::Float(_) | Self::IntLit | Self::FloatLit
        )
    }

    /// Integers and integer literals.
    #[must_use]
    pub const fn is_integer(&self) -> bool {
        matches!(self, Self::Int(_) | Self::IntLit)
    }

    /// Floats and float literals.
    #[must_use]
    pub const fn is_float(&self) -> bool {
        matches!(self, Self::Float(_) | Self::FloatLit)
    }

    /// The concrete type a literal takes when nothing else decides: `i32`, `f64`.
    #[must_use]
    pub fn defaulted(self) -> Self {
        match self {
            Self::IntLit => Self::Int(IntTy::I32),
            Self::FloatLit => Self::Float(FloatTy::F64),
            other => other,
        }
    }

    /// The method converting a number to this type, if it is numeric.
    #[must_use]
    pub fn conversion(&self) -> Option<&'static str> {
        match self {
            Self::Int(i) => Some(i.conversion()),
            Self::Float(f) => Some(f.conversion()),
            _ => None,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(i) => f.write_str(i.name()),
            Self::Float(x) => f.write_str(x.name()),
            Self::IntLit => f.write_str("{integer}"),
            Self::FloatLit => f.write_str("{float}"),
            Self::Bool => f.write_str("bool"),
            Self::String => f.write_str("String"),
            Self::Void => f.write_str("void"),
            Self::Array(t) => write!(f, "Array<{t}>"),
            Self::Option(t) => write!(f, "Option<{t}>"),
            Self::Struct(_, name) => f.write_str(name),
            Self::Fn(_, name) => write!(f, "fn {name}"),
            Self::Error => f.write_str("?"),
        }
    }
}
