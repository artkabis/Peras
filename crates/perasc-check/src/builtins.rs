//! Fields and methods the language provides on its built-in types.
//!
//! Numbers convert explicitly (`x.toF64()`), as the spec requires, and expose
//! checked and wrapping arithmetic; arrays and strings have a `length`.

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

const FLOAT_METHODS: [&str; 7] = ["sqrt", "sin", "cos", "tan", "floor", "ceil", "round"];
const WRAPPING: [&str; 3] = ["wrappingAdd", "wrappingSub", "wrappingMul"];
const CHECKED: [&str; 3] = ["checkedAdd", "checkedSub", "checkedMul"];

/// The type of a field, for the built-in types that have fields.
#[must_use]
pub fn field(receiver: &Ty, name: &str) -> Option<Ty> {
    match (receiver, name) {
        (Ty::Array(_) | Ty::String, "length") => Some(Ty::Int(IntTy::I32)),
        _ => None,
    }
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
            if me.is_integer() && WRAPPING.contains(&name) {
                return Some(Method {
                    params: vec![me.clone()],
                    ret: me,
                });
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

/// Every member name of a built-in type, for suggestions.
#[must_use]
pub fn members(receiver: &Ty) -> Vec<&'static str> {
    let me = receiver.clone().defaulted();
    let mut names = Vec::new();
    match &me {
        Ty::Int(_) | Ty::Float(_) => {
            names.extend(CONVERSIONS.iter().map(|(n, _)| *n));
            names.push("abs");
            if me.is_float() {
                names.extend(FLOAT_METHODS);
            } else {
                names.extend(WRAPPING);
                names.extend(CHECKED);
            }
        }
        Ty::Array(_) => names.extend(["length", "push", "pop"]),
        Ty::String => names.push("length"),
        Ty::Option(_) => names.extend(["isSome", "isNone", "unwrapOr"]),
        _ => {}
    }
    names
}
