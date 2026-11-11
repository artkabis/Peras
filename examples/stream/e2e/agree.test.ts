import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../../../packages/vite-plugin-peras/src/index.ts";
import { batch, injected, isBroken } from "../src/generate.ts";
import { canonical, digest as reference, type Digest } from "../src/digest.ts";

// The stream's digest in Peras gives, batch by batch, the very numbers of the
// TypeScript one, the sums of floats included: both add in the same order.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("../src/telemetry.peras", import.meta.url));

interface Telemetry {
  digest(text: string): Digest;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<Telemetry> {
  const compiled = await compile(source, { perasc, allocator });
  const url = `data:text/javascript;base64,${Buffer.from(moduleCode(compiled)).toString("base64")}`;
  return (await import(url)) as Telemetry;
}

describe("the stream's digest", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`is the TypeScript one, batch by batch, on ${allocator}`, async () => {
      const m = await load(allocator);
      let broken = 0;
      for (let b = 0; b < 12; b++) {
        const text = batch(b * 20000, 20000);
        const got = m.digest(text);
        expect(canonical(got)).toBe(canonical(reference(text)));
        // The lines refused are as many as the lines broken on purpose.
        expect(got.broken).toBe(injected(b * 20000, 20000));
        broken += got.broken;
      }
      // Every kind of broken row came, and none was taken for a reading.
      expect(broken).toBeGreaterThan(400);
      for (const text of ["", "\n", "1,2,hall,20.5,40.0,1.000", "x\n\n1,2,hall,1e400,40,1\n"])
        expect(canonical(m.digest(text))).toBe(canonical(reference(text)));
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`refuses exactly the lines broken on purpose, one by one, on ${allocator}`, async () => {
      const m = await load(allocator);
      const lines = batch(0, 40000).split("\n").slice(0, -1);
      let refused = 0;
      lines.forEach((line, i) => {
        const d = m.digest(line);
        expect(d.broken, line).toBe(isBroken(i) ? 1 : 0);
        expect(d.kept, line).toBe(isBroken(i) ? 0 : 1);
        refused += d.broken;
      });
      expect(refused).toBeGreaterThan(50);
      m.__perasHeap.checkLeaks();
    });
  }

  it("is stricter than a lenient reading, which takes broken lines for readings", () => {
    // What parseFloat makes of the broken values the stream carries: numbers,
    // silently wrong. parseF64 refuses each of them.
    expect(Number.parseFloat("21,37")).toBe(21);
    expect(Number.parseFloat("45.3%")).toBe(45.3);
    expect(Number.isNaN(Number.parseFloat("NaN"))).toBe(true);
    const line = "1793491200,7,lab,21,37,45.3,1.500";
    expect(reference(line).broken).toBe(1);
  });
});
