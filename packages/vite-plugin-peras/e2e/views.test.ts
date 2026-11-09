import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// A PerasArray stays in the module's memory: exported functions take it with
// no copy, and `view()` reads it as a typed array.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/views/points.peras", import.meta.url));

interface View {
  readonly kind: string;
  readonly length: number;
  view(): Float32Array;
  toArray(): number[];
  release(): void;
}

interface Points {
  sum(xs: readonly number[] | View): number;
  scale(xs: number[] | Float32Array | View, k: number): void;
  grow(xs: number[] | Float32Array | View, n: number): void;
  consume(xs: readonly number[] | View): number;
  __perasHeap: {
    live(): number;
    checkLeaks(): void;
    array(kind: string, init: number | readonly number[]): View;
  };
}

async function load(allocator: "debug" | "tlsf"): Promise<{ m: Points; dts: string }> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Points, dts: compiled.dts };
}

describe("arrays without a copy", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`are lent, changed in place and given, on ${allocator}`, async () => {
      const { m } = await load(allocator);
      const xs = m.__perasHeap.array("f32", [1, 2, 3]);
      expect(m.sum(xs)).toBe(6);
      m.scale(xs, 2);
      expect(Array.from(xs.view())).toEqual([2, 4, 6]);
      // Growing moves the elements: a new view sees them.
      m.grow(xs, 2);
      expect(xs.length).toBe(5);
      expect(xs.toArray()).toEqual([2, 4, 6, 0, 1]);
      // Given to `sink`, it can no longer be used.
      expect(m.consume(xs)).toBe(5);
      expect(() => xs.view()).toThrow("given to a sink parameter");
      const zeros = m.__perasHeap.array("f32", 4);
      expect(Array.from(zeros.view())).toEqual([0, 0, 0, 0]);
      zeros.release();
      // Plain arrays are still copied, as before.
      const plain = [1, 2];
      m.scale(plain, 3);
      expect(plain).toEqual([3, 6]);
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }

  it("copy a typed array back, and refuse to make it grow without a leak", async () => {
    const { m } = await load("tlsf");
    const typed = new Float32Array([1, 2]);
    m.scale(typed, 3);
    expect(Array.from(typed)).toEqual([3, 6]);
    // A typed array has a fixed length: the error comes after every buffer
    // is released.
    expect(() => {
      m.grow(typed, 1);
    }).toThrow(RangeError);
    expect(Array.from(typed)).toEqual([3, 6]);
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("refuse a PerasArray of another kind", async () => {
    const { m } = await load("tlsf");
    const ints = m.__perasHeap.array("i32", 3);
    expect(() => m.sum(ints)).toThrow(TypeError);
    ints.release();
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("are declared to TypeScript", async () => {
    const { dts } = await load("tlsf");
    expect(dts).toContain(
      'export declare function sum(xs: readonly number[] | Float32Array | PerasArray<"f32">): number;',
    );
    expect(dts).toContain("export declare class PerasArray<K extends keyof PerasKinds>");
  });
});
