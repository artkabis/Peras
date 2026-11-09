//! Name and type error diagnostics (`E0200`–`E0299`).

use perasc_diag::{Diagnostic, Edit, ErrorCode, Fix, Label, Message, Span, codes};

use crate::ty::Ty;

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

fn insert(at: u32, text: impl Into<String>) -> Edit {
    Edit {
        span: Span::new(at, at),
        replacement: text.into(),
    }
}

fn replace(span: Span, text: impl Into<String>) -> Edit {
    Edit {
        span,
        replacement: text.into(),
    }
}

/// A diagnostic whose message IDs all start with `id`: `id`, `id-label`, `id-why`.
fn diagnostic(code: ErrorCode, title: Message, labels: Vec<Label>, fix: Fix) -> Diagnostic {
    Diagnostic {
        code,
        message: title,
        labels,
        cause: Message::new(why(code)),
        fix,
    }
}

/// The cause message of each code.
const fn why(code: ErrorCode) -> &'static str {
    match code.number() {
        200 => "check-undefined-why",
        201 => "check-shadowing-why",
        202 => "check-duplicate-why",
        203 => "check-unused-why",
        204 => "check-moved-why",
        205 => "check-immutable-why",
        206 => "check-inout-marker-why",
        207 => "check-alias-why",
        209 => "check-move-out-why",
        210 => "check-mismatch-why",
        211 => "check-integer-for-float-why",
        212 => "check-out-of-range-why",
        213 => "check-unknown-type-why",
        214 => "check-argument-count-why",
        215 => "check-unknown-member-why",
        216 => "check-type-argument-count-why",
        217 => "check-condition-why",
        218 => "check-return-why",
        219 => "check-missing-return-why",
        220 => "check-missing-return-type-why",
        222 => "check-match-missing-why",
        223 => "check-unknown-variant-why",
        224 => "check-unreachable-arm-why",
        225 => "check-try-return-why",
        226 => "check-unknown-constraint-why",
        227 => "check-constraint-why",
        228 => "check-missing-constraint-why",
        229 => "check-cannot-infer-why",
        230 => "check-generic-export-why",
        231 => "check-endless-instances-why",
        232 => "check-alias-cycle-why",
        233 => "check-not-constant-why",
        234 => "check-no-methods-why",
        235 => "check-impl-mismatch-why",
        236 => "check-signature-why",
        237 => "check-interface-as-type-why",
        238 => "check-arrow-why",
        239 => "check-template-why",
        _ => "check-operator-why",
    }
}

/// A fix with a "did you mean" edit when a close name exists.
fn suggestion_fix(span: Span, suggestion: Option<&str>, otherwise: &'static str) -> Fix {
    match suggestion {
        Some(name) => Fix {
            message: Message::new("check-did-you-mean").text("name", name),
            edits: vec![replace(span, name)],
        },
        None => Fix {
            message: Message::new(otherwise),
            edits: Vec::new(),
        },
    }
}

/// `E0200`
pub fn undefined(span: Span, name: &str, suggestion: Option<&str>) -> Diagnostic {
    diagnostic(
        codes::UNDEFINED_NAME,
        Message::new("check-undefined").text("name", name),
        vec![primary(span, Message::new("check-undefined-label"))],
        suggestion_fix(span, suggestion, "check-undefined-fix"),
    )
}

/// `E0201`
/// `previous` is `None` for a name the language provides, such as `Some`.
pub fn shadowing(span: Span, name: &str, previous: Option<Span>) -> Diagnostic {
    let mut labels = vec![primary(
        span,
        Message::new("check-shadowing-label").text("name", name),
    )];
    if let Some(previous) = previous {
        labels.push(secondary(
            previous,
            Message::new("check-shadowing-previous-label"),
        ));
    }
    diagnostic(
        codes::SHADOWING,
        Message::new("check-shadowing").text("name", name),
        labels,
        Fix {
            message: Message::new("check-shadowing-fix").text("name", name),
            edits: Vec::new(),
        },
    )
}

/// `E0202`
pub fn duplicate(span: Span, name: &str, previous: Span) -> Diagnostic {
    diagnostic(
        codes::DUPLICATE_DECLARATION,
        Message::new("check-duplicate").text("name", name),
        vec![
            primary(span, Message::new("check-duplicate-label")),
            secondary(previous, Message::new("check-duplicate-previous-label")),
        ],
        Fix {
            message: Message::new("check-duplicate-fix").text("name", name),
            edits: Vec::new(),
        },
    )
}

