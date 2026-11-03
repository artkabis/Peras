# English messages — source of truth.
# Every key here must exist in fr.ftl; CI fails otherwise.

cli-version = perasc { $version }
cli-usage =
    Usage: perasc <command> [options]

    Commands:
      check <file>...    check .peras and .apeiron files and report every error
      explain <code>     explain an error code, e.g. `perasc explain E0001`

    Options:
      --lang <en|fr>     language of messages (default: PERAS_LANG, then the system locale)
      --json             print the diagnostics of `check` as JSON on standard output
      --color <when>     colour diagnostics: auto (default), always, never; NO_COLOR is honoured
      -V, --version      print the version
      -h, --help         print this help
cli-unknown-argument = unknown argument `{ $argument }`
cli-unknown-command = unknown command `{ $command }`
cli-missing-value = `{ $option }` needs a value
cli-invalid-value = invalid value `{ $value }` for `{ $option }`
cli-no-files = `check` needs at least one file
cli-missing-code = `explain` needs an error code, e.g. E0001
cli-unknown-code = unknown error code `{ $code }`
cli-not-a-source = `{ $path }` is not a Peras source: the extension must be .peras or .apeiron
cli-not-found = `{ $path }` does not exist
cli-not-utf8 = `{ $path }` is not UTF-8 text
cli-read-error = cannot read `{ $path }`: { $reason }
cli-hidden-errors = { $count ->
    [one] 1 more error
   *[other] { $count } more errors
} in { $path }, not shown: the limit is { $limit } per file
cli-summary-ok = { $files ->
    [one] 1 file checked, no error
   *[other] { $files } files checked, no error
}
cli-summary-errors = { $errors ->
    [one] 1 error: the build is blocked
   *[other] { $errors } errors: the build is blocked
}

## Lexer (E0001–E0099)

# E0001
lex-invalid-character = { $count ->
    [one] invalid character { $character } ({ $codepoint })
   *[other] { $count } invalid characters, starting with { $character } ({ $codepoint })
}
lex-invalid-character-label = { $count ->
    [one] this character is not allowed in Peras code
   *[other] these characters are not allowed in Peras code
}
lex-invalid-character-why = Outside strings and comments, Peras code only uses ASCII letters, digits, `_`, spaces and the operators of the language.
lex-invalid-character-fix-remove = { $count ->
    [one] remove this character
   *[other] remove these characters
}
lex-invalid-character-fix-replace = replace { $character } with `{ $replacement }`, the ASCII character it looks like
lex-invalid-character-fix-identifier = names use ASCII letters, digits and `_` only: rename it, e.g. `cafe` instead of `café`; to show text, put it in a string
lex-invalid-character-fix-template = template literals are not supported yet: join strings with `+`

# E0002
lex-unterminated-string = unterminated string
lex-unterminated-string-label = this string is never closed
lex-unterminated-string-end-label = the line ends here, before the closing `{ $quote }`
lex-unterminated-string-why = A string must close on the line where it starts; to put a line break in it, write `\n`.
lex-unterminated-string-fix = close the string with `{ $quote }`

# E0003
lex-unterminated-block-comment = unterminated block comment
lex-unterminated-block-comment-label = this comment is never closed
lex-unterminated-block-comment-why = A `/*` comment runs until the next `*/`; without one, the rest of the file is a comment.
lex-unterminated-block-comment-fix = close the comment with `*/`

# E0004
lex-invalid-escape = invalid escape sequence `{ $escape }`
lex-invalid-escape-unknown-label = `\{ $character }` is not an escape sequence
lex-invalid-escape-braces-label = Peras writes the code point between braces
lex-invalid-escape-unicode-label = not a valid Unicode escape
lex-invalid-escape-why = Strings accept only `\n`, `\r`, `\t`, `\0`, `\\`, `\'`, `\"` and `\u{"{"}…{"}"}` with 1 to 6 hexadecimal digits naming a Unicode scalar value.
lex-invalid-escape-fix-backslash = to write a backslash, double it: `\\`
lex-invalid-escape-fix-braces = write `{ $replacement }`
lex-invalid-escape-fix-unicode = write a code point from `\u{"{"}0{"}"}` to `\u{"{"}10FFFF{"}"}`, excluding the surrogates `\u{"{"}D800{"}"}` to `\u{"{"}DFFF{"}"}`

