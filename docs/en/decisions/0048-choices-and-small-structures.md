# 0048: Choices without branches, and small functions that give a structure

_French translation: [../../fr/decisions/0048-choices-and-small-structures.md](../../fr/decisions/0048-choices-and-small-structures.md)._

_Status: protocol registered on 2026-11-25, before any prototype, as the rule of the project asks, after the floors of its options were measured (finding M041). Asked by the owner on 2026-11-25: work the language so that it catches up with TypeScript where it is behind, and P2 is the program the owner measured on a phone, Peras at 1.35 times TypeScript's time._

## Context

What the compiler does today, read before writing this (finding M020):

- **No choice is compiled without a branch.** `min`, `max` and `clamp` are lowered as comparisons and branches (`crates/perasc-ir/src/lower/functional.rs`); an `if` that sets one variable to one of two values is a branch; the IR has no `select`, and the one rewrite without a branch is `branchless_abs` (`crates/perasc-ir/src/opt.rs`). WebAssembly has `select`, which takes no branch.
- **No function is inlined** but a recursive one into itself (`crates/perasc-ir/src/inline.rs`). Decision 0036 measured small functions inlined, i1: it made 19 modules of 173 larger by 1 to 3 % and the garden 5 % slower, and was not kept. It left out a function that gives a structure (`uses_many`), whose fields come back as several results.
- **Where P2's time goes**, decision 0047's full frame with f3, profiled by removing one thing at a time with the same result (finding M013), 15 rounds in one process, against TypeScript, A/A control 1.01 to 1.02: as adopted, 1.77; its `clamp` removed, the floor of a choice without a branch there, 1.59 to 1.69 by run; the pixel written in the loop, with no `rgba`, `channel`, `word` nor `Rgba8`, the floor of inlining those small functions, 1.46. With a rounding written with a branch the processor mispredicts (decision 0047's f1), the same rounding without one went from 3.18 to 1.87 (finding F128).

## The options

- **c0, nothing.** The reference.
- **s1, choices without branches**: `a.min(b)`, `a.max(b)` and `a.clamp(lo, hi)` lowered to WebAssembly's `select` over the same comparisons, so that NaN and a zero's sign give what they give today; and an `if` whose branches only set one local to one of two values, each cheap and unable to trap (`is_simple`), rewritten to that local set to a `select`.
- **s2, small functions that give a structure, inlined**: a function whose body is one `return` of a structure of numbers built from its parameters, or of numbers computed from them, no larger than 40 nodes, no loop, no heap, replaced at each call by its fields, each argument evaluated once and in order; the narrowest form of decision 0036's i1, at the place its profile points to.
- **s3, both**, s1 then s2, measured apart and together (finding M043).

## The programs

- **P2**, decision 0047's full frame with f3, `bench/frame/plasma-f3.peras`, and **A3** with f3, `bench/frame/particles-f3.peras`, against TypeScript.
- **The garden** (decision 0013), the program decision 0036's inlining slowed.
- **Every module of the repository**, by `scripts/check-regression.mjs`.

## What is measured, registered now

1. **Correctness**: the differential test (`crates/perasc-driver/tests/differential.rs`), its generator extended with `min`, `max`, `clamp`, an `if` that sets one local to one of two values, and small functions that give a structure, passed with `PERAS_DIFFERENTIAL_SEEDS=2000`, and failed with each option broken on purpose (finding M015); every result of P2, A3 and the garden the same as the reference's.
2. **Speed**: each option against c0 in one process, the order alternating round by round, 21 rounds, with an A/A control, c0 against itself (finding M042); P2, A3 and the garden.
3. **CPU load**: the time a core is kept busy over P2's frames, as decision 0012 measures it.
4. **Memory and bytes**: each module's size, raw and gzipped, under each option, by `check-regression`; memory at its peak for P2, A3 and the garden.
5. **The compiler**: its time (median ratio at most 1.05) and its size.
6. **Ease**: none of these options changes what a user writes; said, not measured.

## The criteria, registered now

An option is kept when: every result is the same; it passes the differential test and fails it when broken; P2 or A3 takes at most 0.95 of c0's time, with the A/A control's median within 0.95 to 1.05; the garden takes at most 1.05 of c0's time; no module is larger, but those whose time it measures faster, each named with its figures (finding M038); the compiler is no slower than 1.05. Of the options kept, the one that does least; s3 over s1 or s2 only if it takes at most 0.95 of the better one's time on P2.

## What this cannot show, said now

The programs are the project's: a choice without a branch is slower than a branch the processor predicts well, and a program whose choices are predictable may lose by s1; the garden and the modules of the repository are there to find it, not to prove it cannot happen. The engines inline and choose too, and differently on another processor: P2 measured 1.35 on a phone where it measured 1.80 here.

## Record

_To come: the prototypes, the measures, the proposal._
