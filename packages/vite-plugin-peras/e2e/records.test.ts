import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Arrays of structures: one buffer per field inside, arrays of objects outside.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const points = fileURLToPath(new URL("fixtures/records/points.peras", import.meta.url));

interface Point {
  x: number;
  y: number;
  visible: boolean;
}

interface Points {
  centroid(points: readonly Point[]): Point;
  grid(n: number): Point[];
  shift(points: Point[], dx: number): void;
  last_x(points: readonly Point[]): number;
  same(a: readonly Point[], b: readonly Point[]): boolean;
  swap_first(points: Point[]): void;
  maybe(xs: readonly (number | undefined)[]): number;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(): Promise<{ m: Points; dts: string }> {
  const compiled = await compile(points, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Points, dts: compiled.dts };
}

describe("arrays of structures in JavaScript", () => {
  it("are arrays of objects, in and out", async () => {
    const { m } = await load();
    const pts = m.grid(3);
    expect(pts).toEqual([
      { x: 0, y: 0, visible: true },
      { x: 1, y: 1, visible: false },
      { x: 2, y: 4, visible: true },
    ]);
    expect(m.centroid(pts)).toEqual({ x: 1, y: 5 / 3, visible: true });
    expect(m.last_x(pts)).toBe(2);
    expect(m.last_x([])).toBe(-1);
    expect(m.same(pts, m.grid(3))).toBe(true);
    expect(m.maybe([1, undefined, 3])).toBe(104);
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("passed inout, are rewritten element by element", async () => {
    const { m } = await load();
    const pts = m.grid(2);
    m.shift(pts, 10);
    expect(pts).toEqual([
      { x: 10, y: 0, visible: false },
      { x: 11, y: 1, visible: true },
    ]);
    m.swap_first(pts);
    expect(pts.map((p) => p.x)).toEqual([11, 10]);
    m.__perasHeap.checkLeaks();
  });

  it("are declared to TypeScript", async () => {
    const { dts } = await load();
    expect(dts).toContain("export declare function centroid(points: readonly Point[]): Point;");
    expect(dts).toContain(
      "export declare function maybe(xs: readonly (number | undefined)[]): number;",
    );
  });
});
