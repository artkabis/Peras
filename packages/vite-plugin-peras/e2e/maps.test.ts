import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Maps and sets give what JavaScript's Map and Set give, on both allocators,
// and leave nothing alive.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/maps/maps.peras", import.meta.url));

interface Maps {
  frequency(text: string, word: string): number;
  operations(ops: number[], keys: number[], values: number[]): number;
  order(ops: number[], keys: number[]): string;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<Maps> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Maps;
}

/** A deterministic stream of numbers in [0, 1). */
function random(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

const VOCABULARY = ["the", "web", "été", "naïve", "日本", "🦀", "a", "", "The", "loop"];

describe("maps and sets", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`count words as Map and Set do, on ${allocator}`, async () => {
      const m = await load(allocator);
      const next = random(7);
      for (const length of [0, 1, 10, 2000]) {
        const text = Array.from(
          { length },
          () => VOCABULARY[Math.floor(next() * VOCABULARY.length)] ?? "",
        ).join(" ");
        const counts = new Map<string, number>();
        for (const w of text.split(" ")) counts.set(w, (counts.get(w) ?? 0) + 1);
        for (const word of VOCABULARY) {
          expect(m.frequency(text, word), `${String(length)} words, ${word}`).toBe(
            (counts.get(word) ?? 0) * 100000 + counts.size,
          );
        }
      }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`set, read, delete and clear as Map does, on ${allocator}`, async () => {
      const m = await load(allocator);
      const next = random(11);
      // Few keys, so that they come back; many, so that the table grows.
      for (const [count, range] of [
        [0, 1],
        [500, 8],
        [20000, 3000],
        [20000, -1],
      ] as const) {
        const ops = Array.from({ length: count }, () => {
          const r = next();
          return r < 0.4 ? 0 : r < 0.6 ? 1 : r < 0.8 ? 2 : r < 0.9995 ? 3 : 4;
        });
        // A range of -1: any 32-bit key, negative ones included.
        const keys = ops.map(() =>
          range < 0 ? Math.floor(next() * 4294967296) | 0 : Math.floor(next() * range),
        );
        const values = ops.map(() => (next() - 0.5) * 1e6);
        const map = new Map<number, number>();
        let check = 0;
        ops.forEach((op, i) => {
          const k = keys[i] ?? 0;
          if (op === 0) map.set(k, values[i] ?? 0);
          else if (op === 1) check += map.get(k) ?? -1;
          else if (op === 2) check += map.delete(k) ? 1000 : 0;
          else if (op === 3) check += map.has(k) ? 7 : 0;
          else map.clear();
          check = check * 0.5 + map.size;
        });
        expect(m.operations(ops, keys, values), `${String(count)} operations`).toBe(check);
      }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`walk entries in insertion order as Map and Set do, on ${allocator}`, async () => {
      const m = await load(allocator);
      const next = random(13);
      for (const [count, range] of [
        [0, 1],
        [40, 6],
        [3000, 200],
      ] as const) {
        const ops = Array.from({ length: count }, () => {
          const r = next();
          return r < 0.6 ? 0 : r < 0.995 ? 1 : 2;
        });
        const keys = ops.map(() => Math.floor(next() * range) - range / 2);
        const map = new Map<number, number>();
        const seen = new Set<string>();
        ops.forEach((op, i) => {
          const k = keys[i] ?? 0;
          if (op === 0) {
            map.set(k, i);
            seen.add(`k${String(k)}`);
          } else if (op === 1) map.delete(k);
          else map.clear();
        });
        let want = "";
        for (const [k, v] of map) want += `${String(k)}=${String(v)},`;
        want += "|";
        for (const k of seen) want += `${k},`;
        expect(m.order(ops, keys), `${String(count)} operations`).toBe(want);
      }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }
});
