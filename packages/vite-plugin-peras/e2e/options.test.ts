import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Options cross into JavaScript as a value or `undefined`, and back.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const maybe = fileURLToPath(new URL("fixtures/options/maybe.peras", import.meta.url));

interface Range {
  low: number;
  high: number;
}

interface Maybe {
  half(n: number): number | undefined;
  quarter(n: number): number | undefined;
  widen(r: Range | undefined, by: number): Range | undefined;
  product(a: bigint, b: bigint): bigint | undefined;
  either(a: number | undefined, fallback: number): number;
}

async function load(): Promise<{ m: Maybe; dts: string }> {
  const compiled = await compile(maybe, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Maybe, dts: compiled.dts };
}

describe("options in JavaScript", () => {
  it("are a value or undefined, and `?` returns early", async () => {
    const { m } = await load();
    expect(m.half(10)).toBe(5);
    expect(m.half(7)).toBeUndefined();
    expect(m.quarter(12)).toBe(3);
    expect(m.quarter(6)).toBeUndefined();
  });

  it("hold structures, in and out", async () => {
    const { m } = await load();
    expect(m.widen({ low: 1, high: 5 }, 2)).toEqual({ low: -1, high: 7 });
    expect(m.widen(undefined, 2)).toBeUndefined();
    expect(m.widen({ low: -2147483647, high: 0 }, 2)).toBeUndefined();
  });

  it("report overflow instead of wrapping", async () => {
    const { m } = await load();
    expect(m.product(3n, 4n)).toBe(12n);
    expect(m.product(9223372036854775807n, 2n)).toBeUndefined();
    expect(m.either(undefined, 1.5)).toBe(1.5);
    expect(m.either(2.5, 1.5)).toBe(2.5);
  });

  it("are declared to TypeScript as `T | undefined`", async () => {
    const { dts } = await load();
    expect(dts).toContain("export declare function half(n: number): number | undefined;");
    expect(dts).toContain(
      "export declare function widen(r: Range | undefined, by: number): Range | undefined;",
    );
  });
});