/// `E0203`. Parameters get an edit adding `_`; variables are best removed.
pub fn unused(span: Span, name: &str, parameter: bool) -> Diagnostic {
    let what = if parameter { "parameter" } else { "variable" };
    let fix = if parameter {
        Fix {
            message: Message::new("check-unused-fix-parameter").text("name", name),
            edits: vec![insert(span.start, "_")],
        }
    } else {
        Fix {
            message: Message::new("check-unused-fix-variable"),
            edits: Vec::new(),
        }
    };
    diagnostic(
        codes::UNUSED,
        Message::new("check-unused")
            .text("what", what)
            .text("name", name),
        vec![primary(span, Message::new("check-unused-label"))],
        fix,
    )
}

/// What makes a binding immutable.
pub enum Immutable {
    /// A `const`, or a `for…of` variable declared `const`: the keyword to replace.
    Const(Span),
    /// A parameter without `inout`: where to insert it.
    Parameter(u32),
}

/// `E0205`
pub fn assign_to_immutable(span: Span, name: &str, declared: Span, why: &Immutable) -> Diagnostic {
    let (kind, fix) = match why {
        Immutable::Const(keyword) => (
            "const",
            Fix {
                message: Message::new("check-immutable-fix-const").text("name", name),
                edits: vec![replace(*keyword, "let")],
            },
        ),
        Immutable::Parameter(at) => (
            "parameter",
            Fix {
                message: Message::new("check-immutable-fix-parameter").text("name", name),
                edits: vec![insert(*at, "inout ")],
            },
        ),
    };
    diagnostic(
        codes::ASSIGN_TO_IMMUTABLE,
        Message::new("check-immutable")
            .text("kind", kind)
            .text("name", name),
        vec![
            primary(span, Message::new("check-immutable-label")),
            secondary(
                declared,
                Message::new("check-immutable-declared-label").text("kind", kind),
            ),
        ],
        fix,
    )
}

/// A conversion that fixes a numeric mismatch: append `.toF64()`, with parentheses
/// around the expression when it is not a simple operand.
pub struct Conversion {
    /// The expression converted.
    pub span: Span,
    /// The method, e.g. `toF64`.
    pub method: &'static str,
    /// Whether the expression needs parentheses first.
    pub parenthesize: bool,
}

/// `E0210`
pub fn mismatch(
    span: Span,
    expected: &Ty,
    found: &Ty,
    conversion: Option<Conversion>,
) -> Diagnostic {
    let fix = match conversion {
        Some(c) => {
            let mut edits = Vec::new();
            if c.parenthesize {
                edits.push(insert(c.span.start, "("));
                edits.push(insert(c.span.end, format!(").{}()", c.method)));
            } else {
                edits.push(insert(c.span.end, format!(".{}()", c.method)));
            }
            Fix {
                message: Message::new("check-mismatch-fix-convert")
                    .text("method", c.method)
                    .text("expected", expected.to_string()),
                edits,
            }
        }
        None => Fix {
            message: Message::new("check-mismatch-fix").text("expected", expected.to_string()),
            edits: Vec::new(),
        },
    };
    diagnostic(
        codes::MISMATCHED_TYPES,
        Message::new("check-mismatch")
            .text("expected", expected.to_string())
            .text("found", found.to_string()),
        vec![primary(
            span,
            Message::new("check-mismatch-label").text("found", found.to_string()),
        )],
        fix,
    )
}

/// `E0211`
pub fn integer_for_float(span: Span, literal: &str, expected: &Ty) -> Diagnostic {
    diagnostic(
        codes::INTEGER_FOR_FLOAT,
        Message::new("check-integer-for-float")
            .text("expected", expected.clone().defaulted().to_string()),
        vec![primary(
            span,
            Message::new("check-integer-for-float-label").text("literal", literal),
        )],
        Fix {
            message: Message::new("check-integer-for-float-fix")
                .text("literal", format!("{literal}.0")),
            edits: vec![insert(span.end, ".0")],
        },
    )
}

/// `E0212`
pub fn out_of_range(span: Span, literal: &str, ty: &Ty, min: &str, max: &str) -> Diagnostic {
    diagnostic(
        codes::LITERAL_OUT_OF_RANGE,
        Message::new("check-out-of-range")
            .text("literal", literal)
            .text("type", ty.to_string()),
        vec![primary(
            span,
            Message::new("check-out-of-range-label")
                .text("type", ty.to_string())
                .text("min", min)
                .text("max", max),
        )],
        Fix {
            message: Message::new("check-out-of-range-fix"),
            edits: Vec::new(),
        },
    )
}

