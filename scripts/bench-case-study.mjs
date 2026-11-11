// The case study of decision 0011: a table of orders written in TypeScript
// (bench/case-study/orders.ts) and in a managed Peras module
// (bench/case-study/orders.apeiron). In Chromium, for each: the rows and the
// summary shown agree, the time to mount the table, to search and to sort,
// and that nothing reacts once it is removed. Also the size of each module.
// Usage: node scripts/bench-case-study.mjs [--check] [--perasc <path>]
//        --check: one run on 1,000 orders; fails if the two versions do not
//        show the same rows and text, if one still reacts once removed, if two
//        tables in one page share their state, or if the Peras version ships
//        more bytes than the budget of decision 0012.
// Writes bench/case-study/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";
import { minify } from "vite";
import ts from "typescript";

const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "case-study");
const check = process.argv.includes("--check");
/** The bytes of the Peras version as shipped, minified and gzip, that decision
 * 0012 allows: a change that needs more raises this budget there, with its reason. */
const shippedBudget = 1600;
const at = process.argv.indexOf("--perasc");
const perasc = at > 0 ? (process.argv[at + 1] ?? "") : join(root, "target", "release", "perasc");
const work = join(root, "target", "bench-case-study");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

const tsSource = readFileSync(join(dir, "orders.ts"), "utf8");
const tsJs = ts.transpileModule(tsSource, {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
}).outputText;
writeFileSync(join(work, "ts.js"), tsJs);
for (const name of ["orders", "orders-extern"]) {
  const built = spawnSync(
    perasc,
    [
      "build",
      join(dir, `${name}.apeiron`),
      "--out",
      work,
      "--allocator",
      "tlsf",
      "--imports",
      "source",
    ],
    { encoding: "utf8" },
  );
  if (built.status !== 0) throw new Error(`perasc build ${name}\n${built.stderr}`);
}
writeFileSync(join(work, "sort.js"), readFileSync(join(dir, "sort.js"), "utf8"));
writeFileSync(join(work, "page.js"), readFileSync(join(dir, "page.js"), "utf8"));
const perasJs = readFileSync(join(work, "orders.js"), "utf8");

const page = `<!doctype html><meta charset="utf-8"><title>case study</title><link rel="icon" href="data:,"><main id="app"></main>`;
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  if (path === "/") {
    response.writeHead(200, { "content-type": "text/html" });
    response.end(page);
    return;
  }
  try {
    response.writeHead(200, { "content-type": "text/javascript" });
    response.end(readFileSync(join(work, path)));
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
/**
 * The energy the processor package used so far, in microjoules, where Linux
 * gives it (RAPL); `undefined` elsewhere, as in most virtual machines.
 * @returns {number | undefined}
 */
function energy() {
  try {
    return Number(readFileSync("/sys/class/powercap/intel-rapl:0/energy_uj", "utf8"));
  } catch {
    return undefined;
  }
}
const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });

/**
 * Mounts one version over `n` orders in a fresh page, then searches, sorts
 * and removes it; times each step and reads what the page shows.
 * @param {string} file
 * @param {number} n
 */
async function measure(file, n) {
  const tab = await browser.newPage();
  const cdp = await tab.context().newCDPSession(tab);
  await cdp.send("Performance.enable");
  // The heap after a collection, for page.js.
  await tab.exposeBinding("__heap", async () => {
    await cdp.send("HeapProfiler.collectGarbage");
    const { usedSize } = /** @type {{ usedSize: number }} */ (
      await cdp.send("Runtime.getHeapUsage")
    );
    return usedSize;
  });
  await tab.goto(`http://localhost:${String(port)}/`);
  const energyBefore = energy();
  /** @type {unknown} */
  const outcome = await tab.evaluate(
    `import("/page.js").then((p) => p.measure("/${file}", ${String(n)}))`,
  );
  const result =
    /** @type {{ mount: number, sort: number, shown: number, firstSorted: string, summary: string, left: number, reacts: boolean, twice: boolean, heapShown: number, heapLeft: number }} */ (
      outcome
    );
  // The time the page's tasks kept a core busy: scripts, layout, style.
  const { metrics } = /** @type {{ metrics: { name: string, value: number }[] }} */ (
    await cdp.send("Performance.getMetrics")
  );
  const cpu = (metrics.find((m) => m.name === "TaskDuration")?.value ?? 0) * 1000;
  const energyAfter = energy();
  await tab.close();
  return {
    ...result,
    cpu,
    joules:
      energyBefore === undefined || energyAfter === undefined
        ? undefined
        : (energyAfter - energyBefore) / 1e6,
  };
}

