# 0045: Narrowing an option through `&&`, `||` and `!`

_French translation: [../../fr/decisions/0045-narrowing.md](../../fr/decisions/0045-narrowing.md)._

_Status: protocol registered on 2026-11-24, before any prototype and any measure, as the rule of the project asks; options measured at `e1e5ed8`; proposal: n0 stays by the criteria registered; the owner chose n2 and decided M038; landed. Asked by the owner on 2026-11-24, after [decision 0044](0044-pools.md) met finding F118; it comes before an amendment of 0044 that measures a pool lowered by the compiler itself. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression; if none is, the decision says so and nothing lands._

## Context

What the checker does today, read before writing this (finding M020):

- **An option is narrowed by two forms of `if`** (decision 0028, option 2e; finding F115), in `crates/perasc-check/src/stmt.rs`: in the block of `if (x.isSome())`, `x` is its value (`some_test`); after `if (x.isNone()) { … }` whose block leaves, `x` is its value until the end of the enclosing block (`narrowing`). `x` is a `const`, a parameter not `inout`, or a field of one, `child.parent`; a `let` is not narrowed.
- **A narrowed read is marked, not rewritten**: the checker records each name or field read as its value (`Analysis::narrowed`), and the owned lowering reads the value's part of the option there (`crates/perasc-ir/src/lower/values.rs`, `lower.rs`); in a managed module an option is its value or `undefined`, and a narrowed read is the same JavaScript.
- **`&&` and `||` type each side as a `bool`, and narrow nothing** (`binary` in `crates/perasc-check/src/expr.rs`); `!` neither, nor an `else`.
- **What it costs, finding F118**: written as in TypeScript, the five programs of this protocol meet nine errors today (`perasc check`, at `b16e79c`): `E0210` and `E0221`, an option where its value is meant. The fix is to split the condition into nested `if`, or to turn it around.

What others do: TypeScript narrows by control flow, through `&&`, `||`, `!`, `else`, early returns and `while`; Kotlin's smart casts through `&&` and `||`; Swift asks for `if let a, let b` and `guard let`; Rust for `if let Some(p) = p && …`, let chains, stable since 2024.

## Options

- **n0. As today**: the two forms of `if`. The reference.
- **n1. Chains**: a condition that is a chain of `&&` narrows each option tested `x.isSome()` by one of its operands, in the operands after it and in the block it guards; a chain of `||` narrows each option tested `x.isNone()`, in the operands after it, and after an `if` with no `else` whose block leaves. Only those two shapes, read off the tree.
- **n2. Facts**, as TypeScript: each condition proves facts when it is true and when it is false; `x.isSome()` proves `x` when true, `x.isNone()` when false, `!c` exchanges the two, `a && b` is true when both are and false when the facts of both falses agree, `a || b` the reverse. The right operand of `&&` is checked with the left's true facts, of `||` with its false facts; the block of an `if` with its true facts, its `else` with its false facts; after an `if` whose block leaves, its false facts, and after one whose `else` leaves, its true facts; the body of a `while` with its true facts.

Both keep what is narrowed today, `const`, parameters not `inout` and their fields, and the same mark for the lowering: no new lowering, no new runtime.

## Protocol, registered before any prototype

n1 and n2 are prototyped behind `PERAS_PROTO_NARROW=n1|n2`; without it, the compiler is as today, n0. Every module measured is built for production, `--allocator tlsf` (finding M029), by a release build of `perasc`.

### Before any measure: the definitions tested

The five programs are written as a user would from TypeScript (`bench/narrow/*.peras`), then as n0 asks (`bench/narrow/n0/*.peras`), the condition split or turned around and nothing else changed; each n0 form builds, gives the same twice on one instance, and the same as its managed module (tested on 2026-11-24 at `b16e79c`: N1 343772, N2 -372340, N3 40781981, N4 339126400, N5 3012980, each for 100,000).

### The programs

