import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Structures cross into JavaScript as objects, and back. The module is the one
// Vite serves, imported as an ES module.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const geo = fileURLToPath(new URL("fixtures/structs/geo.peras", import.meta.url));

interface Vec2 {
  x: number;
  y: number;
}

interface Segment {
  from: Vec2;
  to: Vec2;
  closed: boolean;
}

interface Geo {
  middle(s: Segment): Vec2;
  flip(s: Segment): Segment;
  swapped(x: number, y: number): number;
  walk(n: number): number;
}

async function load(): Promise<{ geo: Geo; dts: string }> {
  const compiled = await compile(geo, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { geo: (await import(url)) as Geo, dts: compiled.dts };
}

describe("structures in JavaScript", () => {
  it("are objects in and out, nested and with booleans", async () => {
    const { geo: m } = await load();
    const s = { from: { x: 1, y: 2 }, to: { x: 5, y: 10 }, closed: false };
    expect(m.middle(s)).toEqual({ x: 3, y: 6 });
    expect(m.flip(s)).toEqual({ from: { x: 5, y: 10 }, to: { x: 1, y: 2 }, closed: true });
    // The input object is untouched: structures are values.
    expect(s).toEqual({ from: { x: 1, y: 2 }, to: { x: 5, y: 10 }, closed: false });
  });

  it("are values inside: swaps, loops and calls returning several results", async () => {
    const { geo: m } = await load();
    expect(m.swapped(3, 4)).toBe(1403);
    expect(m.walk(5)).toBe(15);
  });

  it("are declared to TypeScript as interfaces", async () => {
    const { dts } = await load();
    expect(dts).toContain("export interface Vec2 { x: number; y: number; }");
    expect(dts).toContain("export interface Segment { from: Vec2; to: Vec2; closed: boolean; }");
    expect(dts).toContain("export declare function flip(s: Segment): Segment;");
  });
});
