# 0019: Reading the fields of a line faster

_French translation: [../../fr/decisions/0019-reading-fields.md](../../fr/decisions/0019-reading-fields.md)._

_Status: proposed on 2026-11-12. Each step is decided by an A/B of two compilers in one process (`node scripts/ab-data.mjs`, finding M017), against criteria registered here before its code._

## Context

Decision 0017 found that the optimized TypeScript owes about 70 ms of its lead to reading the bytes in one pass. Decision 0018 left the large maps as they were. This decision takes the reading.

**The profile.** `node scripts/profile-data.mjs` gives variants on top of `parse-only`, where nothing after the filter runs. The figures are for 1,000,000 rows, in one run, at commit `5d3c443`.

| The reading, with one thing removed              | Time   | What it costs          |
| ------------------------------------------------ | ------ | ---------------------- |
| Parsed and filtered, nothing aggregated          | 139 ms |                        |
| The five integers their lengths, not parsed      | 100 ms | about 39 ms            |
| Without the check of the country's letters       | 113 ms | about 26 ms            |
| The line split and its fields counted, none read | 80 ms  |                        |
| No split: a row made of the line's length        | 48 ms  | the split: about 32 ms |
| The walk over the lines alone                    | 23 ms  |                        |

The parts are of a size: the integers about 39 ms, the split about 32, the check of the letters about 26. Reading the fields straight from the bytes, without their views (Langdale and Lemire, 2019), would remove the split's 32 ms at most. It is also a large change: a field passed to a function, as `isCountry(f[2])`, still needs a string, so a view of it.

**What the code of the integers shows.** `str_is_int` checks the range of an integer as it reads it, against a limit and a tenth of it. It computes them at each call: an `i64.div_u` and an `i64.rem_u`. A division of 64 bits is among the slowest instructions of a processor. For the 32 and 64 bits that Peras compiles, both are constants:

- for 32 bits, the tenth is 214,748,364 and the last digit 7, or 8 after `-`;
- for 64 bits, the tenth is 922,337,203,685,477,580 and the last digit 7, or 8 after `-`.

## Steps

1. **The range of an integer checked against constants.** No division for 32 and 64 bits; other widths compute them as now. Small, and it changes no result.
2. **The fields read from the bytes.** Where a line's split is only read by `f.length` and `f[i]` with constant `i`, the separators are found once and each field is read in place. A field passed to a function keeps a view. This step is decided after step 1, with its own criteria.

## Criteria for step 1, registered before its code

Kept only if both hold, else taken back:

1. **At 1,000,000 rows.** `node scripts/ab-data.mjs` gives a median before / after of at least 1.02 in each of three runs.
2. **At 4,000,000 rows.** The same, at least 0.98 in each of three runs: no loss.

## What does not change

- Every result: an integer is accepted or refused exactly as before. The end-to-end tests compare `parseI32` and `parseI64` with JavaScript at their bounds.
- The differential test passes 2,000 seeds, and fails with the change broken on purpose (M015).
