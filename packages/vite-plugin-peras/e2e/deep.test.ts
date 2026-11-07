import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Arrays of strings, of arrays, and of structures holding them.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const nested = fileURLToPath(new URL("fixtures/deep/nested.peras", import.meta.url));

interface User {
  name: string;
  tags: number[];
  age: number;
}

interface Point {
  x: number;
  y: number;
}

interface Path {
  label: string;
  points: Point[];
}

interface Nested {
  shout(names: readonly string[]): string[];
  matrix(n: number): number[][];
  older(users: User[]): void;
  lengths(paths: readonly Path[]): number[];
  echo(paths: readonly Path[]): Path[];
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(): Promise<{ m: Nested; dts: string }> {
  const compiled = await compile(nested, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Nested, dts: compiled.dts };
}

describe("arrays of buffers in JavaScript", () => {
  it("cross in and out, at any depth, without a leak", async () => {
    const { m } = await load();
    expect(m.shout(["a", "bé"])).toEqual(["a!", "bé!"]);
    expect(m.matrix(2)).toEqual([
      [0, 1],
      [2, 3],
    ]);
    const paths = [
      { label: "ab", points: [{ x: 1, y: 2 }] },
      { label: "", points: [] },
    ];
    expect(m.lengths(paths)).toEqual([201, 0]);
    expect(m.echo(paths)).toEqual(paths);
    m.__perasHeap.checkLeaks();
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("passed inout, have their strings and arrays rewritten", async () => {
    const { m, dts } = await load();
    const users = [{ name: "ann", tags: [1], age: 30 }];
    m.older(users);
    expect(users).toEqual([{ name: "ann+", tags: [1, 31], age: 31 }]);
    expect(m.__perasHeap.live()).toBe(0);
    expect(dts).toContain("export declare function matrix(n: number): number[][];");
    expect(dts).toContain("export interface User { name: string; tags: number[]; age: number; }");
  });
});
