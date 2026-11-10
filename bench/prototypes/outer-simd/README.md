# Vectorizing across the outer loop — a ceiling, measured before deciding

_Version française : [README.fr.md](README.fr.md)._

**The question.** Peras's vector loops run the turns of an inner loop several at a time. Mandelbrot and n-body do not fit: their inner loop is a sequence (a point's iterations, a body's sum of pulls) whose order fixes the result's bits. Their outer loop does fit: pixels, and bodies, are independent. Run two of them at once in the two `f64` lanes of WebAssembly SIMD, each lane doing its own arithmetic in its own order, and the results stay the same, bit for bit. JavaScript has no SIMD: this would turn the ties of the [browser benchmark](../../README.md#browser-benchmark-benchbrowser) into gains over JavaScript written by an expert. The estimate was 1.5 to 1.8 times for both.

**The method.** [`src/lib.rs`](src/lib.rs) writes each kernel twice by hand in Rust, with the safe SIMD functions of `core::arch::wasm32`, no `unsafe`: one value at a time, as Peras compiles them today, and two lanes at a time, as Peras could emit them, the arrays in columns as Peras stores them. [`run.mjs`](run.mjs) checks that the two, Peras today and JavaScript usual and expert give the same result, bit for bit, then times them alternating in Chromium, three rounds. The results are in [RESULTS.md](RESULTS.md).

**What it found.**

| Kernel     | Two lanes against Peras today | Against the fastest JavaScript |
| ---------- | ----------------------------: | -----------------------------: |
| n-body     |                   1.81 – 1.85 |                     about 1.85 |
| Mandelbrot |                   0.93 – 0.97 |                     about 1.02 |

- **n-body gains 1.8 times**, as estimated: every lane runs the same inner loop, a thousand pulls; the pair of a body with itself is masked out of its own lane.
- **Mandelbrot loses 3 to 7 %**, against the estimate. Two neighbouring points rarely escape at the same step: a pair runs as long as its slower point, under a mask, and two lanes leave no room to pay for that. A first version, which compared each lane's step count every turn, lost 12 to 14 %; one counter for both lanes brought it to 3 to 7 %, no further. Four lanes would need `f32`, which changes the results.

**The decision.** Vectorizing across an outer loop is worth building for loops like n-body's: an inner loop with the same number of turns in every lane, a branch at most, taken under a mask. It is not worth it for an inner loop that ends on a condition that differs between lanes, like Mandelbrot's: such a loop stays one value at a time. As for the [SIMD of decision 0004](../../../docs/en/decisions/0004-strings-without-allocation.md#status), the estimate was right once and wrong once; only the measure decides.
