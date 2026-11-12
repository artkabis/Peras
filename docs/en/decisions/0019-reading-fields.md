# 0019: Reading the fields of a line faster

_French translation: [../../fr/decisions/0019-reading-fields.md](../../fr/decisions/0019-reading-fields.md)._

_Status: proposed on 2026-11-12; step 1 built, measured below its criterion and taken back the same day (see [Step 1, measured](#step-1-measured-taken-back)); closed with step 2 not built (see [Outcome](#outcome)). Each step is decided by an A/B of two compilers in one process (`node scripts/ab-data.mjs`, finding M017), against criteria registered here before its code._

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

## Step 1, measured: taken back

The range of an integer checked against constants was written at commit `3ab2b62` and tested:

- the end-to-end tests of `parseI32` and `parseI64` against JavaScript, at and past their bounds;
- the differential test, which gains integers read at the bounds of 32 and 64 bits. 2,000 seeds pass;
- a mutation, a wrong tenth for 32 bits, fails both.

Then `node scripts/ab-data.mjs 491e1b0 3ab2b62` measured it against the criteria:

| Criterion                                 | Runs (median before / after) | Verdict   |
| ----------------------------------------- | ---------------------------- | --------- |
| 1. At 1,000,000 rows, at least 1.02, each | 1.027, 1.022, 1.009          | **fails** |
| 2. At 4,000,000 rows, at least 0.98, each | 1.002, 1.006, 1.003          | holds     |

**Taken back**, as registered. The runtime is as it was before `3ab2b62`; the differential test keeps its integers at the bounds.

**What it says.** Two 64-bit divisions per integer, ten per row, cost 1 to 3 % of the dashboard, not more. The processor runs them while it waits for the bytes of the next field, so removing them saves less than counting them suggests.

**For step 2.** The split costs about 32 ms per 1,000,000 rows, 11 % at most. That holds only if reading the fields in place removes all of it, which steps of this decision and of 0017 and 0018 have not done. Decisions 0017 to 0019 show the same pattern: a profile gives the most a change could save, and the change saves less. The parts of the dashboard that remain are each 10 % or less. Step 2 is weighed with the owner before it is built.

## Outcome

Closed on 2026-11-12 by the owner's decision: step 2 is not built for now. The dashboard's remaining parts are each 10 % or less, and the last three changes tried on it each saved less than their profile gave. Plain Peras computes it 2.39 to 2.63 times as fast as plain TypeScript (decision 0014, run 4). What separates it from TypeScript optimized by hand is mostly what that version knows of the data, which only a change to the language could let a program say. Step 2 stays written here, with its profile, for when the reading becomes the cost again.
