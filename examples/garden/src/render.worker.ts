// The garden drawn off the main thread: one frame per message, the pixels
// handed back without a copy. The scene is bench/garden/garden.peras, the
// very program the benchmark measured.
import { __perasHeap, render, type PerasArray } from "../../../bench/garden/garden.peras";

export interface Request {
  id: number;
  side: number;
  zoom: number;
  cx: number;
  cy: number;
}

export interface Frame extends Request {
  /** RGBA bytes, side × side. */
  pixels: ArrayBuffer;
  /** Milliseconds the render took. */
  ms: number;
}

// One array in the module's memory, kept while the side stays the same.
let pixels: PerasArray<"i32"> | undefined;
let length = 0;

self.onmessage = (event: MessageEvent<Request>) => {
  const { side, zoom, cx, cy } = event.data;
  if (length !== side * side) {
    pixels?.release();
    length = side * side;
    pixels = __perasHeap.array("i32", length);
  }
  if (pixels === undefined) return;
  const start = performance.now();
  render(pixels, side, side, zoom, cx, cy);
  const ms = performance.now() - start;
  const copy = pixels.view().slice().buffer;
  const frame: Frame = { ...event.data, pixels: copy, ms };
  self.postMessage(frame, [copy]);
};
