import { copyFile, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Browser, type Page } from "playwright";
import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import peras from "../src/index.ts";

// The browser, typed, in a managed module (decision 0009): the DOM, events,
// a canvas, timers, fetch and storage, in Chromium, and resources released
// with their owner, a leaked one reported by the debug build.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const fixture = fileURLToPath(new URL("fixtures/browser/ui.apeiron", import.meta.url));

describe("the browser in a managed module", () => {
  let browser: Browser;
  let server: ViteDevServer;
  let dir: string;
  let page: Page;
  const errors: string[] = [];
  const logged: string[] = [];

  beforeAll(async () => {
    dir = await mkdtemp(join(tmpdir(), "peras-browser-"));
    await copyFile(fixture, join(dir, "ui.apeiron"));
    await writeFile(join(dir, "data.txt"), "  hello from the server  \n");
    await writeFile(
      join(dir, "index.html"),
      '<!doctype html><meta charset="utf-8"><title>ui</title><link rel="icon" href="data:,">' +
        '<main id="app"><input id="name"><canvas width="4" height="4"></canvas></main>' +
        '<button id="other">other</button><script type="module" src="./main.js"></script>\n',
    );
    await writeFile(
      join(dir, "main.js"),
      'import * as ui from "./ui.apeiron";\nglobalThis.ui = ui;\ndocument.body.dataset.ready = "true";\n',
    );
    server = await createServer({
      root: dir,
      logLevel: "silent",
      server: { port: 0 },
      plugins: [
        peras({ perasc, dts: false }),
        // A route that answers with an error, for `response.ok`.
        {
          name: "teapot",
          configureServer(vite) {
            vite.middlewares.use("/teapot", (_request, response) => {
              response.statusCode = 418;
              response.end("no coffee");
            });
          },
        },
      ],
    });
    await server.listen();
    const executablePath = process.env.PERAS_CHROMIUM;
    browser = await chromium.launch({
      ...(executablePath === undefined ? {} : { executablePath }),
      // `gc()` in the page, to test the leak detector.
      args: ["--js-flags=--expose-gc"],
    });
    page = await browser.newPage();
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => {
      if (m.type() === "error" && m.text().startsWith("peras:")) logged.push(m.text());
    });
    await page.goto(server.resolvedUrls?.local[0] ?? "");
    await page.waitForSelector("body[data-ready=true]", { state: "attached" });
  });

  afterAll(async () => {
    await browser.close();
    await server.close();
    await rm(dir, { recursive: true, force: true });
  });

  it("builds the page, listens, draws and ticks, then releases it all", async () => {
    await page.evaluate("globalThis.group = ui.mount(document.getElementById('app'))");
    for (let i = 0; i < 3; i++) await page.click("#count");
    expect(await page.textContent("#clicks")).toBe("3");
    await page.fill("#name", "peras");
    expect(await page.textContent("#echo")).toBe("PERAS");
    // The canvas is filled with the colour drawn.
    const pixel = await page.evaluate<number[]>(
      "Array.from(document.querySelector('canvas').getContext('2d').getImageData(1, 1, 1, 1).data)",
    );
    expect(pixel).toEqual([163, 75, 38, 255]);
    await page.waitForFunction("ui.tickCount() > 2");

    // Released: the listeners are removed, the timer cleared.
    await page.evaluate("group.release()");
    const ticks = await page.evaluate<number>("ui.tickCount()");
    await page.click("#count");
    await page.fill("#name", "after");
    await page.waitForTimeout(50);
    expect(await page.textContent("#clicks")).toBe("3");
    expect(await page.textContent("#echo")).toBe("PERAS");
    expect(await page.evaluate<number>("ui.tickCount()")).toBe(ticks);
    // Released twice: nothing more happens.
    await page.evaluate("group.release()");
  });

  it("fetches, waits and stores", async () => {
    expect(await page.evaluate<string>("ui.load('/data.txt')")).toBe("hello from the server");
    expect(await page.evaluate<string>("ui.load('/teapot')")).toBe("error 418");
    expect(await page.evaluate<string>("ui.remember('k', 'v')")).toBe("v true");
    expect(await page.evaluate<number>("ui.waited(20)")).toBeGreaterThanOrEqual(15);
  });

  it("reports a resource collected without being released, with its line", async () => {
    await page.evaluate(`{ ui.forgotten(document.getElementById('other')); }`);
    // Collection and its callbacks are not immediate: ask a few times.
    for (let i = 0; i < 20 && logged.length === 0; i++) {
      await page.evaluate("gc()");
      await page.waitForTimeout(50);
    }
    expect(logged.join("\n")).toContain("ui.apeiron:87");
    expect(errors).toEqual([]);
  });
});
