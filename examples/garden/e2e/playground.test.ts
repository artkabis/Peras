import { fileURLToPath } from "node:url";
import { chromium, type Browser, type Page } from "playwright";
import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { render } from "../../../bench/garden/garden.ts";

// The playground shows the garden's own pixels: what its canvas holds after a
// full frame is, byte for byte, what the TypeScript reference draws for the
// same window. Run on the dev server, where the module is built for debug.
const app = fileURLToPath(new URL("..", import.meta.url));

interface Shown {
  side: number;
  zoom: number;
  cx: number;
  cy: number;
}

describe("the garden playground", { timeout: 120_000 }, () => {
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
    // A small panel keeps the renders quick: the dev server builds the
    // module for debug, its integer overflow checked, and CI machines are slow.
    page = await browser.newPage({ viewport: { width: 320, height: 900 }, deviceScaleFactor: 1 });
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => {
      if (m.type() === "error") errors.push(m.text());
    });
    await page.goto(`${server.resolvedUrls?.local[0] ?? ""}?lang=en`);
    try {
      await page.waitForSelector("body[data-ready=true]", { state: "attached", timeout: 90_000 });
    } catch (cause) {
      // Say what the page said, not only that it never got ready.
      errors.push(
        await page.evaluate(
          () =>
            `${document.getElementById("readout")?.textContent ?? ""} / ${String(document.body.dataset.frame)} / ${location.href}`,
        ),
      );
      throw new Error(
        `the playground never drew a full frame; the page said: ${JSON.stringify(errors)}`,
        {
          cause,
        },
      );
    }
  }, 150_000);

  afterAll(async () => {
    await browser.close();
    await server.close();
  });

  /** Waits for the full frame of a view `accept` takes, then checks its pixels. */
  async function same(accept: (shown: Shown) => boolean): Promise<Shown> {
    let shown: Shown | undefined;
    await expect
      .poll(
        async () => {
          const data = await page.evaluate(() => document.body.dataset.frame ?? "{}");
          const parsed = JSON.parse(data) as Shown;
          shown = parsed;
          return parsed.side > 0 && accept(parsed);
        },
        { timeout: 90_000 },
      )
      .toBe(true);
    if (shown === undefined) throw new Error("no frame");
    const { side, zoom, cx, cy } = shown;
    const drawn = await page.evaluate((n) => {
      const canvas = document.getElementById("drawn") as HTMLCanvasElement;
      const ctx = canvas.getContext("2d");
      if (ctx === null) return [];
      return Array.from(ctx.getImageData(0, 0, n, n).data);
    }, side);
    const reference = new Uint32Array(side * side);
    render(reference, side, side, zoom, cx, cy);
    expect(Buffer.from(Uint8Array.from(drawn)).equals(Buffer.from(reference.buffer))).toBe(true);
    return shown;
  }

  it("shows the whole garden first, as the reference draws it", async () => {
    const shown = await same((s) => s.zoom === 1);
    expect(shown).toMatchObject({ cx: 0.5, cy: 0.5 });
    expect(await page.textContent("#drawn-meta")).toMatch(/5,872 bytes/);
    expect(await page.textContent("#stored-meta")).toMatch(/9,774 bytes/);
  });

  it("zooms 64 times into the meadow, and the URL keeps the place", async () => {
    await page.click("#visit");
    await same((s) => s.zoom === 64);
    expect(page.url()).toContain("#z=64&x=0.37&y=0.83");
  });

  it("follows a drag, still with the reference's pixels", async () => {
    const box = await page.locator("#drawn").boundingBox();
    if (box === null) throw new Error("no canvas");
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2 - 40, { steps: 4 });
    await page.mouse.up();
    const shown = await same((s) => s.zoom === 64 && s.cx !== 0.37);
    expect(shown.cx).toBeLessThan(0.37);
    expect(errors).toEqual([]);
  });
});
