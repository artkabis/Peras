# 0041: A formatter

_French translation: [../../fr/decisions/0041-formatter.md](../../fr/decisions/0041-formatter.md)._

_Status: protocol registered on 2026-11-23, before any prototype and any measure, as the rule of the project asks; options measured at `878186e`; proposal: f2. It is the second step of phase 7 of the [roadmap](../reference/roadmap.md), tooling, in the order the owner chose on 2026-11-23, after the tests of [decision 0040](0040-tests.md): "a formatter, so that every Peras file looks the same". Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

## Context

What Peras does today, read before writing this (finding M020):

- **Nothing formats a Peras file.** `perasc` has no `fmt` command; Prettier, which formats the repository's TypeScript, Markdown and JSON at a width of 100 (`.prettierrc`), does not read `.peras` or `.apeiron`.
- **The lexer drops comments and whitespace** (`Lexed::tokens`, "comments and whitespace excluded"), but every token keeps its span: what lies between two tokens, blanks and comments, can be read back from the source.
- **The syntax tree keeps parentheses** (`ExprKind::Paren`) and a span on every node, but a literal keeps its value, not its text: `Int(u64)`, `Float(f64)`, so `1_000`, `0xff` or `1.50` are only found again through their span.
- **The repository holds 369 Peras files, 12,147 lines**, written by hand with two spaces of indentation; 63 lines are longer than 100 characters, one file holds a tab, 938 lines hold a comment. Some are refused on purpose (bugs of the benchmarks).

## Options

- **f0. As today**: nothing formats; every author lays out as they like.
- **f1. A formatter of tokens**: `perasc fmt` indents by the brackets open, puts the spaces between two tokens by a table of their kinds, keeps the line breaks the author wrote (a run of blank lines one blank line), and every comment where it was. A long line stays long.
- **f2. A printer with a width**: `perasc fmt` prints the file as Prettier prints TypeScript, at a width of 100: a list, a call or a chain that does not fit is broken one element a line, with a trailing comma; literals written as the source wrote them, comments attached to the token they precede or follow. The layout is the printer's, not the author's, but for blank lines between statements, kept to one. It is built on the tree the brackets of the tokens make, not on the syntax tree: a list, a call, a block and a chain are what it breaks, and they are found in the brackets; every token, its text and the comments around it are kept by construction, which the first gate asks, where a printer of the syntax tree would rebuild each from its node. A printer of the syntax tree is the way of Prettier and rustfmt; it is built too if f2 fails a gate or point 2 by what the brackets cannot see.
- **f3. A plugin of Prettier**: checked against the rules of the project (finding M004) and not built: Prettier's TypeScript printer cannot read Peras (`fn`, `inout`, `match`, `parallel for`, `test`), so a plugin would be a second parser and a printer written in JavaScript, which the rule "no dependency on another language than Rust" excludes for the compiler's own tools; a user would also need Prettier installed to format a file the compiler reads.

## Protocol, registered before any prototype

f1 and f2 are prototyped behind `PERAS_PROTO_FMT`, in `perasc fmt` (`--check` to say what would change, nothing written). Every figure names the build of the compiler measured, a release build (finding M029).

1. **Gates, before any ranking**: an option that fails one on a file of the corpus is reported as failing, with the file.
   - **Safe**: the formatted file lexes into the same tokens, comments included and in order, but for a trailing comma f2 adds or removes; and a file that builds gives the same module after formatting, byte for byte, built for production (`--allocator tlsf`, no `--tests`: a test build writes the line of a failure, which moves with the lines; checked before registering that a module, owned or managed, debug or production, keeps no line of its source); a file the checker refuses is refused with the same codes.
   - **Idempotent**: formatting a formatted file changes nothing.
   - **Refused input**: a file with a syntax error is left as it is, the error reported, never half formatted.
