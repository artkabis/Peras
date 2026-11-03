//! Syntax error diagnostics (`E0100`–`E0199`) and small token helpers.

use perasc_diag::{Diagnostic, Edit, Fix, Label, Message, Span, codes};

use crate::ast::{AssignOp, StepOp};
use crate::token::{Token, TokenKind};

/// Longest token text quoted in a message.
const MAX_QUOTED: usize = 24;

/// Adds the `found` and `eof` arguments describing a token, for the `-found` term.
fn found(message: Message, token: Token, text: &str) -> Message {
    let eof = i64::from(token.kind == TokenKind::Eof);
    let quoted = if text.chars().count() > MAX_QUOTED {
        let cut: String = text.chars().take(MAX_QUOTED).collect();
        format!("{cut}…")
    } else {
        text.to_owned()
    };
    message.text("found", quoted).number("eof", eof)
}

fn primary(span: Span, message: Message) -> Label {
    Label {
        span,
        message,
        primary: true,
    }
}

fn secondary(span: Span, message: Message) -> Label {
    Label {
        span,
        message,
        primary: false,
    }
}

fn fix(message: Message, edits: Vec<Edit>) -> Fix {
    Fix { message, edits }
}

fn insert(at: u32, text: impl Into<String>) -> Edit {
    Edit {
        span: Span::new(at, at),
        replacement: text.into(),
    }
}

/// The span to underline for a token: the end of the file is a zero-width point.
fn token_span(token: Token) -> Span {
    if token.kind == TokenKind::Eof {
        Span::new(token.span.start, token.span.start)
    } else {
        token.span
    }
}

pub(super) fn is_assign_op(kind: TokenKind) -> bool {
    assign_op(kind).is_some()
}

pub(super) fn assign_op(kind: TokenKind) -> Option<AssignOp> {
    Some(match kind {
        TokenKind::Eq => AssignOp::Assign,
        TokenKind::PlusEq => AssignOp::Add,
        TokenKind::MinusEq => AssignOp::Sub,
        TokenKind::StarEq => AssignOp::Mul,
        TokenKind::SlashEq => AssignOp::Div,
        TokenKind::PercentEq => AssignOp::Rem,
        _ => return None,
    })
}

pub(super) fn step_op(kind: TokenKind) -> Option<StepOp> {
    match kind {
        TokenKind::PlusPlus => Some(StepOp::Increment),
        TokenKind::MinusMinus => Some(StepOp::Decrement),
        _ => None,
    }
}

/// Label: "expected `x`".
pub(super) fn expected_token(symbol: &str) -> Message {
    Message::new("parse-unexpected-expected-label").text("expected", symbol)
}

/// Label: "expected `,` or `)`".
pub(super) fn expected_separator(close: &str) -> Message {
    Message::new("parse-unexpected-separator-label").text("close", close)
}

/// Label: "this cannot appear here".
pub(super) fn cannot_appear() -> Message {
    Message::new("parse-unexpected-label")
}

/// `E0100`
pub(super) fn expected_expression(token: Token, text: &str) -> Diagnostic {
    Diagnostic {
        code: codes::EXPECTED_EXPRESSION,
        message: found(Message::new("parse-expected-expression"), token, text),
        labels: vec![primary(
            token_span(token),
            Message::new("parse-expected-expression-label"),
        )],
        cause: Message::new("parse-expected-expression-why"),
        fix: fix(Message::new("parse-expected-expression-fix"), Vec::new()),
    }
}

/// `E0101`
pub(super) fn missing_semicolon(at: u32) -> Diagnostic {
    Diagnostic {
        code: codes::MISSING_SEMICOLON,
        message: Message::new("parse-missing-semicolon"),
        labels: vec![primary(
            Span::new(at, at),
            Message::new("parse-missing-semicolon-label"),
        )],
        cause: Message::new("parse-missing-semicolon-why"),
        fix: fix(
            Message::new("parse-missing-semicolon-fix"),
            vec![insert(at, ";")],
        ),
    }
}

