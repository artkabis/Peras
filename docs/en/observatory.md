# The observatory of Peras

_French translation: [../fr/observatory.md](../fr/observatory.md)._

A page that shows what Peras does well and what it does not yet, in one place: scenes drawn live by TypeScript and by Peras side by side, a measure the visitor runs on their own machine, and every figure the repository has recorded, read from its results files when the page is built ([source](../../examples/observatory)). It follows the garden of Apeiron ([decision 0013](decisions/0013-garden-of-apeiron.md)): a measure that matters is also a scene, the same journey for each version, the figures beneath it; the poetry shows what the probes measure and never decides.

This protocol was written before the page was, and before any measure it makes.

## What the page shows

1. **A scene drawn live, twice.** Decision 0047's full frame, a plasma of 640 by 400: on the left, `bench/frame/plasma.ts`, the TypeScript it was specified in; on the right, `bench/frame/canvas-f3.peras`, the Peras module decision 0047 chose, drawing into an array kept in the module's memory and shown through a view, no pixel copied. The page imports those very files: what it draws is what the benchmarks measure. Each frame, both draw the same time `t`, the one that starts alternating frame by frame; every 30 frames, the two frames' bytes are compared, and the page says whether they are the same.
2. **A candle under each**, whose wax burns as the time its version spent drawing, summed since the scene started, against the larger of the two: a picture of the processor's time, not a measure of its own.
3. **The visitor's measure.** On a button, the scene stops and the page measures, with no canvas: three versions, the TypeScript, the Peras module, and the TypeScript again as a second instance (the A/A control, finding M042); 21 rounds of 20 frames, the order alternating round by round; for each round, the ratio of each version's time to the TypeScript's. The page gives the median and the range of each ratio, the browser, and the number of logical processors the browser reports, and lets the visitor save the rounds as JSON. A median of the A/A control outside 0.95 to 1.05 is shown as such: the machine was too noisy for the other ratio to decide anything.
4. **The recorded figures**, read from the repository's results when the page is built, each with its file: decision 0046's four applications against TypeScript (`bench/next/RESULTS.md`), decision 0047's programs against TypeScript (`bench/frame/RESULTS.md`), and decision 0011's seventeen bugs of web applications (`bench/web-bugs/RESULTS.md`). They are drawn on one axis, Peras's time over TypeScript's, on a scale of logarithms with 1 in the middle: where Peras is faster to its left, where it is slower to its right, with the A/A control's band where the file gives it. Nothing is typed into the page by hand: when a decision changes a result, the page shows the new one.

## A departure, said when it was made (2026-11-25)

The first page drew on its map only decisions 0046 and 0047, where Peras is slower in five rows of six, and left out every figure where it is faster; the owner saw it at once. The map now also reads the stream (`bench/stream/RESULTS.md`, decision 0015's H1 and H2 at 100,000 rows), the candle (`bench/candle/RESULTS.md`), the demo of 1,500,000 particles (`bench/demo/RESULTS.md`, a frame's median time) and the kernels in Chromium (`bench/browser/RESULTS.md`, Peras against the fastest JavaScript), in two groups: against TypeScript as it is usually written, and against TypeScript tuned by hand. A ratio within 0.95 to 1.05 is drawn as equal, the noise an A/A control measures. Decision 0046's A3 is left out, as decision 0047's row carries it as its figure before.

## What it cannot show, said now

- The visitor's machine is not the repository's: their ratio is theirs, and the page says where the recorded one was measured. A browser that is not Chromium may compile WebAssembly and JavaScript differently.
- The A/A control's second TypeScript is a second instance of the same class, not a second module: it measures the noise of the machine and of the order, not that of loading a module.
- The live scene's frame times include the page drawing two canvases; only the visitor's measure, with no canvas, is a measure.
- One scene is not every program: the recorded figures are there for that, the applications where Peras is slower among them.
