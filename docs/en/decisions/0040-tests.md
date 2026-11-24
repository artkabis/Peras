# 0040: Tests written in Peras

_French translation: [../../fr/decisions/0040-tests.md](../../fr/decisions/0040-tests.md)._

_Status: protocol registered on 2026-11-23, before any prototype and any measure, as the rule of the project asks; options measured at `d47168a`; v2 chosen by the owner on 2026-11-23. It opens phase 7 of the [roadmap](../reference/roadmap.md), tooling, in the order the owner chose on 2026-11-23: tests, then a formatter, then a language server, then `create-peras`. The [specification](../spec.md#tooling-tests-and-security) foresees a `test "name" { … }` block run by Vitest through an adapter. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

## Context

What Peras does today, read before writing this (finding M020):

- **No test is written in Peras.** The language has no `test` block; `packages/vitest-peras` is a `package.json` and nothing else.
- **A test is written in TypeScript**: Vitest uses the Vite plugin, so a test file imports a `.peras` or `.apeiron` module and calls its exported functions. A function that is not exported cannot be tested; a structure crosses through the loader's conversions; a failure points at the TypeScript, not at the Peras.
- **No example of the repository has tests of its own**; the compiler's tests are in Rust and in TypeScript.

## Options

- **v0. As today**: tests in TypeScript, calling exported functions.
- **v1. `test` blocks in the file**: `test "name" { … }` at the top of a `.peras` or `.apeiron` file, with `assert(cond)` and `assertEqual(a, b)`; a block reaches every function of its file, exported or not; it is compiled for tests only, and `vitest-peras` shows each block as a test of Vitest.
- **v2. v1, with comparisons that explain**: `expect(a).toBe(b)`, `expect(a).toEqual(b)`, `expect(r).toBeErr()` as Vitest writes them; a failure says what was expected, what was got, and where in the `.peras` file.

## Protocol, registered before any prototype

Each option is prototyped behind `PERAS_PROTO_TEST`; the build without it keeps today's behaviour, v0. Every module measured is named with its build (finding M029).

1. **Ease of writing**: three modules and their tests, written for each option, counted as decision 0028 counts them (lines, notions to learn, errors met while writing):
   - **M1**, an owned module, `parsePort(text): Result<u16, String>` and a private helper, ten cases;
   - **M2**, a managed module, a countdown text from a `Duration`, six cases;
   - **M3**, an owned module of arrays and structures, a deduplication that keeps the first of each key, six cases.
2. **Bugs found**: for each module, three versions with one bug planted (an off-by-one, a wrong branch, a case forgotten); for each option, whether the suite fails, and what its failure tells: the test that failed, the values, the line of the Peras source.
3. **Cost**: a production build of a module with its tests gives, byte for byte, the module without them; the test build's size is reported.
4. **Both modes**: every test passes in an owned and in a managed module where the module allows it; an end-to-end test of `vitest-peras`.
5. **Speed**: the time from the command to the result of each suite, Vitest's own run in one process, 11 rounds, the median and its range; it is reported, not ranked below a second.
6. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-tests.mjs` runs points 1, 2, 3 and 5; `--check` checks that every option's suites pass on the modules and fail on the planted bugs, and is run with every check of the project.

## Record

_Each part is recorded here as it lands._

### The measure (2026-11-23)

Two runs at commit d47168a, one after the other, nothing else running; every table in [`bench/tests/RESULTS.md`](../../../bench/tests/RESULTS.md), with the machine. The two runs agree on points 1 to 3 to the character.

| Measure                                   | v0, today: TypeScript                                                                 | v1, `assert`, `assertEqual`                               | v2, `expect(a).toBe(b)`                                                                                  |
| ----------------------------------------- | ------------------------------------------------------------------------------------- | --------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Lines of the tests, M1, M2, M3            | 14, 10, 11                                                                            | 16, 8, 8                                                  | 16, 8, 8                                                                                                 |
| A private function tested                 | cannot be: `digit` is not exported                                                    | yes                                                       | yes                                                                                                      |
| Planted bugs found, of 9                  | 9                                                                                     | 9                                                         | 9                                                                                                        |
| What a failure says                       | the values as JavaScript sees them, `{ kind: 'Err', … }`, at a line of the TypeScript | the line of the Peras source and the assertion as written | the line of the Peras source and both values, `expected 80, got 0`                                       |
| Errors met while writing                  | none                                                                                  | none                                                      | one: an array cannot be written in a failure's text (E0239), so M3 compares with `==` and expects `true` |
| A production build with the tests         | —                                                                                     | the module's bytes, byte for byte                         | the module's bytes, byte for byte                                                                        |
| From the command to the result of a suite | 0.47 to 0.54 s                                                                        | 0.48 to 0.51 s                                            | 0.45 to 0.50 s                                                                                           |

- **Every option finds every bug.** What differs is what the failure tells. v0 says the values, but as the loader gives them to JavaScript, and at a line of the TypeScript; v1 says where in the Peras and what was asserted; v2 says where and what was got. For M3's arrays, v2 says only `expected true, got false`, because a failure's text cannot hold an array.
- **v1 and v2 reach what v0 cannot**: a function that is not exported, `digit`, tested in its own file.
- **The tests cost nothing to a production build**: each module with its tests builds into the module's bytes. A test build is larger: M1 6,518 bytes under v1 and 7,807 under v2, against 4,660.
- **Notions**: v0 asks for Vitest's API and for how each Peras value looks in JavaScript (`{ kind, values }` for a result); v1 adds `test`, `assert` and `assertEqual`; v2 adds `test`, `expect` and four matchers, `toBe`, `toEqual`, `toBeOk`, `toBeErr`, the names Vitest uses.
- **Non-regression**, `scripts/check-regression.mjs 343e454` under v1 and v2: 209 modules compared, none changed; compile time 0.981 and 1.021.
- **The prototypes rewrite the source's tokens** before it is parsed, keeping every line: a test block of an imported file is not run, and a failure's text holds only what a template literal can write. A landing parses the block as the language's own.

### Proposal

**v2**, `expect(a).toBe(b)`: every bug found, a private function tested, the failure saying where in the Peras and both values, with the names a person who tests with Vitest already knows, at no cost to a production build. Its gap is the one M3 met: a value a template literal cannot write, an array or a structure, cannot be shown in a failure; a landing writes any value in a failure's text. v1 finds the same bugs with fewer words to learn, and says less. The owner decides.

### Decision and landing (2026-11-23)

The owner chose **v2**. Every parse blanks out the `test` blocks of a source, byte for byte and line for line (`perasc_syntax::testing::strip`), so that a build, `perasc check` and every diagnostic read the file as if its tests were not there, at the same lines; `perasc build --tests` makes each block an exported function and writes the names of the tests next to the module, `<stem>.tests.json`. `vitest-peras` calls it, and its option for v1 is gone with v1, whose figures are above and whose code is in git at d47168a. The [guide](../guide/tests.md) shows a module and its tests.

The gap the proposal named is not closed by this landing: a failure cannot show an array or a structure, which needs a value of any type written as text, a template literal's need too; it is finding F227, open. `scripts/bench-tests.mjs` measures the build of today against v0 into `bench/tests/CURRENT.md`, and its `--check` runs in the CI; [`RESULTS.md`](../../../bench/tests/RESULTS.md) keeps the options as measured.

Non-regression, `scripts/check-regression.mjs e05338a`: 209 modules compared, none larger, smaller or changed; compile time 0.962 (0.897 to 1.051); the compiler 72 bytes larger, the search for `test` that every parse now makes.