/// `E0213`
pub fn unknown_type(span: Span, name: &str, suggestion: Option<&str>) -> Diagnostic {
    diagnostic(
        codes::UNKNOWN_TYPE,
        Message::new("check-unknown-type").text("name", name),
        vec![primary(span, Message::new("check-unknown-type-label"))],
        suggestion_fix(span, suggestion, "check-unknown-type-fix"),
    )
}

/// `E0214`
pub fn argument_count(
    span: Span,
    name: &str,
    expected: usize,
    found: usize,
    declared: Option<Span>,
) -> Diagnostic {
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    let mut labels = vec![primary(
        span,
        Message::new("check-argument-count-label").number("found", count(found)),
    )];
    if let Some(declared) = declared {
        labels.push(secondary(
            declared,
            Message::new("check-argument-count-declared-label").number("expected", count(expected)),
        ));
    }
    diagnostic(
        codes::ARGUMENT_COUNT,
        Message::new("check-argument-count")
            .text("name", name)
            .number("expected", count(expected))
            .number("found", count(found)),
        labels,
        Fix {
            message: Message::new("check-argument-count-fix").number("expected", count(expected)),
            edits: Vec::new(),
        },
    )
}

/// `E0215`
pub fn unknown_member(span: Span, ty: &Ty, member: &str, suggestion: Option<&str>) -> Diagnostic {
    diagnostic(
        codes::UNKNOWN_MEMBER,
        Message::new("check-unknown-member")
            .text("type", ty.to_string())
            .text("member", member),
        vec![primary(
            span,
            Message::new("check-unknown-member-label").text("type", ty.to_string()),
        )],
        suggestion_fix(span, suggestion, "check-unknown-member-fix"),
    )
}

/// `E0216`
pub fn type_argument_count(span: Span, name: &str, expected: usize, found: usize) -> Diagnostic {
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    diagnostic(
        codes::TYPE_ARGUMENT_COUNT,
        Message::new("check-type-argument-count")
            .text("name", name)
            .number("expected", count(expected))
            .number("found", count(found)),
        vec![primary(
            span,
            Message::new("check-type-argument-count-label").number("expected", count(expected)),
        )],
        Fix {
            message: Message::new("check-type-argument-count-fix")
                .text("name", name)
                .number("expected", count(expected)),
            edits: Vec::new(),
        },
    )
}

