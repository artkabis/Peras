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

## Diagnostic rendering

diag-error = error
diag-why = why
diag-fix = fix
diag-details = details
