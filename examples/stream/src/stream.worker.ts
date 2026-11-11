// The stream's worker: it generates each batch, digests it with Peras and,
// when asked, with TypeScript, and says how long each took. The page only
// draws.
import { __perasHeap, digest } from "./telemetry.peras";
import { batch } from "./generate.ts";
import { canonical, digest as reference } from "./digest.ts";
import type { Sample } from "./metrics.ts";
import type { Digest } from "./telemetry.peras";

export interface Request {
  batch: number;
  rows: number;
  seed: number;
  compare: boolean;
  /** Milliseconds since the stream started, on the page's clock. */
  started: number;
}

export interface Reply {
  sample: Sample;
  digest: Digest;
}

self.onmessage = (event: MessageEvent<Request>) => {
  const r = event.data;
  let t = performance.now();
  const text = batch(r.batch * r.rows, r.rows, r.seed);
  const generateMs = performance.now() - t;

  const first = r.batch % 2 === 0 ? "peras" : "typescript";
  let perasMs = 0;
  let typescriptMs: number | null = null;
  let got: Digest | undefined;
  let want: Digest | undefined;
  const runPeras = () => {
    t = performance.now();
    got = digest(text);
    perasMs = performance.now() - t;
  };
  const runTypescript = () => {
    t = performance.now();
    want = reference(text);
    typescriptMs = performance.now() - t;
  };
  if (r.compare && first === "typescript") runTypescript();
  runPeras();
  if (r.compare && first === "peras") runTypescript();
  if (got === undefined) return;

  const sample: Sample = {
    batch: r.batch,
    rows: r.rows,
    bytes: text.length,
    generateMs,
    perasMs,
    typescriptMs,
    first: r.compare ? first : "peras",
    agree: want === undefined ? null : canonical(got) === canonical(want),
    live: __perasHeap.live(),
    at: performance.timeOrigin + performance.now() - r.started,
  };
  const reply: Reply = { sample, digest: got };
  self.postMessage(reply);
};

// The module above loads its WebAssembly with a top-level await: a message
// sent before this handler exists would be lost (finding F106).
self.postMessage("ready");
