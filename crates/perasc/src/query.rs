//! `perasc serve` (decision 0042, q2): queries for the tools, one process
//! for an editor's session, a JSON request a line on standard input and an
//! answer a line on standard output. Each file opened is kept with its
//! analysis, and analysed again when its text changes, or the text of a file
//! it imports; the protocol is in `docs/en/reference/serve.md`.
//!
//! Every answer comes from the file's tree, its tokens and what the checker
//! learned of it (`perasc_check::Analysis`).

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use perasc_check::ty::Ty;
use perasc_check::{Analysis, NameKind};
use perasc_diag::Span;
use perasc_syntax::ast::{Ast, ExprKind, ItemKind};
use perasc_syntax::{Token, TokenKind, lex};
use serde_json::{Value, json};

/// What a declaration is, as an answer names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum What {
    Local,
    Parameter,
    Function,
    Structure,
    Enumeration,
    Variant,
    Field,
    Global,
    Import,
}

impl What {
    const fn name(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Parameter => "parameter",
            Self::Function => "function",
            Self::Structure => "structure",
            Self::Enumeration => "enumeration",
            Self::Variant => "variant",
            Self::Field => "field",
            Self::Global => "module variable",
            Self::Import => "import",
        }
    }
}

/// A name declared in a file.
#[derive(Debug, Clone)]
struct Decl {
    name: String,
    what: What,
    span: Span,
    /// Its type, when the checker knows it.
    ty: Option<Ty>,
    /// A function's signature, as its declaration writes it.
    signature: Option<String>,
    /// Its index in [`Analysis::bindings`], when it has one.
    binding: Option<usize>,
    /// For an import, the module it comes from, as written.
    from: Option<String>,
}

/// A file read and analysed: what a query answers from.
pub struct Opened {
    path: PathBuf,
    source: String,
    ast: Ast,
    analysis: Analysis,
    tokens: Vec<Token>,
    decls: Vec<Decl>,
    managed: bool,
    /// The codes of its errors, found once for its text.
    diagnostics: std::cell::OnceCell<Vec<String>>,
}

impl Opened {
    /// Reads `source` as the file at `path`; a syntax error leaves the tree
    /// the parser recovered, which is analysed all the same.
    #[must_use]
    pub fn new(path: &Path, source: String) -> Self {
        let parsed = perasc_syntax::parse(&source);
        let context = perasc_driver::analysis_context(path, &parsed.ast);
        let analysis = perasc_check::analyze_with(&parsed.ast, &source, &context);
        let tokens = lex(&source).tokens;
        let mut opened = Self {
            path: path.to_path_buf(),
            managed: context.managed,
            source,
            ast: parsed.ast,
            analysis,
            tokens,
            decls: Vec::new(),
            diagnostics: std::cell::OnceCell::new(),
        };
        opened.decls = opened.declarations();
        opened
    }

    /// The files its imports name, by canonical path.
    fn imports(&self) -> Vec<PathBuf> {
        self.ast
            .items
            .iter()
            .filter_map(|item| match &item.kind {
                ItemKind::Import(i) => perasc_driver::import_path(&self.path, &i.from),
                _ => None,
            })
            .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
            .collect()
    }

