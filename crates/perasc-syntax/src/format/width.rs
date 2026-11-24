//! A printer at a width, as Prettier prints TypeScript.
//!
//! The tokens are read into a tree of their brackets; a block `{ …; … }`
//! puts each statement on its line, a list (arguments, an array, the fields
//! of a structure, the arms of a `match`) stays on one line if it fits and
//! puts each element on its line if it does not, with a trailing comma;
//! a chain of three calls or more breaks before each `.` if it does not fit.

use super::{Item, WIDTH, space, unary_after};
use crate::token::TokenKind as K;

/// A piece of the tree of brackets.
#[derive(Debug)]
enum Node {
    Tok(usize),
    /// A bracket, its contents, and the bracket that closes it.
    Group {
        open: usize,
        body: Vec<Node>,
        close: usize,
    },
    /// A template literal, printed on one line.
    Template(Vec<Node>),
}

impl Node {
    fn first(&self) -> usize {
        match self {
            Self::Tok(i) | Self::Group { open: i, .. } => *i,
            Self::Template(nodes) => nodes.first().map_or(0, Self::first),
        }
    }
    fn last(&self) -> usize {
        match self {
            Self::Tok(i) | Self::Group { close: i, .. } => *i,
            Self::Template(nodes) => nodes.last().map_or(0, Self::last),
        }
    }
}

/// What the printer prints.
#[derive(Debug, Clone)]
enum Doc {
    Text(String),
    /// A space on one line, a line break otherwise.
    Line,
    /// Nothing on one line, a line break otherwise.
    Soft,
    /// A line break, always; two in a row make one.
    Hard,
    /// A blank line, always.
    Blank,
    Indent(Vec<Doc>),
    Group(Vec<Doc>),
    /// Text printed only when its group is broken.
    IfBreak(&'static str),
}

/// Lays `items` out at [`WIDTH`].
pub(super) fn format(items: &[Item<'_>]) -> String {
    let mut at = 0;
    let nodes = tree(items, &mut at, None);
    let mut b = Builder {
        items,
        prev: None,
        prefix: false,
    };
    let mut docs = b.statements(&nodes, 0);
    // The comments after the last token.
    if let Some(eof) = items.last() {
        comments_of(eof, &mut docs, true);
    }
    let mut out = print(&docs);
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    out.push('\n');
    out
}

/// The tree of brackets from `items[*at]` to the bracket that closes `open`.
fn tree(items: &[Item<'_>], at: &mut usize, open: Option<K>) -> Vec<Node> {
    let mut nodes = Vec::new();
    while *at < items.len() {
        let i = *at;
        let kind = items[i].kind;
        match kind {
            K::Eof => return nodes,
            K::RParen | K::RBracket | K::RBrace if open.is_some() => return nodes,
            K::LParen | K::LBracket | K::LBrace => {
                *at += 1;
                let body = tree(items, at, Some(kind));
                let close = (*at).min(items.len() - 1);
                *at += 1;
                nodes.push(Node::Group {
                    open: i,
                    body,
                    close,
                });
            }
            K::TemplateHead => {
                let mut parts = vec![Node::Tok(i)];
                *at += 1;
                loop {
                    let mut inner = tree(items, at, Some(K::TemplateHead));
                    parts.append(&mut inner);
                    let Some(t) = items.get(*at) else { break };
                    parts.push(Node::Tok(*at));
                    *at += 1;
                    if t.kind != K::TemplateMiddle {
                        break;
                    }
                }
                nodes.push(Node::Template(parts));
            }
            K::TemplateMiddle | K::TemplateTail if open == Some(K::TemplateHead) => {
                return nodes;
            }
            _ => {
                nodes.push(Node::Tok(i));
                *at += 1;
            }
        }
    }
    nodes
}

struct Builder<'s, 'a> {
    items: &'s [Item<'a>],
    prev: Option<K>,
    prefix: bool,
}

impl Builder<'_, '_> {
    fn kind(&self, i: usize) -> K {
        self.items[i].kind
    }

    /// The statements of a block or of the file, one a line.
    fn statements(&mut self, nodes: &[Node], depth: usize) -> Vec<Doc> {
        let mut docs = Vec::new();
        for (n, stmt) in self.split(nodes).iter().enumerate() {
            let first = stmt[0].first();
            if n > 0 {
                self.trailing(first, &mut docs);
                docs.push(Doc::Hard);
                if self.blank_before(first) {
                    docs.push(Doc::Blank);
                }
            }
            self.leading(first, &mut docs, n > 0 || depth > 0);
            self.prev = None;
            docs.push(self.statement(stmt));
        }
        docs
    }

    /// `nodes` cut into statements: after a `;`, after a block that no
    /// `else`, `;`, `,`, `.` or operator continues, after an attribute.
    fn split<'n>(&self, nodes: &'n [Node]) -> Vec<&'n [Node]> {
        let mut out = Vec::new();
        let mut start = 0;
        for (j, node) in nodes.iter().enumerate() {
            let next = nodes.get(j + 1).map(|n| self.items[n.first()].clone());
            let ends = match node {
                Node::Tok(i) => {
                    self.kind(*i) == K::Semicolon
                        || (j > start
                            && self.kind(nodes[start].first()) == K::At
                            && next.as_ref().is_some_and(|n| n.kind != K::LParen)
                            && !matches!(node, Node::Tok(t) if self.kind(*t) == K::At))
                }
                Node::Group { open, .. } => {
                    (self.kind(*open) == K::LBrace
                        && next.as_ref().is_some_and(|n| {
                            let continues = matches!(
                                n.kind,
                                K::Else
                                    | K::Semicolon
                                    | K::Comma
                                    | K::Dot
                                    | K::Question
                                    | K::RParen
                            ) || (n.kind == K::Ident && n.text == "from")
                                || is_operator(n.kind);
                            !continues
                        }))
                        || (self.kind(*open) == K::LParen
                            && self.kind(nodes[start].first()) == K::At)
                }
                Node::Template(_) => false,
            };
            if ends && j + 1 < nodes.len() {
                out.push(&nodes[start..=j]);
                start = j + 1;
            }
        }
        if start < nodes.len() {
            out.push(&nodes[start..]);
        }
        out
    }

