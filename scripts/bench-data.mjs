// Data at scale (decision 0014), measured under its registered protocol, as
// amended before any measure: a sales dashboard computed from a CSV export by
//   A. TypeScript, idiomatic (bench/data/dashboard-idiomatic.ts);
//   B. TypeScript, optimized by hand (dashboard-optimized.ts);
//   C. Peras, an owned module compiled to WebAssembly (dashboard.peras);
//   D. Peras in parallel (dashboard-parallel.peras), if perasc builds it.
// 1. The data, generated from a fixed seed (bench/data/generate.ts), and the
//    SHA-256 of each file.
// 2. Same summary: SHA-256 of each version's canonical summary, at each size.
// 3. Time to the summary, warm: median of 15 runs after 2 of warm-up, the
//    versions alternating, in Chromium.
// 4. Time to the first chart, cold: a new browser context per load, median of 7.
// 5. Memory: the peak resident memory of the renderer (VmHWM), a new browser
//    per run, minus its peak before the query; median of 5.
// 6. Bytes shipped, minified and gzip; lines of code; energy where RAPL is.
// Usage: node scripts/bench-data.mjs [--check | --smoke]
//        --check: the versions give the same summary at 100,000 rows, in Node;
//        nothing else is measured or written.
//        --smoke: every step at 20,000 and 40,000 rows, written to
//        target/bench-data, to try the script; not a measure.
// Writes bench/data/RESULTS.md and bench/data/results.json, unless --check or --smoke.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { TextDecoder } from "node:util";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";
import ts from "typescript";
import { minify } from "vite";

const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "data");
const check = process.argv.includes("--check");
const smoke = process.argv.includes("--smoke");
const perasc = join(root, "target", "release", "perasc");
const work = join(root, "target", "bench-data");
rmSync(join(work, "js"), { recursive: true, force: true });
rmSync(join(work, "c"), { recursive: true, force: true });
mkdirSync(join(work, "js"), { recursive: true });
mkdirSync(join(work, "data"), { recursive: true });

/** @param {string} command @param {string[]} args */
function sh(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 28 });
  if (result.error !== undefined) throw result.error;
  return result;
}

