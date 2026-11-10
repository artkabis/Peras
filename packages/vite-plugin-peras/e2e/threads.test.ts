import { mkdtemp, rm, writeFile, copyFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Browser } from "playwright";
import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import peras, { compile, moduleCode } from "../src/index.ts";

// `parallel for` on threads (decision 0007): in a cross-origin isolated page
// the turns run on several threads, elsewhere on one, with the same results,
// bit for bit, as the same kernels in JavaScript.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/threads/threads.peras", import.meta.url));

interface Threads {
  gray(pixels: number[], gain: number): void;
  move(xs: number[], vs: number[], steps: number): void;
  fill(xs: number[], n: number): void;
  __perasHeap: { live(): number; checkLeaks(): void; threads(): number };
}

/** What the page runs, on the module and on the same kernels in JavaScript. */
function check(m: Threads): {
  threads: number;
  same: boolean;
  error: string;
  live: number;
} {
  const n = 600_000;
  const pixels = Array.from({ length: n }, (_, i) => Math.imul(i, -1640531535));
  const want = pixels.map((p) => {
    const y = (((p >> 16) & 255) * 77 + ((p >> 8) & 255) * 150 + (p & 255) * 29) >> 8;
    return y > 200 ? p : y * 3;
  });
  const got = pixels.slice();
  m.gray(got, 3);
  let same = got.every((v, i) => v === want[i]);
  const xs = Array.from({ length: n }, (_, i) => (i % 1000) * 0.01);
  const vs = xs.map((x) => Math.sin(x * 7));
  const [wx, wv] = [xs.slice(), vs.slice()];
  for (let s = 0; s < 5; s++) {
    for (let i = 0; i < n; i++) {
      let v = wv[i] ?? 0;
      const x = (wx[i] ?? 0) + v * 0.01;
      if (x < 0 || x > 10) v = -v;
      wv[i] = v;
      wx[i] = Math.sqrt(x) * 0.5 + x * 0.5;
    }
  }
  m.move(xs, vs, 5);
  same = same && xs.every((x, i) => Object.is(x, wx[i])) && vs.every((v, i) => Object.is(v, wv[i]));
  let error = "";
  try {
    m.fill(
      Array.from({ length: 1000 }, () => 0),
      5000,
    );
  } catch (e) {
    error = `${(e as Error).name}: ${(e as Error).message}`;
  }
  m.__perasHeap.checkLeaks();
  return { threads: m.__perasHeap.threads(), same, error, live: m.__perasHeap.live() };
}

describe("parallel for", () => {
  it("runs on one thread where memory cannot be shared, with the same results", async () => {
    const compiled = await compile(source, { perasc, threads: true });
    const url = `data:text/javascript;base64,${Buffer.from(moduleCode(compiled)).toString("base64")}`;
    const m = (await import(url)) as Threads;
    const result = check(m);
    expect(result).toMatchObject({ threads: 1, same: true, live: 0 });
    expect(result.error).toContain("RangeError");
  });

  describe("in a cross-origin isolated page", () => {
    let browser: Browser;
    let server: ViteDevServer;
    let dir: string;
    beforeAll(async () => {
      dir = await mkdtemp(join(tmpdir(), "peras-threads-"));
      await copyFile(source, join(dir, "threads.peras"));
      await writeFile(
        join(dir, "index.html"),
        '<!doctype html><meta charset="utf-8"><title>threads</title><script type="module" src="./main.js"></script>\n',
      );
      await writeFile(
        join(dir, "main.js"),
        `import * as m from "./threads.peras";\nglobalThis.threads = m;\ndocument.body.dataset.ready = "true";\n`,
      );
      server = await createServer({
        root: dir,
        logLevel: "silent",
        server: { port: 0 },
        plugins: [peras({ perasc, dts: false, threads: true })],
      });
      await server.listen();
      const executablePath = process.env.PERAS_CHROMIUM;
      browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
    });
    afterAll(async () => {
      await browser.close();
      await server.close();
      await rm(dir, { recursive: true, force: true });
    });

    it("runs on several threads, with the same results", async () => {
      const page = await browser.newPage();
      const errors: string[] = [];
      page.on("pageerror", (e) => errors.push(e.message));
      await page.goto(server.resolvedUrls?.local[0] ?? "");
      await page.waitForSelector("body[data-ready=true]", { state: "attached" });
      expect(await page.evaluate("crossOriginIsolated")).toBe(true);
      const result = await page.evaluate<ReturnType<typeof check>>(
        `(${check.toString()})(globalThis.threads)`,
      );
      const cores = await page.evaluate(() => navigator.hardwareConcurrency);
      expect(result).toMatchObject({
        threads: Math.min(Math.max(cores, 1), 16),
        same: true,
        live: 0,
      });
      expect(result.error).toMatch(/RangeError: .*5000|RangeError: .*1000/);
      expect(errors).toEqual([]);
      await page.close();
    });
  });
});