# E0005
lex-malformed-number = malformed number `{ $literal }`
lex-number-no-digits-label = no digits after `{ $prefix }`
lex-number-no-digits-why = A `0x`, `0o` or `0b` prefix must be followed by at least one digit.
lex-number-no-digits-fix = add digits after the prefix, e.g. `0xFF`
lex-number-invalid-digit-label = `{ $digit }` is not { $base ->
    [2] a binary
    [8] an octal
   *[other] a hexadecimal
} digit
lex-number-invalid-digit-why = Binary numbers (`0b`) use the digits 0 and 1, octal numbers (`0o`) 0 to 7, hexadecimal numbers (`0x`) 0 to 9 and a to f.
lex-number-invalid-digit-fix = remove the digit, or change the prefix to the right base
lex-number-leading-zero-label = leading zero
lex-number-leading-zero-why = In JavaScript, `012` used to mean an octal number; Peras forbids the ambiguity and writes octal numbers with `0o`.
lex-number-leading-zero-fix = remove the leading zeros
lex-number-leading-zero-fix-ambiguous = write `{ $octal }` for an octal number, or `{ $decimal }` for a decimal one
lex-number-underscore-label = `_` must sit between two digits
lex-number-underscore-why = Underscores separate groups of digits, as in `1_000_000`; they cannot start or end a number, touch a `.` or a prefix, or follow each other.
lex-number-underscore-fix = remove this `_`
lex-number-exponent-label = the exponent has no digits
lex-number-exponent-why = An exponent needs digits after `e`, as in `1e3` or `2.5e-4`.
lex-number-exponent-fix = add digits after `e`, or remove the `e`
lex-number-suffix-label = unexpected `{ $suffix }` after the number
lex-number-suffix-why = Number literals have no unit or type suffix: the type comes from the context or from an annotation.
lex-number-suffix-fix = remove `{ $suffix }`; to choose a type, annotate the variable, e.g. `let x: f32 = 5.0`
lex-number-radix-fraction-label = only decimal numbers can have a fraction
lex-number-radix-fraction-why = Hexadecimal, octal and binary numbers are integers; only decimal numbers have a fraction or an exponent.
lex-number-radix-fraction-fix = write the number in decimal

# E0006
lex-strict-equality = `{ $operator }` does not exist in Peras
lex-strict-equality-label = write `{ $replacement }`
lex-strict-equality-why = Peras never converts types implicitly, so `==` and `!=` already compare strictly, like `===` and `!==` in JavaScript.
lex-strict-equality-fix = replace `{ $operator }` with `{ $replacement }`

# E0007
lex-bidi-control = invisible bidirectional control character { $codepoint } ({ $name })
lex-bidi-control-label = this invisible character changes the order in which the code is displayed
lex-bidi-control-why = A bidirectional control character can make code look different from what it compiles to (the Trojan Source attack, CVE-2021-42574), so it is refused everywhere, strings and comments included.
lex-bidi-control-fix-remove = remove this character
lex-bidi-control-fix-escape = write it as the escape `{ $escape }`, which stays visible

# E0008
lex-source-too-large = source file too large
lex-source-too-large-label = this file is { $size } bytes long
lex-source-too-large-why = Source positions are 32-bit numbers, so a source file must be smaller than 4 GiB.
lex-source-too-large-fix = split this file into several modules

## Parser (E0100–E0199)

# E0100
parse-expected-expression = expected a value, found { $eof ->
    [1] the end of the file
   *[other] `{ $found }`
}
parse-expected-expression-label = a value is needed here
parse-expected-expression-why = An expression is needed at this point: a number, a string, a name, a call such as `f(x)`, or a calculation such as `a + 1`.
parse-expected-expression-fix = write the missing value, or remove what precedes this point

# E0101
parse-missing-semicolon = missing `;`
parse-missing-semicolon-label = the statement ends here
parse-missing-semicolon-why = Every statement ends with `;`. Peras does not guess where a statement ends, unlike JavaScript's automatic semicolon insertion, which can join two lines into one statement.
parse-missing-semicolon-fix = add `;`

# E0102
parse-expected-name = expected { $what ->
    [function] a function name
    [variable] a variable name
    [parameter] a parameter name
    [field] a field name
    [attribute] an attribute name
    [struct] a structure name
   *[other] a name
}, found { $eof ->
    [1] the end of the file
   *[other] `{ $found }`
}
parse-expected-name-label = a name is needed here
parse-expected-name-why = A name starts with a letter or `_`, followed by letters, digits or `_`. Keywords such as `fn`, `let` or `match` are reserved.
parse-expected-name-fix = write a name here
parse-expected-name-fix-keyword = `{ $keyword }` is a keyword: choose another name, e.g. `{ $keyword }_`

