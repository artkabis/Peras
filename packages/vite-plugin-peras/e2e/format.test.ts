import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Numbers and booleans become text as JavaScript writes them, and text
// becomes an integer only when it is one, in range: run by the runtime of
// both allocators, across the boundary.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/format/format.peras", import.meta.url));

interface Format {
  wide(n: bigint): string;
  narrow(n: number): string;
  real(x: number): string;
  single(x: number): string;
  flag(b: boolean): string;
  readI32(s: string): number | undefined;
  readF64(s: string): number | undefined;
  readI64(s: string): bigint | undefined;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<Format> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Format;
}

/** What a strict reading of an integer of `bits` bits gives, in JavaScript. */
function strict(s: string, bits: number): bigint | undefined {
  if (!/^[+-]?[0-9]+$/.test(s)) return undefined;
  const n = BigInt(s);
  const limit = 1n << BigInt(bits - 1);
  return n >= -limit && n < limit ? n : undefined;
}

const INPUTS = [
  "0",
  "-0",
  "+5",
  "007",
  "",
  "-",
  "+",
  "12a",
  " 1",
  "1 ",
  "1_000",
  "1e3",
  "0x10",
  "١٢",
  "2147483647",
  "2147483648",
  "-2147483648",
  "-2147483649",
  "9223372036854775807",
  "9223372036854775808",
  "-9223372036854775808",
  "-9223372036854775809",
  "99999999999999999999999",
  // Past the limit by a digit before the last, and the limit after zeros.
  "2147483650",
  "-2147483650",
  "0000000000002147483647",
];

describe("numbers as text", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`write integers and booleans as String() does, on ${allocator}`, async () => {
      const m = await load(allocator);
      for (const n of [0n, 7n, -42n, 9223372036854775807n, -9223372036854775808n]) {
        expect(m.wide(n)).toBe(String(n));
      }
      for (const n of [0, -1, 2147483647, -2147483648]) {
        expect(m.narrow(n)).toBe(String(n));
      }
      expect(m.flag(true)).toBe("true");
      expect(m.flag(false)).toBe("false");
    });

    it(`write floats as String() does, and f32 with its own digits, on ${allocator}`, async () => {
      const m = await load(allocator);
      const doubles = [0, -0, 0.1, 0.1 + 0.2, 1 / 3, -2.5, 1e21, 1e20, 1e-6, 1e-7, 5e-324];
      for (const x of [...doubles, Number.MAX_VALUE, NaN, Infinity, -Infinity, 2 ** 53 + 2])
        expect(m.real(x), String(x)).toBe(String(x));
      // Random bit patterns: every kind of float, subnormals included.
      const bits = new DataView(new ArrayBuffer(8));
      let seed = 0x2545f4914f6cdd1dn;
      for (let i = 0; i < 2000; i++) {
        seed ^= (seed << 13n) & 0xffffffffffffffffn;
        seed ^= seed >> 7n;
        seed ^= (seed << 17n) & 0xffffffffffffffffn;
        bits.setBigUint64(0, seed);
        const x = bits.getFloat64(0);
        expect(m.real(x)).toBe(String(x));
      }
      // An f32 is written with the fewest digits that read back as it, not
      // with those of the f64 it widens to.
      const singles: [number, string][] = [
        [0.1, "0.1"],
        [1 / 3, "0.33333334"],
        [16777218, "16777218"],
        [3.4028234663852886e38, "3.4028235e+38"],
        [1.401298464324817e-45, "1e-45"],
        [1e21, "1e+21"],
        [1595310.25, "1595310.3"],
        [-7.5, "-7.5"],
      ];
      for (const [x, text] of singles) expect(m.single(Math.fround(x)), text).toBe(text);
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`read a decimal number only when it is one, rounded as JavaScript's Number, on ${allocator}`, async () => {
      const m = await load(allocator);
      // The grammar of parseF64, stricter than Number; where both accept a
      // text, they must give the same number, bit for bit, -0 included.
      const strict = (s: string): number | undefined =>
        /^[+-]?([0-9]+[.]?[0-9]*|[.][0-9]+)([eE][+-]?[0-9]+)?$/.test(s) ? Number(s) : undefined;
      const same = (s: string) => {
        const got = m.readF64(s);
        const want = strict(s);
        if (!(got === undefined ? want === undefined : want !== undefined && Object.is(got, want)))
          throw new Error(
            `parseF64(${JSON.stringify(s)}) gave ${String(got)}, Number gives ${String(want)}`,
          );
      };
      for (const s of [
        "0",
        "-0",
        "+0",
        "0.0",
        "-0.0",
        ".5",
        "5.",
        "+.5",
        "-.5",
        ".",
        "",
        "-",
        "+",
        "e5",
        "1e",
        "1e+",
        "1e-5",
        "1E5",
        "1.5e3",
        "12.50",
        "007.5",
        "1..2",
        "1.2.3",
        " 1",
        "1 ",
        "1_0",
        "0x1A",
        "Infinity",
        "-Infinity",
        "NaN",
        "1e309",
        "-1e309",
        "1e-400",
        "4.9e-324",
        "2.4703282292062327e-324",
        "2.2250738585072011e-308",
        "2.2250738585072012e-308",
        "9007199254740991",
        "9007199254740992",
        "9007199254740993",
        "123456789012345678901234567890",
        "0.1",
        "0.2",
        "0.3",
        "1.7976931348623157e308",
        "1.7976931348623158e308",
        "1.7976931348623159e308",
        "3.141592653589793238462643383279",
        "1e22",
        "1e23",
        "123456789e-22",
        "123456789e-23",
        "1e99999999999",
        "1e-99999999999",
        "٣.٥",
        "１",
        "1e0000000000000000005",
        "0.000000000000000000000000000000000000000000001",
        "100000000000000000000000000000000000000000000.0",
      ])
        same(s);
      // Random numbers, written every way JavaScript writes them, and random
      // decimal texts of up to 25 digits and exponents from -350 to 350.
      let seed = 0x9e3779b9;
      const next = () => {
        seed ^= seed << 13;
        seed ^= seed >>> 17;
        seed ^= seed << 5;
        return seed >>> 0;
      };
      const bits = new DataView(new ArrayBuffer(8));
      for (let i = 0; i < 12000; i++) {
        bits.setUint32(0, next());
        bits.setUint32(4, next());
        const x = bits.getFloat64(0);
        if (!Number.isFinite(x)) continue;
        same(String(x));
        same(x.toExponential());
        for (let p = 1; p <= 21; p += 4) same(x.toPrecision(p));
        if (Math.abs(x) < 1e15) same(x.toFixed(2));
      }
      const digits = (n: number) => Array.from({ length: n }, () => String(next() % 10)).join("");
      for (let i = 0; i < 40000; i++) {
        const whole = digits(next() % 13);
        const frac = digits(next() % 13);
        const e = (next() % 701) - 350;
        const text = `${whole}${next() % 2 ? "." : ""}${frac}${next() % 3 ? "" : `e${String(e)}`}`;
        same(text);
        same(`-${text}`);
      }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`read an integer only when it is one, in range, on ${allocator}`, async () => {
      const m = await load(allocator);
      for (const s of INPUTS) {
        const i32 = strict(s, 32);
        expect(m.readI32(s), s).toBe(i32 === undefined ? undefined : Number(i32));
        expect(m.readI64(s), s).toBe(strict(s, 64));
      }
    });
  }
});