/// `E0102`
pub(super) fn expected_name(what: &str, token: Token, text: &str) -> Diagnostic {
    let keyword = token.kind.is_keyword();
    let fix_message = if keyword {
        Message::new("parse-expected-name-fix-keyword").text("keyword", text)
    } else {
        Message::new("parse-expected-name-fix")
    };
    Diagnostic {
        code: codes::EXPECTED_NAME,
        message: found(
            Message::new("parse-expected-name").text("what", what),
            token,
            text,
        ),
        labels: vec![primary(
            token_span(token),
            Message::new("parse-expected-name-label"),
        )],
        cause: Message::new("parse-expected-name-why"),
        fix: fix(fix_message, Vec::new()),
    }
}

/// `E0103`
/// `open`: the opening token and its text. `close`: the closing text, where it is
/// missing, and the text the fix inserts there.
pub(super) fn unclosed(
    (open, open_text): (Token, &str),
    (close, at, insertion): (&str, u32, String),
    found_token: Token,
    found_text: &str,
) -> Diagnostic {
    Diagnostic {
        code: codes::UNCLOSED_DELIMITER,
        message: Message::new("parse-unclosed").text("open", open_text),
        labels: vec![
            secondary(
                open.span,
                Message::new("parse-unclosed-open-label").text("open", open_text),
            ),
            primary(
                Span::new(at, at),
                found(
                    Message::new("parse-unclosed-label").text("close", close),
                    found_token,
                    found_text,
                ),
            ),
        ],
        cause: Message::new("parse-unclosed-why"),
        fix: fix(
            Message::new("parse-unclosed-fix").text("close", close),
            vec![insert(at, insertion)],
        ),
    }
}

/// `E0104`
pub(super) fn unexpected(token: Token, text: &str, label: Message) -> Diagnostic {
    Diagnostic {
        code: codes::UNEXPECTED_TOKEN,
        message: found(Message::new("parse-unexpected"), token, text),
        labels: vec![primary(token_span(token), label)],
        cause: Message::new("parse-unexpected-why"),
        fix: fix(Message::new("parse-unexpected-fix"), Vec::new()),
    }
}

/// `E0105`
pub(super) fn expected_type(token: Token, text: &str) -> Diagnostic {
    Diagnostic {
        code: codes::EXPECTED_TYPE,
        message: found(Message::new("parse-expected-type"), token, text),
        labels: vec![primary(
            token_span(token),
            Message::new("parse-expected-type-label"),
        )],
        cause: Message::new("parse-expected-type-why"),
        fix: fix(Message::new("parse-expected-type-fix"), Vec::new()),
    }
}

/// `E0106`
pub(super) fn removed_keyword(span: Span, keyword: &str, replacement: &str) -> Diagnostic {
    Diagnostic {
        code: codes::REMOVED_KEYWORD,
        message: Message::new("parse-removed-keyword").text("keyword", keyword),
        labels: vec![primary(
            span,
            Message::new("parse-removed-keyword-label").text("replacement", replacement),
        )],
        cause: Message::new("parse-removed-keyword-why").text("keyword", keyword),
        fix: fix(
            Message::new("parse-removed-keyword-fix")
                .text("keyword", keyword)
                .text("replacement", replacement),
            vec![Edit {
                span,
                replacement: replacement.to_owned(),
            }],
        ),
    }
}

/// `E0107`
pub(super) fn number_too_large(span: Span, literal: &str, float: bool) -> Diagnostic {
    let label = if float {
        "parse-number-too-large-float-label"
    } else {
        "parse-number-too-large-int-label"
    };
    Diagnostic {
        code: codes::NUMBER_TOO_LARGE,
        message: Message::new("parse-number-too-large").text("literal", literal),
        labels: vec![primary(span, Message::new(label))],
        cause: Message::new("parse-number-too-large-why"),
        fix: fix(Message::new("parse-number-too-large-fix"), Vec::new()),
    }
}

