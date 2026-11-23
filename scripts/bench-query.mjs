// Decision 0042: queries for the tools. q2, chosen, is `perasc serve`, one
// process for an editor's session; q1 (`perasc query`, one process a query)
// was measured against it at commit ca5dd0e and removed once not chosen: its
// figures are in the decision and in bench/query/RESULTS.md. perasc is built
// for release (finding M029).
//
// 1. Answers: bench/query/queries.json, fifty queries on six programs, each
//    answered as written by hand.
// 2. A file being typed: ten queries on the same programs with a syntax error
//    before the position.
// 3. A file follows the text of the file it imports, saved or not.
// 4. Speed and load on the processor: each kind of query on three files, the
//    median time from the request to the answer and its 95th percentile; a
//    scenario of 100 edits, each followed by `diagnostics` and `hover`, its
//    CPU time over its wall time.
// 5. Memory: the peak of the process with every Peras file of the
//    repository opened, read from Linux (`VmHWM` of `/proc/<pid>/status`).
//
// Usage: node scripts/bench-query.mjs [--check] [--rounds N]
//        --check: points 1 to 3 checked, one round of the rest, nothing
//        written.
// Writes bench/query/CURRENT.md, unless --check: RESULTS.md keeps the
// options as decision 0042 measured them.
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { basename, join } from "node:path";
import { performance } from "node:perf_hooks";
import { createInterface } from "node:readline";

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
 *   includes?: string[], names?: string[], codes?: string[], items?: unknown[], unchanged?: boolean }} Answer
 * @typedef {{ file: string, kind: string, at?: string, expect: Answer,
 *   replace?: { line: number, text: string } }} Entry
 */

/** The client of `perasc serve`: one process, a request a line, an answer a line. */
function serve() {
  const child = spawn(perasc, ["serve"], { stdio: ["pipe", "pipe", "inherit"] });
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
    /** @param {Query} q */
    async ask(q) {
      if (q.text !== undefined) await send({ method: "change", file: q.file, text: q.text });
      return send({ method: "query", file: q.file, kind: q.kind, at: q.at });
    },
    /** @param {string} file @param {string} text */
    async change(file, text) {
      await send({ method: "change", file, text });
    },
    /** @param {string} file */
    async close(file) {
      await send({ method: "close", file });
    },
    end: () => child.stdin.end(),
  };
}

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
  "### 1. Answers, 2. a file being typed, 3. a file follows the file it imports",
  "",
];
{
  const c = serve();
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
  // 3. report.peras imports `summarize` from stats.peras: renamed there, not
  // saved, report.peras has an error; named back, it has none.
  const stats = readFileSync(program("stats.peras"), "utf8");
  const report0 = /** @type {Answer} */ (
    await c.ask({ kind: "diagnostics", file: program("report.peras") })
  );
  await c.change(program("stats.peras"), stats.replace("fn summarize(", "fn summary("));
  const report1 = /** @type {Answer} */ (
    await c.ask({ kind: "diagnostics", file: program("report.peras") })
  );
  await c.close(program("stats.peras"));
  const report2 = /** @type {Answer} */ (
    await c.ask({ kind: "diagnostics", file: program("report.peras") })
  );
  const follows =
    report0.codes?.length === 0 &&
    (report1.codes?.length ?? 0) > 0 &&
    // Each error also as `perasc check --json` writes it, for an editor.
    report1.items?.length === report1.codes?.length &&
    report2.codes?.length === 0;
  c.end();
  report.push(
    `- ${String(good)} of ${String(expected.queries.length)} answers right; ${String(typed)} of ${String(expected.typing.length)} on a file being typed.`,
    `- A file follows the text of the file it imports: ${follows ? "yes" : "no"} (its errors ${JSON.stringify(report0.codes)}, then ${JSON.stringify(report1.codes)} with the imported file changed and not saved, then ${JSON.stringify(report2.codes)} once it is closed).`,
  );
  if (check && (good !== expected.queries.length || typed !== expected.typing.length || !follows))
    throw new Error(`wrong answers: ${wrong.join("; ")}\n${report.join("\n")}`);
}

// 4. Speed. Three files: 50 lines, the largest of the repository, 5,000.
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
  "### 4. Speed: each kind of query, median (95th percentile), ms",
  "",
  `| File | ${KINDS.join(" | ")} |`,
  `| --- |${KINDS.map(() => " ---: |").join("")}`,
);
for (const f of files) {
  const pos = position(f.text);
  /** @type {Record<string, number[]>} */
  const t = {};
  const c = serve();
  await c.change(f.path, f.text);
  for (let round = 0; round < ROUNDS; round++) {
    for (const kind of KINDS) {
      const t0 = performance.now();
      await c.ask({ kind, file: f.path, at: pos });
      (t[kind] ??= []).push(performance.now() - t0);
    }
  }
  c.end();
  const cells = KINDS.map((k) => {
    const xs = t[k] ?? [];
    return `${pct(xs, 0.5).toFixed(1)} (${pct(xs, 0.95).toFixed(1)})`;
  });
  report.push(`| ${f.name} | ${cells.join(" | ")} |`);
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
  "### 4. Speed: 100 edits of the largest file, each followed by `diagnostics` and `hover`",
  "",
  "| Total, median | Range | CPU / wall |",
  "| ---: | --- | ---: |",
);
const edits = check ? 10 : 100;
const scenario = files[1];
if (scenario === undefined) throw new Error("no file");
const pos = position(scenario.text);
const walls = [];
let cpu = 0;
let total = 0;
for (let round = 0; round < ROUNDS; round++) {
  const c = serve();
  await c.change(scenario.path, scenario.text);
  const own0 = process.cpuUsage();
  const child0 = cpuOf(c.pid);
  const t0 = performance.now();
  for (let e = 0; e < edits; e++) {
    // A comment typed at the end of the file, then removed, and again.
    const text = `${scenario.text}${e % 2 === 0 ? `// ${String(e)}` : ""}`;
    await c.change(scenario.path, text);
    await c.ask({ kind: "diagnostics", file: scenario.path });
    await c.ask({ kind: "hover", file: scenario.path, at: pos });
  }
  const wall = performance.now() - t0;
  const own = process.cpuUsage(own0);
  cpu += (own.user + own.system) / 1000 + (cpuOf(c.pid) - child0);
  total += wall;
  walls.push(wall);
  c.end();
}
report.push(
  `| ${pct(walls, 0.5).toFixed(0)} ms | ${Math.min(...walls).toFixed(0)} to ${Math.max(...walls).toFixed(0)} ms | ${(cpu / total).toFixed(2)} |`,
);

// 5. Memory.
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
const server = serve();
for (const f of corpus) await server.change(join(root, f), readFileSync(join(root, f), "utf8"));
await server.ask({ kind: "hover", file: largestPath, at: pos });
const top = peak(server.pid);
server.end();
report.push(
  "",
  "### 5. Memory",
  "",
  `The peak of the process with the ${String(corpus.length)} Peras files of the repository opened: ${top.toFixed(1)} MB.`,
);

console.log(report.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "CURRENT.md"),
    `# Queries for the tools: the build of today\n\nGenerated by \`node scripts/bench-query.mjs\`, for [decision 0042](../../docs/en/decisions/0042-queries.md).\n\n${report.join("\n")}\n`,
  );
}