    /// Every name the file declares.
    fn declarations(&self) -> Vec<Decl> {
        let mut out = Vec::new();
        let mut params: Vec<Span> = Vec::new();
        let text = |s: Span| {
            self.source
                .get(s.start as usize..s.end as usize)
                .unwrap_or("")
        };
        let decl = |name: String, what: What, span: Span| Decl {
            name,
            what,
            span,
            ty: None,
            signature: None,
            binding: None,
            from: None,
        };
        for item in &self.ast.items {
            match &item.kind {
                ItemKind::Fn(f) => {
                    let mut d = decl(
                        self.ast.name(f.name.symbol).to_owned(),
                        What::Function,
                        f.name.span,
                    );
                    d.signature = Some(self.signature(item.span.start, f.params_span.end));
                    out.push(d);
                    params.extend(f.params.iter().map(|p| p.name.span));
                }
                ItemKind::Struct(s) => {
                    let name = self.ast.name(s.name.symbol).to_owned();
                    out.push(decl(name.clone(), What::Structure, s.name.span));
                    let fields = self.fields(&name);
                    for field in &s.fields {
                        let fname = self.ast.name(field.name.symbol).to_owned();
                        let mut d = decl(fname.clone(), What::Field, field.name.span);
                        d.ty = fields
                            .iter()
                            .find(|(n, _)| *n == fname)
                            .map(|(_, t)| t.clone());
                        out.push(d);
                    }
                }
                ItemKind::Enum(e) => {
                    out.push(decl(
                        self.ast.name(e.name.symbol).to_owned(),
                        What::Enumeration,
                        e.name.span,
                    ));
                    for v in &e.variants {
                        out.push(decl(
                            self.ast.name(v.name.symbol).to_owned(),
                            What::Variant,
                            v.name.span,
                        ));
                    }
                }
                ItemKind::Global(l) => {
                    out.push(decl(
                        self.ast.name(l.name.symbol).to_owned(),
                        What::Global,
                        l.name.span,
                    ));
                }
                ItemKind::Import(i) => {
                    for n in &i.names {
                        let mut d = decl(self.ast.name(n.symbol).to_owned(), What::Import, n.span);
                        d.from = Some(i.from.clone());
                        out.push(d);
                    }
                }
                _ => {}
            }
        }
        for e in &self.ast.exprs {
            if let ExprKind::Lambda { params: p, .. } = &e.kind {
                params.extend(p.iter().map(|(id, _)| id.span));
            }
        }
        for (b, bound) in self.analysis.bindings.iter().enumerate() {
            let Some(span) = bound.span else { continue };
            if let Some(d) = out.iter_mut().find(|d| d.span == span) {
                d.binding = Some(b);
                d.ty.get_or_insert_with(|| bound.ty.clone());
                continue;
            }
            if bound.kind == NameKind::Local && text(span) == bound.name {
                let what = if params.contains(&span) {
                    What::Parameter
                } else {
                    What::Local
                };
                let mut d = decl(bound.name.clone(), what, span);
                d.ty = Some(bound.ty.clone());
                d.binding = Some(b);
                out.push(d);
            }
        }
        out
    }

    /// A function's signature, from its first keyword to its result's type.
    fn signature(&self, start: u32, params_end: u32) -> String {
        let rest = self.source.get(params_end as usize..).unwrap_or("");
        let end = params_end as usize + rest.find('{').unwrap_or(rest.len());
        let text = self.source.get(start as usize..end).unwrap_or("");
        let text = text.trim().strip_prefix("export ").unwrap_or(text.trim());
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// The fields of the structure `name`, as the checker knows them.
    fn fields(&self, name: &str) -> Vec<(String, Ty)> {
        self.analysis
            .structs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f.clone())
            .unwrap_or_default()
    }

    /// A type as Peras writes it in this file: `number` in a managed one.
    fn show(&self, ty: &Ty) -> String {
        let text = ty.to_string();
        if !self.managed {
            return text;
        }
        let mut out = String::new();
        let mut word = String::new();
        for c in text.chars().chain(std::iter::once(' ')) {
            if c.is_alphanumeric() || c == '_' {
                word.push(c);
            } else {
                out.push_str(if word == "f64" { "number" } else { &word });
                word.clear();
                out.push(c);
            }
        }
        out.pop();
        out
    }

