# Operators

## By precedence

From the tightest to the loosest. Operators on the same line have the same precedence and group from left to right.

| Operators         | Meaning                                         |
| ----------------- | ----------------------------------------------- |
| `-x` `!x` `~x`    | negation, logical not, bitwise not              |
| `*` `/` `%`       | multiplication, division, remainder             |
| `+` `-`           | addition, subtraction; `+` also joins strings   |
| `<<` `>>`         | shifts; `>>` keeps the sign of a signed integer |
| `<` `<=` `>` `>=` | comparison                                      |
| `==` `!=`         | equality                                        |
| `&`               | bitwise and                                     |
| `^`               | bitwise exclusive or                            |
| `\|`              | bitwise or                                      |
| `&&`              | logical and, short-circuit                      |
| `\|\|`            | logical or, short-circuit                       |

Both operands always have the same type: there is no implicit conversion ([E0210](../errors/E0210.md)). An operator that does not apply to its operands, such as `&&` on numbers, is refused ([E0221](../errors/E0221.md)).

## Rules that differ from JavaScript

| JavaScript                   | Peras                                                                        |
| ---------------------------- | ---------------------------------------------------------------------------- |
| `===`, `!==`                 | do not exist: `==` and `!=` are already strict ([E0006](../errors/E0006.md)) |
| `a < b < c`                  | refused: write `a < b && b < c` ([E0108](../errors/E0108.md))                |
| `a ? b : c`                  | does not exist: use `if`                                                     |
| `x++` inside an expression   | `++` and `--` are statements only                                            |
| `x = y` inside an expression | assignment is a statement only                                               |

## Assignment

`=`, `+=`, `-=`, `*=`, `/=`, `%=`, `++`, `--` are statements. Their target is a `let` variable, or a field of one ([E0205](../errors/E0205.md)).

## Borrow

`&x` passes a variable to an `inout` parameter, so that the change is visible at the call. <span class="status checked">checked</span>
