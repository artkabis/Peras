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
  charsOf(text: string): string;
  countWords(text: string): string;
  renumbered(n: number): number;
  changedKey(): number;
  putBetween(): number;
  tallies(text: string, drop: string): string;
  intTemplate(a: number, b: bigint): string;
  echo(s: string | Uint8Array): string;
  tagOr(s: string): string;
  tagOrGiven(s: string, fallback: string): string;
  intOr(s: string): number;
  literalLength(): number;
  whichLiteral(s: string): number;
  literalKey(text: string): number;
  upperCount(text: string, stop: string): number;
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

    it(`looks a key up once where the map cannot have changed, and again where it can, on ${allocator}`, async () => {
      const m = await load(allocator);
      expect(m.countWords("a b a c b a")).toBe("3,2,1,");
      expect(m.countWords("x")).toBe("1,");
      // A delete then sets that grow the map renumber its entries.
      for (const n of [0, 1, 5, 40]) expect(m.renumbered(n)).toBe(3);
      expect(m.changedKey()).toBe(57);
      expect(m.putBetween()).toBe(9);
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`takes bytes where it takes a String, exactly as a fatal TextDecoder does, on ${allocator}`, async () => {
      const m = await load(allocator);
      const decoder = new TextDecoder("utf-8", { fatal: true });
      const encoder = new TextEncoder();
      /** What the decoder makes of the bytes, or undefined when it refuses them. */
      const decoded = (bytes: Uint8Array): string | undefined => {
        try {
          return decoder.decode(bytes);
        } catch {
          return undefined;
        }
      };
      const check = (bytes: Uint8Array) => {
        const want = decoded(bytes);
        if (want === undefined) expect(() => m.echo(bytes)).toThrow(TypeError);
        else expect(m.echo(bytes)).toBe(want);
      };
      // The Unicode Standard, chapter 3, table 3-7, at and past each bound.
      const edges = [
        [],
        [0x7f],
        [0x80],
        [0xc0, 0x80],
        [0xc1, 0xbf],
        [0xc2, 0x80],
        [0xdf, 0xbf],
        [0xc2],
        [0xe0, 0x9f, 0x80],
        [0xe0, 0xa0, 0x80],
        [0xed, 0x9f, 0xbf],
        [0xed, 0xa0, 0x80],
        [0xed, 0xbf, 0xbf],
        [0xee, 0x80, 0x80],
        [0xef, 0xbf, 0xbf],
        [0xe1, 0x80],
        [0xf0, 0x8f, 0xbf, 0xbf],
        [0xf0, 0x90, 0x80, 0x80],
        [0xf4, 0x8f, 0xbf, 0xbf],
        [0xf4, 0x90, 0x80, 0x80],
        [0xf5, 0x80, 0x80, 0x80],
        [0xff],
        [0xf0, 0x90, 0x80],
      ];
      for (const edge of edges) {
        check(new Uint8Array(edge));
        // The same after sixteen bytes of ASCII and more, as a block read at once.
        check(
          new Uint8Array([
            ...encoder.encode("abcdefghijklmnopqrstuvwxyz0123456789"),
            ...edge,
            0x41,
          ]),
        );
      }
      let seed = 0x9e3779b97f4a7c15n;
      const next = (below: number) => {
        seed ^= (seed << 13n) & 0xffffffffffffffffn;
        seed ^= seed >> 7n;
        seed ^= (seed << 17n) & 0xffffffffffffffffn;
        return Number(seed % BigInt(below));
      };
      for (let i = 0; i < 10000; i++) {
        let bytes: Uint8Array;
        if (i % 3 === 0) {
          // Random bytes: most are refused.
          bytes = Uint8Array.from({ length: next(40) }, () => next(256));
        } else {
          // Valid text, of every plane, then one byte changed one time in two.
          const points = Array.from(
            { length: next(30) },
            () =>
              [
                next(0x80),
                next(0x800),
                next(0xd800),
                0xe000 + next(0x2000),
                0x10000 + next(0x100000),
              ][next(5)] ?? 0,
          );
          bytes = encoder.encode(String.fromCodePoint(...points));
          if (i % 2 === 0 && bytes.length > 0) bytes[next(bytes.length)] = next(256);
        }
        check(bytes);
      }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`writes a template's integers as JavaScript does, on ${allocator}`, async () => {
      const m = await load(allocator);
      const cases: [number, bigint][] = [
        [0, 0n],
        [-1, -1n],
        [9, 10n],
        [-10, 99n],
        [2147483647, 9223372036854775807n],
        [-2147483648, -9223372036854775808n],
      ];
      for (const [a, b] of cases) {
        expect(m.intTemplate(a, b)).toBe(`${String(a)}|${String(b)}|${String(a)}${String(b)}:0`);
      }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`changes a map's entry in place where it was just found, on ${allocator}`, async () => {
      const m = await load(allocator);
      expect(m.tallies("a bb a c bb a", "")).toBe("a=a:3,bb=bb:2,c=c:1,1#4,2#2,");
      // A word deleted between the lookup and the set goes to the end.
      expect(m.tallies("a bb a c bb a", "a")).toBe("bb=bb:2,c=c:1,a=a:3,1#4,2#2,");
      expect(m.tallies("", "")).toBe("=:1,0#1,");
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`makes a quiet fallback of unwrapOr only when the option is empty, on ${allocator}`, async () => {
      const m = await load(allocator);
      expect(m.tagOr("abcd")).toBe("abcd:4");
      expect(m.tagOr("ab")).toBe("none:-1");
      expect(m.tagOrGiven("abcde", "x")).toBe("abcde:5");
      expect(m.tagOrGiven("a", "given")).toBe("given:0");
      expect(m.intOr("42")).toBe(42);
      expect(m.intOr("4x")).toBe(-7);
      for (let i = 0; i < 500; i++) m.tagOr(i % 2 === 0 ? "abc" : "z");
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`lends literals only read from a static header, and copies them past its room, on ${allocator}`, async () => {
      const m = await load(allocator);
      expect(m.literalLength()).toBe(6);
      for (let i = 0; i < 70; i++)
        expect(m.whichLiteral(`literal number ${String(i).padStart(2, "0")}!`)).toBe(i);
      expect(m.whichLiteral("literal number 70!")).toBe(-1);
      expect(m.literalKey("a b c b zz b")).toBe(31);
      for (let k = 0; k < 1000; k++) m.literalLength();
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`walks the characters of split("") one at a time, as split gives them, on ${allocator}`, async () => {
      const m = await load(allocator);
      // Peras's split("") cuts at characters, code points, not UTF-16 units.
      const chars = (t: string) => Array.from(t);
      for (const t of ["", "a", "abc", "héllo", "日本語", "a😀b", "𝄞x", " ,\n", "ÉtÉ"]) {
        expect(m.charsOf(t), JSON.stringify(t)).toBe(
          chars(t)
            .map((c) => `${c}|`)
            .join(""),
        );
        for (const stop of ["", "x", "b", "語"]) {
          let n = 0;
          for (const c of chars(t)) {
            if (c === stop) break;
            if (c.toLowerCase() !== c && /^[A-Z]$/.test(c)) n++;
          }
          expect(m.upperCount(t, stop), `${t} / ${stop}`).toBe(n);
        }
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
