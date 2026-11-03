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

## Diagnostic rendering

diag-error = error
diag-why = why
diag-fix = fix
diag-details = details
