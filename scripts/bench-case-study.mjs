// The case study of decision 0011: a table of orders written in TypeScript
// (bench/case-study/orders.ts) and in a managed Peras module
// (bench/case-study/orders.apeiron). In Chromium, for each: the rows and the
// summary shown agree, the time to mount the table, to search and to sort,
// and that nothing reacts once it is removed. Also the size of each module.
// Usage: node scripts/bench-case-study.mjs [--check] [--perasc <path>]
//        --check: one run on 1,000 orders; fails if the two versions do not
//        show the same rows, or if one still reacts once removed.
// Writes bench/case-study/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";
import ts from "typescript";

const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "case-study");
const check = process.argv.includes("--check");
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
  await tab.goto(`http://localhost:${String(port)}/`);
  /** @type {unknown} */
  const outcome = await tab.evaluate(
    `import("/page.js").then((p) => p.measure("/${file}", ${String(n)}))`,
  );
  const result =
    /** @type {{ mount: number, sort: number, shown: number, firstSorted: string, summary: string, left: number, reacts: boolean }} */ (
      outcome
    );
  await tab.close();
  return result;
}

const sizes = check ? [1000] : [1000, 10000];
const runs = check ? 1 : 5;
/** @type {string[]} */
const problems = [];
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;
const rows = [];
/** @type {string[]} */
const checks = [];
for (const n of sizes) {
  let firstShown = -1;
  /** @type {Record<string, { mount: number[], sort: number[] }>} */
  const times = {
    ts: { mount: [], sort: [] },
    peras: { mount: [], sort: [] },
    extern: { mount: [], sort: [] },
  };
  for (let r = 0; r < runs; r++) {
    for (const [side, file] of /** @type {const} */ ([
      ["ts", "ts.js"],
      ["peras", "orders.js"],
      ["extern", "orders-extern.js"],
    ])) {
      const o = await measure(file, n);
      if (o.reacts) problems.push(`${side}: still reacts once removed`);
      if (side !== "ts" && o.shown !== firstShown)
        problems.push(`${side}: ${String(o.shown)} rows, TypeScript ${String(firstShown)}`);
      if (side === "ts") firstShown = o.shown;
      times[side]?.mount.push(o.mount);
      times[side]?.sort.push(o.sort);
      if (r === 0) {
        checks.push(
          `- ${side}, ${String(n)} orders: ${String(o.shown)} rows for "ada", first sorted \`${o.firstSorted}\`, summary \`${o.summary}\`; once removed, ${o.left === 0 ? "the list is emptied" : `${String(o.left)} rows stay in the page`}, and ${o.reacts ? "it still reacts" : "nothing reacts"}.`,
        );
      }
    }
  }
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
  "| Orders | Step | TypeScript (ms) | Peras, insertion sort (ms, speed) | Peras, sort through `extern` (ms, speed) |",
  "| --: | --- | --: | --: | --: |",
  ...rows,
  "",
  "| | TypeScript | Peras |",
  "| --- | --: | --: |",
  `| lines of code, without comments and blank lines | ${String(lines(tsSource))} | ${String(lines(apeiron))} |`,
  `| JavaScript, gzip (bytes) | ${String(gz(tsJs))} | ${String(gz(perasJs))} |`,
  "",
  "What each shows:",
  "",
  ...checks,
  "",
].join("\n");
if (check) {
  if (problems.length > 0) throw new Error(problems.join("\n"));
  console.log(
    "The case study shows the same rows in TypeScript and in Peras, and nothing reacts once removed.",
  );
} else {
  writeFileSync(join(dir, "RESULTS.md"), report);
  console.log(report);
}
