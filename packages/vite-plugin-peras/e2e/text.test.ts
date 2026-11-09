import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// The methods of strings, run by the runtime of both allocators, across the
// JavaScript boundary: UTF-8 in, UTF-8 out, nothing left alive.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/text/words.peras", import.meta.url));

interface Words {
  shout(s: string): string;
  fields(line: string): string[];
  tidy(line: string): string;
  middle(s: string, start: number, end: number): string;
  find(s: string, needle: string): number;
  characters(s: string): number;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<Words> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Words;
}

describe("strings in JavaScript", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`split, trim, change case, replace and join, on ${allocator}`, async () => {
      const m = await load(allocator);
      expect(m.shout("  hello big world ")).toBe("HELLO_BIG_WORLD");
      expect(m.fields(" a, b ,c")).toEqual(["a", "b", "c"]);
      expect(m.tidy("Ada, #skip, GRACE")).toBe("ada grace");
      expect(m.find("hello", "l")).toBe(2);
      expect(m.find("hello", "z")).toBe(-1);
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }

  it("count bytes, and never cut a character", async () => {
    const m = await load("tlsf");
    // `é` is two bytes: an index inside it moves back to its start.
    expect(m.middle("aéb", 0, 2)).toBe("a");
    expect(m.middle("aéb", 1, 3)).toBe("é");
    expect(m.middle("hello", -3, 100)).toBe("llo");
    expect(m.characters("hé!")).toBe(3);
    expect(m.__perasHeap.live()).toBe(0);
  });
});

describe("text without SIMD", () => {
  it("gives the same results, and its loader does not check for SIMD", async () => {
    const compiled = await compile(source, { perasc, allocator: "tlsf", simd: false });
    const withSimd = await compile(source, { perasc, allocator: "tlsf" });
    expect(moduleCode(withSimd)).toContain("WebAssembly.validate");
    expect(moduleCode(compiled)).not.toContain("WebAssembly.validate");
    const code = moduleCode(compiled);
    const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
    const scalar = (await import(url)) as Words;
    const simd = await load("tlsf");
    // Over sixteen bytes, where the runtime with SIMD compares them at once.
    const line = " Été, naïve ,# skip , DONNÉES  ,x,  ".repeat(40);
    expect(scalar.tidy(line)).toBe(simd.tidy(line));
    expect(scalar.fields(line)).toEqual(simd.fields(line));
    expect(scalar.characters(line)).toBe(simd.characters(line));
    expect(scalar.__perasHeap.live()).toBe(0);
  });
});