# E0103
parse-unclosed = unclosed `{ $open }`
parse-unclosed-open-label = `{ $open }` opened here
parse-unclosed-label = `{ $close }` expected here, before { $eof ->
    [1] the end of the file
   *[other] `{ $found }`
}
parse-unclosed-why = Every `(`, `[`, `{"{"}` and `<` needs its closing `)`, `]`, `{"}"}` or `>`; the parser reached the end of the construct first.
parse-unclosed-fix = close it with `{ $close }`

# E0104
parse-unexpected = unexpected { $eof ->
    [1] the end of the file
   *[other] `{ $found }`
}
parse-unexpected-label = this cannot appear here
parse-unexpected-expected-label = expected `{ $expected }`
parse-unexpected-separator-label = expected `,` or `{ $close }`
parse-unexpected-why = Each construct has a fixed shape, such as `fn name(parameter: Type): Type {"{"} … {"}"}` or `let name = value;`, and this token does not fit at this point.
parse-unexpected-fix = check this line for a missing operator, separator or bracket

# E0105
parse-expected-type = expected a type, found { $eof ->
    [1] the end of the file
   *[other] `{ $found }`
}
parse-expected-type-label = a type is needed here, e.g. `i32`
parse-expected-type-why = A type follows `:` in a parameter, a return type or an annotated variable: `i32`, `f64`, `bool`, `Array<f32>`, `Option<i32>`…
parse-expected-type-fix = write a type, e.g. `i32`

# E0106
parse-removed-keyword = `{ $keyword }` does not exist in Peras
parse-removed-keyword-label = write `{ $replacement }`
parse-removed-keyword-why = { $keyword ->
    [var] `var` is scoped to the whole function and can be declared twice; Peras only has `let` and `const`, scoped to their block.
   *[other] Peras declares functions with `fn`, which never has a dynamic `this`.
}
parse-removed-keyword-fix = replace `{ $keyword }` with `{ $replacement }`

# E0107
parse-number-too-large = number `{ $literal }` is too large
parse-number-too-large-int-label = does not fit in 64 bits
parse-number-too-large-float-label = is infinite as a 64-bit float
parse-number-too-large-why = The widest integers have 64 bits (`i64` up to 9223372036854775807, `u64` up to 18446744073709551615), and an `f64` cannot exceed about 1.8e308.
parse-number-too-large-fix = use a smaller value

# E0108
parse-chained-comparison = comparisons cannot be chained
parse-chained-comparison-first-label = first comparison
parse-chained-comparison-label = second comparison
parse-chained-comparison-why = In JavaScript, `a < b < c` compares the boolean result of `a < b` with `c`, which is almost never intended; Peras refuses it.
parse-chained-comparison-fix = compare each pair and join them with `&&`: `{ $suggestion }`
parse-chained-comparison-fix-mixed = split the expression with `&&`, or add parentheses to show what is compared

# E0109
parse-bitwise-mix = `{ $bitwise }` and `{ $comparison }` need parentheses
parse-bitwise-mix-label = bitwise operator
parse-bitwise-mix-comparison-label = comparison
parse-bitwise-mix-why = JavaScript reads `a & b == c` as `a & (b == c)`, because `&`, `|` and `^` bind more loosely than comparisons; that is rarely intended, so Peras makes you write the order.
parse-bitwise-mix-fix = add parentheses: `{ $suggestion }`

# E0110
parse-expected-item = expected a declaration, found { $eof ->
    [1] the end of the file
   *[other] `{ $found }`
}
parse-expected-item-label = only declarations can appear at the top level
parse-expected-item-why = A module holds declarations: `fn`, `struct`, `let` and `const`, optionally preceded by `export` or an attribute such as `@frame`. Statements go inside a function body.
parse-expected-item-fix = move this code into a function

# E0111
parse-too-deep = nesting is too deep
parse-too-deep-label = more than { $limit } levels deep
parse-too-deep-why = The parser stops at { $limit } levels of nested expressions or types, so that no file, even a hostile or generated one, can exhaust the compiler's stack.
parse-too-deep-fix = split the expression into named intermediate variables

# E0112
parse-const-without-value = `{ $keyword }` without a value
parse-const-without-value-label = `{ $name }` needs a value
parse-const-without-value-why = A `const` is given its value once, where it is declared, and never changes.
parse-const-without-value-fix = give it a value with `= …`, or declare it with `let` to assign it later

# E0113
parse-assignment-in-expression = an assignment is not an expression
parse-assignment-in-expression-label = `{ $operator }` assigns here
parse-assignment-in-expression-why = In Peras, an assignment is a statement of its own and has no value, so `if (a = b)` or `x = y = 0` cannot hide a mistake.
parse-assignment-in-expression-fix-compare = to compare, write `==`
parse-assignment-in-expression-fix = make the assignment a statement of its own, on its own line

