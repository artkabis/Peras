import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Browser } from "playwright";
import { build, createServer, preview, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import peras, { EXPORTS_MARKER, PerasError, compile } from "../src/index.ts";

const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const example = join(root, "examples", "fib");
const broken = fileURLToPath(new URL("fixtures/broken", import.meta.url));

/** fib computed in JavaScript, with the same 32-bit wrapping. */
function fib(n: number): number {
  let [a, b] = [0, 1];
  for (let i = 0; i < n; i++) [a, b] = [b, (a + b) | 0];
  return a;
}

describe("compile", () => {
  it("builds a module, its exports and its declarations", async () => {
    const compiled = await compile(join(example, "src", "fib.peras"), { perasc });
    expect(Array.from(compiled.wasm.subarray(0, 4))).toEqual([0, 0x61, 0x73, 0x6d]);
    expect(compiled.exports.startsWith(EXPORTS_MARKER)).toBe(true);
    expect(compiled.dts).toContain("export declare function fib(n: number): number;");
  });

  it("locates the first error and keeps the rendered diagnostics", async () => {
    const error = await compile(join(broken, "broken.peras"), { perasc, lang: "fr" }).catch(
      (e: unknown) => e,
    );
    expect(error).toBeInstanceOf(PerasError);
    const { message, loc, frame } = error as PerasError;
    expect(message).toBe("E0101: `;` manquant");
    expect(loc).toEqual({ file: join(broken, "broken.peras"), line: 2, column: 12 });
    // Excerpt, cause and fix, in the language asked.
    expect(frame).toContain("let x = 1;");
    expect(frame).toContain("pourquoi");
    expect(frame).toContain("correction");
  });

  it("explains how to get perasc when it is missing", async () => {
    const error = await compile(join(example, "src", "fib.peras"), {
      perasc: "perasc-that-does-not-exist",
    }).catch((e: unknown) => e);
    expect(String(error)).toContain("cargo build --release -p perasc");
  });
});

describe("in Vite", () => {
  let server: ViteDevServer;
  beforeAll(async () => {
    server = await createServer({
      root: broken,
      logLevel: "silent",
      server: { port: 0 },
      plugins: [peras({ perasc, lang: "en", dts: false })],
    });
  });
  afterAll(async () => {
    await server.close();
  });

  it("reports an error with its location, which fails the build", async () => {
    const error = await server.transformRequest("/broken.peras").catch((e: unknown) => e);
    expect(error).toMatchObject({
      message: expect.stringContaining("E0101: missing `;`") as unknown,
    });
    const failed = await build({
      root: broken,
      logLevel: "silent",
      build: { outDir: await mkdtemp(join(tmpdir(), "peras-out-")) },
      plugins: [peras({ perasc, dts: false })],
    }).catch((e: unknown) => e);
    expect(String(failed)).toContain("E0101");
  });
});

describe("in the browser", () => {
  let browser: Browser;
  beforeAll(async () => {
    const executablePath = process.env.PERAS_CHROMIUM;
    browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
  });
  afterAll(async () => {
    await browser.close();
  });

  /** Opens the example, checks fib(30), then runs fib(20) from the form. */
  async function runsFib(url: string): Promise<void> {
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => {
      if (m.type() === "error") errors.push(m.text());
    });
    page.on("response", (r) => {
      if (r.status() >= 400) errors.push(`${String(r.status())} ${r.url()}`);
    });
    await page.goto(url);
    await page.waitForSelector("body[data-ready=true]");
    expect(await page.textContent("#wasm-result")).toBe(String(fib(30)));
    expect(await page.textContent("#js-result")).toBe(String(fib(30)));
    await page.fill("#n", "20");
    await page.click("#run");
    expect(await page.textContent("#wasm-result")).toBe(String(fib(20)));
    expect(errors).toEqual([]);
    await page.close();
  }

  it("runs fib from the production build", async () => {
    const outDir = await mkdtemp(join(tmpdir(), "peras-fib-"));
    try {
      await build({ root: example, logLevel: "silent", build: { outDir, emptyOutDir: true } });
      const assets = await readFile(join(outDir, "index.html"), "utf8");
      expect(assets).toContain("/assets/index-");
      const server = await preview({
        root: example,
        logLevel: "silent",
        build: { outDir },
        preview: { port: 0 },
      });
      try {
        await runsFib(server.resolvedUrls?.local[0] ?? "");
      } finally {
        await server.close();
      }
    } finally {
      await rm(outDir, { recursive: true, force: true });
    }
  });

  it("runs fib from the dev server", async () => {
    const server = await createServer({ root: example, logLevel: "silent", server: { port: 0 } });
    await server.listen();
    try {
      await runsFib(server.resolvedUrls?.local[0] ?? "");
    } finally {
      await server.close();
    }
  });

  it("shows the error in the overlay of the dev server", async () => {
    const server = await createServer({
      root: broken,
      logLevel: "silent",
      server: { port: 0 },
      plugins: [peras({ perasc, lang: "fr", dts: false })],
    });
    await server.listen();
    try {
      const page = await browser.newPage();
      await page.goto(server.resolvedUrls?.local[0] ?? "");
      // The overlay lives in an open shadow root, which CSS locators pierce.
      const overlay = page.locator("vite-error-overlay");
      await overlay.locator(".message-body").waitFor();
      expect(await overlay.locator(".message-body").textContent()).toContain("E0101: `;` manquant");
      expect(await overlay.locator(".file").textContent()).toContain("broken.peras:2:12");
      const frame = await overlay.locator(".frame").textContent();
      expect(frame).toContain("pourquoi");
      expect(frame).toContain("let x = 1;");
      await page.close();
    } finally {
      await server.close();
    }
  });
});
