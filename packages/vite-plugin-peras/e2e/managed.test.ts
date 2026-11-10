import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium, type Browser } from "playwright";
import { build, createServer, preview } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import peras, { PerasError, compile, compileManaged, moduleCode } from "../src/index.ts";

// Managed modules (decision 0008): `.apeiron` files compiled to JavaScript,
// which compute what the same source computes compiled to WebAssembly, and
// call owned modules.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const fixtures = fileURLToPath(new URL("fixtures/managed", import.meta.url));

type Kernels = Record<string, (...args: unknown[]) => unknown>;

/** Imports JavaScript from its text. */
async function importCode(code: string): Promise<Kernels> {
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Kernels;
}

/** What a call gives, or the class and text of what it throws. */
function outcome(m: Kernels, name: string, args: unknown[]): unknown {
  try {
    const f = m[name];
    if (f === undefined) return `missing ${name}`;
    return f(...args);
  } catch (e) {
    return `${(e as Error).constructor.name}: ${(e as Error).message}`;
  }
}

const calls: [string, ...unknown[]][] = [
  ["integers", 123456789, -77],
  ["integers", -2147483648, 3],
  ["integers", -2147483648, -1],
  ["longs", 9007199254740993n, -12345n],
  ["longs", 5n, 0n],
  ["floats", 2.75],
  ["floats", 1e10],
  ["floats", -0.3],
  ["conversions", 3.99],
  ["conversions", -0.5],
  ["words", "b a b  c a b"],
  ["sorted", [3, -1, 0.5, -0, 0, 2]],
  ["shapes", 1.5],
  ["options", 12],
  ["options", 6],
  ["options", 7],
  ["formatted", 0.1, -5],
  ["formatted", 1e21, 7],
  ["faults", 2],
  ["faults", 3],
  ["faults", 1],
];

/** `report(n)` computed in JavaScript. */
function report(n: number): string {
  const xs = Array.from({ length: n }, (_, i) => ((i * 37) % 100) / 100);
  const mean = xs.reduce((a, x) => a + x, 0) / n;
  const buckets = [0, 0, 0, 0];
  for (const x of xs) {
    const k = Math.min(Math.max(Math.trunc(x * 4), 0), 3);
    buckets[k] = (buckets[k] ?? 0) + 1;
  }
  return `mean ${String(mean)}, buckets ${buckets.join(",")}`;
}

describe("a managed module", () => {
  it("computes what the same source computes in an owned one", async () => {
    const owned = await importCode(
      moduleCode(await compile(join(fixtures, "kernels.peras"), { perasc })),
    );
    const managed = await importCode(
      (await compileManaged(join(fixtures, "kernels.apeiron"), { perasc })).js,
    );
    for (const [name, ...args] of calls) {
      const copy = structuredClone(args);
      expect(outcome(managed, name, copy), `${name}(${String(args)})`).toEqual(
        outcome(owned, name, args),
      );
    }
  });

  it("is declared to TypeScript", async () => {
    const { dts } = await compileManaged(join(fixtures, "app.apeiron"), { perasc });
    expect(dts).toContain("export declare function report(n: number): Promise<string>;");
  });

  it("is built by the CLI with the owned modules it imports", async () => {
    const out = await mkdtemp(join(tmpdir(), "peras-managed-"));
    try {
      await promisify(execFile)(perasc, ["build", join(fixtures, "app.apeiron"), "--out", out]);
      const js = await readFile(join(out, "app.js"), "utf8");
      expect(js).toContain('from "./lib/stats.js"');
      const app = (await import(pathToFileURL(join(out, "app.js")).href)) as {
        report(n: number): Promise<string>;
      };
      expect(await app.report(10)).toBe(report(10));
    } finally {
      await rm(out, { recursive: true, force: true });
    }
  });

  it("reports an import that cannot be made", async () => {
    const dir = await mkdtemp(join(tmpdir(), "peras-managed-"));
    try {
      const file = join(dir, "bad.apeiron");
      await writeFile(
        file,
        'import { median } from "./missing.peras";\n\nexport fn f(): number {\n  return median([1.0]);\n}\n',
      );
      const error = await compileManaged(file, { perasc, lang: "en" }).catch((e: unknown) => e);
      expect(error).toBeInstanceOf(PerasError);
      expect((error as PerasError).message).toContain("E0244");
      expect((error as PerasError).loc).toEqual({ file, line: 1, column: 24 });
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});

describe("a managed module in Vite", () => {
  let browser: Browser;
  let dir: string;
  beforeAll(async () => {
    dir = await mkdtemp(join(tmpdir(), "peras-managed-vite-"));
    await mkdir(join(dir, "lib"));
    await copyFile(join(fixtures, "app.apeiron"), join(dir, "app.apeiron"));
    await copyFile(join(fixtures, "lib", "stats.peras"), join(dir, "lib", "stats.peras"));
    await writeFile(
      join(dir, "index.html"),
      '<!doctype html><meta charset="utf-8"><title>managed</title><link rel="icon" href="data:,"><script type="module" src="./main.js"></script>\n',
    );
    await writeFile(
      join(dir, "main.js"),
      'import { report } from "./app.apeiron";\ndocument.body.textContent = await report(10);\ndocument.body.dataset.ready = "true";\n',
    );
    const executablePath = process.env.PERAS_CHROMIUM;
    browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
  });
  afterAll(async () => {
    await browser.close();
    await rm(dir, { recursive: true, force: true });
  });

  /** Opens the page and reads what the module wrote. */
  async function reads(url: string): Promise<string | null> {
    const page = await browser.newPage();
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("response", (r) => {
      if (r.status() >= 400) errors.push(`${String(r.status())} ${r.url()}`);
    });
    await page.goto(url);
    await page.waitForSelector("body[data-ready=true]");
    const text = await page.textContent("body");
    expect(errors).toEqual([]);
    await page.close();
    return text;
  }

  it("runs from the dev server, and writes its declarations", async () => {
    const server = await createServer({
      root: dir,
      logLevel: "silent",
      server: { port: 0 },
      plugins: [peras({ perasc })],
    });
    await server.listen();
    try {
      expect(await reads(server.resolvedUrls?.local[0] ?? "")).toBe(report(10));
      const dts = await readFile(join(dir, "app.d.apeiron.ts"), "utf8");
      expect(dts).toContain("Generated by vite-plugin-peras from the .apeiron file");
      expect(dts).toContain("export declare function report(n: number): Promise<string>;");
    } finally {
      await server.close();
    }
  });

  it("runs from the production build", async () => {
    const outDir = await mkdtemp(join(tmpdir(), "peras-managed-out-"));
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
        expect(await reads(server.resolvedUrls?.local[0] ?? "")).toBe(report(10));
      } finally {
        await server.close();
      }
    } finally {
      await rm(outDir, { recursive: true, force: true });
    }
  });
});
