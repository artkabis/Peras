import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Methods and interfaces work behind exported functions; tuples cross into
// JavaScript as arrays; module constants are their values.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/methods/shapes.peras", import.meta.url));

interface Shapes {
  areas(circles: readonly { r: number }[], squares: readonly { side: number }[]): number;
  scaled(r: number, k: number): number;
  bounds(xs: readonly number[]): [number, number];
  label(pair: [number, string]): string;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(): Promise<{ m: Shapes; dts: string }> {
  const compiled = await compile(source, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Shapes, dts: compiled.dts };
}

describe("methods, interfaces and tuples in JavaScript", () => {
  it("call the method of each type through an interface", async () => {
    const { m } = await load();
    expect(m.areas([{ r: 1 }, { r: 2 }], [{ side: 3 }])).toBe(24);
    expect(m.scaled(1, 2)).toBe(15);
  });

  it("pass tuples as arrays, and read constants, without a leak", async () => {
    const { m } = await load();
    expect(m.bounds([3, -2, 7])).toEqual([-2, 7]);
    expect(m.label([1, "shape"])).toBe("shape: square");
    m.__perasHeap.checkLeaks();
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("stop at an index out of bounds in a constant", async () => {
    const { m } = await load();
    // A trap ends the call where it is: what it had allocated in the module
    // stays allocated, so the heap is not checked after it.
    expect(() => m.label([2, "x"])).toThrow(RangeError);
  });

  it("declare tuples to TypeScript as tuples", async () => {
    const { dts } = await load();
    expect(dts).toContain(
      'export declare function bounds(xs: readonly number[] | PerasArray<"i32">): [number, number];',
    );
    expect(dts).toContain("export declare function label(pair: [number, string]): string;");
    expect(dts).not.toContain("Circle_area");
  });
});
