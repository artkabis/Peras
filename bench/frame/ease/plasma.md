# P2, a full frame: runs of perasc check (runs/plasma_*.json)

Written with Peras's own forms for declarations (struct/impl for the class), as the guide teaches first.

## f0, an `Array<u8>`

run 0, 3 errors: E0104 x3 `new Array(n).fill(0)` (cause: no `new`, no `fill`) -> a loop of `push(0)`, for the frame, the palette, the columns and the rows
run 1, 9 errors: E0210 x8, E0221 x1 (cause: a JavaScript number used as an integer and as a float): `i / 256`, `x * 0.031`, `f * 0.05` -> `.toF64()`, offered; `Math.floor(…) & 255` -> `.toI32()`; `sum * 31 + pixels[i]` -> `.toI32()`, offered
run 2, 6 errors: E0210 x6, a float stored into a byte and a byte into a float -> `.toU8()` and `.toF64()`, offered
run 3, 3 errors: E0210 x3, the shaded channels -> `.toU8()`, offered
run 4: checks and builds; gives 10|134207831 where TypeScript gives 10|357921442, and a negative sum at 120 frames, with no diagnostic: `.toU8()` truncates where a `Uint8ClampedArray` clamps and rounds a half to the even, and `sum * 31` wraps an `i32` in a production build where TypeScript's number is exact (cause: a framebuffer's rounding; a number used as an integer)
run 5: `clamp8` written by hand, eleven lines, as A3 has it, and the sum an `f64`: builds; same result as TypeScript, twice on one instance.

## f1, an `Array<u32>`

Written from f0 as it builds, its frame one word a pixel; the digest reads each word's four bytes.
run 0: builds; same result as TypeScript, twice on one instance.

## f2, a `Framebuffer` imported

Written from f0 as it builds, its frame a `Framebuffer`, its colours `rgba(…)`, `clamp8` gone; free functions, as the methods of an imported structure cannot be called (decision 0022).
run 0, 1 error: E0206 `setPixel(self.frame, …)`, an `inout` argument written with `&` -> `&self.frame`, offered
run 1: builds; same result as TypeScript, twice on one instance.

The library, `framebuffer.peras`, checked at its first run.

## What this count can show

f1 and f2 were written after f0, from f0 as it builds, by the author who had met f0's errors: their counts are not the errors a user meets writing them first, and are not compared with f0's. What is compared is what each asks a user to write and to know: f0's `clamp8` and four stores a pixel, f1's packing of four channels into a word, f2's `rgba` and `setPixel`.
