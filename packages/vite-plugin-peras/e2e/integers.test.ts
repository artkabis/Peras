import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Every integer type compiled in an owned module (decision 0024): as a
// parameter, a result, a field, an array element, a map key and a module
// variable, against the same calls written in JavaScript. A value JavaScript
// gives out of its type's range is wrapped, as a typed array of that type
// stores it; a `u32` and a `u64` come back non-negative.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/integers/integers.peras", import.meta.url));

interface Pixel {
  r: number;
  g: number;
  b: number;
  depth: number;
  id: number;
  stamp: bigint;
}

interface PerasArray {
  readonly kind: string;
  view(): ArrayLike<number | bigint>;
  release(): void;
}

interface Integers {
  mix(a: number, b: number, c: number, d: number, e: number, f: bigint): bigint;
  echo8(x: number): number;
  echoU32(x: number): number;
  echoU64(x: bigint): bigint;
  brighter(p: Pixel): Pixel;
  grey(ps: Pixel[]): number[];
  invert(xs: number[] | Uint8Array | PerasArray): number[];
  halve(xs: number[]): void;
  hashes(n: number): number[];
  largest(xs: bigint[]): bigint;
  ordered(xs: number[]): number[];
  histogram(xs: number[]): Map<number, number>;
  owners(ids: Set<number>): number;
  tick(): number;
  backwards(xs: number[]): number[];
  sortedBytes(xs: number[]): number[];
  byStamp(ps: Pixel[]): number[];
  __perasHeap: { live(): number; array(kind: string, init: unknown): PerasArray };
}

let loads = 0;

/** A new instance each time: the same URL would give the cached module. */
async function load(allocator: "debug" | "tlsf"): Promise<Integers> {
  const compiled = await compile(source, { perasc, allocator });
  loads += 1;
  const code = `${moduleCode(compiled)}\n// load ${String(loads)}\n`;
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Integers;
}

// Each type's wrapping, as its typed array stores a number.
const i8 = (x: number): number => (x << 24) >> 24;
const i16 = (x: number): number => (x << 16) >> 16;
const u8 = (x: number): number => x & 255;
const u16 = (x: number): number => x & 65535;
const u32 = (x: number): number => x >>> 0;
const u64 = (x: bigint): bigint => BigInt.asUintN(64, x);

/** The module, written in JavaScript. */
function reference(): Omit<Integers, "__perasHeap"> {
  let ticks = 65530;
  return {
    mix: (a, b, c, d, e, f) =>
      u64(
        BigInt(i8(a)) + BigInt(i16(b)) + BigInt(u8(c)) + BigInt(u16(d)) + BigInt(u32(e)) + u64(f),
      ),
    echo8: i8,
    echoU32: u32,
    echoU64: u64,
    brighter: (p) => ({
      r: u8(p.r + 10),
      g: u8(p.g),
      b: Math.trunc(u8(p.b) / 2),
      depth: i16(p.depth - 1),
      id: u32(p.id) + 1,
      stamp: u64(p.stamp),
    }),
    grey: (ps) => ps.map((p) => Math.trunc((u8(p.r) + u8(p.g) + u8(p.b)) / 3)),
    invert: (xs) => Array.from(xs as ArrayLike<number>, (x) => 255 - u8(x)),
    halve: (xs) => {
      for (let i = 0; i < xs.length; i++) xs[i] = Math.trunc(i16(xs[i] ?? 0) / 2);
    },
    hashes: (n) => {
      const out: number[] = [];
      let h = 2166136261;
      for (let i = 0; i < n; i++) {
        h = Math.imul(h ^ i, 16777619) >>> 0;
        out.push(h);
      }
      return out;
    },
    largest: (xs) => xs.map(u64).reduce((a, b) => (a > b ? a : b), 0n),
    ordered: (xs) => Array.from(new Uint32Array(xs).sort()),
    histogram: (xs) => {
      const m = new Map<number, number>();
      for (const x of xs.map(u8)) m.set(x, (m.get(x) ?? 0) + 1);
      return m;
    },
    // Wrapped, two numbers can become one key.
    owners: (ids) => [...new Set([...ids].map(u32))].reduce((a, b) => (a ^ b) >>> 0, 0),
    tick: () => (ticks = u16(ticks + 4)),
    sortedBytes: (xs) => Array.from(new Int8Array(xs).sort()),
    // Stable: equal stamps keep their order.
    byStamp: (ps) =>
      ps
        .map((p, i) => ({ id: u32(p.id), stamp: u64(p.stamp), i }))
        .sort((a, b) => (a.stamp < b.stamp ? 1 : a.stamp > b.stamp ? -1 : a.i - b.i))
        .map((p) => p.id),
    backwards: (xs) => {
      const w = xs.map(i16);
      return [...w].reverse().concat(w.slice(1, 3));
    },
  };
}

