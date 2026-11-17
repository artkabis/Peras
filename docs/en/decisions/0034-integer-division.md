# 0034: Integer division, as a user from JavaScript meets it

_French translation: [../../fr/decisions/0034-integer-division.md](../../fr/decisions/0034-integer-division.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks; options measured at `4dc37e9`; option d1 accepted by the owner on 2026-11-17. It answers finding F113, which [decision 0030](0030-numeric-literals.md) found: `const t = i / (count - 1); total += t;` sums 1 where JavaScript sums 2.5. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

## Context

What the compiler does today, read before writing this (finding M020):

- **`/` on two integers is an integer division**, truncated toward zero, as in Rust, Go, C and Swift (`BinOp::Div`, `crates/perasc-ir/src/lower.rs`; `$div` in a managed module). JavaScript has no integer: its `/` gives a fraction, and an integer division is written `Math.floor(a / b)` or `Math.trunc`.
- **A variable is never converted** (decision 0030): `total += t`, `t` an `i32` and `total` an `f64`, meets E0210 at the sum. Its fix, `t.toF64()`, converts the quotient already truncated: the program builds and gives 1 where JavaScript gives 2.5. Followed as offered, the compiler's fix gives a wrong number, silently.
- **Every diagnostic blocks the build**: there are no warnings (`crates/perasc-diag/src/lib.rs`). An option can tell, or refuse; it cannot only advise.
- **The programs of the repository divide integers on purpose**: a middle `(lo + hi) / 2`, a row `p / width`. 280 lines of the benchmarks, examples and documentation hold `/`; how many are integer divisions, and what an option that changes `/` would break, the prototypes count.

## Options

- **d0. As today.**
- **d1. The error tells where.** When E0210 meets an integer where a float is expected, and that integer is a division of integers, written there or held by a `const` that one initialised, the message shows the division, says it truncates, and its fix is at the division: `i.toF64() / (count - 1).toF64()`. Nothing else changes: `/` keeps its meaning.
- **d2. `/` as JavaScript's.** `/` on two integers gives an `f64`; an integer division is written `a.div(b)`.
- **d3. Neither, refused.** `/` on two integers is an error that offers both: `a.div(b)` for the integer quotient, `a.toF64() / b.toF64()` for the fraction.

## Protocol, registered before any prototype

Each option is prototyped behind `PERAS_PROTO_DIV`; the build without it keeps today's behaviour, d0.

1. **Ease of writing, and the results the fixes give**: five programs, written as a user writes them from JavaScript, then fixed by applying the fix each diagnostic offers, as given, until they build:
   - Q1, the gradient's steps summed, `const t = i / (count - 1); total += t;` (`bench/numbers/ease/p5t.peras`), a fraction meant;
   - Q2, an average returned as an `f64`, `return sum / count;`, a fraction meant;
   - Q3, the middle of a binary search, `Math.floor((lo + hi) / 2)` in JavaScript, an integer meant;
   - Q4, a pixel's row and column, `Math.floor(p / width)` and `p % width`, an integer meant;
   - Q5, a percentage shown as a whole number, `Math.round(done * 100 / total)`, a fraction rounded.

   For each: the errors met, with their codes; the lines changed; and **whether the program so fixed gives JavaScript's result**. A wrong result reached by following the compiler's own fix counts against the option, whatever else it gains.

2. **Non-regression** (the owner's rule of 2026-11-17): every program of the benchmarks and examples built under each option against today's compiler, `scripts/check-regression.mjs`: how many no longer build, how many change. An option under which a program of the repository no longer builds, or computes something else, regresses: it is excluded unless the decision migrates every such program, the migration counted in lines.
3. **Speed and load on the processor**: two loops of 10,000,000 turns, each option's version as fixed in point 1, owned modules on TLSF in Node, in one process, the order alternating, 21 rounds, the median of the ratios to d0 round by round and their range (finding M017), the CPU time over the wall time: W1, a row and a column per pixel, an integer meant; W2, the gradient's sum, a fraction meant.
4. **Bytes**: each version's module, raw and gzipped.
5. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-division.mjs` runs them; `--check` checks that every program builds as recorded and gives what it gave, and is run with every check of the project.

## Record

_Each part is recorded here as it lands._

### A precision of the protocol (2026-11-17)

Point 1 fixes a program by the edits its diagnostics offer, as given. The first version met two diagnostics that offer none, in every option: `Math` is not defined in an owned module (E0200, finding F116), and an integer has no `floor` (E0215, finding F117). Before any measure, two rules a user follows were added to the script, the same for every option: `Math.f(x)` is written `(x).f()`, the guide's way; `.floor()`, `.round()`, `.ceil()` or `.trunc()` on an integer goes, the integer being whole already. Without them, Q3 to Q5 were stuck in d0, d1 and d3, and only d2 built them.

### The measure (2026-11-17)

Run 1, at commit 4dc37e9, nothing else running; every table in [`bench/division/RESULTS.md`](../../../bench/division/RESULTS.md), with the machine; non-regression by `node scripts/check-regression.mjs 4dc37e9 PERAS_PROTO_DIV=<option>`.

| Quantity                                                                         | d0            | d1                     | d2                     | d3                     |
| -------------------------------------------------------------------------------- | ------------- | ---------------------- | ---------------------- | ---------------------- |
| Programs giving JavaScript's result once fixed by the compiler's own fixes, of 5 | 2             | **4**                  | 5                      | 2                      |
| Q1, the gradient (JavaScript: 2.5)                                               | 1             | 2.5                    | 2.5                    | 1                      |
| Q2, an average (7 / 3)                                                           | 2             | 2.333…                 | 2.333…                 | 2                      |
| Q5, a percentage rounded (67 %)                                                  | 66 %          | 66 %                   | 67 %                   | 66 %                   |
| Programs of the repository that no longer build                                  | -             | **0**                  | 13                     | 17                     |
| Programs that still build and compute something else                             | -             | **0**                  | 4                      | 0                      |
| Compile time, / d0                                                               | 1             | 1.003                  | 0.970                  | 0.908                  |
| W1, an integer meant, / d0                                                       | 1             | 1.000 (0.944 to 1.032) | 0.858 (0.794 to 3.501) | 1.001 (0.909 to 1.156) |
| W2, a fraction meant, / d0                                                       | 1 (and wrong) | 0.668 (0.452 to 0.923) | 0.666 (0.456 to 0.720) | 0.996 (and wrong)      |
| Bytes of W1 (gzip)                                                               | 135 (123)     | 135 (123)              | 148 (140)              | 135 (123)              |

What it says:

- **d0 leads a user to a wrong number**: following its own fix, `t.toF64()`, the gradient sums 1 and the average gives 2, silently.
- **d1 fixes that where it starts**: the fix is at the division, and both programs give JavaScript's result; nothing else changes, not a byte of any module of the repository. The one it misses, Q5, rounds a fraction that no diagnostic meets: `done * 100 / total` is an integer from the start, used as one.
- **d2 is JavaScript's meaning, at the cost of every program that divides integers**: 13 of the repository no longer build, and 4 build and compute something else, the worst kind, since nothing says it. Under the owner's rule, it regresses, unless the decision migrates every such program; and a user's own programs, outside the repository, would change without a word.
- **d3 refuses what d2 changes**: 17 programs no longer build, and its fix, `a.div(b)` first, leads Q1 and Q2 to the same wrong numbers as d0.
- **W2 is faster when it is right**: a float division costs less than an integer division whose divisor is checked; d1 and d2 run the fraction in two thirds of d0's time, d0 and d3 computing it wrong.

### Proposal

**d1**: the only option that is an improvement without a regression, under the owner's rule of 2026-11-17. It turns two of the three wrong results into right ones, changes no module, and costs nothing in time or bytes. What it leaves, Q5, and the two frictions met on the way, F116 (`Math`) and F117 (`floor` on an integer), are each a step of their own, measured as this one was. The owner decides.
