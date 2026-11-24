# Queries for the tools: the programs and the answers expected

For [decision 0042](../../docs/en/decisions/0042-queries.md). [`programs/`](programs) holds six programs of the repository's kinds; [`queries.json`](queries.json) holds fifty queries on them, `queries`, and ten on the same programs being typed, `typing`, each with its answer, written by hand before any prototype ran.

The contract of an answer:

- A position is `line:column`, both from 1, the column counted in characters, at the first character of a name.
- `hover`: the `name`, `what` it is (`local`, `parameter`, `function`, `structure`, `enumeration`, `variant`, `field`, `module variable`), and its `type` as Peras writes it; for a function, its signature as its declaration writes it.
- `definition`: the `file` and the position `at` of the name in its declaration.
- `references`: every position the declaration is read at, the declaration's own included, in order.
- `symbols`: the declarations at the top of a file, each `name`, `what` and `line`.
- `completion`: the names it offers `includes` those listed; others may come with them.
- `diagnostics`: the `codes` of the file's errors, in order.
- `format`: whether `perasc fmt` leaves the text `unchanged`.

A query of `typing` first replaces the line `replace.line` of its file by `replace.text`, which leaves a syntax error before the position, then asks.