const pixel: Pixel = {
  r: 250,
  g: 300,
  b: -1,
  depth: -32768,
  id: 4294967290,
  stamp: 2n ** 64n - 1n,
};

function calls(m: Omit<Integers, "__perasHeap">): unknown[] {
  const halved = [-7, 40001, 3, -32768];
  m.halve(halved);
  return [
    m.mix(-1, 40000, 300, -1, 2 ** 32 + 7, 2n ** 64n + 5n),
    m.mix(127, -32768, 255, 65535, 4294967295, 2n ** 63n),
    [m.echo8(128), m.echo8(-129), m.echo8(300)],
    [m.echoU32(-1), m.echoU32(2 ** 31), m.echoU32(2 ** 32 + 9)],
    [m.echoU64(-1n), m.echoU64(2n ** 63n), m.echoU64(2n ** 64n + 1n)],
    m.brighter(pixel),
    m.grey([pixel, { ...pixel, r: 1, g: 2, b: 3 }]),
    m.invert([0, 1, 255, 256, -1]),
    m.invert(new Uint8Array([9, 200])),
    halved,
    m.hashes(6),
    m.largest([1n, 2n ** 63n + 1n, -2n, 7n]),
    m.ordered([3e9, 1, 2 ** 31, 7, -1]),
    [...m.histogram([1, 257, 2, 1, -255, 0]).entries()],
    m.owners(new Set([3e9, 1, 2 ** 32 + 1])),
    [m.tick(), m.tick(), m.tick()],
    // Elements of two bytes reversed, sliced and joined.
    m.backwards([1, -2, 300, -32768, 7, 40000]),
    // A few by insertion, many by the runtime's radix sort.
    m.sortedBytes([5, -1, 200, -128, 127]),
    m.sortedBytes(Array.from({ length: 300 }, (_, i) => (i * 97) % 512)),
    m.byStamp(
      Array.from({ length: 90 }, (_, i) => ({
        ...pixel,
        id: i,
        stamp: BigInt((i * 37) % 11) * 2n ** 61n - 1n,
      })),
    ),
  ];
}

describe("integer types other than i32 and i64", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`cross the boundary as JavaScript's own would, on ${allocator}`, async () => {
      const m = await load(allocator);
      expect(calls(m)).toEqual(calls(reference()));
      expect(m.__perasHeap.live()).toBe(0);
    });

    it(`are stored at their width in a PerasArray, on ${allocator}`, async () => {
      const m = await load(allocator);
      const kinds = [
        ["u8", Uint8Array, [1, 255, 256]],
        ["i8", Int8Array, [1, 128, -129]],
        ["i16", Int16Array, [-1, 32768]],
        ["u16", Uint16Array, [-1, 65536]],
        ["u32", Uint32Array, [-1, 2 ** 31]],
      ] as const;
      for (const [kind, Typed, values] of kinds) {
        const array = m.__perasHeap.array(kind, values);
        const view = array.view();
        expect(view).toBeInstanceOf(Typed);
        expect(Array.from(view)).toEqual(Array.from(new Typed(values)));
        array.release();
      }
      const big = m.__perasHeap.array("u64", [-1n, 2n ** 63n]);
      expect(Array.from(big.view())).toEqual([2n ** 64n - 1n, 2n ** 63n]);
      big.release();
      // An array of bytes passed with no copy, read at its width.
      const bytes = m.__perasHeap.array("u8", [0, 10, 255]);
      expect(m.invert(bytes)).toEqual([255, 245, 0]);
      bytes.release();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }
});
