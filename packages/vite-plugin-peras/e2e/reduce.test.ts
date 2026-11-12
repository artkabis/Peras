import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// `reduce` with an accumulator that holds a string or an array (decision
// 0021, finding F307), against the same `reduce` written in JavaScript, and
// nothing left alive afterwards.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/reduce/reduce.peras", import.meta.url));

interface Reduce {
  joined(words: string[]): string;
  last(words: string[], start: string): string;
  widest(words: string[]): string;
  doubled(xs: number[]): number[];
  tally(words: string[]): { text: string; count: number };
  total(words: string[]): number;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<Reduce> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Reduce;
}

const lists: string[][] = [
  [],
  ["a"],
  ["ab", "", "été", "x;y"],
  Array.from({ length: 300 }, (_, i) => `w${String(i)}`),
];

// Peras counts bytes of UTF-8 and changes the case of ASCII letters only
// (reference/methods.md).
const bytes = (s: string): number => new TextEncoder().encode(s).length;
const upper = (s: string): string => s.replace(/[a-z]/g, (c) => c.toUpperCase());

describe("reduce with a string or an array as its accumulator", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`computes what JavaScript's reduce computes, on ${allocator}`, async () => {
      const m = await load(allocator);
      for (const words of lists) {
        expect(m.joined(words)).toBe(words.reduce((acc, w) => acc + w + ";", ""));
        expect(m.last(words, "s")).toBe(words.reduce((_acc, w) => w, "s") + "s");
        expect(m.widest(words)).toBe(
          words.reduce((acc, w) => acc.slice(0, 2) + w.slice(0, 1), "--"),
        );
        expect(m.tally(words)).toEqual(
          words.reduce((acc, w) => ({ text: acc.text + upper(w), count: acc.count + bytes(w) }), {
            text: "",
            count: 0,
          }),
        );
        expect(m.total(words)).toBe(bytes(words.reduce((acc, w) => acc + w, "")));
      }
      for (const xs of [[], [1], [3, -4, 5], Array.from({ length: 200 }, (_, i) => i)]) {
        expect(m.doubled(xs)).toEqual(xs.reduce((acc: number[], x) => acc.concat([x, x]), [0]));
      }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }
});
