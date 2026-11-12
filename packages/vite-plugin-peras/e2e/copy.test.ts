import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// `copy()` on a structure (decision 0021, finding F102): the copy changed,
// the original left as it was, and nothing left alive afterwards.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/copy/copy.peras", import.meta.url));

interface Copy {
  changed(seed: number): string;
  kept(n: number): string;
  temporary(seed: number): number;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<Copy> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Copy;
}

describe("copy() on a structure", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`gives a copy of its own, on ${allocator}`, async () => {
      const m = await load(allocator);
      for (const seed of [0, 3, -5]) {
        const s = String(seed);
        expect(m.changed(seed)).toBe(
          `r${s},x+yy,${s},2,n,t,in,2,1.5|r${s}!,w+yy+zzz,99,2,n,t,out,3,2.5`,
        );
        expect(m.temporary(seed)).toBe(2 + seed);
      }
      expect(m.kept(0)).toBe("");
      expect(m.kept(7)).toBe("l0,l2,l4,l6");
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }
});
