# 0045: Narrowing an option through `&&`, `||` and `!`

_French translation: [../../fr/decisions/0045-narrowing.md](../../fr/decisions/0045-narrowing.md)._

_Status: protocol registered on 2026-11-24, before any prototype and any measure, as the rule of the project asks. Asked by the owner on 2026-11-24, after [decision 0044](0044-pools.md) met finding F118; it comes before an amendment of 0044 that measures a pool lowered by the compiler itself. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression; if none is, the decision says so and nothing lands._

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

_To come: the prototypes, the measure, the proposal._
