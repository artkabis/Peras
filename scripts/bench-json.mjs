// Decision 0038: JSON. j1, chosen, `T.fromJson` and `x.toJson()` derived by
// the compiler, is the build's, measured against j0, as Peras read JSON
// before (`extern` and JavaScript's `JSON.parse`); j2, a `Json` value, was
// measured at commit 33d5838 and removed once not chosen: its figures are in
// the decision. Every module is a managed module built for production
// (`--allocator tlsf`, finding M029).
//
// 1. Ease: the lines of each program of bench/json/ease, blank lines and
//    comments left out, the JavaScript j0 needs counted with it.
// 2. Correctness: every option gives the reference's results; P4's invalid
//    texts are refused, what each option tells the program recorded.
// 3. Speed and load on the processor, in Node: the reference written by
//    hand (`JSON.parse`, then the fields checked one by one, as a careful
//    program does) and every option's module in one process, the order
//    alternating, the median ratio round by round and its range, the CPU
//    time over the wall time.
//    W1, 100,000 records read and one field summed (P2);
//    W2, 100,000 records written;
//    W3, the settings of P1 read 100,000 times.
// 4. Bytes: each module, raw and gzipped.
//
// Usage: node scripts/bench-json.mjs [--check] [--rounds N]
//        --check: one round, smaller workloads, results checked, nothing
//        written.
// Writes bench/json/CURRENT.md, unless --check: RESULTS.md keeps the
// options as decision 0038 measured them.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";

const check = process.argv.includes("--check");
const at = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(at > 0 ? process.argv[at + 1] : 21);
const OPTIONS = ["j0", "j1"];
const root = join(import.meta.dirname, "..");
const ease = join(root, "bench", "json", "ease");
const work = join(root, "target", "bench-json");
const perasc = join(root, "target", "release", "perasc");
const scale = check ? 0.01 : 1;

rmSync(work, { recursive: true, force: true });

/** @typedef {Record<string, (...args: any[]) => unknown>} Loaded */

/**
 * Builds one program of an option as a managed module for production, and
 * loads it.
 * @param {string} option
 * @param {string} program
 */
async function built(option, program) {
  const dir = join(work, option);
  mkdirSync(dir, { recursive: true });
  const r = spawnSync(
    perasc,
    [
      "build",
      join(ease, option, `${program}.apeiron`),
      "--out",
      dir,
      "--allocator",
      "tlsf",
      "--color",
      "never",
    ],
    { encoding: "utf8", env: process.env },
  );
  if (r.status !== 0) throw new Error(`${option} ${program}: ${r.stderr}`);
  if (option === "j0") copyFileSync(join(ease, "j0", "json.js"), join(dir, "json.js"));
  const js = readFileSync(join(dir, `${program}.js`));
  /** @type {unknown} */
  const m = await import(join(dir, `${program}.js`));
  return { m: /** @type {Loaded} */ (m), bytes: { raw: js.length, gzip: gzipSync(js).length } };
}

/** Lines of code: blank lines and comments left out. @param {string} file */
const lines = (file) =>
  readFileSync(file, "utf8")
    .split("\n")
    .filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;

// The references, written by hand: JSON.parse, then each field checked.
/** @param {unknown} v @param {string} what */
const need = (v, what) => {
  if (v === undefined) throw new Error(`${what}: missing`);
  return v;
};
/** @param {unknown} v */
const int = (v) =>
  typeof v === "number" && Number.isInteger(v) && v >= -2147483648 && v <= 2147483647;
const reference = {
  /** @param {string} text */
  total(text) {
    /** @type {unknown} */
    const parsed = JSON.parse(text);
    const doc = /** @type {{ items?: { price?: unknown }[] }} */ (parsed);
    if (!Array.isArray(doc.items)) return -1;
    let sum = 0;
    for (const item of doc.items) {
      if (typeof item.price !== "number") return -1;
      sum += item.price;
    }
    return sum;
  },
  /** @param {number} n */
  write(n) {
    const records = [];
    for (let i = 0; i < n; i++)
      records.push({ id: i, name: `item ${String(i)}`, price: i * 0.25, tags: ["a", "b"] });
    return JSON.stringify(records);
  },
  /** @param {string} text */
  describe(text) {
    try {
      /** @type {unknown} */
      const parsed = JSON.parse(text);
      const s = /** @type {Record<string, unknown>} */ (parsed);
      if (typeof need(s.title, "title") !== "string") return "invalid: title";
      if (!int(need(s.retries, "retries"))) return "invalid: retries";
      if (s.theme !== "Light" && s.theme !== "Dark") return "invalid: theme";
      if (!Array.isArray(s.servers)) return "invalid: servers";
      for (const sv of /** @type {Record<string, unknown>[]} */ (s.servers)) {
        if (typeof sv.host !== "string" || !int(sv.port)) return "invalid: servers";
      }
      if (s.proxy !== null && s.proxy !== undefined && typeof s.proxy !== "string")
        return "invalid: proxy";
      return `${String(s.title)}: ${String(s.servers.length)} servers, ${String(s.retries)} retries`;
    } catch (e) {
      return `invalid: ${String(e)}`;
    }
  },
};