# E0114
parse-invalid-target = cannot assign to this expression
parse-invalid-target-label = not a variable, a field or an element
parse-invalid-target-why = Only a variable (`x`), a field (`p.x`) or an element (`a[i]`) can be assigned, incremented or decremented.
parse-invalid-target-fix = assign to a variable, a field or an element instead

# E0115
parse-condition-parentheses = the condition of `{ $keyword }` needs parentheses
parse-condition-parentheses-label = this condition
parse-condition-parentheses-why = As in TypeScript, the condition of `if` and `while` is written in parentheses: `if (x > 0)`.
parse-condition-parentheses-fix = put the condition in parentheses

# E0116
parse-for-in = `for…in` does not exist in Peras
parse-for-in-label = write `of`
parse-for-in-why = In JavaScript, `for…in` walks the keys of an object, inherited ones included, as strings; Peras only has `for…of`, which walks the values of a collection.
parse-for-in-fix = replace `in` with `of`

# E0117
parse-jump-outside-loop = `{ $keyword }` outside a loop
parse-jump-outside-loop-label = no loop around this `{ $keyword }`
parse-jump-outside-loop-why = `{ $keyword }` only works inside `for` or `while`, in the same function.
parse-jump-outside-loop-fix = remove `{ $keyword }`, or use `return` to leave the function

# E0118
parse-optional-marker = `?:` does not exist in Peras
parse-optional-marker-label = optional marker
parse-optional-marker-why = Peras has no implicit `undefined` or `null`: a value that may be missing has the type `Option<T>`, and the compiler makes you handle both cases.
parse-optional-marker-fix = write the type `Option<{ $type }>`

# E0119
parse-empty-body = `;` right after `{ $keyword }` leaves it empty
parse-empty-body-label = this `;` ends the `{ $keyword }`
parse-empty-body-why = In JavaScript, `if (c); {"{"} … {"}"}` runs the block every time: the `;` is the whole body. Peras refuses this silent mistake; an intentionally empty body is written `{"{"}{"}"}`.
parse-empty-body-fix = remove the `;`, or write `{"{"}{"}"}` for an empty body

## Names and types (E0200–E0299)

check-did-you-mean = did you mean `{ $name }`?

# E0200
check-undefined = `{ $name }` is not defined
check-undefined-label = not found in this scope
check-undefined-why = A name must be declared before it is used, in this scope or an enclosing one: with `let`, `const`, `fn` or `struct`, or as a parameter.
check-undefined-fix = declare it, or check its spelling

# E0201
check-shadowing = `{ $name }` hides another declaration
check-shadowing-label = this `{ $name }` hides the other one
check-shadowing-previous-label = first declared here
check-shadowing-why = Peras refuses shadowing: two variables with the same name in nested scopes make it easy to read or change the wrong one.
check-shadowing-fix = give this `{ $name }` another name

# E0202
check-duplicate = `{ $name }` is declared twice
check-duplicate-label = declared again here
check-duplicate-previous-label = first declared here
check-duplicate-why = A name is declared once per scope: functions, structures and module constants share one namespace, and so do the fields of a structure and the parameters of a function.
check-duplicate-fix = rename or remove one of the two `{ $name }`

# E0203
check-unused = { $what ->
    [parameter] parameter
   *[other] variable
} `{ $name }` is never read
check-unused-label = never read
check-unused-why = A value that is never read is dead code, and often a bug: a typo, or a use that was forgotten. A name starting with `_` marks a parameter as unused on purpose.
check-unused-fix-parameter = if it is unused on purpose, name it `_{ $name }`
check-unused-fix-variable = remove it, or use its value

# E0205
check-immutable = cannot modify { $kind ->
    [parameter] the parameter
   *[other] the constant
} `{ $name }`
check-immutable-label = modified here
check-immutable-declared-label = { $kind ->
    [parameter] read-only parameter
   *[other] declared with `const`
}
check-immutable-why = A `const` never changes, and a parameter is read-only unless it is passed `inout`, which shows the caller that the function modifies it.
check-immutable-fix-const = declare `{ $name }` with `let`
check-immutable-fix-parameter = pass `{ $name }` as `inout`

# E0210
check-mismatch = mismatched types: expected `{ $expected }`, found `{ $found }`
check-mismatch-label = this is `{ $found }`
check-mismatch-why = Peras never converts a value implicitly, not even between numbers: `i32 + f64` is an error, because a silent conversion can lose precision or overflow.
check-mismatch-fix-convert = convert it explicitly with `.{ $method }()`
check-mismatch-fix = use a value of type `{ $expected }`

