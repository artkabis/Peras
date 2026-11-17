# 0029: The pond of a thousand lanterns, a scene in 3D measured

_French translation: [../../fr/decisions/0029-pond-of-lanterns.md](../../fr/decisions/0029-pond-of-lanterns.md)._

_Status: protocol registered on 2026-11-17 (commit `724bd40`), before any version but the reference and before any measure; study completed with its second run, at commit `43ad222`. Four hypotheses hold (H2, H5, H6, H7), two are refuted (H1, the speed against TypeScript, and H3, the managed module), one is undetermined (H4, the threads). Results and record below. The protocol was chosen by the owner, before `Rc<T>` (finding F310), so that its options are measured on this scene._

## Context

The owner asked whether Peras makes 3D elements in a canvas faster than JavaScript, TypeScript and WebAssembly, with a benchmark that is also a poem. [Decision 0013](0013-garden-of-apeiron.md) measured a scene computed pixel by pixel; this one measures what a 3D engine does: meshes turned, projected and filled with a depth buffer, a thousand times a frame.

What it can and cannot say, before measuring:

- **It measures the processor, not the graphics card.** A WebGL version would draw this scene faster than every version here; it is out of the protocol, as Peras has no type for WebGL yet. What is compared is a frame computed by the processor, as a software renderer, a physics step or a worker preparing geometry computes it.
- **Peras writes WebAssembly.** Against Rust compiled to WebAssembly, the best it can reach is the same speed; the question is how close, and at what cost of writing.
- **Peras is measured before its graphics phase.** `Framebuffer`, pixel types and seeded random numbers are planned (phases 3 and 6 of the roadmap); the scene is written with what is compiled today, and what it lacks is entered as findings.
- **One mesh is shared by a thousand lanterns.** Without `Rc<T>`, the mesh is kept once and each lantern reads it, as data-oriented code does in any language; what a lantern holding its own reference to the mesh would cost to write is recorded, for decision 0030 to measure `Rc<T>`'s options on it.

## The scene

At night, a pond. A thousand paper lanterns, each the same mesh of three rings of eight points, 44 triangles, drift on the water; a quarter are sky lanterns, rising slowly and starting again from the water. The moon looks on and the water keeps its trace. One frame, numbered _t_, is drawn as follows, in this order, by every version:

1. **The sky and the water**, each pixel: a gradient; stars where a hash of the pixel falls below 0.0015, twinkling with _t_; the moon, a disk with a soft edge; below the horizon, the water darker near, and the moon's path on it, broken by ripples.
2. **Each lantern**, in the order of its number: its place, its turn, its size, its colour and its flame drawn from a hash of its number and moved with _t_. Its mesh is turned, placed and projected; each triangle is filled where the centre of a pixel is inside it and nearer than what the depth buffer holds, with a flat shade, the paper lit from inside, brighter where it faces the eye.
3. **Its reflection**: the same triangles mirrored in the water, each point moved sideways by a ripple of its height on the screen, darker.
4. **Its halo, then the halo's reflection**: each pixel near the flame adds the lantern's colour, fading with the square of the distance.
5. **The colours**, each clamped and rounded to a byte, packed in one RGBA word per pixel.