/// How to turn a non-boolean condition into a comparison.
pub enum ConditionFix {
    /// Append text, e.g. ` != 0`, with parentheses when needed.
    Append(&'static str, bool),
    /// No certain fix.
    None,
}

/// `E0217`
pub fn condition_not_bool(span: Span, found: &Ty, fix: &ConditionFix) -> Diagnostic {
    let fix = match fix {
        ConditionFix::Append(text, parenthesize) => {
            let mut edits = Vec::new();
            if *parenthesize {
                edits.push(insert(span.start, "("));
                edits.push(insert(span.end, format!("){text}")));
            } else {
                edits.push(insert(span.end, *text));
            }
            Fix {
                message: Message::new("check-condition-fix-compare")
                    .text("comparison", text.trim()),
                edits,
            }
        }
        ConditionFix::None => Fix {
            message: Message::new("check-condition-fix"),
            edits: Vec::new(),
        },
    };
    diagnostic(
        codes::CONDITION_NOT_BOOL,
        Message::new("check-condition").text("found", found.to_string()),
        vec![primary(
            span,
            Message::new("check-condition-label").text("found", found.to_string()),
        )],
        fix,
    )
}

/// `E0218`: `return;` where a value is expected, or a value in a `void` function.
pub fn return_mismatch(
    span: Span,
    expected: &Ty,
    has_value: bool,
    ret_span: Option<Span>,
) -> Diagnostic {
    let key = if has_value { "value" } else { "missing" };
    let mut labels = vec![primary(
        span,
        Message::new("check-return-label").text("kind", key),
    )];
    if let Some(ret) = ret_span {
        labels.push(secondary(
            ret,
            Message::new("check-return-declared-label").text("expected", expected.to_string()),
        ));
    }
    diagnostic(
        codes::RETURN_MISMATCH,
        Message::new("check-return")
            .text("kind", key)
            .text("expected", expected.to_string()),
        labels,
        Fix {
            message: Message::new("check-return-fix").text("kind", key),
            edits: Vec::new(),
        },
    )
}

/// `E0219`
pub fn missing_return(end: Span, name: &str, expected: &Ty, ret_span: Span) -> Diagnostic {
    diagnostic(
        codes::MISSING_RETURN,
        Message::new("check-missing-return").text("name", name),
        vec![
            primary(end, Message::new("check-missing-return-label")),
            secondary(
                ret_span,
                Message::new("check-missing-return-declared-label")
                    .text("expected", expected.to_string()),
            ),
        ],
        Fix {
            message: Message::new("check-missing-return-fix")
                .text("expected", expected.to_string()),
            edits: Vec::new(),
        },
    )
}

/// `E0220`. `inferred` is the type of the returned values, when known.
pub fn missing_return_type(
    name_span: Span,
    name: &str,
    exported: bool,
    insert_at: u32,
    inferred: &Ty,
) -> Diagnostic {
    let reason = if exported { "exported" } else { "value" };
    diagnostic(
        codes::MISSING_RETURN_TYPE,
        Message::new("check-missing-return-type").text("name", name),
        vec![primary(
            name_span,
            Message::new("check-missing-return-type-label").text("reason", reason),
        )],
        Fix {
            message: Message::new("check-missing-return-type-fix")
                .text("type", inferred.to_string()),
            edits: vec![insert(insert_at, format!(": {inferred}"))],
        },
    )
}

/// `E0221`
pub fn invalid_operator(span: Span, operator: &str, ty: &Ty) -> Diagnostic {
    diagnostic(
        codes::INVALID_OPERATOR,
        Message::new("check-operator")
            .text("operator", operator)
            .text("type", ty.to_string()),
        vec![primary(
            span,
            Message::new("check-operator-label").text("type", ty.to_string()),
        )],
        Fix {
            message: Message::new("check-operator-fix").text("operator", operator),
            edits: Vec::new(),
        },
    )
}

/// `.copy()` inserted after an expression ending at `at`.
fn copy_fix(at: Option<u32>, otherwise: &'static str) -> Fix {
    match at {
        Some(at) => Fix {
            message: Message::new("check-fix-copy"),
            edits: vec![insert(at, ".copy()")],
        },
        None => Fix {
            message: Message::new(otherwise),
            edits: Vec::new(),
        },
    }
}

/// `E0204`: `name` is used at `span` after it moved at `moved_at`.
///
/// The labels tell the value's history: where it was given, where it moved,
/// where it is used again.
pub fn moved(
    span: Span,
    name: &str,
    moved_at: Span,
    given: Option<Span>,
    copyable: bool,
) -> Diagnostic {
    // The same place: a loop moved it in the previous turn.
    let mut labels = if moved_at == span {
        vec![primary(span, Message::new("check-moved-loop-label"))]
    } else {
        vec![
            primary(span, Message::new("check-moved-label")),
            secondary(moved_at, Message::new("check-moved-here-label")),
        ]
    };
    if let Some(given) = given.filter(|g| *g != span && *g != moved_at) {
        labels.insert(
            0,
            secondary(
                given,
                Message::new("check-moved-given-label").text("name", name),
            ),
        );
    }
    diagnostic(
        codes::USE_AFTER_MOVE,
        Message::new("check-moved").text("name", name),
        labels,
        copy_fix(copyable.then_some(moved_at.end), "check-moved-fix"),
    )
}

/// `E0209`: a parameter that only borrows its value moves it.
pub fn move_borrowed_parameter(span: Span, name: &str, sink_at: Option<u32>) -> Diagnostic {
    let fix = match sink_at {
        Some(at) => Fix {
            message: Message::new("check-move-out-fix-sink").text("name", name),
            edits: vec![insert(at, "sink ")],
        },
        None => Fix {
            message: Message::new("check-move-out-fix"),
            edits: Vec::new(),
        },
    };
    diagnostic(
        codes::MOVE_OUT_OF_BORROW,
        Message::new("check-move-out")
            .text("what", "parameter")
            .text("name", name),
        vec![primary(span, Message::new("check-move-out-label"))],
        fix,
    )
}

/// `E0209`: a field, an element or a module-level value moves.
pub fn move_out(span: Span, what: &'static str, copyable: bool) -> Diagnostic {
    diagnostic(
        codes::MOVE_OUT_OF_BORROW,
        Message::new("check-move-out")
            .text("what", what)
            .text("name", ""),
        vec![primary(span, Message::new("check-move-out-label"))],
        copy_fix(copyable.then_some(span.end), "check-move-out-fix"),
    )
}

/// `E0206`: an `inout` argument without `&` (`missing`), or `&` on another.
pub fn inout_marker(span: Span, missing: bool) -> Diagnostic {
    let (kind, fix) = if missing {
        (
            "missing",
            Fix {
                message: Message::new("check-inout-marker-fix-add"),
                edits: vec![insert(span.start, "&")],
            },
        )
    } else {
        (
            "extra",
            Fix {
                message: Message::new("check-inout-marker-fix-remove"),
                edits: vec![replace(Span::new(span.start, span.start + 1), "")],
            },
        )
    };
    diagnostic(
        codes::INOUT_MARKER,
        Message::new("check-inout-marker").text("kind", kind),
        vec![primary(
            span,
            Message::new("check-inout-marker-label").text("kind", kind),
        )],
        fix,
    )
}

/// `E0207`: `name` reached by the argument at `span` and the one at `other`.
pub fn alias(
    span: Span,
    name: &str,
    callee: &str,
    other: Span,
    copy_at: Option<u32>,
) -> Diagnostic {
    diagnostic(
        codes::ALIASING,
        Message::new("check-alias")
            .text("name", name)
            .text("callee", callee),
        vec![
            primary(span, Message::new("check-alias-label")),
            secondary(other, Message::new("check-alias-other-label")),
        ],
        copy_fix(copy_at, "check-alias-fix"),
    )
}

/// `E0207`: `name` changes at `span` while a `for…of` goes through it.
pub fn changed_while_iterated(span: Span, name: &str, iterated: Span) -> Diagnostic {
    diagnostic(
        codes::ALIASING,
        Message::new("check-alias-loop").text("name", name),
        vec![
            primary(span, Message::new("check-alias-loop-label")),
            secondary(iterated, Message::new("check-alias-loop-iterated-label")),
        ],
        Fix {
            message: Message::new("check-alias-loop-fix").text("name", name),
            edits: vec![insert(iterated.end, ".copy()")],
        },
    )
}

/// `E0222`: the patterns `missing` are not covered; the arms close at `close`.
pub fn non_exhaustive(span: Span, ty: &Ty, missing: &[String], close: u32) -> Diagnostic {
    let list = missing.join(", ");
    diagnostic(
        codes::NON_EXHAUSTIVE_MATCH,
        Message::new("check-match-missing")
            .text("ty", ty.to_string())
            .text("missing", list.clone()),
        vec![
            primary(
                Span::new(close, close.saturating_add(1)),
                Message::new("check-match-missing-label").text("missing", list),
            ),
            secondary(span, Message::new("check-match-missing-match-label")),
        ],
        Fix {
            message: Message::new("check-match-missing-fix"),
            edits: Vec::new(),
        },
    )
}

/// `E0223`
pub fn unknown_variant(span: Span, ty: &Ty, name: &str, suggestion: Option<&str>) -> Diagnostic {
    diagnostic(
        codes::UNKNOWN_VARIANT,
        Message::new("check-unknown-variant")
            .text("ty", ty.to_string())
            .text("name", name),
        vec![primary(span, Message::new("check-unknown-variant-label"))],
        suggestion_fix(span, suggestion, "check-unknown-variant-fix"),
    )
}

/// `E0224`
pub fn unreachable_arm(span: Span) -> Diagnostic {
    diagnostic(
        codes::UNREACHABLE_ARM,
        Message::new("check-unreachable-arm"),
        vec![primary(span, Message::new("check-unreachable-arm-label"))],
        Fix {
            message: Message::new("check-unreachable-arm-fix"),
            edits: Vec::new(),
        },
    )
}

/// `E0225`: `?` gives back `early`, which the function, returning `ret`, cannot.
pub fn try_return(span: Span, early: &Ty, ret: Option<&Ty>) -> Diagnostic {
    let ret = ret.map_or_else(|| "void".to_owned(), ToString::to_string);
    let kind = if matches!(early, Ty::Option(_)) {
        "option"
    } else {
        "result"
    };
    diagnostic(
        codes::TRY_RETURN,
        Message::new("check-try-return")
            .text("kind", kind)
            .text("ret", ret),
        vec![primary(
            span,
            Message::new("check-try-return-label").text("kind", kind),
        )],
        Fix {
            message: Message::new("check-try-return-fix").text("kind", kind),
            edits: Vec::new(),
        },
    )
}

/// `E0226`
pub fn unknown_constraint(span: Span, name: &str, suggestion: Option<&str>) -> Diagnostic {
    diagnostic(
        codes::UNKNOWN_CONSTRAINT,
        Message::new("check-unknown-constraint").text("name", name),
        vec![primary(
            span,
            Message::new("check-unknown-constraint-label"),
        )],
        suggestion_fix(span, suggestion, "check-unknown-constraint-fix"),
    )
}

/// `E0227`: `ty`, given for `param`, does not meet `bound`.
pub fn constraint_not_met(
    span: Span,
    ty: &Ty,
    param: &str,
    bound: &str,
    declared: Span,
) -> Diagnostic {
    diagnostic(
        codes::CONSTRAINT_NOT_MET,
        Message::new("check-constraint")
            .text("ty", ty.to_string())
            .text("bound", bound),
        vec![
            primary(
                span,
                Message::new("check-constraint-label")
                    .text("ty", ty.to_string())
                    .text("param", param),
            ),
            secondary(
                declared,
                Message::new("check-constraint-declared")
                    .text("param", param)
                    .text("bound", bound),
            ),
        ],
        Fix {
            message: Message::new("check-constraint-fix")
                .text("param", param)
                .text("bound", bound),
            edits: Vec::new(),
        },
    )
}

/// `E0228`: `op` on `param` needs `bound`, written at `insert_at`.
pub fn missing_constraint(
    span: Span,
    op: &str,
    param: &str,
    bound: &str,
    insert_at: u32,
    bounded: bool,
) -> Diagnostic {
    let text = if bounded {
        format!(" + {bound}")
    } else {
        format!(": {bound}")
    };
    diagnostic(
        codes::MISSING_CONSTRAINT,
        Message::new("check-missing-constraint")
            .text("op", op)
            .text("param", param),
        vec![primary(
            span,
            Message::new("check-missing-constraint-label")
                .text("op", op)
                .text("bound", bound),
        )],
        Fix {
            message: Message::new("check-missing-constraint-fix")
                .text("param", param)
                .text("bound", bound),
            edits: vec![insert(insert_at, text)],
        },
    )
}

/// `E0229`: nothing tells which type `param` of `name` stands for.
pub fn cannot_infer(span: Span, param: &str, name: &str, declared: Span) -> Diagnostic {
    diagnostic(
        codes::CANNOT_INFER,
        Message::new("check-cannot-infer")
            .text("param", param)
            .text("name", name),
        vec![
            primary(
                span,
                Message::new("check-cannot-infer-label").text("param", param),
            ),
            secondary(
                declared,
                Message::new("check-cannot-infer-declared").text("param", param),
            ),
        ],
        Fix {
            message: Message::new("check-cannot-infer-fix").text("param", param),
            edits: Vec::new(),
        },
    )
}

/// `E0230`
pub fn generic_export(span: Span, name: &str) -> Diagnostic {
    diagnostic(
        codes::GENERIC_EXPORT,
        Message::new("check-generic-export").text("name", name),
        vec![primary(span, Message::new("check-generic-export-label"))],
        Fix {
            message: Message::new("check-generic-export-fix").text("name", name),
            edits: Vec::new(),
        },
    )
}

/// `E0231`
pub fn endless_instances(span: Span, name: &str) -> Diagnostic {
    diagnostic(
        codes::ENDLESS_INSTANCES,
        Message::new("check-endless-instances").text("name", name),
        vec![primary(span, Message::new("check-endless-instances-label"))],
        Fix {
            message: Message::new("check-endless-instances-fix"),
            edits: Vec::new(),
        },
    )
}

/// A diagnostic with one label and a fix without an edit.
fn simple(code: ErrorCode, title: Message, span: Span, label: Message, fix: Message) -> Diagnostic {
    diagnostic(
        code,
        title,
        vec![primary(span, label)],
        Fix {
            message: fix,
            edits: Vec::new(),
        },
    )
}

/// `E0232`
pub fn alias_cycle(span: Span, name: &str) -> Diagnostic {
    simple(
        codes::ALIAS_CYCLE,
        Message::new("check-alias-cycle").text("name", name),
        span,
        Message::new("check-alias-cycle-label"),
        Message::new("check-alias-cycle-fix"),
    )
}

/// `E0233`
pub fn not_constant(span: Span) -> Diagnostic {
    simple(
        codes::NOT_CONSTANT,
        Message::new("check-not-constant"),
        span,
        Message::new("check-not-constant-label"),
        Message::new("check-not-constant-fix"),
    )
}

/// `E0234`
pub fn no_methods(span: Span, ty: &Ty) -> Diagnostic {
    simple(
        codes::NO_METHODS,
        Message::new("check-no-methods").text("ty", ty.to_string()),
        span,
        Message::new("check-no-methods-label"),
        Message::new("check-no-methods-fix"),
    )
}

/// `E0235`: methods of `interface` that the implementation for `ty` lacks.
pub fn missing_methods(span: Span, interface: &str, ty: &Ty, missing: &[String]) -> Diagnostic {
    let list: Vec<String> = missing.iter().map(|m| format!("`{m}`")).collect();
    simple(
        codes::IMPL_MISMATCH,
        Message::new("check-impl-missing")
            .text("interface", interface)
            .text("ty", ty.to_string())
            .text("missing", list.join(", ")),
        span,
        Message::new("check-impl-missing-label").text("missing", list.join(", ")),
        Message::new("check-impl-missing-fix").text("interface", interface),
    )
}

/// `E0235`: a method the interface does not have.
pub fn extra_method(span: Span, name: &str, interface: &str) -> Diagnostic {
    simple(
        codes::IMPL_MISMATCH,
        Message::new("check-impl-extra")
            .text("name", name)
            .text("interface", interface),
        span,
        Message::new("check-impl-extra-label").text("interface", interface),
        Message::new("check-impl-extra-fix").text("name", name),
    )
}

/// `E0236`
pub fn signature_mismatch(
    span: Span,
    name: &str,
    interface: &str,
    expected: &str,
    declared: Span,
) -> Diagnostic {
    diagnostic(
        codes::SIGNATURE_MISMATCH,
        Message::new("check-signature")
            .text("name", name)
            .text("interface", interface),
        vec![
            primary(
                span,
                Message::new("check-signature-label").text("expected", expected),
            ),
            secondary(
                declared,
                Message::new("check-signature-declared").text("interface", interface),
            ),
        ],
        Fix {
            message: Message::new("check-signature-fix").text("expected", expected),
            edits: Vec::new(),
        },
    )
}

/// `E0236`: an interface's method must take `self`.
pub fn interface_without_self(span: Span, name: &str) -> Diagnostic {
    simple(
        codes::SIGNATURE_MISMATCH,
        Message::new("check-interface-self").text("name", name),
        span,
        Message::new("check-interface-self-label"),
        Message::new("check-interface-self-fix").text("name", name),
    )
}

/// `E0237`
pub fn interface_as_type(span: Span, name: &str) -> Diagnostic {
    simple(
        codes::INTERFACE_AS_TYPE,
        Message::new("check-interface-as-type").text("name", name),
        span,
        Message::new("check-interface-as-type-label"),
        Message::new("check-interface-as-type-fix").text("name", name),
    )
}

/// `E0239`: a value a template literal cannot write as text yet.
pub fn template_value(span: Span, ty: &Ty) -> Diagnostic {
    diagnostic(
        codes::TEMPLATE_VALUE,
        Message::new("check-template").text("type", ty.to_string()),
        vec![primary(
            span,
            Message::new("check-template-label").text("type", ty.to_string()),
        )],
        Fix {
            message: Message::new("check-template-fix"),
            edits: Vec::new(),
        },
    )
}

/// `E0238`: an arrow function where no method takes one.
pub fn arrow_outside(span: Span) -> Diagnostic {
    simple(
        codes::ARROW_FUNCTION,
        Message::new("check-arrow"),
        span,
        Message::new("check-arrow-label"),
        Message::new("check-arrow-fix"),
    )
}

/// `E0238`: a method that takes an arrow function was given something else.
pub fn arrow_expected(span: Span) -> Diagnostic {
    simple(
        codes::ARROW_FUNCTION,
        Message::new("check-arrow-expected"),
        span,
        Message::new("check-arrow-expected-label"),
        Message::new("check-arrow-expected-fix"),
    )
}
