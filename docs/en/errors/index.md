# Error codes

One page per code, named after it (`E0204.md`). Each page explains the cause, shows a failing example and its fix. Codes are permanent: a retired code keeps its page, marked as retired.

Codes `E0001` to `E0099` are reserved for the lexer.

| Code              | Error                           | Stage |
| ----------------- | ------------------------------- | ----- |
| [E0001](E0001.md) | Invalid character               | lexer |
| [E0002](E0002.md) | Unterminated string             | lexer |
| [E0003](E0003.md) | Unterminated block comment      | lexer |
| [E0004](E0004.md) | Invalid escape sequence         | lexer |
| [E0005](E0005.md) | Malformed number                | lexer |
| [E0006](E0006.md) | Strict equality operator        | lexer |
| [E0007](E0007.md) | Bidirectional control character | lexer |
| [E0008](E0008.md) | Source file too large           | lexer |
