# 0020: Text given as bytes

_French translation: [../../fr/decisions/0020-text-as-bytes.md](../../fr/decisions/0020-text-as-bytes.md)._

_Status: proposed on 2026-11-12, with its criteria and an amendment to decision 0014's version C registered before any code._

## Context

Peras's purpose includes using memory well: blocks released when they stop being used, proved by the compiler, with no collector, no pause and no leak. Decision 0014 measures how much memory the sales dashboard takes. In run 4, Peras's peak was 0.78 times plain TypeScript's at 4,000,000 rows (H3, which asks for 0.5, refuted).

**What the peak is made of.** A probe in Node, at 4,000,000 rows (`target/mem-probe`, the same build as decision 0014's version C), measured the resident memory after a collection at each step:

| Step                                                         | Memory added |
| ------------------------------------------------------------ | ------------ |
| The file's bytes, a `Uint8Array`, before the query           | 121.6 MiB    |
| `new TextDecoder().decode(bytes)`: the string given to Peras | +118.2 MiB   |
| `summarize(text)`: the module's memory                       | +183.3 MiB   |

The module's memory holds the text copied into it, 121.6 MiB, and the maps, about 62 MiB. The JavaScript string exists only because an exported function of Peras that takes a `String` accepts a JavaScript string and nothing else. The data arrives as bytes. JavaScript decodes them into a string, then Peras's loader encodes that string back into UTF-8 bytes in the module's memory. Both TypeScript versions read the bytes and make no such string.

**What cannot go.** A WebAssembly module reads only its own memory. Under decision 0014's protocol, the data starts as a `Uint8Array` outside it, so one copy into the module remains. A copy of bytes is a `memcpy`, far cheaper than decoding then encoding text.

## The change

Where an exported function takes a `String`, the loader also accepts a `Uint8Array`:

1. It copies the bytes as they are into a new string of the module.
2. The runtime checks that they are UTF-8, as `new TextDecoder("utf-8", { fatal: true })` does.
3. Bytes that are not UTF-8 throw the same `TypeError` as such a decoder, and nothing is left allocated.

The check is the rule of the Unicode Standard, chapter 3, table 3-7:

- no byte `C0`, `C1` or `F5` to `FF`;
- no overlong form;
- no surrogate, `ED A0` to `ED BF`;
- nothing above U+10FFFF;
- no sequence cut short.

Text that is ASCII is checked 16 bytes at a time with SIMD (the approach of Keiser and Lemire, _Validating UTF-8 in less than one instruction per byte_, Software: Practice and Experience 51(5), 2021, kept to its ASCII fast path).

**For a program.** Nothing in Peras changes: syntax, types and every result stay the same. The generated TypeScript declaration of such a parameter becomes `string | Uint8Array`, an addition to the API a page sees, which breaks no existing call.

**For safety.** Peras's strings are UTF-8 by construction; the check keeps them so. A string that is not UTF-8 would make the walk over its characters and its case changes wrong, so the check is never skipped, in debug or in production.

## Amendment to decision 0014, registered before any measure

Version C's glue, the line of `scripts/bench-data.mjs` that calls it, becomes `peras.summarize(bytes)` instead of `peras.summarize(new TextDecoder().decode(bytes))`. The protocol measures from the file's bytes in memory, "including any decoding and any copy into a module". Versions A and B already take the bytes. C's Peras source, A, B, the data and the thresholds do not change. The next run of 0014 is run 5, and it lists this as its departure from run 4.

## Criteria, registered before any code

Kept only if all three hold, else taken back:

1. **Memory.** In run 5 of decision 0014, H3, C's peak memory over A's at 4,000,000 rows, is at most 0.65. It was 0.78 in run 4.
2. **Time.** In run 5, C over A is at least 2.58 at 1,000,000 rows and 2.34 at 4,000,000. That is run 4's 2.63 and 2.39 less 0.05: no loss.
3. **Correctness.** An end-to-end test on both allocators gives 10,000 random byte strings and the edge cases of table 3-7. A `Uint8Array` is accepted exactly when `TextDecoder` with `fatal: true` accepts it, and then gives the same string. Nothing is left alive.
