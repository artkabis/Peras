# Reading diagnostics

Peras has no warnings: every diagnostic is an error, and nothing is built until all are fixed. In exchange, each one says what is wrong, why the rule exists, and how to fix it.

## Anatomy of an error

```text
error[E0006]: `===` does not exist in Peras
  --> demo.peras:3:17
   |
 3 |     const zero = b === 0.0;
   |                    ^^^ write `==`
   |
   = why: Peras never converts types implicitly, so `==` and `!=` already compare strictly, like `===` and `!==` in JavaScript.
   = fix: replace `===` with `==`
   |
 3 -     const zero = b === 0.0;
 3 +     const zero = b == 0.0;
   |
   = details: perasc explain E0006
```

| Part           | Says                                                         |
| -------------- | ------------------------------------------------------------ |
| `error[E0006]` | a stable code: it never changes meaning, and is never reused |
| `-->`          | the file, line and column, clickable in most editors         |
| the excerpt    | the source, with what is wrong underlined                    |
| `why`          | why the rule exists: what would go wrong without it          |
| `fix`          | what to do, often as a diff to apply                         |
| `details`      | the command showing the full page about this error           |

## All errors at once

The compiler reports every error of a file in one pass, at most 50 per file, and never the consequence of another error: one mistake, one message.

## In French

`--lang fr` shows every message in French; without it, `perasc` follows your system's language.

```sh
perasc check app.peras --lang fr
```

## The full page: `perasc explain`

```sh
perasc explain E0006
```

shows the page of this error, with an example and its fix. The same pages are in this documentation: [error codes](../errors/index.md).

## For tools: `--json`

`perasc check --json` and `perasc build --json` print the same diagnostics as JSON, with every position and the fix's edits. The Vite plugin uses it to show errors in its overlay; editors and CI can use it too.

Next: [Peras and JavaScript](javascript.md).
