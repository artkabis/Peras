//! Fields and methods the language provides on its built-in types.
//!
//! Numbers convert explicitly (`x.toF64()`), as the spec requires, and expose
//! checked and wrapping arithmetic; arrays and strings have a `length`, maps
//! and sets a `size`.

use crate::ty::{FloatTy, IntTy, Ty};

/// A method: parameter types and return type.
pub struct Method {
    /// Parameter types, after the receiver.
    pub params: Vec<Ty>,
    /// The result type.
    pub ret: Ty,
}

const CONVERSIONS: [(&str, Ty); 10] = [
    ("toI8", Ty::Int(IntTy::I8)),
    ("toI16", Ty::Int(IntTy::I16)),
    ("toI32", Ty::Int(IntTy::I32)),
    ("toI64", Ty::Int(IntTy::I64)),
    ("toU8", Ty::Int(IntTy::U8)),
    ("toU16", Ty::Int(IntTy::U16)),
    ("toU32", Ty::Int(IntTy::U32)),
    ("toU64", Ty::Int(IntTy::U64)),
    ("toF32", Ty::Float(FloatTy::F32)),
    ("toF64", Ty::Float(FloatTy::F64)),
];

const FLOAT_METHODS: [&str; 9] = [
    "sqrt", "sin", "cos", "tan", "floor", "ceil", "round", "exp", "log",
];
/// The methods of floats with a second operand of the same type.
const FLOAT_BINARY: [&str; 3] = ["pow", "atan2", "hypot"];
const WRAPPING: [&str; 3] = ["wrappingAdd", "wrappingSub", "wrappingMul"];
const CHECKED: [&str; 3] = ["checkedAdd", "checkedSub", "checkedMul"];

/// The type of a field, for the built-in types that have fields.
#[must_use]
pub fn field(receiver: &Ty, name: &str) -> Option<Ty> {
    match (receiver, name) {
        (Ty::Array(_) | Ty::String, "length") | (Ty::Map(..) | Ty::Set(_), "size") => {
            Some(Ty::Int(IntTy::I32))
        }
        _ => None,
    }
}

/// Whether a method converts a number to another type, as `toF64`.
#[must_use]
pub fn is_conversion(name: &str) -> bool {
    CONVERSIONS.iter().any(|(n, _)| *n == name)
}

/// A method of a built-in type.
#[must_use]
pub fn method(receiver: &Ty, name: &str) -> Option<Method> {
    let me = receiver.clone().defaulted();
    let simple = |ret: Ty| {
        Some(Method {
            params: Vec::new(),
            ret,
        })
    };
    match &me {
        Ty::Int(_) | Ty::Float(_) => {
            if let Some((_, target)) = CONVERSIONS.iter().find(|(n, _)| *n == name) {
                return simple(target.clone());
            }
            if name == "abs" || (me.is_float() && FLOAT_METHODS.contains(&name)) {
                return simple(me);
            }
            if me.is_float() && FLOAT_BINARY.contains(&name) {
                return Some(Method {
                    params: vec![me.clone()],
                    ret: me,
                });
            }
            if me.is_integer() && WRAPPING.contains(&name) {
                return Some(Method {
                    params: vec![me.clone()],
                    ret: me,
                });
            }
            if name == "toString" {
                return simple(Ty::String);
            }
            if me.is_integer() && CHECKED.contains(&name) {
                return Some(Method {
                    params: vec![me.clone()],
                    ret: Ty::Option(Box::new(me)),
                });
            }
            None
        }
        Ty::Array(element) => match name {
            "push" => Some(Method {
                params: vec![(**element).clone()],
                ret: Ty::Void,
            }),
            "pop" => simple(Ty::Option(element.clone())),
            "copy" => simple(me.clone()),
            // The strings of an array, joined.
            "join" if **element == Ty::String => Some(Method {
                params: vec![Ty::String],
                ret: Ty::String,
            }),
            "slice" => Some(Method {
                params: vec![Ty::Int(IntTy::I32), Ty::Int(IntTy::I32)],
                ret: me.clone(),
            }),
            "concat" => Some(Method {
                params: vec![me.clone()],
                ret: me.clone(),
            }),
            "reverse" => simple(Ty::Void),
            "sort" if sortable(element) => simple(Ty::Void),
            "includes" if searchable(element) => Some(Method {
                params: vec![(**element).clone()],
                ret: Ty::Bool,
            }),
            "indexOf" if searchable(element) => Some(Method {
                params: vec![(**element).clone()],
                ret: Ty::Int(IntTy::I32),
            }),
            _ => None,
        },
        Ty::Bool if name == "toString" => simple(Ty::String),
        Ty::String => string_method(name),
        Ty::Map(..) | Ty::Set(_) => collection_method(&me, name),
        Ty::Result(ok, _) => match name {
            "isOk" | "isErr" => simple(Ty::Bool),
            "unwrapOr" => Some(Method {
                params: vec![(**ok).clone()],
                ret: (**ok).clone(),
            }),
            _ => None,
        },
        Ty::Option(inner) => match name {
            "isSome" | "isNone" => simple(Ty::Bool),
            "unwrapOr" => Some(Method {
                params: vec![(**inner).clone()],
                ret: (**inner).clone(),
            }),
            _ => None,
        },
        _ => None,
    }
}

