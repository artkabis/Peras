//! The parser: hand-written recursive descent, with Pratt parsing for expressions.
//!
//! It reports every syntax error of a file in one pass. After an error, it skips
//! to the next statement boundary (`;`, `}`, or a keyword starting a line) and goes
//! on; until then, further errors are suppressed, so one mistake gives one error.
//! Tokens the lexer already reported ([`TokenKind::Error`]) never cause a second
//! error. Nesting is bounded ([`MAX_DEPTH`]) so no input can exhaust the stack.

mod control;
mod errors;
mod expr;

use perasc_diag::{Diagnostic, Span};

use crate::ast::{
    Ast, Block, Convention, Field, FnDecl, Ident, Item, ItemKind, Local, Param, Stmt, StmtKind,
    StructDecl, Type, TypeKind,
};
use crate::lexer::lex;
use crate::token::{Token, TokenKind};

/// Deepest nesting of expressions and types the parser accepts.
pub const MAX_DEPTH: usize = 128;

/// The result of parsing one file.
#[derive(Debug, Clone)]
pub struct Parsed {
    /// The tree, with error nodes where parsing failed.
    pub ast: Ast,
    /// Lexical and syntax errors, sorted by position.
    pub diagnostics: Vec<Diagnostic>,
}

/// Lexes and parses a source file, gathering the errors of both stages.
#[must_use]
pub fn parse(source: &str) -> Parsed {
    let lexed = lex(source);
    let mut parser = Parser::new(source, &lexed.tokens);
    // Where unterminated strings end: what they swallowed (`;`, `)`) is not reported again.
    parser.unterminated = lexed
        .diagnostics
        .iter()
        .filter(|d| d.code == perasc_diag::codes::UNTERMINATED_STRING)
        .filter_map(|d| d.labels.iter().find(|l| !l.primary).map(|l| l.span.end))
        .collect();
    parser.module();
    let mut diagnostics = lexed.diagnostics;
    diagnostics.append(&mut parser.diagnostics);
    diagnostics.sort_by_key(|d| d.primary_span().map(|s| s.start));
    Parsed {
        ast: parser.ast,
        diagnostics,
    }
}

/// What a name names, for "expected a … name" messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameOf {
    Function,
    Variable,
    Parameter,
    Field,
    Attribute,
    Struct,
}

impl NameOf {
    const fn key(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Variable => "variable",
            Self::Parameter => "parameter",
            Self::Field => "field",
            Self::Attribute => "attribute",
            Self::Struct => "struct",
        }
    }
}

struct Parser<'src> {
    source: &'src str,
    tokens: &'src [Token],
    pos: usize,
    ast: Ast,
    diagnostics: Vec<Diagnostic>,
    /// Current nesting of expressions and types.
    depth: usize,
    /// Set after an error until the next statement boundary: suppresses cascades.
    recovering: bool,
    /// End offsets of the strings the lexer reported unterminated.
    unterminated: Vec<u32>,
    /// Number of loops around the current statement, for `break` and `continue`.
    loop_depth: usize,
}

const EOF: Token = Token {
    kind: TokenKind::Eof,
    span: Span::new(0, 0),
};

