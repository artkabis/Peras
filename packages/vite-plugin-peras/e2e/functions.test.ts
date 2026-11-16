import { copyFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { compile, compileManaged, moduleCode } from "../src/index.ts";

// Decision 0025: fields moved out of structures, and functions as values. The
// same source, built as an owned module on each allocator, gives what it
// gives built as a managed module, which is JavaScript, and leaves nothing
// alive.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/functions/functions.peras", import.meta.url));

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
  ["taken", 3],
  ["branches", 4],
  ["branches", 5],
  ["gathered", 10],
  ["gathered", 3],
  ["early", 2],
  ["early", 9],
  ["applied", 6],
  ["table", 10],
  ["chosen", 2, 9],
  ["chosen", 1, 9],
  ["chosen", 0, 9],
  ["ops", 2],
  ["ops", 5],
  // ASCII only: `length` counts bytes in an owned module, UTF-16 units in a
  // managed one (decision 0008).
  ["strings", ["ab", "eta", ""]],
  ["methods", 9],
];

function results(m: Module): unknown[] {
  return calls.map(([name, ...args]) => m[name]?.(...structuredClone(args)));
}

describe("fields moved and functions as values", () => {
  let dir = "";
  let managed: Module;

  beforeAll(async () => {
    dir = await mkdtemp(join(tmpdir(), "peras-functions-"));
    const copy = join(dir, "functions.apeiron");
    await copyFile(source, copy);
    managed = await importCode((await compileManaged(copy, { perasc })).js);
  });

  afterAll(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  for (const allocator of ["debug", "tlsf"] as const) {
    it(`compute in an owned module what the managed one does, on ${allocator}`, async () => {
      const owned = await importCode(moduleCode(await compile(source, { perasc, allocator })));
      expect(results(owned)).toEqual(results(managed));
      expect(owned.__perasHeap?.live()).toBe(0);
    });
  }
});
