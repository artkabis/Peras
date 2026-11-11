# 0013: The garden of Apeiron, a procedural scene measured

_French translation: [../../fr/decisions/0013-garden-of-apeiron.md](../../fr/decisions/0013-garden-of-apeiron.md)._

_Status: protocol, registered before any measure. The results will be added below it, whether they confirm the hypotheses or not; the protocol itself will not change after the first measure, and any departure from it will be listed as such._

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
