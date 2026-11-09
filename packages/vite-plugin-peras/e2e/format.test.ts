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
  flag(b: boolean): string;
  readI32(s: string): number | undefined;
  readI64(s: string): bigint | undefined;
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
