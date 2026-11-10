import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Maps and sets cross between JavaScript and the module as copies, in
// insertion order, both ways; the allocators must end empty.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(
  new URL("fixtures/boundary-maps/boundary-maps.peras", import.meta.url),
);

interface Stock {
  name: string;
  price: number;
  tags: string[];
  active: boolean;
}

interface Maps {
  total(prices: ReadonlyMap<string, number>): number;
  invert(m: ReadonlyMap<string, number>): Map<number, string>;
  stocks(names: ReadonlySet<string>): Map<string, Stock>;
  bump(counts: Map<bigint, number>, key: bigint): void;
  evens(n: number): Set<number>;
  nested(groups: ReadonlyMap<string, ReadonlyMap<number, boolean>>): number;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<{ m: Maps; dts: string }> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Maps, dts: compiled.dts };
}

/** A deterministic stream of numbers in [0, 1). */
function random(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

describe("maps and sets in JavaScript", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`cross in and out in insertion order, on ${allocator}`, async () => {
      const { m } = await load(allocator);
      const next = random(23);
      for (const size of [0, 1, 7, 2000]) {
        const words = ["été", "a", "", "日本", "z", "🦀"];
        const prices = new Map<string, number>();
        const counts = new Map<string, number>();
        for (let i = 0; i < size; i++) {
          const word = `${words[i % words.length] ?? ""}${String(Math.floor(next() * size))}`;
          prices.set(word, Math.floor(next() * 1000) / 8);
          counts.set(word, Math.floor(next() * 50) - 25);
        }
        let sum = 0;
        for (const p of prices.values()) sum += p;
        expect(m.total(prices)).toBe(sum);

        const inverted = new Map<number, string>();
        for (const [k, v] of counts) inverted.set(v, k);
        expect([...m.invert(counts)]).toEqual([...inverted]);

        const names = new Set(counts.keys());
        const stocks = m.stocks(names);
        expect([...stocks.keys()]).toEqual([...names]);
        [...names].forEach((n, i) => {
          expect(stocks.get(n)).toEqual({
            name: `${n}!`,
            price: i * 1.5,
            tags: [n, "x"],
            active: i % 2 === 0,
          });
        });

        const bumped = new Map<bigint, number>([[3n, 1]]);
        const want = new Map(bumped);
        for (let i = 0; i < size % 50; i++) {
          const key = BigInt(Math.floor(next() * 10) - 5);
          m.bump(bumped, key);
          want.set(key, (want.get(key) ?? 0) + 1);
        }
        expect([...bumped]).toEqual([...want]);

        expect([...m.evens(size % 30)]).toEqual(
          Array.from({ length: Math.ceil((size % 30) / 2) }, (_, i) => i * 2),
        );
      }
      const groups = new Map([
        [
          "g",
          new Map([
            [1, true],
            [2, false],
            [4, true],
          ]),
        ],
        ["h", new Map([[10, true]])],
        ["empty", new Map<number, boolean>()],
      ]);
      expect(m.nested(groups)).toBe(15);
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }

  it("are declared to TypeScript, read-only when only read", async () => {
    const { dts } = await load("debug");
    expect(dts).toContain(
      "export declare function total(prices: ReadonlyMap<string, number>): number;",
    );
    expect(dts).toContain(
      "export declare function invert(m: ReadonlyMap<string, number>): Map<number, string>;",
    );
    expect(dts).toContain(
      "export declare function stocks(names: ReadonlySet<string>): Map<string, Stock>;",
    );
    expect(dts).toContain(
      "export declare function bump(counts: Map<bigint, number>, key: bigint): void;",
    );
    expect(dts).toContain(
      "export interface Stock { name: string; price: number; tags: string[]; active: boolean; }",
    );
  });
});
