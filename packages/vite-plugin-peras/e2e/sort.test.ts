import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// `sort()` gives the order of JavaScript's typed arrays, -0 before +0 and
// every NaN last, on every length around the switch between its two
// algorithms, on both allocators.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/sort/sort.peras", import.meta.url));

type Kind = "f64" | "f32" | "i32" | "i64";
type Typed = Float64Array | Float32Array | Int32Array | BigInt64Array;

interface View {
  view(): Typed;
  release(): void;
}

interface Sorts {
  sortF64(xs: View): void;
  sortF32(xs: View): void;
  sortI32(xs: View): void;
  sortI64(xs: View): void;
  __perasHeap: {
    live(): number;
    array(kind: Kind, init: number): View;
  };
}

async function load(allocator: "debug" | "tlsf"): Promise<Sorts> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Sorts;
}

/** A deterministic stream of numbers in [0, 1). */
function random(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/** `n` numbers of `kind`, with the hard cases mixed in. */
function data(kind: Kind, n: number, seed: number): Typed {
  const next = random(seed);
  const pick = (k: number) => Math.floor(next() * k);
  if (kind === "i64") {
    const special = [0n, -1n, 1n, -(2n ** 63n), 2n ** 63n - 1n];
    return BigInt64Array.from({ length: n }, () =>
      next() < 0.2
        ? (special[pick(special.length)] ?? 0n)
        : BigInt.asIntN(64, BigInt(pick(2 ** 31)) * BigInt(pick(2 ** 31)) - 2n ** 61n),
    );
  }
  if (kind === "i32") {
    const special = [0, -1, 1, -(2 ** 31), 2 ** 31 - 1];
    return Int32Array.from({ length: n }, () =>
      next() < 0.2 ? (special[pick(special.length)] ?? 0) : pick(2 ** 32) | 0,
    );
  }
  const out = kind === "f64" ? new Float64Array(n) : new Float32Array(n);
  const view = new DataView(out.buffer);
  const special = [0, -0, Infinity, -Infinity, 1e-310, -1e-310, 3.5, 3.5];
  for (let i = 0; i < n; i++) {
    const r = next();
    if (r < 0.05) {
      // A NaN with its sign bit set, and one without.
      if (kind === "f64") view.setFloat64(i * 8, NaN, true);
      else view.setFloat32(i * 4, NaN, true);
      if (next() < 0.5) {
        const at = kind === "f64" ? i * 8 + 7 : i * 4 + 3;
        view.setUint8(at, view.getUint8(at) | 0x80);
      }
    } else if (r < 0.25) {
      out[i] = special[pick(special.length)] ?? 0;
    } else {
      out[i] = (next() - 0.5) * 10 ** pick(12);
    }
  }
  return out;
}

const SORTS = { f64: "sortF64", f32: "sortF32", i32: "sortI32", i64: "sortI64" } as const;
const LENGTHS = [0, 1, 2, 3, 17, 63, 64, 65, 200, 4096, 100_000];

describe("sort", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`gives the order of typed arrays, -0 first and NaN last, on ${allocator}`, async () => {
      const m = await load(allocator);
      for (const kind of ["f64", "f32", "i32", "i64"] as const) {
        for (const [k, n] of LENGTHS.entries()) {
          const input = data(kind, n, k + 1);
          const expected = input.slice().sort();
          const xs = m.__perasHeap.array(kind, n);
          // Byte for byte: the NaN keep their sign and payload.
          const view = xs.view();
          new Uint8Array(view.buffer, view.byteOffset, view.byteLength).set(
            new Uint8Array(input.buffer, input.byteOffset, input.byteLength),
          );
          m[SORTS[kind]](xs);
          const got = Array.from(xs.view() as ArrayLike<number | bigint>);
          const want = Array.from(expected as ArrayLike<number | bigint>);
          const same = got.every((v, i) => Object.is(v, want[i]));
          expect(same, `${kind}, ${String(n)} numbers`).toBe(true);
          xs.release();
        }
      }
      expect(m.__perasHeap.live()).toBe(0);
    });
  }
});
