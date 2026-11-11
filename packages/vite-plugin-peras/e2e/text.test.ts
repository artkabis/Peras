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
  csvSum(text: string): number;
  firstKept(line: string): string;
  afterChange(line: string): string;
  partsOf(line: string): string[];
  sameLower(a: string, b: string): boolean;
  sameUpper(a: string, b: string): boolean;
  notLower(a: string, b: string): boolean;
  partsComma(text: string): string;
  partsDash(text: string): string;
  firstLong(text: string, min: number): string;
  countUntil(text: string, stop: string): number;
  nestedLengths(text: string): number;
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
    it(`walk the parts of a split one by one as JavaScript's split gives them, on ${allocator}`, async () => {
      const m = await load(allocator);
      const parts = (text: string, sep: string) => {
        const p = text.split(sep);
        return `${String(p.length)}:${p.map((x) => `${x}|`).join("")}`;
      };
      const texts = [
        "",
        ",",
        ",,",
        "a",
        "a,",
        ",a",
        "a,,b",
        "é,ü,🌿",
        "x -- y",
        " -- ",
        "a -- -- b",
        "a - - b",
      ];
      for (const t of texts) {
        expect(m.partsComma(t), JSON.stringify(t)).toBe(parts(t, ","));
        expect(m.partsDash(t), JSON.stringify(t)).toBe(parts(t, " -- "));
      }
      expect(m.firstLong("ab;abcd;abcdef", 4)).toBe("abcd");
      expect(m.firstLong("ab;cd", 9)).toBe("");
      const until = (t: string, stop: string) => {
        let n = 0;
        for (const p of t.split(",")) {
          if (p === stop) break;
          if (p.length === 0) continue;
          n++;
        }
        return n;
      };
      for (const [t, stop] of [
        ["a,,b,STOP,c", "STOP"],
        ["a,b", "z"],
        ["", ""],
        [",,", "x"],
      ] as const)
        expect(m.countUntil(t, stop), t).toBe(until(t, stop));
      const nested = (t: string) =>
        t
          .split("\n")
          .reduce((n, line) => n + line.split(",").reduce((k, f) => k + f.length * 10 + 1, 0), 0);
      for (const t of ["a,bb\nccc", "", "\n", "x,\n,y\n"])
        expect(m.nestedLengths(t), JSON.stringify(t)).toBe(nested(t));
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`compare a string whose case changes as JavaScript's ASCII rule does, on ${allocator}`, async () => {
      const m = await load(allocator);
      const words = [
        "",
        "A",
        "a",
        "Z",
        "z",
        "@",
        "[",
        "`",
        "{",
        "AbC-9",
        "abc-9",
        "ABC-9",
        "é",
        "É",
        "ÉA",
        "éa",
        "ab",
        "abc",
        "US",
        "us",
      ];
      // Peras changes the case of ASCII letters only, as its documentation says.
      const lower = (s: string) => s.replace(/[A-Z]/g, (c) => c.toLowerCase());
      const upper = (s: string) => s.replace(/[a-z]/g, (c) => c.toUpperCase());
      for (const a of words)
        for (const b of words) {
          expect(m.sameLower(a, b), `${a}.toLowerCase() == ${b}`).toBe(lower(a) === b);
          expect(m.sameUpper(a, b), `${b} == ${a}.toUpperCase()`).toBe(b === upper(a));
          expect(m.notLower(a, b), `${a}.toLowerCase() != ${b}`).toBe(lower(a) !== b);
        }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`split into a constant only read gives views, and copies where it must, on ${allocator}`, async () => {
      const m = await load(allocator);
      expect(m.csvSum("1,ab,xyz\n2,c,de\nbad\n")).toBe(10);
      expect(m.firstKept("ada,b")).toBe("ada");
      expect(m.afterChange("x,y")).toBe("x|x,y!");
      expect(m.partsOf("a,b")).toEqual(["a", "b"]);
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

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
