// The stream of sensors, measured (decision 0015), under its registered
// protocol. Four timed calls on the same batches of CSV text:
//   A. TypeScript, idiomatic (examples/stream/src/digest.ts, the app's own);
//   B. TypeScript, optimized by hand (bench/stream/digest-optimized.ts);
//   C. Peras, the app's module (examples/stream/src/telemetry.peras), TLSF;
//   X. the boundary alone: the text into C's kind of module and its length
//      back (bench/stream/boundary.peras), to split C's time.
// 1. Agreement: at each batch size, every batch of the pool gives the same
//    canonical digest in A, B and C, and C refuses exactly the lines the
//    generator broke on purpose.
// 2. Time per call: R independent browser processes; in each, at each batch
//    size, W rounds of warm-up then K rounds measured; in a round each of A,
//    B, C and X digests the same batch, in an order that rotates by a Latin
//    square; a cross-origin isolated page, for a 5 µs clock.
// 3. Statistics: medians per process, ratios of medians, 95 % intervals by a
//    hierarchical bootstrap (processes, then calls), 10,000 resamples, seed 15.
// 4. Figures: SVG, written from results.json.
// Usage: node scripts/bench-stream.mjs [--check | --smoke]
//        --check: agreement only, in Node, at 10,000 rows; nothing is written.
//        --smoke: every step with 2 processes and few rounds, written to
//        target/bench-stream, to try the script; not a measure.
// Writes bench/stream/RESULTS.md, results.json and docs/assets/stream-*.svg,
// unless --check or --smoke.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus, platform, release } from "node:os";
import { extname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { chromium } from "playwright";
import ts from "typescript";

const root = join(import.meta.dirname, "..");
// The commit measured, read at the start (finding M010).
const commit = spawnSync("git", ["rev-parse", "--short=7", "HEAD"], {
  cwd: root,
  encoding: "utf8",
}).stdout.trim();
const dirty =
  spawnSync("git", ["status", "--porcelain"], { cwd: root, encoding: "utf8" }).stdout.trim() !== "";
const check = process.argv.includes("--check");
const smoke = process.argv.includes("--smoke");
const perasc = join(root, "target", "release", "perasc");
const work = join(root, "target", "bench-stream");
const out = smoke ? work : join(root, "bench", "stream");
const assets = smoke ? work : join(root, "docs", "assets");
rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "js"), { recursive: true });

// ---- The protocol's numbers --------------------------------------------------

/** Rows per batch. */
const SIZES = check ? [10_000] : [1_000, 10_000, 100_000];
/** Independent browser processes. */
const PROCESSES = smoke ? 2 : 10;
/** Distinct batches per size, read in turn. */
const POOL = { 1000: 64, 10000: 32, 100000: 12 };
/** Rounds of warm-up, then rounds measured, per size and process. */
const WARMUP = smoke ? { 1000: 20, 10000: 5, 100000: 2 } : { 1000: 200, 10000: 40, 100000: 10 };
const ROUNDS = smoke ? { 1000: 40, 10000: 10, 100000: 4 } : { 1000: 800, 10000: 200, 100000: 60 };
const RESAMPLES = smoke ? 500 : 10_000;
const SEED = 15;
const VERSIONS = ["A", "B", "C", "X"];
/** A Latin square: each version first, second, third and fourth equally often. */
const ORDERS = [
  ["A", "B", "C", "X"],
  ["B", "C", "X", "A"],
  ["C", "X", "A", "B"],
  ["X", "A", "B", "C"],
];

// ---- 1. The versions ------------------------------------------------------------

/** @param {string} command @param {string[]} args */
function sh(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 28 });
  if (result.error !== undefined) throw result.error;
  return result;
}