// 1. The versions: the TypeScript transpiled as written, Peras compiled.
/** @param {string} name */
function transpile(name) {
  const source = readFileSync(join(dir, `${name}.ts`), "utf8");
  const js = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  writeFileSync(join(work, "js", `${name}.js`), js.replaceAll(/\.ts(["'])/g, ".js$1"));
  return source;
}
for (const name of ["summary", "generate", "dashboard-idiomatic", "dashboard-optimized"])
  transpile(name);
const built = sh(perasc, [
  "build",
  join(dir, "dashboard.peras"),
  "--allocator",
  "tlsf",
  "--out",
  join(work, "c"),
]);
if (built.status !== 0) throw new Error(`perasc build dashboard.peras\n${built.stderr}`);
// D: built with threads, if perasc accepts it; its refusal is a finding.
const parallel = sh(perasc, [
  "build",
  join(dir, "dashboard-parallel.peras"),
  "--threads",
  "on",
  "--allocator",
  "tlsf",
  "--out",
  join(work, "d"),
  "--color",
  "never",
]);
const parallelRefusal =
  parallel.status === 0 ? undefined : (parallel.stderr.split("\n")[0] ?? "refused");

/** @typedef {{ summarize: (bytes: Uint8Array) => unknown }} Version */
/** @typedef {{ summarize: (text: string) => unknown }} PerasModule */
/** @param {string} path @returns {Promise<unknown>} */
async function load(path) {
  /** @type {unknown} */
  const loaded = await import(pathToFileURL(path).href);
  return loaded;
}
const summaryModule = /** @type {{ canonical: (s: unknown) => string }} */ (
  await load(join(work, "js", "summary.js"))
);
const generator = /** @type {{ generate: (rows: number) => Uint8Array }} */ (
  await load(join(work, "js", "generate.js"))
);
const peras = /** @type {PerasModule} */ (await load(join(work, "c", "dashboard.js")));
/** @type {Record<string, (bytes: Uint8Array) => unknown>} */
const versions = {
  A: /** @type {Version} */ (await load(join(work, "js", "dashboard-idiomatic.js"))).summarize,
  B: /** @type {Version} */ (await load(join(work, "js", "dashboard-optimized.js"))).summarize,
  C: (bytes) => peras.summarize(new TextDecoder().decode(bytes)),
};
const sha = (/** @type {string | Uint8Array} */ x) => createHash("sha256").update(x).digest("hex");

// 2. The data.
const sizes = check ? [100_000] : smoke ? [20_000, 40_000] : [100_000, 1_000_000, 4_000_000];
const small = String(sizes[0]);
const out = smoke ? work : dir;
/** @type {Record<string, { bytes: number, sha256: string }>} */
const files = {};
for (const n of sizes) {
  const bytes = generator.generate(n);
  writeFileSync(join(work, "data", `rows-${String(n)}.csv`), bytes);
  files[String(n)] = { bytes: bytes.length, sha256: sha(bytes) };
}

if (check) {
  const bytes = readFileSync(join(work, "data", "rows-100000.csv"));
  const hashes = Object.entries(versions).map(([name, f]) => [
    name,
    sha(summaryModule.canonical(f(new Uint8Array(bytes)))),
  ]);
  const distinct = new Set(hashes.map(([, h]) => h));
  if (distinct.size !== 1) {
    console.error("The versions give different summaries at 100,000 rows:", hashes);
    process.exit(1);
  }
  console.log(
    `The dashboard gives the same summary in A, B and C at 100,000 rows: ${String([...distinct][0]).slice(0, 16)}…`,
  );
  process.exit(0);
}

// 3. The page: the versions, the data, the shared chart.
const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".html": "text/html",
  ".csv": "text/csv",
});
const chart = `
export function draw(summary, canvas) {
  const ctx = canvas.getContext("2d");
  const w = canvas.width, h = canvas.height;
  ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, w, h);
  const daily = summary.daily.map(Number);
  const top = Math.max(1, ...daily);
  ctx.strokeStyle = "#2f6b3a"; ctx.lineWidth = 2; ctx.beginPath();
  daily.forEach((v, i) => {
    const x = 40 + (i / (daily.length - 1)) * (w - 80), y = h / 2 - 20 - (v / top) * (h / 2 - 60);
    if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
  });
  ctx.stroke();
  const by = new Map();
  for (const p of summary.pairs) by.set(p.country, (by.get(p.country) ?? 0) + Number(p.revenue));
  const bars = [...by.entries()], most = Math.max(1, ...bars.map(([, v]) => v));
  const bw = (w - 80) / Math.max(1, bars.length);
  ctx.fillStyle = "#8a5a2b"; ctx.font = "12px sans-serif";
  bars.forEach(([country, v], i) => {
    const bh = (v / most) * (h / 2 - 60);
    ctx.fillRect(40 + i * bw + 2, h - 30 - bh, bw - 4, bh);
    ctx.fillText(country, 40 + i * bw + 4, h - 12);
  });
}`;
writeFileSync(join(work, "js", "chart.js"), chart);
const entry = {
  A: "/js/dashboard-idiomatic.js",
  B: "/js/dashboard-optimized.js",
  C: "/c/dashboard.js",
};
const call = `(m, v) => v === "C" ? (bytes) => m.summarize(new TextDecoder().decode(bytes)) : m.summarize`;
const cold = `<!doctype html><meta charset="utf-8"><title>cold</title><link rel="icon" href="data:,">
<canvas width="1200" height="600"></canvas>
<script type="module">
  const q = new URLSearchParams(location.search);
  const v = q.get("v"), n = q.get("n");
  const m = await import(${JSON.stringify(entry)}[v]);
  const { draw } = await import("/js/chart.js");
  const bytes = new Uint8Array(await (await fetch("/data/rows-" + n + ".csv")).arrayBuffer());
  const summary = (${call})(m, v)(bytes);
  draw(summary, document.querySelector("canvas"));
  requestAnimationFrame(() => requestAnimationFrame(() => { window.__painted = performance.now(); }));
</script>`;
const blank = `<!doctype html><meta charset="utf-8"><title>data</title><link rel="icon" href="data:,">`;
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  if (path === "/" || path === "/cold") {
    response.writeHead(200, {
      "content-type": "text/html",
      "cache-control": "no-store",
    });
    response.end(path === "/" ? blank : cold);
    return;
  }
  try {
    const file = join(work, path);
    response.writeHead(200, {
      "content-type": types[extname(file)] ?? "text/plain",
      "cache-control": "no-store",
    });
    response.end(readFileSync(file));
  } catch {
    response.writeHead(404);
    response.end();
  }
});
await new Promise((resolve) => {
  server.listen(0, () => {
    resolve(undefined);
  });
});
const address = server.address();
const port = typeof address === "object" && address !== null ? address.port : 0;
const origin = `http://localhost:${String(port)}`;
const executablePath = process.env.PERAS_CHROMIUM;
const launchOptions = {
  ...(executablePath === undefined ? {} : { executablePath }),
  args: ["--js-flags=--expose-gc"],
};
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;

