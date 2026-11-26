# 0048: Choices without branches, and small functions that give a structure

_French translation: [../../fr/decisions/0048-choices-and-small-structures.md](../../fr/decisions/0048-choices-and-small-structures.md)._

_Status: protocol registered on 2026-11-25, before any prototype, as the rule of the project asks, after the floors of its options were measured (finding M041); s1 built and measured at `b56cab7`, s2 and s3 measured by their floor; proposal: c0 stays, no option meeting the criteria registered; for the owner to decide. Asked by the owner on 2026-11-25: work the language so that it catches up with TypeScript where it is behind, and P2 is the program the owner measured on a phone, Peras at 1.35 times TypeScript's time._

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

### The prototype s1 (2026-11-25)

At `ec62b13`: the IR gains a select, its three operands all evaluated (`ExprKind::Select`), emitted as WebAssembly's `select`, as a choice in Rust, and as `v128.bitselect` under a mask in a loop SIMD vectorizes. A pass, `branchless_choices` in `crates/perasc-ir/src/opt.rs`, on only when `PERAS_PROTO_CHOICE` names s1 or s3, rewrites an `if` that sets a scalar local to one of two simple values (no call, division nor read of memory, `is_simple`) as that local set to a select, after `branchless_abs`; `min`, `max` and `clamp` are lowered as such an `if`, so they take it too. The whole test suite passes with s1 on, but `other_sign_tests_keep_their_branch`, which checks the very shape s1 changes. At `99788a9`, the differential test's generator gains `min`, `max` and `clamp` on integers and floats, NaN and a zero's sign among them, and an `if` that sets a local to one of two values, with one branch or two: it passes 2000 seeds with s1, and fails with s1's select given its two values swapped (finding M015).

### s2 and s3, measured by their floor (finding M041)

s2's floor is P2 with `rgba`, its one small function that gives a structure on the pixel's path, written inline by hand, with the same result: 1.009 times c0's time, 15 rounds, against an A/A control of 1.01 to 1.02. The time the profile found in `rgba`, `channel`, `word` and `Rgba8` together, 1.77 to 1.46 against TypeScript, is spread over three small functions that give a number and one that gives a structure: removing the one s2 reaches removes none of it. s2 cannot meet 0.95, so it is not built; s3 is s1 and s2, neither of which gains, and is not built either.

### The measure (2026-11-25)

At `b56cab7`, a script of this session, not kept in the repository, as s1 leaves the compiler with it; its figures are these: P2 (120 frames), A3 (300 frames) and the garden (512 × 512 at zooms 1, 4, 16 and 64), each built for production (`--allocator tlsf`) under c0, c0 again and s1, and TypeScript; 21 rounds in one process, the order alternating round by round; the ratio to c0 taken round by round. CPU time by `process.cpuUsage` over each version's runs. Every version gives c0's result, twice on one instance. Intel Xeon at 2.10 GHz, 4 cores, Node 24.22.1.

| Program | Version  | time / c0, median (range)  | time / TypeScript | CPU ms a run | CPU / wall | memory at the end | bytes, raw and gzip |
| ------- | -------- | -------------------------- | ----------------: | -----------: | ---------: | ----------------: | ------------------- |
| P2      | c0       | 1.000                      |             1.755 |        459.9 |      0.988 |          2112 KiB | 7022, 3340          |
| P2      | c0 again | 1.021 (0.838 to 1.292)     |             1.804 |        471.8 |      0.990 |          2112 KiB | 7022, 3340          |
| P2      | s1       | **1.108** (0.876 to 1.324) |             1.940 |        499.5 |      0.993 |          2112 KiB | 7022, 3339          |
| A3      | c0       | 1.000                      |             2.028 |         91.6 |      0.994 |           640 KiB | 12111, 5125         |
| A3      | c0 again | 0.987 (0.878 to 1.220)     |             2.002 |         91.3 |      0.996 |           640 KiB | 12111, 5125         |
| A3      | s1       | **1.003** (0.853 to 1.550) |             2.062 |         96.6 |      0.988 |           640 KiB | 12111, 5124         |
| garden  | c0       | 1.000                      |             0.925 |       2175.9 |      0.994 |          1088 KiB | 6166, 3078          |
| garden  | c0 again | 0.981 (0.911 to 1.032)     |             0.906 |       2128.5 |      0.995 |          1088 KiB | 6166, 3078          |
| garden  | s1       | **0.981** (0.869 to 1.076) |             0.925 |       2135.9 |      0.994 |          1088 KiB | 6166, 3082          |

`node scripts/check-regression.mjs HEAD PERAS_PROTO_CHOICE=s1`, against `b56cab7`: of 307 modules, 7 larger by 14 to 18 bytes (the pond, as written and as kept, under both allocators, its bands, and W1 of decision 0032 under both), 19 smaller by about 5 bytes, 50 changed at the same size; the compiler 1.000 times its time (0.969 to 1.109), its binary the same size, the pass being in it whether on or off.

Beside the protocol, as F128 asks: decision 0047's f1, whose rounding is the branch the processor mispredicts, takes 0.991 times c0's time under s1 (0.923 to 1.241, 15 rounds, A/A 0.975): its condition, `d > 0.5 || (d == 0.5 && f % 2 == 1)`, holds `||`, `&&` and a float remainder, a call of the runtime, which s1 leaves as they are. The rounding F128 found has its own answer, `roundEven()`, decision 0047's f3.

### Proposal

By the criteria registered, **no option is kept, and c0 stays**:

- **s1** gives every result the same and passes the differential test, failing it when broken; but it takes 1.108 times c0's time on P2 and 1.003 on A3, where 0.95 was asked, the A/A controls at 1.021 and 0.987, within 0.95 to 1.05; the garden 0.981, the same as its A/A control. Seven modules grow by up to 18 bytes, none of which it measures faster. A select costs both sides every time; the branches of these programs are predicted well, which this protocol said could happen.
- **s2** cannot meet 0.95 by its floor, 1.009; **s3** combines two options that gain nothing.

What remains of P2's gap to TypeScript is not in its choices nor in a small function that gives a structure: removing the clamp gave 1.59 to 1.69 and writing the whole pixel inline 1.46, the work of the three functions that give a number together, which only a general inlining reaches, and decision 0036 measured that slowing the garden by 5 %. Once the owner decides, s1's code and `PERAS_PROTO_CHOICE` are removed from the compiler, the code kept in git at `ec62b13`, the differential test's extension kept. Finding F128 stays open with what this measured: an if-conversion of conditions joined by `&&` and `||` would be another decision, with no program of the project yet that it would make faster.
