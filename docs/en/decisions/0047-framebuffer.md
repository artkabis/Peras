# 0047: A framebuffer of pixels

_French translation: [../../fr/decisions/0047-framebuffer.md](../../fr/decisions/0047-framebuffer.md)._

_Status: protocol registered on 2026-11-25, before any prototype, as the rule of the project asks; the floors of two options measured first (finding M041). Ranked first, tied, by [decision 0046](0046-what-to-build-next-again.md), whose particle scene, A3, could not write a framebuffer; asked by the owner on 2026-11-25 as the first step of a lot with packages, `create-peras` and `@frame`._

## Context

What Peras does today, read before writing this (finding M020):

- **A frame is an `Array<u8>`, four bytes a pixel**, written one byte at a time with a check of its index each (`crates/perasc-ir`, decision 0037 for managed modules), cleared by a loop of byte stores: Peras has no `fill` (decision 0046, rank 9). A3 writes `Uint8ClampedArray`'s rounding by hand, `clamp8`, eleven lines.
- **The canvas can already read a module's memory without a copy**: an array of numbers kept in the module is a `PerasArray`, whose `view()` is a typed array over its elements (`crates/perasc-codegen-wasm/src/lib.rs`, the loader); a `Uint8ClampedArray` over the same bytes makes an `ImageData`. Nothing names it: a user writes the four lines, or copies the frame.
- **Where A3's time goes**, profiled by removing one thing at a time with the same result kept (finding M013), 21 rounds in one process, against its TypeScript: A3 as written 2.79; its pixels as one `u32` each, written once and cleared as 64,000 words, the floor of a framebuffer, 2.10; its grid without the copy of a list a map's `get` gives, 2.07; both, 1.59. The A/A control, 0.97. Two costs of about 40 ms each of 172: the bytes of the frame, this decision's; the copies of the grid, finding F125, not this decision's.

## The options

