import { copyFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { compile, compileManaged, moduleCode } from "../src/index.ts";

// Decision 0026: closures in owned modules. The same source, built as an owned
// module on each allocator, gives what it gives built as a managed module,
// which is JavaScript, and leaves nothing alive.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/closures/closures.peras", import.meta.url));

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

const calls: [string, ...unknown[]][] = [
  ["adders", 3, 4],
  ["adders", -7, 0],
  ["looped", 0],
  ["looped", 5],
  ["copies", 7],
  ["held", 2, true],
  ["held", 2, false],
  ["replaced", 6],
  ["keptI8", -128],
  ["keptI8", 127],
  ["keptU32", 0],
  ["keptU32", 4294967295],
  ["keptI64", -9223372036854775808n],
  ["keptI64", 9223372036854775807n],
  ["keptU64", 0n],
  ["keptU64", 18446744073709551615n],
  // Values an `f32` holds exactly, so that JavaScript's numbers agree.
  ["keptF32", -3.4028234663852886e38],
  ["keptF32", 1.401298464324817e-45],
  ["keptF32", -0],
  ["keptF64", Number.MAX_VALUE],
  ["keptF64", -Number.MIN_VALUE],
  ["keptF64", -0],
  ["keptF64", Number.NaN],
  ["keptF64", Number.NEGATIVE_INFINITY],
  ["keptBool", true, false],
  ["keptBool", true, true],
  ["mixed", 1.5, -2.25, 3, true],
];

function results(m: Module): unknown[] {
  return calls.map(([name, ...args]) => m[name]?.(...structuredClone(args)));
}

describe("closures", () => {
  let dir = "";
  let managed: Module;

  beforeAll(async () => {
    dir = await mkdtemp(join(tmpdir(), "peras-closures-"));
    const copy = join(dir, "closures.apeiron");
    await copyFile(source, copy);
    managed = await importCode((await compileManaged(copy, { perasc })).js);
  });

  afterAll(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  for (const allocator of ["debug", "tlsf"] as const) {
    it(`compute in an owned module what the managed one does, on ${allocator}`, async () => {
      const owned = await importCode(moduleCode(await compile(source, { perasc, allocator })));
      const expected = results(managed);
      // Every call gives a value: a function missing from both would not.
      expect(expected.every((v) => v !== undefined)).toBe(true);
      expect(results(owned)).toEqual(expected);
      expect(owned.__perasHeap?.live()).toBe(0);
    });
  }
});
