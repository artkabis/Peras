// Decision 0039: durations and instants. t1, chosen, `Duration` and
// `Instant`, is the build's, measured against TypeScript; t0 (numbers of
// milliseconds) and t2 (literals with a unit) were measured at commit
// 370b4ce and removed once not chosen: their figures are in the decision.
// Every module is a production build (`--allocator tlsf`, finding M029).
//
// 1. Bugs caught: bench/time/bugs, each bug in TypeScript, type-checked by
//    its compiler, and in Peras.
// 2. Ease: the lines of each program of bench/time/ease, blank lines and
//    comments left out.
// 3. Correctness: P2 to P4 and W1 give what TypeScript gives.
// 4. Speed and load on the processor: W1, 1,000,000 durations added and
//    compared, in a managed and in an owned module, against TypeScript's
//    numbers, in one process, the order alternating, the median ratio round
//    by round and its range, the CPU time over the wall time; the bytes.
//
// Usage: node scripts/bench-time.mjs [--check] [--rounds N]
//        --check: one round, a smaller workload, results checked, nothing
//        written.
// Writes bench/time/CURRENT.md, unless --check: RESULTS.md keeps the
// options as decision 0039 measured them.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";
import ts from "typescript";

const check = process.argv.includes("--check");
const at = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(at > 0 ? process.argv[at + 1] : 21);
const OPTIONS = ["t1"];
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "time");
const work = join(root, "target", "bench-time");
const perasc = join(root, "target", "release", "perasc");

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

let builds = 0;
/**
 * Builds a file under an option, for production.
 * @param {string} file
 * @param {string} option
 */
function build(file, option) {
  builds += 1;
  const dir = join(work, `${String(builds)}-${option}`);
  mkdirSync(dir, { recursive: true });
  const r = spawnSync(
    perasc,
    ["build", file, "--out", dir, "--allocator", "tlsf", "--color", "never"],
    { encoding: "utf8", env: process.env },
  );
  const errors = [...r.stderr.matchAll(/^error\[(E\d+)\]: (.*)$/gm)].map(
    (m) => `${m[1] ?? ""} ${m[2] ?? ""}`,
  );
  return { ok: r.status === 0, dir, errors };
}

/**
 * Builds and loads a module.
 * @param {string} file
 * @param {string} option
 */
async function load(file, option) {
  const b = build(file, option);
  if (!b.ok) throw new Error(`${file}, ${option}: ${b.errors.join("; ")}`);
  const name =
    file
      .split("/")
      .pop()
      ?.replace(/\.(peras|apeiron)$/, "") ?? "m";
  const shipped = file.endsWith(".peras") ? `${name}.wasm` : `${name}.js`;
  const bytes = readFileSync(join(b.dir, shipped));
  /** @type {unknown} */
  const m = await import(join(b.dir, `${name}.js`));
  return {
    m: /** @type {Record<string, (...a: unknown[]) => unknown>} */ (m),
    bytes: { raw: bytes.length, gzip: gzipSync(bytes).length },
  };
}

/** Lines of code: blank lines and comments left out. @param {string} file */
const lines = (file) =>
  readFileSync(file, "utf8")
    .split("\n")
    .filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;

