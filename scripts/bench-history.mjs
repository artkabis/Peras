// The history of the compiler: the same programs, as written today, compiled
// by milestones of perasc from the first that emitted WebAssembly to the
// current one, and run in Chromium against TypeScript.
//   1. Each milestone is taken from git, built (cached in target/history),
//      and asked to compile the kernels of bench/backends as written on the
//      first day, bench/garden/garden.peras and bench/data/dashboard.peras as
//      written today; a refusal is kept with its first line, the reason.
//   2. What compiles runs in Chromium: the kernels fib, primes and mandel; the
//      garden's frame; the dashboard of decision 0014. Each result is checked
//      against TypeScript's (the same numbers, the same pixels, the same
//      summary), then timed, median of 7 runs after 2 of warm-up, in each of
//      5 rounds, the versions in a new order each round; a measure is the
//      median of the rounds, kept with their least and most.
//   3. Bytes shipped, gzip.
// Usage: node scripts/bench-history.mjs
// Writes bench/history/results.json, and bench/history/rows.json: one row per
// version, program and measure, as the visual report reads them.
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";
import ts from "typescript";

const root = join(import.meta.dirname, "..");
const work = join(root, "target", "history");
mkdirSync(join(work, "bin"), { recursive: true });

/**
 * The milestones: the first compiler that emitted WebAssembly, the release, the merges.
 * @type {[string, string][]}
 */
const milestones = [
  ["484c80c", "first WebAssembly"],
  ["ef33ccb", "0.0.1"],
  ["ac717cd", "PR #4"],
  ["b2403fe", "PR #7"],
  ["e2e0465", "PR #10"],
  ["7129e0a", "PR #13"],
  ["a93195e", "PR #17"],
  ["caff747", "PR #19, main"],
  ["HEAD", "today"],
];

/** @param {string} command @param {string[]} args @param {object} [options] */
function sh(command, args, options = {}) {
  return spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 28, ...options });
}

// 1. The compilers.
const head = sh("git", ["rev-parse", "--short=7", "HEAD"], { cwd: root }).stdout.trim();
/** @type {{ commit: string, label: string, date: string, perasc: string }[]} */
const versions = [];
for (const [ref, label] of milestones) {
  const commit = ref === "HEAD" ? head : ref;
  const date = sh("git", ["log", "-1", "--format=%ad", "--date=short", ref], {
    cwd: root,
  }).stdout.trim();
  const binary = join(work, "bin", `perasc-${commit}`);
  if (ref === "HEAD") {
    copyFileSync(join(root, "target", "release", "perasc"), binary);
  } else if (!existsSync(binary)) {
    console.log(`building perasc at ${commit}…`);
    const src = join(work, "src", commit);
    rmSync(src, { recursive: true, force: true });
    mkdirSync(src, { recursive: true });
    // \`-m\` dates the files now: with the commit's dates, older than what the
    // shared target holds, cargo would link the crates built for another
    // version (finding M009).
    const archive = sh("sh", ["-c", `git archive ${commit} | tar -x -m -C "${src}"`], {
      cwd: root,
    });
    if (archive.status !== 0) throw new Error(archive.stderr);
    const built = sh("cargo", ["build", "--release", "--locked", "-q", "-p", "perasc"], {
      cwd: src,
      env: { ...process.env, CARGO_TARGET_DIR: join(work, "build"), CARGO_INCREMENTAL: "0" },
    });
    if (built.status !== 0) throw new Error(`cargo build at ${commit}\n${built.stderr}`);
    copyFileSync(join(work, "build", "release", "perasc"), binary);
    rmSync(src, { recursive: true, force: true });
  }
  versions.push({ commit, label, date, perasc: binary });
}

// The escape sequences of coloured diagnostics, removed from a reason.
const colours = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, "g");

