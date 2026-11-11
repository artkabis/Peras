# Error codes

One page per code, named after it (`E0204.md`). Each page explains the cause, shows a failing example and its fix. Codes are permanent: a retired code keeps its page, marked as retired.

Codes `E0001` to `E0099` are reserved for the lexer, `E0100` to `E0199` for the parser, `E0200` to `E0299` for names and types, `E0900` to `E0999` for limits of code generation.

| Code              | Error                                                           | Stage           |
| ----------------- | --------------------------------------------------------------- | --------------- |
| [E0001](E0001.md) | Invalid character                                               | lexer           |
| [E0002](E0002.md) | Unterminated string                                             | lexer           |
| [E0003](E0003.md) | Unterminated block comment                                      | lexer           |
| [E0004](E0004.md) | Invalid escape sequence                                         | lexer           |
| [E0005](E0005.md) | Malformed number                                                | lexer           |
| [E0006](E0006.md) | Strict equality operator                                        | lexer           |
| [E0007](E0007.md) | Bidirectional control character                                 | lexer           |
| [E0008](E0008.md) | Source file too large                                           | lexer           |
| [E0100](E0100.md) | Expected expression                                             | parser          |
| [E0101](E0101.md) | Missing semicolon                                               | parser          |
| [E0102](E0102.md) | Expected name                                                   | parser          |
| [E0103](E0103.md) | Unclosed delimiter                                              | parser          |
| [E0104](E0104.md) | Unexpected token                                                | parser          |
| [E0105](E0105.md) | Expected type                                                   | parser          |
| [E0106](E0106.md) | Removed keyword                                                 | parser          |
| [E0107](E0107.md) | Number too large                                                | parser          |
| [E0108](E0108.md) | Chained comparison                                              | parser          |
| [E0109](E0109.md) | Bitwise operator mixed with a comparison                        | parser          |
| [E0110](E0110.md) | Expected declaration                                            | parser          |
| [E0111](E0111.md) | Nesting too deep                                                | parser          |
| [E0112](E0112.md) | Const without a value                                           | parser          |
| [E0113](E0113.md) | Assignment used as an expression                                | parser          |
| [E0114](E0114.md) | Invalid assignment target                                       | parser          |
| [E0115](E0115.md) | Condition without parentheses                                   | parser          |
| [E0116](E0116.md) | `for…in` loop                                                   | parser          |
| [E0117](E0117.md) | `break` or `continue` outside a loop                            | parser          |
| [E0118](E0118.md) | Optional marker `?:`                                            | parser          |
| [E0119](E0119.md) | Empty body after `;`                                            | parser          |
| [E0200](E0200.md) | Undefined name                                                  | names and types |
| [E0201](E0201.md) | Shadowing                                                       | names and types |
| [E0202](E0202.md) | Duplicate declaration                                           | names and types |
| [E0203](E0203.md) | Unused variable or parameter                                    | names and types |
| [E0204](E0204.md) | Value used after a move                                         | ownership       |
| [E0205](E0205.md) | Cannot modify                                                   | names and types |
| [E0206](E0206.md) | `&` and `inout` arguments                                       | ownership       |
| [E0207](E0207.md) | Aliasing with an `inout` argument                               | ownership       |
| [E0209](E0209.md) | Moving what is only borrowed                                    | ownership       |
| [E0210](E0210.md) | Mismatched types                                                | names and types |
| [E0211](E0211.md) | Integer literal where a float is expected                       | names and types |
| [E0212](E0212.md) | Literal out of range                                            | names and types |
| [E0213](E0213.md) | Unknown type                                                    | names and types |
| [E0214](E0214.md) | Wrong number of arguments                                       | names and types |
| [E0215](E0215.md) | Unknown field or method                                         | names and types |
| [E0216](E0216.md) | Wrong number of type arguments                                  | names and types |
| [E0217](E0217.md) | Condition is not a bool                                         | names and types |
| [E0218](E0218.md) | Return value mismatch                                           | names and types |
| [E0219](E0219.md) | Missing return                                                  | names and types |
| [E0220](E0220.md) | Missing return type                                             | names and types |
| [E0221](E0221.md) | Invalid operator                                                | names and types |
| [E0222](E0222.md) | Match does not cover every value                                | names and types |
| [E0223](E0223.md) | Unknown variant                                                 | names and types |
| [E0224](E0224.md) | Unreachable arm                                                 | names and types |
| [E0225](E0225.md) | `?` in a function that cannot return early                      | names and types |
| [E0226](E0226.md) | Unknown constraint                                              | names and types |
| [E0227](E0227.md) | Type argument that does not meet a constraint                   | names and types |
| [E0228](E0228.md) | Operation a type parameter may not support                      | names and types |
| [E0229](E0229.md) | Type argument that nothing determines                           | names and types |
| [E0230](E0230.md) | Exported generic function                                       | names and types |
| [E0231](E0231.md) | Generic code that needs endless copies                          | names and types |
| [E0232](E0232.md) | Type alias that contains itself                                 | names and types |
| [E0233](E0233.md) | Module constant computed at run time                            | names and types |
| [E0234](E0234.md) | Methods for a type that cannot have them                        | names and types |
| [E0235](E0235.md) | Implementation that does not match its interface                | names and types |
| [E0236](E0236.md) | Method that does not match its interface                        | names and types |
| [E0237](E0237.md) | Interface used as a type                                        | names and types |
| [E0238](E0238.md) | Arrow function where none can go                                | names and types |
| [E0248](E0248.md) | A promise never awaited                                         | names and types |
| [E0247](E0247.md) | An extern declaration that cannot be kept                       | names and types |
| [E0246](E0246.md) | A property that can only be read                                | names and types |
| [E0245](E0245.md) | A resource without an owner                                     | names and types |
| [E0244](E0244.md) | An import that cannot be made                                   | names and types |
| [E0243](E0243.md) | Something only an owned module has                              | names and types |
| [E0242](E0242.md) | Something only a managed module has                             | names and types |
| [E0241](E0241.md) | A loop that cannot run in parallel                              | names and types |
| [E0240](E0240.md) | A map's key or a set's element of a type that cannot be one yet | names and types |
| [E0239](E0239.md) | A value a template literal cannot write yet                     | names and types |
| [E0900](E0900.md) | Not supported by code generation yet                            | code generation |
