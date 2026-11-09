// Browser benchmark: Peras against the same code written in JavaScript, in
// Chromium. Builds bench/browser/kernels.peras with the production allocator,
// serves it next to bench/browser/reference.js, checks in the page that both
// give the same results, then times them and the start of each module.
// Writes bench/browser/RESULTS.md.
// Usage: node scripts/bench-browser.mjs           (needs pnpm install and a release perasc)
//        node scripts/bench-browser.mjs --check   small inputs, one run: only
//        checks the results agree, measures nothing, writes nothing.
// PERAS_CHROMIUM names the Chromium to use, as for the end-to-end tests.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { URLSearchParams } from "node:url";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";

/** @typedef {import("playwright").Browser} Browser */
/** @typedef {import("../bench/browser/runner.js").Case} Case */
/** @typedef {import("../bench/browser/runner.js").Config} Config */

const check = process.argv.includes("--check");
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "browser");
const work = join(root, "target", "bench-browser");
const perasc = join(root, "target", "release", "perasc");

/** Builds the kernels and gathers the files the page loads. */
function build() {
  rmSync(work, { recursive: true, force: true });
  mkdirSync(work, { recursive: true });
  const args = ["build", join(bench, "kernels.peras"), "--allocator", "tlsf", "--out", work];
  const result = spawnSync(perasc, args, { encoding: "utf8" });
  if (result.status !== 0)
    throw new Error(`perasc ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  // runner.js is type-checked against the declarations kept next to it.
  const declared = readFileSync(join(bench, "kernels.d.ts"), "utf8");
  if (readFileSync(join(work, "kernels.d.ts"), "utf8") !== declared)
    throw new Error(
      "bench/browser/kernels.d.ts is stale: copy the one perasc wrote to target/bench-browser",
    );
  for (const file of ["reference.js", "runner.js"])
    copyFileSync(join(bench, file), join(work, file));
  // An empty page for the start measures, and the page that runs the kernels
  // with the settings of its query, then reports to the functions this
  // script exposes.
  const head = "<!doctype html><meta charset=utf-8><title>bench</title>\n";
  writeFileSync(join(work, "blank.html"), head);
  writeFileSync(
    join(work, "index.html"),
    `${head}<script type="module">
import { run } from "./runner.js";
const q = new URLSearchParams(location.search);
try {
  await report(run({ scale: Number(q.get("scale")), runs: Number(q.get("runs")), warm: Number(q.get("warm")) }));
} catch (e) {
  await fail(e instanceof Error ? e.stack ?? e.message : String(e));
}
</script>
`,
  );
}

/** @type {Record<string, string>} */
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };

/**
 * Serves the work folder on a free port of 127.0.0.1.
 * @returns {Promise<{ server: import("node:http").Server, base: string }>}
 */
function serve() {
  const server = createServer((request, response) => {
    const path = join(work, new URL(request.url ?? "/", "http://x").pathname);
    if (!path.startsWith(work)) {
      response.writeHead(403).end();
      return;
    }
    try {
      const body = readFileSync(path);
      response.writeHead(200, {
        "content-type": types[extname(path)] ?? "application/octet-stream",
      });
      response.end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address !== null ? address.port : 0;
      resolve({ server, base: `http://127.0.0.1:${String(port)}` });
    });
  });
}

/**
 * Time to import a module in a fresh context, with nothing cached: fetch,
 * compile, instantiate. The median of `samples`.
 * @param {Browser} browser
 * @param {string} base
 * @param {string} file
 * @param {number} samples
 * @returns {Promise<number>}
 */
async function startup(browser, base, file, samples) {
  /** @type {number[]} */
  const times = [];
  for (let i = 0; i < samples; i++) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await page.goto(`${base}/blank.html`);
    times.push(
      await page.evaluate(async (url) => {
        const t0 = performance.now();
        await import(url);
        return performance.now() - t0;
      }, `${base}/${file}`),
    );
    await context.close();
  }
  times.sort((a, b) => a - b);
  return times[Math.floor(times.length / 2)] ?? 0;
}

/**
 * Raw and gzip size of a built file.
 * @param {string} file
 */
function size(file) {
  const bytes = readFileSync(join(work, file));
  return { raw: bytes.length, gzip: gzipSync(bytes, { level: 9 }).length };
}

/** @param {number} x */
const ms = (x) => (x >= 100 ? x.toFixed(0) : x >= 10 ? x.toFixed(1) : x.toFixed(2));
/** @param {number} x */
const kb = (x) => (x / 1024).toFixed(1);