/// A method of `Map` or `Set`.
fn collection_method(me: &Ty, name: &str) -> Option<Method> {
    let (params, ret) = match me {
        Ty::Map(key, value) => match name {
            "set" => (vec![(**key).clone(), (**value).clone()], Ty::Void),
            "get" => (vec![(**key).clone()], Ty::Option(value.clone())),
            "has" | "delete" => (vec![(**key).clone()], Ty::Bool),
            "clear" => (Vec::new(), Ty::Void),
            "copy" => (Vec::new(), me.clone()),
            _ => return None,
        },
        Ty::Set(element) => match name {
            "add" => (vec![(**element).clone()], Ty::Void),
            "has" | "delete" => (vec![(**element).clone()], Ty::Bool),
            "clear" => (Vec::new(), Ty::Void),
            "copy" => (Vec::new(), me.clone()),
            _ => return None,
        },
        _ => return None,
    };
    Some(Method { params, ret })
}

/// Whether `sort()` orders elements of this type: `i32`, `i64` and floats.
fn sortable(element: &Ty) -> bool {
    matches!(
        element,
        Ty::Int(IntTy::I32 | IntTy::I64) | Ty::Float(_) | Ty::IntLit | Ty::FloatLit
    )
}

/// Whether `includes` and `indexOf` find elements of this type: numbers,
/// booleans and strings.
fn searchable(element: &Ty) -> bool {
    sortable(element) || matches!(element, Ty::Bool | Ty::String)
}

/// The methods of strings beyond `copy`.
const STRING_METHODS: [&str; 14] = [
    "indexOf",
    "includes",
    "startsWith",
    "endsWith",
    "slice",
    "trim",
    "toUpperCase",
    "toLowerCase",
    "repeat",
    "replace",
    "replaceAll",
    "split",
    "parseI32",
    "parseI64",
];

/// A method of `String`.
fn string_method(name: &str) -> Option<Method> {
    let int = Ty::Int(IntTy::I32);
    let (params, ret) = match name {
        "copy" | "trim" | "toUpperCase" | "toLowerCase" => (Vec::new(), Ty::String),
        "indexOf" => (vec![Ty::String], int),
        "includes" | "startsWith" | "endsWith" => (vec![Ty::String], Ty::Bool),
        "slice" => (vec![int.clone(), int], Ty::String),
        "repeat" => (vec![int], Ty::String),
        "replace" | "replaceAll" => (vec![Ty::String, Ty::String], Ty::String),
        "split" => (vec![Ty::String], Ty::Array(Box::new(Ty::String))),
        "parseI32" => (Vec::new(), Ty::Option(Box::new(int))),
        "parseI64" => (Vec::new(), Ty::Option(Box::new(Ty::Int(IntTy::I64)))),
        _ => return None,
    };
    Some(Method { params, ret })
}

/// Every member name of a built-in type, for suggestions.
#[must_use]
pub fn members(receiver: &Ty) -> Vec<&'static str> {
    let me = receiver.clone().defaulted();
    let mut names = Vec::new();
    match &me {
        Ty::Int(_) | Ty::Float(_) => {
            names.extend(CONVERSIONS.iter().map(|(n, _)| *n));
            names.extend(["abs", "min", "max", "clamp"]);
            if me.is_float() {
                names.extend(FLOAT_METHODS);
                names.extend(FLOAT_BINARY);
            } else {
                names.extend(WRAPPING);
                names.extend(CHECKED);
            }
            names.push("toString");
        }
        Ty::Array(element) => {
            names.extend([
                "length", "push", "pop", "copy", "slice", "concat", "reverse",
            ]);
            if **element == Ty::String {
                names.push("join");
            }
            if sortable(element) {
                names.push("sort");
            }
            if searchable(element) {
                names.extend(["includes", "indexOf"]);
            }
            names.extend(crate::stdlib::FUNCTION_METHODS);
        }
        Ty::String => {
            names.extend(["length", "copy"]);
            names.extend(STRING_METHODS);
        }
        Ty::Bool => names.push("toString"),
        Ty::Map(..) => names.extend(["size", "set", "get", "has", "delete", "clear", "copy"]),
        Ty::Set(_) => names.extend(["size", "add", "has", "delete", "clear", "copy"]),
        Ty::Option(_) => names.extend(["isSome", "isNone", "unwrapOr"]),
        Ty::Result(..) => names.extend(["isOk", "isErr", "unwrapOr"]),
        _ => {}
    }
    names
}
