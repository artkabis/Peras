import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../../../packages/vite-plugin-peras/src/index.ts";
import { batch } from "../src/generate.ts";
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
        broken += got.broken;
      }
      // Every kind of broken row came, and none was taken for a reading.
      expect(broken).toBeGreaterThan(400);
      for (const text of ["", "\n", "1,2,hall,20.5,40.0,1.000", "x\n\n1,2,hall,1e400,40,1\n"])
        expect(canonical(m.digest(text))).toBe(canonical(reference(text)));
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }
});
