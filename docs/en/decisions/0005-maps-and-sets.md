# 0005: Maps and sets: in insertion order, hashed with a secret key

_French translation: [../../fr/decisions/0005-maps-and-sets.md](../../fr/decisions/0005-maps-and-sets.md)._

## Context

Phase 3 of the [roadmap](../reference/roadmap.md#phase-3-a-standard-library) brings `Map<K, V>` and `Set<T>`. Web programs count words, index records by their identifier, and remember what they have already seen. In JavaScript, `Map` and `Set` give back their entries in the order they were inserted, and a TypeScript developer expects that.

A hash table answers three questions, and each one has a cost:

- **In what order does it give back its entries?** Insertion order, or the order of the slots, which depends on the hash.
- **With which hash?** A fast one, or one that resists keys chosen to collide.
- **What does it hold, and for how long?** Peras has owners, loans and moves; a map must follow those rules.

## What others have done

- **Insertion order.** CPython's compact dict (Raymond Hettinger, 2012; CPython 3.6, guaranteed since 3.7) keeps its entries in a dense array, in insertion order, and finds them through a sparse index of small integers. JavaScript's `Map` and `Set` give back insertion order too, by specification.
- **Swiss tables.** Abseil's `flat_hash_map` (Benzaquen, Evlogimenos, Kulukundis and Perepelitsa, CppCon 2017) and Rust's `hashbrown` keep one control byte per slot, compared sixteen at a time with SIMD, and the entries in the slots: no indirection, and no order.
- **Hash flooding.** Crosby and Wallach (_Denial of Service via Algorithmic Complexity Attacks_, USENIX Security 2003) showed that keys chosen to collide turn every operation into a scan. Klink and Wälde (28C3, 2011) did it to the web frameworks of the day with the parameters of one request (CVE-2011-4885 for PHP, among others). Aumasson and Bernstein (_SipHash: a fast short-input PRF_, INDOCRYPT 2012; _Hash-flooding DoS reloaded_, 29C3, 2012) found collisions in MurmurHash and CityHash that do not depend on their seed, so a secret seed did not save them. SipHash is a keyed function with a security proof. Python uses it since 3.4 (PEP 456), and Rust uses SipHash-1-3 with keys drawn at random.

## Measurements

Before deciding, two layouts and three hashes were written in Rust, compiled to WebAssembly with SIMD, the instructions the Peras runtime would use, and run in Chromium next to JavaScript's `Map`. Each variant gave exactly the same results as `Map`. The prototypes are not committed: they were written to decide.

- **Ordered** is CPython's layout: an index of 4-byte integers at most two thirds full, with linear probing, and a dense array of entries (hash, key, value) in insertion order.
- **Swiss** is hashbrown's: one control byte per slot, 16 compared at once, and the entries in the slots.
- **SipHash-1-3** is keyed with 128 bits. **Multiplicative** is keyed too, made of multiplies and shifts, as fast hashes are. **FxHash** has no key, as in the Rust compiler.

**1 million integer keys** (insert, find each, look for a million that are not there, delete half), median of 7 runs, in ms:

| Layout and hash         | Random keys | Identifiers, steps of 4096 |
| ----------------------- | ----------: | -------------------------: |
| JavaScript `Map`        |   673 – 840 |                        601 |
| Ordered, SipHash-1-3    |   249 – 266 |                        271 |
| Ordered, multiplicative |   165 – 191 |                        181 |
| Ordered, FxHash         |   159 – 180 |                 **18,182** |
| Swiss, SipHash-1-3      |   181 – 190 |                        182 |
| Swiss, multiplicative   |   112 – 124 |                        119 |
| Swiss, FxHash           |     86 – 90 |                        197 |

**Counting the words of a 1 MB text** (19,627 different words), in ms: JavaScript's `Map` and `split`, 21.6 to 29.1; ordered with SipHash-1-3, 8.3 to 11.9; Swiss with SipHash-1-3, 7.5 to 7.6; Swiss, multiplicative, 6.2 to 6.3. The module's figures do not count the text crossing into it, 0.7 ms per million characters since [decision 0004](0004-strings-without-allocation.md).

**Memory** held for `i32` keys and `i32` values, the capacities of the arrays: ordered, 25 to 31 bytes per key; Swiss, 12 to 19.

Four facts come out of these measurements:

1. **A hash without a key collapses on real data.** FxHash with linear probing took 18 seconds on identifiers in steps of 4096, a hundred times longer, because its low bits did not vary. An earlier run of these measurements, with keys that a faulty generator had made repetitive, had shown the same collapse.
2. **Insertion order costs 1.4 to 1.5 times on a million integers**, and 10 to 20 % on words, which fit in the cache. That cost is the index: a found key is read in two places. Keeping the hash in the index, so that a slot that does not match is never followed, was measured too: no gain (263 against 266 ms).
3. **SipHash-1-3 costs about 40 % more than a multiplicative hash.**
4. **Every variant is faster than JavaScript's `Map`**: the slowest, ordered with SipHash-1-3, is 2.2 to 3.2 times faster on integers and 2.4 to 2.6 times on words. That is JavaScript's `Map`, as usually used, not a table written by hand over typed arrays, as an expert would for integers: the [browser benchmark](../../../bench/README.md#browser-benchmark-benchbrowser) showed such twins narrowing every gap, so these ratios are a ceiling, to measure again against an expert's table before they are quoted.

## Decision

1. **Insertion order, in CPython's layout.** A `Map` or a `Set` gives back its entries in the order they were inserted, as in JavaScript. Removing an entry leaves a hole in the entries, removed when the table grows. Iterating reads the dense array and steps over the holes.
2. **SipHash-1-3, with a secret key drawn when the module loads.** The loader draws 128 bits with `crypto.getRandomValues` for each instance of the module. Since the order of the entries does not depend on the hash, the key never shows in what a program computes or prints: the same program gives the same result on every run, and keys chosen to collide cost no more than other keys. Each entry keeps 32 bits of its hash, compared before the key, and used again when the table grows.
3. **Keys: integers, booleans and `String` first.** A float is not a key yet: JavaScript compares keys with SameValueZero, where `NaN` is equal to itself and `-0` to `+0`, and that rule will be decided with a use for it. Structures and enumerations come later, with an interface that says how to hash them and when two are equal.
4. **The map owns what it holds.** `m.set(k, v)` takes its key and its value, as `push` does. `m.get(k)` lends its key and gives an `Option<V>`, read as an element of an array is read. `m.has(k)`, `m.delete(k)` (which says whether the key was there), `m.size` and `m.clear()` complete it, under the names JavaScript uses. `for (const entry of m)` lends each entry as a tuple `(key, value)`; changing the map in the loop is refused, as for an array. `Set<T>` is the same table without values: `add`, `has`, `delete`, `size`, `for…of`. A map is made with `Map.new()`, its types written on its variable.
5. **Across the boundary, a copy.** An exported function takes and gives a JavaScript `Map` or `Set`, copied in insertion order, both ways; TypeScript sees `Map<string, number>`. A map that stays in the module's memory, as [`PerasArray`](../guide/javascript.md) does for arrays, waits for a measure that calls for it.
6. **Checked like the rest.** The tables are functions of the runtime, written once for both allocators, with a model in Rust for the interpreter. Tests run random sequences of `set`, `delete`, `get` and iteration on both sides, against JavaScript's `Map`, entry by entry and in order, and are broken on purpose to show that they catch a wrong table.

```peras planned
export fn counts(text: String): Map<String, i32> {
  let seen: Map<String, i32> = Map.new();
  for (const word of text.split(" ")) {
    if (word.length > 0) {
      seen.set(word.copy(), seen.get(word).unwrapOr(0) + 1);
    }
  }
  return seen;
}
```

## Not chosen now

- **Swiss tables.** They are 1.4 to 1.5 times faster on a large table and use half the memory for small entries, but their order depends on the hash, hence on the secret key: a program would give back its entries in a different order on each run. The compiler can later give a map that is never iterated nor passed to JavaScript, where the order cannot be seen, the Swiss layout. That requires knowing where each map goes, and the measured gain says when it is worth it.
- **A multiplicative hash by default.** It is 40 % faster, but such hashes have had collisions that no seed prevents; keys come from the network as often as from the program. A faster hash for keys the compiler proves come from the program itself may follow, measured.
- **FxHash, and any hash without a key.** Measured: a hundred times slower on ordinary identifiers.

## Status

**Done.** `Map<K, V>` and `Set<T>` are compiled for integer, boolean and string keys, with values of any type, in four steps:

1. **The table** is the runtime's, in WebAssembly, written once for both allocators: a 32-byte header, then entries of 16 bytes and 8 per value slot, in insertion order, and an index of slots that holds 0 for empty, −1 for a removed entry, `e + 1` otherwise. SipHash-1-3 is checked against the vector of its paper. The loader draws the 128-bit key with `crypto.getRandomValues` for a module that has maps, and only then.
2. **`for…of`** gives the live entries in insertion order, each lent as `(key, value)`; a body that changes the map is refused.
3. **Values of any type** are kept as their scalars, one 64-bit slot each, as an array of structures keeps one column per scalar. The map is made with its shape, the number of slots and a bit per slot that owns a buffer, and releases those buffers when a key is set again, deleted, cleared, or the map released. `get` copies a value that owns buffers; `for…of` lends it. A map can be held by a structure, an array, an option or another map; the runtime copies it deeply with what holds it, and `m.copy()` gives a map of its own.
4. **Across the boundary**, an exported function takes and gives a JavaScript `Map` or `Set`, copied in insertion order; TypeScript sees `ReadonlyMap<string, number>` for a parameter the function only reads.

Tests run random operations three ways, against Rust's maps or vectors in insertion order: compiled to WebAssembly on the debug allocator, in the IR interpreter, and in Chromium against JavaScript's `Map` and `Set` on both allocators, with no block left alive. Each step was broken on purpose to show the tests catch it: a removed entry not marked, a `bool` key widened wrongly, a removed entry walked, a value not released on delete, the wrong slot read, a map copied as a plain buffer.

Not done: keys that are structures, enumerations or floats, which need a rule for hashing and equality; a `get` that lends instead of copying, which needs lending a value out of an expression; and Swiss tables for maps whose order cannot be seen.

## Consequences

- The collections row of the [roadmap](../reference/roadmap.md#phase-3-a-standard-library) links here; `Map` and `Set` come next, for integer, boolean and string keys, then `Deque`.
- The loader draws the key once per instance: one call to `crypto.getRandomValues`, available in every browser, in Node and in workers.
