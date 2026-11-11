# 0013: The garden of Apeiron, a procedural scene measured

_French translation: [../../fr/decisions/0013-garden-of-apeiron.md](../../fr/decisions/0013-garden-of-apeiron.md)._

_Status: protocol registered on 2026-11-11 at 05:21 UTC (commit `eb861d0`); study completed with its second run, at 06:02 UTC on commit `f527ff2`. Four hypotheses hold, one is refuted: H3, the speed. Results and record below._

_Protocol status: registered before any measure. The results will be added below it, whether they confirm the hypotheses or not; the protocol itself will not change after the first measure, and any departure from it will be listed as such._

## Context

The best compression of an image is the shortest program that draws it, its Kolmogorov complexity; the demoscene makes an art of it, with films of a few kilobytes. A scene computed pixel by pixel from rules can be drawn at any resolution, and zoomed without end: like a vector image, it stretches, and unlike one, it shows more detail as it is zoomed, where a stored image only blurs.

The owner asked whether Peras can join compression, speed, a quality beyond what exists, and poetry in its algorithms, and asked that it be done with the rigour of engineering and science, not naively. The poetry is in the name of the project: _peras_, the limit, imposed on _apeiron_, the unlimited, from which, for the Pythagoreans, every good thing arises. The garden of Apeiron is an unlimited landscape, born of a few finite rules.

The risks of naivety are known, and the protocol answers each:

