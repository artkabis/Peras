// A/B benchmark: two builds of Peras, compared kernel by kernel in one
// Chromium page, so that a change is decided on measures, not estimates.
//
// A and B are each `current` (target/release/perasc, the working tree),
// `ref:<git ref>` (built in a worktree under target/bench-ab) or
// `bin:<path>`, with their own perasc options. Each round is a fresh page:
// the kernels of bench/browser run with A, B and JavaScript alternating, are
// checked to give the same results and to leave nothing alive, and the
// allocations of one call of each variant are counted by its build's
// runtime. Then the size, the import time and the compile time of each build.
// The report applies the thresholds of bench/README.md and goes to
// target/bench-ab/REPORT.md.
//
// Usage: node scripts/bench-ab.mjs --a ref:HEAD --b current
//        node scripts/bench-ab.mjs --a current --b current --b-flags "--simd off"
//        options: --rounds 4 (even: each build first as often) --runs 30
//        --warm 3; --check: small inputs, one
//        run, checks only that both builds agree and the script works.
// PERAS_CHROMIUM names the Chromium to use, as for the end-to-end tests.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { URLSearchParams } from "node:url";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";

/** @typedef {import("playwright").Browser} Browser */
/** @typedef {import("../bench/browser/runner.js").Case} Case */
/** @typedef {import("../bench/browser/runner.js").Count} Count */
/** @typedef {{ cases: Case[], counts: Count[] }} Round */
/** @typedef {{ name: string, spec: string, flags: string[], perasc: string }} Side */

const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "browser");
const work = join(root, "target", "bench-ab");

/** The thresholds of bench/README.md. */
const GAIN = -0.1;
const REGRESSION = 0.03;
const MAX_GZIP = 1024;
const MAX_COMPILE = 0.1;

/**
 * The value after `--name`, or `fallback`.
 * @param {string} name
 * @param {string} fallback
 */
function option(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at >= 0 ? (process.argv[at + 1] ?? fallback) : fallback;
}

const check = process.argv.includes("--check");
const rounds = check ? 1 : Number(option("rounds", "4"));
const config = check
  ? { scale: 0.01, runs: 1, warm: 0 }
  : { scale: 1, runs: Number(option("runs", "30")), warm: Number(option("warm", "3")) };

/**
 * Runs a command, failing loudly, and returns its standard output.
 * @param {string} command
 * @param {string[]} args
 * @param {{ cwd?: string, env?: NodeJS.ProcessEnv }} [where]
 */