/** @param {string} from @param {string} name */
function transpile(from, name) {
  const source = readFileSync(join(root, from), "utf8");
  const js = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  writeFileSync(join(work, "js", `${name}.js`), js.replaceAll(/\.ts(["'])/g, ".js$1"));
}
transpile("examples/stream/src/generate.ts", "generate");
transpile("examples/stream/src/digest.ts", "digest");
transpile("bench/stream/digest-optimized.ts", "optimized");
for (const [source, name] of [
  ["examples/stream/src/telemetry.peras", "c"],
  ["bench/stream/boundary.peras", "x"],
]) {
  const built = sh(perasc, [
    "build",
    join(root, source),
    "--allocator",
    "tlsf",
    "--out",
    join(work, name),
  ]);
  if (built.status !== 0) throw new Error(`perasc build ${String(source)}\n${built.stderr}`);
}

if (check) {
  /** @typedef {{ broken: number, kept: number }} DigestLike */
  /** @typedef {{ batch: (first: number, rows: number) => string, injected: (first: number, rows: number) => number }} Generator */
  /** @typedef {{ digest: (text: string) => DigestLike, canonical: (d: DigestLike) => string }} Reference */
  /** @typedef {{ digest: (text: string) => DigestLike }} Digester */
  /** @param {string} path @returns {Promise<unknown>} */
  const load = (path) => import(pathToFileURL(path).href);
  const gen = /** @type {Generator} */ (await load(join(work, "js", "generate.js")));
  const a = /** @type {Reference} */ (await load(join(work, "js", "digest.js")));
  const b = /** @type {Digester} */ (await load(join(work, "js", "optimized.js")));
  const c = /** @type {Digester} */ (await load(join(work, "c", "telemetry.js")));
  for (let k = 0; k < 8; k++) {
    const text = gen.batch(k * 10_000, 10_000);
    const want = a.canonical(a.digest(text));
    const peras = c.digest(text);
    if (a.canonical(b.digest(text)) !== want || a.canonical(peras) !== want) {
      console.error(`batch ${String(k)}: the versions disagree`);
      process.exit(1);
    }
    if (peras.broken !== gen.injected(k * 10_000, 10_000)) {
      console.error(`batch ${String(k)}: Peras refused other lines than those broken on purpose`);
      process.exit(1);
    }
  }
  console.log("The stream's digest is the same in A, B and C on 8 batches of 10,000 rows.");
  process.exit(0);
}

// ---- 2. The page and the measure -------------------------------------------------

const page = `<!doctype html><meta charset="utf-8"><title>stream</title><link rel="icon" href="data:,">`;
const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
});
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  // Cross-origin isolation gives the page a clock of 5 µs instead of 100.
  const isolation = {
    "cross-origin-opener-policy": "same-origin",
    "cross-origin-embedder-policy": "require-corp",
    "cache-control": "no-store",
  };
  if (path === "/") {
    response.writeHead(200, { ...isolation, "content-type": "text/html" });
    response.end(page);
    return;
  }
  try {
    const file = join(work, path);
    const body = readFileSync(file);
    response.writeHead(200, { ...isolation, "content-type": types[extname(file)] ?? "text/plain" });
    response.end(body);
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

/** The program each process runs, for one size: agreement, then the rounds. */
const program = (/** @type {number} */ size) => `(async () => {
  const gen = await import("/js/generate.js");
  const a = await import("/js/digest.js");
  const b = await import("/js/optimized.js");
  const c = await import("/c/telemetry.js");
  const x = await import("/x/boundary.js");
  const size = ${String(size)}, pool = ${String(POOL[/** @type {1000} */ (size)])};
  const texts = Array.from({ length: pool }, (_, k) => gen.batch(k * size, size));
  const disagree = [];
  texts.forEach((text, k) => {
    const want = a.canonical(a.digest(text));
    const peras = c.digest(text);
    if (a.canonical(b.digest(text)) !== want) disagree.push("B" + k);
    if (a.canonical(peras) !== want) disagree.push("C" + k);
    if (peras.broken !== gen.injected(k * size, size)) disagree.push("broken" + k);
  });
  const fns = { A: a.digest, B: b.digest, C: c.digest, X: x.bytes };
  const orders = ${JSON.stringify(ORDERS)};
  const warmup = ${String(WARMUP[/** @type {1000} */ (size)])};
  const rounds = ${String(ROUNDS[/** @type {1000} */ (size)])};
  const times = { A: [], B: [], C: [], X: [] };
  let sink = 0;
  for (let r = 0; r < warmup + rounds; r++) {
    const text = texts[r % pool];
    for (const v of orders[r % orders.length]) {
      const t0 = performance.now();
      const result = fns[v](text);
      const t1 = performance.now();
      sink += typeof result === "number" ? result : result.kept;
      if (r >= warmup) times[v].push(t1 - t0);
    }
  }
  // The clock's step: the smallest difference it shows.
  let step = Infinity, last = performance.now();
  for (let i = 0; i < 200000; i++) {
    const now = performance.now();
    if (now > last) { step = Math.min(step, now - last); last = now; }
  }
  return { disagree, times, sink, step, isolated: crossOriginIsolated, bytes: texts[0].length };
})()`;

/** @typedef {{ disagree: string[], times: Record<string, number[]>, sink: number, step: number, isolated: boolean, bytes: number }} Run */
/** @type {Record<string, Run[]>} */
const runs = Object.fromEntries(SIZES.map((s) => [String(s), []]));
let browserVersion = "";
for (let p = 0; p < PROCESSES; p++) {
  // A new browser process for each: the processes are the independent units.
  const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
  browserVersion = browser.version();
  const tab = await browser.newPage();
  await tab.goto(`${origin}/`);
  for (const size of SIZES) {
    console.log(`process ${String(p + 1)} of ${String(PROCESSES)}, ${String(size)} rows…`);
    const run = /** @type {Run} */ (await tab.evaluate(program(size)));
    runs[String(size)]?.push(run);
  }
  await browser.close();
}
server.close();

// ---- 3. Statistics ----------------------------------------------------------------

/** A small, seeded generator (mulberry32), so that the intervals can be redone. */
function random(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
/** @param {number[]} xs @param {number} q 0 to 1 */
function quantile(xs, q) {
  const s = [...xs].sort((u, v) => u - v);
  if (s.length === 0) return Number.NaN;
  const at = (s.length - 1) * q;
  const lo = Math.floor(at);
  const hi = Math.ceil(at);
  return (s[lo] ?? 0) + ((s[hi] ?? 0) - (s[lo] ?? 0)) * (at - lo);
}
const median = (/** @type {number[]} */ xs) => quantile(xs, 0.5);

/**
 * A hierarchical bootstrap of `statistic`, a function of each process's
 * samples: processes are drawn with replacement, then each one's calls.
 * @param {Run[]} sample
 * @param {(runs: Record<string, number[]>[]) => number} statistic
 */
function bootstrap(sample, statistic) {
  const next = random(SEED);
  const pick = (/** @type {number} */ n) => Math.floor(next() * n);
  const values = [];
  for (let i = 0; i < RESAMPLES; i++) {
    const drawn = sample.map(() => {
      const run = sample[pick(sample.length)];
      /** @type {Record<string, number[]>} */
      const times = {};
      for (const v of VERSIONS) {
        const xs = run?.times[v] ?? [];
        times[v] = xs.map(() => xs[pick(xs.length)] ?? 0);
      }
      return times;
    });
    values.push(statistic(drawn));
  }
  return [quantile(values, 0.025), quantile(values, 0.975)];
}

/** The geometric mean over processes of the ratio of two versions' medians: how many times faster `fast` is than `slow`. */
const speedup =
  (/** @type {string} */ fast, /** @type {string} */ slow) =>
  (/** @type {Record<string, number[]>[]} */ rs) =>
    Math.exp(
      rs.reduce((s, r) => s + Math.log(median(r[slow] ?? []) / median(r[fast] ?? [])), 0) /
        rs.length,
    );

/** @typedef {{ value: number, ci: number[] }} Estimate */
/** @typedef {{ calls: number, medianMs: number, processMediansMs: number[], minProcessMs: number, maxProcessMs: number, cv: number, p50Ms: number, p95Ms: number, p99Ms: number, rowsPerSecond: number, nsPerRow: number }} VersionStats */
/** @typedef {{ bytes: number, disagreements: string[], versions: Record<string, VersionStats>, speedupCoverA: Estimate, speedupCoverB: Estimate, speedupBoverA: Estimate, tailC: Estimate, boundaryShare: Estimate, clockStepMs: number, isolated: boolean }} SizeStats */
/** @type {Record<string, SizeStats>} */
const bySize = {};
/** @param {number | string} size @returns {SizeStats} */
function at(size) {
  const found = bySize[String(size)];
  if (found === undefined) throw new Error(`no results at ${String(size)} rows`);
  return found;
}
/** @param {SizeStats} stats @param {string} v @returns {VersionStats} */
function ver(stats, v) {
  const found = stats.versions[v];
  if (found === undefined) throw new Error(`no results for ${v}`);
  return found;
}
for (const size of SIZES) {
  const sample = runs[String(size)] ?? [];
  const pooled = (/** @type {string} */ v) => sample.flatMap((r) => r.times[v] ?? []);
  /** @type {Record<string, VersionStats>} */
  const versions = {};
  for (const v of VERSIONS) {
    const perProcess = sample.map((r) => median(r.times[v] ?? []));
    const all = pooled(v);
    const mean = perProcess.reduce((s, m) => s + m, 0) / perProcess.length;
    const sd = Math.sqrt(
      perProcess.reduce((s, m) => s + (m - mean) ** 2, 0) / (perProcess.length - 1),
    );
    versions[v] = {
      calls: all.length,
      medianMs: median(perProcess),
      processMediansMs: perProcess,
      minProcessMs: Math.min(...perProcess),
      maxProcessMs: Math.max(...perProcess),
      cv: sd / mean,
      p50Ms: quantile(all, 0.5),
      p95Ms: quantile(all, 0.95),
      p99Ms: quantile(all, 0.99),
      rowsPerSecond: size / (median(perProcess) / 1000),
      nsPerRow: (median(perProcess) * 1e6) / size,
    };
  }
  const tail = (/** @type {Record<string, number[]>[]} */ rs) => {
    const all = rs.flatMap((r) => r.C ?? []);
    return quantile(all, 0.99) / quantile(all, 0.5);
  };
  const share = (/** @type {Record<string, number[]>[]} */ rs) =>
    rs.reduce((s, r) => s + median(r.X ?? []) / median(r.C ?? []), 0) / rs.length;
  bySize[String(size)] = {
    bytes: sample[0]?.bytes ?? 0,
    disagreements: sample.flatMap((r) => r.disagree),
    versions,
    speedupCoverA: {
      value: speedup("C", "A")(sample.map((r) => r.times)),
      ci: bootstrap(sample, speedup("C", "A")),
    },
    speedupCoverB: {
      value: speedup("C", "B")(sample.map((r) => r.times)),
      ci: bootstrap(sample, speedup("C", "B")),
    },
    speedupBoverA: {
      value: speedup("B", "A")(sample.map((r) => r.times)),
      ci: bootstrap(sample, speedup("B", "A")),
    },
    tailC: { value: tail(sample.map((r) => r.times)), ci: bootstrap(sample, tail) },
    boundaryShare: { value: share(sample.map((r) => r.times)), ci: bootstrap(sample, share) },
    clockStepMs: Math.max(...sample.map((r) => r.step)),
    isolated: sample.every((r) => r.isolated),
  };
}

// C's throughput at the smallest batch over its throughput at the largest.
const smallest = String(SIZES[0]);
const largest = String(SIZES[SIZES.length - 1]);
const smallOverLarge = (() => {
  const s = runs[smallest] ?? [];
  const l = runs[largest] ?? [];
  const value = (/** @type {Run[]} */ a, /** @type {Run[]} */ b) =>
    Number(largest) / median(b.map((r) => median(r.times.C ?? []))) === 0
      ? 0
      : Number(smallest) /
        median(a.map((r) => median(r.times.C ?? []))) /
        (Number(largest) / median(b.map((r) => median(r.times.C ?? []))));
  // Bootstrap: processes of each size drawn independently.
  const next = random(SEED + 1);
  const draw = (/** @type {Run[]} */ xs) =>
    xs.map(() => xs[Math.floor(next() * xs.length)] ?? xs[0]);
  const values = [];
  for (let i = 0; i < RESAMPLES; i++)
    values.push(value(/** @type {Run[]} */ (draw(s)), /** @type {Run[]} */ (draw(l))));
  return { value: value(s, l), ci: [quantile(values, 0.025), quantile(values, 0.975)] };
})();

/** The verdict of a hypothesis "value ≥ threshold" (or ≤, with `atMost`), from its interval. */
function verdict(/** @type {number[]} */ ci, /** @type {number} */ threshold, atMost = false) {
  const [lo = Number.NaN, hi = Number.NaN] = ci;
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return "undetermined";
  if (atMost) return hi <= threshold ? "holds" : lo > threshold ? "refuted" : "undetermined";
  return lo >= threshold ? "holds" : hi < threshold ? "refuted" : "undetermined";
}
/** Two verdicts that must both hold. */
const both = (/** @type {string} */ u, /** @type {string} */ w) =>
  u === "refuted" || w === "refuted"
    ? "refuted"
    : u === "holds" && w === "holds"
      ? "holds"
      : "undetermined";

const s10 = at(10_000);
const s100 = at(100_000);
const anyDisagreement = SIZES.some((s) => at(s).disagreements.length > 0);
const hypotheses = {
  H1: both(verdict(s10.speedupCoverA.ci, 1.5), verdict(s100.speedupCoverA.ci, 1.5)),
  H2: both(verdict(s10.speedupCoverB.ci, 0.8), verdict(s100.speedupCoverB.ci, 0.8)),
  H3: verdict(smallOverLarge.ci, 0.75),
  H4: verdict(s100.boundaryShare.ci, 0.15, true),
  H5: verdict(s100.tailC.ci, 1.5, true),
};
if (anyDisagreement)
  for (const h of Object.keys(hypotheses)) hypotheses[/** @type {"H1"} */ (h)] = "undetermined";

const results = {
  decision: "0015",
  commit,
  dirty,
  date: new Date().toISOString(),
  smoke,
  environment: {
    cpu: cpus()[0]?.model ?? "unknown",
    cores: cpus().length,
    os: `${platform()} ${release()}`,
    node: process.version,
    chromium: browserVersion,
  },
  design: {
    sizes: SIZES,
    processes: PROCESSES,
    pool: POOL,
    warmup: WARMUP,
    rounds: ROUNDS,
    resamples: RESAMPLES,
    seed: SEED,
    orders: ORDERS,
  },
  bySize,
  smallOverLarge,
  hypotheses,
  raw: runs,
};
mkdirSync(out, { recursive: true });
writeFileSync(join(out, "results.json"), `${JSON.stringify(results, null, 1)}\n`);

// ---- 4. The report and the figures -------------------------------------------------

const f1 = (/** @type {number} */ x) => x.toFixed(1);
const f2 = (/** @type {number} */ x) => x.toFixed(2);
const ci = (/** @type {{ value: number, ci: number[] }} */ e) =>
  `${f2(e.value)} [${f2(e.ci[0] ?? 0)}, ${f2(e.ci[1] ?? 0)}]`;
/** @type {Record<string, string>} */
const names = {
  A: "A, TypeScript idiomatic",
  B: "B, TypeScript optimized",
  C: "C, Peras",
  X: "X, the boundary alone",
};
const nameOf = (/** @type {string} */ v) => names[v] ?? v;
let md = `# Decision 0015: results${smoke ? " (smoke, not a measure)" : ""}\n\n`;
md += `Measured at commit \`${commit}\`${dirty ? " (with uncommitted changes: a departure)" : ""}, ${results.date.slice(0, 10)}, under [the protocol](../../docs/en/decisions/0015-stream-measured.md). Every number below is in [results.json](results.json), with every call's time.\n\n`;
md += `- Machine: ${results.environment.cpu}, ${String(results.environment.cores)} cores, ${results.environment.os}; Chromium ${browserVersion}; Node ${process.version}.\n`;
md += `- Design: ${String(PROCESSES)} browser processes; batches of ${SIZES.map(String).join(", ")} rows; the calls in a Latin square of 4 orders; 95 % intervals by a hierarchical bootstrap of ${String(RESAMPLES)} resamples, seed ${String(SEED)}.\n`;
md += `- Clock: cross-origin isolated ${String(SIZES.every((s) => at(s).isolated))}, its step at most ${(Math.max(...SIZES.map((s) => at(s).clockStepMs)) * 1000).toFixed(1)} µs.\n`;
md += `- Agreement: ${anyDisagreement ? `**disagreements**: ${SIZES.map((s) => at(s).disagreements.join(" ")).join(" ")}` : "every batch of every pool gave the same digest in A, B and C, and C refused exactly the lines broken on purpose"}.\n\n`;
md +=
  "## Hypotheses\n\n| | Registered | Measured, 95 % interval | Verdict |\n| --- | --- | --- | --- |\n";
md += `| H1 | C at least 1.5 times as fast as A, at 10,000 and 100,000 rows | ${ci(s10.speedupCoverA)}; ${ci(s100.speedupCoverA)} | ${hypotheses.H1} |\n`;
md += `| H2 | C at least 0.8 times as fast as B, at 10,000 and 100,000 rows | ${ci(s10.speedupCoverB)}; ${ci(s100.speedupCoverB)} | ${hypotheses.H2} |\n`;
md += `| H3 | C's rows per second at 1,000 rows at least 0.75 times those at 100,000 | ${ci(smallOverLarge)} | ${hypotheses.H3} |\n`;
md += `| H4 | The boundary at most 15 % of C's time at 100,000 rows | ${ci(s100.boundaryShare)} | ${hypotheses.H4} |\n`;
md += `| H5 | C's 99th percentile at most 1.5 times its median at 100,000 rows | ${ci(s100.tailC)} | ${hypotheses.H5} |\n\n`;
md +=
  "## Time per batch\n\nThe median over processes of each process's median, in ms; rows per second from it; the spread of the process medians; the percentiles of all calls pooled.\n\n";
md +=
  "| Rows | Version | Median ms | Min–max ms | CV | p95 ms | p99 ms | Rows/s | ns/row |\n| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n";
for (const size of SIZES) {
  for (const v of VERSIONS) {
    const r = ver(at(size), v);
    md += `| ${size.toLocaleString("en")} | ${nameOf(v)} | ${f2(r.medianMs)} | ${f2(r.minProcessMs)}–${f2(r.maxProcessMs)} | ${(r.cv * 100).toFixed(1)} % | ${f2(r.p95Ms)} | ${f2(r.p99Ms)} | ${Math.round(r.rowsPerSecond).toLocaleString("en")} | ${f1(r.nsPerRow)} |\n`;
  }
}
md +=
  "\n## Ratios\n\n| Rows | C over A | C over B | B over A | C's boundary share | C's p99 / p50 |\n| ---: | ---: | ---: | ---: | ---: | ---: |\n";
for (const size of SIZES) {
  const s = at(size);
  md += `| ${size.toLocaleString("en")} | ${ci(s.speedupCoverA)} | ${ci(s.speedupCoverB)} | ${ci(s.speedupBoverA)} | ${ci(s.boundaryShare)} | ${ci(s.tailC)} |\n`;
}
md += "\n## Figures\n\n";
for (const [file, title] of [
  ["stream-throughput.svg", "Rows per second, by batch size"],
  ["stream-ratios.svg", "How many times faster C is, with 95 % intervals"],
  ["stream-latency.svg", "The distribution of the time per row at 100,000 rows"],
  ["stream-breakdown.svg", "C's time per row, split into the boundary and the digest"],
])
  md += `![${String(title)}](${smoke ? "" : "../../docs/assets/"}${String(file)})\n\n`;
writeFileSync(join(out, "RESULTS.md"), md);

// The figures: light and dark, the same marks; text in ink, colour for identity.
/** Each version's colour, on the light surface and on the dark one. */
const colors = {
  A: { light: "#eb6834", dark: "#d95926" },
  B: { light: "#1baf7a", dark: "#199e70" },
  C: { light: "#2a78d6", dark: "#3987e5" },
  X: { light: "#6250d6", dark: "#9085e9" },
};
const style = `<style>
  .bg{fill:#fcfcfb}.ink{fill:#0b0b0b}.muted{fill:#52514e}.grid{stroke:#e6e5df}.axis{stroke:#9a9890}
  .A{fill:${colors.A.light};stroke:${colors.A.light}}.B{fill:${colors.B.light};stroke:${colors.B.light}}
  .C{fill:${colors.C.light};stroke:${colors.C.light}}.X{fill:${colors.X.light};stroke:${colors.X.light}}
  .threshold{stroke:#52514e;stroke-dasharray:4 3}
  text{font-family:system-ui,sans-serif;font-size:12px;font-variant-numeric:tabular-nums}
  .title{font-size:14px;font-weight:600}
  @media (prefers-color-scheme: dark){
    .bg{fill:#1a1a19}.ink{fill:#f0efec}.muted{fill:#c3c2b7}.grid{stroke:#2c2c2a}.axis{stroke:#6b6a63}
    .A{fill:${colors.A.dark};stroke:${colors.A.dark}}.B{fill:${colors.B.dark};stroke:${colors.B.dark}}
    .C{fill:${colors.C.dark};stroke:${colors.C.dark}}.X{fill:${colors.X.dark};stroke:${colors.X.dark}}
    .threshold{stroke:#c3c2b7}
  }
</style>`;
const W = 720;
/** The note under a figure, cut into lines of at most 105 characters. */
const wrap = (/** @type {string} */ note) => {
  const lines = [""];
  for (const word of note.split(" ")) {
    const last = lines.length - 1;
    if ((lines[last] ?? "").length + word.length > 105) lines.push(word);
    else lines[last] = lines[last] === "" ? word : `${lines[last] ?? ""} ${word}`;
  }
  return lines;
};
const svg = (
  /** @type {number} */ h,
  /** @type {string} */ title,
  /** @type {string} */ body,
  /** @type {string} */ note,
) => {
  const lines = wrap(note);
  const total = h + lines.length * 16;
  const text = lines
    .map((l, i) => `<text class="muted" x="20" y="${String(h + i * 16 - 4)}">${l}</text>`)
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${String(W)} ${String(total)}" width="${String(W)}" height="${String(total)}" role="img" aria-label="${title}">${style}<rect class="bg" width="${String(W)}" height="${String(total)}" rx="8"/><text class="ink title" x="20" y="28">${title}</text>${body}${text}</svg>\n`;
};
const n0 = (/** @type {number} */ x) => String(Math.round(x * 10) / 10);
const legend = (/** @type {string[]} */ vs, /** @type {number} */ y) =>
  vs
    .map(
      (v, i) =>
        `<rect class="${v}" x="${n0(20 + i * 190)}" y="${n0(y - 9)}" width="12" height="12" rx="2"/><text class="ink" x="${n0(38 + i * 190)}" y="${n0(y + 1)}">${nameOf(v)}</text>`,
    )
    .join("");
/** Round ticks from 0 that cover `max`: a step of 1, 2, 2.5 or 5 times a power of ten. */
const niceTicks = (/** @type {number} */ max) => {
  const raw = max / 4;
  const power = 10 ** Math.floor(Math.log10(raw));
  const step = ([1, 2, 2.5, 5, 10].find((m) => m * power >= raw) ?? 10) * power;
  const count = Math.ceil(max / step - 1e-9);
  return Array.from({ length: count + 1 }, (_, i) => Math.round(i * step * 1e6) / 1e6);
};
const lastOf = (/** @type {number[]} */ xs) => xs[xs.length - 1] ?? 1;
const where = `${String(PROCESSES)} browser processes; commit ${commit}; Chromium ${browserVersion}; ${results.environment.cpu}`;

// Figure 1: rows per second by batch size; a dot per process, a bar at the median.
{
  const H = 380;
  const plot = { x: 70, y: 70, w: W - 100, h: 240 };
  const max = Math.max(
    ...SIZES.flatMap((s) =>
      ["A", "B", "C"].flatMap((v) =>
        ver(at(s), v).processMediansMs.map((/** @type {number} */ m) => s / m / 1000),
      ),
    ),
  );
  const yTicks = niceTicks(max * 1.08);
  const top = lastOf(yTicks);
  const y = (/** @type {number} */ v) => plot.y + plot.h * (1 - v / top);
  let b = legend(["A", "B", "C"], 52);
  for (const t of yTicks)
    b += `<line class="grid" x1="${n0(plot.x)}" x2="${n0(plot.x + plot.w)}" y1="${n0(y(t))}" y2="${n0(y(t))}"/><text class="muted" x="${n0(plot.x - 8)}" y="${n0(y(t) + 4)}" text-anchor="end">${n0(t)}</text>`;
  b += `<text class="muted" transform="translate(18 ${n0(plot.y + plot.h / 2)}) rotate(-90)" text-anchor="middle">million rows per second</text>`;
  const group = plot.w / SIZES.length;
  SIZES.forEach((s, gi) => {
    const gx = plot.x + gi * group;
    b += `<text class="ink" x="${n0(gx + group / 2)}" y="${n0(plot.y + plot.h + 20)}" text-anchor="middle">${s.toLocaleString("en")} rows per batch</text>`;
    ["A", "B", "C"].forEach((v, vi) => {
      const cx = gx + group * (0.25 + vi * 0.25);
      const r = ver(at(s), v);
      const m = s / r.medianMs / 1000;
      b += `<rect class="${v}" x="${n0(cx - 14)}" y="${n0(y(m))}" width="28" height="${n0(plot.y + plot.h - y(m))}" rx="3" opacity="0.35"/>`;
      b += `<line class="${v}" x1="${n0(cx - 16)}" x2="${n0(cx + 16)}" y1="${n0(y(m))}" y2="${n0(y(m))}" stroke-width="2"/>`;
      for (const pm of r.processMediansMs)
        b += `<circle class="${v}" cx="${n0(cx)}" cy="${n0(y(s / pm / 1000))}" r="2.5"/>`;
      const highest = Math.max(
        m,
        ...r.processMediansMs.map((/** @type {number} */ pm) => s / pm / 1000),
      );
      b += `<text class="ink" x="${n0(cx)}" y="${n0(y(highest) - 7)}" text-anchor="middle">${f2(m)}</text>`;
    });
  });
  writeFileSync(
    join(assets, "stream-throughput.svg"),
    svg(
      H,
      "Rows per second, by batch size (higher is faster)",
      b,
      `Bar: median over processes; dot: one process's median. ${where}.`,
    ),
  );
}

// Figure 2: the ratios with their 95 % intervals, and the registered thresholds.
{
  const rows = SIZES.flatMap((s) => [
    {
      label: `C over A, ${s.toLocaleString("en")} rows`,
      e: at(s).speedupCoverA,
      v: "A",
      threshold: 1.5,
    },
    {
      label: `C over B, ${s.toLocaleString("en")} rows`,
      e: at(s).speedupCoverB,
      v: "B",
      threshold: 0.8,
    },
  ]);
  const H = 90 + rows.length * 30 + 30;
  const plot = { x: 200, y: 60, w: W - 240 };
  const xTicks = niceTicks(Math.max(2, ...rows.map((r) => r.e.ci[1] ?? 0)) * 1.05);
  const max = lastOf(xTicks);
  const x = (/** @type {number} */ v) => plot.x + ((plot.w - 40) * v) / max;
  let b = "";
  for (const t of xTicks)
    b += `<line class="grid" x1="${n0(x(t))}" x2="${n0(x(t))}" y1="${n0(plot.y - 10)}" y2="${n0(plot.y + rows.length * 30)}"/><text class="muted" x="${n0(x(t))}" y="${n0(plot.y + rows.length * 30 + 16)}" text-anchor="middle">${n0(t)}×</text>`;
  b += `<line class="axis" x1="${n0(x(1))}" x2="${n0(x(1))}" y1="${n0(plot.y - 10)}" y2="${n0(plot.y + rows.length * 30)}" stroke-width="1.5"/>`;
  rows.forEach((r, i) => {
    const cy = plot.y + i * 30 + 10;
    b += `<text class="ink" x="${n0(plot.x - 12)}" y="${n0(cy + 4)}" text-anchor="end">${r.label}</text>`;
    b += `<line class="threshold" x1="${n0(x(r.threshold))}" x2="${n0(x(r.threshold))}" y1="${n0(cy - 10)}" y2="${n0(cy + 10)}" stroke-width="1.5"/>`;
    b += `<line class="${r.v}" x1="${n0(x(r.e.ci[0] ?? 0))}" x2="${n0(x(r.e.ci[1] ?? 0))}" y1="${n0(cy)}" y2="${n0(cy)}" stroke-width="2"/>`;
    b += `<circle class="${r.v}" cx="${n0(x(r.e.value))}" cy="${n0(cy)}" r="5"/>`;
    b += `<text class="ink" x="${n0(x(r.e.ci[1] ?? 0) + 8)}" y="${n0(cy + 4)}">${f2(r.e.value)}</text>`;
  });
  writeFileSync(
    join(assets, "stream-ratios.svg"),
    svg(
      H,
      "How many times faster Peras (C) is than A and B",
      b,
      `Dot: geometric mean over processes of the ratio of medians; line: 95 % bootstrap interval; dashes: the registered threshold (H1 1.5, H2 0.8); 1× marked.`,
    ),
  );
}

// Figure 3: the empirical distribution of every call's time per row, at the largest size.
{
  const H = 380;
  const plot = { x: 70, y: 70, w: W - 100, h: 240 };
  const s = Number(largest);
  const series = ["A", "B", "C"].map((v) => ({
    v,
    xs: (runs[largest] ?? [])
      .flatMap((r) => r.times[v] ?? [])
      .map((t) => (t * 1e6) / s)
      .sort((p, q) => p - q),
  }));
  const xTicks = niceTicks(Math.max(...series.map((d) => d.xs[d.xs.length - 1] ?? 0)));
  const max = lastOf(xTicks);
  const x = (/** @type {number} */ v) => plot.x + plot.w * Math.min(1, v / max);
  const y = (/** @type {number} */ q) => plot.y + plot.h * (1 - q);
  let b = legend(["A", "B", "C"], 52);
  for (const t of xTicks)
    b += `<line class="grid" x1="${n0(x(t))}" x2="${n0(x(t))}" y1="${n0(plot.y)}" y2="${n0(plot.y + plot.h)}"/><text class="muted" x="${n0(x(t))}" y="${n0(plot.y + plot.h + 16)}" text-anchor="middle">${n0(t)}</text>`;
  for (const q of [0, 0.25, 0.5, 0.75, 1])
    b += `<line class="grid" x1="${n0(plot.x)}" x2="${n0(plot.x + plot.w)}" y1="${n0(y(q))}" y2="${n0(y(q))}"/><text class="muted" x="${n0(plot.x - 8)}" y="${n0(y(q) + 4)}" text-anchor="end">${String(q * 100)} %</text>`;
  b += `<text class="muted" x="${n0(plot.x + plot.w / 2)}" y="${n0(plot.y + plot.h + 34)}" text-anchor="middle">nanoseconds per row (left is faster)</text>`;
  for (const d of series) {
    const path = d.xs
      .map((t, i) => `${i === 0 ? "M" : "L"}${n0(x(t))},${n0(y((i + 1) / d.xs.length))}`)
      .join("");
    b += `<path class="${d.v}" d="${path}" style="fill:none" stroke-width="2" stroke-linejoin="round"/>`;
  }
  writeFileSync(
    join(assets, "stream-latency.svg"),
    svg(
      H + 20,
      `Share of calls done within a time per row, ${s.toLocaleString("en")} rows per batch`,
      b,
      `Each curve: every measured call of every process (${String(series[0]?.xs.length ?? 0)} per version). A curve to the left is faster; a long tail to the right means slow calls.`,
    ),
  );
}

// Figure 4: C's time per row, split into the boundary and the rest.
{
  const H = 300;
  const plot = { x: 150, y: 60, w: W - 200 };
  const parts = SIZES.map((s) => {
    const stats = at(s);
    const boundary = ver(stats, "X").nsPerRow;
    return { s, boundary, digest: Math.max(0, ver(stats, "C").nsPerRow - boundary) };
  });
  const xTicks = niceTicks(Math.max(...parts.map((p) => p.boundary + p.digest)) * 1.35);
  const max = lastOf(xTicks);
  const x = (/** @type {number} */ v) => plot.x + (plot.w * v) / max;
  let b = `<rect class="X" x="20" y="43" width="12" height="12" rx="2"/><text class="ink" x="38" y="53">the boundary: the text into the module</text><rect class="C" x="330" y="43" width="12" height="12" rx="2"/><text class="ink" x="348" y="53">the digest itself</text>`;
  for (const t of xTicks)
    b += `<line class="grid" x1="${n0(x(t))}" x2="${n0(x(t))}" y1="${n0(plot.y + 10)}" y2="${n0(plot.y + parts.length * 50 + 10)}"/><text class="muted" x="${n0(x(t))}" y="${n0(plot.y + parts.length * 50 + 26)}" text-anchor="middle">${n0(t)}</text>`;
  parts.forEach((p, i) => {
    const cy = plot.y + 20 + i * 50;
    b += `<text class="ink" x="${n0(plot.x - 12)}" y="${n0(cy + 14)}" text-anchor="end">${p.s.toLocaleString("en")} rows</text>`;
    b += `<rect class="X" x="${n0(x(0))}" y="${n0(cy)}" width="${n0(x(p.boundary) - x(0))}" height="22" rx="3"/>`;
    b += `<rect class="C" x="${n0(x(p.boundary) + 2)}" y="${n0(cy)}" width="${n0(Math.max(0, x(p.boundary + p.digest) - x(p.boundary) - 2))}" height="22" rx="3"/>`;
    b += `<text class="ink" x="${n0(x(p.boundary + p.digest) + 8)}" y="${n0(cy + 15)}">${f1(p.boundary + p.digest)} ns/row, boundary ${f1((p.boundary / (p.boundary + p.digest)) * 100)} %</text>`;
  });
  b += `<text class="muted" x="${n0(plot.x + plot.w / 2)}" y="${n0(plot.y + parts.length * 50 + 44)}" text-anchor="middle">nanoseconds per row (median)</text>`;
  writeFileSync(
    join(assets, "stream-breakdown.svg"),
    svg(
      H,
      "Where Peras's time goes",
      b,
      `The boundary is X, timed alone on the same text; the digest is C minus X. ${where}.`,
    ),
  );
}

console.log(`Written: ${join(out, "RESULTS.md")}, results.json and four figures in ${assets}.`);
console.log(JSON.stringify(hypotheses));