/// `E0108`. `suggestion` is the rewritten expression and the text to insert.
pub(super) fn chained_comparison(
    first: Span,
    second: Span,
    insert_at: u32,
    suggestion: Option<(String, String)>,
) -> Diagnostic {
    let fix = match suggestion {
        Some((whole, inserted)) => Fix {
            message: Message::new("parse-chained-comparison-fix").text("suggestion", whole),
            edits: vec![insert(insert_at, inserted)],
        },
        None => Fix {
            message: Message::new("parse-chained-comparison-fix-mixed"),
            edits: Vec::new(),
        },
    };
    Diagnostic {
        code: codes::CHAINED_COMPARISON,
        message: Message::new("parse-chained-comparison"),
        labels: vec![
            secondary(first, Message::new("parse-chained-comparison-first-label")),
            primary(second, Message::new("parse-chained-comparison-label")),
        ],
        cause: Message::new("parse-chained-comparison-why"),
        fix,
    }
}

/// `E0109`
pub(super) fn bitwise_mix(
    bitwise: Span,
    bitwise_text: &str,
    comparison: Span,
    comparison_text: &str,
    parens: (u32, u32),
    suggestion: &str,
) -> Diagnostic {
    Diagnostic {
        code: codes::AMBIGUOUS_PRECEDENCE,
        message: Message::new("parse-bitwise-mix")
            .text("bitwise", bitwise_text)
            .text("comparison", comparison_text),
        labels: vec![
            primary(bitwise, Message::new("parse-bitwise-mix-label")),
            secondary(
                comparison,
                Message::new("parse-bitwise-mix-comparison-label"),
            ),
        ],
        cause: Message::new("parse-bitwise-mix-why"),
        fix: fix(
            Message::new("parse-bitwise-mix-fix").text("suggestion", suggestion),
            vec![insert(parens.0, "("), insert(parens.1, ")")],
        ),
    }
}

/// `E0110`
pub(super) fn expected_item(token: Token, text: &str) -> Diagnostic {
    Diagnostic {
        code: codes::EXPECTED_ITEM,
        message: found(Message::new("parse-expected-item"), token, text),
        labels: vec![primary(
            token_span(token),
            Message::new("parse-expected-item-label"),
        )],
        cause: Message::new("parse-expected-item-why"),
        fix: fix(Message::new("parse-expected-item-fix"), Vec::new()),
    }
}

/// `E0111`
pub(super) fn too_deep(span: Span) -> Diagnostic {
    let limit = i64::try_from(super::MAX_DEPTH).unwrap_or(i64::MAX);
    Diagnostic {
        code: codes::NESTING_TOO_DEEP,
        message: Message::new("parse-too-deep"),
        labels: vec![primary(
            span,
            Message::new("parse-too-deep-label").number("limit", limit),
        )],
        cause: Message::new("parse-too-deep-why").number("limit", limit),
        fix: fix(Message::new("parse-too-deep-fix"), Vec::new()),
    }
}

/// `E0112`
pub(super) fn const_without_value(name: Span, name_text: &str, keyword: &str) -> Diagnostic {
    Diagnostic {
        code: codes::CONST_WITHOUT_VALUE,
        message: Message::new("parse-const-without-value").text("keyword", keyword),
        labels: vec![primary(
            name,
            Message::new("parse-const-without-value-label").text("name", name_text),
        )],
        cause: Message::new("parse-const-without-value-why"),
        fix: fix(Message::new("parse-const-without-value-fix"), Vec::new()),
    }
}