    /// One statement: a group whose `&&`, `||`, `+`, `-` and chain of
    /// calls break together if it does not fit.
    fn statement(&mut self, nodes: &[Node]) -> Doc {
        let calls = nodes
            .windows(3)
            .filter(|w| {
                matches!(w[0], Node::Tok(i) if self.kind(i) == K::Dot)
                    && matches!(w[1], Node::Tok(i) if self.kind(i) == K::Ident)
                    && matches!(w[2], Node::Group { open, .. } if self.kind(open) == K::LParen)
            })
            .count();
        let mut breaks = false;
        let mut docs = Vec::new();
        for (j, node) in nodes.iter().enumerate() {
            let first = node.first();
            let kind = self.kind(first);
            if j > 0 {
                let after_logic = matches!(self.prev, Some(K::AmpAmp | K::PipePipe))
                    || (matches!(self.prev, Some(K::Plus | K::Minus)) && !self.prefix);
                let breaks_chain = calls >= 3
                    && kind == K::Dot
                    && matches!(nodes.get(j + 2), Some(Node::Group { open, .. }) if self.kind(*open) == K::LParen);
                if after_logic {
                    breaks = true;
                    self.trailing(first, &mut docs);
                    docs.push(Doc::Line);
                    self.leading(first, &mut docs, true);
                    self.prev = None;
                } else if breaks_chain {
                    breaks = true;
                    self.trailing(first, &mut docs);
                    docs.push(Doc::Soft);
                    self.leading(first, &mut docs, true);
                    self.prev = None;
                } else {
                    comments_of(&self.items[first], &mut docs, true);
                }
            }
            self.node(node, &mut docs);
        }
        if breaks {
            Doc::Group(vec![Doc::Indent(docs)])
        } else {
            Doc::Group(docs)
        }
    }

    /// A node, its comments printed before it by the caller.
    fn node(&mut self, node: &Node, docs: &mut Vec<Doc>) {
        match node {
            Node::Tok(i) => self.token(*i, docs),
            Node::Template(parts) => {
                // A template literal is never broken inside its `${…}`.
                let mut inner = Vec::new();
                for (j, part) in parts.iter().enumerate() {
                    if j > 0 {
                        comments_of(&self.items[part.first()], &mut inner, true);
                    }
                    self.node(part, &mut inner);
                }
                flatten(inner, docs);
            }
            Node::Group { open, body, close } => self.group(*open, body, *close, docs),
        }
    }

    /// A token's text, a space before it if the table says so.
    fn token(&mut self, i: usize, docs: &mut Vec<Doc>) {
        let item = &self.items[i];
        if let Some(p) = self.prev
            && space(p, item.kind, self.prefix, item.spaced)
        {
            docs.push(Doc::Text(" ".to_owned()));
        }
        docs.push(Doc::Text(item.text.to_owned()));
        self.prefix = match item.kind {
            K::Minus | K::Plus | K::PlusPlus | K::MinusMinus => unary_after(self.prev),
            _ => false,
        };
        self.prev = Some(item.kind);
    }

