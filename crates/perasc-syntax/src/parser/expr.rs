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
use crate::ast::{BinaryOp, ExprId, ExprKind, UnaryOp};
use crate::lexer::string_value;
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

    fn prefix(&mut self) -> ExprId {
        let token = self.peek();
        let unary = match token.kind {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            TokenKind::Amp => Some(UnaryOp::Borrow),
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
            TokenKind::True | TokenKind::False => {
                self.bump();
                self.alloc(ExprKind::Bool(token.kind == TokenKind::True), token.span)
            }
            TokenKind::Ident => {
                self.bump();
                let symbol = self.ast.interner.intern(self.text(token));
                self.alloc(ExprKind::Name(symbol), token.span)
            }
            TokenKind::LParen => {
                self.bump();
                let inner = self.expr();
                self.expect_close(token, TokenKind::RParen, ")");
                let span = self.since(token.span.start);
                self.alloc(ExprKind::Paren(inner), span)
            }
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
                    self.alloc(ExprKind::Call { callee: expr, args }, span)
                }
                TokenKind::LBracket => {
                    self.bump();
                    let index = self.expr();
                    self.expect_close(token, TokenKind::RBracket, "]");
                    let span = self.since(start);
                    self.alloc(ExprKind::Index { base: expr, index }, span)
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
