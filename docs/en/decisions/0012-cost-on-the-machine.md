# 0012: The cost on the machine, measured with every step

_French translation: [../../fr/decisions/0012-cost-on-the-machine.md](../../fr/decisions/0012-cost-on-the-machine.md)._

## Context

Peras promises programs without leaks, typed strongly, close to TypeScript. A language that keeps those promises by shipping more code, holding more memory or keeping the processor busier would move their cost to the users' machines: their battery, their data plan, the energy they draw. The owner asked that this cost be watched from now on, before the audit tools come, and that energy be thought of at once.

A browser does not tell a page the energy it spends, and neither do most virtual machines: the processor's counters (RAPL on Linux) are absent from the machine the benchmarks run on. What can be measured, and what the studies of web energy use, are the quantities that drive it: the bytes sent over the network, the time a core is kept busy, and the memory held.

## Decision

Every step of the language is measured on four quantities, in the case study of [decision 0011](0011-what-to-build-next.md) (`node scripts/bench-case-study.mjs`, [results](../../../bench/case-study/RESULTS.md)), against the same program in TypeScript:

1. **Bytes shipped**: the JavaScript minified by Vite, then gzip, as a page receives it.
2. **CPU time**: the time the page's tasks keep a core busy for the whole scenario, Chromium's `TaskDuration`, the closest measure of energy a browser gives.
3. **Memory**: the JavaScript heap, after a collection, while the table is shown and once it is removed.
4. **Energy**: the processor's, read from Linux (RAPL) where the machine gives it; reported as not measured elsewhere, never estimated.

Leaks are not left to these measures: owned modules run their tests on the debug allocator, which counts every block alive ([decision 0001](0001-webassembly-backend.md)), and managed modules report every resource collected without being released, in debug ([decision 0009](0009-browser-and-resources.md)).

The bytes shipped are the one quantity a machine gives the same each time: CI holds them to a **budget**, 1,600 bytes for the case study. A change that needs more raises the budget here, with its reason. The others vary by about 10 % from one run to the next, and are reported, not checked.

Three rules follow, already applied:

- **A helper of the runtime is written only when used, and only the variant its types need.** `sortBy` by a number brings the radix sort, by a string the comparison of code points, never both.
- **A production build carries no debugging code.** Without the leak detector, a resource is a plain object with `release()`.
- **What JavaScript does natively, it does.** `toFixed` is JavaScript's own in a managed module, and the host's in an owned one: no bytes added. A string all ASCII is lowered by `toLowerCase()`, 2.7 times as fast as the general path, with the same result.

## Results

The case study at this decision, medians of 5 pages in Chromium:

| Quantity                             | TypeScript |      Peras |
| ------------------------------------ | ---------: | ---------: |
| bytes shipped, minified and gzip     |        633 |      1,512 |
| CPU time, 1,000 orders               |   78–84 ms |   82–91 ms |
| CPU time, 10,000 orders              | 467–539 ms | 520–540 ms |
| heap held while shown, 1,000 orders  |    100 KiB |    123 KiB |
| heap left once removed, 1,000 orders |    121 KiB |    143 KiB |

These measures moved this step's work: the sort's helpers were split by key type and the debugging code left out of production builds, 2,513 bytes gzip down to 2,124 before minification; a string's lowering, 1.3 ms saved per 10,000 names. The CPU time is within the noise of the runs at 10,000 orders and 5 to 10 % above at 1,000. The 850 bytes more are the runtime that keeps the promises: resources, timers released when replaced, groups, the sort. The heap left once removed is higher because releasing a group leaves the rows in the page, point 8 of decision 0011, to be removed by step 7.

## Not chosen now

- **Estimating energy from bytes or time** with a model such as CO2.js: a model's coefficients are not a measure; the quantities it would start from are reported as they are.
- **A budget on CPU time**: its noise would make CI fail at random; a dedicated, quiet machine could hold one.
- **Writing the runtime's helpers shorter by hand**: a minifier does it for the bytes that are shipped, and the source stays readable.

## Consequences

- The audit tools to come can start from these four quantities, in the results of each benchmark.
- Each next step of decision 0011 reports them, and a step that costs the machine more says why.
