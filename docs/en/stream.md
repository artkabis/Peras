# A stream of sensors

_French translation: [../fr/stream.md](../fr/stream.md)._

An application that does what Peras is meant for: it reads a stream of data, as text, in the browser, and turns it into numbers fast enough to keep up. It also measures itself while it runs, and hands over what it measured.

**[Open it](https://artkabis.github.io/Peras/stream/)** ([source](../../examples/stream)). To run it from the repository: `cargo build --release -p perasc`, then `pnpm --filter peras-example-stream build` and `pnpm --filter peras-example-stream preview` to measure. `dev` also works, with a module built for debug, every memory access and integer overflow checked, and slower.

## What it does

- **The stream.** 480 sensors in twelve zones of a building send a reading every five seconds: time, sensor, zone, temperature, humidity and power, as CSV text. About one line in 400 is broken, of five kinds: a missing field, `NaN`, a decimal comma, a zone in capitals, a `%` after a number. A batch is a pure function of its number and a seed: the same batch gives the same bytes on every run, in every browser.
- **The digest, in Peras** ([`telemetry.peras`](../../examples/stream/src/telemetry.peras)). One pass over a batch: it splits the lines, reads three integers or names and three decimals with `parseF64`, refuses what is broken, and sums by zone the readings, alerts, temperatures, their minimum and maximum, humidity and energy; it also counts a histogram of temperatures and finds the hottest reading. It runs in a Web Worker, so the page never waits.
- **The same digest in TypeScript** ([`digest.ts`](../../examples/stream/src/digest.ts)), written plainly, as it often is. If asked, the worker runs both on every batch, one first then the other in turn, and compares their results: the same numbers, the sums of floats included, since both add in the same order.
- **The page** merges each batch's digest into the totals and draws them: the time per batch of each language, the temperatures read, and a table of the zones.

## What it measures

Each batch gives one sample:

| Field          | What it is                                                                        |
| -------------- | --------------------------------------------------------------------------------- |
| `batch`        | its number, from 0                                                                |
| `rows`         | the lines it holds                                                                |
| `bytes`        | the size of its text                                                              |
| `generateMs`   | the time to make its text, apart from the digests                                 |
| `perasMs`      | Peras's digest, the text's way into WebAssembly and the result's way out included |
| `typescriptMs` | the TypeScript digest, or empty when not compared                                 |
| `first`        | which ran first, `peras` or `typescript`                                          |
| `agree`        | whether both gave the same numbers, or empty when not compared                    |
| `live`         | the blocks of Peras's memory still alive after the call: 0, or a leak             |
| `at`           | the milliseconds since the stream started, when the batch ended                   |

The page shows, from them: the rows read; the rows per second of each digest alone; how many times faster Peras is, over the batches where both ran; Peras's time per batch at the 50th, 95th and 99th percentiles; how many batches gave the same numbers; the blocks left; the broken lines; and the text read per second.

**Export JSON** gives the whole report: the environment (browser, cores, memory where the browser says it, the build), the settings (rows per batch, comparison, seed), the summary and every sample. **Export CSV** gives the samples, one line per batch. A batch's size and the comparison are fixed once the stream has started, so that every sample of an export was taken under the settings it names; **Reset** starts again.

## What it is not

- **Not a measure under a protocol.** The figures the page shows are those of one machine, at one moment, with whatever else it runs. They are for watching and for exporting; a claim about Peras is made only under a protocol registered before any measure, as [decision 0014](decisions/0014-data-at-scale.md) does for a larger version of the same work.
- **Not against the fastest TypeScript.** The TypeScript is written as it often is: `split`, regular expressions, `Number`, a `Map` of objects. Written by hand for speed, it would go faster; decision 0014 measured such a version, and Peras does not reach it yet.
- **Not the worker's memory.** No browser tells a worker its own JavaScript heap, and the loader does not say how large the module's memory has grown ([finding F108](findings.md)): `live` says only that nothing leaks.

## Tested

- `examples/stream/e2e/agree.test.ts`: on twelve batches of 20,000 rows and edge cases, Peras's digest gives exactly the TypeScript one, on both allocators, with no leak.
- `examples/stream/e2e/app.test.ts`: in Chromium, the app reads batches, finds the same numbers in each, leaks nothing, alternates which digest goes first, and exports a JSON report and a CSV that match.
- `examples/stream/test/metrics.test.ts`: the percentiles, the summary, the merge of digests and the CSV.
