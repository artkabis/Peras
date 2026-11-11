import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { chromium, type Browser, type Page } from "playwright";
import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { Report } from "../src/metrics.ts";

// The app reads the stream in its worker, Peras and TypeScript in turn, finds
// the same numbers in every batch, leaks nothing, and exports what it measured.
// Run on the dev server, where the module is built for debug: every memory
// access and integer overflow is checked while it runs.
const app = fileURLToPath(new URL("..", import.meta.url));

describe("the stream app", { timeout: 120_000 }, () => {
  let browser: Browser;
  let server: ViteDevServer;
  let page: Page;
  const errors: string[] = [];

  beforeAll(async () => {
    server = await createServer({
      root: app,
      configFile: fileURLToPath(new URL("../vite.config.ts", import.meta.url)),
      logLevel: "silent",
      server: { port: 0 },
    });
    await server.listen();
    const executablePath = process.env.PERAS_CHROMIUM;
    browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
    page = await browser.newPage({ acceptDownloads: true });
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => {
      if (m.type() === "error") errors.push(m.text());
    });
    await page.goto(`${server.resolvedUrls?.local[0] ?? ""}?lang=en`);
    await page.waitForSelector("body[data-ready=true]", { state: "attached", timeout: 90_000 });
  }, 150_000);

  afterAll(async () => {
    await browser.close();
    await server.close();
  });

  it("reads batches with the same numbers in both languages, and exports them", async () => {
    await page.selectOption("#rows", "10000");
    await page.click("#run");
    await page.waitForFunction(() => Number(document.body.dataset.batches) >= 6, undefined, {
      timeout: 90_000,
    });
    await page.click("#run");
    // The batch in flight when paused still comes back.
    await page.waitForTimeout(500);

    const text = await page.evaluate(() =>
      (window as unknown as { __streamReport: () => string }).__streamReport(),
    );
    const report = JSON.parse(text) as Report;
    expect(report.samples.length).toBeGreaterThanOrEqual(6);
    expect(report.summary.compared).toBe(report.samples.length);
    expect(report.summary.disagreed).toBe(0);
    expect(report.summary.maxLive).toBe(0);
    // Peras refused exactly the lines the generator broke, in every batch.
    expect(report.summary.brokenMismatches).toBe(0);
    expect(report.summary.broken).toBeGreaterThan(0);
    expect(report.summary.rows).toBe(report.samples.length * 10000);
    expect(report.settings).toEqual({ rowsPerBatch: 10000, compare: true, seed: 7 });
    expect(report.environment.build).toMatch(/^development/);
    // The two digests take turns going first.
    expect(new Set(report.samples.map((s) => s.first))).toEqual(new Set(["peras", "typescript"]));

    const zones = await page.locator("#zones tbody tr").count();
    expect(zones).toBe(13); // twelve zones and the hottest reading

    const [download] = await Promise.all([page.waitForEvent("download"), page.click("#csv")]);
    const csv = await readFile(await download.path(), "utf8");
    const lines = csv.trimEnd().split("\n");
    expect(lines[0]).toBe(
      "batch,rows,bytes,generateMs,perasMs,typescriptMs,first,agree,live,broken,injected,at",
    );
    expect(lines.length).toBe(report.samples.length + 1);
    expect(errors).toEqual([]);
  });
});
