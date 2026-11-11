//! The language's methods on its built-in types, as the owned backends
//! compute them.

use perasc_check::ty::{FloatTy, IntTy, Ty};
use perasc_syntax::ast::{BinaryOp, ExprId};

use crate::{Module, is_big, range, round, wrap};

/// Whether copying a collection of these needs a deep copy: its elements
/// are objects, arrays or collections of their own.
fn shallow(element: &Ty) -> bool {
    matches!(element, Ty::Int(_) | Ty::Float(_) | Ty::Bool | Ty::String)
}

impl Module<'_> {
    /// A built-in method or member: `xs.push(x)`, `s.trim()`, `n.toF64()`.
    pub(crate) fn builtin(
        &mut self,
        id: ExprId,
        base: ExprId,
        name: &str,
        args: &[ExprId],
    ) -> String {
        let receiver = self.ty(base);
        let result = self.ty(id);
        let mut all = Vec::with_capacity(args.len() + 1);
        all.push(base);
        all.extend_from_slice(args);
        let values = self.exprs(&all);
        let me = values[0].clone();
        let rest = &values[1..];
        let arg = |i: usize| {
            rest.get(i)
                .cloned()
                .unwrap_or_else(|| "undefined".to_owned())
        };
        let out = match &receiver {
            Ty::Int(int) => self.int_method(*int, name, &me, rest, &result, base),
            Ty::Float(float) => self.float_method(*float, name, &me, rest, &result, base),
            Ty::Bool => Some(format!("String({me})")),
            Ty::Array(_) if name == "sortBy" || name == "sortByDescending" => {
                let key = match args.first().map(|&f| self.ty(f)) {
                    Some(Ty::Callback(_, key)) => *key,
                    _ => Ty::Error,
                };
                // Only the sort the key needs: numbers by their bits, `i64`
                // as `bigint`s, strings by their code points.
                let helper = self.helper(match key {
                    Ty::Int(IntTy::I64 | IntTy::U64) => "$sortBig",
                    Ty::String => "$sortStr",
                    _ => "$sortNum",
                });
                let desc = name == "sortByDescending";
                Some(format!("{helper}({me}, {}, {desc})", arg(0)))
            }
            Ty::Array(element) => self.array_method(element, name, &me, rest),
            Ty::String => self.string_method(name, &me, rest),
            Ty::Map(_, value) => match name {
                "copy" if shallow(value) => Some(format!("new Map({me})")),
                "copy" => Some(format!("structuredClone({me})")),
                "set" => Some(format!("{me}.set({}, {})", arg(0), arg(1))),
                "get" | "has" | "delete" => Some(format!("{me}.{name}({})", arg(0))),
                "clear" => Some(format!("{me}.clear()")),
                _ => None,
            },
            Ty::Set(element) => match name {
                "copy" if shallow(element) => Some(format!("new Set({me})")),
                "copy" => Some(format!("structuredClone({me})")),
                "add" | "has" | "delete" => Some(format!("{me}.{name}({})", arg(0))),
                "clear" => Some(format!("{me}.clear()")),
                _ => None,
            },
            Ty::Option(_) => match name {
                "isSome" => Some(format!("({me} !== undefined)")),
                "isNone" => Some(format!("({me} === undefined)")),
                "unwrapOr" => {
                    let me = self.spill(me);
                    Some(format!("({me} !== undefined ? {me} : {})", arg(0)))
                }
                _ => None,
            },
            Ty::Result(..) => match name {
                "isOk" => Some(format!("({me}.kind === \"Ok\")")),
                "isErr" => Some(format!("({me}.kind === \"Err\")")),
                "unwrapOr" => {
                    let me = self.spill(me);
                    Some(format!(
                        "({me}.kind === \"Ok\" ? {me}.values[0] : {})",
                        arg(0)
                    ))
                }
                _ => None,
            },
            _ => None,
        };
        out.unwrap_or_else(|| {
            let span = self.ast.expr(id).span;
            self.unsupported(span, "method");
            "undefined".to_owned()
        })
    }

    fn int_method(
        &mut self,
        int: IntTy,
        name: &str,
        me: &str,
        rest: &[String],
        result: &Ty,
        base: ExprId,
    ) -> Option<String> {
        let arg = |i: usize| rest.get(i).cloned().unwrap_or_default();
        let big = is_big(int);
        let ty = Ty::Int(int);
        Some(match name {
            "toString" => self.text_of(base, me),
            // JavaScript's own: no helper, and the same text as in an owned module.
            "toFixed" => format!("({me}).toFixed({})", arg(0)),
            "abs" if big => {
                let me = self.spill(me.to_owned());
                wrap(int, &format!("{me} < 0n ? -{me} : {me}"))
            }
            "abs" => wrap(int, &format!("Math.abs({me})")),
            "min" | "max" if big => {
                let helper = self.helper(if name == "min" { "$min" } else { "$max" });
                format!("{helper}({me}, {})", arg(0))
            }
            "min" | "max" => format!("Math.{name}({me}, {})", arg(0)),
            "clamp" if big => {
                let (min, max) = (self.helper("$min"), self.helper("$max"));
                format!("{min}({max}({me}, {}), {})", arg(0), arg(1))
            }
            "clamp" => format!("Math.min(Math.max({me}, {}), {})", arg(0), arg(1)),
            "wrappingAdd" | "wrappingSub" | "wrappingMul" => {
                let op = match name {
                    "wrappingAdd" => BinaryOp::Add,
                    "wrappingSub" => BinaryOp::Sub,
                    _ => BinaryOp::Mul,
                };
                let outer = std::mem::replace(&mut self.current.wrapping, true);
                let text = self.operation(op, &ty, me, &arg(0));
                self.current.wrapping = outer;
                text
            }
            "checkedAdd" | "checkedSub" | "checkedMul" => {
                let symbol = match name {
                    "checkedAdd" => "+",
                    "checkedSub" => "-",
                    _ => "*",
                };
                let exact = format!("{me} {symbol} {}", arg(0));
                if big {
                    let helper = self.helper("$checked64");
                    format!("{helper}({exact}, {})", int == IntTy::I64)
                } else {
                    let (lo, hi) = if int == IntTy::I32 {
                        (-2_147_483_648, 2_147_483_647)
                    } else {
                        range(int)
                    };
                    let helper = self.helper("$checked");
                    format!("{helper}({exact}, {lo}, {hi})")
                }
            }
            _ => return self.conversion(&ty, name, me, result),
        })
    }

    fn float_method(
        &mut self,
        float: FloatTy,
        name: &str,
        me: &str,
        rest: &[String],
        result: &Ty,
        base: ExprId,
    ) -> Option<String> {
        let arg = |i: usize| rest.get(i).cloned().unwrap_or_default();
        let ty = Ty::Float(float);
        Some(match name {
            "toString" => self.text_of(base, me),
            // JavaScript's own: no helper, and the same text as in an owned module.
            "toFixed" => format!("({me}).toFixed({})", arg(0)),
            "abs" | "sqrt" | "floor" | "ceil" | "round" | "sin" | "cos" | "tan" | "exp" | "log" => {
                round(&ty, format!("Math.{name}({me})"))
            }
            "pow" | "atan2" | "hypot" | "min" | "max" => {
                round(&ty, format!("Math.{name}({me}, {})", arg(0)))
            }
            "clamp" => format!("Math.min(Math.max({me}, {}), {})", arg(0), arg(1)),
            _ => return self.conversion(&ty, name, me, result),
        })
    }

    /// `toI32()` and the other conversions: between integers, the low bits;
    /// from a float, rounded toward zero and saturated, NaN to 0.
    fn conversion(&mut self, from: &Ty, name: &str, me: &str, to: &Ty) -> Option<String> {
        if !name.starts_with("to") {
            return None;
        }
        Some(match (from, to) {
            (Ty::Int(a), Ty::Int(b)) => match (is_big(*a), is_big(*b)) {
                (false, false) | (true, true) => wrap(*b, me),
                (false, true) => wrap(*b, &format!("BigInt({me})")),
                (true, false) => wrap(*b, &format!("Number(BigInt.asIntN(32, {me}))")),
            },
            (Ty::Int(a), Ty::Float(_)) => {
                let number = if is_big(*a) {
                    format!("Number({me})")
                } else {
                    me.to_owned()
                };
                round(to, number)
            }
            (Ty::Float(_), Ty::Int(b)) => {
                if is_big(*b) {
                    let helper = self.helper("$saturate64");
                    format!("{helper}({me}, {})", *b == IntTy::I64)
                } else {
                    let (lo, hi) = if *b == IntTy::I32 {
                        (-2_147_483_648, 2_147_483_647)
                    } else {
                        range(*b)
                    };
                    let helper = self.helper("$saturate");
                    format!("{helper}({me}, {lo}, {hi})")
                }
            }
            (Ty::Float(_), Ty::Float(_)) => round(to, me.to_owned()),
            _ => return None,
        })
    }

    fn array_method(
        &mut self,
        element: &Ty,
        name: &str,
        me: &str,
        rest: &[String],
    ) -> Option<String> {
        let arg = |i: usize| rest.get(i).cloned().unwrap_or_default();
        Some(match name {
            "push" => format!("{me}.push({})", arg(0)),
            "pop" => format!("{me}.pop()"),
            "copy" if shallow(element) => format!("{me}.slice()"),
            "copy" => format!("structuredClone({me})"),
            "slice" => format!("{me}.slice({}, {})", arg(0), arg(1)),
            "concat" | "join" => format!("{me}.{name}({})", arg(0)),
            "reverse" => format!("{me}.reverse()"),
            "sort" => {
                let typed = match element {
                    Ty::Int(IntTy::I64) => "BigInt64Array",
                    Ty::Int(_) => "Int32Array",
                    _ => "Float64Array",
                };
                let helper = self.helper("$sort");
                format!("{helper}({me}, {typed})")
            }
            "includes" => format!("({me}.indexOf({}) >= 0)", arg(0)),
            "indexOf" => format!("{me}.indexOf({})", arg(0)),
            "map" | "filter" | "find" | "findIndex" | "some" | "every" => {
                format!("{me}.{name}({})", arg(0))
            }
            "reduce" => format!("{me}.reduce({}, {})", arg(0), arg(1)),
            _ => return None,
        })
    }

    fn string_method(&mut self, name: &str, me: &str, rest: &[String]) -> Option<String> {
        let arg = |i: usize| rest.get(i).cloned().unwrap_or_default();
        Some(match name {
            "copy" => me.to_owned(),
            "indexOf" | "includes" | "startsWith" | "endsWith" => {
                format!("{me}.{name}({})", arg(0))
            }
            "slice" => format!("{me}.slice({}, {})", arg(0), arg(1)),
            "trim" | "toUpperCase" | "toLowerCase" | "split" | "parseI32" | "parseI64"
            | "parseF64" | "repeat" => {
                let helper = self.helper(match name {
                    "trim" => "$trim",
                    "toUpperCase" => "$upper",
                    "toLowerCase" => "$lower",
                    "split" => "$split",
                    "parseI32" => "$parseI32",
                    "parseI64" => "$parseI64",
                    "parseF64" => "$parseF64",
                    _ => "$repeat",
                });
                let args: Vec<String> = std::iter::once(me.to_owned())
                    .chain(rest.iter().cloned())
                    .collect();
                format!("{helper}({})", args.join(", "))
            }
            "replace" | "replaceAll" => {
                // A function gives the text as it is: `$&` and the others
                // mean nothing in it. Each value is computed once, in order.
                let me = self.spill(me.to_owned());
                let from = self.spill(arg(0));
                let to = self.spill(arg(1));
                format!("{me}.{name}({from}, () => {to})")
            }
            _ => return None,
        })
    }
}