- **N1, the parents** (`links.peras`): a walk up the parents ended by `if (p.isNone() || !alive[p]) { break; }`.
- **N2, the pull** (`pull.peras`): `if (p.isSome() && alive[p]) { sum += xs[p] - xs[i]; }`.
- **N3, both there** (`both.peras`): `if (a.isNone() || b.isNone()) { return -1; } return a + b;`.
- **N4, the tally** (`tally.peras`): `if (x.isNone()) { … } else { total += x; }`, and `if (!x.isSome()) { continue; }`.
- **N5, a field** (`weigh.peras`): `if (c.parent.isSome() && alive[c.parent]) { … }`.

### The quantities

1. **Speed**: the module built from the TypeScript form under n1 and under n2 against the module built from the n0 form under n0, in one process, the order alternating round by round, 21 rounds, the median of the ratios and their range (finding M017); each called twice on one instance and its second answer checked (finding M035). A program an option refuses is not timed under it.
2. **CPU load**: the time a core is kept busy, `process.cpuUsage`, over the same rounds.
3. **Memory and bytes**: each module's memory at the end and its size, raw and gzip, a module identical to the reference's counted at 1.000 (finding M037); `check-regression` against the commit of this protocol, the compiler's time and its size.
4. **Ease of writing**: the errors the compiler reports for each TypeScript form under each option, counted by `perasc check --json`, and the lines, blank lines and comments apart, of the form that builds: the TypeScript form where it builds, the n0 form otherwise.
5. **Soundness**: programs that must stay refused under every option, each a test of `crates/perasc-check`: `x` read in the block of `if (x.isSome() || c)`, after `if (x.isNone() && c) { return; }`, in the `else` of `if (x.isSome() && c)`, in the right operand of `x.isSome() || …`; a `let` or an `inout` tested in a chain; the options that n2 accepts, `if (!(x.isSome() && c)) { return; }` among them, checked to give what the n0 form gives.

### Correctness, before any figure counts

Each option passes `cargo test --workspace` with its variable set, the end-to-end tests, and the differential test with `PERAS_DIFFERENTIAL_SEEDS=2000`, its generator extended with conditions that test an option through `&&`, `||` and `!` and read it where the option narrows; then once broken on purpose, the right operand of `||` given the left's true facts, to see a test fail (findings M015, M024).

### Criteria, registered now

- An option that changes a module of a program that does not use it, fails `check-regression`, or fails a check of correctness or of soundness above, is out.
- An option is an improvement only if it reports **fewer errors** than n0 on N1 to N5, its forms that build in **no more lines** than n0's, each module at most **1.05** times the n0 module's time, its range taken into account, its memory at most **1.10 times or one page of 64 KiB more** and its bytes no more than the n0 module's, and the compiler at most 1.05 times its time.
- Among the improvements, the one with fewer errors left on N1 to N5; then the one that grows the compiler less, in bytes.
- If no option is one, n0 stays, the prototypes are removed, their figures kept here, and finding F118 says why.

## Record

### The prototypes (2026-11-24)

At `e1e5ed8`, in the checker alone (`crates/perasc-check/src/stmt.rs`, `expr.rs`): `facts` gives what a condition proves when true and when false, under n0 as before, under n1 through chains, under n2 through `!`, `&&` and `||`; `binary` checks the right operand of `&&` or `||` with what the left one proves; `if`, `else`, `while` and what follows an `if` that leaves narrow with them. The lowering and the runtime are unchanged: a narrowed read keeps its mark. Each option passes `cargo test --workspace`, the end-to-end tests and the differential test with `PERAS_DIFFERENTIAL_SEEDS=2000`, its generator writing two statements more that narrow through `&&` and `||`, and under n2 through `!` and `else`; given to n1 once, the statement of n2 failed the differential test, as a program refused must. The soundness test, `narrowing_through_conditions_is_sound`, refuses eleven programs under every option and accepts nine where each option narrows; with the right operand of `||` given the left one's true facts, it fails under n1 and under n2. Writing it found one of its own cases wrong: `if (x.isSome() || c) { … } else { return 0; }` does not prove `x` after it, since `c` alone makes the condition true; n2 refused it, and the case went to those refused.

### The measure (2026-11-24)