    /// A bracket and its contents: a block, or a list.
    fn group(&mut self, open: usize, body: &[Node], close: usize, docs: &mut Vec<Doc>) {
        let kind = self.kind(open);
        let before = self.prev;
        self.token(open, docs);
        if body.is_empty() {
            let mut inner = Vec::new();
            comments_of(&self.items[close], &mut inner, false);
            if !inner.is_empty() {
                docs.push(Doc::Indent(inner));
                docs.push(Doc::Hard);
            }
            self.prev = Some(kind);
            self.token(close, docs);
            return;
        }
        let is_block = kind == K::LBrace && self.is_block(body);
        if is_block {
            let first = body[0].first();
            let mut inner = Vec::new();
            self.trailing(first, docs);
            inner.push(Doc::Hard);
            inner.extend(self.statements(body, 1));
            comments_of(&self.items[close], &mut inner, false);
            docs.push(Doc::Indent(inner));
            docs.push(Doc::Hard);
            self.prev = Some(K::LBrace);
            self.prefix = false;
            let text = self.items[close].text.to_owned();
            docs.push(Doc::Text(text));
            self.prev = Some(K::RBrace);
            return;
        }
        self.list(open, before, body, close, docs);
    }

    /// A list in brackets: its elements cut at the commas at its top, on
    /// one line if they fit, one a line otherwise.
    fn list(
        &mut self,
        open: usize,
        before: Option<K>,
        body: &[Node],
        close: usize,
        docs: &mut Vec<Doc>,
    ) {
        let kind = self.kind(open);
        let mut elements: Vec<&[Node]> = Vec::new();
        let mut commas = 0;
        let mut start = 0;
        for (j, n) in body.iter().enumerate() {
            if matches!(n, Node::Tok(i) if self.kind(*i) == K::Comma) {
                elements.push(&body[start..j]);
                commas += 1;
                start = j + 1;
            }
        }
        let trailing_comma = start == body.len();
        if !trailing_comma {
            elements.push(&body[start..]);
        }
        let edge = if kind == K::LBrace {
            Doc::Line
        } else {
            Doc::Soft
        };
        // The last element a block, the rest on one line: hugged.
        let hug = kind == K::LParen
            && elements.last().is_some_and(|e| {
                e.last().is_some_and(|n| {
                    matches!(n, Node::Group { open, body, .. }
                        if self.kind(*open) == K::LBrace && self.is_block(body))
                })
            })
            && !elements.is_empty()
            && elements[..elements.len() - 1].iter().all(|e| {
                e.iter().all(
                    |n| !matches!(n, Node::Group { open, .. } if self.kind(*open) == K::LBrace),
                )
            });
        let declares = kind == K::LBrace && self.declares(open);
        let mut inner = Vec::new();
        for (j, element) in elements.iter().enumerate() {
            let Some(first) = element.first().map(Node::first) else {
                continue;
            };
            self.trailing(first, &mut inner);
            if j > 0 {
                inner.push(if hug {
                    Doc::Text(" ".to_owned())
                } else {
                    Doc::Line
                });
                // A declaration keeps the blank lines between its fields.
                if declares && self.blank_before(first) {
                    inner.push(Doc::Blank);
                }
            } else if !hug {
                inner.push(edge.clone());
            } else if kind == K::LBrace {
                inner.push(Doc::Text(" ".to_owned()));
            }
            self.leading(first, &mut inner, true);
            self.prev = None;
            inner.push(self.element(element));
            if j + 1 < elements.len() {
                self.prev = None;
                let comma = comma_after(element);
                comments_of(&self.items[comma], &mut inner, true);
                inner.push(Doc::Text(",".to_owned()));
                self.prev = Some(K::Comma);
            }
        }
        if trailing_comma && let Some(last) = elements.last() {
            let comma = comma_after(last);
            comments_of(&self.items[comma], &mut inner, true);
        }
        // A call's arguments take a trailing comma even when alone; a
        // parenthesized expression does not, which would make it a tuple.
        let call =
            kind == K::LParen && matches!(before, Some(K::Ident | K::RParen | K::RBracket | K::Gt));
        if (commas > 0 || call) && !hug {
            inner.push(Doc::IfBreak(","));
        }
        // The body of a declaration always breaks, as Prettier's interfaces.
        if declares {
            inner.push(Doc::Hard);
        }
        comments_of(&self.items[close], &mut inner, false);
        let mut group = if hug {
            inner
        } else {
            vec![Doc::Indent(inner), edge]
        };
        self.prev = Some(kind);
        self.prefix = false;
        let text = self.items[close].text.to_owned();
        group.push(Doc::Text(text));
        self.prev = Some(self.kind(close));
        docs.push(Doc::Group(group));
    }

