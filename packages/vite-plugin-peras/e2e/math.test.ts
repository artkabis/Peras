import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, compileManaged, moduleCode } from "../src/index.ts";

// Decision 0035: `Math` as JavaScript writes it gives JavaScript's values,
// in an owned module on both allocators and in a managed one.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/math/math.peras", import.meta.url));

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

// What JavaScript gives for each call, `Math` as written.
const expected: [string, unknown[], unknown][] = [
  [
    "polar",
    [3, -4],
    `${Math.sqrt(25).toFixed(2)} ${((Math.atan2(-4, 3) * 180) / Math.PI).toFixed(1)}`,
  ],
  ["clamp", [300], 255],
  ["clamp", [-5], 0],
  ["cents", [0.125], Math.round(12.5) / 100],
  ["cents", [19.999], Math.round(1999.9) / 100],
  ["percent", [2, 3], `${String(Math.round(200 / 3))} %`],
  ["growth", [0.25], Math.pow(Math.E, 0.25) + 0.25 + Math.floor(2.5) + Math.ceil(0.25)],
];

function check(m: Module): void {
  for (const [name, args, want] of expected) {
    expect([name, m[name]?.(...args)]).toEqual([name, want]);
  }
}

describe("Math as JavaScript writes it", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`gives JavaScript's values in an owned module, on ${allocator}`, async () => {
      const m = await importCode(moduleCode(await compile(source, { perasc, allocator })));
      check(m);
      expect(m.__perasHeap?.live()).toBe(0);
    });
  }

  it("gives JavaScript's values in a managed module", async () => {
    const dir = await mkdtemp(join(tmpdir(), "peras-math-"));
    try {
      const copy = join(dir, "math.apeiron");
      await writeFile(copy, await readFile(source, "utf8"));
      check(await importCode((await compileManaged(copy, { perasc })).js));
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