const out = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; every module built for production.`,
  "",
  "### 1. Bugs caught",
  "",
  `| Bug | TypeScript | ${OPTIONS.join(" | ")} |`,
  `| --- | --- |${OPTIONS.map(() => " --- |").join("")}`,
];
const bugs = join(bench, "bugs");
for (let b = 1; b <= 6; b++) {
  const program = ts.createProgram([join(bugs, `b${String(b)}.ts`)], {
    strict: true,
    noEmit: true,
    target: ts.ScriptTarget.ES2022,
    lib: ["lib.es2022.d.ts", "lib.dom.d.ts"],
    types: [],
  });
  const tsErrors = ts.getPreEmitDiagnostics(program).length;
  const cells = OPTIONS.map((option) => {
    const file = join(bugs, `b${String(b)}-${option}.apeiron`);
    copyFileSync(join(bugs, "clock.js"), join(work, "clock.js"));
    const r = build(file, option);
    return r.ok ? "builds unseen" : `refused: ${(r.errors[0] ?? "").replaceAll("|", "\\|")}`;
  });
  out.push(
    `| B${String(b)} | ${tsErrors === 0 ? "builds unseen" : "refused"} | ${cells.join(" | ")} |`,
  );
}

out.push(
  "",
  "### 2. Ease: lines of each program (blank lines and comments left out)",
  "",
  "| Option | P1, a search after 300 ms | P2, the time taken | P3, a countdown | P4, a playlist (owned) | W1 |",
  "| --- | ---: | ---: | ---: | ---: | ---: |",
);
const ease = join(bench, "ease");
for (const option of OPTIONS) {
  const count = (/** @type {string} */ p, /** @type {string} */ ext) =>
    String(lines(join(ease, `${p}-${option}.${ext}`)));
  out.push(
    `| ${option} | ${count("p1", "apeiron")} | ${count("p2", "apeiron")} | ${count("p3", "apeiron")} | ${count("p4", "peras")} | ${count("w1", "peras")} |`,
  );
}

// 3. Correctness.
const countdown = (/** @type {number} */ ms) => {
  const minutes = Math.floor(ms / 60000);
  const m = minutes % 60;
  return `${String(Math.trunc(minutes / 60))} h ${m < 10 ? "0" : ""}${String(m)} min`;
};
const tracks = [
  { title: "a", seconds: 90 },
  { title: "b", seconds: 150.5 },
  { title: "c", seconds: 61 },
];
const playlist = tracks.reduce((s, t) => s + t.seconds, 0) / 60;
/** @param {number} n */
const spread = (n) => {
  let total = 0;
  let over = 0;
  for (let i = 0; i < n; i++) {
    const d = (i % 1000) * 1.5;
    total += d;
    if (d > 700) over += 1;
  }
  return total + over;
};
for (const option of OPTIONS) {
  const p2 = String((await load(join(ease, `p2-${option}.apeiron`), option)).m.timed?.(1000));
  if (!/^-?\d+: \d+\.\d ms$/.test(p2)) throw new Error(`P2, ${option}: ${p2}`);
  for (const ms of [0, 59_999, 7_500_000, 36_000_000]) {
    const got = (await load(join(ease, `p3-${option}.apeiron`), option)).m.countdown?.(ms);
    if (got !== countdown(ms)) throw new Error(`P3, ${option}, ${String(ms)}: ${String(got)}`);
  }
  const p4 = (await load(join(ease, `p4-${option}.peras`), option)).m.total?.(tracks);
  if (Math.abs(Number(p4) - playlist) > 1e-9) throw new Error(`P4, ${option}: ${String(p4)}`);
}

// 4. W1.
const n = check ? 10_000 : 1_000_000;
for (const mode of ["managed", "owned"]) {
  /** @type {{ name: string, bytes: { raw: number, gzip: number } | undefined, run: () => unknown }[]} */
  const sides = [
    {
      name: "TypeScript",
      bytes: /** @type {{ raw: number, gzip: number } | undefined} */ (undefined),
      run: () => spread(n),
    },
  ];
  for (const option of OPTIONS) {
    let file = join(ease, `w1-${option}.peras`);
    if (mode === "managed") {
      const copy = join(work, `w1-${option}.apeiron`);
      copyFileSync(file, copy);
      file = copy;
    }
    const b = await load(file, option);
    sides.push({ name: option, bytes: b.bytes, run: () => b.m.spread?.(n) });
  }
  const want = spread(n);
  for (const side of sides) {
    const got = Number(side.run());
    if (got !== want)
      throw new Error(`W1 ${mode}, ${side.name}: ${String(got)}, not ${String(want)}`);
  }
  const times = sides.map(() => /** @type {number[]} */ ([]));
  const cpu = sides.map(() => 0);
  const wall = sides.map(() => 0);
  for (let round = 0; round < ROUNDS; round++) {
    const order = sides.map((_, i) => i);
    if (round % 2 === 1) order.reverse();
    for (const i of order) {
      const c0 = process.cpuUsage();
      const t0 = performance.now();
      sides[i]?.run();
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[i]?.push(t);
      wall[i] = (wall[i] ?? 0) + t;
      cpu[i] = (cpu[i] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  /** @param {number[]} xs */
  const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
  out.push(
    "",
    `### 4. W1, ${String(n)} durations added and compared, ${mode} module`,
    "",
    "| Version | Median | / TypeScript, median (range) | CPU / wall | Bytes (gzip) |",
    "| --- | ---: | --- | ---: | ---: |",
  );
  sides.forEach(({ name, bytes }, i) => {
    const own = times[i] ?? [];
    const ratios = own.map((t, k) => t / (times[0]?.[k] ?? Number.NaN));
    const size = bytes === undefined ? "" : `${String(bytes.raw)} (${String(bytes.gzip)})`;
    out.push(
      `| ${name} | ${median(own).toFixed(2)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[i] ?? 0) / (wall[i] ?? 1)).toFixed(2)} | ${size} |`,
    );
  });
}

console.log(out.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "CURRENT.md"),
    `# Durations and instants: the build of today\n\nGenerated by \`node scripts/bench-time.mjs\`, for [decision 0039](../../docs/en/decisions/0039-durations-and-instants.md).\n\n${out.join("\n")}\n`,
  );
}