    /// The byte offset of `line:column`, both from 1, in characters.
    fn offset(&self, at: &str) -> Option<u32> {
        let (line, column) = at.split_once(':')?;
        let (line, column): (usize, usize) = (line.parse().ok()?, column.parse().ok()?);
        let mut start = 0;
        for (i, l) in self.source.split('\n').enumerate() {
            if i + 1 == line {
                let byte = l.char_indices().nth(column - 1).map_or(l.len(), |(b, _)| b);
                return u32::try_from(start + byte).ok();
            }
            start += l.len() + 1;
        }
        None
    }

    /// `line:column` of a byte offset.
    fn position(&self, offset: u32) -> String {
        let before = self.source.get(..offset as usize).unwrap_or("");
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        format!("{line}:{column}")
    }

    /// The declaration the name at `offset` is, or refers to.
    fn target(&self, offset: u32) -> Option<Target> {
        let inside = |s: Span| s.start <= offset && offset < s.end.max(s.start + 1);
        if let Some(i) = self.decls.iter().position(|d| inside(d.span)) {
            return Some(self.follow(i));
        }
        // A field: the name after a `.`.
        for e in &self.ast.exprs {
            if let ExprKind::Field { name, .. } = &e.kind
                && inside(name.span)
            {
                let ty = self
                    .analysis
                    .types
                    .iter()
                    .find(|(k, _)| self.ast.expr(**k).span == e.span)
                    .map(|(_, t)| t.clone());
                return Some(Target::Field {
                    name: self.ast.name(name.symbol).to_owned(),
                    ty,
                });
            }
        }
        // A name read: the declaration the checker resolved it to.
        let mut best: Option<(Span, usize)> = None;
        for (id, b) in &self.analysis.names {
            let s = self.ast.expr(*id).span;
            if inside(s) && best.is_none_or(|(o, _)| s.end - s.start < o.end - o.start) {
                best = Some((s, *b));
            }
        }
        if let Some((_, b)) = best
            && let Some(i) = self.decls.iter().position(|d| d.binding == Some(b))
        {
            return Some(self.follow(i));
        }
        // A type written, or any name: the declaration of that name.
        let token = self.tokens.iter().find(|t| inside(t.span))?;
        if token.kind != TokenKind::Ident {
            return None;
        }
        let name = token.text(&self.source).to_owned();
        let found = self
            .decls
            .iter()
            .enumerate()
            .filter(|(_, d)| d.name == name && d.span.start <= offset)
            .min_by_key(|(_, d)| {
                (
                    d.what == What::Local || d.what == What::Parameter,
                    u32::MAX - d.span.start,
                )
            })
            .map(|(i, _)| i)
            .or_else(|| self.decls.iter().position(|d| d.name == name))?;
        Some(self.follow(found))
    }

    /// A declaration, followed into the file it is imported from.
    fn follow(&self, i: usize) -> Target {
        let d = &self.decls[i];
        if d.what == What::Import
            && let Some(from) = &d.from
            && let Some(file) = perasc_driver::import_path(&self.path, from)
            && let Ok(text) = perasc_driver::read_source(&file)
        {
            let other = Self::new(&file, text);
            if let Some(j) = other
                .decls
                .iter()
                .position(|o| o.name == d.name && o.what != What::Import)
            {
                return Target::Foreign(Box::new(other), j);
            }
        }
        Target::Here(i)
    }

