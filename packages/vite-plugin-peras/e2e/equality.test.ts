import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// `==` on arrays of strings and of arrays (decision 0021, finding F305),
// against the same comparison written in JavaScript, element by element.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/equality/equality.peras", import.meta.url));

interface Item {
  name: string;
  n: number;
  scores: number[];
}

interface Equality {
  sameStrings(a: string[], b: string[]): boolean;
  differentGrids(a: number[][], b: number[][]): boolean;
  sameWords(a: string[][], b: string[][]): boolean;
  sameItems(a: Item[], b: Item[]): boolean;
  splitsAs(text: string, parts: string[]): boolean;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<Equality> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Equality;
}

/** `==` as Peras defines it on arrays: same length, each element `==`. */
function same(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((x, i) => same(x, b[i]));
  }
  if (typeof a === "object" && a !== null && typeof b === "object" && b !== null) {
    const keys = Object.keys(a);
    return keys.every((k) =>
      same((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]),
    );
  }
  return a === b;
}

describe("== on arrays of strings and of arrays", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`compares element by element as JavaScript would, on ${allocator}`, async () => {
      const m = await load(allocator);
      const strings: [string[], string[]][] = [
        [[], []],
        [["a"], ["a"]],
        [
          ["a", "b"],
          ["a", "b"],
        ],
        [
          ["a", "b"],
          ["a", "c"],
        ],
        [["a"], ["a", ""]],
        [
          ["été", ""],
          ["été", ""],
        ],
        [
          ["ab", "c"],
          ["a", "bc"],
        ],
      ];
      for (const [a, b] of strings) expect(m.sameStrings(a, b)).toBe(same(a, b));
      const grids: [number[][], number[][]][] = [
        [
          [[1, 2], []],
          [[1, 2], []],
        ],
        [[[0]], [[-0]]],
        [[[NaN]], [[NaN]]],
        [[[1.5, 2]], [[1.5, 2.0000001]]],
        [[[1], [2]], [[1, 2]]],
      ];
      for (const [a, b] of grids) expect(m.differentGrids(a, b)).toBe(!same(a, b));
      const words: [string[][], string[][]][] = [
        [
          [["a", "b"], ["c"]],
          [["a", "b"], ["c"]],
        ],
        [
          [["a", "b"], ["c"]],
          [["a", "b"], ["d"]],
        ],
        [[[]], [[""]]],
      ];
      for (const [a, b] of words) expect(m.sameWords(a, b)).toBe(same(a, b));
      const item = (name: string, n: number, scores: number[]): Item => ({ name, n, scores });
      const items: [Item[], Item[]][] = [
        [[item("x", 1, [0.5])], [item("x", 1, [0.5])]],
        [[item("x", 1, [0.5])], [item("y", 1, [0.5])]],
        [[item("x", 1, [0.5])], [item("x", 2, [0.5])]],
        [[item("x", 1, [0.5])], [item("x", 1, [0.5, 1])]],
        [[item("x", 1, [NaN])], [item("x", 1, [NaN])]],
      ];
      for (const [a, b] of items) expect(m.sameItems(a, b)).toBe(same(a, b));
      expect(m.splitsAs("a,b,,c", ["a", "b", "", "c"])).toBe(true);
      expect(m.splitsAs("a,b", ["a", "b", ""])).toBe(false);
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }
});