const n = Math.round(100_000 * scale);
const records = Array.from({ length: n }, (_, i) => ({
  id: i,
  name: `item ${String(i)}`,
  price: (i % 1000) * 0.5,
  tags: ["a", "b", "c"].slice(0, (i % 3) + 1),
}));
const recordsText = JSON.stringify({ items: records });
const settings = {
  title: "Pond",
  retries: 3,
  theme: "Dark",
  servers: [
    { host: "a.example", port: 80 },
    { host: "b.example", port: 81 },
  ],
  proxy: null,
};
const settingsText = JSON.stringify(settings);
const invalid = {
  malformed: '{"title": "Pond",',
  missing: JSON.stringify({ ...settings, retries: undefined }),
  wrongType: JSON.stringify({ ...settings, servers: [{ host: "a", port: "80" }] }),
  outOfRange: JSON.stringify({ ...settings, retries: 3000000000 }),
  unknownVariant: JSON.stringify({ ...settings, theme: "Blue" }),
};
const reads = Math.round(100_000 * scale);

const out = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; managed modules built for production.`,
  "",
  "### Ease: lines of each program (blank lines and comments left out)",
  "",
  "| Option | P1, settings | P2, records summed | P3, a summary written | W2, records written | JavaScript needed |",
  "| --- | ---: | ---: | ---: | ---: | ---: |",
];
for (const option of OPTIONS) {
  const count = (/** @type {string} */ p) => String(lines(join(ease, option, `${p}.apeiron`)));
  const js = option === "j0" ? String(lines(join(ease, "j0", "json.js"))) : "0";
  out.push(
    `| ${option} | ${count("p1")} | ${count("p2")} | ${count("p3")} | ${count("w2")} | ${js} |`,
  );
}
out.push(
  "",
  "### P4: what each option tells the program of an invalid text",
  "",
  "| Text | " + OPTIONS.join(" | ") + " |",
  "| --- |" + OPTIONS.map(() => " --- |").join(""),
);
/** @type {Record<string, Loaded>} */
const p1 = {};
for (const option of OPTIONS) p1[option] = (await built(option, "p1")).m;
for (const [name, text] of Object.entries(invalid)) {
  const cells = OPTIONS.map((option) => {
    try {
      const r = String(p1[option]?.describe?.(text));
      if (!r.startsWith("invalid: ")) throw new Error(`${option}, ${name}: read as ${r}`);
      return `an error it tests: ${r.slice(9).replaceAll("|", "\\|")}`;
    } catch (e) {
      if (e instanceof Error && e.message.includes("read as")) throw e;
      return `an exception: ${e instanceof Error ? e.constructor.name : "?"}`;
    }
  });
  out.push(`| ${name} | ${cells.join(" | ")} |`);
}
out.push("");

/**
 * @typedef {{ label: string, program: string, run: (m: Loaded) => unknown }} Workload
 * @type {Workload[]}
 */
const workloads = [
  {
    label: `W1, ${String(n)} records read, one field summed`,
    program: "p2",
    run: (m) => m.total?.(recordsText),
  },
  {
    label: `W2, ${String(n)} records written`,
    program: "w2",
    run: (m) => String(m.write?.(n)).length,
  },
  {
    label: `W3, the settings of P1 read ${String(reads)} times`,
    program: "p1",
    run: (m) => {
      let last = "";
      for (let i = 0; i < reads; i++) last = String(m.describe?.(settingsText));
      return last;
    },
  },
];
for (const { label, program, run } of workloads) {
  const sides = [
    {
      name: "reference",
      bytes: /** @type {{ raw: number, gzip: number } | undefined} */ (undefined),
      m: /** @type {Loaded} */ (reference),
    },
  ];
  for (const option of OPTIONS) {
    const b = await built(option, program);
    sides.push({ name: option, bytes: b.bytes, m: b.m });
  }
  const want = String(run(reference));
  for (const side of sides) {
    const got = String(run(side.m));
    if (got !== want) throw new Error(`${label}, ${side.name}: ${got}, not ${want}`);
  }
  const times = sides.map(() => /** @type {number[]} */ ([]));
  const cpu = sides.map(() => 0);
  const wall = sides.map(() => 0);
  for (let round = 0; round < ROUNDS; round++) {
    const order = sides.map((_, i) => i);
    if (round % 2 === 1) order.reverse();
    for (const i of order) {
      const side = sides[i];
      if (side === undefined) continue;
      const c0 = process.cpuUsage();
      const t0 = performance.now();
      run(side.m);
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
    `### ${label}`,
    "",
    "| Version | Median | / reference, median (range) | CPU / wall | Bytes (gzip) |",
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
  out.push("");
}

console.log(out.join("\n"));
if (!check) {
  writeFileSync(
    join(root, "bench", "json", "CURRENT.md"),
    `# JSON: the build of today against j0\n\nGenerated by \`node scripts/bench-json.mjs\`, for [decision 0038](../../docs/en/decisions/0038-json.md).\n\n${out.join("\n")}`,
  );
}
