# 0017: Aggregating without copies, from a profile of the dashboard

_French translation: [../../fr/decisions/0017-aggregating-without-copies.md](../../fr/decisions/0017-aggregating-without-copies.md)._

_Status: proposed on 2026-11-12. Each change is measured when it lands: first by the profile below, in Node, then by a new run of [decision 0014](0014-data-at-scale.md)'s protocol, unchanged. A change that does not show its gain there is reported as such._

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
