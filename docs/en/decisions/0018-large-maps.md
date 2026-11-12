# 0018: Large maps, from a profile of the dashboard at 4,000,000 rows

_French translation: [../../fr/decisions/0018-large-maps.md](../../fr/decisions/0018-large-maps.md)._

_Status: proposed on 2026-11-12; the index keeping keys of 32 bits was built, measured below its registered criterion, and taken back the same day (see [Measured: taken back](#measured-taken-back)). Each change is decided by an A/B of two compilers in one process at 4,000,000 rows (finding M017), then measured by a new run of [decision 0014](0014-data-at-scale.md)'s protocol, unchanged._

## Context

Run 4 of decision 0014 left one hypothesis moving the wrong way. H6 asks that Peras's time at 4,000,000 rows be at most 4.4 times its time at 1,000,000. It was 3.80 in run 2, 4.42 in run 3 and 4.88 in run 4. Decision 0017 took 11 % off the time at 1,000,000 rows but only 2 % at 4,000,000. Something grows faster than the rows, and it is not the work per row that 0017 removed.

**The profile.** `node scripts/profile-data.mjs --rows N --only …` times the same variants of `bench/data/dashboard.peras` at 1,000,000 and at 4,000,000 rows, in one session, at commit `8417667`. The figures are medians of 9 rounds, in Node.

| What is left out                                | 1,000,000 rows | 4,000,000 rows | 4M / 1M |
| ----------------------------------------------- | -------------- | -------------- | ------- |
| Nothing: the dashboard as written               | 278 ms         | 1,265 ms       | 4.55    |
| Everything but the walk over the lines          | 21 ms is all   | 85 ms is all   | 4.05    |
| Everything but the reading and the filter       | 137 ms is all  | 521 ms is all  | 3.80    |
| The map of pairs                                | 54 ms          | 245 ms         | 4.5     |
| The map of products                             | 15 ms          | 65 ms          | 4.3     |
| The `add` of a user to the set of their country | 69 ms          | 420 ms         | 6.1     |
| The daily revenue                               | within noise   | within noise   |         |

Everything grows with the rows but one thing: adding a user to the set of their country. Each row kept costs about 140 ns at 1,000,000 rows and about 214 ns at 4,000,000. A cost per operation that rises with the size of the data is the mark of memory that is no longer in the cache. The sets hold more distinct users as the file grows: up to 200,000 users and 20 countries, so up to millions of pairs at 4,000,000 rows. Finding F215 already ruled out the hash.

**Where the misses are.** A map or a set of Peras keeps the order of insertion, as Python's dicts do since 3.6: entries in order, and an index of entry numbers. A lookup reads an index slot, then the entry that slot names, to compare its key. In a large set, both are out of the cache, so each lookup costs two misses. Most of the dashboard's `add`s find their user already there, and such an `add` changes nothing: it pays both misses only to learn that.

## The technique

**The key in the index, for integer keys of 32 bits.** Kulukundis, _Designing a Fast, Efficient, Cache-friendly Hash Table, Step by Step_, CppCon 2017 (Abseil's Swiss tables), keeps a few bits of each hash next to its slot, so that a probe rarely reads an entry that is not the one sought. For a key of 32 bits, more can be kept: the key itself.

- An index slot of 8 bytes holds the entry's number and the key.
- A lookup that finds its key reads one slot, and no entry.
- A lookup that meets another key moves on without reading that key's entry either.

The order of insertion is kept as it is, in the entries. Iterating does not change.

**Where it applies.** The compiler knows a map's key type. `Set<i32>` and `Map<i32, …>` would get the new index; maps with keys of 64 bits and maps with string keys keep theirs. For string keys, the same idea with a few bits of the hash would apply. That is a second step, decided by its own measure.

**What it costs.** The index takes twice the memory: 8 bytes a slot instead of 4, at most a third empty. For a set of 1,000,000 users, that is about 4 MB more. A measure of memory is part of the decision: H3 of decision 0014 and the peak in run 5.

**Expected.** The `add`s that find their user already there pay one miss instead of two. If the misses are the cost, most of what grows from 140 to 214 ns per row, and part of the 140. This is an estimate: a profile cannot remove a cache miss. Only the A/B will tell.

## What does not change

- The order of iteration, which is the order of insertion, and every result. The runtime's model tests of maps and sets compare both with a model in Rust, on both allocators, and gain sets of `i32` keys.
- SipHash, the protection against keys chosen to collide (F215).
- The protocol of 0014, and `dashboard.peras`, which is not rewritten.
- The differential test (M015): 2,000 seeds, and once with the change broken on purpose.

## Criteria, registered before any code

Registered on 2026-11-12, before the change is written. The new index is kept only if all three hold:

1. **4,000,000 rows.** `node scripts/ab-data.mjs` at 4,000,000 rows gives a median before / after of at least 1.10 in each of three runs.
2. **1,000,000 rows.** The same A/B at 1,000,000 rows gives a median of at least 0.98 in each of three runs: no loss above 2 %.
3. **Memory.** A new run of decision 0014 gives H3, C's peak memory over A's at 4,000,000 rows, at most 0.90. It was 0.78 in runs 3 and 4.

If any fails, the change is taken back: the commit reverted, the measures kept, the finding written.

## Measured: taken back

The index that keeps keys of 32 bits was written and tested at commit `61ef836`.

- A runtime model test of keys negative and at the bounds of an `i32`, with deletes, growth, a clear and a copy, on both allocators.
- The differential test, which gains a growing `Set<i32>` and a `Map<i64, i32>` whose keys share their low 32 bits. 2,000 seeds pass.
- Two mutations each fail the tests: the key not kept in the index, and keys of 64 bits given the new index.
- Every `--check` of `scripts/` and the 120 end-to-end tests pass.

Then it was measured against the criteria registered before the code, with `node scripts/ab-data.mjs 1b92494 61ef836`:

| Criterion                                 | Runs (median before / after)  | Verdict   |
| ----------------------------------------- | ----------------------------- | --------- |
| 1. At 4,000,000 rows, at least 1.10, each | 1.112, 1.038, 1.091           | **fails** |
| 2. At 1,000,000 rows, at least 0.98, each | 1.016, 0.994, 1.009           | holds     |
| 3. H3 at most 0.90                        | not measured: 1 already fails |           |

**The change is taken back**, as registered: the runtime, the compiler and the runtime's test as they were before `61ef836`. What stays is useful without it: the differential test's new maps, `--rows` in `scripts/ab-data.mjs`, and these measures.

**What it says.**

- **The gain was real but small**: 4 to 11 % at 4,000,000 rows, nothing at 1,000,000. Keeping the key in the index removed the read of the entry, but not most of what grows.
- **The other miss remains.** The index slot itself is out of the cache for a large set, and doubling its size makes fewer slots fit.
- **What the profile could not say.** It showed that the `add` grows 6.1 times from 1,000,000 to 4,000,000 rows. It could not say how much of that is the entry, the slot, or the growth of the set. The estimate was the most this change could remove; it removed less.

The next step on this cost is not another layout of the index. It is to know where the time goes inside the `add`. That calls for a measure of its parts, the probe, the growth of the set and the copy of its entries when it grows, at 4,000,000 rows.