    /// Whether a `{ … }` holds statements: a `;`, or more than one.
    fn is_block(&self, body: &[Node]) -> bool {
        body.iter()
            .any(|n| matches!(n, Node::Tok(i) if self.kind(*i) == K::Semicolon))
            || self.split(body).len() > 1
    }

    /// Whether the `{` at `open` opens the body of a `struct`, an `enum`,
    /// an `interface`, a `trait` or an `impl`.
    fn declares(&self, open: usize) -> bool {
        for i in (0..open).rev().take(12) {
            match self.kind(i) {
                K::Struct | K::Enum | K::Interface | K::Trait | K::Impl => return true,
                K::Ident | K::Lt | K::Gt | K::Comma | K::Colon => {}
                _ => return false,
            }
        }
        false
    }

    /// An element of a list, laid out as a statement.
    fn element(&mut self, nodes: &[Node]) -> Doc {
        self.statement(nodes)
    }

    /// Whether the source left a blank line before item `i`.
    fn blank_before(&self, i: usize) -> bool {
        let item = &self.items[i];
        item.comments
            .iter()
            .find(|c| c.breaks > 0)
            .map_or(item.breaks, |c| c.breaks)
            > 1
    }

    /// The comments on the line of what precedes item `i`.
    fn trailing(&self, i: usize, docs: &mut Vec<Doc>) {
        for c in self.items[i].comments.iter().take_while(|c| c.breaks == 0) {
            docs.push(Doc::Text(format!(" {}", c.text)));
            if c.line {
                docs.push(Doc::Hard);
            }
        }
    }

    /// The comments on lines of their own before item `i`; `after_token`
    /// says that a token precedes it, whose line holds the others.
    fn leading(&self, i: usize, docs: &mut Vec<Doc>, after_token: bool) {
        let item = &self.items[i];
        let inside = after_token;
        let mut own = !after_token;
        for (n, c) in item.comments.iter().enumerate() {
            if !own && c.breaks == 0 {
                continue;
            }
            own = true;
            if n > 0 && c.breaks > 1 && inside {
                docs.push(Doc::Blank);
            }
            docs.push(Doc::Hard);
            docs.push(Doc::Text(c.text.to_owned()));
            docs.push(Doc::Hard);
        }
        if !own && item.comments.last().is_some_and(|c| c.line) {
            docs.push(Doc::Hard);
        }
        if own && item.breaks > 1 {
            docs.push(Doc::Blank);
        }
    }
}

/// Every comment before `item`, where no line break is planned; `after`
/// says that something follows them on their line.
fn comments_of(item: &Item<'_>, docs: &mut Vec<Doc>, after: bool) {
    {
        for c in &item.comments {
            if c.breaks > 0 {
                docs.push(Doc::Hard);
                docs.push(Doc::Text(c.text.to_owned()));
            } else {
                docs.push(Doc::Text(format!(" {}", c.text)));
            }
            if c.line || (c.breaks > 0 && after) {
                docs.push(Doc::Hard);
            } else if after {
                docs.push(Doc::Text(" ".to_owned()));
            }
        }
        if after && !item.comments.is_empty() && item.breaks > 0 {
            docs.push(Doc::Hard);
        }
    }
}

/// The comma that follows an element of a list.
fn comma_after(element: &[Node]) -> usize {
    element.last().map_or(0, |n| n.last() + 1)
}

/// An operator that a block before it is the left side of.
const fn is_operator(kind: K) -> bool {
    matches!(
        kind,
        K::Eq
            | K::EqEq
            | K::BangEq
            | K::Lt
            | K::LtEq
            | K::Gt
            | K::GtEq
            | K::Plus
            | K::Minus
            | K::Star
            | K::Slash
            | K::Percent
            | K::AmpAmp
            | K::PipePipe
            | K::Amp
            | K::Pipe
            | K::Caret
            | K::FatArrow
    )
}