impl<'src> Parser<'src> {
    fn new(source: &'src str, tokens: &'src [Token]) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
            ast: Ast::default(),
            diagnostics: Vec::new(),
            depth: 0,
            recovering: false,
            unterminated: Vec::new(),
            loop_depth: 0,
        }
    }

    // --- Token navigation ---------------------------------------------------

    fn nth(&self, n: usize) -> Token {
        self.tokens
            .get(self.pos + n)
            .or_else(|| self.tokens.last())
            .copied()
            .unwrap_or(EOF)
    }

    fn peek(&self) -> Token {
        self.nth(0)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn at_eof(&self) -> bool {
        self.at(TokenKind::Eof)
    }

    fn bump(&mut self) -> Token {
        let token = self.peek();
        if token.kind != TokenKind::Eof {
            self.pos += 1;
        }
        token
    }

    fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        self.at(kind).then(|| self.bump())
    }

    fn text(&self, token: Token) -> &'src str {
        token.text(self.source)
    }

    fn slice(&self, span: Span) -> &'src str {
        let start = usize::try_from(span.start).unwrap_or(0);
        let end = usize::try_from(span.end).unwrap_or(0);
        self.source.get(start..end).unwrap_or("")
    }

    /// End of the last consumed token: where a missing `;` or `)` belongs.
    fn prev_end(&self) -> u32 {
        self.pos
            .checked_sub(1)
            .and_then(|i| self.tokens.get(i))
            .map_or(0, |t| t.span.end)
    }

    /// Span from `start` to the end of the last consumed token.
    fn since(&self, start: u32) -> Span {
        Span::new(start, self.prev_end().max(start))
    }

    /// Whether `token` is the first token of its line.
    fn starts_line(&self, token: Token) -> bool {
        let before = self.slice(Span::new(self.prev_end(), token.span.start));
        self.pos == 0 || before.contains('\n')
    }

    /// Whether the last token is a string the lexer reported unterminated: it may
    /// have swallowed the `;` or `)` that follows, already covered by its fix.
    fn after_unterminated_string(&self) -> bool {
        self.unterminated.contains(&self.prev_end())
    }

    /// Whether the current token is the identifier `word`.
    fn at_word(&self, word: &str) -> bool {
        self.at(TokenKind::Ident) && self.text(self.peek()) == word
    }

    // --- Errors -------------------------------------------------------------

    /// Records an error and enters recovery, unless already recovering.
    fn error(&mut self, diagnostic: Diagnostic) {
        if !self.recovering {
            self.diagnostics.push(diagnostic);
        }
        self.recovering = true;
    }

    /// Records an error after which parsing goes on normally.
    fn soft_error(&mut self, diagnostic: Diagnostic) {
        if !self.recovering {
            self.diagnostics.push(diagnostic);
        }
    }

    /// Reports that `found` cannot appear here; tokens already reported are skipped silently.
    fn unexpected(&mut self, found: Token, label: perasc_diag::Message) {
        if found.kind == TokenKind::Error {
            self.recovering = true;
            return;
        }
        if errors::is_assign_op(found.kind) {
            let operator = self.text(found).to_owned();
            self.error(errors::assignment_in_expression(found.span, &operator));
            return;
        }
        let d = errors::unexpected(found, self.text(found), label);
        self.error(d);
    }

    /// Consumes `kind` or reports what was found instead.
    fn expect(&mut self, kind: TokenKind, symbol: &'static str) -> Option<Token> {
        if let Some(token) = self.eat(kind) {
            return Some(token);
        }
        let found = self.peek();
        self.unexpected(found, errors::expected_token(symbol));
        None
    }

    /// Consumes the closing delimiter of `open`, or reports it missing.
    fn expect_close(&mut self, open: Token, close: TokenKind, symbol: &'static str) -> bool {
        if self.eat(close).is_some() {
            return true;
        }
        if self.after_unterminated_string() {
            self.recovering = true;
            return false;
        }
        let found = self.peek();
        let ends_here = matches!(
            found.kind,
            TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof
        ) || self.starts_line(found);
        if ends_here && found.kind != TokenKind::Error {
            let open_text = self.text(open).to_owned();
            // A body left open at the end of the file closes on a line of its own.
            let (at, insertion) = if close == TokenKind::RBrace && found.kind == TokenKind::Eof {
                let newline = if self.source.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                (found.span.start, format!("}}{newline}"))
            } else {
                (self.prev_end(), symbol.to_owned())
            };
            let found_text = self.text(found);
            self.error(errors::unclosed(
                (open, &open_text),
                (symbol, at, insertion),
                found,
                found_text,
            ));
        } else {
            self.unexpected(found, errors::expected_separator(symbol));
        }
        false
    }

    /// Expects the `;` ending a statement.
    fn expect_semicolon(&mut self) {
        if self.eat(TokenKind::Semicolon).is_some() || self.after_unterminated_string() {
            return;
        }
        let found = self.peek();
        let missing =
            matches!(found.kind, TokenKind::RBrace | TokenKind::Eof) || self.starts_line(found);
        if missing && !self.recovering && found.kind != TokenKind::Error {
            // The statement is complete: report the `;` and go on normally.
            self.soft_error(errors::missing_semicolon(self.prev_end()));
            return;
        }
        if !self.recovering {
            self.unexpected(found, errors::expected_token(";"));
        }
        self.recover_statement();
    }

    /// Skips to the next statement boundary: after a `;`, before a `}`, or before a
    /// keyword starting a line. Iterative, so it is safe on any input.
    fn recover_statement(&mut self) {
        let mut depth = 0usize;
        loop {
            let token = self.peek();
            match token.kind {
                TokenKind::Eof => break,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
                TokenKind::RBrace if depth == 0 => break,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Semicolon if depth == 0 => {
                    self.bump();
                    break;
                }
                TokenKind::Let
                | TokenKind::Const
                | TokenKind::Return
                | TokenKind::Fn
                | TokenKind::If
                | TokenKind::For
                | TokenKind::While
                | TokenKind::Break
                | TokenKind::Continue
                    if depth == 0 && self.starts_line(token) =>
                {
                    break;
                }
                _ => {}
            }
            self.bump();
        }
    }

    /// Skips to the next declaration at the top level, past the current token
    /// first when `skip_current` is set.
    fn recover_item(&mut self, skip_current: bool) {
        let mut depth = 0usize;
        if skip_current {
            self.bump();
        }
        loop {
            let token = self.peek();
            match token.kind {
                TokenKind::Eof => break,
                TokenKind::LBrace | TokenKind::LParen | TokenKind::LBracket => depth += 1,
                TokenKind::RBrace | TokenKind::RParen | TokenKind::RBracket => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Fn
                | TokenKind::Struct
                | TokenKind::Export
                | TokenKind::Async
                | TokenKind::At
                | TokenKind::Let
                | TokenKind::Const
                    if depth == 0 =>
                {
                    break;
                }
                _ => {}
            }
            self.bump();
        }
    }

    /// Enters one level of nesting; false (and one error) when too deep.
    fn enter(&mut self) -> bool {
        if self.depth >= MAX_DEPTH {
            let span = self.peek().span;
            self.error(errors::too_deep(span));
            return false;
        }
        self.depth += 1;
        true
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // --- Names and types ----------------------------------------------------

    fn ident(&mut self, of: NameOf) -> Option<Ident> {
        let token = self.peek();
        if token.kind == TokenKind::Ident {
            self.bump();
            let symbol = self.ast.interner.intern(self.text(token));
            return Some(Ident {
                symbol,
                span: token.span,
            });
        }
        if token.kind == TokenKind::Error {
            self.recovering = true;
        } else {
            let d = errors::expected_name(of.key(), token, self.text(token));
            self.error(d);
        }
        None
    }

    fn ty(&mut self) -> Type {
        let start = self.peek().span.start;
        if !self.enter() {
            return Type {
                kind: TypeKind::Error,
                span: self.peek().span,
            };
        }
        let token = self.peek();
        let kind = if token.kind == TokenKind::Ident {
            self.bump();
            let name = Ident {
                symbol: self.ast.interner.intern(self.text(token)),
                span: token.span,
            };
            let mut args = Vec::new();
            if let Some(open) = self.eat(TokenKind::Lt) {
                loop {
                    args.push(self.ty());
                    if self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                }
                self.expect_close(open, TokenKind::Gt, ">");
            }
            TypeKind::Named { name, args }
        } else {
            if token.kind == TokenKind::Error {
                self.recovering = true;
                self.bump();
            } else {
                let d = errors::expected_type(token, self.text(token));
                self.error(d);
            }
            TypeKind::Error
        };
        self.leave();
        Type {
            kind,
            span: self.since(start),
        }
    }

    // --- Declarations -------------------------------------------------------

    fn module(&mut self) {
        while !self.at_eof() {
            self.recovering = false;
            let before = self.pos;
            match self.item() {
                Some(item) => self.ast.items.push(item),
                // A failed declaration: skip what is left of it.
                None => self.recover_item(false),
            }
            if self.pos == before {
                self.bump();
            }
        }
    }

    fn item(&mut self) -> Option<Item> {
        let start = self.peek().span.start;
        let mut attributes = Vec::new();
        while self.eat(TokenKind::At).is_some() {
            attributes.extend(self.ident(NameOf::Attribute));
        }
        let exported = self.eat(TokenKind::Export).is_some();
        let is_async = self.eat(TokenKind::Async).is_some();

        if self.at_word("function") {
            let token = self.peek();
            self.soft_error(errors::removed_keyword(token.span, "function", "fn"));
        } else if self.at_word("var") {
            let token = self.peek();
            self.soft_error(errors::removed_keyword(token.span, "var", "let"));
            let local = self.local()?;
            self.expect_semicolon();
            return Some(Item {
                kind: ItemKind::Global(local),
                span: self.since(start),
            });
        }

        match self.peek().kind {
            TokenKind::Fn => {}
            TokenKind::Ident if self.at_word("function") => {}
            TokenKind::Struct => {
                let decl = self.struct_decl(exported)?;
                return Some(Item {
                    kind: ItemKind::Struct(decl),
                    span: self.since(start),
                });
            }
            TokenKind::Let | TokenKind::Const => {
                let local = self.local()?;
                self.expect_semicolon();
                return Some(Item {
                    kind: ItemKind::Global(local),
                    span: self.since(start),
                });
            }
            _ => {
                let found = self.peek();
                if found.kind != TokenKind::Error {
                    let d = errors::expected_item(found, self.text(found));
                    self.error(d);
                }
                self.recover_item(true);
                return None;
            }
        }
        self.bump(); // `fn`, or `function` already reported.
        let name = self.ident(NameOf::Function)?;
        let params = self.params()?;
        let ret = self.eat(TokenKind::Colon).map(|_| self.ty());
        let outer_loops = std::mem::take(&mut self.loop_depth);
        let body = self.block();
        self.loop_depth = outer_loops;
        let body = body?;
        Some(Item {
            kind: ItemKind::Fn(FnDecl {
                attributes,
                exported,
                is_async,
                name,
                params,
                ret,
                body,
            }),
            span: self.since(start),
        })
    }

    fn params(&mut self) -> Option<Vec<Param>> {
        let open = self.expect(TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at_eof() {
            match self.param() {
                Some(param) => params.push(param),
                // A broken parameter: skip to the next one, keep the function.
                None => self.recover_param(),
            }
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        if self.recovering {
            // Already reported; resume at `)` if it is there.
            return self.eat(TokenKind::RParen).map(|_| params);
        }
        self.expect_close(open, TokenKind::RParen, ")")
            .then_some(params)
    }

    /// `name: [inout|sink] Type`
    fn param(&mut self) -> Option<Param> {
        let name = self.ident(NameOf::Parameter)?;
        let optional = self.eat(TokenKind::Question);
        self.expect(TokenKind::Colon, ":")?;
        let convention = if self.eat(TokenKind::Inout).is_some() {
            Convention::Inout
        } else if self.eat(TokenKind::Sink).is_some() {
            Convention::Sink
        } else {
            Convention::Let
        };
        let ty = self.ty();
        self.check_optional(optional, &ty);
        Some(Param {
            name,
            convention,
            ty,
        })
    }

    /// `E0118`: `name?: T` is written `name: Option<T>`.
    fn check_optional(&mut self, marker: Option<Token>, ty: &Type) {
        if let Some(marker) = marker {
            let type_text = self.slice(ty.span).to_owned();
            self.soft_error(errors::optional_marker(marker.span, ty.span, &type_text));
        }
    }

    /// `struct Name { field: Type; … }`, fields separated by `;` or `,`.
    fn struct_decl(&mut self, exported: bool) -> Option<StructDecl> {
        self.bump(); // `struct`
        let name = self.ident(NameOf::Struct)?;
        let open = self.expect(TokenKind::LBrace, "{")?;
        let mut fields = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at_eof() {
            self.recovering = false;
            let Some(field_name) = self.ident(NameOf::Field) else {
                self.recover_field();
                continue;
            };
            let optional = self.eat(TokenKind::Question);
            if self.expect(TokenKind::Colon, ":").is_none() {
                self.recover_field();
                continue;
            }
            let ty = self.ty();
            self.check_optional(optional, &ty);
            fields.push(Field {
                name: field_name,
                ty,
            });
            if self.eat(TokenKind::Semicolon).is_none()
                && self.eat(TokenKind::Comma).is_none()
                && !self.at(TokenKind::RBrace)
            {
                self.expect_semicolon();
            }
        }
        if self.at_eof() {
            self.recovering = false;
        }
        self.expect_close(open, TokenKind::RBrace, "}");
        Some(StructDecl {
            exported,
            name,
            fields,
        })
    }

    /// Skips past the end of a broken field: after `;` or `,`, or before `}`.
    fn recover_field(&mut self) {
        loop {
            match self.peek().kind {
                TokenKind::Eof | TokenKind::RBrace => break,
                TokenKind::Semicolon | TokenKind::Comma => {
                    self.bump();
                    break;
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// Skips to the `,` or `)` ending a parameter, without consuming it.
    fn recover_param(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.peek().kind {
                TokenKind::Eof | TokenKind::LBrace => break,
                TokenKind::Comma | TokenKind::RParen if depth == 0 => break,
                TokenKind::LParen | TokenKind::Lt => depth += 1,
                TokenKind::RParen | TokenKind::Gt => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.bump();
        }
    }

    fn block(&mut self) -> Option<Block> {
        let open = self.expect(TokenKind::LBrace, "{")?;
        let mut stmts = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at_eof() {
            self.recovering = false;
            let before = self.pos;
            stmts.extend(self.stmt());
            if self.pos == before {
                let found = self.bump();
                if !self.recovering {
                    let label = errors::cannot_appear();
                    self.unexpected(found, label);
                }
            }
        }
        if self.at_eof() {
            self.recovering = false;
        }
        self.expect_close(open, TokenKind::RBrace, "}");
        Some(Block {
            stmts,
            span: self.since(open.span.start),
        })
    }

    // --- Statements ---------------------------------------------------------

    /// A statement; `None` for an empty statement (`;`).
    fn stmt(&mut self) -> Option<Stmt> {
        let start = self.peek().span.start;
        if !self.enter() {
            self.recover_statement();
            return Some(Stmt {
                kind: StmtKind::Error,
                span: self.since(start),
            });
        }
        let kind = match self.peek().kind {
            TokenKind::Let | TokenKind::Const => self.local_stmt(),
            TokenKind::LBrace => self.block_stmt(),
            TokenKind::If => self.if_stmt(),
            TokenKind::While => self.while_stmt(),
            TokenKind::For => self.for_stmt(),
            TokenKind::Break | TokenKind::Continue => self.jump_stmt(),
            TokenKind::Ident if self.at_word("var") && self.nth(1).kind == TokenKind::Ident => {
                let token = self.peek();
                self.soft_error(errors::removed_keyword(token.span, "var", "let"));
                self.local_stmt()
            }
            TokenKind::Return => {
                self.bump();
                let value = (!matches!(
                    self.peek().kind,
                    TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof
                ))
                .then(|| self.expr());
                self.expect_semicolon();
                StmtKind::Return(value)
            }
            TokenKind::Semicolon => {
                self.bump(); // An empty statement.
                self.leave();
                return None;
            }
            _ => self.expr_stmt(),
        };
        self.leave();
        Some(Stmt {
            kind,
            span: self.since(start),
        })
    }

    /// A `let` or `const` statement, `;` included.
    fn local_stmt(&mut self) -> StmtKind {
        if let Some(local) = self.local() {
            self.expect_semicolon();
            StmtKind::Local(local)
        } else {
            self.recover_statement();
            StmtKind::Error
        }
    }

    /// `let` or `const`, after which the caller expects `;`.
    fn local(&mut self) -> Option<Local> {
        let keyword = self.bump();
        let mutable = keyword.kind != TokenKind::Const;
        let name = self.ident(NameOf::Variable)?;
        let ty = self.eat(TokenKind::Colon).map(|_| self.ty());
        let init = self.eat(TokenKind::Eq).map(|_| self.expr());
        if !mutable && init.is_none() && !self.recovering {
            let text = self.text(keyword).to_owned();
            let name_text = self.ast.name(name.symbol).to_owned();
            self.soft_error(errors::const_without_value(name.span, &name_text, &text));
        }
        Some(Local {
            mutable,
            name,
            ty,
            init,
        })
    }

    /// An expression statement, an assignment or an increment, `;` included.
    fn expr_stmt(&mut self) -> StmtKind {
        let kind = self.simple_stmt();
        if !matches!(kind, StmtKind::Error) {
            self.expect_semicolon();
        }
        kind
    }

    /// An expression, an assignment or an increment, without its `;`.
    fn simple_stmt(&mut self) -> StmtKind {
        let target = self.expr();
        if self.recovering {
            // The expression already failed: what follows is not an assignment.
            self.recover_statement();
            return StmtKind::Error;
        }
        let token = self.peek();
        if let Some(op) = errors::assign_op(token.kind) {
            self.bump();
            self.check_target(target);
            let value = self.expr();
            return StmtKind::Assign { target, op, value };
        }
        if let Some(op) = errors::step_op(token.kind) {
            self.bump();
            self.check_target(target);
            return StmtKind::Step { target, op };
        }
        StmtKind::Expr(target)
    }

    /// Only names, fields and elements can be assigned.
    fn check_target(&mut self, target: crate::ast::ExprId) {
        use crate::ast::ExprKind;
        let expr = self.ast.expr(target);
        let valid = matches!(
            expr.kind,
            ExprKind::Name(_) | ExprKind::Field { .. } | ExprKind::Index { .. } | ExprKind::Error
        );
        if !valid {
            let span = expr.span;
            self.soft_error(errors::invalid_target(span));
        }
    }
}

#[cfg(test)]
mod tests;