    /// The answer to a query.
    #[must_use]
    pub fn answer(&self, kind: &str, at: Option<&str>) -> Value {
        let offset = at.and_then(|a| self.offset(a));
        match kind {
            "diagnostics" => {
                let codes = self.diagnostics.get_or_init(|| {
                    let checked = perasc_driver::check_file(&self.path, &self.source);
                    checked
                        .diagnostics
                        .iter()
                        .map(|d| d.code.to_string())
                        .collect()
                });
                json!({ "codes": codes })
            }
            "format" => match perasc_syntax::format::format(&self.source) {
                Ok(text) => json!({ "unchanged": text == self.source, "text": text }),
                Err(_) => json!({ "refused": true }),
            },
            "symbols" => {
                let symbols: Vec<Value> = self
                    .decls
                    .iter()
                    .filter(|d| {
                        matches!(d.what, What::Function | What::Structure | What::Enumeration | What::Global)
                    })
                    .map(|d| {
                        json!({ "name": d.name, "what": d.what.name(), "line": self.position(d.span.start).split(':').next().unwrap_or("0").parse::<u32>().unwrap_or(0) })
                    })
                    .collect();
                json!({ "symbols": symbols })
            }
            "references" => offset
                .and_then(|o| self.target(o))
                .map_or(Value::Null, |t| match t {
                    Target::Here(i) => self.references(i),
                    _ => Value::Null,
                }),
            "completion" => offset.map_or(Value::Null, |o| self.completion(o)),
            _ => json!({ "error": format!("unknown query `{kind}`") }),
        }
    }

    /// Every place the declaration `i` is, and is read.
    fn references(&self, i: usize) -> Value {
        let d = &self.decls[i];
        let mut spans = vec![d.span];
        if let Some(b) = d.binding {
            for (id, n) in &self.analysis.names {
                if *n == b {
                    spans.push(self.ast.expr(*id).span);
                }
            }
        }
        spans.sort_by_key(|s| s.start);
        spans.dedup();
        let at: Vec<String> = spans.iter().map(|s| self.position(s.start)).collect();
        json!({ "at": at })
    }

    /// The names that can be written at `offset`: the members of the value
    /// before a `.`, or the names in scope.
    fn completion(&self, offset: u32) -> Value {
        let before = self.source.get(..offset as usize).unwrap_or("");
        let typed = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
        let names: Vec<String> = if let Some(receiver) = typed.strip_suffix('.') {
            let dot = u32::try_from(receiver.len()).unwrap_or(0);
            self.receiver(dot)
                .map_or_else(Vec::new, |ty| self.members(&ty))
        } else {
            let mut names: Vec<String> = self
                .decls
                .iter()
                .filter(|d| match d.what {
                    What::Local | What::Parameter => d.span.start < offset,
                    What::Field | What::Variant => false,
                    _ => true,
                })
                .map(|d| d.name.clone())
                .collect();
            names.sort();
            names.dedup();
            names
        };
        json!({ "names": names })
    }

    /// The type of the value that ends where the `.` at `dot` begins.
    fn receiver(&self, dot: u32) -> Option<Ty> {
        let ends = self.source.get(..dot as usize)?.trim_end().len();
        let ends = u32::try_from(ends).ok()?;
        let mut best: Option<(u32, Ty)> = None;
        for (id, ty) in &self.analysis.types {
            let s = self.ast.expr(*id).span;
            if s.end == ends
                && best.as_ref().is_none_or(|(start, _)| s.start < *start)
                && *ty != Ty::Error
            {
                best = Some((s.start, ty.clone()));
            }
        }
        if let Some((_, ty)) = best {
            return Some(ty);
        }
        // The tree has no value there, a line being typed: the name before.
        let token = self.tokens.iter().rev().find(|t| t.span.end <= ends)?;
        match self.target(token.span.start)? {
            Target::Here(i) => self.decls[i].ty.clone(),
            Target::Foreign(other, j) => other.decls[j].ty.clone(),
            Target::Field { ty, .. } => ty,
        }
    }

    /// The fields and methods of a value of type `ty`.
    fn members(&self, ty: &Ty) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        if let Ty::Struct(_, name) = ty {
            names.extend(self.fields(name).into_iter().map(|(n, _)| n));
        }
        names.extend(
            perasc_check::builtins::members(ty)
                .iter()
                .map(|s| (*s).to_owned()),
        );
        names.sort();
        names.dedup();
        names
    }
}

/// What a position names.
enum Target {
    Here(usize),
    Foreign(Box<Opened>, usize),
    Field { name: String, ty: Option<Ty> },
}