At `e1e5ed8`, `node scripts/bench-narrow.mjs`, 21 rounds ([results](../../../bench/narrow/RESULTS.md)); then `check-regression` against `cbb757c`. **Departure**: `check-regression` was first run with `PERAS_PROTO_NARROW` in its environment, which it unsets on purpose, an option being given as an argument (`PERAS_PROTO_NARROW=n2`); that run compared n0 with n0, is kept in the scratchpad of the session, and was run again as the script asks (finding M039).

|     | errors met, N1 to N5 | lines, N1 to N5    | time / n0, median per program (range) | memory    | bytes, raw, against n0's      |
| --- | -------------------- | ------------------ | ------------------------------------- | --------- | ----------------------------- |
| n0  | 8                    | 31, 33, 24, 37, 34 | 1.000                                 |           |                               |
| n1  | 2, N4 refused        | 28, 31, 21, 37, 32 | 0.931 to 0.979 (0.681 to 1.303)       | identical | +9, +18, 0, (N4 refused), +10 |
| n2  | **0**                | 28, 31, 21, 37, 32 | 0.906 to 0.993 (0.427 to 1.495)       | identical | +9, +18, 0, +1, +10           |

`check-regression`: of 270 modules, none larger, none smaller, none changed, under n1 and under n2; the TypeScript forms build now only, four under n1, five under n2; the compiler 1.014 and 1.005 times its time, 13,776 bytes larger, the same binary for both. Every range straddles 1; every median is below 1.

Against the criteria registered:

- **Both pass** the regression, correctness and soundness; report fewer errors than n0 (n1 2, n2 0, against 8); write no more lines (2 to 3 fewer, N4 the same); time at most 1.05 times n0's by the median, memory identical, the compiler 1.014 and 1.005 times its time.
- **Both fail the bytes**: the module of the TypeScript form is 9 to 18 bytes larger than the n0 form's on N1, N2 and N5 (0.2 to 0.4 %), and n2's one byte larger on N4. Part of it is not the narrowing: with no option at all, `if (a && b) { … }` builds 130 bytes where the same written as two `if` builds 127, `&&` lowered as a value then tested; the rest was not traced, no disassembler being installed.

### Proposal

By the criteria registered, **no option is an improvement, and n0 stays**: both grow the bytes of the programs that use them, which the protocol forbids, as it registered. The criteria are not changed after the measure. What the measure shows, for the owner to decide:

- n2 takes every program of the protocol, written as in TypeScript, where n0 refuses eight times; n1 takes the chains, not `else` nor `!`; their time is the same as n0's or below, and nothing else changes.
- The bytes they add are the cost of the form a user writes, `&&` against nested `if`, more than of the narrowing; finding M038, open, already asks whether a module of a program that chooses a new construct may grow by what the decision measures.

Two ways forward, the owner's to choose: keep n0, F118 staying open; or decide M038 first, or lower `if (a && b)` as nested branches by a decision of its own, measured on every program, and measure this protocol again.

### Decision and landing (2026-11-24)

The owner chose **n2**, and decided finding M038 with it: a program that does not use what a decision brings keeps its module identical; one that chooses it may grow by what the decision measures and records, here 0 to 18 bytes on the programs that write `&&`, 0.4 % at most. n2 is the compiler: an option is narrowed through `&&`, `||`, `!`, `else` and `while` (`Checker::facts`, `condition_facts`, `operand_facts`). n1 is removed, its figures kept above and its code in git at `e1e5ed8`; `PERAS_PROTO_NARROW=n0` builds the compiler before this decision, for `scripts/bench-narrow.mjs` only, which now compares n0 with n2 and checks that n2 takes every program of the protocol. The soundness test and the differential test run with n2 by default. `check-regression` against `a687330`: of 270 modules, none larger, none smaller, none changed; the five TypeScript forms build now only; the compiler 1.006 times its time, 1,240 bytes smaller than the prototypes, 12,536 bytes larger than before the decision. The guide (`guide/enums.md`), the status and finding F118, closed, say what a program can now write.
