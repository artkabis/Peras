import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Enumerations cross into JavaScript as `{ kind, values }`, typed as unions.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const shapes = fileURLToPath(new URL("fixtures/enums/shapes.peras", import.meta.url));

type Shape =
  | { kind: "Circle"; values: [number] }
  | { kind: "Rect"; values: [number, number] }
  | { kind: "Empty"; values: [] };
type Event =
  | { kind: "Text"; values: [string] }
  | { kind: "Points"; values: [readonly number[]] }
  | { kind: "Quit"; values: [] };

interface Shapes {
  area(s: Shape): number;
  grow(s: Shape): Shape;
  describe(e: Event): string;
  echo(e: Event): Event;
  first(names: readonly string[]): string | undefined;
  total(shapes: readonly Shape[]): number;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(): Promise<{ m: Shapes; dts: string }> {
  const compiled = await compile(shapes, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Shapes, dts: compiled.dts };
}

describe("enumerations in JavaScript", () => {
  it("are objects with a kind and values, in and out", async () => {
    const { m } = await load();
    expect(m.area({ kind: "Circle", values: [1] })).toBe(3);
    expect(m.area({ kind: "Empty", values: [] })).toBe(0);
    expect(m.grow({ kind: "Rect", values: [2, 3] })).toEqual({ kind: "Rect", values: [3, 2] });
    expect(
      m.total([
        { kind: "Circle", values: [1] },
        { kind: "Rect", values: [2, 2] },
      ]),
    ).toBe(7);
  });

  it("carry strings and arrays, and release them whatever the variant", async () => {
    const { m } = await load();
    expect(m.describe({ kind: "Text", values: ["hé"] })).toBe("text: hé");
    expect(m.echo({ kind: "Points", values: [[1, 2]] })).toEqual({
      kind: "Points",
      values: [[1, 2]],
    });
    expect(m.echo({ kind: "Quit", values: [] })).toEqual({ kind: "Quit", values: [] });
    expect(m.first(["a", "b"])).toBe("b");
    expect(m.first([])).toBeUndefined();
    m.__perasHeap.checkLeaks();
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("refuse an unknown kind", async () => {
    const { m } = await load();
    expect(() => m.area({ kind: "Square", values: [1] } as unknown as Shape)).toThrow(TypeError);
  });

  it("are declared to TypeScript as unions", async () => {
    const { dts } = await load();
    expect(dts).toContain(
      'export type Shape = { kind: "Circle"; values: [number] } | { kind: "Rect"; values: [number, number] } | { kind: "Empty"; values: [] };',
    );
  });
});
