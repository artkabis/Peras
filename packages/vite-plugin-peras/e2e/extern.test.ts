import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Browser } from "playwright";
import { build, createServer, preview } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import peras from "../src/index.ts";

// Calling JavaScript from a managed module (decision 0010): an npm package,
// imported by its name, called with the types declared in an `extern`
// block; what it gives back is checked, in Chromium, from Vite's dev server
// and its production build.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");

const app = `extern from "peras-demo-chart" {
  type Chart;
  fn makeChart(canvas: CanvasElement): Chart;
  fn plot(self: Chart, values: Array<number>): i32;
  fn onPoint(self: Chart, f: (i32, number) => void): void;
  fn canvasOf(self: Chart): CanvasElement;
  fn broken(self: Chart): Array<i32>;
}

let picked = "";

fn point(i: i32, value: number): void {
  picked += \`\${i}:\${value} \`;
}

export fn run(): String {
  match (document.querySelector("canvas")) {
    Some(el) => match (el.asCanvas()) {
      Some(canvas) => {
        const chart = makeChart(canvas);
        chart.onPoint((i, value) => point(i, value));
        const drawn = chart.plot([1.5, 2.0, 0.5]);
        const same = chart.canvasOf().width == canvas.width;
        return \`\${drawn} \${same} \${picked.trim()}\`;
      }
      None => {}
    },
    None => {}
  }
  return "no canvas";
}

export fn lie(): String {
  match (document.querySelector("canvas")) {
    Some(el) => match (el.asCanvas()) {
      Some(canvas) => {
        const chart = makeChart(canvas);
        return \`\${chart.broken().length}\`;
      }
      None => {}
    },
    None => {}
  }
  return "no canvas";
}
`;

const library = `export class Chart {
  constructor(canvas) { this.canvas = canvas; this.listeners = []; }
  plot(values) {
    const ctx = this.canvas.getContext("2d");
    values.forEach((v, i) => { ctx.fillRect(i * 10, 0, 8, v * 10); for (const f of this.listeners) f(i, v); });
    return values.length;
  }
  onPoint(f) { this.listeners.push(f); }
  canvasOf() { return this.canvas; }
  broken() { return [1, "two", 3]; }
}
export const makeChart = (canvas) => new Chart(canvas);
`;

describe("an npm package called from a managed module", () => {
  let browser: Browser;
  let dir: string;
  beforeAll(async () => {
    dir = await mkdtemp(join(tmpdir(), "peras-extern-"));
    const pkg = join(dir, "node_modules", "peras-demo-chart");
    await mkdir(pkg, { recursive: true });
    await writeFile(
      join(pkg, "package.json"),
      JSON.stringify({
        name: "peras-demo-chart",
        version: "1.0.0",
        type: "module",
        main: "index.js",
      }),
    );
    await writeFile(join(pkg, "index.js"), library);
    await writeFile(join(dir, "app.apeiron"), app);
    await writeFile(
      join(dir, "index.html"),
      '<!doctype html><meta charset="utf-8"><title>extern</title><link rel="icon" href="data:,">' +
        '<canvas width="40" height="40"></canvas><script type="module" src="./main.js"></script>\n',
    );
    await writeFile(
      join(dir, "main.js"),
      'import { run, lie } from "./app.apeiron";\n' +
        "document.body.dataset.result = run();\n" +
        "try { lie(); } catch (e) { document.body.dataset.error = `${e.name}: ${e.message}`; }\n" +
        'document.body.dataset.ready = "true";\n',
    );
    const executablePath = process.env.PERAS_CHROMIUM;
    browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
  });
  afterAll(async () => {
    await browser.close();
    await rm(dir, { recursive: true, force: true });
  });

  /** What the page computed, and the error the lying method threw. */
  async function results(url: string): Promise<{ result?: string; error?: string }> {
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.goto(url);
    await page.waitForSelector("body[data-ready=true]");
    const data = await page.evaluate<{ result?: string; error?: string }>(
      "({ ...document.body.dataset })",
    );
    expect(errors).toEqual([]);
    await page.close();
    return data;
  }

  const expected = {
    result: "3 true 0:1.5 1:2 2:0.5",
    error:
      'TypeError: peras: Chart.broken(…) returned the string "two" where its declaration says Array<i32>',
  };

  it("runs from the dev server", async () => {
    const server = await createServer({
      root: dir,
      logLevel: "silent",
      server: { port: 0 },
      plugins: [peras({ perasc, dts: false })],
    });
    await server.listen();
    try {
      expect(await results(server.resolvedUrls?.local[0] ?? "")).toMatchObject(expected);
    } finally {
      await server.close();
    }
  });

  it("runs from the production build", async () => {
    const outDir = await mkdtemp(join(tmpdir(), "peras-extern-out-"));
    try {
      await build({
        root: dir,
        logLevel: "silent",
        build: { outDir, emptyOutDir: true },
        plugins: [peras({ perasc, dts: false })],
      });
      const server = await preview({
        root: dir,
        logLevel: "silent",
        build: { outDir },
        preview: { port: 0 },
      });
      try {
        expect(await results(server.resolvedUrls?.local[0] ?? "")).toMatchObject(expected);
      } finally {
        await server.close();
      }
    } finally {
      await rm(outDir, { recursive: true, force: true });
    }
  });
});
