// Decision 0042: queries for the tools. q1 (`perasc query`, one process a
// query) and q2 (`perasc serve`, one process for an editor's session), both
// prototypes behind `PERAS_PROTO_QUERY`; perasc built for release (finding
// M029).
//
// 1. Answers: bench/query/queries.json, fifty queries on six programs, each
//    answered by both options as written by hand.
// 2. A file being typed: ten queries on the same programs with a syntax error
//    before the position; which still answer.
// 3. Speed and load on the processor: each kind of query on three files, the
//    median time from the request to the answer and its 95th percentile; a
//    scenario of 100 edits, each followed by `diagnostics` and `hover`, its
//    CPU time over its wall time; the options alternating in one process
//    (finding M017).
// 4. Memory: the peak of q2's process with every Peras file of the
//    repository opened, and of a q1 process on the largest file, read from
//    Linux (`VmHWM` of `/proc/<pid>/status`).
//
// Usage: node scripts/bench-query.mjs [--check] [--rounds N]
//        --check: points 1 and 2 checked, one round of the rest, nothing
//        written.
// Writes bench/query/RESULTS.md, unless --check.
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { basename, join } from "node:path";
import { performance } from "node:perf_hooks";
import { createInterface } from "node:readline";
import { clearInterval, setInterval } from "node:timers";

const check = process.argv.includes("--check");
const at = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(at > 0 ? process.argv[at + 1] : 11);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "query");
const work = join(root, "target", "bench-query");
const perasc = join(root, "target", "release", "perasc");

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/**
 * @typedef {{ kind: string, file: string, at?: string, text?: string }} Query
 * @typedef {{ name?: string, what?: string, type?: string, file?: string,
 *   at?: string | string[], symbols?: { name: string, what: string, line: number }[],
 *   includes?: string[], names?: string[], codes?: string[], unchanged?: boolean }} Answer
 * @typedef {{ file: string, kind: string, at?: string, expect: Answer,
 *   replace?: { line: number, text: string } }} Entry
 * @typedef {{ ask: (q: Query) => Promise<unknown>, change: (file: string, text: string) => Promise<void>, close: () => void, pid?: number }} Client
 */

/**
 * The client of q1: one process a query, the text on its standard input.
 * @returns {Client}
 */
function q1() {
  return {
    ask(q) {
      const args = ["query", q.kind, q.file];
      if (q.at !== undefined) args.push("--at", q.at);
      if (q.text !== undefined) args.push("--stdin");
      const r = spawnSync(perasc, args, {
        input: q.text,
        encoding: "utf8",
        env: { ...process.env, PERAS_PROTO_QUERY: "q1" },
      });
      /** @type {unknown} */
      const answer = JSON.parse(r.stdout || "null");
      return Promise.resolve(answer);
    },
    change: () => Promise.resolve(),
    close: () => undefined,
  };
}

/**
 * The client of q2: one process, a request a line, an answer a line.
 * @returns {Client}
 */
function q2() {
  const child = spawn(perasc, ["serve"], {
    env: { ...process.env, PERAS_PROTO_QUERY: "q2" },
    stdio: ["pipe", "pipe", "inherit"],
  });
  let next = 0;
  /** @type {Map<number, (v: unknown) => void>} */
  const waiting = new Map();
  createInterface({ input: child.stdout }).on("line", (line) => {
    /** @type {unknown} */
    const parsed = JSON.parse(line);
    const reply = /** @type {{ id: number, result: unknown }} */ (parsed);
    waiting.get(reply.id)?.(reply.result);
    waiting.delete(reply.id);
  });
  /** @param {Record<string, unknown>} request @returns {Promise<unknown>} */
  const send = (request) => {
    const id = next++;
    return new Promise((resolve) => {
      waiting.set(id, resolve);
      child.stdin.write(`${JSON.stringify({ id, ...request })}\n`);
    });
  };
  return {
    pid: child.pid,
    async ask(q) {
      if (q.text !== undefined) await send({ method: "change", file: q.file, text: q.text });
      return send({ method: "query", file: q.file, kind: q.kind, at: q.at });
    },
    async change(file, text) {
      await send({ method: "change", file, text });
    },
    close: () => child.stdin.end(),
  };
}

