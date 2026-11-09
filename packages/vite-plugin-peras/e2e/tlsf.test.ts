import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// The production allocator, TLSF, behind the same modules as the debug one:
// the same results, nothing left alive, and memory reused call after call.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const fixture = (path: string) => fileURLToPath(new URL(`fixtures/${path}`, import.meta.url));

interface Heap {
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load<T>(path: string, allocator: "debug" | "tlsf"): Promise<T & Heap> {
  const compiled = await compile(fixture(path), { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as T & Heap;
}

interface Nested {
  shout(names: readonly string[]): string[];
  matrix(n: number): number[][];
}

describe("the TLSF allocator", () => {
  it("gives the results of the debug allocator, leaving nothing alive", async () => {
    for (const allocator of ["debug", "tlsf"] as const) {
      const m = await load<Nested>("deep/nested.peras", allocator);
      expect(m.shout(["a", "bé"])).toEqual(["a!", "bé!"]);
      expect(m.matrix(3)).toEqual([
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
      ]);
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    }
  });

  it("reuses memory over many calls", async () => {
    const m = await load<Nested>("deep/nested.peras", "tlsf");
    const names = Array.from({ length: 50 }, (_, i) => `name ${String(i)}`);
    for (let i = 0; i < 2000; i++) {
      expect(m.shout(names)).toHaveLength(50);
    }
    expect(m.__perasHeap.live()).toBe(0);
  });
});