// Same summary and warm time, one size at a time, in one page.
/** @type {Record<string, Record<string, string>>} */
const summaries = {};
/** @type {Record<string, Record<string, number[]>>} */
const warm = {};
{
  const browser = await chromium.launch(launchOptions);
  const page = await browser.newPage();
  await page.goto(`${origin}/`);
  for (const n of sizes) {
    console.log(`warm, ${String(n)} rows…`);
    /** @type {unknown} */
    const out = await page.evaluate(
      `(async () => {
        const entry = ${JSON.stringify(entry)};
        const call = ${call};
        const { canonical } = await import("/js/summary.js");
        const fns = {};
        for (const v of Object.keys(entry)) fns[v] = call(await import(entry[v]), v);
        const bytes = new Uint8Array(await (await fetch("/data/rows-${String(n)}.csv")).arrayBuffer());
        const hex = async (s) => [...new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s)))].map((b) => b.toString(16).padStart(2, "0")).join("");
        const sums = {};
        for (const v of Object.keys(fns)) sums[v] = await hex(canonical(fns[v](bytes)));
        const times = Object.fromEntries(Object.keys(fns).map((v) => [v, []]));
        for (let run = 0; run < 17; run++)
          for (const v of Object.keys(fns)) {
            gc();
            const t0 = performance.now();
            fns[v](bytes);
            const ms = performance.now() - t0;
            if (run >= 2) times[v].push(ms);
          }
        return { sums, times };
      })()`,
    );
    const result =
      /** @type {{ sums: Record<string, string>, times: Record<string, number[]> }} */ (out);
    summaries[String(n)] = result.sums;
    warm[String(n)] = result.times;
  }
  await browser.close();
}

// Cold: a new context per load, nothing cached.
/** @type {Record<string, Record<string, number[]>>} */
const coldMs = {};
{
  const browser = await chromium.launch(launchOptions);
  for (const n of sizes) {
    console.log(`cold, ${String(n)} rows…`);
    /** @type {Record<string, number[]>} */
    const byVersion = { A: [], B: [], C: [] };
    for (let load = 0; load < 7; load++)
      for (const v of Object.keys(byVersion)) {
        const context = await browser.newContext();
        const page = await context.newPage();
        await page.goto(`${origin}/cold?v=${v}&n=${String(n)}`);
        await page.waitForFunction("window.__painted !== undefined", null, { timeout: 600_000 });
        const painted = /** @type {number} */ (await page.evaluate("window.__painted"));
        byVersion[v]?.push(painted);
        await context.close();
      }
    coldMs[String(n)] = byVersion;
  }
  // Exploratory: under a throttled network, at the smallest size only.
  /** @type {Record<string, number[]>} */
  const throttled = { A: [], B: [], C: [] };
  for (let load = 0; load < 3; load++)
    for (const v of Object.keys(throttled)) {
      const context = await browser.newContext();
      const page = await context.newPage();
      const cdp = await context.newCDPSession(page);
      // About a fast 4G: 9 Mb/s down, 1.5 Mb/s up, 170 ms of latency.
      await cdp.send("Network.emulateNetworkConditions", {
        offline: false,
        latency: 170,
        downloadThroughput: (9 * 1024 * 1024) / 8,
        uploadThroughput: (1.5 * 1024 * 1024) / 8,
      });
      await page.goto(`${origin}/cold?v=${v}&n=${small}`);
      await page.waitForFunction("window.__painted !== undefined", null, { timeout: 600_000 });
      throttled[v]?.push(/** @type {number} */ (await page.evaluate("window.__painted")));
      await context.close();
    }
  coldMs.throttled100000 = throttled;
  await browser.close();
}

