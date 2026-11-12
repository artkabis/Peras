# Decision 0015: results

Measured at commit `99f9202`, 2026-11-12, under [the protocol](../../docs/en/decisions/0015-stream-measured.md). Every number below is in [results.json](results.json), with every call's time.

- Machine: Intel(R) Xeon(R) Processor @ 2.30GHz, 4 cores, linux 6.18.44-fc-v378; Chromium 153.0.8010.12; Node v24.22.1.
- Design: 10 browser processes; batches of 1000, 10000, 100000 rows; the calls in a Latin square of 4 orders; 95 % intervals by a hierarchical bootstrap of 10000 resamples, seed 15.
- Clock: cross-origin isolated true, its step at most 5.0 µs.
- Agreement: every batch of every pool gave the same digest in A, B and C, and C refused exactly the lines broken on purpose.

## Hypotheses

| | Registered | Measured, 95 % interval | Verdict |
| --- | --- | --- | --- |
| H1 | C at least 1.5 times as fast as A, at 10,000 and 100,000 rows | 2.96 [2.92, 3.01]; 3.35 [3.30, 3.40] | holds |
| H2 | C at least 0.8 times as fast as B, at 10,000 and 100,000 rows | 1.78 [1.74, 1.84]; 1.94 [1.87, 2.00] | holds |
| H3 | C's rows per second at 1,000 rows at least 0.75 times those at 100,000 | 0.91 [0.87, 0.94] | holds |
| H4 | The boundary at most 15 % of C's time at 100,000 rows | 0.10 [0.10, 0.10] | holds |
| H5 | C's 99th percentile at most 1.5 times its median at 100,000 rows | 1.79 [1.57, 1.85] | refuted |

## Time per batch

The median over processes of each process's median, in ms; rows per second from it; the spread of the process medians; the percentiles of all calls pooled.

| Rows | Version | Median ms | Min–max ms | CV | p95 ms | p99 ms | Rows/s | ns/row |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | A, TypeScript idiomatic | 0.68 | 0.66–0.70 | 2.1 % | 1.02 | 1.27 | 1,481,481 | 675.0 |
| 1,000 | B, TypeScript optimized | 0.41 | 0.40–0.46 | 4.4 % | 0.59 | 0.71 | 2,469,136 | 405.0 |
| 1,000 | C, Peras | 0.24 | 0.23–0.26 | 3.4 % | 0.35 | 0.47 | 4,166,667 | 240.0 |
| 1,000 | X, the boundary alone | 0.02 | 0.02–0.02 | 0.0 % | 0.04 | 0.06 | 50,000,000 | 20.0 |
| 10,000 | A, TypeScript idiomatic | 6.74 | 6.61–6.82 | 1.1 % | 7.83 | 10.82 | 1,483,955 | 673.9 |
| 10,000 | B, TypeScript optimized | 3.98 | 3.86–4.58 | 4.9 % | 4.67 | 6.56 | 2,510,198 | 398.4 |
| 10,000 | C, Peras | 2.27 | 2.19–2.38 | 2.4 % | 2.61 | 3.82 | 4,405,286 | 227.0 |
| 10,000 | X, the boundary alone | 0.18 | 0.17–0.19 | 5.0 % | 0.32 | 0.39 | 55,555,556 | 18.0 |
| 100,000 | A, TypeScript idiomatic | 73.83 | 71.47–74.61 | 1.3 % | 94.31 | 111.81 | 1,354,555 | 738.3 |
| 100,000 | B, TypeScript optimized | 41.88 | 41.04–48.09 | 4.8 % | 52.34 | 67.85 | 2,387,917 | 418.8 |
| 100,000 | C, Peras | 21.82 | 21.62–22.58 | 1.6 % | 28.87 | 39.25 | 4,583,739 | 218.2 |
| 100,000 | X, the boundary alone | 2.15 | 2.13–2.19 | 1.0 % | 2.71 | 3.57 | 46,403,712 | 21.5 |

## Ratios

| Rows | C over A | C over B | B over A | C's boundary share | C's p99 / p50 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 2.81 [2.76, 2.86] | 1.70 [1.66, 1.77] | 1.65 [1.60, 1.69] | 0.08 [0.08, 0.08] | 1.98 [1.75, 2.14] |
| 10,000 | 2.96 [2.92, 3.01] | 1.78 [1.74, 1.84] | 1.67 [1.61, 1.71] | 0.08 [0.08, 0.08] | 1.69 [1.39, 1.85] |
| 100,000 | 3.35 [3.30, 3.40] | 1.94 [1.87, 2.00] | 1.73 [1.68, 1.79] | 0.10 [0.10, 0.10] | 1.79 [1.57, 1.85] |

## Figures

![Rows per second, by batch size](../../docs/assets/stream-throughput.svg)

![How many times faster C is, with 95 % intervals](../../docs/assets/stream-ratios.svg)

![The distribution of the time per row at 100,000 rows](../../docs/assets/stream-latency.svg)

![C's time per row, split into the boundary and the digest](../../docs/assets/stream-breakdown.svg)

