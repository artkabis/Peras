# 0017: Aggregating without copies, from a profile of the dashboard

_French translation: [../../fr/decisions/0017-aggregating-without-copies.md](../../fr/decisions/0017-aggregating-without-copies.md)._

_Status: proposed on 2026-11-12. Step 1 landed at `037bb74`, step 2 at `02ff9c2`, step 3 at `a497971` (see [Measured after step 1](#measured-after-step-1), [after step 2](#measured-after-step-2) and [after step 3](#measured-after-step-3)). Each change is measured when it lands: first by the profile below, in Node, then by a new run of [decision 0014](0014-data-at-scale.md)'s protocol, unchanged. A change that does not show its gain there is reported as such._

## Context

In run 3 of decision 0014, Peras written plainly computes the sales dashboard 2.06 to 2.36 times as fast as plain TypeScript. TypeScript optimized by hand is still 3.6 to 4 times faster than it. Decision 0016 removed the costs that the stream app and the dashboard share: literals, calls into the runtime, small blocks, a lookup done twice, the search for separators. What is left is mostly the dashboard's own: its aggregation.

**The profile.** `node scripts/profile-data.mjs` times variants of `bench/data/dashboard.peras` that each leave out or replace one thing, at commit `989c3c4`. The figures are for 1,000,000 rows, about 490,000 of them kept, the median of 9 rounds, the variants alternating. The dashboard as written takes 322 ms.

| What is left out or replaced                                        | Saves                | Finding                |
| ------------------------------------------------------------------- | -------------------- | ---------------------- |
| Everything aggregated: the lines are only parsed and filtered       | 190 ms, 59 %         |                        |
| The map of pairs, keyed by `` `${country}\|${category}` ``          | 96 ms, 30 %          | [F216](../findings.md) |
| … of which the key built from a template, the country alone instead | 27 ms, 8 %           | [F202](../findings.md) |
| … of which the copy of the country in `unwrapOr`'s fallback         | 11 ms, 3 %           | [F209](../findings.md) |
| The map of countries and the sets of users                          | 76 ms, 24 %          |                        |
| … of which only the `add` of a user to the set of their country     | 64 ms, 20 %          | [F215](../findings.md) |
| The map of products, `get` then `set` of an integer key             | 22 ms, 7 %           | [F216](../findings.md) |
| The check of the country's letters                                  | 23 ms, 7 %           | [F207](../findings.md) |
| The daily revenue, an array indexed by day                          | 9 ms, 3 %            |                        |
| A second copy of the country, which the row could lend              | 7 ms, 2 %            | [F214](../findings.md) |
| Reading at all: the text passed in, a summary returned              | 8 ms is all it costs |                        |

The lines alone are walked in 22 ms, and parsed and filtered in 133.

**What each kept row does.** The same dashboard was built with the runtime beside the module (`--link off`), and every call into it counted, on 100,000 rows. Each row kept calls the runtime about 40 times. It copies a string 8 times and releases a block 13 times. Each integer is read by two calls: `str_is_int`, then `str_parse_int`. The map of pairs is the main cause. `let pair = pairs.get(key).unwrapOr(…)` copies the entry's `Pair` out of the map, its country included. Then `pairs.set(key, pair)` looks the key up again, releases the key passed and the value it replaces, and stores the copy back.

**What did not explain it.** The `add` of a user costs 64 ns. SipHash is not the reason. The runtime was rebuilt with a multiplication instead of SipHash for integer keys, and the dashboard took 342 ms instead of 322: no faster (F215). The sets are large, up to some 100,000 users per country. A lookup reads the index, then the entry it points to, and both are out of the cache. The optimized TypeScript keeps a bitmap of 32 KB per country instead. It knows from the data that users are below 2^18, which a compiler cannot know.

## The techniques

**1. Update an entry in place** (F216). Lorenzen and Leijen, _Reference Counting with Frame Limited Reuse_, ICFP 2022, and Reinking, Xie, de Moura and Leijen, _Perceus_, PLDI 2021, compile a value that is taken apart and rebuilt as an update of the same memory, when nothing else holds it. Morel and Renvoise, 1979, already applied in decision 0016, remove the second lookup. The pattern here is: `let v = m.get(k).unwrapOr(d)`, then changes to `v`, then `m.set(k, v)`, with no other use of `m` between. The change: the IR recognizes it and changes the entry where it is. It makes one lookup, and puts `d` in only when `k` is missing. Nothing is copied out of the map, and neither the key nor the old value is released. Rust's standard library offers the same as an API, `HashMap::entry`; here the compiler finds it in code written plainly. **Expected**: most of the 96 ms of the pairs, less the 27 of the key, and a part of the 22 of the products.

**2. Do not build a key that is only looked up** (F202). Wadler, _Deforestation: Transforming Programs to Eliminate Trees_, Theoretical Computer Science 73(2), 1990, removes the intermediate structures a program builds only to consume them. A key made from a template, `` `${country}|${category}` ``, is such a structure when it is found. The change: its parts are hashed one after the other, as the bytes of the whole would be. SipHash takes its input in pieces (Aumasson and Bernstein, INDOCRYPT 2012). The key is compared with the entry part by part, and it is built only when it is inserted. **Expected**: most of the 27 ms.

**3. A quiet fallback may copy** (F209). Decision 0016 makes `unwrapOr`'s fallback only when the option is empty, if making it has no effect a program can see. A copy of a value has none: only an allocation, which the program cannot observe. The change: `x.copy()` of a quiet value is quiet. **Expected**: the 11 ms, and a block per row.

**4. Read an integer with one call** (parse). `parseI32` asks the runtime whether the text is an integer, then asks it for the integer: two passes over the same bytes. The change: one call returns both, the value and whether it is one. **Expected**: a part of the 111 ms of parsing; small, measured.

**5. Fewer misses in large sets** (F215). Kulukundis, _Designing a Fast, Efficient, Cache-friendly Hash Table, Step by Step_, CppCon 2017 (Abseil's Swiss tables), keeps a few bits of each hash in the index, so that most comparisons do not read the entry. Celis, _Robin Hood Hashing_, PhD thesis, University of Waterloo, 1986, keeps probes short. Peras's maps keep the order of insertion, as Python's do since 3.6: entries in order, and an index of numbers. The change: the index keeps the hash's high bits next to the entry's number, so that a probe on another key does not read its entry. This is a guess, not a measure: a lookup that finds its key still reads its entry. **Expected**: uncertain. It is tried as a prototype, measured, and kept only if the profile shows a gain.

## Order

By what the profile shows, and by risk, each measured before the next:

1. A quiet fallback that copies (3): small, and the differential test covers it.
2. Update in place (1): the largest.
3. A key not built (2).
4. One call per integer (4).
5. The index with the hash's bits (5), as a prototype.

If steps 1 to 4 each give most of what the profile shows, the dashboard goes from about 322 to about 200 ms per 1,000,000 rows in Node. Peras would then be about 0.45 times as fast as TypeScript optimized by hand, against 0.25 to 0.28. That is a hope, not a result: a profile by removal gives the most a change can save (finding M013). Parity is not the aim. The optimized TypeScript uses what it knows of the data: 676 country codes, users below 2^18. A compiler cannot know that from code written plainly.

## What does not change

- The protocol of 0014, its versions A, B and C, its data and its thresholds. `dashboard.peras` is not rewritten to win: the gain must come from the compiler.
- Safety. Every change keeps the debug allocator's checks, the release verifier, the tests of both allocators, and the differential test: 2,000 seeds once, and once with the change broken on purpose (M015). The differential test's generator gains maps of structures updated by `get` then `set`, and keys built from templates.
- SipHash stays: it protects maps against keys chosen to collide, and it is not what costs here (F215).

## How the optimized TypeScript gets its speed

`node scripts/profile-optimized.mjs` profiles version B the same way: each variant of `bench/data/dashboard-optimized.ts` replaces one of its techniques by what plain code does. The figures are for 1,000,000 rows in Node, the median of 9 rounds, at commit `037bb74`.

| B, with one technique replaced                                       | Time   | Adds   |
| -------------------------------------------------------------------- | ------ | ------ |
| B as written                                                         | 106 ms |        |
| Every user in a `Set` per country, instead of a bitmap               | 186 ms | 80 ms  |
| The pairs in a `Map` keyed by a string built per row                 | 133 ms | 26 ms  |
| The pairs in a `Map` keyed by a number                               | 109 ms | 3 ms   |
| Every product in a `Map`, instead of a dense array                   | 111 ms | 5 ms   |
| All three: B's reading of the bytes, with plain code's maps and sets | 236 ms | 130 ms |
| For comparison, `TextDecoder` alone, the bytes into C's string       | 18 ms  |        |

**What it says.** B is fast for two reasons, of unequal size.

- **Most of its lead is knowledge of the data, about 130 ms of 236.** B knows three things that the code does not say:
  - users are below 2^18, so a bitmap of 32 KB per country holds them;
  - a country is two capital letters, so it is a number below 676;
  - products are below 8,192.

  The bitmap alone is worth 80 ms. A compiler cannot find these facts in code written plainly. The same `Set` in JavaScript costs more than in Peras: 80 ms added in V8, against 48 to 64 for Peras's sets (F215).

- **The rest is reading the bytes in one pass, about 70 ms.** B with plain structures takes 236 ms; Peras takes 304 (304 to 322, above). B walks the bytes once: it reads each integer as it meets its digits, and it allocates nothing per row. Peras:
  - splits each line into views;
  - asks the runtime twice for each integer;
  - checks the country letter by letter;
  - and, in decision 0014's measure, is given a string that `TextDecoder` made from the bytes, 18 ms that B does not pay.

  This part is the compiler's. Steps 2 to 4 aim at it, and one more technique fits it: reading the fields of a line straight from its bytes, without its views (Langdale and Lemire, 2019, already cited in decision 0016).

So Peras written plainly could catch up with B written with plain structures. It will not catch up with B's bitmap by compiling alone. Reaching that would take a way, in the language, for a program to state what it knows: for example a set of integers below a bound, kept as a bitmap. That is a change to the language, and it is not part of this decision. It would be proposed separately, its cost on the language weighed, and measured like the rest.

## Measured after step 1

Step 1 landed at commit `037bb74`. A fallback of `unwrapOr` that copies a value read is now quiet, so it is made only when the option is empty (F209). It is correct and tested. The differential test gains a map of strings, fallbacks that copy and a fallback that moves a variable. 2,000 seeds pass. With the lazy fallback of a buffer broken on purpose, the test fails at the first seed. The end-to-end tests of both allocators pass, 116 of 116.

It is taken: counting the runtime's calls, each row kept copies a string 7 times instead of 8 and releases a block 12 times instead of 13. In the profile, the variant without the copy no longer saves anything (−1 ms). The dashboard went from 322 to 304 ms per 1,000,000 rows in Node. That is more than the 11 ms the profile gave, but the variants spread by about 10 ms (one shows −10), so the gain is between 8 and 28 ms.

## Measured after step 2

Step 2 landed at commit `02ff9c2`, in two parts, each its own small change.

- **`m.get(k).unwrapOr(fallback)` lowered as one thing**, when the option is only a step. One lookup; each scalar of the value read only when the entry is there; a buffer copied once; the fallback made only when the key is missing. Before, the option had an empty buffer made for a missing value, and its string was copied twice. A fallback that is not quiet still runs first, after the entry is read, as written.
- **A `set` of a key the lookup pass knows is given the entry.** `map_put_at_i`, `map_put_at_s` and `map_put_at_s_lent` release the old value and the key without looking the key up again. The pass gives one only where the map and the key cannot have changed since the lookup: the same rule as decision 0016's. An integer key widened from an `i32` local is now known to it too.

**What guards it.** Each guard is meant to fail if the step is wrong:

- The runtime checks that the entry it is given holds the key. When it does not, it looks the key up as `set` always did, so a wrong entry costs time, never a wrong map. The runtime's own tests give it wrong entries on purpose, on both allocators.
- The interpreter of the IR traps on a wrong entry, so a wrong proof of the pass fails the differential test.
- The differential test gains maps of structures taken out, changed and put back, a fallback that is not quiet, and a key or a map changed between the lookup and the set. 2,000 seeds pass. Three mutations each fail it:
  - the pass not forgetting a map's changes;
  - any known entry of the map given to a `set`, whatever its key;
  - the entry's value read before the choice, when it may be missing.
- An end-to-end test changes entries of structures in place across the JavaScript boundary on both allocators, a key deleted in between included. The end-to-end tests pass, 118 of 118.

**It is taken.** Counting the runtime's calls (`node scripts/profile-data.mjs --calls`), each row kept:

- releases a block 10 times instead of 12;
- copies a string 6 times instead of 7;
- makes one empty buffer fewer;
- looks each key of the pairs and of the products up once, with `map_put_at_s` and `map_put_at_i` in place of `map_put_s` and `map_put_i`.

**The gain: 8.5 to 13.5 %.** Two profiles run an hour apart could not show it. `full` went from 304 to 310 ms, but the variants this step does not touch slowed down as much: the walk over the lines alone went from 21 to 24 ms. The machine had slowed between the two (finding M017). So the two compilers were compared in one process with `node scripts/ab-data.mjs 037bb74 02ff9c2`: the same text summarized by each in turn, the order alternating, and the ratio taken round by round. Over four such runs, before / after was 1.085, 1.097, 1.104 and 1.135 at the median. The summaries were the same.

## Measured after step 3

Step 3 landed at commit `a497971`, in a form other than the one planned. Before building the key in pieces (technique 2), a profile by removal asked what the key costs. `` `${country}|` `` instead of `` `${country}|${category}` `` saved 25 ms of 32. That variant pointed at the integer: `${category}` was made a string of its own, copied into the key, then released.

**The change.** An integer in a template is evaluated in its place:

- `fmt_int_len` counts its bytes, so that the template's string is sized once;
- `fmt_int_append` writes its digits into that string.

So every template does one allocation and one release fewer for each of its integers. This is general, not only for keys, and smaller than technique 2.

**What guards it.**

- An end-to-end test compares templates of `i32` and `i64` at their bounds with JavaScript's own templates, on both allocators.
- The differential test gains templates of negative, zero and wide integers. 2,000 seeds pass.
- Two mutations each fail it: a digit miscounted, and the sign written wrong. The end-to-end test catches the sign too.

**It is taken.** In `--calls`, `fmt_int` is gone, and each row kept releases a block 9 times instead of 10.

**The gain: 1.3 to 7.4 %, less than the profile gave.** `node scripts/ab-data.mjs f989a03 a497971` gave a median before / after of 1.074, 1.034 and 1.013 over three runs, the same summaries.

The variant had changed two things, which is finding M013 again. Without the category, the map held one key per country instead of up to twelve: a smaller map, more often in the cache. The 25 ms were the integer's cost and the smaller map's together, and only the first is what this step removes. Technique 2, the key not built at all, is not pursued now. The key is built once per row kept, and building it is now one allocation.
