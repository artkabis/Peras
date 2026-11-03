//! Control flow: blocks, `if`, `while`, `for`, `for…of`, `break` and `continue`.
//!
//! As in TypeScript, a body is a block or a single statement, and conditions are
//! written in parentheses. Peras refuses what hides bugs: a condition without
//! parentheses is reported with the fix (`E0115`), `for…in` becomes `for…of`
//! (`E0116`), and a `;` right after `if (…)` or a loop header, which silently
//! empties the body, is an error (`E0119`).

use perasc_diag::Span;

use super::{NameOf, Parser, errors};
use crate::ast::{ExprId, Stmt, StmtKind};
use crate::token::TokenKind;

impl Parser<'_> {
    /// `{ … }` as a statement.
    pub(super) fn block_stmt(&mut self) -> StmtKind {
        self.block().map_or(StmtKind::Error, StmtKind::Block)
    }

    /// The body of an `if`, `else` or loop: a block or one statement.
    fn body(&mut self, keyword: &'static str) -> Stmt {
        let start = self.peek().span.start;
        if let Some(semicolon) = self.eat(TokenKind::Semicolon) {
            // A block right after was meant as the body (`if (c);\n{ … }`): removing the
            // `;` attaches it. Otherwise the `;` is more likely a deliberate empty body,
            // as in `while (waiting());`, which `{}` keeps while making it explicit.
            let block_follows = self.at(TokenKind::LBrace);
            let replacement = if block_follows { "" } else { " {}" };
            self.soft_error(errors::empty_body(semicolon.span, keyword, replacement));
            return Stmt {
                kind: StmtKind::Block(crate::ast::Block {
                    stmts: Vec::new(),
                    span: semicolon.span,
                }),
                span: semicolon.span,
            };
        }
        self.stmt().unwrap_or(Stmt {
            kind: StmtKind::Error,
            span: Span::new(start, start),
        })
    }

    /// `( condition )` after `if` or `while`; without parentheses, reported and parsed anyway.
    fn condition(&mut self, keyword: &'static str) -> ExprId {
        if let Some(open) = self.eat(TokenKind::LParen) {
            let cond = self.expr();
            if !self.recovering {
                self.expect_close(open, TokenKind::RParen, ")");
            }
            return cond;
        }
        let cond = self.expr();
        if !self.recovering {
            let span = self.ast.expr(cond).span;
            self.soft_error(errors::condition_without_parentheses(span, keyword));
        }
        cond
    }

    /// `if (cond) then [else otherwise]`. A dangling `else` belongs to the nearest `if`.
    pub(super) fn if_stmt(&mut self) -> StmtKind {
        self.bump();
        let cond = self.condition("if");
        let then = Box::new(self.body("if"));
        let otherwise = self
            .eat(TokenKind::Else)
            .map(|_| Box::new(self.body("else")));
        StmtKind::If {
            cond,
            then,
            otherwise,
        }
    }

    /// `while (cond) body`
    pub(super) fn while_stmt(&mut self) -> StmtKind {
        self.bump();
        let cond = self.condition("while");
        let body = Box::new(self.loop_body("while"));
        StmtKind::While { cond, body }
    }

    /// A loop body, inside which `break` and `continue` are allowed.
    fn loop_body(&mut self, keyword: &'static str) -> Stmt {
        self.loop_depth += 1;
        let body = self.body(keyword);
        self.loop_depth -= 1;
        body
    }

    /// `for (init; cond; update) body` or `for (const x of xs) body`.
    pub(super) fn for_stmt(&mut self) -> StmtKind {
        self.bump();
        let Some(open) = self.expect(TokenKind::LParen, "(") else {
            self.recover_statement();
            return StmtKind::Error;
        };

        let declares = matches!(self.peek().kind, TokenKind::Let | TokenKind::Const)
            || (self.at_word("var") && self.nth(1).kind == TokenKind::Ident);
        if declares && matches!(self.nth(2).kind, TokenKind::Ident | TokenKind::Colon) {
            let after_name = if self.nth(2).kind == TokenKind::Colon {
                None
            } else {
                Some(self.text(self.nth(2)))
            };
            if matches!(after_name, Some("of" | "in")) {
                return self.for_of(open);
            }
            if after_name.is_none() && self.colon_type_then_of() {
                return self.for_of(open);
            }
        }

        let init = if self.at(TokenKind::Semicolon) {
            None
        } else if declares {
            if self.at_word("var") {
                let token = self.peek();
                self.soft_error(errors::removed_keyword(token.span, "var", "let"));
            }
            let start = self.peek().span.start;
            let local = self.local();
            local.map(|local| {
                Box::new(Stmt {
                    kind: StmtKind::Local(local),
                    span: self.since(start),
                })
            })
        } else {
            Some(Box::new(self.simple_stmt_boxed()))
        };
        if self.recovering || self.expect(TokenKind::Semicolon, ";").is_none() {
            self.recover_statement();
            return StmtKind::Error;
        }
        let cond = (!self.at(TokenKind::Semicolon)).then(|| self.expr());
        if self.recovering || self.expect(TokenKind::Semicolon, ";").is_none() {
            self.recover_statement();
            return StmtKind::Error;
        }
        let update = (!self.at(TokenKind::RParen)).then(|| Box::new(self.simple_stmt_boxed()));
        if self.recovering || !self.expect_close(open, TokenKind::RParen, ")") {
            self.recover_statement();
            return StmtKind::Error;
        }
        let body = Box::new(self.loop_body("for"));
        StmtKind::For {
            init,
            cond,
            update,
            body,
        }
    }

    /// Whether `let x: Type of` follows: scans the annotation without consuming it.
    fn colon_type_then_of(&self) -> bool {
        let mut n = 3; // keyword, name, `:`
        let mut depth = 0usize;
        loop {
            let token = self.nth(n);
            match token.kind {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => depth = depth.saturating_sub(1),
                TokenKind::Ident if depth == 0 && n > 3 => {
                    return matches!(self.text(token), "of" | "in");
                }
                TokenKind::Ident | TokenKind::Comma => {}
                _ => return false,
            }
            n += 1;
        }
    }

    /// `for (const x[: T] of iterable) body`, with `in` reported and read as `of`.
    fn for_of(&mut self, open: crate::token::Token) -> StmtKind {
        let keyword = self.peek();
        if self.at_word("var") {
            self.soft_error(errors::removed_keyword(keyword.span, "var", "let"));
        }
        self.bump();
        let mutable = keyword.kind != TokenKind::Const;
        let Some(name) = self.ident(NameOf::Variable) else {
            self.recover_statement();
            return StmtKind::Error;
        };
        let ty = self.eat(TokenKind::Colon).map(|_| self.ty());
        let word = self.bump(); // `of` or `in`, checked by the caller.
        if self.text(word) == "in" {
            self.soft_error(errors::for_in(word.span));
        }
        let iterable = self.expr();
        if self.recovering || !self.expect_close(open, TokenKind::RParen, ")") {
            self.recover_statement();
            return StmtKind::Error;
        }
        let body = Box::new(self.loop_body("for"));
        StmtKind::ForOf {
            mutable,
            keyword: keyword.span,
            name,
            ty,
            iterable,
            body,
        }
    }

    /// `break;` or `continue;`, only inside a loop.
    pub(super) fn jump_stmt(&mut self) -> StmtKind {
        let token = self.bump();
        if self.loop_depth == 0 {
            let keyword = self.text(token).to_owned();
            self.soft_error(errors::jump_outside_loop(token.span, &keyword));
        }
        self.expect_semicolon();
        if token.kind == TokenKind::Break {
            StmtKind::Break
        } else {
            StmtKind::Continue
        }
    }

    /// An assignment, increment or expression without its `;`, as a statement.
    fn simple_stmt_boxed(&mut self) -> Stmt {
        let start = self.peek().span.start;
        let kind = self.simple_stmt();
        Stmt {
            kind,
            span: self.since(start),
        }
    }
}
