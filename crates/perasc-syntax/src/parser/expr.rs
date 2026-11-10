//! Expressions, by Pratt parsing.
//!
//! Precedence follows JavaScript, from loosest to tightest:
//! `||`, `&&`, `|`, `^`, `&`, `== !=`, `< <= > >=`, `<< >>`, `+ -`, `* / %`,
//! prefix `- ! ~ &`, then postfix calls, indexing, fields and `?`.
//! Where JavaScript's rules hide bugs, Peras refuses the expression and explains:
//! chained comparisons (`a < b < c`) and bitwise operators mixed with comparisons
//! without parentheses (`a & 1 == 0`, which JavaScript reads as `a & (1 == 0)`).

use perasc_diag::Span;

use super::{NameOf, Parser, errors};
use crate::ast::{
    Arm, ArmBody, BinaryOp, ExprId, ExprKind, Ident, Pattern, PatternKind, Type, TypeKind, UnaryOp,
};
use crate::lexer::{string_value, template_value};
use crate::token::{Token, TokenKind};

/// Binding power of prefix operators.
const PREFIX: u8 = 21;

impl Parser<'_> {
    /// Parses a full expression.
    pub(super) fn expr(&mut self) -> ExprId {
        self.expr_bp(0)
    }

    fn alloc(&mut self, kind: ExprKind, span: Span) -> ExprId {
        self.ast.alloc(kind, span)
    }

    fn error_expr(&mut self, span: Span) -> ExprId {
        self.alloc(ExprKind::Error, span)
    }

    /// The binary operator at the current position: (operator, left and right
    /// binding powers, number of tokens). `<<` and `>>` are two adjacent tokens.
    fn infix(&self) -> Option<(BinaryOp, u8, u8, usize)> {
        let token = self.peek();
        let next = self.nth(1);
        let adjacent = next.span.start == token.span.end;
        let (op, power) = match token.kind {
            TokenKind::PipePipe => (BinaryOp::Or, 1),
            TokenKind::AmpAmp => (BinaryOp::And, 3),
            TokenKind::Pipe => (BinaryOp::BitOr, 5),
            TokenKind::Caret => (BinaryOp::BitXor, 7),
            TokenKind::Amp => (BinaryOp::BitAnd, 9),
            TokenKind::EqEq => (BinaryOp::Eq, 11),
            TokenKind::BangEq => (BinaryOp::Ne, 11),
            TokenKind::Lt if adjacent && next.kind == TokenKind::Lt => {
                return Some((BinaryOp::Shl, 15, 16, 2));
            }
            TokenKind::Gt if adjacent && next.kind == TokenKind::Gt => {
                return Some((BinaryOp::Shr, 15, 16, 2));
            }
            TokenKind::Lt => (BinaryOp::Lt, 13),
            TokenKind::LtEq => (BinaryOp::Le, 13),
            TokenKind::Gt => (BinaryOp::Gt, 13),
            TokenKind::GtEq => (BinaryOp::Ge, 13),
            TokenKind::Plus => (BinaryOp::Add, 17),
            TokenKind::Minus => (BinaryOp::Sub, 17),
            TokenKind::Star => (BinaryOp::Mul, 19),
            TokenKind::Slash => (BinaryOp::Div, 19),
            TokenKind::Percent => (BinaryOp::Rem, 19),
            _ => return None,
        };
        Some((op, power, power + 1, 1))
    }

    /// Parses an expression whose operators bind at least as tightly as `min_bp`.
    fn expr_bp(&mut self, min_bp: u8) -> ExprId {
        if !self.enter() {
            let span = self.peek().span;
            return self.error_expr(span);
        }
        let mut lhs = self.prefix();
        loop {
            lhs = self.postfix(lhs);
            if self.recovering {
                break;
            }
            let Some((op, left, right, width)) = self.infix() else {
                break;
            };
            if left < min_bp {
                break;
            }
            let op_start = self.peek().span.start;
            for _ in 0..width {
                self.bump();
            }
            let op_span = self.since(op_start);
            let rhs = self.expr_bp(right);
            let span = self.ast.expr(lhs).span.to(self.ast.expr(rhs).span);
            let binary = self.alloc(ExprKind::Binary { op, lhs, rhs }, span);
            self.check_comparison_chain(op, op_span, lhs, rhs);
            self.check_bitwise_mix(op, op_span, lhs, rhs);
            lhs = binary;
        }
        self.leave();
        lhs
    }

    /// `` `text${a}text${b}text` ``: the pieces of text and the values between.
    fn template(&mut self, head: Token) -> ExprId {
        self.bump();
        let mut parts = vec![template_value(self.text(head))];
        let mut exprs = Vec::new();
        loop {
            exprs.push(self.expr());
            if self.recovering {
                break;
            }
            let next = self.peek();
            match next.kind {
                TokenKind::TemplateMiddle => {
                    self.bump();
                    parts.push(template_value(self.text(next)));
                }
                TokenKind::TemplateTail => {
                    self.bump();
                    parts.push(template_value(self.text(next)));
                    break;
                }
                _ => {
                    self.expect(TokenKind::TemplateTail, "}");
                    break;
                }
            }
        }
        // One more piece of text than values, even after an error.
        parts.resize(exprs.len() + 1, String::new());
        let span = self.since(head.span.start);
        self.alloc(ExprKind::Template { parts, exprs }, span)
    }

    fn prefix(&mut self) -> ExprId {
        let token = self.peek();
        let unary = match token.kind {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            TokenKind::Amp => Some(UnaryOp::Borrow),
            TokenKind::Await => Some(UnaryOp::Await),
            _ => None,
        };
        if let Some(op) = unary {
            self.bump();
            let operand = self.expr_bp(PREFIX);
            let span = token.span.to(self.ast.expr(operand).span);
            return self.alloc(ExprKind::Unary { op, operand }, span);
        }
        match token.kind {
            TokenKind::Int | TokenKind::Float => {
                self.bump();
                self.number(token)
            }
            TokenKind::String => {
                self.bump();
                let value = string_value(self.text(token));
                self.alloc(ExprKind::String(value), token.span)
            }
            TokenKind::TemplateHead => self.template(token),
            TokenKind::True | TokenKind::False => {
                self.bump();
                self.alloc(ExprKind::Bool(token.kind == TokenKind::True), token.span)
            }
            // `x => body`
            TokenKind::Ident if self.nth(1).kind == TokenKind::FatArrow => self.lambda(token),
            TokenKind::Ident => {
                self.bump();
                let symbol = self.ast.interner.intern(self.text(token));
                self.alloc(ExprKind::Name(symbol), token.span)
            }
            TokenKind::LParen if self.lambda_ahead() => self.lambda(token),
            TokenKind::LParen => {
                self.bump();
                let inner = self.expr();
                // `(a, b)`: a tuple.
                if !self.recovering && self.at(TokenKind::Comma) {
                    let mut items = vec![inner];
                    while self.eat(TokenKind::Comma).is_some() && !self.at(TokenKind::RParen) {
                        items.push(self.expr());
                        if self.recovering {
                            break;
                        }
                    }
                    self.expect_close(token, TokenKind::RParen, ")");
                    let span = self.since(token.span.start);
                    return self.alloc(ExprKind::Tuple(items), span);
                }
                self.expect_close(token, TokenKind::RParen, ")");
                let span = self.since(token.span.start);
                self.alloc(ExprKind::Paren(inner), span)
            }
            TokenKind::Match => self.match_expr(),
            TokenKind::LBracket => {
                self.bump();
                let mut items = Vec::new();
                while !self.at(TokenKind::RBracket) && !self.at_eof() {
                    items.push(self.expr());
                    if self.recovering || self.eat(TokenKind::Comma).is_none() {
                        break;
                    }
                }
                if !self.recovering {
                    self.expect_close(token, TokenKind::RBracket, "]");
                }
                let span = self.since(token.span.start);
                self.alloc(ExprKind::Array(items), span)
            }
            TokenKind::Error => {
                // Already reported by the lexer: an error node, without a second error.
                self.bump();
                self.error_expr(token.span)
            }
            _ => {
                if !self.recovering {
                    let d = errors::expected_expression(token, self.text(token));
                    self.error(d);
                }
                self.recovering = true;
                self.error_expr(Span::new(token.span.start, token.span.start))
            }
        }
    }

    /// `match (value) { pattern => body, … }`: each body is a value, or a
    /// block of statements; `,` after a block is optional.
    fn match_expr(&mut self) -> ExprId {
        let keyword = self.bump();
        let scrutinee = match self.expect(TokenKind::LParen, "(") {
            Some(paren) => {
                let value = self.expr();
                self.expect_close(paren, TokenKind::RParen, ")");
                value
            }
            None => self.error_expr(keyword.span),
        };
        let mut arms = Vec::new();
        if let Some(open) = self.expect(TokenKind::LBrace, "{") {
            while !self.at(TokenKind::RBrace) && !self.at_eof() && !self.recovering {
                let start = self.peek().span.start;
                let Some(pattern) = self.pattern() else {
                    break;
                };
                if self.expect(TokenKind::FatArrow, "=>").is_none() {
                    break;
                }
                let body = if self.at(TokenKind::LBrace) {
                    match self.block() {
                        Some(block) => ArmBody::Block(block),
                        None => break,
                    }
                } else {
                    ArmBody::Expr(self.expr())
                };
                let block_body = matches!(body, ArmBody::Block(_));
                arms.push(Arm {
                    pattern,
                    body,
                    span: self.since(start),
                });
                if self.eat(TokenKind::Comma).is_none()
                    && !block_body
                    && !self.at(TokenKind::RBrace)
                {
                    self.expect(TokenKind::Comma, ",");
                    break;
                }
            }
            if !self.recovering {
                self.expect_close(open, TokenKind::RBrace, "}");
            }
        }
        let span = self.since(keyword.span.start);
        self.alloc(ExprKind::Match { scrutinee, arms }, span)
    }

    /// A pattern: `_`, a name, a literal, or `Variant(p, …)`.
    fn pattern(&mut self) -> Option<Pattern> {
        let token = self.peek();
        let start = token.span.start;
        let kind = match token.kind {
            TokenKind::Ident if self.text(token) == "_" => {
                self.bump();
                PatternKind::Wildcard
            }
            TokenKind::Ident => {
                self.bump();
                let first = Ident {
                    symbol: self.ast.interner.intern(self.text(token)),
                    span: token.span,
                };
                let (qualifier, name) = if self.eat(TokenKind::Dot).is_some() {
                    let next = self.peek();
                    if next.kind != TokenKind::Ident {
                        let d = errors::expected_expression(next, self.text(next));
                        self.error(d);
                        self.recovering = true;
                        return None;
                    }
                    self.bump();
                    let name = Ident {
                        symbol: self.ast.interner.intern(self.text(next)),
                        span: next.span,
                    };
                    (Some(first), name)
                } else {
                    (None, first)
                };
                if let Some(paren) = self.eat(TokenKind::LParen) {
                    let mut fields = Vec::new();
                    while !self.at(TokenKind::RParen) && !self.at_eof() {
                        fields.push(self.pattern()?);
                        if self.eat(TokenKind::Comma).is_none() {
                            break;
                        }
                    }
                    self.expect_close(paren, TokenKind::RParen, ")");
                    PatternKind::Variant {
                        qualifier,
                        name,
                        fields,
                    }
                } else if qualifier.is_some() {
                    PatternKind::Variant {
                        qualifier,
                        name,
                        fields: Vec::new(),
                    }
                } else {
                    PatternKind::Name(name)
                }
            }
            TokenKind::Int
            | TokenKind::Float
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Minus => PatternKind::Literal(self.expr_bp(PREFIX)),
            _ => {
                if !self.recovering {
                    let d = errors::expected_expression(token, self.text(token));
                    self.error(d);
                }
                self.recovering = true;
                return None;
            }
        };
        Some(Pattern {
            kind,
            span: self.since(start),
        })
    }

    /// An arrow function, from its first token: `x => body` or
    /// `(a, b: T) => body`.
    fn lambda(&mut self, token: Token) -> ExprId {
        let params = if token.kind == TokenKind::Ident {
            self.bump();
            self.bump(); // `=>`
            let name = Ident {
                symbol: self.ast.interner.intern(self.text(token)),
                span: token.span,
            };
            vec![(name, None)]
        } else {
            self.lambda_params()
        };
        let body = self.expr();
        let span = self.since(token.span.start);
        self.alloc(ExprKind::Lambda { params, body }, span)
    }

    /// Whether `(` starts an arrow function: `()`, or names with optional
    /// types, then `)` and `=>`. Read ahead, nothing consumed.
    fn lambda_ahead(&mut self) -> bool {
        let pos = self.pos;
        let reported = self.diagnostics.len();
        let recovering = self.recovering;
        let found = self.lambda_params_ahead();
        self.pos = pos;
        self.diagnostics.truncate(reported);
        self.recovering = recovering;
        found
    }

    fn lambda_params_ahead(&mut self) -> bool {
        self.bump(); // `(`
        while !self.at(TokenKind::RParen) {
            if self.eat(TokenKind::Ident).is_none() {
                return false;
            }
            if self.eat(TokenKind::Colon).is_some() {
                let ty = self.ty();
                if matches!(ty.kind, TypeKind::Error) {
                    return false;
                }
            }
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.eat(TokenKind::RParen).is_some() && self.at(TokenKind::FatArrow)
    }

    /// `(a, b: T) =>`, known to be there.
    fn lambda_params(&mut self) -> Vec<(Ident, Option<Type>)> {
        self.bump(); // `(`
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) {
            let token = self.bump();
            let name = Ident {
                symbol: self.ast.interner.intern(self.text(token)),
                span: token.span,
            };
            let ty = self.eat(TokenKind::Colon).map(|_| self.ty());
            params.push((name, ty));
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        self.bump(); // `)`
        self.bump(); // `=>`
        params
    }

    /// `<Type, …>` followed by `(`, read ahead: when what follows `<` is
    /// not that, nothing is consumed and `<` is a comparison.
    fn type_arguments(&mut self) -> Option<Vec<Type>> {
        if self.nth(1).kind != TokenKind::Ident {
            return None;
        }
        let pos = self.pos;
        let reported = self.diagnostics.len();
        let recovering = self.recovering;
        self.bump(); // `<`
        let mut types = Vec::new();
        let ok = loop {
            let ty = self.ty();
            if matches!(ty.kind, TypeKind::Error) || self.diagnostics.len() > reported {
                break false;
            }
            types.push(ty);
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            break self.eat(TokenKind::Gt).is_some() && self.at(TokenKind::LParen);
        };
        if ok {
            return Some(types);
        }
        self.pos = pos;
        self.diagnostics.truncate(reported);
        self.recovering = recovering;
        None
    }

    /// Calls, indexing, fields and `?`, which bind tighter than any operator.
    fn postfix(&mut self, mut expr: ExprId) -> ExprId {
        loop {
            if self.recovering {
                // After an error, the following tokens are skipped, not interpreted.
                return expr;
            }
            let token = self.peek();
            let start = self.ast.expr(expr).span.start;
            expr = match token.kind {
                TokenKind::LParen => {
                    self.bump();
                    let args = self.arguments(token);
                    let span = self.since(start);
                    self.alloc(
                        ExprKind::Call {
                            callee: expr,
                            types: Vec::new(),
                            args,
                        },
                        span,
                    )
                }
                // `larger<i64>(a, b)`: a comparison cannot be followed by
                // another (`E0108`), so `name<…>(` is always a call.
                TokenKind::Lt if matches!(self.ast.expr(expr).kind, ExprKind::Name(_)) => {
                    let Some(types) = self.type_arguments() else {
                        return expr;
                    };
                    let open = self.bump(); // `(`
                    let args = self.arguments(open);
                    let span = self.since(start);
                    self.alloc(
                        ExprKind::Call {
                            callee: expr,
                            types,
                            args,
                        },
                        span,
                    )
                }
                TokenKind::LBracket => {
                    self.bump();
                    let index = self.expr();
                    self.expect_close(token, TokenKind::RBracket, "]");
                    let span = self.since(start);
                    self.alloc(ExprKind::Index { base: expr, index }, span)
                }
                TokenKind::Dot if self.nth(1).kind == TokenKind::Int => {
                    // `pair.0`: a field of a tuple.
                    self.bump();
                    let index = self.bump();
                    let name = Ident {
                        symbol: self.ast.interner.intern(self.text(index)),
                        span: index.span,
                    };
                    let span = self.since(start);
                    self.alloc(ExprKind::Field { base: expr, name }, span)
                }
                TokenKind::Dot if self.nth(1).kind == TokenKind::Float => {
                    // `nested.0.1` is read as `nested`, `.`, `0.1`: two fields.
                    self.bump();
                    let number = self.bump();
                    let text = self.text(number);
                    let Some((first, second)) = text.split_once('.') else {
                        return expr;
                    };
                    let (first, second) = (first.to_owned(), second.to_owned());
                    let split = number.span.start + u32::try_from(first.len()).unwrap_or(0);
                    let name = Ident {
                        symbol: self.ast.interner.intern(&first),
                        span: Span::new(number.span.start, split),
                    };
                    let outer = Span::new(start, split);
                    let inner = self.alloc(ExprKind::Field { base: expr, name }, outer);
                    let name = Ident {
                        symbol: self.ast.interner.intern(&second),
                        span: Span::new(split + 1, number.span.end),
                    };
                    let span = self.since(start);
                    self.alloc(ExprKind::Field { base: inner, name }, span)
                }
                TokenKind::Dot => {
                    self.bump();
                    let Some(name) = self.ident(NameOf::Field) else {
                        return expr;
                    };
                    let span = self.since(start);
                    self.alloc(ExprKind::Field { base: expr, name }, span)
                }
                TokenKind::Question => {
                    self.bump();
                    let span = self.since(start);
                    self.alloc(ExprKind::Try(expr), span)
                }
                _ => return expr,
            };
        }
    }

    /// Call arguments after `(`, up to and including `)`. A trailing comma is allowed.
    fn arguments(&mut self, open: Token) -> Vec<ExprId> {
        let mut args = Vec::new();
        while !self.at(TokenKind::RParen) && !self.at_eof() {
            args.push(self.expr());
            if self.recovering || self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        if !self.recovering {
            self.expect_close(open, TokenKind::RParen, ")");
        }
        args
    }

    /// An integer or float literal; `E0107` when it does not fit in 64 bits.
    fn number(&mut self, token: Token) -> ExprId {
        let text = self.text(token);
        let digits: String = text.chars().filter(|&c| c != '_').collect();
        if token.kind == TokenKind::Float {
            let value = digits.parse::<f64>().unwrap_or(f64::INFINITY);
            if value.is_finite() {
                return self.alloc(ExprKind::Float(value), token.span);
            }
            self.soft_error(errors::number_too_large(token.span, text, true));
            return self.error_expr(token.span);
        }
        let (radix, body) = match digits.get(..2) {
            Some("0x" | "0X") => (16, digits.get(2..).unwrap_or("")),
            Some("0o" | "0O") => (8, digits.get(2..).unwrap_or("")),
            Some("0b" | "0B") => (2, digits.get(2..).unwrap_or("")),
            _ => (10, digits.as_str()),
        };
        if let Ok(value) = u64::from_str_radix(body, radix) {
            return self.alloc(ExprKind::Int(value), token.span);
        }
        self.soft_error(errors::number_too_large(token.span, text, false));
        self.error_expr(token.span)
    }

    /// `E0108`: `a < b < c` compares a boolean with `c` in JavaScript.
    fn check_comparison_chain(&mut self, op: BinaryOp, op_span: Span, lhs: ExprId, rhs: ExprId) {
        if !op.is_comparison() {
            return;
        }
        let ExprKind::Binary {
            op: first,
            rhs: middle,
            ..
        } = self.ast.expr(lhs).kind
        else {
            return;
        };
        if !first.is_comparison() {
            return;
        }
        let first_span = self.operator_span(lhs, first);
        let same_kind = first.is_ordering() == op.is_ordering();
        let middle_text = self.slice(self.ast.expr(middle).span).to_owned();
        let left_text = self.slice(self.ast.expr(lhs).span).to_owned();
        let right_text = self.slice(self.ast.expr(rhs).span).to_owned();
        let insert_at = self.ast.expr(lhs).span.end;
        let suggestion = same_kind.then(|| {
            (
                format!("{left_text} && {middle_text} {} {right_text}", op.symbol()),
                format!(" && {middle_text}"),
            )
        });
        let d = errors::chained_comparison(first_span, op_span, insert_at, suggestion);
        self.soft_error(d);
    }

    /// `E0109`: `a & b == c` needs parentheses, since JavaScript reads `a & (b == c)`.
    fn check_bitwise_mix(&mut self, op: BinaryOp, op_span: Span, lhs: ExprId, rhs: ExprId) {
        if !op.is_bitwise() {
            return;
        }
        // The comparison operand, and where the intended parentheses go.
        let (comparison, open_at, close_at) = match (
            self.ast.expr(lhs).kind.clone(),
            self.ast.expr(rhs).kind.clone(),
        ) {
            // `(x cmp y) & r`, meant as `x cmp (y & r)`.
            (ExprKind::Binary { op: c, rhs: y, .. }, _) if c.is_comparison() => (
                lhs,
                self.ast.expr(y).span.start,
                self.ast.expr(rhs).span.end,
            ),
            // `l & (x cmp y)`, meant as `(l & x) cmp y`.
            (_, ExprKind::Binary { op: c, lhs: x, .. }) if c.is_comparison() => (
                rhs,
                self.ast.expr(lhs).span.start,
                self.ast.expr(x).span.end,
            ),
            _ => return,
        };
        let ExprKind::Binary { op: c, .. } = self.ast.expr(comparison).kind else {
            return;
        };
        let comparison_span = self.operator_span(comparison, c);
        let whole = self.ast.expr(lhs).span.to(self.ast.expr(rhs).span);
        let mut suggestion = self.slice(whole).to_owned();
        let base = usize::try_from(whole.start).unwrap_or(0);
        let close = usize::try_from(close_at).unwrap_or(0).saturating_sub(base);
        let open = usize::try_from(open_at).unwrap_or(0).saturating_sub(base);
        if close <= suggestion.len() && open <= close {
            suggestion.insert(close, ')');
            suggestion.insert(open, '(');
        }
        let d = errors::bitwise_mix(
            op_span,
            op.symbol(),
            comparison_span,
            c.symbol(),
            (open_at, close_at),
            &suggestion,
        );
        self.soft_error(d);
    }

    /// Span of the operator of a binary expression, found between its operands.
    fn operator_span(&self, binary: ExprId, op: BinaryOp) -> Span {
        let ExprKind::Binary { lhs, rhs, .. } = self.ast.expr(binary).kind else {
            return self.ast.expr(binary).span;
        };
        let from = self.ast.expr(lhs).span.end;
        let to = self.ast.expr(rhs).span.start;
        let between = self.slice(Span::new(from, to));
        let offset = between.find(op.symbol()).unwrap_or(0);
        let start = from + u32::try_from(offset).unwrap_or(0);
        let len = u32::try_from(op.symbol().len()).unwrap_or(1);
        Span::new(start, start + len)
    }
}
