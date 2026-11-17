import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { compile, compileManaged, moduleCode } from "../src/index.ts";

// Decision 0028: a move at the call, narrowing, and a fallback evaluated only
// when needed. The same source, built as an owned module on each allocator,
// gives what it gives built as a managed module, which is JavaScript, and
// leaves nothing alive.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/options/options.peras", import.meta.url));

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
  ["p1", "a b a c b a"],
  ["p2", "a,1\nbad\nb,22"],
  ["p3", "tea"],
  ["p3", "coffee"],
  ["p4", 1],
  ["p4", 7],
  ["p5", 24],
  ["p5", 6],
  ["p5", 3],
  ["p6", "a,1\nb,2,3\na,4"],
  ["narrowedString", 4],
  ["narrowedString", 6],
  ["narrowedLoop", 8],
  ["writtenBefore", 8],
  ["movedAtCall", "ab c ab  d"],
  ["fallbacks", 1],
  ["fallbacks", 2],
];

function results(m: Module): unknown[] {
  return calls.map(([name, ...args]) => m[name]?.(...structuredClone(args)));
}

describe("options and moves", () => {
  let dir = "";
  let managedCode = "";

  beforeAll(async () => {
    dir = await mkdtemp(join(tmpdir(), "peras-options-"));
    const copy = join(dir, "options.apeiron");
    // A managed module moves nothing, so it takes no `sink` (E0243): the
    // same source without the convention.
    await writeFile(copy, (await readFile(source, "utf8")).replaceAll(": sink ", ": "));
    managedCode = (await compileManaged(copy, { perasc })).js;
  });

  afterAll(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  for (const allocator of ["debug", "tlsf"] as const) {
    it(`compute in an owned module what the managed one does, on ${allocator}`, async () => {
      const owned = await importCode(moduleCode(await compile(source, { perasc, allocator })));
      // A fresh instance each run: `calls`, a module variable, starts at 0.
      const expected = results(await importCode(managedCode));
      // Every call gives a value: a function missing from both would not.
      expect(expected.every((v) => v !== undefined)).toBe(true);
      expect(results(owned)).toEqual(expected);
      expect(owned.__perasHeap?.live()).toBe(0);
    });
  }
});
