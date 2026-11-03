# Error codes

One page per code, named after it (`E0204.md`). Each page explains the cause, shows a failing example and its fix. Codes are permanent: a retired code keeps its page, marked as retired.

Codes `E0001` to `E0099` are reserved for the lexer, `E0100` to `E0199` for the parser.

| Code              | Error                                    | Stage  |
| ----------------- | ---------------------------------------- | ------ |
| [E0001](E0001.md) | Invalid character                        | lexer  |
| [E0002](E0002.md) | Unterminated string                      | lexer  |
| [E0003](E0003.md) | Unterminated block comment               | lexer  |
| [E0004](E0004.md) | Invalid escape sequence                  | lexer  |
| [E0005](E0005.md) | Malformed number                         | lexer  |
| [E0006](E0006.md) | Strict equality operator                 | lexer  |
| [E0007](E0007.md) | Bidirectional control character          | lexer  |
| [E0008](E0008.md) | Source file too large                    | lexer  |
| [E0100](E0100.md) | Expected expression                      | parser |
| [E0101](E0101.md) | Missing semicolon                        | parser |
| [E0102](E0102.md) | Expected name                            | parser |
| [E0103](E0103.md) | Unclosed delimiter                       | parser |
| [E0104](E0104.md) | Unexpected token                         | parser |
| [E0105](E0105.md) | Expected type                            | parser |
| [E0106](E0106.md) | Removed keyword                          | parser |
| [E0107](E0107.md) | Number too large                         | parser |
| [E0108](E0108.md) | Chained comparison                       | parser |
| [E0109](E0109.md) | Bitwise operator mixed with a comparison | parser |
| [E0110](E0110.md) | Expected declaration                     | parser |
| [E0111](E0111.md) | Nesting too deep                         | parser |
| [E0112](E0112.md) | Const without a value                    | parser |
| [E0113](E0113.md) | Assignment used as an expression         | parser |
| [E0114](E0114.md) | Invalid assignment target                | parser |