// Memory: VmHWM of the renderer process, a new browser per run.
/** The pids of the renderer processes descending from `pid`. @param {number} pid */
function renderers(pid) {
  /** @type {Map<number, number[]>} */
  const children = new Map();
  for (const entry of readdirSync("/proc")) {
    if (!/^\d+$/.test(entry)) continue;
    try {
      const stat = readFileSync(`/proc/${entry}/stat`, "utf8");
      const ppid = Number(stat.slice(stat.lastIndexOf(")") + 2).split(" ")[1]);
      children.set(ppid, [...(children.get(ppid) ?? []), Number(entry)]);
    } catch {
      // The process ended.
    }
  }
  /** @type {number[]} */
  const out = [];
  const walk = (/** @type {number} */ p) => {
    for (const c of children.get(p) ?? []) {
      try {
        if (readFileSync(`/proc/${String(c)}/cmdline`, "utf8").includes("--type=renderer"))
          out.push(c);
      } catch {
        // The process ended.
      }
      walk(c);
    }
  };
  walk(pid);
  return out;
}
/** The largest VmHWM of the renderers, in KiB. @param {number} pid */
function peak(pid) {
  let most = 0;
  for (const r of renderers(pid)) {
    try {
      const status = readFileSync(`/proc/${String(r)}/status`, "utf8");
      const kb = Number(/VmHWM:\s+(\d+)/.exec(status)?.[1] ?? 0);
      most = Math.max(most, kb);
    } catch {
      // The process ended.
    }
  }
  return most;
}
/** @type {Record<string, Record<string, number[]>>} */
const memory = {};
for (const n of sizes) {
  console.log(`memory, ${String(n)} rows…`);
  /** @type {Record<string, number[]>} */
  const byVersion = { A: [], B: [], C: [] };
  for (let run = 0; run < 5; run++)
    for (const v of Object.keys(byVersion)) {
      const server = await chromium.launchServer(launchOptions);
      const pid = server.process().pid ?? 0;
      const browser = await chromium.connect(server.wsEndpoint());
      const page = await browser.newPage();
      await page.goto(`${origin}/`);
      await page.evaluate(
        `(async () => {
          const m = await import(${JSON.stringify(entry)}["${v}"]);
          window.__run = (${call})(m, "${v}");
          window.__bytes = new Uint8Array(await (await fetch("/data/rows-${String(n)}.csv")).arrayBuffer());
          gc();
        })()`,
      );
      const before = peak(pid);
      await page.evaluate("void window.__run(window.__bytes)");
      const after = peak(pid);
      byVersion[v]?.push((after - before) * 1024);
      await browser.close();
      await server.close();
    }
  memory[String(n)] = byVersion;
}
server.close();

// 6. Bytes, lines, energy.
const gz = (/** @type {string | Uint8Array} */ x) => gzipSync(x, { level: 9 }).length;
/** The version's module with the constants it imports, minified and gzip. @param {string} name */
const bundled = async (name) => {
  const constants = readFileSync(join(work, "js", "summary.js"), "utf8").replace(
    /export function canonical[\s\S]*$/,
    "",
  );
  const own = readFileSync(join(work, "js", `${name}.js`), "utf8").replace(/^import .*$/m, "");
  return gz((await minify(`${name}.js`, constants + own)).code);
};
/** @type {Record<string, number>} */
const shipped = {
  A: await bundled("dashboard-idiomatic"),
  B: await bundled("dashboard-optimized"),
  C:
    gz((await minify("dashboard.js", readFileSync(join(work, "c", "dashboard.js"), "utf8"))).code) +
    gz(readFileSync(join(work, "c", "dashboard.wasm"))),
};
const lines = (/** @type {string} */ s) =>
  s.split("\n").filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;