/**
 * The results as Markdown.
 * @param {Case[]} cases
 * @param {{ peras: { raw: number, gzip: number, time: number }, js: { raw: number, gzip: number, time: number } }} start
 * @param {string} browserVersion
 */
function report(cases, start, browserVersion) {
  const machine = `${cpus()[0]?.model ?? "?"}, ${String(cpus().length)} cores`;
  const lines = [
    "# Browser benchmark — results",
    "",
    "Generated by `node scripts/bench-browser.mjs` from `bench/browser/kernels.peras` and `bench/browser/reference.js`. What it measures and what the results say: [README](../README.md#browser-benchmark-benchbrowser).",
    "",
    `Machine: ${machine}. Chromium ${browserVersion}, headless. Peras built with the TLSF allocator.`,
    "",
    "## Run time (ms)",
    "",
    "Median of the timed runs, after warm-up; _cold_ is the very first call, before V8 has optimized anything, which is what a page that runs a function once sees. Ratio: JavaScript's median divided by this variant's; above 1, Peras is faster.",
    "",
    "| Kernel | Work | Variant | Cold | Median | Best | Ratio |",
    "| --- | --- | --- | --: | --: | --: | --: |",
  ];
  for (const c of cases) {
    const reference = c.result[0]?.median ?? 0;
    c.result.forEach((r, k) => {
      const ratio = k === 0 ? "1.00" : (reference / r.median).toFixed(2);
      const [name, what] = k === 0 ? [c.name, c.what] : ["", ""];
      lines.push(
        `| ${name} | ${what} | ${r.label} | ${ms(r.cold)} | ${ms(r.median)} | ${ms(r.min)} | ${ratio} |`,
      );
    });
  }
  lines.push(
    "",
    "Every variant gave exactly the same result, checked in the page before timing (`Object.is`, positions and pixels included), and no block was left alive.",
    "",
    "## Start and download",
    "",
    "Import time: median of 15 fresh browser contexts, nothing cached, served from 127.0.0.1: fetching, compiling and instantiating the module (for Peras, the allocator and the program).",
    "",
    "| Module | Files | Raw (KiB) | gzip (KiB) | Import (ms) |",
    "| --- | --- | --: | --: | --: |",
    `| Peras | kernels.js + kernels.wasm | ${kb(start.peras.raw)} | ${kb(start.peras.gzip)} | ${ms(start.peras.time)} |`,
    `| JavaScript | reference.js | ${kb(start.js.raw)} | ${kb(start.js.gzip)} | ${ms(start.js.time)} |`,
    "",
    "The Peras loader carries the allocator inlined and the code that crosses the boundary; it is shared by every function of a module.",
    "",
  );
  return lines.join("\n");
}

build();
const { server, base } = await serve();
const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
try {
  const page = await (await browser.newContext()).newPage();
  /** @type {Config} */
  const config = check ? { scale: 0.01, runs: 1, warm: 0 } : { scale: 1, runs: 15, warm: 3 };
  /** @type {(cases: Case[]) => void} */
  let done = () => undefined;
  /** @type {(error: Error) => void} */
  let failed = () => undefined;
  /** @type {Promise<Case[]>} */
  const ran = new Promise((resolve, reject) => {
    done = resolve;
    failed = reject;
  });
  await page.exposeFunction(
    "report",
    /** @param {Case[]} cases */ (cases) => {
      done(cases);
    },
  );
  await page.exposeFunction(
    "fail",
    /** @param {string} message */ (message) => {
      failed(new Error(message));
    },
  );
  page.on("pageerror", (e) => {
    failed(e);
  });
  const query = new URLSearchParams({
    scale: String(config.scale),
    runs: String(config.runs),
    warm: String(config.warm),
  });
  await page.goto(`${base}/index.html?${query.toString()}`);
  const cases = await ran;
  if (check) {
    console.log(
      `Peras and JavaScript agree on ${String(cases.length)} kernels in Chromium ${browser.version()}.`,
    );
  } else {
    const wasm = size("kernels.wasm");
    const loader = size("kernels.js");
    const start = {
      peras: {
        raw: wasm.raw + loader.raw,
        gzip: wasm.gzip + loader.gzip,
        time: await startup(browser, base, "kernels.js", 15),
      },
      js: { ...size("reference.js"), time: await startup(browser, base, "reference.js", 15) },
    };
    const markdown = report(cases, start, browser.version());
    writeFileSync(join(bench, "RESULTS.md"), markdown);
    console.log(markdown);
  }
} finally {
  await browser.close();
  server.close();
}