function run(command, args, where = {}) {
  const result = spawnSync(command, args, {
    cwd: where.cwd ?? root,
    env: where.env ?? process.env,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  return result.stdout;
}

/**
 * The perasc binary a spec names, built when needed.
 * @param {string} spec
 */
function perascOf(spec) {
  if (spec === "current") {
    run("cargo", ["build", "--release", "-q", "-p", "perasc"]);
    return join(root, "target", "release", "perasc");
  }
  if (spec.startsWith("bin:")) return spec.slice(4);
  if (spec.startsWith("ref:")) {
    const sha = run("git", ["rev-parse", spec.slice(4)]).trim();
    const binary = join(work, "bin", sha, "perasc");
    if (existsSync(binary)) return binary;
    const tree = join(work, "src", sha);
    if (!existsSync(tree)) run("git", ["worktree", "add", "--detach", tree, sha]);
    const target = join(work, "cargo");
    run("cargo", ["build", "--release", "-q", "-p", "perasc"], {
      cwd: tree,
      env: { ...process.env, CARGO_TARGET_DIR: target },
    });
    mkdirSync(join(work, "bin", sha), { recursive: true });
    copyFileSync(join(target, "release", "perasc"), binary);
    return binary;
  }
  throw new Error(`--a and --b take current, ref:<git ref> or bin:<path>, not ${spec}`);
}

/**
 * Builds the kernels of a side into its folder; its loader also exports its
 * runtime, here only, so that the page counts allocations.
 * @param {Side} side
 */
function build(side) {
  const out = join(work, "page", side.name);
  mkdirSync(out, { recursive: true });
  const args = ["build", join(bench, "kernels.peras"), "--allocator", "tlsf", "--out", out];
  run(side.perasc, [...args, ...side.flags]);
  const loader = join(out, "kernels.js");
  writeFileSync(loader, `${readFileSync(loader, "utf8")}export const __perasRuntime = $runtime;\n`);
}

/**
 * Median wall-clock milliseconds of building the kernels, `samples` times.
 * @param {Side} side
 * @param {number} samples
 */
function compileTime(side, samples) {
  const out = join(work, "compile", side.name);
  const args = ["build", join(bench, "kernels.peras"), "--allocator", "tlsf", "--out", out];
  /** @type {number[]} */
  const times = [];
  for (let i = 0; i < samples; i++) {
    const t0 = performance.now();
    run(side.perasc, [...args, ...side.flags]);
    times.push(performance.now() - t0);
  }
  return median(times);
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;

/**
 * Raw and gzip bytes of a side's loader and module.
 * @param {string} name
 */
function size(name) {
  const bytes = ["kernels.js", "kernels.wasm"].map((f) =>
    readFileSync(join(work, "page", name, f)),
  );
  return {
    raw: bytes.reduce((n, b) => n + b.length, 0),
    gzip: bytes.reduce((n, b) => n + gzipSync(b, { level: 9 }).length, 0),
  };
}

/** @type {Record<string, string>} */
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };

/**
 * Serves the page folder on a free port of 127.0.0.1.
 * @returns {Promise<{ server: import("node:http").Server, base: string }>}
 */
function serve() {
  const folder = join(work, "page");
  const server = createServer((request, response) => {
    const path = join(folder, new URL(request.url ?? "/", "http://x").pathname);
    if (!path.startsWith(folder)) {
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
 * One round: a fresh page runs every kernel with A, B and JavaScript.
 * The build that runs first takes the first, coldest call of each kernel:
 * rounds alternate it.
 * @param {Browser} browser
 * @param {string} base
 * @param {string} first
 * @returns {Promise<Round>}
 */
async function round(browser, base, first) {
  const context = await browser.newContext();
  const page = await context.newPage();
  /** @type {(r: Round) => void} */
  let done = () => undefined;
  /** @type {(e: Error) => void} */
  let failed = () => undefined;
  /** @type {Promise<Round>} */
  const ran = new Promise((resolve, reject) => {
    done = resolve;
    failed = reject;
  });
  await page.exposeFunction(
    "report",
    /** @param {Round} r */ (r) => {
      done(r);
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
    first,
  });
  await page.goto(`${base}/index.html?${query.toString()}`);
  const result = await ran;
  await context.close();
  return result;
}

/**
 * Median time to import a side's loader in a fresh context, nothing cached.
 * @param {Browser} browser
 * @param {string} base
 * @param {string} name
 * @param {number} samples
 */
async function importTime(browser, base, name, samples) {
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
      }, `${base}/${name}/kernels.js`),
    );
    await context.close();
  }
  return median(times);
}

/** @param {number} x */
const ms = (x) => (x >= 100 ? x.toFixed(0) : x >= 10 ? x.toFixed(1) : x.toFixed(2));
/** @param {number} x */
const pct = (x) => `${x >= 0 ? "+" : ""}${(x * 100).toFixed(1)} %`;

/**
 * The report, with a verdict on each threshold.
 * @param {Side[]} sides
 * @param {Round[]} results
 * @param {{ size: { raw: number, gzip: number }, imports: number, compile: number }[]} builds
 * @param {string} chromiumVersion
 */
function report(sides, results, builds, chromiumVersion) {
  const [a, b] = sides;
  const [ba, bb] = builds;
  if (a === undefined || b === undefined || ba === undefined || bb === undefined) return "";
  const lines = [
    "# A/B benchmark",
    "",
    `A: \`${a.spec}${a.flags.length > 0 ? ` ${a.flags.join(" ")}` : ""}\` — B: \`${b.spec}${b.flags.length > 0 ? ` ${b.flags.join(" ")}` : ""}\``,
    "",
    `${cpus()[0]?.model ?? "?"}, ${String(cpus().length)} cores, Chromium ${chromiumVersion}. ${String(results.length)} rounds, each a fresh page; in each, ${String(config.runs)} timed calls after ${String(config.warm)} of warm-up, A, B and JavaScript alternating, A first in odd rounds and B in even ones. Cold: the first call, median of the rounds. Every variant gave the same result and left nothing alive.`,
    "",
    "## Kernels",
    "",
    "Medians in ms. Change: B against A, in each round; a gain counts when it is 10 % or more in every round, a regression when it is 3 % or more in every round.",
    "",
    "| Kernel | Variant | JavaScript | A | B | Change, by round | Cold A / B | Allocations A / B | Verdict |",
    "| --- | --- | --: | --: | --: | --- | --: | --: | --- |",
  ];
  /** @type {string[]} */
  const gains = [];
  /** @type {string[]} */
  const regressions = [];
  let moreAllocations = false;
  const first = results[0];
  for (const c of first?.cases ?? []) {
    const labels = c.result.filter((r) => r.label.startsWith("A: ")).map((r) => r.label.slice(3));
    for (const label of labels) {
      /** @param {Round} r @param {string} who */
      const timing = (r, who) =>
        r.cases.find((x) => x.name === c.name)?.result.find((x) => x.label === `${who}: ${label}`);
      const changes = results.map((r) => {
        const ta = timing(r, "A");
        const tb = timing(r, "B");
        return ta && tb ? tb.median / ta.median - 1 : 0;
      });
      const medA = median(results.map((r) => timing(r, "A")?.median ?? 0));
      const medB = median(results.map((r) => timing(r, "B")?.median ?? 0));
      const javascript = median(
        results.map((r) => r.cases.find((x) => x.name === c.name)?.result[0]?.median ?? 0),
      );
      const ca = median(results.map((r) => timing(r, "A")?.cold ?? 0));
      const cb = median(results.map((r) => timing(r, "B")?.cold ?? 0));
      const count = (/** @type {string} */ who) =>
        first?.counts.find((x) => x.build === who && x.label === `${c.name}, ${label}`)
          ?.allocations ?? 0;
      const [na, nb] = [count("A"), count("B")];
      if (nb > na) moreAllocations = true;
      const name = `${c.name}, ${label}`;
      let verdict = "within noise";
      if (changes.every((x) => x <= GAIN)) {
        verdict = "**gain**";
        gains.push(`${name} (${pct(median(changes))})`);
      } else if (changes.every((x) => x >= REGRESSION)) {
        verdict = "**regression**";
        regressions.push(`${name} (${pct(median(changes))})`);
      }
      lines.push(
        `| ${c.name} | ${label} | ${ms(javascript)} | ${ms(medA)} | ${ms(medB)} | ${changes.map(pct).join(", ")} | ${ms(ca)} / ${ms(cb)} | ${String(na)} / ${String(nb)} | ${verdict} |`,
      );
    }
  }
  const gzip = bb.size.gzip - ba.size.gzip;
  const compile = bb.compile / ba.compile - 1;
  lines.push(
    "",
    "## Builds",
    "",
    "| | Raw bytes | gzip bytes | Import (ms) | Compile (ms) |",
    "| --- | --: | --: | --: | --: |",
    `| A | ${String(ba.size.raw)} | ${String(ba.size.gzip)} | ${ms(ba.imports)} | ${ms(ba.compile)} |`,
    `| B | ${String(bb.size.raw)} | ${String(bb.size.gzip)} | ${ms(bb.imports)} | ${ms(bb.compile)} |`,
    "",
    "## Verdict",
    "",
    `- Gains of 10 % or more in every round: ${gains.length > 0 ? gains.join("; ") : "none"}.`,
    `- Regressions of 3 % or more in every round: ${regressions.length > 0 ? regressions.join("; ") : "none"}.`,
    `- Allocations: ${moreAllocations ? "**B allocates more**" : "B allocates no more than A"}.`,
    `- Download: ${gzip >= 0 ? "+" : ""}${String(gzip)} bytes gzip, ${gzip <= MAX_GZIP ? "within" : "**over**"} the +1 KiB threshold.`,
    `- Compile time: ${pct(compile)}, ${compile <= MAX_COMPILE ? "within" : "**over**"} the +10 % threshold.`,
    "",
  );
  return lines.join("\n");
}

/** @param {string} which */
const flagsOf = (which) => option(`${which}-flags`, "").split(" ").filter(Boolean);
/** @type {Side[]} */
const sides = ["a", "b"].map((which) => {
  const spec = option(which, which === "a" ? "ref:HEAD" : "current");
  return { name: which.toUpperCase(), spec, flags: flagsOf(which), perasc: perascOf(spec) };
});

rmSync(join(work, "page"), { recursive: true, force: true });
mkdirSync(join(work, "page"), { recursive: true });
for (const side of sides) build(side);
for (const file of ["reference.js", "runner.js"])
  copyFileSync(join(bench, file), join(work, "page", file));
const head = "<!doctype html><meta charset=utf-8><title>bench a/b</title>\n";
writeFileSync(join(work, "page", "blank.html"), head);
writeFileSync(
  join(work, "page", "index.html"),
  `${head}<script type="module">
import { ab } from "./runner.js";
import * as a from "./A/kernels.js";
import * as b from "./B/kernels.js";
const q = new URLSearchParams(location.search);
try {
  const builds = [
    { name: "A", peras: a, runtime: a.__perasRuntime },
    { name: "B", peras: b, runtime: b.__perasRuntime },
  ];
  if (q.get("first") === "B") builds.reverse();
  await report(ab(builds, { scale: Number(q.get("scale")), runs: Number(q.get("runs")), warm: Number(q.get("warm")) }));
} catch (e) {
  await fail(e instanceof Error ? e.stack ?? e.message : String(e));
}
</script>
`,
);

const { server, base } = await serve();
const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
try {
  /** @type {Round[]} */
  const results = [];
  for (let r = 0; r < rounds; r++)
    results.push(await round(browser, base, r % 2 === 0 ? "A" : "B"));
  const samples = check ? 1 : 15;
  const builds = [];
  for (const side of sides) {
    builds.push({
      size: size(side.name),
      imports: await importTime(browser, base, side.name, samples),
      compile: compileTime(side, check ? 1 : 20),
    });
  }
  if (check) {
    console.log(`A and B agree on every kernel in Chromium ${browser.version()}.`);
  } else {
    const markdown = report(sides, results, builds, browser.version());
    writeFileSync(join(work, "REPORT.md"), markdown);
    console.log(markdown);
  }
} finally {
  await browser.close();
  server.close();
}
