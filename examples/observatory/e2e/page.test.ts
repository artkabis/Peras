import { fileURLToPath } from "node:url";
import { chromium, type Browser, type Page } from "playwright";
import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

// The page draws the same frame in both languages, says so, measures on the
// machine it runs on, and draws the recorded figures. Run on the dev server,
// where the module is built for debug: every memory access is checked.
const app = fileURLToPath(new URL("..", import.meta.url));

describe("the observatory", { timeout: 180_000 }, () => {
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
    page = await browser.newPage();
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

  it("draws the same pixels in both languages", async () => {
    await page.waitForFunction(() => Number(document.body.dataset.frames) >= 61, undefined, {
      timeout: 120_000,
    });
    expect(await page.getAttribute("body", "data-same")).toBe("true");
  });

  it("measures on the machine, with its control", async () => {
    await page.click("#measure");
    await page.waitForSelector("body[data-measured=true]", { state: "attached", timeout: 150_000 });
    const rows = await page.locator("#measured tr").count();
    expect(rows).toBe(4);
  });

  it("draws every recorded figure and every bug", async () => {
    expect(await page.locator("#map circle").count()).toBeGreaterThanOrEqual(6);
    expect(await page.locator("#bugs .cell").count()).toBe(34);
    expect(errors).toEqual([]);
  });
});