// The types of the summary are declared in summary.ts for A and B, which is not
// counted; C declares them in its file, as `export struct`: left out alike.
const withoutResultTypes = (/** @type {string} */ s) =>
  s.replaceAll(/^export struct \w+ \{[^}]*\}\n/gm, "");
/** @type {Record<string, number>} */
const code = {
  A: lines(readFileSync(join(dir, "dashboard-idiomatic.ts"), "utf8")),
  B: lines(readFileSync(join(dir, "dashboard-optimized.ts"), "utf8")),
  C: lines(withoutResultTypes(readFileSync(join(dir, "dashboard.peras"), "utf8"))),
};
const rapl = existsSync("/sys/class/powercap/intel-rapl:0/energy_uj");

// The verdicts, as the amended protocol states them.
/** @param {Record<string, number[]>} t @param {string} v */
const speed = (t, v) => 1 / median(t[v] ?? []);
const ratio = (/** @type {string} */ n, /** @type {string} */ x, /** @type {string} */ y) =>
  speed(warm[n] ?? {}, x) / speed(warm[n] ?? {}, y);
const verdict = (/** @type {boolean} */ holds) => (holds ? "holds" : "refuted");
const same = Object.fromEntries(
  Object.entries(summaries).map(([n, s]) => [n, new Set(Object.values(s)).size === 1]),
);
const memMedian = (/** @type {string} */ n, /** @type {string} */ v) =>
  median(memory[n]?.[v] ?? []);
const h = {
  H1: {
    1000000: ratio("1000000", "C", "A"),
    4000000: ratio("4000000", "C", "A"),
  },
  H2: {
    1000000: ratio("1000000", "C", "B"),
    4000000: ratio("4000000", "C", "B"),
  },
  H3: memMedian("4000000", "C") / memMedian("4000000", "A"),
  H4: (shipped.C ?? 0) / (shipped.A ?? 1),
  H6: median(warm["4000000"]?.C ?? []) / median(warm["1000000"]?.C ?? []),
  H7: (code.C ?? 0) / (code.A ?? 1),
};
const verdicts = {
  H1: verdict(h.H1["1000000"] >= 2 && h.H1["4000000"] >= 2),
  H2: verdict(h.H2["1000000"] >= 0.8 && h.H2["4000000"] >= 0.8),
  H3: verdict(h.H3 <= 0.5),
  H4: verdict(h.H4 <= 3),
  H5: parallelRefusal === undefined ? "not measured" : "refuted",
  H6: verdict(h.H6 <= 4.4),
  H7: verdict(h.H7 <= 1.25),
};

const commit = spawnSync("git", ["rev-parse", "--short=7", "HEAD"], {
  encoding: "utf8",
}).stdout.trim();
const results = {
  commit,
  machine: cpus()[0]?.model ?? "unknown",
  cores: cpus().length,
  files,
  summaries,
  same,
  warmMs: warm,
  coldMs,
  memoryBytes: memory,
  shipped,
  code,
  energy: rapl ? "RAPL present" : "not measured: no RAPL on this machine",
  parallel: parallelRefusal ?? "built",
  ratios: h,
  verdicts,
};
writeFileSync(join(out, "results.json"), `${JSON.stringify(results, null, 2)}\n`);

