//! `extern` functions in JavaScript ([decision 0010]): each is imported under
//! a name of its own, and called through a wrapper that checks every value
//! JavaScript gives back against the type declared, and every argument
//! JavaScript gives a callback. A value that does not match throws a
//! `TypeError` that names the function and the type declared.
//!
//! [decision 0010]: https://github.com/artkabis/Peras/blob/main/docs/en/decisions/0010-calling-javascript.md

use std::collections::BTreeMap;
use std::fmt::Write as _;

use perasc_check::ExternInfo;
use perasc_check::ty::{FloatTy, IntTy, Ty};

use crate::expr::string_literal;
use crate::{Module, safe};

/// How deep a shape describes a value; below, a structure that contains
/// itself through an array is taken as it is.
const DEPTH: usize = 12;

impl Module<'_> {
    /// The name of each `extern` function's wrapper, by its id.
    pub(crate) fn extern_names(&mut self) {
        for info in &self.analysis.externs {
            let name = match &info.method_of {
                Some(type_name) => format!("{type_name}${}", info.name),
                None => safe(&info.name),
            };
            self.fns.insert(info.id.0, name);
        }
    }

    /// The imports of the `extern` blocks and the wrappers of their functions.
    pub(crate) fn externs_code(&mut self) -> (String, String) {
        let mut imports: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut wrappers = String::new();
        let externs = &self.analysis.externs;
        for info in externs {
            if info.method_of.is_none() {
                imports
                    .entry(info.from.as_str())
                    .or_default()
                    .push(format!("{} as $x_{}", info.name, info.name));
            }
        }
        let mut import_lines = String::new();
        for (from, names) in imports {
            let _ = writeln!(
                import_lines,
                "import {{ {} }} from {};",
                names.join(", "),
                string_literal(from)
            );
        }
        for info in externs.clone() {
            let line = self.wrapper(&info);
            wrappers.push_str(&line);
        }
        (import_lines, wrappers)
    }

    /// A checker for each value JavaScript gives, made once, then
    /// `const name = (a, b) => $k1($x_name(a, b));`
    fn wrapper(&mut self, info: &ExternInfo) -> String {
        let name = self.fns.get(&info.id.0).cloned().unwrap_or_default();
        let id = info.id.0;
        let params: Vec<String> = (0..info.params.len()).map(|i| format!("p{i}")).collect();
        let at = match &info.method_of {
            Some(type_name) => format!("{type_name}.{}(…)", info.name),
            None => format!("{}(…)", info.name),
        };
        let mut out = String::new();
        // Arguments: callbacks wrapped, so that what JavaScript gives them is
        // checked too.
        let mut args = Vec::with_capacity(params.len());
        for (i, ty) in info.params.iter().enumerate() {
            if let Ty::Callback(ps, _) = ty {
                let callback = self.helper("$callback");
                let valid = self.helper("$valid");
                let where_ = string_literal(&format!("{at} gave its callback"));
                let checks: Vec<String> = ps
                    .iter()
                    .map(|t| {
                        let decl = string_literal(&t.to_string());
                        format!("{valid}({}, {where_}, {decl})", self.shape(t, 0))
                    })
                    .collect();
                let _ = writeln!(out, "const $k{id}_{i} = [{}];", checks.join(", "));
                args.push(format!("{callback}(p{i}, $k{id}_{i})"));
            } else {
                args.push(format!("p{i}"));
            }
        }
        let call = match &info.method_of {
            Some(_) => format!(
                "p0.{}({})",
                info.name,
                args.get(1..).unwrap_or_default().join(", ")
            ),
            None => format!("$x_{}({})", info.name, args.join(", ")),
        };
        let returned = string_literal(&format!("{at} returned"));
        let (checked, promise) = match &info.ret {
            Ty::Promise(inner) => (&**inner, true),
            ret => (ret, false),
        };
        let body = if *checked == Ty::Void && !promise {
            format!("{{ {call}; }}")
        } else {
            let valid = self.helper("$valid");
            let decl = string_literal(&checked.to_string());
            let shape = self.shape(checked, 0);
            let _ = writeln!(out, "const $k{id} = {valid}({shape}, {returned}, {decl});");
            if promise {
                format!("Promise.resolve({call}).then($k{id})")
            } else {
                format!("$k{id}({call})")
            }
        };
        let _ = writeln!(out, "const {name} = ({}) => {body};", params.join(", "));
        out
    }

    /// What a value of `ty` must look like, for `$valid`.
    pub(crate) fn shape(&self, ty: &Ty, depth: usize) -> String {
        if depth > DEPTH {
            return "\"any\"".to_owned();
        }
        let inner = |t: &Ty| self.shape(t, depth + 1);
        match ty {
            Ty::Int(int) => format!("\"{}\"", int_name(*int)),
            Ty::Float(FloatTy::F32) => "\"f32\"".to_owned(),
            Ty::Float(_) | Ty::FloatLit => "\"f64\"".to_owned(),
            Ty::IntLit => "\"i32\"".to_owned(),
            Ty::Bool => "\"bool\"".to_owned(),
            Ty::String => "\"string\"".to_owned(),
            Ty::Void => "\"void\"".to_owned(),
            Ty::Opaque(..) => "\"opaque\"".to_owned(),
            Ty::Array(t) => format!("[\"array\", {}]", inner(t)),
            Ty::Option(t) => format!("[\"option\", {}]", inner(t)),
            Ty::Set(t) => format!("[\"set\", {}]", inner(t)),
            Ty::Map(k, v) => format!("[\"map\", {}, {}]", inner(k), inner(v)),
            Ty::Result(ok, err) => format!(
                "[\"enum\", {{ Ok: [{}], Err: [{}] }}]",
                inner(ok),
                inner(err)
            ),
            Ty::Host(h) => format!("[\"host\", {}]", string_literal(h.ts_name())),
            Ty::Struct(id, _) => {
                let fields = self
                    .analysis
                    .structs
                    .get(id.0)
                    .map(|(_, f)| f.clone())
                    .unwrap_or_default();
                if fields.first().is_some_and(|(n, _)| n == "0") {
                    let items: Vec<String> = fields.iter().map(|(_, t)| inner(t)).collect();
                    format!("[\"tuple\", [{}]]", items.join(", "))
                } else {
                    let items: Vec<String> = fields
                        .iter()
                        .map(|(n, t)| format!("{}: {}", string_literal(n), inner(t)))
                        .collect();
                    format!("[\"struct\", {{ {} }}]", items.join(", "))
                }
            }
            Ty::Enum(id, _) => {
                let variants = self
                    .analysis
                    .enums
                    .get(id.0)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let items: Vec<String> = variants
                    .iter()
                    .map(|(n, ts)| {
                        let shapes: Vec<String> = ts.iter().map(&inner).collect();
                        format!("{}: [{}]", string_literal(n), shapes.join(", "))
                    })
                    .collect();
                format!("[\"enum\", {{ {} }}]", items.join(", "))
            }
            Ty::Promise(_) | Ty::Callback(..) | Ty::Fn(..) | Ty::Param(..) | Ty::Error => {
                "\"any\"".to_owned()
            }
        }
    }
}

const fn int_name(int: IntTy) -> &'static str {
    match int {
        IntTy::I8 => "i8",
        IntTy::I16 => "i16",
        IntTy::I32 => "i32",
        IntTy::I64 => "i64",
        IntTy::U8 => "u8",
        IntTy::U16 => "u16",
        IntTy::U32 => "u32",
        IntTy::U64 => "u64",
    }
}
