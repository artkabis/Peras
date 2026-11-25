import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { applications, bugs, frames, machine, ratio, tally } from "../src/recorded.ts";

// The page reads the repository's results files as the benchmarks write
// them: read here from the same files, so that a change of their format
// fails a test, not the page in silence.
const read = (path: string) =>
  readFileSync(fileURLToPath(new URL(`../../../${path}`, import.meta.url)), "utf8");

describe("the recorded figures", () => {
  it("reads a ratio with or without its range", () => {
    expect(ratio("5.800 (2.945 to 7.082)")).toEqual({ median: 5.8, low: 2.945, high: 7.082 });
    expect(ratio("1.798")).toEqual({ median: 1.798 });
    expect(ratio("–")).toBeUndefined();
  });

  it("reads decision 0046's four applications with their control", () => {
    const apps = applications(read("bench/next/RESULTS.md"));
    expect(apps.map((a) => a.label.slice(0, 2))).toEqual(["A1", "A2", "A3", "A4"]);
    for (const a of apps) {
      expect(a.peras.median).toBeGreaterThan(0);
      expect(a.peras.low).toBeLessThanOrEqual(a.peras.median);
      expect(a.control?.median).toBeGreaterThan(0);
    }
  });

  it("reads decision 0047's programs, f3 against TypeScript, f0 before it", () => {
    const text = read("bench/frame/RESULTS.md");
    const progs = frames(text);
    expect(progs.map((p) => p.label.slice(0, 2))).toEqual(["A3", "P2"]);
    for (const p of progs) {
      expect(p.before?.median).toBeGreaterThan(p.peras.median);
      expect(p.control?.median).toBeGreaterThan(0.9);
    }
    expect(machine(text)).toMatch(/cores/);
  });

  it("reads the seventeen bugs and tallies where each toolchain stops them", () => {
    const list = bugs(read("bench/web-bugs/RESULTS.md"));
    expect(list).toHaveLength(17);
    const ts = tally(list, "typescript");
    const peras = tally(list, "peras");
    expect(ts.compile + ts.run + ts.silent + ts.none).toBe(17);
    expect(peras.compile + peras.run + peras.silent + peras.none).toBe(17);
  });
});