// 2. The programs, compiled by each: the kernels as they were written on the
// first day, which every version should accept; the garden and the dashboard
// as written today, which the early versions may refuse.
const firstKernels = join(work, "kernels.peras");
writeFileSync(
  firstKernels,
  sh("git", ["show", "484c80c:bench/backends/kernels.peras"], { cwd: root }).stdout,
);
const programs = {
  kernels: firstKernels,
  garden: join(root, "bench", "garden", "garden.peras"),
  dashboard: join(root, "bench", "data", "dashboard.peras"),
};
/** @type {Record<string, Record<string, { ok: boolean, why?: string, flags?: string[], bytes?: number }>>} */
const builds = {};
for (const v of versions) {
  builds[v.commit] = {};
  for (const [name, file] of Object.entries(programs)) {
    const out = join(work, "out", v.commit, name);
    rmSync(out, { recursive: true, force: true });
    // A production build where the compiler has one; the oldest have no option.
    let flags = ["--allocator", "tlsf"];
    let result = sh(v.perasc, ["build", file, ...flags, "--out", out, "--lang", "en"]);
    if (
      result.status !== 0 &&
      /allocator|unknown|unexpected/i.test(result.stderr) &&
      !result.stderr.includes("error[E")
    ) {
      flags = [];
      result = sh(v.perasc, ["build", file, "--out", out, "--lang", "en"]);
    }
    const entry = builds[v.commit];
    if (entry === undefined) continue;
    if (result.status !== 0) {
      const lines = `${result.stderr}${result.stdout}`.split("\n").map((l) => l.trim());
      const first =
        lines.find((l) => l.startsWith("error")) ?? lines.find((l) => l !== "") ?? "refused";
      entry[name] = { ok: false, why: first.replace(colours, "").slice(0, 160) };
      continue;
    }
    const files = readdirSync(out);
    const bytes = files
      .filter((f) => f.endsWith(".js") || f.endsWith(".wasm"))
      .reduce((n, f) => n + gzipSync(readFileSync(join(out, f)), { level: 9 }).length, 0);
    entry[name] = { ok: true, flags, bytes };
  }
}