const sizes = check ? [1000] : [1000, 10000];
const runs = check ? 1 : 5;
/** @type {string[]} */
const problems = [];
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;
const rows = [];
/** @type {string[]} */
const costs = [];
/** @type {string[]} */
const checks = [];
for (const n of sizes) {
  let firstShown = -1;
  /** @type {Record<string, { mount: number[], sort: number[], cpu: number[], shown: number[], left: number[], joules: number[] }>} */
  const times = {
    ts: { mount: [], sort: [], cpu: [], shown: [], left: [], joules: [] },
    peras: { mount: [], sort: [], cpu: [], shown: [], left: [], joules: [] },
    extern: { mount: [], sort: [], cpu: [], shown: [], left: [], joules: [] },
  };
  let firstText = "";
  let summaryText = "";
  for (let r = 0; r < runs; r++) {
    for (const [side, file] of /** @type {const} */ ([
      ["ts", "ts.js"],
      ["peras", "orders.js"],
      ["extern", "orders-extern.js"],
    ])) {
      const o = await measure(file, n);
      if (o.reacts) problems.push(`${side}: still reacts once removed`);
      if (!o.twice) problems.push(`${side}: two tables in one page share their state`);
      if (side !== "ts" && o.shown !== firstShown)
        problems.push(`${side}: ${String(o.shown)} rows, TypeScript ${String(firstShown)}`);
      // The same text as TypeScript's, to the character: numbers included.
      if (side !== "ts" && (o.firstSorted !== firstText || o.summary !== summaryText))
        problems.push(
          `${side}: shows \`${o.firstSorted}\` and \`${o.summary}\`, TypeScript \`${firstText}\` and \`${summaryText}\``,
        );
      if (side === "ts") {
        firstShown = o.shown;
        firstText = o.firstSorted;
        summaryText = o.summary;
      }
      const t = times[side];
      t?.mount.push(o.mount);
      t?.sort.push(o.sort);
      t?.cpu.push(o.cpu);
      t?.shown.push(o.heapShown / 1024);
      t?.left.push(o.heapLeft / 1024);
      if (o.joules !== undefined) t?.joules.push(o.joules);
      if (r === 0) {
        checks.push(
          `- ${side}, ${String(n)} orders: ${String(o.shown)} rows for "ada", first sorted \`${o.firstSorted}\`, summary \`${o.summary}\`; once removed, ${o.left === 0 ? "the list is emptied" : `${String(o.left)} rows stay in the page`}, and ${o.reacts ? "it still reacts" : "nothing reacts"}; two tables in one page ${o.twice ? "keep their own state" : "share their state"}.`,
        );
      }
    }
  }
  // What each version costs the machine, for the whole scenario.
  const cost = (
    /** @type {"cpu" | "shown" | "left" | "joules"} */ k,
    /** @type {number} */ digits,
  ) =>
    ["ts", "peras", "extern"].map((s) => median(times[s]?.[k] ?? []).toFixed(digits)).join(" | ");
  costs.push(`| ${String(n)} | CPU time of the page's tasks (ms) | ${cost("cpu", 1)} |`);
  costs.push(`| ${String(n)} | JavaScript heap held while shown (KiB) | ${cost("shown", 0)} |`);
  costs.push(`| ${String(n)} | JavaScript heap left once removed (KiB) | ${cost("left", 0)} |`);
  if ((times.ts?.joules.length ?? 0) > 0)
    costs.push(`| ${String(n)} | energy of the processor, RAPL (J) | ${cost("joules", 2)} |`);
  for (const step of /** @type {const} */ (["mount", "sort"])) {
    const t = median(times.ts?.[step] ?? []);
    const p = median(times.peras?.[step] ?? []);
    const x = median(times.extern?.[step] ?? []);
    rows.push(
      `| ${String(n)} | ${step} | ${t.toFixed(1)} | ${p.toFixed(1)} (${(t / p).toFixed(2)}) | ${x.toFixed(1)} (${(t / x).toFixed(2)}) |`,
    );
  }
}
await browser.close();
server.close();

const gz = (/** @type {string} */ s) => gzipSync(s).length;
const lines = (/** @type {string} */ s) =>
  s.split("\n").filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;
const apeiron = readFileSync(join(dir, "orders.apeiron"), "utf8");
const report = [
  "# Case study — results",
  "",
  "Generated by `node scripts/bench-case-study.mjs`: the table of orders of [orders.ts](orders.ts) and [orders.apeiron](orders.apeiron), in Chromium, medians of " +
    `${String(runs)} fresh pages. Machine: ${cpus()[0]?.model ?? "?"}. Speed: TypeScript's time divided by Peras's.`,
  "",
  "| Orders | Step | TypeScript (ms) | Peras, `sortBy` (ms, speed) | Peras, sort through `extern` (ms, speed) |",
  "| --: | --- | --: | --: | --: |",
  ...rows,
  "",
  "| | TypeScript | Peras |",
  "| --- | --: | --: |",
  `| lines of code, without comments and blank lines | ${String(lines(tsSource))} | ${String(lines(apeiron))} |`,
  `| JavaScript, gzip (bytes) | ${String(gz(tsJs))} | ${String(gz(perasJs))} |`,
  `| JavaScript as shipped: minified by Vite, then gzip (bytes) | ${String(gz((await minify("ts.js", tsJs)).code))} | ${String(gz((await minify("orders.js", perasJs)).code))} |`,
  "",
  "What each costs the machine, for the whole scenario of one page, medians: the time the page's tasks kept a core busy (Chromium's `TaskDuration`, the closest measure of the energy spent that a browser gives), and the JavaScript heap, measured after a collection, while the table is shown and once it is removed. " +
    (costs.some((c) => c.includes("RAPL"))
      ? "The energy is the processor package's, read from Linux (RAPL)."
      : "The energy itself is not measured: this machine does not give the processor's counters (RAPL)."),
  "",
  "| Orders | Measure | TypeScript | Peras | Peras, sort through `extern` |",
  "| --: | --- | --: | --: | --: |",
  ...costs,
  "",
  "What each shows:",
  "",
  ...checks,
  "",
].join("\n");
if (check) {
  const shipped = gz((await minify("orders.js", perasJs)).code);
  if (shipped > shippedBudget)
    problems.push(
      `the Peras version ships ${String(shipped)} bytes, minified and gzip, above the budget of ${String(shippedBudget)} of decision 0012`,
    );
  if (problems.length > 0) throw new Error(problems.join("\n"));
  console.log(
    "The case study shows the same rows and text in TypeScript and in Peras, nothing reacts once removed, two tables keep their own state, and it ships within its budget.",
  );
} else {
  writeFileSync(join(dir, "RESULTS.md"), report);
  console.log(report);
}