**The same pixels everywhere.** The scene uses only operations IEEE 754 defines exactly, `+`, `-`, `*`, `/`, `sqrt`, `floor`, `abs`, `min`, `max`, on `f64`, and an integer hash of 32 bits (`lowbias32`, [Wellons, 2018](https://nullprogram.com/blog/2018/07/31/)), as decision 0013. A lantern turns without `sin` or `cos`, by the rational parametrization of the circle; it sways and flickers by a triangle wave. Additions are made in the same order in every version, so that their rounding is the same.

**The specification is the reference.** `bench/lanterns/pond.ts`, at the commit that registers this protocol, says every number and every order; each other version translates it operation for operation.

## The versions

- **TypeScript, written for speed**: typed arrays, plain loops; the reference.
- **Peras, owned**, compiled to WebAssembly, written as plainly as the TypeScript, built for production (TLSF) and for debug.
- **Peras, owned, on threads**: the same, its rows split by `parallel for` with `--threads on`, if the language lets it be written (finding F301: no call, no allocation in a turn).
- **Peras, managed**: the same source in a `.apeiron` module, compiled to JavaScript.
- **Rust**, compiled to WebAssembly with `wasm-bindgen`, as `bench/garden/rust` builds it, its toolchain pinned.

Every version takes its buffers from its caller, the pixels, the depth and the light, allocated once, as an engine keeps them between frames. Every version is written by the Peras authors; that is a threat to validity, listed below. Each competitor is within the rules of the project: Rust is the only other language, no npm package is installed for a version (finding M004).

## Hypotheses

Each with what refutes it. Times are of frame 600 at 1024 × 768 with 1,000 lanterns, in Chromium.

- **H1, Peras is faster than TypeScript.** Peras owned draws a frame at least 1.3 times as fast as the TypeScript, the median of the ratios round by round. _Refuted_ below 1.3. The threshold is decision 0013's, where the garden reached 1.19.
- **H2, Peras is close to Rust.** At least 0.85 times as fast as Rust. _Refuted_ below.
- **H3, managed costs nothing.** The managed module draws within 1.15 times the TypeScript's time. _Refuted_ above.
- **H4, the threads pay.** On threads, at least 2 times as fast as Peras owned on one, on the 4 cores of the machine. _Refuted_ below; _undetermined_ if `parallel for` cannot express the frame, with the diagnostic as evidence (finding M001).
- **H5, a small module.** Peras owned, module and loader, minified and gzip, ships in no more bytes than Rust's module and its `wasm-bindgen` glue. _Refuted_ above.
- **H6, as easy to write as TypeScript.** Peras owned is at most 1.25 times the TypeScript's lines, comments and blank lines aside. _Refuted_ above. The errors met before it builds are listed with their codes, and each friction is entered as a finding.
- **H7, no cost of safety at the frame.** The debug build, its integer overflow and indices checked, within 1.5 times production's time. _Refuted_ above.

## Measures

- **Same pixels**: SHA-256 of each version's render at frames 0, 300 and 600, all equal.
- **Time**: milliseconds per frame, median of 15 runs after 2 of warm-up, the versions alternating within each run in one Chromium page, and the ratio to the TypeScript round by round (finding M017).
- **Load on the processor and memory**: the CPU time over the wall time of each version's runs, and the memory of each module, as decision 0012 measures them.
- **Bytes**: what each version ships, minified by Vite where it applies, raw and gzip.
- **Code**: lines without comments or blank lines; the errors met while writing; the notions TypeScript does not ask for.
- **The shared mesh**: a lantern holding its mesh, written as a user from JavaScript writes it, built by Peras; what it refuses, recorded as the starting point of decision 0030.

`node scripts/bench-lanterns.mjs` runs them; `--check` checks the same pixels at 256 × 192 with 120 lanterns, at frames 0 and 600, and is run with every check of the project.

## Threats to validity

- One machine, headless, 4 cores; a phone may order the versions differently.
- The authors wrote every version, from the TypeScript; an expert of another language may write it faster.
- Exact operations only: no `sin`, `cos` or `pow`, which a real engine uses and whose results differ between libraries. The scene is shaped by that rule; the speed of those functions is not measured.
- The triangles of a scene this size are small: the cost per triangle, which projection and set-up make, weighs more than in a scene of large triangles.

## Consequences

- `bench/lanterns`, `node scripts/bench-lanterns.mjs`, and its results under this protocol, with a frame of the scene.
- Decision 0030, `Rc<T>`, measures its options on this scene.

## Record of the study

| When (2026-11-17)       | Commit    | Step                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----------------------- | --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| before the registration | —         | The reference written in TypeScript, then drawn at frames 0, 300 and 600 in Node to see that the specification draws and is deterministic. Seen on the first renders, before any other version: the lanterns made a wall at the horizon, so they are spread wider and deeper and the eye raised from 1.2 to 3; their halo was faint, so it reaches farther; the moon, its path on the water and the sky lanterns were added. The script printed the reference's time in Node, 45 to 48 ms a frame after one of warm-up: an informal measure of the reference alone, which bears on no ratio of the hypotheses, whose thresholds are decision 0013's. |
| after the registration  | `afca8f0` | Peras owned and managed, and Rust, translated from the reference; each gives its pixels bit for bit at frames 0, 300 and 600, at 256 × 192 and 1024 × 768, nothing left alive. The first Peras version is kept as written, refused with 45 errors; the threads and the shared mesh are kept as written, refused. The script printed the time of these checks in Node: informal, after every threshold was registered.                                                                                                                                                                                                                                |
|                         | `afca8f0` | Run 1, kept with its flaws in [run-1](../../../bench/lanterns/run-1/RESULTS.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
|                         | `43ad222` | The corrections of run 1; run 2, below.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

### Run 1, at commit `afca8f0`: kept, with two flaws

- **Rust was built without WebAssembly SIMD**, where the protocol builds it as `bench/garden/rust` is built, with `+simd128`, and as Peras builds by default. H2 was measured against a weaker Rust: 0.82, refuted.
- **The JavaScript heap leaves out the bytes of typed arrays**: the 27 MiB of buffers TypeScript and the managed module draw into were counted nowhere, so their memory looked 36 times smaller than the WebAssembly modules'.

**The corrections**, at commit `43ad222`, on the method only: Rust built with `+simd128`; each version's bytes outside the heap counted, its WebAssembly memory or the typed arrays it draws into.

### Run 2, at commit `43ad222`: the results

[Results of run 2](../../../bench/lanterns/RESULTS.md). An Intel Xeon at 2.30 GHz, 4 cores, Node 24, Chromium headless; frame 600 at 1024 × 768 with 1,000 lanterns, 15 runs after 2 of warm-up, the versions alternating.

| Hypothesis                 | Verdict      | Measured                                                              |
| -------------------------- | ------------ | --------------------------------------------------------------------- |
| H1, faster than TypeScript | **refuted**  | 0.97 times TypeScript's speed (0.88 to 1.15); threshold 1.3           |
| H2, close to Rust          | holds        | 0.89 times Rust's speed (0.80 to 0.99); threshold 0.85                |
| H3, managed costs nothing  | **refuted**  | 2.41 times TypeScript's time (2.15 to 3.13); threshold 1.15           |
| H4, the threads pay        | undetermined | `parallel for` cannot express the frame (E0241)                       |
| H5, a small module         | holds        | 8,164 bytes gzip against Rust's 16,491                                |
| H6, as easy to write       | holds        | 302 lines, 1.10 times TypeScript's 275; 45 errors met before it built |
| H7, no cost of safety      | holds        | debug at 1.08 times production's time (0.97 to 1.13)                  |

| Version             |    Frame | CPU / wall | Memory, heap and outside it | Shipped, gzip |
| ------------------- | -------: | ---------: | --------------------------: | ------------: |
| TypeScript          |  46.0 ms |       1.00 |        763 KiB and 27.0 MiB |         1,866 |
| Peras, managed      | 106.2 ms |       1.00 |        753 KiB and 27.0 MiB |         2,492 |
| Peras, owned        |  45.0 ms |       1.00 |        711 KiB and 27.1 MiB |         8,164 |
| Peras, owned, debug |  49.6 ms |       1.00 |        711 KiB and 27.1 MiB |             - |
| Rust                |  41.1 ms |       1.00 |        695 KiB and 28.3 MiB |        16,491 |

### What the study says

- **On a 3D frame computed by the processor, Peras is as fast as TypeScript, not faster** (H1). V8 compiles this loop of floating arithmetic over typed arrays about as well as WebAssembly runs it; the garden of decision 0013, whose pixels each call the noise functions many times, gave Peras 1.19. A gain over JavaScript has to come from what JavaScript cannot do: SIMD on the pixels, or threads, which H4 could not reach.
- **Peras is within 0.89 of Rust** (H2), in half its bytes (H5), and its safety costs 8 % (H7).
- **The managed module is 2.4 times slower than the TypeScript** (H3, finding F221): the same source compiled to JavaScript does not give the JavaScript a person writes. Not looked into here.
- **Peras is as long as TypeScript, but not as easy to write**: 1.10 times its lines once it builds (H6 holds), after 45 errors, every one an integer literal where an `f64` is expected or an array typed by its first element (finding F112), the most any program has met.
- **What the scene lacks, entered as findings**: `parallel for` cannot draw it (F301), and a mesh shared by a thousand lanterns cannot be held by each (F310, decision 0030).

### Departures from the protocol

- **H4 has no measure**: its version cannot be written, as the protocol foresaw; `bench/lanterns/threads.peras` keeps the attempt and its diagnostic.
- **The CPU time over the wall time** is measured on each version alone in a fresh page, 5 frames after 2, the versions not alternating; the times of H1 to H3 and H7 come from the alternating runs.
- **The memory outside the heap** was added after run 1, the protocol's measure, the heap alone, leaving out the buffers of the JavaScript versions.
