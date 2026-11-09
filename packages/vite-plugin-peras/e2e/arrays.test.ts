import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Arrays cross into JavaScript as copies; the debug allocator must end empty.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const numbers = fileURLToPath(new URL("fixtures/arrays/numbers.peras", import.meta.url));

interface Stats {
  values: number[];
  count: number;
}

interface Numbers {
  evens(n: number): number[];
  sum(xs: readonly number[]): number;
  scale(xs: number[], k: number): void;
  at(xs: readonly number[], i: number): number;
  wide(xs: readonly bigint[]): bigint[];
  flags(n: number): boolean[];
  stats(values: readonly number[]): Stats;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(): Promise<{ m: Numbers; dts: string }> {
  const compiled = await compile(numbers, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Numbers, dts: compiled.dts };
}

describe("arrays in JavaScript", () => {
  it("are copied in and out, of every element type", async () => {
    const { m } = await load();
    expect(m.evens(9)).toEqual([0, 2, 4, 6, 8]);
    expect(m.evens(0)).toEqual([]);
    expect(m.sum([1.5, 2.5, 3])).toBe(7);
    expect(m.wide([1n, -2n])).toEqual([1n, -2n, 9007199254740993n]);
    expect(m.flags(4)).toEqual([true, false, false, true]);
    expect(m.stats([2, 4])).toEqual({ values: [2, 4], count: 2 });
    const big = Array.from({ length: 100_000 }, (_, i) => i);
    expect(m.sum(big)).toBe(4_999_950_000);
    m.__perasHeap.checkLeaks();
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("copy an inout parameter back, and leave others untouched", async () => {
    const { m } = await load();
    const xs = [1, 2, 3];
    m.scale(xs, 2.5);
    expect(xs).toEqual([2.5, 5, 7.5]);
    const kept = [1n];
    m.wide(kept);
    expect(kept).toEqual([1n]);
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("refuse an index out of bounds with a RangeError, and leak nothing", async () => {
    const { m } = await load();
    expect(m.at([5, 6], 1)).toBe(6);
    expect(() => m.at([5, 6], 2)).toThrow(RangeError);
    expect(() => m.at([5, 6], -1)).toThrow("index -1 is out of bounds: the length is 2");
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("are declared to TypeScript, read-only when only read", async () => {
    const { dts } = await load();
    expect(dts).toContain("export declare function evens(n: number): number[];");
    expect(dts).toContain(
      'export declare function sum(xs: readonly number[] | Float64Array | PerasArray<"f64">): number;',
    );
    expect(dts).toContain(
      'export declare function scale(xs: number[] | Float64Array | PerasArray<"f64">, k: number): void;',
    );
    expect(dts).toContain("export interface Stats { values: number[]; count: number; }");
  });
});