/// `docs` on one line: every group flat, every possible break a space or
/// nothing.
fn flatten(docs: Vec<Doc>, out: &mut Vec<Doc>) {
    for d in docs {
        match d {
            Doc::Line => out.push(Doc::Text(" ".to_owned())),
            Doc::Soft | Doc::IfBreak(_) => {}
            Doc::Indent(inner) | Doc::Group(inner) => flatten(inner, out),
            other => out.push(other),
        }
    }
}

/// Whether a doc holds a line break that always breaks.
fn has_hard(docs: &[Doc]) -> bool {
    docs.iter().any(|d| match d {
        Doc::Hard | Doc::Blank => true,
        Doc::Indent(inner) | Doc::Group(inner) => has_hard(inner),
        _ => false,
    })
}

struct Printer {
    out: String,
    column: usize,
    /// A line break asked for and not printed yet, at this indentation.
    pending: Option<usize>,
    blank: bool,
}

impl Printer {
    fn text(&mut self, t: &str) {
        if let Some(indent) = self.pending.take() {
            if !self.out.is_empty() {
                self.out.push('\n');
                if self.blank {
                    self.out.push('\n');
                }
            }
            self.blank = false;
            for _ in 0..indent {
                self.out.push_str("  ");
            }
            self.column = indent * 2;
            let t = t.trim_start_matches(' ');
            self.out.push_str(t);
            self.column += t.chars().count();
            return;
        }
        self.out.push_str(t);
        self.column += t.chars().count();
    }
}

/// A doc to print, at an indentation, in a group broken or not.
type Command<'d> = (usize, bool, &'d Doc);

/// Whether `next`, then what follows it on the stack, fits in `room`
/// columns up to the first line break.
fn fits(next: &[Command<'_>], rest: &[Command<'_>], mut room: usize) -> bool {
    let mut todo: Vec<Command<'_>> = next.to_vec();
    let mut rest = rest.iter().rev();
    loop {
        let Some((indent, broken, doc)) = todo.pop() else {
            match rest.next() {
                Some(c) => {
                    todo.push(*c);
                    continue;
                }
                None => return true,
            }
        };
        match doc {
            Doc::Text(t) => match room.checked_sub(t.chars().count()) {
                Some(r) => room = r,
                None => return false,
            },
            Doc::IfBreak(t) => {
                if broken {
                    match room.checked_sub(t.len()) {
                        Some(r) => room = r,
                        None => return false,
                    }
                }
            }
            Doc::Line => {
                if broken {
                    return true;
                }
                match room.checked_sub(1) {
                    Some(r) => room = r,
                    None => return false,
                }
            }
            Doc::Soft => {
                if broken {
                    return true;
                }
            }
            Doc::Hard | Doc::Blank => return true,
            Doc::Indent(inner) => {
                for d in inner.iter().rev() {
                    todo.push((indent + 1, broken, d));
                }
            }
            Doc::Group(inner) => {
                for d in inner.iter().rev() {
                    todo.push((indent, broken, d));
                }
            }
        }
    }
}

/// Prints `docs` at [`WIDTH`].
fn print(docs: &[Doc]) -> String {
    let mut p = Printer {
        out: String::new(),
        column: 0,
        pending: None,
        blank: false,
    };
    let mut stack: Vec<Command<'_>> = docs.iter().rev().map(|d| (0, true, d)).collect();
    while let Some((indent, broken, doc)) = stack.pop() {
        match doc {
            Doc::Text(t) => p.text(t),
            Doc::Line => {
                if broken {
                    p.pending = Some(indent);
                } else {
                    p.text(" ");
                }
            }
            Doc::Soft => {
                if broken {
                    p.pending = Some(indent);
                }
            }
            Doc::Hard => p.pending = Some(indent),
            Doc::Blank => {
                if !p.out.is_empty() {
                    p.blank = true;
                }
                p.pending = Some(indent);
            }
            Doc::IfBreak(t) => {
                if broken {
                    p.text(t);
                }
            }
            Doc::Indent(inner) => {
                for d in inner.iter().rev() {
                    stack.push((indent + 1, broken, d));
                }
            }
            Doc::Group(inner) => {
                let column = p.pending.map_or(p.column, |i| i * 2);
                let room = WIDTH.saturating_sub(column);
                let flat: Vec<Command<'_>> =
                    inner.iter().rev().map(|d| (indent, false, d)).collect();
                let flat_fits = !has_hard(inner) && fits(&flat, &stack, room);
                for d in inner.iter().rev() {
                    stack.push((indent, !flat_fits, d));
                }
            }
        }
    }
    p.out
}
