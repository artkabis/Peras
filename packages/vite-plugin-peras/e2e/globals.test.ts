import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Module variables (decision 0023): each kept from one call to the next, as
// the same variables written in JavaScript are.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/globals/globals.peras", import.meta.url));

interface Globals {
  bump(): number;
  add(x: number): number;
  toggle(): boolean;
  swap(): number;
  twice(n: number): number;
  __perasHeap?: { live(): number };
}

let loads = 0;

/** A new instance each time: the same URL would give the cached module. */
async function load(allocator: "debug" | "tlsf"): Promise<Globals> {
  const compiled = await compile(source, { perasc, allocator });
  loads += 1;
  const code = `${moduleCode(compiled)}\n// load ${String(loads)}\n`;
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Globals;
}

/** The same module, written in JavaScript: `| 0` is the wrapping of `i32`. */
function reference(): Globals {
  let count = 20;
  let total = 0.5;
  let on = false;
  let origin = { x: 1, y: 2.5 };
  const bumped = (): number => (count = (count + 3) | 0);
  return {
    bump: () => (count = (count + 2) | 0),
    add: (x) => (total = total + x),
    toggle: () => (on = !on),
    swap: () => {
      origin = { x: Math.trunc(origin.y), y: origin.x };
      origin.x = (origin.x + 10) | 0;
      return origin.x + origin.y;
    },
    twice: (n) => {
      let seen = 0;
      for (let i = 0; i < n; i++) {
        const before = count;
        seen = (seen + ((bumped() - before) | 0)) | 0;
      }
      return (seen + count) | 0;
    },
  };
}

function calls(m: Globals): unknown[] {
  return [
    m.bump(),
    m.bump(),
    m.add(1.25),
    m.add(-0.75),
    m.toggle(),
    m.toggle(),
    m.toggle(),
    m.swap(),
    m.swap(),
    m.twice(4),
    m.bump(),
    m.twice(0),
  ];
}

describe("module variables", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`keep their values between calls, as JavaScript's do, on ${allocator}`, async () => {
      const m = await load(allocator);
      expect(calls(m)).toEqual(calls(reference()));
      // Two loads, two instances: each starts from the initial values.
      const again = await load(allocator);
      expect(again.bump()).toBe(22);
    });
  }
});
