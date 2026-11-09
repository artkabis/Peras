# 0004: Text without an allocation per piece: views, then fusion, chosen by the compiler

_French translation: [../../fr/decisions/0004-strings-without-allocation.md](../../fr/decisions/0004-strings-without-allocation.md)._

## Context

The `words` kernel of the [browser benchmark](../../../bench/README.md#browser-benchmark-benchbrowser) counts the words of a text of one million characters:

```peras build
export fn words(text: String): i32 {
  return text.split(" ").filter((w) => w.length > 0).length;
}
```

It was the slowest kernel against JavaScript. Measured step by step in Chromium, its 31 ms went:

| Step                                     | Time  |
| ---------------------------------------- | ----- |
| the text crossing into the module, UTF-8 | 4 ms  |
| `split`, about 200,000 pieces            | 14 ms |
| `filter`, copying each piece kept        | 13 ms |

The boundary was not the cost: allocation was, a block and a release for every piece. Two changes since then took the kernel to 17 ms, twice JavaScript's time: `filter` keeps the elements of a temporary array in place instead of copying them, and a short string is one block instead of two. What remains is one block per piece.

## What others have done

- **Fusion.** _Deforestation_ (Wadler, TCS 1990) and _Stream fusion: from lists to streams to nothing at all_ (Coutts, Leshchinskiy and Stewart, ICFP 2007) remove the intermediate structures of a chain of operations. Rust does it by hand: `text.split(' ').filter(|w| !w.is_empty()).count()` allocates nothing, since `split` gives a lazy iterator.
- **Views.** The pieces that Rust's `split` gives are `&str`, an address and a length into the text, whose lifetime the borrow checker proves (_Oxide: The Essence of Rust_, Weiss et al., 2019). Umbra's strings (Neumann and Freitag, CIDR 2020), then DuckDB, Velox and Apache Arrow's `StringView`, keep up to 12 bytes inline and a prefix and an address otherwise.
- **Reuse.** _Perceus_ (Reinking, Xie, de Moura and Leijen, PLDI 2021), _Counting Immutable Beans_ (Ullrich and de Moura, IFL 2019) and _FP²: Fully in-Place Functional Programming_ (Lorenzen, Leijen and Swierstra, ICFP 2023) rewrite in place a value nobody else holds. `filter` on a temporary array is a case of it.
- **Regions.** _Region-Based Memory Management_ (Tofte and Talpin, Information and Computation, 1997), Cyclone (Grossman et al., PLDI 2002) and Rust's arenas allocate by moving a pointer and release a whole region at once.

In Rust, the developer chooses: `String` or `&str`, a `Vec` or an iterator, and writes the lifetimes. Peras's compiler already knows who owns each value, how long each lives, and which temporaries each statement makes. It can choose the representation itself, with nothing written in the program.

## Measurements

Before deciding, the two paths were measured where they would end, with functions written by hand in the runtime of a scratch build, in Chromium, the text crossing in included, 30 runs after 3 of warm-up, three times:

| Variant                                       | Time            | Against JavaScript   |
| --------------------------------------------- | --------------- | -------------------- |
| `words` today                                 | 19 to 21 ms     | 2 times slower       |
| **fused**: no array at all                    | 7.3 to 7.6 ms   | **1.4 times faster** |
| **views**: one array of (start, length) pairs | 7.8 to 8.0 ms   | **1.3 times faster** |
| JavaScript                                    | 10.4 to 10.8 ms | 1                    |
| of which the text crossing in                 | 4.1 to 4.4 ms   | —                    |

Every variant found the same 195,889 words, and left nothing alive.

## Decision

1. **Views first.** A piece of text that `split`, `slice` or `trim` gives, and that does not outlive the text it comes from, is a view: a start and a length in that text, not a copy. An array of such pieces is one buffer of pairs. The views come within 0.5 ms of fusion, and they are more general: the array exists, so it can be indexed, kept for a while, passed to a function.
2. **The compiler chooses, the program does not change.** A view is never written in a program and never seen from JavaScript. The compiler uses one where it proves both that:
   - the text it comes from lives as long as the view, and does not change meanwhile; the borrow rules already give this, as for any value lent;
   - the view does not leave that lifetime: it is not returned, given to a `sink` parameter, stored in a structure or an array that lives longer, nor crosses to JavaScript.

   Anywhere else, the piece is copied at that point, as today: the copy moves from where the piece is made to where it escapes, and only for the pieces that do.

3. **Then fusion.** A chain whose producer is `split`, `map` or `filter` and whose consumer is `length`, `reduce`, `some`, `every`, `find`, `findIndex` or `for…of` becomes one loop with no array between. Fusion changes the order of the functions' calls (`f` then `g` on each element, instead of `f` on all then `g` on all), so it applies only when that cannot be seen: every function of the chain but the last cannot trap and calls nothing, the condition the optimizer already uses to move code ([`is_simple`](../../../crates/perasc-ir/src/opt.rs)). Otherwise the chain stays as written.
4. **Results are always those of the program as written.** As for the other optimizations, tests compare both forms on every input, and are broken on purpose to show they catch a wrong one.

## Status

**Views of `split` are done.** Where the array that `text.split(sep)` makes is read at once, by a method (`filter`, `map`, `length`, `join`, `some`…) or a `for…of`, its parts are views:

- One block holds a 16-byte header per part, whose bytes are those of the text, and the handles to them. A view is an ordinary string header: every operation on strings reads it unchanged, and a copy of it, of the array (`copy`, `slice`, `concat`) or of a part, is an ordinary string.
- When an argument of that method, or the body of that loop, names the variable the text comes from, which could then change it, the array is copied first. A `filter` whose result leaves the statement gives a copy too.
- A separator of one byte, the usual case, is split by two plain loops over the bytes.
- `push` now calls the runtime only when the buffer is full, for every array.

Tests run the WebAssembly on the debug allocator, which poisons released bytes, and check every result against Rust's `split`, the text changing while its parts are in use included. Taking away either copy, or the conversion of a copied array, makes them fail; they needed texts over 256 bytes to see it, since a shorter one keeps its old bytes in its header's block when it grows.

`words` went from 17 to 11.2 ms, against 10.3 for JavaScript; in the benchmark, from 0.51 to 0.87 times JavaScript's speed. Of the rest, 4.2 ms is the text crossing in, 4.8 the split and 2.3 the filter.

**Next:** views for `slice` and `trim`, then fusion. Searching for the separator sixteen bytes at a time with SIMD would take about 1.5 ms more off, but the runtime is in every module that uses strings or arrays: every such module would need WebAssembly SIMD (Safari 16.4 and later), not only those with a vector loop. That choice is left open.

## Not chosen now

- **Regions per statement.** The temporaries of a statement could be allocated by moving a pointer and released at once at its end; Peras knows them exactly, which Rust cannot. It changes the memory model, and the views take most of its gain on this workload: it will have its own decision, with the `@frame` rule of [decision 0003](0003-runtime-proved-at-compile-time.md).
- **Short strings inline in their handle**, as Umbra's. It changes how every string is represented and crosses to JavaScript, for a gain the views already give here.

## Consequences

- Once the pieces cost nothing, the text crossing into the module is more than half of what remains (4 ms of 7.5). The next step after this decision measures the two ways to shorten it: `TextEncoder.encodeInto` writing straight into the module's memory, one pass instead of two, and WebAssembly's JavaScript string builtins, which would let the module read the text without copying it.
- The search for a separator, a byte at a time, is the rest. Searching sixteen bytes at a time with WebAssembly SIMD, as [the vector loops](../guide/javascript.md#vector-loops) do, comes with it.
- The roadmap of phase 3 points here; the benchmark's README says which step brought what, measured.