# E0211
check-integer-for-float = integer literal where `{ $expected }` is expected
check-integer-for-float-label = `{ $literal }` is an integer
check-integer-for-float-why = An integer literal never becomes a float implicitly; the decimal point makes the intent visible.
check-integer-for-float-fix = write `{ $literal }`

# E0212
check-out-of-range = `{ $literal }` does not fit in `{ $type }`
check-out-of-range-label = `{ $type }` holds values from { $min } to { $max }
check-out-of-range-why = A literal is checked against its type at compile time, so that it never overflows or wraps around silently.
check-out-of-range-fix = use a value in range, or a wider type

# E0213
check-unknown-type = unknown type `{ $name }`
check-unknown-type-label = not a type
check-unknown-type-why = A type is either built in (`i32`, `f64`, `bool`, `String`, `Array<T>`, `Option<T>`…) or a `struct` declared in the module.
check-unknown-type-fix = declare a `struct` with this name, or use a built-in type

# E0214
check-argument-count = `{ $name }` takes { $expected ->
    [one] 1 argument
   *[other] { $expected } arguments
}, found { $found }
check-argument-count-label = { $found ->
    [one] 1 argument given
   *[other] { $found } arguments given
}
check-argument-count-declared-label = { $expected ->
    [one] takes 1 argument
   *[other] takes { $expected } arguments
}
check-argument-count-why = Every parameter receives exactly one argument, in order: Peras has no optional parameters and no implicit `undefined`.
check-argument-count-fix = pass exactly { $expected ->
    [one] 1 argument
   *[other] { $expected } arguments
}

# E0215
check-unknown-member = `{ $type }` has no field or method `{ $member }`
check-unknown-member-label = not a member of `{ $type }`
check-unknown-member-why = The fields of a structure come from its declaration; built-in types have a fixed set of members, such as `length` or `toF64()`.
check-unknown-member-fix = check the name of the field or method

# E0216
check-type-argument-count = `{ $name }` takes { $expected } type arguments, found { $found }
check-type-argument-count-label = expects { $expected }
check-type-argument-count-why = `Array` and `Option` take exactly one type argument, as in `Array<f32>`; other types take none.
check-type-argument-count-fix = write `{ $name }` with { $expected } type arguments

# E0217
check-condition = condition of type `{ $found }`, expected `bool`
check-condition-label = this is `{ $found }`, not `bool`
check-condition-why = Peras has no truthiness: `0`, `""` and `None` are not false. A condition is a `bool`, so that its meaning is always explicit.
check-condition-fix-compare = compare explicitly: `{ $comparison }`
check-condition-fix = write a comparison that gives a `bool`

# E0218
check-return = { $kind ->
    [value] this function returns no value
   *[other] missing return value of type `{ $expected }`
}
check-return-label = { $kind ->
    [value] value returned here
   *[other] `return` without a value
}
check-return-declared-label = the function returns `{ $expected }`
check-return-why = A function returns exactly what its return type says: a value of that type on every `return`, or nothing when it is `void`.
check-return-fix = { $kind ->
    [value] remove the value, or declare a return type
   *[other] return a value of the declared type
}

# E0219
check-missing-return = `{ $name }` can end without returning a value
check-missing-return-label = the end of the function can be reached here
check-missing-return-declared-label = returns `{ $expected }`
check-missing-return-why = A function with a return type returns a value on every path; reaching the end of its body would return nothing.
check-missing-return-fix = end every path with a `return` of type `{ $expected }`

# E0220
check-missing-return-type = `{ $name }` has no return type
check-missing-return-type-label = { $reason ->
    [exported] an exported function declares its return type
   *[other] returns a value without declaring its type
}
check-missing-return-type-why = Public signatures are always written out, so a module's interface never depends on what its body happens to return; a function returning a value declares its type.
check-missing-return-type-fix = add the return type `{ $type }`

# E0221
check-operator = `{ $operator }` cannot be used on `{ $type }`
check-operator-label = this is `{ $type }`
check-operator-why = Each operator applies to specific types: arithmetic to numbers, `+` also to strings, `&`, `|`, `^`, `~`, `<<` and `>>` to integers, `!`, `&&` and `||` to `bool`, unary `-` to signed numbers, `[]` to arrays.
check-operator-fix = convert the value, or use another operation

## Diagnostic rendering

diag-error = error
diag-why = why
diag-fix = fix
diag-details = details
