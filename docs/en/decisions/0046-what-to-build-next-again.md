# 0046: What to build next, measured again

_French translation: [../../fr/decisions/0046-what-to-build-next-again.md](../../fr/decisions/0046-what-to-build-next-again.md)._

_Status: protocol registered on 2026-11-25, before any program is written, as the rule of the project asks. Asked by the owner on 2026-11-25: decide what comes next by measure, as [decision 0011](0011-what-to-build-next.md) did, rather than by the order of the [roadmap](../reference/roadmap.md)._

## Context

The last time what to build was ranked by measure was decision 0011, thirty-four decisions ago: seventeen bugs of web applications and a case study written from TypeScript. Since then the decisions have followed the roadmap's order, and three in a row kept what was there: arenas (0043) and pools (0044) were measured and not kept, and the first verdict of 0045 too. Meanwhile the one friction found by writing programs as a user would, an option not narrowed through `&&` and `||` (finding F118), was worth a decision of its own: eight errors on five programs, none since 0045.

Frictions known and not yet ranked, each from a finding: JSON in owned modules (`E0900`), a `let` never narrowed, the reads of a field the tools cannot resolve (F313), a failing test that cannot show an array or a structure (F227), no allocation in a turn of a `parallel for` (F301), module variables that cannot be strings or arrays (decision 0023), packages (phase 5). And the next step of the roadmap, the `@frame` rule.

## Method, registered before any program is written

### The programs

Four applications, each written first in TypeScript, the specification, then in Peras by translating it as a developer coming from TypeScript would, without reshaping it to fit what Peras offers. They are chosen to cover both modes and the kinds of work the roadmap names, and none of them is a program the repository already has:

- **A1, notes** (a managed module): a list of notes kept in the page's storage as JSON, searched as the user types, sorted by date, with tags; the DOM, events, timers, storage, JSON.
- **A2, a ledger** (an owned module called from a managed one): a CSV of transactions read, grouped by month and by category, with a running balance and the largest movements; strings, maps, sorting, numbers of money.
- **A3, a particle scene** (an owned module): a frame of particles that are born, move, collide on a grid and die, drawn into a framebuffer; arrays of structures, the work of a frame, what `@frame` would rule.
- **A4, a route finder** (an owned module): the shortest path between stops of a small network read from JSON, with options for a stop that is missing; structures that point to one another, options, a priority queue.

### What is counted, for each program

1. **Errors met**: each run of `perasc check --json` on the Peras version as written, then after each fix, until it builds; every diagnostic recorded with its code, its message, and the fix made. A fix is the smallest change a user would make from the diagnostic.
2. **What cannot be written**: a part of the TypeScript the Peras version cannot express at all, recorded with what was done instead (`E0900`, a missing method, a missing type).
3. **The lines**: of each version, blank lines and comments apart, and the lines the Peras version changes from a literal translation.
4. **The results**: each Peras version gives what its TypeScript gives, on the same inputs, built for production (`--allocator tlsf`, finding M029).
5. **The time**, secondary: each version's time on its workload against its TypeScript's, in one process, the order alternating round by round, 21 rounds, with an A/A control, the TypeScript timed against itself in the same rounds (finding M042).

### The ranking

Each error, each thing that cannot be written and each changed line is attributed to one cause: an existing finding, a step of the roadmap, or a new finding. A cause is scored by the programs it blocks (a part that cannot be written), then by the errors it causes, then by the lines it adds. The ranked list is the order of the next decisions; a cause that no program meets is not ranked, and says so.

### What this cannot show, said now

The programs are written by the authors of Peras, who know what it refuses: they cover the applications the authors know, not every application, as decision 0011 said of its corpus. To bound that: each TypeScript version is written and committed before its Peras version, and not changed after; each fix is the one the diagnostic offers, or the smallest a user would make, recorded as such.

## Record

### The specifications (2026-11-25, before any Peras)

The four programs in TypeScript, `bench/next/{notes,ledger,particles,routes}.ts`, type-checked at TypeScript's strictest (`bench/next/tsconfig.json`: `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), each with a `run` that makes its own input and gives a line of text to compare: A1 for 2,000 notes gives `2000|139|1986|books:800,…`; A2 for 20,000 rows, twelve months and a lowest balance of -382,191.29; A3 after 300 frames, 1,792 particles and a digest of the pixels, 694335550; A4 on a grid of 60 by 60 stops, `298:119|330:121|missing nowhere`. Written before this commit, the ledger's generator was corrected twice, before any line of Peras: its low bits made five months empty, and a salary of 2,500 kept the balance from ever falling below zero; it takes the generator's high bits, and a salary of 300. From this commit on, they are not changed.

_To come: the Peras versions, the measures, the ranking._
