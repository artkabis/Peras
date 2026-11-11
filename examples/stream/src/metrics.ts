// What the page keeps of each batch, the totals it shows, and the files it
// exports. Pure functions, tested in test/metrics.test.ts.
import type { Digest, Zone } from "./telemetry.peras";

/** One batch, as the worker measured it. */
export interface Sample {
  batch: number;
  rows: number;
  bytes: number;
  /** Milliseconds to generate the batch's text. */
  generateMs: number;
  /** Milliseconds for Peras's digest, the text's way into WebAssembly included. */
  perasMs: number;
  /** Milliseconds for the TypeScript digest, or null when not compared. */
  typescriptMs: number | null;
  /** Which ran first: they alternate, so that neither always finds the cache warm. */
  first: "peras" | "typescript";
  /** Whether both digests gave the same numbers, or null when not compared. */
  agree: boolean | null;
  /** Blocks of Peras's heap alive after the call: 0, or a leak. */
  live: number;
  /** Lines Peras refused as broken. */
  broken: number;
  /** Lines the generator broke on purpose: Peras must refuse exactly these. */
  injected: number;
  /** Milliseconds since the stream started, when the batch ended. */
  at: number;
}

/** The running totals of the stream, merged from each batch's digest. */
export interface Totals {
  lines: number;
  broken: number;
  kept: number;
  zones: Map<string, Zone>;
  histogram: number[];
  hottestSensor: number;
  hottest: number;
}

export function emptyTotals(bins: number): Totals {
  return {
    lines: 0,
    broken: 0,
    kept: 0,
    zones: new Map(),
    histogram: new Array<number>(bins).fill(0),
    hottestSensor: -1,
    hottest: -1000,
  };
}

/** Adds a batch's digest to the totals, in place. */
export function merge(totals: Totals, d: Digest): void {
  totals.lines += d.lines;
  totals.broken += d.broken;
  totals.kept += d.kept;
  for (const z of d.zones) {
    const t = totals.zones.get(z.name);
    if (t === undefined) {
      totals.zones.set(z.name, { ...z });
      continue;
    }
    t.readings += z.readings;
    t.alerts += z.alerts;
    t.tempSum += z.tempSum;
    t.tempMin = Math.min(t.tempMin, z.tempMin);
    t.tempMax = Math.max(t.tempMax, z.tempMax);
    t.humiditySum += z.humiditySum;
    t.energy += z.energy;
  }
  d.histogram.forEach((n, i) => {
    totals.histogram[i] = (totals.histogram[i] ?? 0) + n;
  });
  if (d.hottest > totals.hottest) {
    totals.hottest = d.hottest;
    totals.hottestSensor = d.hottestSensor;
  }
}

/** The `p`-th percentile (0 to 100) of `values`, nearest rank; NaN when empty. */
export function percentile(values: readonly number[], p: number): number {
  if (values.length === 0) return Number.NaN;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return sorted[rank - 1] ?? Number.NaN;
}

export interface Summary {
  batches: number;
  rows: number;
  bytes: number;
  /** Milliseconds of Peras's digest, over every batch. */
  perasMs: number;
  /** Rows per second of Peras's digest alone, over every batch. */
  perasRowsPerSecond: number;
  /** Rows per second of the TypeScript digest alone, over the compared batches. */
  typescriptRowsPerSecond: number | null;
  /** TypeScript's time over Peras's, on the batches where both ran. */
  speedup: number | null;
  perasP50: number;
  perasP95: number;
  perasP99: number;
  typescriptP50: number | null;
  typescriptP95: number | null;
  /** Compared batches, and how many of them disagreed. */
  compared: number;
  disagreed: number;
  /** The largest number of Peras blocks alive after a call. */
  maxLive: number;
  /** Lines refused, lines broken on purpose, and batches where the two differ. */
  broken: number;
  injected: number;
  brokenMismatches: number;
}

export function summarize(samples: readonly Sample[]): Summary {
  const rows = samples.reduce((n, s) => n + s.rows, 0);
  const bytes = samples.reduce((n, s) => n + s.bytes, 0);
  const perasMs = samples.reduce((n, s) => n + s.perasMs, 0);
  const both = samples.filter((s) => s.typescriptMs !== null);
  const tsMs = both.reduce((n, s) => n + (s.typescriptMs ?? 0), 0);
  const tsRows = both.reduce((n, s) => n + s.rows, 0);
  const perasOnBoth = both.reduce((n, s) => n + s.perasMs, 0);
  const tsTimes = both.map((s) => s.typescriptMs ?? 0);
  const perasTimes = samples.map((s) => s.perasMs);
  return {
    batches: samples.length,
    rows,
    bytes,
    perasMs,
    perasRowsPerSecond: perasMs > 0 ? (rows / perasMs) * 1000 : 0,
    typescriptRowsPerSecond: both.length > 0 && tsMs > 0 ? (tsRows / tsMs) * 1000 : null,
    speedup: both.length > 0 && perasOnBoth > 0 ? tsMs / perasOnBoth : null,
    perasP50: percentile(perasTimes, 50),
    perasP95: percentile(perasTimes, 95),
    perasP99: percentile(perasTimes, 99),
    typescriptP50: both.length > 0 ? percentile(tsTimes, 50) : null,
    typescriptP95: both.length > 0 ? percentile(tsTimes, 95) : null,
    compared: both.length,
    disagreed: both.filter((s) => s.agree === false).length,
    maxLive: samples.reduce((n, s) => Math.max(n, s.live), 0),
    broken: samples.reduce((n, s) => n + s.broken, 0),
    injected: samples.reduce((n, s) => n + s.injected, 0),
    brokenMismatches: samples.filter((s) => s.broken !== s.injected).length,
  };
}

const columns = [
  "batch",
  "rows",
  "bytes",
  "generateMs",
  "perasMs",
  "typescriptMs",
  "first",
  "agree",
  "live",
  "broken",
  "injected",
  "at",
] as const;

/** The samples as CSV, one batch per line; an empty cell is a value not measured. */
export function toCsv(samples: readonly Sample[]): string {
  const cell = (v: string | number | boolean | null): string => (v === null ? "" : String(v));
  const lines = samples.map((s) => columns.map((c) => cell(s[c])).join(","));
  return `${columns.join(",")}\n${lines.map((l) => `${l}\n`).join("")}`;
}

export interface Report {
  /** What produced the measures: the browser, its cores, the build. */
  environment: Record<string, string | number | boolean | null>;
  settings: Record<string, string | number | boolean>;
  summary: Summary;
  samples: Sample[];
}

/** The report as JSON, the summary first, then every sample. */
export function toJson(report: Report): string {
  return `${JSON.stringify(report, null, 2)}\n`;
}
