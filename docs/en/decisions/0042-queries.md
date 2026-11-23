# 0042: Queries for the tools, the language server's half in the open core

_French translation: [../../fr/decisions/0042-queries.md](../../fr/decisions/0042-queries.md)._

_Status: protocol registered on 2026-11-23, before any prototype and any measure, as the rule of the project asks; options measured at `ca5dd0e`; proposal: q2. It is the third step of phase 7 of the [roadmap](../reference/roadmap.md), tooling: "an editor extension and a language server". Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

## Context

[Decision 0006](0006-licensing.md) left open whether the language server is a tool of the open core or a proprietary one, "decided when it is built". The owner decided on 2026-11-23: **the language server and the VS Code extension are proprietary**, in a private repository (`peras-tools`), and **the open core publishes what they need**, a query interface of `perasc`, usable by any tool, so that no AGPL code goes into them. This decision is that interface.

What Peras does today, read before writing this (finding M020):

- **`perasc check --json`** gives the diagnostics of files on disk, nothing else: not of a text an editor holds unsaved, and no answer about a position.
- **The checker already knows what the queries ask** (`perasc_check::Analysis`): the type of every expression (`types`), the declaration each name read refers to (`names`), every declaration with its span and type (`bindings`), the method each call reaches (`method_calls`), the structures and their fields, the enumerations and their variants.
- **A file with a syntax error is not checked**: `check_file` returns the parser's diagnostics and stops, so a query in a file being typed, `xs.` before its method is written, has no analysis to answer from.
- **A check takes about 10 ms** on the largest file of the repository (`pond-bands.peras`, 355 lines) with the binary in the page cache, 0.34 s the first time.

## Options

Each answers the same queries, in JSON, from the same analysis:

- `diagnostics` of a text, saved or not;
- `hover` at a position: the name, what it is, its type;
- `definition` at a position: where the name is declared, in its file or in a file it imports;
- `references` at a position: every place the declaration is read, the declaration's own included, which a renaming needs;
- `symbols` of a file: its functions, structures, enumerations, module variables, with their spans;
- `completion` at a position: the fields and methods of the value before a `.`, or the names in scope;
- `format` of a text (decision 0041).

The options:

- **q0. As today**: `perasc check --json` on files saved; nothing else.
- **q1. One process a query**: `perasc query <kind> <file> [--at line:column]`, the text on standard input when it is not saved, one JSON answer on standard output.
- **q2. One process for an editor's session**: `perasc serve`, one JSON request a line on standard input and one answer a line on standard output; it keeps each file opened and its last analysis, checks a file again only when its text changes, and answers from what it keeps.
- **q3. The language server in the open core**: not an option since the owner's decision above; named so that it is not taken for forgotten.

## Protocol, registered before any prototype

q1 and q2 are prototyped behind `PERAS_PROTO_QUERY`; without it, `perasc` is as today, q0. Every figure names the build of `perasc` measured, a release build (finding M029).

1. **Answers, before any ranking**: `bench/query/`, six programs of the repository's kinds (owned and managed modules, an import, a structure and an enumeration, a generic function, closures) and fifty queries on them, each with its expected answer written by hand before any prototype runs. An option that gives one wrong answer is reported as failing, with the query. Both options answer from the same code, so they give the same answers; the gate is that the code is right.
2. **A file being typed**: ten queries on the same programs with a syntax error before the position (an unfinished line, a `.` with nothing after it); for each option, which queries still answer, and what.
3. **Speed and load on the processor**, from a client in Node, one process alternating the options (finding M017), 11 rounds:
   - each kind of query on three files: 50 lines, the largest of the repository (355), and one of 5,000 lines made by repeating its functions; the median time from the request sent to the answer read, and its 95th percentile;
   - a scenario of typing: 100 edits of a file of 355 lines, each followed by `diagnostics` and `hover`, as an editor asks; its total time, and the CPU time over the wall time.
4. **Memory and bytes**: the peak of memory of q2's process with every file of the repository opened, and of one q1 process on the largest file; the bytes each option adds to `perasc`.
5. **Ease**: the client that runs the scenario, written for each option: its lines, the notions it needs (a process, a protocol, a state to keep), the errors met writing it.
6. **An option not built** is reported as such, with why, never ranked by a guess (finding M001): q3 above.

