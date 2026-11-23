# 0040: Tests written in Peras

_French translation: [../../fr/decisions/0040-tests.md](../../fr/decisions/0040-tests.md)._

_Status: protocol registered on 2026-11-23, before any prototype and any measure, as the rule of the project asks. It opens phase 7 of the [roadmap](../reference/roadmap.md), tooling, in the order the owner chose on 2026-11-23: tests, then a formatter, then a language server, then `create-peras`. The [specification](../spec.md#tooling-tests-and-security) foresees a `test "name" { … }` block run by Vitest through an adapter. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

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
