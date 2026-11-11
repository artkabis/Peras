# 0016: Making the compiler faster, from what was measured and what is published

_French translation: [../../fr/decisions/0016-faster-from-the-literature.md](../../fr/decisions/0016-faster-from-the-literature.md)._

_Status: proposed on 2026-11-11. Each change is measured when it lands, by a new run of [decision 0015](0015-stream-measured.md)'s protocol, unchanged; a change that does not show its gain there is reported as such._

## Context

Decision 0015 measured the stream app's digest under a registered protocol: Peras is 1.5 to 1.7 times as fast as TypeScript as written, but 0.87 to 0.96 times as fast as TypeScript written for speed. A profile by removal (`node scripts/profile-stream.mjs`) then found where the time goes, per 1,000,000 rows, of about 363 ms:

| Step                                                          | Cost                      | Finding                |
| ------------------------------------------------------------- | ------------------------- | ---------------------- |
| Checking a zone's letters with `for (const c of s.split(""))` | 90 to 98 ms               | [F207](../findings.md) |
| Splitting each line into an array of fields                   | 40 to 58 ms               | [F208](../findings.md) |
| Returning `Option<Reading>`, and `unwrapOr`'s fallback        | 35 to 55 ms               | [F209](../findings.md) |
| `has` then `get` on a map keyed by strings                    | 41 to 44 ms               | [F210](../findings.md) |
| Walking the lines                                             | about 33 ms               |                        |
| Reading the numbers                                           | about 30 ms in all, noisy |                        |

None of these is a cost of WebAssembly: the boundary is 5 % (0015, H4). Each is work the compiler makes the program do that the program did not ask for: an array it never keeps, a block for a value that lives a moment, a hash computed twice. Compilers have known how to remove each of them for decades. This decision lists, for each, the published technique, how it would apply to Peras, and what it should save.

## The techniques

**1. Fuse the walk over characters with its consumer** (F207). Coutts, Leshchinskiy and Stewart, _Stream Fusion: From Lists to Streams to Nothing at All_, ICFP 2007, remove the intermediate list between a producer and its consumer by turning both into a stream and fusing them. Peras already does this for `for (const part of text.split(sep))` with a literal separator (F206), not for the empty one. The change: the same streaming for `split("")`, one character at a time in one view, and `charCodeAt(i)` so that a class of characters is checked on an integer, with no string at all. **Expected**: most of the 90 ms; the check becomes a loop over bytes.

**2. Find the redundant lookup and do it once** (F210). Morel and Renvoise, _Global Optimization by Suppression of Partial Redundancies_, Communications of the ACM 22(2), 1979, remove a computation done twice on a path. Here the computation is the lookup of one key: `m.has(k)` then `m.get(k)`, or `m.get(k)` then `m.set(k, …)`, with no change to `m` or `k` between. The change: the IR recognizes the pair and keeps the slot found by the first. SipHash stays (Aumasson and Bernstein, INDOCRYPT 2012): it protects against keys chosen to collide, and the change halves how often it runs. **Expected**: about half of the 41 to 44 ms.

**3. Do not allocate what does not escape** (F208). Choi, Gupta, Serrano, Sreedhar and Midkiff, _Escape Analysis for Java_, OOPSLA 1999, find the objects that do not outlive the method that makes them and allocate them on its stack. Tofte and Talpin, _Region-Based Memory Management_, Information and Computation 132(2), 1997, infer where memory can be freed all at once. Reinking, Xie, de Moura and Leijen, _Perceus: Garbage Free Reference Counting with Reuse_, PLDI 2021, reuse a block freed just before an allocation of the same size. The change: an array of views made in a loop's body and not leaving it reuses one block from one iteration to the next. **Expected**: most of the 40 to 58 ms.

**4. Return small values in values** (F209). The same escape analysis applies to a function's result: an `Option` of a structure that the caller unwraps at once need not be a block of the heap; WebAssembly's multiple results can carry it. And `unwrapOr(fallback)` evaluates its fallback only when the value is missing, as `??` does in JavaScript, which F104 asks for. **Expected**: most of the 35 to 55 ms.

**5. Scan many bytes at once** (lines, fields, numbers). Langdale and Lemire, _Parsing Gigabytes of JSON per Second_, The VLDB Journal 28(6), 2019, find the structural characters of a text with SIMD instructions, many bytes per instruction. WebAssembly has 128-bit SIMD, which Peras already uses in loops. The change: `split` and the streamed walk look for their separator 16 bytes at a time. Lemire, _Number Parsing at a Gigabyte per Second_, Software: Practice and Experience 51(8), 2021, parses decimals exactly without the slow path in most cases; Peras's `parseF64` uses Clinger's fast path (_How to Read Floating Point Numbers Accurately_, PLDI 1990) and asks the host beyond it. **Expected**: a part of the 33 ms of lines and of the numbers; smaller, and later.

## Order

By gain over cost, each measured before the next:

1. Streamed characters and `charCodeAt` (technique 1).
2. One lookup for `has` or `get` then `get` or `set` of one key (technique 2).
3. Reuse of a loop body's array of views (technique 3).
4. `Option` of a structure returned in values, and a lazy `unwrapOr` (technique 4).
5. SIMD search for separators (technique 5).

If 1 to 4 each give most of what the profile shows, the digest goes from about 363 to about 200 ms per 1,000,000 rows: faster than the TypeScript written for speed (B, about 418 ms at 100,000 rows in 0015). That is a hope, not a result: a profile by removal gives the most a change can save, and the TypeScript engines improve too. Each run of 0015 says how much of it holds.

## What does not change

- The protocol of 0015, its versions A and B, and its thresholds. The app's Peras program is not rewritten by hand to win: the gain must come from the compiler, for every program that is written plainly.
- Safety: every change keeps the debug allocator's checks, the release verifier and the tests of both modes. A block reused is still a block the verifier can account for.