`node scripts/bench-query.mjs` runs points 1 to 4; `--check` checks the answers of points 1 and 2 and runs with every check of the project.

## Record

### The measure (2026-11-23)

`node scripts/bench-query.mjs` at `ca5dd0e`, perasc built for release, the full tables in [`bench/query/RESULTS.md`](../../../bench/query/RESULTS.md).

|                                                                  | q1, a process a query                                        | q2, a process for a session                                                                              |
| ---------------------------------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| 1. Right answers, of 50                                          | 50                                                           | 50                                                                                                       |
| 2. A file being typed, of 10                                     | 10                                                           | 10                                                                                                       |
| 3. A query on 53 lines, median                                   | 3.0 to 3.7 ms                                                | 0.1 to 0.4 ms                                                                                            |
| 3. A query on 356 lines, median (95th percentile)                | 7.1 to 11.5 ms (up to 22.3)                                  | 0.1 to 2.2 ms (up to 4.4)                                                                                |
| 3. A query on 5,340 lines, median (95th percentile)              | 72 to 139 ms (up to 165)                                     | 0.1 to 27 ms; a text first checked 63 ms                                                                 |
| 3. 100 edits, each followed by `diagnostics` and `hover`, median | 2,046 ms (1,915 to 2,087)                                    | 862 ms (819 to 954), 2.4 times faster                                                                    |
| 3. CPU time over wall time, the same scenario                    | 1.00                                                         | 0.97                                                                                                     |
| 4. Peak of memory                                                | 5.5 MB, one process on 356 lines                             | 20.7 MB, the 378 Peras files of the repository opened                                                    |
| 4. Bytes added to `perasc` (4,621,432 without queries)           | 124,976                                                      | 166,584                                                                                                  |
| 5. The client of the scenario                                    | 18 lines; a process a query, the text on its input; no state | 32 lines; one process, requests numbered and their answers matched, the files opened told of each change |
| 5. Errors met writing the client                                 | none                                                         | the types of the answers matched by number, which the linter asked for                                   |

What the figures say:

- **Both are right**: the fifty answers written by hand, and the ten on a file being typed, from one engine; a file with a syntax error is analysed from the tree the parser recovered, which answers a hover, a definition or the members after a `.` on the line being typed. Point 2 does not tell the options apart: they answer from the same code.
- **q2 answers what an editor asks while a person types, q1 does not**: an answer in a tenth of a millisecond from what q2 keeps, against 7 to 11 ms for q1 on the largest file of the repository, and 72 to 139 ms on 5,340 lines, past the 50 ms an editor's hover is expected under. On the scenario of typing, q2 takes 862 ms where q1 takes 2,046.
- **q2 costs memory and bytes**: 20.7 MB with every file of the repository opened, where a q1 process takes 5.5 MB and lasts a query; 42 KB more in `perasc`, the reading of JSON requests; and a client that keeps a process and numbers its requests, 14 lines more.

Departures from the protocol: one position of `queries.json` written before any prototype was wrong, on the word `mean` of a template's text rather than on `s.mean` (`report.peras`, line 5, column 30 for 39); found by the first run of q1, it was corrected in the commit of the prototypes and its expected answer left as written. The CPU time of q1's processes is read from Linux as the time of the children Node waited for (`cutime` and `cstime` of `/proc/self/stat`), the peak of memory of a q1 process sampled every millisecond while it runs; the bytes are those of three builds of `perasc`, with one option, the other or none, made apart from the repository at `ca5dd0e` and `628ce00`.

### Proposal

**q2**, `perasc serve`: the language server of `peras-tools` keeps one process for an editor's session and gets its answers under a millisecond, where q1 spends a check on each and passes 50 ms on a large file. Its costs are those of a process that stays: 20 MB with a whole repository open, 42 KB in `perasc`. q1 is simpler for a script that asks once, which `perasc check --json` and `perasc fmt` already serve. The owner decides. If q2 is chosen, the landing removes q1 and `PERAS_PROTO_QUERY`, writes the protocol of `perasc serve` in the reference, a page the private repository builds on, and its answers to a file that imports another follow the changes to that other.
