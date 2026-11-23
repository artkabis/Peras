// Decision 0041: a formatter. f1 (tokens, the author's line breaks kept) and
// f2 (a printer at a width of 100), both prototypes of `perasc fmt` behind
// `PERAS_PROTO_FMT`, measured on the repository's Peras files. f0 is the
// repository as it is; f3 (a plugin of Prettier) is not built (the decision
// says why). Every figure is of the release build of perasc (finding M029).
//
// 1. Gates: every file formatted, then formatted again, changes no more
//    (idempotent); a file that builds for production builds into the same
//    module, byte for byte; a file the checker refuses is refused with the
//    same codes; a file with a syntax error is refused and left as it is.
//    That its tokens and comments are kept is the Rust test
//    `format_corpus` of perasc-syntax.
// 2. Every file looks the same: bench/fmt/same, six programs written twice;
//    whether each option gives one text for both; the lines longer than 100
//    left in the corpus.
// 3. What each option changes in the corpus: lines and files.
// 4. Speed and load on the processor: the corpus with `--check`, both
//    options alternating, the median ratio round by round and its range,
//    the CPU time over the wall time; the largest file alone.
// 5. Memory: the peak of memory formatting the largest file.
//
// Usage: node scripts/bench-fmt.mjs [--check] [--rounds N]
//        --check: one round, the gates checked, nothing written.
// Writes bench/fmt/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { dirname, join } from "node:path";

const check = process.argv.includes("--check");
const at = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(at > 0 ? process.argv[at + 1] : 11);
const OPTIONS = ["f1", "f2"];
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "fmt");
const work = join(root, "target", "bench-fmt");
const perasc = join(root, "target", "release", "perasc");

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

const corpus = spawnSync("git", ["ls-files", "*.peras", "*.apeiron"], {
  cwd: root,
  encoding: "utf8",
})
  .stdout.split("\n")
  .filter((f) => f !== "");

/** Copies the corpus under `dir`. @param {string} dir */
function copy(dir) {
  for (const f of corpus) {
    mkdirSync(dirname(join(dir, f)), { recursive: true });
    cpSync(join(root, f), join(dir, f));
  }
}

/**
 * Runs perasc fmt on files of `dir`.
 * @param {string} option
 * @param {string} dir
 * @param {string[]} files
 * @param {string[]} extra
 */
function fmt(option, dir, files, extra = []) {
  return spawnSync(perasc, ["fmt", ...extra, ...files], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, PERAS_PROTO_FMT: option },
    maxBuffer: 1 << 26,
  });
}

/**
 * What a build of `file` gives: the module's bytes, or the codes refused.
 * @param {string} file
 * @param {string} out
 */
function build(file, out) {
  const r = spawnSync(
    perasc,
    ["build", file, "--out", out, "--allocator", "tlsf", "--color", "never"],
    { encoding: "utf8" },
  );
  if (r.status !== 0) {
    const codes = [...r.stderr.matchAll(/^error\[(E\d+)\]/gm)].map((m) => m[1]).join(" ");
    return `refused: ${codes}`;
  }
  const stem =
    file
      .split("/")
      .pop()
      ?.replace(/\.(peras|apeiron)$/, "") ?? "";
  const shipped = join(out, file.endsWith(".peras") ? `${stem}.wasm` : `${stem}.js`);
  return existsSync(shipped) ? readFileSync(shipped).toString("base64") : "built, nothing shipped";
}

/** Lines longer than 100. @param {string} dir */
const long = (dir) =>
  corpus.reduce(
    (n, f) =>
      n +
      readFileSync(join(dir, f), "utf8")
        .split("\n")
        .filter((l) => l.length > 100).length,
    0,
  );