- **A demo chosen to win.** The hypotheses, their thresholds and the competitors are fixed here, before measuring; a hypothesis refuted is reported as refuted.
- **Versions that do not do the same work.** Every version must give the same pixels, bit for bit, checked by a hash of each image.
- **A quality judged by eye.** Quality is measured by SSIM ([Wang et al., 2004](https://doi.org/10.1109/TIP.2003.819861)) and PSNR, computed by FFmpeg, not by us.
- **A tautology.** A procedural image is, by definition, its own reference at every zoom; what is measured is what a stored image must spend to come close to it.

## The scene

One function, `pixel(x, y, zoom)`, gives the colour of a point of the plane, written once in each language from the same specification:

1. **The sky**: a gradient from the zenith to the horizon, and clouds, a fractal sum of value noise ([Perlin, 1985](https://doi.org/10.1145/325165.325247)), thresholded.
2. **The mountains**: three ranges, each a ridge of one-dimensional fractal noise, paler with distance, as the air makes them ([da Vinci's aerial perspective](https://en.wikipedia.org/wiki/Aerial_perspective)).
3. **The meadow**: flowers placed by cellular noise ([Worley, 1996](https://doi.org/10.1145/237170.237267)), one per cell at a point drawn from the cell's hash, each a disk of a colour drawn the same way.

**Unlimited detail.** The number of octaves of each noise grows with the logarithm of the zoom: zoomed 2 times, one octave more. A pixel's cost grows with the logarithm of the zoom only.

**The same pixels everywhere.** To be computed identically in TypeScript, Peras and Rust, the scene uses only operations IEEE 754 defines exactly: `+`, `-`, `*`, `/`, `sqrt`, `floor`, on `f64`; its randomness is an integer hash of 32 bits (`lowbias32`, [Wellons, 2018](https://nullprogram.com/blog/2018/07/31/)), computed with wrapping multiplications and logical shifts. No `sin`, `exp` or `pow`, whose results differ between libraries. A colour becomes a byte by `floor(c * 255 + 0.5)` after clamping.

## The versions

- **TypeScript, written for speed**: typed arrays, plain loops, the reference.
- **Peras**, an owned module compiled to WebAssembly, written as plainly as the TypeScript.
- **Rust**, compiled to WebAssembly with `wasm-bindgen`, as `bench/browser` builds it.

Each version is written from the specification above, as naturally as its language allows, by the Peras authors; that is a threat to validity, listed below.

## Hypotheses

Each with what refutes it. Sizes are of JavaScript and WebAssembly as shipped: minified by Vite where it applies, then gzip.

- **H1, the program is smaller than its image.** The Peras garden, module and loader, ships in fewer bytes than the smallest AVIF of its 1024 × 1024 render that reaches an SSIM of 0.95 against it. _Refuted_ if an AVIF that small or smaller reaches 0.95.
- **H2, a stored image must grow with the zoom; the program does not.** To reach an SSIM of 0.95 against the procedural render of a window zoomed _z_ times, for _z_ = 1, 4, 16 and 64, a stored image of the whole scene needs about _z_² times the bytes; the program needs the same bytes at every zoom. _Refuted_ if the AVIF needed at _z_ = 64 is less than 16 times the one needed at _z_ = 1.
- **H3, Peras is fast where it counts.** Peras renders a 1024 × 1024 frame of the garden at least 1.3 times as fast as the TypeScript, median of 15 runs in Chromium, and at least 0.85 times as fast as Rust. _Refuted_ by either ratio below its threshold.
- **H4, the eye forgives from afar.** Seen from farther, an image reduced 4 times as the eye integrates it, the SSIM of each lossy format against the reduced reference is higher than at full size, at the same bytes. _Refuted_ if it is lower for any format. This tests the owner's intuition that a detail lost is less seen once zoomed out.
- **H5, no cost of safety at the frame.** The Peras garden built for debug, its integer overflow checked (decision 0011, step 5), renders within 1.5 times its production build. _Refuted_ above.

## Measures

- **Same pixels**: SHA-256 of each version's 1024 × 1024 RGBA render, at zooms 1, 4, 16 and 64, all equal.
- **Bytes**: what each version ships, minified and gzip; PNG, WebP and AVIF of the render, at qualities from lowest to highest, encoded by FFmpeg.
- **Quality**: SSIM and PSNR between a stored image, decoded and enlarged by Lanczos to the window, and the procedural render of that window, by FFmpeg's `ssim` and `psnr`.
- **Time**: milliseconds per 1024 × 1024 frame, median of 15 runs after 2 of warm-up, in Chromium; the CPU time and memory of decision 0012.
- **Code**: lines without comments or blank lines, per version.

## Threats to validity

- One machine, headless, 4 cores; a phone may order the versions differently.
- The authors wrote every version: an expert of one language may write it faster.
- A landscape of noise suits the procedural; a photograph has no short program, and nothing here says otherwise.
- SSIM is one model of perception among others; Butteraugli and SSIMULACRA2, which model the eye more closely, are not installed on the machine that measures, and are left to a later run.

## Consequences

- `bench/garden`, `node scripts/bench-garden.mjs`, and its results under this protocol; a check in CI that the versions give the same pixels.
- If the hypotheses hold, the garden becomes the second demo of the README, zoomed from the whole landscape to a single flower. If they do not, the results say so, and so does the README.

## Record of the study

A laboratory notebook: each step, dated, with the commit that holds it, what was decided after the protocol was registered, and what was observed. Nothing here changes the hypotheses or their thresholds.

### Steps

| When (UTC, 2026-11-11) | Commit    | Step                                                                                                                                                                                                                                                                                                                                  |
| ---------------------- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 05:21                  | `eb861d0` | The protocol is registered, before any code of the scene and any measure.                                                                                                                                                                                                                                                             |
| 05:23–05:50            | —         | The reference is written in TypeScript; the scene is drawn for the first time, at zooms 1, 8 and 64, to see that it draws. The script printed the time of these renders, 327 ms at 512 × 512 and zoom 1 in Node: an informal measure of the reference alone, before the other versions existed, after every threshold was registered. |
| 05:50                  | —         | The versions in Peras and Rust, from the same specification; each gives the reference's bytes at 256 × 256 and zooms 1 to 64.                                                                                                                                                                                                         |
| 05:54                  | `0fb61c9` | The versions and `scripts/bench-garden.mjs` are committed, with the first run kept in `bench/garden/run-1`, and its flaw.                                                                                                                                                                                                             |
| 05:55                  | `f527ff2` | The reference's formatting, after the first run; its pixels unchanged. The second run starts on this commit.                                                                                                                                                                                                                          |
| 06:02                  | `f527ff2` | The second run ends; its results below.                                                                                                                                                                                                                                                                                               |

### Decided after the registration, before the first measure

The protocol fixes the structure of the scene, not every number of it. Before any measure, after looking at the first renders:

- **Colours.** A sky of dawn rather than of noon, clouds thresholded at 0.56 rather than 0.5, mountains from violet to green, and a meadow whose edge undulates by noise instead of a straight line at _y_ = 0.74. Aesthetics only: no hypothesis depends on a colour.
- **The zoom as a number.** `render` takes the zoom, not its logarithm, so that a video can zoom continuously; at the zooms of the protocol, powers of two, the computation is exact and the same in every version.

### Parameters the protocol left open

- The renders checked for the same pixels are centred on (0.37, 0.83), in the meadow, where the flowers of every level are.
- H1 compresses the whole scene at zoom 1, centred on (0.5, 0.5).
- H2 draws 8 tiles per zoom from a generator seeded with 13; at zoom 1 the only tile is the whole scene.
- AVIF is encoded by libaom through FFmpeg, `-still-picture 1 -cpu-used 6`, the CRF searched by bisection from 0 to 63; WebP by libwebp, `-compression_level 6`.
- SSIM and PSNR are FFmpeg's, on both images converted to YUV 4:4:4, the value of all three planes ("All").
- H4 compares WebP at qualities 20, 50 and 80 and AVIF at CRF 50, 35 and 20, then the same files reduced 4 times by area averaging.
- The times of H3 and H5 come from one Chromium page loading every version, the versions alternating within each run.

### Run 1, at commit `0fb61c9`: kept, with a flaw

[Results of run 1](../../../bench/garden/run-1/RESULTS.md). **The flaw.** The smallest AVIF of each tile of H2 was searched in 4:2:0 only, the subsampling that won at zoom 1. For some zoomed tiles, whose flowers are small and saturated, no AVIF in 4:2:0 reached an SSIM of 0.95, even at CRF 0; the search returned no size, the script wrote it as infinite and counted H2 as holding. H2 was undetermined by this run.

**The correction**, at commit `0fb61c9`, on the method only: each tile's AVIF is searched in 4:4:4 and in 4:2:0, the smaller kept, as the protocol's "smallest AVIF" says; a tile no AVIF brings to 0.95 is counted as unreached, and a zoom with an unreached tile makes H2 undetermined.

**What run 1 measured, which the flaw does not touch**, provisional until run 2:

| Hypothesis  | Run 1        | Measured                                                                                                  |
| ----------- | ------------ | --------------------------------------------------------------------------------------------------------- |
| Same pixels | yes          | TypeScript, Peras, Peras in debug and Rust, at zooms 1, 4, 16, 64 at 1024 × 1024                          |
| H1          | holds        | Peras ships 5,872 bytes; the smallest AVIF at SSIM 0.95, 9,774 (4:2:0, CRF 53)                            |
| H2          | undetermined | the flaw above                                                                                            |
| H3          | **refuted**  | Peras at 1.21 times TypeScript's speed, below the 1.3 registered; at 1.13 times Rust's                    |
| H4          | holds        | for each of the six files, the SSIM is higher reduced 4 times, from 0.948 to 0.977 for WebP at quality 20 |
| H5          | holds        | Peras in debug at 1.02 times its production time                                                          |

One observation the hypotheses did not foresee, and which must go with H1: the same program in TypeScript ships in 1,119 bytes, five times fewer than Peras; it is the procedural scene that is smaller than its image, and Peras pays the weight of its WebAssembly runtime and loader.

### Run 2, at commit `f527ff2`: the results

[Results of run 2](../../../bench/garden/RESULTS.md), [every number](../../../bench/garden/results.json). Finished at 06:02 UTC.

| Hypothesis                                                   | Verdict     | Measured                                                                                                                            |
| ------------------------------------------------------------ | ----------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Same pixels                                                  | yes         | TypeScript, Peras, Peras in debug and Rust, at zooms 1, 4, 16, 64 at 1024 × 1024                                                    |
| H1, the program smaller than its image                       | **holds**   | Peras ships 5,872 bytes; the smallest AVIF at SSIM 0.95, 9,774 bytes (4:2:0, CRF 53); the lossless PNG, 482,545                     |
| H2, a stored image grows with the zoom                       | **holds**   | at zoom 64, the whole scene needs 4,452 times the AVIF bytes of zoom 1, against 16 required                                         |
| H3, Peras 1.3 times TypeScript's speed and 0.85 times Rust's | **refuted** | 769 ms a frame against 914 for TypeScript: 1.19 times; 855 ms for Rust: Peras 1.11 times as fast as Rust                            |
| H4, the eye forgives from afar                               | **holds**   | the SSIM of each of the six files rises reduced 4 times: WebP at quality 20 from 0.948 to 0.977, AVIF at CRF 50 from 0.956 to 0.966 |
| H5, no cost of safety at the frame                           | **holds**   | Peras in debug, its overflow checked, at 1.02 times its production time                                                             |

### Analysis

**H2 is robust to its sample.** Each zoom's bytes come from 8 tiles, whose costs spread widely: from 0.3 KiB for a tile of sky to 83 KiB for a tile of meadow at zoom 16. Even if every tile at zoom 64 cost the least observed, the whole scene would need 142 times the bytes of zoom 1, against 16 required: no draw of the sample could refute H2. At zooms 16 and 64 the growth follows _z_² closely, 288 against 256 and 4,452 against 4,096.

**An anomaly, unexplained.** At zoom 4, the whole scene needed 0.9 times the bytes of zoom 1, not about 16 times as H2 predicted for every zoom; the eight tiles of zoom 4 all cost about 0.5 KiB. One explanation is that none fell in the meadow, a quarter of the plane: eight draws miss it with a probability of about 9 %. It is a hypothesis, not a finding; a later run with more tiles would say. The verdict of H2 rests on zoom 64, as registered.

**Reproducible.** The two runs agree: the same pixels and bytes, and times within 1 %, Peras at 1.21 then 1.19 times TypeScript's speed.

**H3, refuted twice.** Peras is faster than TypeScript, and than Rust, at every run, but not by the margin registered. V8 compiles this kind of code, floating-point arithmetic in plain loops, close to what WebAssembly does; the gap is the integer hash and the absence of bounds checks on the pixels. Peras's advantage is larger where it was measured before, on integers and on arrays of numbers ([browser benchmark](../../../bench/README.md#browser-benchmark-benchbrowser)); this scene is not of that kind. The margin of 1.3 was a guess made before measuring, and it was wrong.

**What H1 does not say.** The same program in TypeScript ships in 1,119 bytes, five times fewer than Peras's 5,872, which carry the WebAssembly runtime and its loader; Rust ships 11,695. It is the procedural scene that is smaller than its image; among the three, TypeScript ships it smallest.

### What it means

- **The program as compression holds.** A scene of 6 KB of code gives what an image of 10 KB gives at one zoom, and what 41.5 MB of AVIF would give at zoom 64. Stretched, it does not blur: it adds detail, as no stored image can.
- **The eye forgives from afar**, with the model of SSIM: each lossy format scores higher reduced 4 times, by 0.01 to 0.03. The owner's intuition holds; models closer to the eye, Butteraugli and SSIMULACRA2, are left to a later run.
- **Safety costs nothing here**: the overflow checks of decision 0011 cost 2 % of a frame in debug, and nothing in production.
- **Peras is not much faster than TypeScript on floating-point loops.** It renders the scene 1.2 times as fast, and 1.1 times as fast as Rust; that is the honest margin, smaller than hoped.
- **Its runtime weighs.** 5 KB of the 6 Peras ships are its runtime and loader, which a scene this small does not need: shipping a module without heap arrays without them is a lead for later, measurable by this benchmark.

### Limits

One machine, headless, 4 cores; versions written by the Peras authors; a scene of noise, which suits the procedural, where a photograph has no short program; SSIM as the only model of the eye; 8 tiles per zoom for H2. None of these is hidden by the verdicts above.