/// Answers through `opened`, filling what the target needs from it.
fn answer_in(opened: &Opened, kind: &str, at: Option<&str>) -> Value {
    let offset = at.and_then(|a| opened.offset(a));
    let target = offset.and_then(|o| opened.target(o));
    match (kind, target) {
        ("hover", Some(t)) => match t {
            Target::Here(i) => hover(opened, i),
            Target::Foreign(o, j) => hover(&o, j),
            field @ Target::Field { .. } => {
                let Target::Field { name, ty } = field else {
                    return Value::Null;
                };
                json!({ "name": name, "what": "field", "type": ty.map(|t| opened.show(&t)) })
            }
        },
        ("definition", Some(t)) => match t {
            Target::Here(i) => {
                json!({ "file": opened.path.display().to_string(), "at": opened.position(opened.decls[i].span.start) })
            }
            Target::Foreign(o, j) => {
                json!({ "file": o.path.display().to_string(), "at": o.position(o.decls[j].span.start) })
            }
            Target::Field { .. } => Value::Null,
        },
        ("hover" | "definition", None) => Value::Null,
        _ => opened.answer(kind, at),
    }
}

/// The hover of declaration `i` of `opened`.
fn hover(opened: &Opened, i: usize) -> Value {
    let d = &opened.decls[i];
    let ty = match d.what {
        What::Function => d.signature.clone(),
        What::Structure | What::Enumeration => Some(d.name.clone()),
        _ => d.ty.as_ref().map(|t| opened.show(t)),
    };
    json!({ "name": d.name, "what": d.what.name(), "type": ty })
}

/// `perasc serve`: a request a line, an answer a line, every file opened
/// kept with its analysis until it, or a file it imports, changes.
///
/// # Errors
///
/// When standard input or output fails.
pub fn serve(out: &mut dyn Write) -> io::Result<u8> {
    let mut files: HashMap<String, Opened> = HashMap::new();
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let field = |k: &str| request.get(k).and_then(Value::as_str).map(str::to_owned);
        let file = field("file").unwrap_or_default();
        let result = match field("method").as_deref() {
            Some("open" | "change") => {
                let text = field("text")
                    .unwrap_or_else(|| std::fs::read_to_string(&file).unwrap_or_default());
                perasc_driver::set_open(Path::new(&file), Some(text.clone()));
                files.insert(file.clone(), Opened::new(Path::new(&file), text));
                reanalyse_importers(&mut files, &file);
                json!({ "ok": true })
            }
            Some("close") => {
                files.remove(&file);
                perasc_driver::set_open(Path::new(&file), None);
                reanalyse_importers(&mut files, &file);
                json!({ "ok": true })
            }
            Some("query") => {
                if !files.contains_key(&file) {
                    let text = std::fs::read_to_string(&file).unwrap_or_default();
                    files.insert(file.clone(), Opened::new(Path::new(&file), text));
                }
                let kind = field("kind").unwrap_or_default();
                files
                    .get(&file)
                    .map_or(Value::Null, |o| answer_in(o, &kind, field("at").as_deref()))
            }
            Some("exit") => break,
            _ => json!({ "error": "unknown method" }),
        };
        writeln!(out, "{}", json!({ "id": id, "result": result }))?;
        out.flush()?;
    }
    Ok(0)
}

/// Analyses again each file opened that imports `changed`.
fn reanalyse_importers(files: &mut HashMap<String, Opened>, changed: &str) {
    let changed = std::fs::canonicalize(changed).unwrap_or_else(|_| PathBuf::from(changed));
    let importers: Vec<String> = files
        .iter()
        .filter(|(_, o)| o.imports().contains(&changed))
        .map(|(f, _)| f.clone())
        .collect();
    for f in importers {
        if let Some(o) = files.remove(&f) {
            let source = o.source;
            files.insert(f.clone(), Opened::new(Path::new(&f), source));
        }
    }
}
