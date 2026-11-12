import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Generic code is compiled once per set of type arguments; exported functions
// call it with concrete types, and generic structures and enumerations reach
// JavaScript under names made from their arguments.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/generics/generic.peras", import.meta.url));

type Maybe = { kind: "Just"; values: [number] } | { kind: "Nothing"; values: [] };

interface Generic {
  largerInt(a: number, b: number): number;
  largerFloat(a: number, b: number): number;
  total(xs: readonly bigint[], floats: readonly number[]): number;
  named(n: number, name: string): { first: number; second: string };
  half(n: number): Maybe;
  lastWord(words: readonly string[]): string;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(): Promise<{ m: Generic; dts: string }> {
  const compiled = await compile(source, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Generic, dts: compiled.dts };
}

describe("generics in JavaScript", () => {
  it("run one copy per type", async () => {
    const { m } = await load();
    expect(m.largerInt(3, -4)).toBe(3);
    expect(m.largerFloat(1.5, 2.25)).toBe(2.25);
    expect(m.total([1n, 2n, 3n], [0.5, 0.25])).toBe(6.75);
  });

  it("carry generic structures, enumerations and strings without a leak", async () => {
    const { m } = await load();
    expect(m.named(7, "seven")).toEqual({ first: 7, second: "seven" });
    expect(m.half(8)).toEqual({ kind: "Just", values: [4] });
    expect(m.half(3)).toEqual({ kind: "Nothing", values: [] });
    expect(m.lastWord(["a", "été"])).toBe("été");
    expect(m.lastWord([])).toBe("none");
    m.__perasHeap.checkLeaks();
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("are declared to TypeScript under the names of their instances", async () => {
    const { dts } = await load();
    expect(dts).toContain("export interface Pair_i32_String { first: number; second: string; }");
    expect(dts).toContain(
      'export type Maybe_i32 = { kind: "Just"; values: [number] } | { kind: "Nothing"; values: [] };',
    );
    expect(dts).toContain(
      "export declare function named(n: number, name: string | Uint8Array): Pair_i32_String;",
    );
    expect(dts).not.toContain("larger(");
  });
});