/// `E0113`
pub(super) fn assignment_in_expression(span: Span, operator: &str) -> Diagnostic {
    let fix = if operator == "=" {
        Fix {
            message: Message::new("parse-assignment-in-expression-fix-compare"),
            edits: vec![Edit {
                span,
                replacement: "==".to_owned(),
            }],
        }
    } else {
        Fix {
            message: Message::new("parse-assignment-in-expression-fix"),
            edits: Vec::new(),
        }
    };
    Diagnostic {
        code: codes::ASSIGNMENT_IN_EXPRESSION,
        message: Message::new("parse-assignment-in-expression"),
        labels: vec![primary(
            span,
            Message::new("parse-assignment-in-expression-label").text("operator", operator),
        )],
        cause: Message::new("parse-assignment-in-expression-why"),
        fix,
    }
}

/// `E0114`
pub(super) fn invalid_target(span: Span) -> Diagnostic {
    Diagnostic {
        code: codes::INVALID_ASSIGNMENT_TARGET,
        message: Message::new("parse-invalid-target"),
        labels: vec![primary(span, Message::new("parse-invalid-target-label"))],
        cause: Message::new("parse-invalid-target-why"),
        fix: fix(Message::new("parse-invalid-target-fix"), Vec::new()),
    }
}

/// `E0115`
pub(super) fn condition_without_parentheses(span: Span, keyword: &str) -> Diagnostic {
    Diagnostic {
        code: codes::CONDITION_WITHOUT_PARENTHESES,
        message: Message::new("parse-condition-parentheses").text("keyword", keyword),
        labels: vec![primary(
            span,
            Message::new("parse-condition-parentheses-label"),
        )],
        cause: Message::new("parse-condition-parentheses-why"),
        fix: fix(
            Message::new("parse-condition-parentheses-fix"),
            vec![insert(span.start, "("), insert(span.end, ")")],
        ),
    }
}

/// `E0116`
pub(super) fn for_in(span: Span) -> Diagnostic {
    Diagnostic {
        code: codes::FOR_IN,
        message: Message::new("parse-for-in"),
        labels: vec![primary(span, Message::new("parse-for-in-label"))],
        cause: Message::new("parse-for-in-why"),
        fix: fix(
            Message::new("parse-for-in-fix"),
            vec![Edit {
                span,
                replacement: "of".to_owned(),
            }],
        ),
    }
}

/// `E0117`
pub(super) fn jump_outside_loop(span: Span, keyword: &str) -> Diagnostic {
    Diagnostic {
        code: codes::JUMP_OUTSIDE_LOOP,
        message: Message::new("parse-jump-outside-loop").text("keyword", keyword),
        labels: vec![primary(
            span,
            Message::new("parse-jump-outside-loop-label").text("keyword", keyword),
        )],
        cause: Message::new("parse-jump-outside-loop-why").text("keyword", keyword),
        fix: fix(
            Message::new("parse-jump-outside-loop-fix").text("keyword", keyword),
            Vec::new(),
        ),
    }
}

/// `E0118`: `name?: T` becomes `name: Option<T>`.
pub(super) fn optional_marker(marker: Span, ty: Span, type_text: &str) -> Diagnostic {
    Diagnostic {
        code: codes::OPTIONAL_MARKER,
        message: Message::new("parse-optional-marker"),
        labels: vec![primary(marker, Message::new("parse-optional-marker-label"))],
        cause: Message::new("parse-optional-marker-why"),
        fix: fix(
            Message::new("parse-optional-marker-fix").text("type", type_text),
            vec![
                Edit {
                    span: marker,
                    replacement: String::new(),
                },
                insert(ty.start, "Option<"),
                insert(ty.end, ">"),
            ],
        ),
    }
}

/// `E0119`
pub(super) fn empty_body(semicolon: Span, keyword: &str, replacement: &str) -> Diagnostic {
    Diagnostic {
        code: codes::EMPTY_BODY,
        message: Message::new("parse-empty-body").text("keyword", keyword),
        labels: vec![primary(
            semicolon,
            Message::new("parse-empty-body-label").text("keyword", keyword),
        )],
        cause: Message::new("parse-empty-body-why"),
        fix: fix(
            Message::new("parse-empty-body-fix"),
            vec![Edit {
                span: semicolon,
                replacement: replacement.to_owned(),
            }],
        ),
    }
}