- **f0, nothing**: an `Array<u8>`, as A3 writes it. The reference.
- **f1, a guide**: the frame as an `Array<u32>`, one word a pixel, `r | g << 8 | b << 16 | a << 24`, cleared by a loop; a page of the guide and the four lines that give it to a canvas. Nothing in the compiler.
- **f2, a type written in Peras**: `Framebuffer` and `Rgba8` in a `.peras` file a program imports (decision 0022), as Peras has no library written in Peras yet: the first thing the packages of the next decision would carry. A structure over an `Array<u32>` with `width`, `height`, `set(x, y, color)`, `get(x, y)`, `clear(color)`, and `Rgba8.clamped(r, g, b, a)` rounding as `Uint8ClampedArray` does; and, in the loader, `toImageData()` on the handle, a view and no copy. The compiler unchanged, but for an array's `fill`, which `clear` needs and decision 0046 ranked.
- **f3, a type the compiler knows**: f2's surface, its `set` lowered to one store with its check, its `clear` to `memory.fill` or a loop of wide stores, a frame's bounds checked once per call where the compiler proves the coordinates (decision 0036's invariants). Built only if f2 misses the criteria by what f3's floor would recover.

A managed module's framebuffer, a `Uint8ClampedArray` itself, is the same surface; it is measured for f2 and f3, not as an option of its own.

## The programs

- **A3**, decision 0046's particle scene, unchanged but for its frame: 300 frames of 320 by 200, the same line as its TypeScript.
- **P2, a full frame**: each pixel of 640 by 400 computed every frame (a plasma of `sin` and `cos`, 120 frames), where the frame is all of the work; written first in TypeScript, as decision 0046's programs were.
- **P3, the frame to a canvas**: P2 drawn into a canvas in Chromium (`PERAS_CHROMIUM`), each frame given by a view or by a copy, the time of `putImageData` and of the frame measured apart.

## What is measured, registered now

1. **The results**: each version gives its TypeScript's line, twice on one instance, built for production (`--allocator tlsf`, finding M029).
2. **Speed**: the time of each program against its TypeScript and against f0, in one process, the order alternating round by round, 21 rounds; the A/A control, the TypeScript against itself and f0 against itself, in the same rounds (finding M042).
3. **CPU load**: the time a core is kept busy over P3's 120 frames, as decision 0012 measures it.
4. **Memory and bytes**: the module's memory at its peak, and its bytes, for each option; a bound of 1.10 times or one page of 64 KiB more (finding M037).
5. **Ease**: A3 and P2 written for each option: the lines, the notions a user learns, the errors met, recorded run by run as in decision 0046.
6. **The compiler**: `check-regression` for f2 and f3, every module identical or smaller but those that use what the option brings (finding M038).

## The criteria, registered now

An option is kept when, against f0, on A3 and P2: its time is at most 0.85 times f0's, with the A/A control's median within 0.95 to 1.05; its memory within the bound; it writes fewer lines and meets no more errors; P3 gives a frame to the canvas with no copy. Among the options kept, the one that needs least in the compiler; f3 over f2 only where its time is at most 0.90 times f2's. If none is kept, f0 stays, and the guide says what f1 measured.

## What this cannot show, said now

The programs are the project's, and a framebuffer's worth depends on what a user draws: A3 draws a few pixels a frame, P2 all of them, the two ends. Text, images and lines drawn into the frame are not measured: they are later decisions, which this one's type would carry. SIMD types (phase 6) are not an option here; f3's wide stores are the compiler's, not the user's.

## Record

### The specification of P2 (2026-11-25, before any Peras)

`bench/frame/plasma.ts`, type-checked at TypeScript's strictest (`bench/frame/tsconfig.json`): a plasma of 640 by 400, a sine per column and per row computed each frame, a palette of 256 colours, a shade per row whose products round as a `Uint8ClampedArray` rounds; 120 frames give `120|135900051`, in 289 ms on this machine. The sines are tables of a row and a column, as a plasma is written, so that the frame, not the sines, is the work. From this commit on, it is not changed.

### The versions (2026-11-25)

`bench/frame/`: P2 for each option, `plasma-f0.peras`, `plasma-f1.peras`, `plasma-f2.peras`; A3's f1 and f2, `particles-f1.peras` and `particles-f2.peras`, its f0 decision 0046's; f2's library, `framebuffer.peras`, 46 lines. Each gives its TypeScript's line, twice on one instance. The runs of `perasc check` are in [`bench/frame/ease`](../../../bench/frame/ease): P2's f0 met 21 errors in six runs, then a result changed in silence, `.toU8()` truncating where a `Uint8ClampedArray` rounds, and a sum that wraps; f2 met one, and A3's f2 a bug of perasc, F126, fixed. A departure from the protocol: f1 and f2 were written from f0 as it builds, by its author, so their errors are not compared with f0's, only what each asks a user to write.

### What P2's floor shows (2026-11-25, before the measure)

Measured as A3's was, one thing removed with the same work, 11 rounds of 60 frames in one process, against TypeScript: f1 as written, 3.52; its rounding by `Math.round`, 6.43 (finding F127, a call into JavaScript each time); the same rounding written with `floor` and a branch, 3.18; written without a branch, `Math.floor(v + 0.5)`, 1.87 (finding F128, no `select`); truncated, a different result, 1.58. P2's frame is not its cost: its rounding is, three times a pixel, a branch the processor mispredicts where a `Uint8ClampedArray` rounds in one instruction. f3's `rgba`, lowered to WebAssembly's `f64.nearest`, which rounds a half to the even as a `Uint8ClampedArray` does, in one instruction, has the truncation's floor: f3 is built, as registered, if f2 misses the criteria.

### The measure (2026-11-25)

`node scripts/bench-frame.mjs`, 21 rounds in one process, every version built for production, at `1d3d627`; [`bench/frame/RESULTS.md`](../../../bench/frame/RESULTS.md). Time against f0, median:

| Option |    A3 |    P2 | Lines, A3 and P2 | Bytes, gzip, A3 and P2 |
| ------ | ----: | ----: | ---------------- | ---------------------- |
| f0     |     1 |     1 | 120, 77          | 4,970, 3,209           |
| f1     | 0.760 | 1.029 | 123, 80          | 4,982, 3,220           |
| f2     | 0.746 | 0.884 | 106, 73          | 5,168, 3,389           |
| f3     | 0.698 | 0.499 | 106, 73          | 5,125, 3,340           |

The A/A controls lie within the band registered: the TypeScript against itself 1.001 and 1.003, f0 against itself 0.986 and 0.990. Memory is the same for every option, 640 KiB and 2,112 KiB. Against TypeScript, f3 brings A3 from 3.08 to 2.14 times its time and P2 from 3.57 to 1.80.

By the criteria: f1 and f2 miss P2's 0.85; f3 meets it on both programs, writes fewer lines than f0, keeps the memory; against f2, its time is 0.94 on A3 and 0.56 on P2, so f3 over f2 holds on P2, where the rounding is the work. The criterion of P3, the frame given to a canvas with no copy, is still to be measured.

_To come: P3, the proposal._
