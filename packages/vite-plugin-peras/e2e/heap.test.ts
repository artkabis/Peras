import { execFile } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

// Modules that use the heap, linked to the debug allocator by the loader perasc
// writes. No Peras construct allocates yet: the fixtures are IR written by hand
// in crates/perasc-driver/tests/fixtures/heap_ir.rs.
const root = fileURLToPath(new URL("../../..", import.meta.url));

interface Heap {
  live(): number;
  checkLeaks(): void;
}

interface Fixture {
  __perasHeap: Heap;
  work?: (n: number) => number;
  twice?: (n: number) => void;
  keep?: (n: number) => void;
}

/** Writes the fixtures in `language`, then imports one of them. */
async function fixtures(language: "en" | "fr"): Promise<(name: string) => Promise<Fixture>> {
  const dir = await mkdtemp(join(tmpdir(), `peras-heap-${language}-`));
  dirs.push(dir);
  await promisify(execFile)(
    "cargo",
    [
      "run",
      "--release",
      "--quiet",
      "-p",
      "perasc-driver",
      "--example",
      "heap_fixture",
      "--",
      dir,
      language,
    ],
    { cwd: root },
  );
  return async (name) => (await import(pathToFileURL(join(dir, `${name}.js`)).href)) as Fixture;
}

const dirs: string[] = [];
let english: (name: string) => Promise<Fixture>;
let french: (name: string) => Promise<Fixture>;

beforeAll(async () => {
  [english, french] = await Promise.all([fixtures("en"), fixtures("fr")]);
}, 300_000);

afterAll(async () => {
  await Promise.all(dirs.map((dir) => rm(dir, { recursive: true, force: true })));
});

/** The error a call throws. */
function thrown(
  run: () => void,
): Error & { kind?: number; allocSite?: string; releaseSite?: string } {
  try {
    run();
  } catch (error) {
    return error as Error;
  }
  throw new Error("nothing was thrown");
}

describe("the debug allocator in JavaScript", () => {
  it("lets released blocks go, with nothing left at the end", async () => {
    const { work, __perasHeap } = await english("work");
    expect(work?.(1000)).toBe(1000);
    expect(__perasHeap.live()).toBe(0);
    expect(() => {
      __perasHeap.checkLeaks();
    }).not.toThrow();
  });

  it("names both sites of a double release", async () => {
    const { twice } = await english("twice");
    const error = thrown(() => twice?.(0));
    expect(error.name).toBe("PerasMemoryError");
    expect(error.message).toBe(
      "the block allocated at twice (alloc 1) is released a second time, at twice (release 2)",
    );
    expect({
      kind: error.kind,
      allocSite: error.allocSite,
      releaseSite: error.releaseSite,
    }).toEqual({
      kind: 5,
      allocSite: "twice (alloc 1)",
      releaseSite: "twice (release 2)",
    });
  });

  it("reports leaks in the language asked", async () => {
    const { keep, __perasHeap } = await french("keep");
    keep?.(0);
    keep?.(0);
    expect(__perasHeap.live()).toBe(2);
    const error = thrown(() => {
      __perasHeap.checkLeaks();
    });
    expect(error.message).toBe(
      "2 bloc(s) encore vivant(s) à la fin ; le premier a été alloué à keep (alloc 1)",
    );
  });
});
