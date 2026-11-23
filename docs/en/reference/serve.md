# `perasc serve`: queries for the tools

`perasc serve` answers the questions an editor asks while a person types: errors, the type of a name, where it is declared, where it is read, what can be written after a `.` ([decision 0042](../decisions/0042-queries.md)). One process lasts an editor's session; it keeps each file opened with what the checker learned of it, and answers from that in a tenth of a millisecond on the largest file of the repository. It is what a language server, an extension or any other tool uses Peras through, without its code ([decision 0006](../decisions/0006-licensing.md)).

## The protocol

One request a line on standard input, one answer a line on standard output, both JSON. A request has an `id`, which its answer carries back, and a `method`:

| `method`         | Fields                                         | Does                                                                                             | Answers          |
| ---------------- | ---------------------------------------------- | ------------------------------------------------------------------------------------------------ | ---------------- |
| `open`, `change` | `file`, `text` (when absent, the file on disk) | holds `text` as the file, saved or not; analyses it again, and every file opened that imports it | `{ "ok": true }` |
| `close`          | `file`                                         | reads the file on disk again; analyses again the files opened that import it                     | `{ "ok": true }` |
| `query`          | `file`, `kind`, `at`                           | answers from what it holds; a file not opened is read from disk                                  | below            |
| `exit`           |                                                | ends the process                                                                                 | nothing          |

```json
{"id": 1, "method": "change", "file": "src/stats.peras", "text": "…"}
{"id": 1, "result": {"ok": true}}
{"id": 2, "method": "query", "file": "src/stats.peras", "kind": "hover", "at": "17:16"}
{"id": 2, "result": {"name": "total", "what": "function", "type": "fn total(xs: Array<f64>): f64"}}
```

A position `at` is `line:column`, both from 1, the column counted in characters. A query that finds nothing at its position answers `null`.

## The queries

| `kind`        | Answers                                                                                                                                                                                                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `diagnostics` | `{ "codes": [...], "items": [...] }`: the codes of the file's errors, in order, as `perasc check` finds them, and each error as `perasc check --json` writes it, its message, ranges, cause and fix, in the language of `PERAS_LANG`                                     |
| `hover`       | `{ "name", "what", "type" }`: `what` is `local`, `parameter`, `function`, `structure`, `enumeration`, `variant`, `field` or `module variable`; `type` is written as Peras writes it (`number` in a managed module), a function's as its declaration writes its signature |
| `definition`  | `{ "file", "at" }`: the declaration of the name, in its file or in the file it is imported from                                                                                                                                                                          |
| `references`  | `{ "at": [...] }`: every position the declaration is read at, its own included, in order: what a renaming changes                                                                                                                                                        |
| `symbols`     | `{ "symbols": [{ "name", "what", "line" }] }`: the functions, structures, enumerations and module variables of the file                                                                                                                                                  |
| `completion`  | `{ "names": [...] }`: after a `.`, the fields and methods of the value before it; elsewhere, the names in scope                                                                                                                                                          |
| `format`      | `{ "unchanged", "text" }`: the file as `perasc fmt` prints it ([decision 0041](../decisions/0041-formatter.md)), or `{ "refused": true }` for a file with a syntax error                                                                                                 |

A file with a syntax error is analysed from the tree the parser recovers: on a line being typed, `xs.` before its method is written, a hover, a definition or a completion still answers.