const f1 = (/** @type {number} */ x) => x.toFixed(1);
const f2 = (/** @type {number} */ x) => x.toFixed(2);
const mb = (/** @type {number} */ x) => `${(x / 1048576).toFixed(1)} MiB`;
const names = { A: "A. TypeScript, idiomatic", B: "B. TypeScript, optimized", C: "C. Peras" };
const report = [
  "# Data at scale — results",
  "",
  `Generated by \`node scripts/bench-data.mjs\` under the protocol of [decision 0014](../../docs/en/decisions/0014-data-at-scale.md), registered before these measures and amended before any. Compiler at commit \`${commit}\`. Machine: ${results.machine}, ${String(results.cores)} cores. Every number is also in [results.json](results.json).`,
  "",
  "## Verdicts",
  "",
  "| Hypothesis | Verdict | Measured |",
  "| --- | --- | --- |",
  `| H1 | ${verdicts.H1} | C at ${f2(h.H1["1000000"])} times A's speed at 1,000,000 rows, ${f2(h.H1["4000000"])} at 4,000,000 |`,
  `| H2 | ${verdicts.H2} | C at ${f2(h.H2["1000000"])} times B's speed at 1,000,000 rows, ${f2(h.H2["4000000"])} at 4,000,000 |`,
  `| H3 | ${verdicts.H3} | C's peak memory ${f2(h.H3)} times A's at 4,000,000 rows |`,
  `| H4 | ${verdicts.H4} | C ships ${String(shipped.C)} bytes, ${f2(h.H4)} times A's ${String(shipped.A)} |`,
  `| H5 | ${verdicts.H5} | ${parallelRefusal === undefined ? "D built" : `D cannot be written: \`${parallelRefusal}\``} |`,
  `| H6 | ${verdicts.H6} | C's time at 4,000,000 rows ${f2(h.H6)} times its time at 1,000,000 |`,
  `| H7 | ${verdicts.H7} | C counts ${String(code.C)} lines, ${f2(h.H7)} times A's ${String(code.A)} |`,
  "",
  "## The data",
  "",
  "| Rows | Bytes | SHA-256 |",
  "| --: | --: | --- |",
  ...sizes.map(
    (n) =>
      `| ${n.toLocaleString("en")} | ${String(files[String(n)]?.bytes)} | \`${String(files[String(n)]?.sha256).slice(0, 16)}…\` |`,
  ),
  "",
  "## Same summary",
  "",
  "| Rows | Same in A, B and C | SHA-256 of the summary |",
  "| --: | --- | --- |",
  ...sizes.map(
    (n) =>
      `| ${n.toLocaleString("en")} | ${same[String(n)] === true ? "yes" : "**no**"} | \`${String(summaries[String(n)]?.A).slice(0, 16)}…\` |`,
  ),
  "",
  "## Time to the summary, warm",
  "",
  "Milliseconds from the file's bytes in memory to the summary, in Chromium, medians of 15 runs after 2 of warm-up, the versions alternating, a collection before each run.",
  "",
  "| Version | " + sizes.map((n) => n.toLocaleString("en")).join(" | ") + " |",
  "| --- | " + sizes.map(() => "--:").join(" | ") + " |",
  ...Object.entries(names).map(
    ([v, name]) =>
      `| ${name} | ${sizes.map((n) => f1(median(warm[String(n)]?.[v] ?? []))).join(" | ")} |`,
  ),
  "",
  "## Time to the first chart, cold",
  "",
  "Milliseconds from the start of navigation to the chart painted, a new browser context per load, the program and the data from a local server; medians of 7 loads. Tested against no hypothesis.",
  "",
  "| Version | " +
    sizes.map((n) => n.toLocaleString("en")).join(" | ") +
    ` | ${Number(small).toLocaleString("en")}, fast 4G (exploratory, 3 loads) |`,
  "| --- | " + sizes.map(() => "--:").join(" | ") + " | --: |",
  ...Object.entries(names).map(
    ([v, name]) =>
      `| ${name} | ${sizes.map((n) => f1(median(coldMs[String(n)]?.[v] ?? []))).join(" | ")} | ${f1(median(coldMs.throttled100000[v] ?? []))} |`,
  ),
  "",
  "## Memory",
  "",
  "The peak resident memory of the renderer process (VmHWM) added by the query, over its peak with the version loaded and the data in memory; a new browser per run, medians of 5.",
  "",
  "| Version | " + sizes.map((n) => n.toLocaleString("en")).join(" | ") + " |",
  "| --- | " + sizes.map(() => "--:").join(" | ") + " |",
  ...Object.entries(names).map(
    ([v, name]) => `| ${name} | ${sizes.map((n) => mb(memMedian(String(n), v))).join(" | ")} |`,
  ),
  "",
  "## The programs",
  "",
  "| Version | Bytes shipped, minified and gzip | Lines of code |",
  "| --- | --: | --: |",
  ...Object.entries(names).map(
    ([v, name]) => `| ${name} | ${String(shipped[v])} | ${String(code[v])} |`,
  ),
  "",
  `Energy: ${results.energy}.`,
  "",
];
writeFileSync(join(out, "RESULTS.md"), report.join("\n"));
console.log(report.slice(0, 16).join("\n"));
