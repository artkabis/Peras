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

_To come: the prototypes, the measures, the proposal._
