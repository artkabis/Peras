# The research behind Peras

_French translation: [../fr/research.md](../fr/research.md)._

Peras's design choices rest on published, peer-reviewed work. This page lists that work: for each choice, the paper, what it shows, what it does not show, and where Peras applies it. It also lists the research that argues against parts of the project, because that research sets its limits as much as the rest sets its design.

## How to read this page

- **A paper supports a choice; it does not prove that Peras is right.** Each paper studied its own subject: Rust, JavaScript, an allocator for real-time systems. What Peras claims about itself rests on its own tests and benchmarks, linked from each entry.
- **Each reference was checked** against the publisher's record or the authors' page: authors, title, venue, year, DOI where there is one. Every result quoted here comes from the paper's abstract or its published summary. No figure is given that was not read there.
- **Research against the project is listed too**, in [adoption and its risks](#adoption-and-its-risks).

## Memory safety without a garbage collector

**Ownership can be made sound.** Jung, Jourdan, Krebbers and Dreyer, _RustBelt: Securing the Foundations of the Rust Programming Language_, POPL 2018 ([DOI 10.1145/3158154](https://doi.org/10.1145/3158154)).

- **Shows**: the first formal, machine-checked proof of safety for a realistic subset of Rust, and the conditions a library using `unsafe` must meet to remain safe.
- **Does not show**: anything about Peras's checker. Peras follows mutable value semantics, not Rust's references, and has no formal proof yet. Its checks are tested by UI tests, a verifier of the IR that checks every release, the debug allocator and a fuzz target.
- **In Peras**: `inout`, `sink` and the ownership checks ([memory](guide/memory.md), [architecture](architecture.md)).

**Values without references can compile to efficient code.** Racordon, Shabalin, Zheng, Abrahams and Saeta, _Implementation Strategies for Mutable Value Semantics_, Journal of Object Technology 21(2), 2022.

- **Shows**: strategies to compile mutable value semantics, studied on a core of Swift, into efficient native code: fixed-size values on the stack, where ordinary optimizations apply, and copy-on-write for containers.
- **Does not show**: the speed of Peras, which is measured instead ([benchmarks](../../bench/README.md)).
- **In Peras**: the reason why no lifetime is written ([spec](spec.md)).

## Static checks catch bugs

Gao, Bird and Barr, _To Type or Not to Type: Quantifying Detectable Bugs in JavaScript_, ICSE 2017 ([DOI 10.1109/ICSE.2017.75](https://doi.org/10.1109/ICSE.2017.75)).

- **Shows**: on public bugs of JavaScript projects, TypeScript 2.0 and Flow would each have reported about 15 % before they shipped, between 11.5 % and 18.5 % at 95 % confidence.
- **Does not show**: that stricter rules, such as Peras's, pay back their cost in learning. No study measures that for Peras.
- **In Peras**: no implicit conversion, no `any`, exhaustive `match`, checked in both modes.

## WebAssembly as a target

**Why WebAssembly.** Haas, Rossberg, Schuff, Titzer, Holman, Gohman, Wagner, Zakai and Bastien, _Bringing the Web up to Speed with WebAssembly_, PLDI 2017 ([DOI 10.1145/3062341.3062363](https://doi.org/10.1145/3062341.3062363)).

- **Shows**: the motivation, design and formal semantics of WebAssembly, and early experience with its implementations. JavaScript is described as an inconsistent compilation target.
- **In Peras**: owned modules compile to WebAssembly ([decision 0001](decisions/0001-webassembly-backend.md)).

**WebAssembly is not always faster.** Jangda, Powers, Berger and Guha, _Not So Fast: Analyzing the Performance of WebAssembly vs. Native Code_, USENIX ATC 2019.

- **Shows**: on the SPEC CPU programs, WebAssembly ran 45 % (Firefox) to 55 % (Chrome) slower than native code on average. Earlier results near parity came from small scientific kernels.
- **In Peras**: speed is measured, never assumed. A function moves from `.apeiron` to `.peras` after a measure. `scripts/bench-managed.mjs` shows why: an owned module is 1.7 to 1.9 times faster than hand-written JavaScript on counting words, and 6 times slower on a loop whose JavaScript arrays it must copy in and out ([managed modules benchmark](../../bench/README.md#managed-modules-benchmanaged)).

## The allocator

Masmano, Ripoll, Crespo and Real, _TLSF: A New Dynamic Memory Allocator for Real-Time Systems_, ECRTS 2004 ([DOI 10.1109/EMRTS.2004.1311009](https://doi.org/10.1109/EMRTS.2004.1311009)).

- **Shows**: Two-Level Segregated Fit allocates and releases in constant time, bounded for real-time systems.
- **In Peras**: production modules use TLSF ([decision 0002](decisions/0002-tlsf-allocator.md)).

## Hash tables

**Keys chosen to collide.** Crosby and Wallach, _Denial of Service via Algorithmic Complexity Attacks_, USENIX Security 2003.

- **Shows**: a new class of low-bandwidth denial of service, by inputs that make the data structures of common programs fall into their worst case.

**A keyed hash for short inputs.** Aumasson and Bernstein, _SipHash: a Fast Short-Input PRF_, INDOCRYPT 2012 (LNCS 7668, pp. 489–508).

- **Shows**: a family of pseudorandom functions fast on short inputs, designed for hash tables protected against hash flooding.
- **In Peras**: maps and sets hash with SipHash-1-3 under a key drawn for each instance ([decision 0005](decisions/0005-maps-and-sets.md)).

**Metadata beside the slots.** Kulukundis, _Designing a Fast, Efficient, Cache-friendly Hash Table, Step by Step_, CppCon 2017 (Abseil's Swiss tables).

- **Shows**: a hash table that keeps a few bits of each hash next to its slots, so that most probes are decided without reading the entries, with SIMD over groups of slots.
- **In Peras**: tried for maps with keys of 32 bits, the key kept in the index; 4 to 11 % faster on the dashboard at 4,000,000 rows, below the criterion registered, and taken back ([decision 0018](decisions/0018-large-maps.md)).

## Reading data fast

**Decimal text to the nearest float.** Clinger, _How to Read Floating Point Numbers Accurately_, PLDI 1990 (pp. 92–101).

- **Shows**: an algorithm that always finds the best binary approximation of a decimal number, a problem no arithmetic of fixed precision solves alone.
- **In Peras**: `parseF64` computes the common case exactly in WebAssembly by its fast path, and asks the host's `Number` beyond it ([methods](reference/methods.md)).

**Numbers parsed at a gigabyte per second.** Lemire, _Number Parsing at a Gigabyte per Second_, Software: Practice and Experience 51(8), 2021, pp. 1700–1727 ([DOI 10.1002/spe.2984](https://doi.org/10.1002/spe.2984)).

- **Shows**: decimal text read into the nearest float at about a gigabyte per second on 64-bit processors, by optimizing the common case of at most 19 digits, without losing accuracy.
- **In Peras**: the way to read every `parseF64` in WebAssembly, without asking the host ([decision 0016](decisions/0016-faster-from-the-literature.md)).

**Structure found many bytes at a time.** Langdale and Lemire, _Parsing Gigabytes of JSON per Second_, The VLDB Journal 28(6), 2019 ([DOI 10.1007/s00778-019-00578-5](https://doi.org/10.1007/s00778-019-00578-5)).

- **Shows**: a validating JSON parser that processes gigabytes per second on one core, with a quarter or fewer of the instructions of a reference parser, by making extensive use of SIMD instructions.
- **In Peras**: the way to search for a separator 16 bytes at a time in `split` ([decision 0016](decisions/0016-faster-from-the-literature.md)).

**UTF-8 validated fast.** Keiser and Lemire, _Validating UTF-8 in less than one instruction per byte_, Software: Practice and Experience 51(5), 2021 ([DOI 10.1002/spe.2940](https://doi.org/10.1002/spe.2940)). The rule itself: the Unicode Standard, chapter 3, table 3-7, _Well-Formed UTF-8 Byte Sequences_.

- **Shows**: UTF-8 validated with SIMD at less than one instruction per byte, the common case of ASCII the fastest.
- **In Peras**: bytes given where a `String` is expected are checked by table 3-7, sixteen ASCII bytes at a time ([decision 0020](decisions/0020-text-as-bytes.md)).

## Removing work the program did not ask for

**No intermediate list.** Coutts, Leshchinskiy and Stewart, _Stream Fusion: From Lists to Streams to Nothing at All_, ICFP 2007 (pp. 315–326).

- **Shows**: an automatic deforestation, by equational transformations, that fuses a wider range of functions than earlier short-cut fusion, left folds and nested lists included.
- **In Peras**: `for (const part of text.split(sep))` walks the parts without the array (finding F206); the characters of `split("")` are next ([decision 0016](decisions/0016-faster-from-the-literature.md)).

**A computation done twice, done once.** Morel and Renvoise, _Global Optimization by Suppression of Partial Redundancies_, Communications of the ACM 22(2), 1979.

- **Shows**: redundancy elimination and loop-invariant code motion treated as one problem, the suppression of computations partially redundant, by data-flow analysis.
- **In Peras**: one lookup for `has` or `get` followed by `get` or `set` of the same key (finding F210, [decision 0016](decisions/0016-faster-from-the-literature.md)); a `set` given the entry the lookup found ([decision 0017](decisions/0017-aggregating-without-copies.md)).

**Objects that do not escape.** Choi, Gupta, Serrano, Sreedhar and Midkiff, _Escape Analysis for Java_, OOPSLA 1999.

- **Shows**: a data-flow analysis, on a connection graph, that finds the objects that do not outlive the method making them, so that they can be allocated on its stack.
- **In Peras**: an array of views that does not leave a loop's body, and an `Option` returned and unwrapped at once, need no block of the heap each time (findings F208, F209).

**Regions.** Tofte and Talpin, _Region-Based Memory Management_, Information and Computation 132(2), 1997, pp. 109–176.

- **Shows**: memory allocated and freed in regions, the points of allocation and release inferred by a type and effect analysis, with no garbage collector.

**Reuse.** Reinking, Xie, de Moura and Leijen, _Perceus: Garbage Free Reference Counting with Reuse_, PLDI 2021.

- **Shows**: precise reference counting that keeps only live data, proved sound and garbage free, whose reuse analysis turns functional code into updates in place.
- **Does not show**: anything about Peras's ownership, which counts no references; the reuse of a block freed just before an allocation of its size applies to it all the same.
- **In Peras**: a loop's array reused from one iteration to the next (finding F208).

**Reuse within a frame.** Lorenzen and Leijen, _Reference Counting with Frame Limited Reuse_, ICFP 2022.

- **Shows**: a value taken apart and rebuilt as an update of the same memory, the reuse limited to what a function's frame can prove.
- **In Peras**: `let p = m.get(k).unwrapOr(…)`, changes, then `m.set(k, p)`: the entry read where it is and updated with one lookup (finding F216, [decision 0017](decisions/0017-aggregating-without-copies.md)).

**Deforestation.** Wadler, _Deforestation: Transforming Programs to Eliminate Trees_, Theoretical Computer Science 73(2), 1990, pp. 231–248.

- **Shows**: a transformation that removes the intermediate structures a program builds only to consume them.
- **In Peras**: a template's integers written into its string with no string of their own ([decision 0017](decisions/0017-aggregating-without-copies.md)); a key built only to be looked up is not built yet.

## Floats as text

Adams, _Ryū: Fast Float-to-String Conversion_, PLDI 2018 ([DOI 10.1145/3192366.3192369](https://doi.org/10.1145/3192366.3192369)).

- **Shows**: an algorithm, with a proof of correctness, that writes the shortest decimal text from which the same float is read back.
- **In Peras**: `toString()` on floats writes that same shortest text, today by asking JavaScript's `String()`. Ryū is the planned way to write it in WebAssembly if a program needs it faster ([changelog](../../CHANGELOG.md)).

## Diagnostics

Becker, Denny, Pettit, Bouchard, Bouvier, Harrington, Kamil, Karkare, McDonald, Osera, Pearce and Prather, _Compiler Error Messages Considered Unhelpful: The Landscape of Text-Based Programming Error Message Research_, ITiCSE Working Group Reports 2019 ([DOI 10.1145/3344429.3372508](https://doi.org/10.1145/3344429.3372508)).

- **Shows**: a review of more than 300 works on programming error messages, from 1965 on. Its title gives its finding.
- **In Peras**: every diagnostic gives its cause and its fix, in English and in French. Every code has a page whose example and fix are checked by the tests ([error codes](errors/index.md)).

## Adoption and its risks

This research argues against parts of the project, or bounds where it fits.

**Libraries decide adoption.** Meyerovich and Rabkin, _Empirical Analysis of Programming Language Adoption_, OOPSLA 2013 ([DOI 10.1145/2509136.2509515](https://doi.org/10.1145/2509136.2509515)).

- **Shows**: on about 213,000 projects and surveys of developers, open-source libraries, existing code and experience weigh most in the choice of a language. A language's intrinsic features weigh less.
- **For Peras**: a module cannot use npm today. Peras therefore fits as modules called from a TypeScript application, which keeps npm and its frameworks. The typed use of JavaScript libraries through their `.d.ts` is planned ([roadmap, phase 5](reference/roadmap.md)).

**What one knows gets in the way.** Shrestha, Botta, Barik and Parnin, _Here We Go Again: Why Is It Difficult for Developers to Learn Another Programming Language?_, ICSE 2020 ([DOI 10.1145/3377811.3380352](https://doi.org/10.1145/3377811.3380352)).

- **Shows**: on Stack Overflow questions and interviews of professional programmers, the knowledge of a known language interferes with learning a new one. This interference is widespread.
- **For Peras**: its syntax is TypeScript's and its rules are not, which is the case this paper describes. Diagnostics that name the TypeScript habit and give the Peras form are planned: the ternary `a ? b : c`, object literals, `any`.

**What adopting a safe language costs.** Fulton, Chan, Votipka, Hicks and Mazurek, _Benefits and Drawbacks of Adopting a Secure Programming Language: Rust as a Case Study_, SOUPS 2021.

- **Shows**: in interviews of 16 developers and a survey of 178, Rust is seen as reaching its goals of security and performance. Its drawbacks are a steep learning curve, limited libraries and worries about hiring. Developers also report that it improved how they reason about secure code beyond Rust.
- **For Peras**: the same drawbacks are to be expected, and are listed in [before using Peras in production](reference/roadmap.md#before-using-peras-in-production).

**AI assistants know little of a young language.** Cassano et al., _MultiPL-E: A Scalable and Polyglot Approach to Benchmarking Neural Code Generation_, IEEE TSE 49(7), 2023 ([DOI 10.1109/TSE.2023.3267446](https://doi.org/10.1109/TSE.2023.3267446)), and Cassano et al., _Knowledge Transfer from High-Resource to Low-Resource Programming Languages for Code LLMs_, OOPSLA 2024 ([arXiv 2308.09895](https://arxiv.org/abs/2308.09895)).

- **Show**: code models do well on languages frequent in their training data, and struggle on rare ones. Training data translated from Python and validated by tests doubled their results on Racket and OCaml.
- **For Peras**: an assistant writes TypeScript that Peras refuses. The compiler's diagnostics, which give the fix, and this documentation, whose every example is compiled by the tests, are what it can learn from. A dataset of validated Peras programs would follow that paper's method.
