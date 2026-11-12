import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Browser } from "playwright";
import { createServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import peras, { compile, moduleCode } from "../src/index.ts";

// Owned modules that import each other (decision 0022): a program split into
// three files computes, in Chromium, what the same program in one file
// computes; and the dev server rebuilds a module when a file it imports
// changes.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const fixtures = fileURLToPath(new URL("fixtures/modules", import.meta.url));

/** What the page computes with both modules. */
interface Results {
  split: number[];
  one: number[];
  live: number[];
}

/** A module's export, as the dev server loads it. */
interface Run {
  run(n: number): number;
}

describe("owned modules that import each other", () => {
  let browser: Browser;
  beforeAll(async () => {
    const executablePath = process.env.PERAS_CHROMIUM;
    browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
  });
  afterAll(async () => {
    await browser.close();
  });

  for (const allocator of ["debug", "tlsf"] as const) {
    it(`three files compute what one file computes, in Chromium, on ${allocator}`, async () => {
      const split = await compile(join(fixtures, "main.peras"), { perasc, allocator });
      const one = await compile(join(fixtures, "one.peras"), { perasc, allocator });
      expect(split.inputs.map((p) => p.slice(fixtures.length + 1))).toEqual([
        "lib/size.peras",
        "lib/area.peras",
        "main.peras",
      ]);
      const page = await browser.newPage();
      // A script of its own: the test runner would rewrite an `import()`
      // written in a function here.
      const script = `(async () => {
        const url = (code) => "data:text/javascript;base64," + btoa(code);
        const split = await import(url(${JSON.stringify(moduleCode(split))}));
        const one = await import(url(${JSON.stringify(moduleCode(one))}));
        const ns = [-5, 0, 1, 3, 4, 7, 20, 1000];
        return {
          split: ns.map((n) => split.run(n)),
          one: ns.map((n) => one.run(n)),
          live: [split.__perasHeap.live(), one.__perasHeap.live()],
        };
      })()`;
      const results = await page.evaluate<Results>(script);
      await page.close();
      expect(results.split).toEqual(results.one);
      expect(results.live).toEqual([0, 0]);
      // The same code: within 1 % of the same size.
      expect(Math.abs(split.wasm.length / one.wasm.length - 1)).toBeLessThan(0.01);
    });
  }

  it("rebuilds a module in the dev server when a file it imports changes", async () => {
    const dir = await mkdtemp(join(tmpdir(), "peras-modules-"));
    try {
      await cp(fixtures, dir, { recursive: true });
      const server = await createServer({
        root: dir,
        logLevel: "silent",
        server: { port: 0 },
        plugins: [peras({ perasc, dts: false })],
      });
      await server.listen();
      try {
        const before = (await server.ssrLoadModule("/main.peras")) as Run;
        expect(before.run(7)).toBe(22);
        // `helper` of `size.peras` doubles the weight; make it triple it.
        const size = join(dir, "lib", "size.peras");
        const text = await readFile(size, "utf8");
        await writeFile(size, text.replace("return x * 2;", "return x * 3;"));
        const main = join(dir, "main.peras");
        // The watcher invalidates `main.peras`, which imports the file.
        for (let i = 0; i < 200; i++) {
          const node = server.moduleGraph.getModuleById(main);
          if (node?.ssrTransformResult === null) break;
          await new Promise((r) => setTimeout(r, 25));
        }
        const after = (await server.ssrLoadModule("/main.peras")) as Run;
        expect(after.run(7)).toBe(29);
      } finally {
        await server.close();
      }
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
