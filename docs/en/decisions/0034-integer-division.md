# 0034: Integer division, as a user from JavaScript meets it

_French translation: [../../fr/decisions/0034-integer-division.md](../../fr/decisions/0034-integer-division.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks. It answers finding F113, which [decision 0030](0030-numeric-literals.md) found: `const t = i / (count - 1); total += t;` sums 1 where JavaScript sums 2.5. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

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