const OPTIONS = ["q1", "q2"];
/** @param {string} option */
const client = (option) => (option === "q1" ? q1() : q2());

/**
 * Whether an answer is the one expected.
 * @param {Entry} q
 * @param {unknown} answer
 */
function right(q, answer) {
  const e = q.expect;
  if (answer === null || answer === undefined) return false;
  const a = /** @type {Answer} */ (answer);
  switch (q.kind) {
    case "hover":
      return a.name === e.name && a.what === e.what && a.type === e.type;
    case "definition":
      return basename(a.file ?? "") === e.file && a.at === e.at;
    case "references":
      return JSON.stringify(a.at) === JSON.stringify(e.at);
    case "symbols":
      return (
        a.symbols?.length === e.symbols?.length &&
        (e.symbols ?? []).every((x, i) => {
          const y = a.symbols?.[i];
          return y?.name === x.name && y.what === x.what && y.line === x.line;
        })
      );
    case "completion":
      return (e.includes ?? []).every((n) => (a.names ?? []).includes(n));
    case "diagnostics":
      return JSON.stringify(a.codes) === JSON.stringify(e.codes);
    case "format":
      return a.unchanged === e.unchanged;
  }
  return false;
}

/** @type {unknown} */
const parsed = JSON.parse(readFileSync(join(bench, "queries.json"), "utf8"));
const expected = /** @type {{ queries: Entry[], typing: Entry[] }} */ (parsed);
const program = (/** @type {string} */ f) => join(bench, "programs", f);
const report = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; perasc built for release.`,
  "",
  "### 1. Answers and 2. a file being typed",
  "",
  "| Option | Right answers, of 50 | Being typed, of 10 |",
  "| --- | ---: | ---: |",
];
for (const option of OPTIONS) {
  const c = client(option);
  let good = 0;
  const wrong = [];
  for (const q of expected.queries) {
    const a = await c.ask({ kind: q.kind, file: program(q.file), at: q.at });
    if (right(q, a)) good++;
    else wrong.push(`${q.file} ${q.kind} ${q.at ?? ""}`);
  }
  let typed = 0;
  for (const q of expected.typing) {
    const original = readFileSync(program(q.file), "utf8");
    const lines = original.split("\n");
    if (q.replace !== undefined) lines[q.replace.line - 1] = q.replace.text;
    const a = await c.ask({
      kind: q.kind,
      file: program(q.file),
      at: q.at,
      text: lines.join("\n"),
    });
    if (right(q, a)) typed++;
    // The file as it is again, for the queries after.
    await c.change(program(q.file), original);
  }
  c.close();
  report.push(`| ${option} | ${String(good)} | ${String(typed)} |`);
  if (check && good !== expected.queries.length)
    throw new Error(`${option}: wrong answers: ${wrong.join("; ")}`);
}

// 3. Speed. Three files: 50 lines, the largest of the repository, 5,000.
const largestPath = join(root, "bench", "parallel", "as-written", "pond-bands.peras");
const largest = readFileSync(largestPath, "utf8");
/** The top-level items of a file, each with its text. @param {string} text */
const items = (text) => text.split(/\n(?=(?:export )?fn |struct |enum |let |const )/);
const names = [...largest.matchAll(/^(?:export )?(?:fn|struct|enum|const|let) (\w+)/gm)].map(
  (m) => m[1] ?? "",
);
/** The file's declarations again, each renamed with `k`. @param {number} k */
const renamed = (k) =>
  names.reduce((t, n) => t.replace(new RegExp(`\\b${n}\\b`, "g"), `${n}_${String(k)}`), largest);
let small = "";
for (const item of items(largest)) {
  if (small.split("\n").length >= 50) break;
  small += `${item}\n`;
}
let huge = largest;
for (let k = 1; huge.split("\n").length < 5000; k++) huge += `\n${renamed(k)}`;
const files = [
  { name: "small", path: join(work, "small.peras"), text: small },
  { name: "the largest", path: join(work, "largest.peras"), text: largest },
  { name: "large", path: join(work, "huge.peras"), text: huge },
];
for (const f of files) {
  writeFileSync(f.path, f.text);
  const r = spawnSync(perasc, ["check", f.path, "--color", "never"], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`${f.name}: does not check: ${r.stderr.slice(0, 300)}`);
  f.name = `${f.name}, ${String(f.text.split("\n").length)} lines`;
}
/** A position on a name: the first local a function declares. @param {string} text */
function position(text) {
  const lines = text.split("\n");
  const i = lines.findIndex((l) => /^\s+(let|const) \w/.test(l));
  const col = (lines[i] ?? "").search(/(?<=(let|const) )\w/);
  return `${String(i + 1)}:${String(col + 1)}`;
}
const KINDS = [
  "diagnostics",
  "hover",
  "definition",
  "references",
  "symbols",
  "completion",
  "format",
];
/** @param {number[]} xs @param {number} p */
const pct = (xs, p) =>
  [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(xs.length * p))] ?? Number.NaN;

report.push(
  "",
  "### 3. Speed: each kind of query, median (95th percentile), ms",
  "",
  `| File | Option | ${KINDS.join(" | ")} |`,
  `| --- | --- |${KINDS.map(() => " ---: |").join("")}`,
);
for (const f of files) {
  const pos = position(f.text);
  /** @type {Record<string, Record<string, number[]>>} */
  const t = {};
  /** @type {Record<string, Client>} */
  const clients = { q1: q1(), q2: q2() };
  await clients.q2?.change(f.path, f.text);
  for (let round = 0; round < ROUNDS; round++) {
    for (const option of round % 2 === 0 ? OPTIONS : [...OPTIONS].reverse()) {
      for (const kind of KINDS) {
        const t0 = performance.now();
        await clients[option]?.ask({ kind, file: f.path, at: pos });
        ((t[option] ??= {})[kind] ??= []).push(performance.now() - t0);
      }
    }
  }
  clients.q2?.close();
  for (const option of OPTIONS) {
    const cells = KINDS.map((k) => {
      const xs = t[option]?.[k] ?? [];
      return `${pct(xs, 0.5).toFixed(1)} (${pct(xs, 0.95).toFixed(1)})`;
    });
    report.push(`| ${f.name} | ${option} | ${cells.join(" | ")} |`);
  }
}

/**
 * The CPU time of this process and of its children that ended, in ms, from
 * Linux: `utime`, `stime`, `cutime` and `cstime` of `/proc/self/stat`.
 */
function ownCpu() {
  const stat = readFileSync("/proc/self/stat", "utf8").split(") ")[1]?.split(" ") ?? [];
  return ((Number(stat[11]) + Number(stat[12]) + Number(stat[13]) + Number(stat[14])) * 1000) / 100;
}
/** The CPU time of a running process, in ms, from Linux. @param {number | undefined} pid */
function cpuOf(pid) {
  if (pid === undefined) return 0;
  try {
    const stat =
      readFileSync(`/proc/${String(pid)}/stat`, "utf8")
        .split(") ")[1]
        ?.split(" ") ?? [];
    return ((Number(stat[11]) + Number(stat[12])) * 1000) / 100;
  } catch {
    return 0;
  }
}

// The scenario: 100 edits of the largest file, each followed by
// `diagnostics` and `hover`.
report.push(
  "",
  "### 3. Speed: 100 edits of the largest file, each followed by `diagnostics` and `hover`",
  "",
  "| Option | Total, median | Range | CPU / wall |",
  "| --- | ---: | --- | ---: |",
);
const edits = check ? 10 : 100;
const scenario = files[1];
if (scenario === undefined) throw new Error("no file");
const pos = position(scenario.text);
/** @type {Record<string, { wall: number[], cpu: number, total: number }>} */
const s = {};
for (let round = 0; round < ROUNDS; round++) {
  for (const option of round % 2 === 0 ? OPTIONS : [...OPTIONS].reverse()) {
    const c = client(option);
    await c.change(scenario.path, scenario.text);
    const own0 = ownCpu();
    const child0 = cpuOf(c.pid);
    const t0 = performance.now();
    for (let e = 0; e < edits; e++) {
      // A comment typed at the end of the file, then removed, and again.
      const text = `${scenario.text}${e % 2 === 0 ? `// ${String(e)}` : ""}`;
      const sent = option === "q1" ? text : undefined;
      await c.change(scenario.path, text);
      await c.ask({ kind: "diagnostics", file: scenario.path, text: sent });
      await c.ask({ kind: "hover", file: scenario.path, at: pos, text: sent });
    }
    const wall = performance.now() - t0;
    // Node's own time and its children's that ended (q1's processes), and
    // q2's process, still running.
    const used = ownCpu() - own0 + (cpuOf(c.pid) - child0);
    c.close();
    const r = (s[option] ??= { wall: [], cpu: 0, total: 0 });
    r.wall.push(wall);
    r.cpu += used;
    r.total += wall;
  }
}
for (const option of OPTIONS) {
  const r = s[option];
  if (r === undefined) continue;
  report.push(
    `| ${option} | ${pct(r.wall, 0.5).toFixed(0)} ms | ${Math.min(...r.wall).toFixed(0)} to ${Math.max(...r.wall).toFixed(0)} ms | ${(r.cpu / r.total).toFixed(2)} |`,
  );
}

// 4. Memory.
const corpus = spawnSync("git", ["ls-files", "*.peras", "*.apeiron"], {
  cwd: root,
  encoding: "utf8",
})
  .stdout.split("\n")
  .filter((f) => f !== "");
/** The peak of memory of a process so far, in MB. @param {number | undefined} pid */
function peak(pid) {
  try {
    const kb = /VmHWM:\s+(\d+) kB/.exec(readFileSync(`/proc/${String(pid)}/status`, "utf8"))?.[1];
    return Number(kb) / 1024;
  } catch {
    return Number.NaN;
  }
}
const server = q2();
for (const f of corpus) await server.change(join(root, f), readFileSync(join(root, f), "utf8"));
await server.ask({ kind: "hover", file: largestPath, at: pos });
const q2Peak = peak(server.pid);
server.close();
let q1Peak = 0;
{
  const child = spawn(perasc, ["query", "diagnostics", largestPath], {
    env: { ...process.env, PERAS_PROTO_QUERY: "q1" },
    stdio: ["ignore", "ignore", "ignore"],
  });
  const timer = setInterval(() => {
    const m = peak(child.pid);
    if (Number.isFinite(m)) q1Peak = Math.max(q1Peak, m);
  }, 1);
  await new Promise((resolve) => child.on("exit", resolve));
  clearInterval(timer);
}
report.push(
  "",
  "### 4. Memory",
  "",
  "| Option | Peak |",
  "| --- | ---: |",
  `| q1, one process on the largest file (sampled every millisecond) | ${q1Peak.toFixed(1)} MB |`,
  `| q2, the ${String(corpus.length)} Peras files of the repository opened | ${q2Peak.toFixed(1)} MB |`,
);

console.log(report.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# Queries for the tools: q1 against q2\n\nGenerated by \`node scripts/bench-query.mjs\`, for [decision 0042](../../docs/en/decisions/0042-queries.md).\n\n${report.join("\n")}\n`,
  );
}
