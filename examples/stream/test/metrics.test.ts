import { describe, expect, it } from "vitest";
import { emptyTotals, merge, percentile, summarize, toCsv, type Sample } from "../src/metrics.ts";

const sample = (over: Partial<Sample>): Sample => ({
  batch: 0,
  rows: 1000,
  bytes: 38000,
  generateMs: 1,
  perasMs: 10,
  typescriptMs: 20,
  first: "peras",
  agree: true,
  live: 0,
  broken: 2,
  injected: 2,
  at: 0,
  ...over,
});

describe("the stream's metrics", () => {
  it("takes percentiles by nearest rank", () => {
    expect(percentile([5, 1, 4, 2, 3], 50)).toBe(3);
    expect(percentile([5, 1, 4, 2, 3], 95)).toBe(5);
    expect(percentile([7], 99)).toBe(7);
    expect(percentile([], 50)).toBeNaN();
  });

  it("compares the two digests only on the batches where both ran", () => {
    const s = summarize([
      sample({ perasMs: 10, typescriptMs: 30 }),
      sample({ perasMs: 10, typescriptMs: 10, agree: false }),
      sample({ perasMs: 20, typescriptMs: null, agree: null, live: 3 }),
    ]);
    expect(s.rows).toBe(3000);
    expect(s.perasRowsPerSecond).toBe(75000);
    expect(s.typescriptRowsPerSecond).toBe(50000);
    expect(s.speedup).toBe(2);
    expect(s.compared).toBe(2);
    expect(s.disagreed).toBe(1);
    expect(s.maxLive).toBe(3);
  });

  it("merges digests into totals", () => {
    const t = emptyTotals(2);
    const zone = {
      name: "hall",
      readings: 2,
      alerts: 1,
      tempSum: 40,
      tempMin: 18,
      tempMax: 22,
      humiditySum: 80,
      energy: 3,
    };
    merge(t, {
      lines: 3,
      broken: 1,
      kept: 2,
      zones: [zone],
      histogram: [1, 1],
      hottestSensor: 4,
      hottest: 22,
    });
    merge(t, {
      lines: 1,
      broken: 0,
      kept: 1,
      zones: [{ ...zone, readings: 1, tempMin: 15, tempMax: 15, tempSum: 15 }],
      histogram: [0, 1],
      hottestSensor: 9,
      hottest: 15,
    });
    expect(t.kept).toBe(3);
    expect(t.zones.get("hall")).toMatchObject({
      readings: 3,
      tempSum: 55,
      tempMin: 15,
      tempMax: 22,
    });
    expect(t.histogram).toEqual([1, 2]);
    expect(t.hottestSensor).toBe(4);
    // The first zone's object is a copy: the digest is not changed.
    expect(zone.readings).toBe(2);
  });

  it("writes one CSV line per batch, an empty cell for what was not measured", () => {
    const csv = toCsv([sample({ typescriptMs: null, agree: null })]);
    expect(csv).toBe(
      "batch,rows,bytes,generateMs,perasMs,typescriptMs,first,agree,live,broken,injected,at\n0,1000,38000,1,10,,peras,,0,2,2,0\n",
    );
  });
});