// The TypeScript the programs are measured against, transpiled as written.
mkdirSync(join(work, "ts"), { recursive: true });
for (const [from, to] of [
  ["bench/garden/garden.ts", "garden.js"],
  ["bench/data/summary.ts", "summary.js"],
  ["bench/data/dashboard-idiomatic.ts", "dashboard-idiomatic.js"],
  ["bench/data/generate.ts", "generate.js"],
]) {
  const js = ts.transpileModule(readFileSync(join(root, from), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  writeFileSync(join(work, "ts", to), js.replaceAll(/\.ts(["'])/g, ".js$1"));
}
writeFileSync(
  join(work, "ts", "kernels.js"),
  `// The JavaScript reference of bench/backends/kernels.peras, from scripts/bench-backends.mjs.
export function fib(n) { return n < 2 ? n : (fib(n - 1) + fib(n - 2)) | 0; }
export function primes(limit) {
  let count = 0;
  for (let n = 2; n < limit; n++) {
    let prime = true;
    for (let d = 2; Math.imul(d, d) <= n; d++) if (n % d === 0) { prime = false; break; }
    if (prime) count++;
  }
  return count;
}
export function mandel(size) {
  let inside = 0;
  const scale = 3.0 / size;
  for (let py = 0; py < size; py++)
    for (let px = 0; px < size; px++) {
      const cx = px * scale - 2.0, cy = py * scale - 1.5;
      let x = 0.0, y = 0.0, step = 0;
      while (step < 64 && x * x + y * y <= 4.0) { const next = x * x - y * y + cx; y = 2.0 * x * y + cy; x = next; step++; }
      if (step === 64) inside++;
    }
  return inside;
}
`,
);

// 3. Run in Chromium.
const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".html": "text/html",
});
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  if (path === "/") {
    response.writeHead(200, { "content-type": "text/html" });
    response.end(
      '<!doctype html><meta charset="utf-8"><title>history</title><link rel="icon" href="data:,">',
    );
    return;
  }
  try {
    const file = join(work, path);
    response.writeHead(200, { "content-type": types[extname(file)] ?? "text/plain" });
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
const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
const page = await browser.newPage();
await page.goto(`http://localhost:${String(port)}/`);

/** Runs in the page: loads a build, by its loader where it has one, else its WebAssembly alone. */
const harness = `
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const timed = (f, runs = 7) => { for (let i = 0; i < 2; i++) f(); const t = []; for (let i = 0; i < runs; i++) { const s = performance.now(); f(); t.push(performance.now() - s); } return median(t); };
async function load(dir, name) {
  try { const m = await import(dir + "/" + name + ".js"); if (typeof m.default === "function" && m.fib === undefined && m.render === undefined && m.summarize === undefined) await m.default(); return m; }
  catch (e) {
    const bytes = await (await fetch(dir + "/" + name + ".wasm")).arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return instance.exports;
  }
}
const hex = async (bytes) => [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map((b) => b.toString(16).padStart(2, "0")).join("");
`;

/** Every round's results, by target: five rounds, the targets in a new order each time. */
/** @type {Record<string, Record<string, Record<string, unknown>>[]>} */
const rounds = {};
const targets = [...versions.map((v) => v.commit), "typescript"];
const ROUNDS = 5;
for (let round = 0; round < ROUNDS; round++) {
  // Rotated, so that no version always runs first or last.
  const order = targets.map((_, i) => targets[(i + round * 2) % targets.length] ?? "typescript");
  for (const target of order) {
    console.log(`round ${String(round + 1)}, ${target}…`);
    /** @type {unknown} */
    const out = await page.evaluate(
      `(async () => {
      ${harness}
      const target = ${JSON.stringify(target)};
      const ts = target === "typescript";
      const built = ${JSON.stringify(builds)}[target] ?? {};
      const result = {};
      // Kernels.
      if (ts || built.kernels?.ok) {
        try {
          const k = ts ? await import("/ts/kernels.js") : await load("/out/" + target + "/kernels", "kernels");
          const ref = await import("/ts/kernels.js");
          const same = k.fib(25) === ref.fib(25) && k.primes(20000) === ref.primes(20000) && k.mandel(120) === ref.mandel(120);
          result.kernels = { same, fib: timed(() => k.fib(30)), primes: timed(() => k.primes(300000)), mandel: timed(() => k.mandel(400)) };
        } catch (e) { result.kernels = { error: String(e).slice(0, 160) }; }
      }
      // The garden.
      if (ts || built.garden?.ok) {
        try {
          const g = ts ? await import("/ts/garden.js") : await load("/out/" + target + "/garden", "garden");
          const ref = await import("/ts/garden.js");
          const frame = (side, zoom) => {
            if (ts) { const px = new Uint32Array(side * side); g.render(px, side, side, zoom, 0.37, 0.83); return px; }
            // Before arrays in the module's memory, an inout array was a plain one, copied back.
            const a = g.__perasHeap?.array ? g.__perasHeap.array("i32", side * side) : new Array(side * side).fill(0);
            g.render(a, side, side, zoom, 0.37, 0.83);
            const view = a.view ? a.view().slice() : Int32Array.from(a);
            a.release?.();
            return new Uint32Array(view.buffer, view.byteOffset, side * side);
          };
          const expected = (side, zoom) => { const px = new Uint32Array(side * side); ref.render(px, side, side, zoom, 0.37, 0.83); return px; };
          const same = (await hex(frame(128, 1))) === (await hex(expected(128, 1))) && (await hex(frame(128, 64))) === (await hex(expected(128, 64)));
          result.garden = { same, frame: timed(() => frame(384, 8)) };
        } catch (e) { result.garden = { error: String(e).slice(0, 160) }; }
      }
      // The dashboard.
      if (ts || built.dashboard?.ok) {
        try {
          const { generate } = await import("/ts/generate.js");
          const { canonical } = await import("/ts/summary.js");
          const a = await import("/ts/dashboard-idiomatic.js");
          const d = ts ? null : await load("/out/" + target + "/dashboard", "dashboard");
          const run = ts ? (bytes) => a.summarize(bytes) : (bytes) => d.summarize(new TextDecoder().decode(bytes));
          const small = generate(20000);
          const same = canonical(run(small)) === canonical(a.summarize(small));
          const big = generate(300000);
          result.dashboard = { same, summary: timed(() => run(big), 5) };
        } catch (e) { result.dashboard = { error: String(e).slice(0, 160) }; }
      }
      return result;
    })()`,
    );
    (rounds[target] ??= []).push(/** @type {Record<string, Record<string, unknown>>} */ (out));
  }
}
await browser.close();

// Each measure: the median of the rounds, with their least and most; "same"
// only if every round gave TypeScript's result.
/** @param {number[]} xs */
const middle = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;
/** @type {Record<string, Record<string, unknown>>} */
const runs = {};
for (const [target, list] of Object.entries(rounds)) {
  /** @type {Record<string, Record<string, unknown>>} */
  const merged = {};
  for (const program of ["kernels", "garden", "dashboard"]) {
    const parts = list.map((r) => r[program]).filter((p) => p !== undefined);
    const first = parts[0];
    if (first === undefined) continue;
    /** @type {Record<string, unknown>} */
    const out = {};
    for (const key of Object.keys(first)) {
      const values = parts.map((p) => p[key]);
      if (values.every((x) => typeof x === "number")) {
        const numbers = /** @type {number[]} */ (values);
        out[key] = middle(numbers);
        out[`${key}_min`] = Math.min(...numbers);
        out[`${key}_max`] = Math.max(...numbers);
      } else if (key === "same") {
        out.same = values.every((x) => x === true);
      } else {
        out[key] = values[0];
      }
    }
    merged[program] = out;
  }
  runs[target] = merged;
}
server.close();

const results = {
  machine: cpus()[0]?.model ?? "unknown",
  cores: cpus().length,
  versions: versions.map(({ commit, label, date }) => ({ commit, label, date })),
  builds,
  runs,
};
mkdirSync(join(root, "bench", "history"), { recursive: true });
writeFileSync(
  join(root, "bench", "history", "results.json"),
  `${JSON.stringify(results, null, 2)}\n`,
);
// One row per version, program and measure; a program a version refuses is
// one row with its reason. The speed is TypeScript's time over Peras's.
/** @typedef {Record<string, number | boolean | string | undefined>} Run */
const measures = {
  kernels: ["fib", "primes", "mandel"],
  garden: ["frame"],
  dashboard: ["summary"],
};
const tsRuns = /** @type {Record<string, Run | undefined>} */ (runs.typescript ?? {});
/** @param {unknown} top @param {unknown} bottom */
const ratio = (top, bottom) =>
  typeof top === "number" && typeof bottom === "number" && bottom > 0
    ? Math.round((top / bottom) * 100) / 100
    : null;
const rows = versions.flatMap((v, order) =>
  Object.entries(measures).flatMap(([program, names]) => {
    const built = builds[v.commit]?.[program];
    const forVersion = /** @type {Record<string, Run | undefined> | undefined} */ (runs[v.commit]);
    const run = forVersion?.[program];
    const base = { order: order + 1, commit: v.commit, date: v.date, version: v.label, program };
    if (built?.ok !== true) {
      return [
        {
          ...base,
          measure: "",
          compiles: false,
          why: built?.why ?? "",
          bytes: null,
          peras_ms: null,
          typescript_ms: null,
          speed: null,
          speed_low: null,
          speed_high: null,
          same: null,
        },
      ];
    }
    return names.map((measure) => {
      const peras = run?.[measure];
      const reference = tsRuns[program]?.[measure];
      const perasMs = typeof peras === "number" ? Math.round(peras * 10) / 10 : null;
      const tsMs = typeof reference === "number" ? Math.round(reference * 10) / 10 : null;
      return {
        ...base,
        measure,
        compiles: true,
        why: typeof run?.error === "string" ? run.error : "",
        bytes: built.bytes ?? null,
        peras_ms: perasMs,
        typescript_ms: tsMs,
        speed:
          perasMs === null || tsMs === null || perasMs === 0
            ? null
            : Math.round((tsMs / perasMs) * 100) / 100,
        // The widest the speed could be over the rounds: TypeScript's least
        // over Peras's most, and its most over Peras's least.
        speed_low: ratio(tsRuns[program]?.[`${measure}_min`], run?.[`${measure}_max`]),
        speed_high: ratio(tsRuns[program]?.[`${measure}_max`], run?.[`${measure}_min`]),
        same: typeof run?.same === "boolean" ? run.same : null,
      };
    });
  }),
);
writeFileSync(join(root, "bench", "history", "rows.json"), `${JSON.stringify(rows, null, 1)}\n`);
console.log(`${String(rows.length)} rows written to bench/history/rows.json`);
