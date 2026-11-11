# Decision 0015: results

Measured at commit `7470d08`, 2026-11-11, under [the protocol](../../docs/en/decisions/0015-stream-measured.md). Every number below is in [results.json](results.json), with every call's time.

- Machine: Intel(R) Xeon(R) Processor @ 2.30GHz, 4 cores, linux 6.18.44-fc-v378; Chromium 153.0.8010.12; Node v24.22.1.
- Design: 10 browser processes; batches of 1000, 10000, 100000 rows; the calls in a Latin square of 4 orders; 95 % intervals by a hierarchical bootstrap of 10000 resamples, seed 15.
- Clock: cross-origin isolated true, its step at most 5.0 µs.
- Agreement: every batch of every pool gave the same digest in A, B and C, and C refused exactly the lines broken on purpose.

## Hypotheses

| | Registered | Measured, 95 % interval | Verdict |
| --- | --- | --- | --- |
| H1 | C at least 1.5 times as fast as A, at 10,000 and 100,000 rows | 2.22 [2.20, 2.24]; 2.46 [2.43, 2.49] | holds |
| H2 | C at least 0.8 times as fast as B, at 10,000 and 100,000 rows | 1.31 [1.29, 1.34]; 1.41 [1.37, 1.44] | holds |
| H3 | C's rows per second at 1,000 rows at least 0.75 times those at 100,000 | 0.94 [0.92, 0.95] | holds |
| H4 | The boundary at most 15 % of C's time at 100,000 rows | 0.07 [0.07, 0.07] | holds |
| H5 | C's 99th percentile at most 1.5 times its median at 100,000 rows | 1.75 [1.50, 1.88] | refuted |

## Time per batch

The median over processes of each process's median, in ms; rows per second from it; the spread of the process medians; the percentiles of all calls pooled.

| Rows | Version | Median ms | Min–max ms | CV | p95 ms | p99 ms | Rows/s | ns/row |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | A, TypeScript idiomatic | 0.68 | 0.67–0.71 | 2.1 % | 1.03 | 1.20 | 1,470,588 | 680.0 |
| 1,000 | B, TypeScript optimized | 0.41 | 0.39–0.45 | 4.0 % | 0.59 | 0.68 | 2,469,136 | 405.0 |
| 1,000 | C, Peras | 0.32 | 0.32–0.33 | 2.2 % | 0.49 | 0.58 | 3,076,923 | 325.0 |
| 1,000 | X, the boundary alone | 0.02 | 0.02–0.02 | 0.0 % | 0.04 | 0.06 | 50,000,001 | 20.0 |
| 10,000 | A, TypeScript idiomatic | 6.72 | 6.62–7.17 | 2.3 % | 8.22 | 11.24 | 1,487,542 | 672.3 |
| 10,000 | B, TypeScript optimized | 3.98 | 3.82–4.44 | 4.1 % | 4.75 | 6.85 | 2,514,142 | 397.7 |
| 10,000 | C, Peras | 3.03 | 3.00–3.14 | 1.6 % | 3.68 | 5.11 | 3,297,609 | 303.3 |
| 10,000 | X, the boundary alone | 0.17 | 0.16–0.19 | 7.1 % | 0.32 | 0.37 | 57,971,015 | 17.2 |
| 100,000 | A, TypeScript idiomatic | 74.40 | 72.47–78.91 | 2.6 % | 92.84 | 110.96 | 1,344,131 | 744.0 |
| 100,000 | B, TypeScript optimized | 42.71 | 40.05–47.27 | 4.2 % | 54.98 | 70.29 | 2,341,166 | 427.1 |
| 100,000 | C, Peras | 30.47 | 30.00–31.21 | 1.1 % | 41.21 | 53.44 | 3,281,917 | 304.7 |
| 100,000 | X, the boundary alone | 2.20 | 2.09–2.32 | 3.5 % | 2.82 | 3.60 | 45,558,087 | 22.0 |

## Ratios

| Rows | C over A | C over B | B over A | C's boundary share | C's p99 / p50 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 2.10 [2.08, 2.13] | 1.26 [1.24, 1.28] | 1.67 [1.63, 1.71] | 0.06 [0.06, 0.06] | 1.80 [1.65, 1.88] |
| 10,000 | 2.22 [2.20, 2.24] | 1.31 [1.29, 1.34] | 1.69 [1.64, 1.73] | 0.06 [0.06, 0.06] | 1.68 [1.58, 1.86] |
| 100,000 | 2.46 [2.43, 2.49] | 1.41 [1.37, 1.44] | 1.74 [1.69, 1.80] | 0.07 [0.07, 0.07] | 1.75 [1.50, 1.88] |

## Figures

![Rows per second, by batch size](../../docs/assets/stream-throughput.svg)

![How many times faster C is, with 95 % intervals](../../docs/assets/stream-ratios.svg)

![The distribution of the time per row at 100,000 rows](../../docs/assets/stream-latency.svg)

![C's time per row, split into the boundary and the digest](../../docs/assets/stream-breakdown.svg)

