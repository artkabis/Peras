//! TypeScript declarations of a managed module's exports, in the shapes the
//! generated JavaScript gives its values.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use perasc_check::Analysis;
use perasc_check::ty::{FnId, Ty};
use perasc_syntax::ast::Ast;

use crate::is_big;

/// The declarations of the exported functions, and of the types they name.
#[must_use]
pub fn declarations(ast: &Ast, analysis: &Analysis) -> String {
    let mut types = Types {
        analysis,
        named: BTreeMap::new(),
    };
    let mut functions = String::new();
    for (index, decl) in ast.functions().iter().enumerate() {
        if !decl.exported || decl.impl_of.is_some() || analysis.is_generic(FnId(index)) {
            continue;
        }
        let Some(export) = analysis
            .exports
            .iter()
            .find(|e| e.name == ast.name(decl.name.symbol))
        else {
            continue;
        };
        let params: Vec<String> = decl
            .params
            .iter()
            .zip(&export.params)
            .map(|(p, (ty, _))| format!("{}: {}", ast.name(p.name.symbol), types.ts(ty)))
            .collect();
        let ret = types.ts(&export.ret);
        let _ = writeln!(
            functions,
            "export declare function {}({}): {ret};",
            ast.name(decl.name.symbol),
            params.join(", ")
        );
    }
    let mut out = String::new();
    for (name, body) in &types.named {
        let _ = writeln!(out, "export type {name} = {body};");
    }
    out.push_str(&functions);
    out
}

struct Types<'a> {
    analysis: &'a Analysis,
    /// The structures and enumerations named so far, by their TypeScript name.
    named: BTreeMap<String, String>,
}

/// A checker's type name as a TypeScript one: `Pair<i32>` is `Pair_i32`.
fn ts_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    while out.ends_with('_') {
        out.pop();
    }
    out
}

impl Types<'_> {
    fn ts(&mut self, ty: &Ty) -> String {
        match ty {
            Ty::Int(int) if is_big(*int) => "bigint".to_owned(),
            Ty::Int(_) | Ty::IntLit | Ty::Float(_) | Ty::FloatLit => "number".to_owned(),
            Ty::Bool => "boolean".to_owned(),
            Ty::String => "string".to_owned(),
            Ty::Void => "void".to_owned(),
            Ty::Array(t) => format!("{}[]", self.wrapped(t)),
            Ty::Map(k, v) => format!("Map<{}, {}>", self.ts(k), self.ts(v)),
            Ty::Set(t) => format!("Set<{}>", self.ts(t)),
            Ty::Promise(t) => format!("Promise<{}>", self.ts(t)),
            Ty::Option(t) => format!("{} | undefined", self.wrapped(t)),
            Ty::Result(ok, err) => format!(
                "{{ kind: \"Ok\"; values: [{}] }} | {{ kind: \"Err\"; values: [{}] }}",
                self.ts(ok),
                self.ts(err)
            ),
            Ty::Struct(id, _) => {
                let Some((name, fields)) = self.analysis.structs.get(id.0) else {
                    return "unknown".to_owned();
                };
                if fields.first().is_some_and(|(f, _)| f == "0") {
                    let items: Vec<String> = fields.iter().map(|(_, t)| self.ts(t)).collect();
                    return format!("[{}]", items.join(", "));
                }
                let name = ts_name(name);
                if !self.named.contains_key(&name) {
                    // Named before its fields: a type that contains itself stops here.
                    self.named.insert(name.clone(), String::new());
                    let fields: Vec<String> = fields
                        .iter()
                        .map(|(f, t)| format!("{f}: {}", self.ts(t)))
                        .collect();
                    self.named
                        .insert(name.clone(), format!("{{ {} }}", fields.join("; ")));
                }
                name
            }
            Ty::Enum(id, _) => {
                let Some((name, variants)) = self.analysis.enums.get(id.0) else {
                    return "unknown".to_owned();
                };
                let name = ts_name(name);
                if !self.named.contains_key(&name) {
                    self.named.insert(name.clone(), String::new());
                    let cases: Vec<String> = variants
                        .iter()
                        .map(|(v, types)| {
                            let values: Vec<String> = types.iter().map(|t| self.ts(t)).collect();
                            format!("{{ kind: \"{v}\"; values: [{}] }}", values.join(", "))
                        })
                        .collect();
                    self.named.insert(name.clone(), cases.join(" | "));
                }
                name
            }
            Ty::Fn(..) | Ty::Param(..) | Ty::Error => "unknown".to_owned(),
        }
    }

    /// A type inside `T[]` or `T | undefined`: in parentheses when it is a union.
    fn wrapped(&mut self, ty: &Ty) -> String {
        let text = self.ts(ty);
        if matches!(ty, Ty::Option(_) | Ty::Result(..)) {
            format!("({text})")
        } else {
            text
        }
    }
}