const original = join(work, "f0");
copy(original);
const report = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; perasc built for release; ${String(corpus.length)} files.`,
  "",
  "### 1. Gates",
  "",
  "| Option | Refused (syntax) | Not idempotent | Module changed | Refusal changed |",
  "| --- | ---: | ---: | ---: | ---: |",
];
/** @type {Record<string, { refused: string[], changed: string[] }>} */
const results = {};
for (const option of OPTIONS) {
  const dir = join(work, option);
  copy(dir);
  const first = fmt(option, dir, corpus);
  const refused = [...first.stderr.matchAll(/^(.*): refused/gm)].map((m) => m[1] ?? "");
  const again = fmt(option, dir, corpus, ["--check"]);
  const unstable = again.stdout.split("\n").filter((l) => l !== "");
  const modules = [];
  const refusals = [];
  for (const f of corpus) {
    if (refused.includes(f)) continue;
    const before = readFileSync(join(original, f), "utf8");
    const after = readFileSync(join(dir, f), "utf8");
    if (before === after) continue;
    const a = build(join(original, f), join(work, "out", option, "a"));
    const b = build(join(dir, f), join(work, "out", option, "b"));
    if (a !== b) (a.startsWith("refused") ? refusals : modules).push(f);
  }
  const changed = corpus.filter(
    (f) => readFileSync(join(original, f), "utf8") !== readFileSync(join(dir, f), "utf8"),
  );
  results[option] = { refused, changed };
  report.push(
    `| ${option} | ${String(refused.length)} | ${String(unstable.length)}${unstable.length > 0 ? `: ${unstable.slice(0, 3).join(", ")}` : ""} | ${String(modules.length)}${modules.length > 0 ? `: ${modules.slice(0, 3).join(", ")}` : ""} | ${String(refusals.length)}${refusals.length > 0 ? `: ${refusals.slice(0, 3).join(", ")}` : ""} |`,
  );
  if (check && (unstable.length > 0 || modules.length > 0 || refusals.length > 0))
    throw new Error(`${option}: a gate fails\n${report.join("\n")}`);
}
const refusedFiles = [...new Set(Object.values(results).flatMap((r) => r.refused))];
report.push("", `Refused by both for a syntax error: ${refusedFiles.join(", ") || "none"}.`);

// 2. The same program, two layouts.
report.push(
  "",
  "### 2. Every file looks the same",
  "",
  `| Program | ${OPTIONS.join(" | ")} |`,
  `| --- |${OPTIONS.map(() => " --- |").join("")}`,
);
const same = join(bench, "same");
for (let p = 1; p <= 6; p++) {
  const cells = OPTIONS.map((option) => {
    const dir = join(work, "same", option);
    mkdirSync(dir, { recursive: true });
    const texts = ["a", "b"].map((v) => {
      const ext = existsSync(join(same, `p${String(p)}-${v}.peras`)) ? "peras" : "apeiron";
      const file = join(dir, `p${String(p)}-${v}.${ext}`);
      cpSync(join(same, `p${String(p)}-${v}.${ext}`), file);
      const r = fmt(option, dir, [file]);
      if (r.status !== 0) throw new Error(`P${String(p)}${v}, ${option}: ${r.stderr}`);
      return readFileSync(file, "utf8").split("\n");
    });
    const [a = [], b = []] = texts;
    if (a.join("\n") === b.join("\n")) return "one text";
    const inB = new Set(b);
    return `${String(a.filter((l) => !inB.has(l)).length + b.filter((l) => !new Set(a).has(l)).length)} lines differ`;
  });
  report.push(`| P${String(p)} | ${cells.join(" | ")} |`);
}
report.push(
  "",
  `Lines longer than 100 in the corpus: f0 ${String(long(original))}, ${OPTIONS.map((o) => `${o} ${String(long(join(work, o)))}`).join(", ")}.`,
);

// 3. What each option changes.
report.push(
  "",
  "### 3. What it changes in the corpus",
  "",
  "| Option | Files changed | Lines added | Lines removed |",
  "| --- | ---: | ---: | ---: |",
);
for (const option of OPTIONS) {
  const r = spawnSync("git", ["diff", "--no-index", "--numstat", original, join(work, option)], {
    encoding: "utf8",
    maxBuffer: 1 << 26,
  });
  let added = 0;
  let removed = 0;
  for (const line of r.stdout.split("\n")) {
    const [a, d] = line.split("\t");
    added += Number(a) || 0;
    removed += Number(d) || 0;
  }
  report.push(
    `| ${option} | ${String(results[option]?.changed.length ?? 0)} | ${String(added)} | ${String(removed)} |`,
  );
}

// 4. Speed and load, 5. memory.
const sizes = corpus.map((f) => [f, readFileSync(join(root, f)).length]);
sizes.sort((x, y) => Number(y[1]) - Number(x[1]));
const largest = String(sizes[0]?.[0] ?? "");
/**
 * Times a run of perasc fmt --check, with bash's `time`: wall, user, system.
 * @param {string} option
 * @param {string[]} files
 */
function timed(option, files) {
  const r = spawnSync(
    "bash",
    [
      "-c",
      `TIMEFORMAT='%R %U %S'; time "${perasc}" fmt --check "$@" > /dev/null 2>&1`,
      "_",
      ...files,
    ],
    { cwd: original, encoding: "utf8", env: { ...process.env, PERAS_PROTO_FMT: option } },
  );
  const [wall = 0, user = 0, sys = 0] = r.stderr.trim().split(/\s+/).slice(-3).map(Number);
  return { wall: wall * 1000, cpu: (user + sys) * 1000 };
}
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((x, y) => x - y)[Math.floor(xs.length / 2)] ?? Number.NaN;
for (const [label, files] of [
  [`the corpus, ${String(corpus.length)} files`, corpus],
  [`the largest file, ${largest}`, [largest]],
]) {
  /** @type {Record<string, { wall: number[], cpu: number, total: number }>} */
  const t = {};
  for (let round = 0; round < ROUNDS; round++) {
    for (const option of round % 2 === 0 ? OPTIONS : [...OPTIONS].reverse()) {
      const r = timed(option, /** @type {string[]} */ (files));
      const s = (t[option] ??= { wall: [], cpu: 0, total: 0 });
      s.wall.push(r.wall);
      s.cpu += r.cpu;
      s.total += r.wall;
    }
  }
  report.push(
    "",
    `### 4. Speed: ${String(label)}`,
    "",
    "| Option | Median | / f1, median (range) | CPU / wall |",
    "| --- | ---: | --- | ---: |",
  );
  for (const option of OPTIONS) {
    const s = t[option];
    if (s === undefined) continue;
    const ratios = s.wall.map((w, k) => w / (t.f1?.wall[k] ?? Number.NaN));
    report.push(
      `| ${option} | ${median(s.wall).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${(s.cpu / s.total).toFixed(2)} |`,
    );
  }
}
report.push(
  "",
  "### 5. Memory: the peak formatting the largest file",
  "",
  "| Option | Peak |",
  "| --- | ---: |",
);
for (const option of OPTIONS) {
  const r = spawnSync(perasc, ["fmt", "--check", largest], {
    cwd: original,
    encoding: "utf8",
    env: { ...process.env, PERAS_PROTO_FMT: option, PERAS_FMT_PEAK: "1" },
  });
  const kb = Number(/VmHWM:\s+(\d+) kB/.exec(r.stderr)?.[1]);
  report.push(
    `| ${option} | ${Number.isFinite(kb) ? `${(kb / 1024).toFixed(1)} MB` : "not measured"} |`,
  );
}

console.log(report.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# A formatter: f1 against f2\n\nGenerated by \`node scripts/bench-fmt.mjs\`, for [decision 0041](../../docs/en/decisions/0041-formatter.md).\n\n${report.join("\n")}\n`,
  );
}