2. **Every file looks the same** (the roadmap's aim): six programs, each written twice by different layouts (one line, broken everywhere, mixed indentation, long calls and chains, comments between arguments); whether each option gives one text for both, and if not, how many lines differ. The lines longer than 100 left in the corpus after formatting.
3. **What it changes in the corpus**: the lines each option changes in the 369 files, and the files changed.
4. **Speed and load on the processor**: the corpus formatted with `--check`, both options alternating in one process (finding M017), 11 rounds, the median ratio round by round and its range, the CPU time over the wall time; and the largest file of the corpus alone, the time a save in an editor would wait.
5. **Memory and bytes**: the peak of memory formatting the largest file; the size the option adds to `perasc`, which every user downloads.
6. **Ease**: the notions a user learns (commands, settings), and the lines of the formatter's code, its tests apart, which the project maintains.
7. **An option not built** is reported as such, with why, never ranked by a guess (finding M001): f3 above.

`node scripts/bench-fmt.mjs` runs points 1 to 5; `--check` checks the gates on the corpus and is run with every check of the project.

## Record

### The measure (2026-11-23)

`node scripts/bench-fmt.mjs` at `878186e`, perasc built for release, the full table in [`bench/fmt/RESULTS.md`](../../../bench/fmt/RESULTS.md). The corpus is the 372 Peras files of the repository at that commit, the twelve programs of point 2 among them.

|                                                            | f0, as today            | f1, tokens                                                                                                | f2, a width                                                                                              |
| ---------------------------------------------------------- | ----------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Gates: refused for a syntax error                          |                         | 43, every one a file written wrong on purpose (tests of the lexer and the parser, bugs of the benchmarks) | the same 43                                                                                              |
| Gates: not idempotent, module changed, refusal changed     |                         | 0, 0, 0                                                                                                   | 0, 0, 0                                                                                                  |
| Gates: tokens and comments kept (`format_corpus`)          |                         | every file                                                                                                | every file                                                                                               |
| 2. Six programs written twice: one text                    |                         | none of six, 6 to 16 lines differ                                                                         | six of six                                                                                               |
| 2. Lines longer than 100 left                              | 66                      | 66                                                                                                        | 7: five template literals, never broken; a sum, which f2 does not break; a file refused, left as written |
| 3. What it changes in the corpus                           |                         | 16 files, 52 lines added, 53 removed                                                                      | 72 files, 485 added, 290 removed                                                                         |
| 4. The corpus with `--check`, median of 11 rounds          |                         | 28 ms                                                                                                     | 56 ms, 1.93 times f1's (1.29 to 2.78)                                                                    |
| 4. CPU time over wall time                                 |                         | 0.98                                                                                                      | 0.96                                                                                                     |
| 4. The largest file alone (`pond-bands.peras`)             |                         | 4 ms                                                                                                      | 5 ms: both at the resolution of the clock and the start of a process                                     |
| 5. Peak of memory, the largest file                        |                         | 4.8 MB                                                                                                    | 5.7 MB                                                                                                   |
| 5. Bytes added to `perasc` (4,576,144 without a formatter) |                         | 16,400                                                                                                    | 41,528                                                                                                   |
| 6. Notions a user learns                                   | none, and no file alike | `perasc fmt`, `--check`; the layout stays theirs to choose                                                | `perasc fmt`, `--check`; nothing to choose                                                               |
| 6. Lines of the formatter's code, its tests apart          |                         | 98, and 183 shared                                                                                        | 629, and 183 shared                                                                                      |

What the figures say:

- **Both are safe** on every file of the repository: the same tokens, the same comments, the same module built for production, the same refusals, a second pass that changes nothing.
- **Only f2 meets the roadmap's aim**, every file alike: the six programs written twice give one text each under f2 and none under f1, which keeps what the author broke. f1 changes 16 files of the corpus, spaces and indentation; f2 changes 72, mostly long parameter lists and calls broken one element a line, and declarations broken as Prettier breaks an interface.
- **f2 costs more where nobody waits**: twice f1's time on the whole corpus, 56 ms for 372 files; a file saved in an editor waits about 5 ms with either. 0.9 MB more memory, 25 KB more in the compiler, six times the code.
- **What f2 does not do yet**: it breaks no arithmetic (`a * 100000 + b * 10000 + …` stays on one line), and keeps no blank line between the fields of a structure. Neither changes a gate; both are a landing's work, measured by the same script.

Departures from the protocol: the peak of memory is read from Linux (`VmHWM` of `/proc/self/status`, asked by `PERAS_FMT_PEAK`), since this machine has no `/usr/bin/time`; the bytes are those of three builds of `perasc` with one option, the other or none, made apart from the repository at `878186e` and `2f5f460`. f2 was fixed four times before the measure, each found by reading what it did to the corpus (a block without `;` printed as a list, declarations joined, a template literal broken inside `${…}`, a call's single argument without its trailing comma); the measure is of the prototype as committed.

### Proposal

**f2**: the only option that makes every file look the same, which is what the roadmap asks of a formatter, as safe as f1 on every file of the repository, at a cost a user does not feel (5 ms a file saved) and 41 KB in the compiler. f1 is cheaper and leaves the author's layout alone, which is what it cannot help doing: two files of the same program stay two layouts. The owner decides. If f2 is chosen, the landing removes f1 and `PERAS_PROTO_FMT`, breaks arithmetic, keeps a blank line between fields, and gives the editor's language server (the next step of phase 7) its formatter.
