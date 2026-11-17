import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Decision 0032, option r1: shared values, read only, copied on write. A
// `const` read from a place is lent only while nothing may change or move
// that place; each case where lending would be wrong gives another number,
// or a use after release on the debug allocator. Nothing is left alive.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/shared/shared.peras", import.meta.url));

type Module = Record<string, (...args: unknown[]) => unknown> & {
  __perasHeap?: { live(): number };
};

let loads = 0;

/** Imports JavaScript from its text, a new instance each time. */
async function importCode(code: string): Promise<Module> {
  loads += 1;
  const text = `${code}\n// load ${String(loads)}\n`;
  return (await import(
    `data:text/javascript;base64,${Buffer.from(text).toString("base64")}`
  )) as Module;
}

const expected: [string, unknown[], number][] = [
  ["lent", [], 12],
  ["reassigned", [], 31],
  ["index", [], 31],
  ["replaced", [], 3101],
  ["moved", [], 31],
  ["holders", [], 3417],
  ["tree", [100], 5050],
  ["nested", [], 110521],
  ["inner", [], 19],
  ["elements", [], 4371],
  ["elementFields", [], 128],
  ["stepped", [], 112],
];

describe("shared values, read only and copied on write", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`give each holder its own value, and leave nothing alive, on ${allocator}`, async () => {
      const m = await importCode(moduleCode(await compile(source, { perasc, allocator })));
      for (const [name, args, want] of expected) {
        expect([name, m[name]?.(...args)]).toEqual([name, want]);
      }
      expect(m.__perasHeap?.live()).toBe(0);
    });
  }
});
