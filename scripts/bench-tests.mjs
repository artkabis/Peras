// Decision 0040: tests written in Peras. v2, chosen, `test` blocks with
// `expect(a).toBe(b)`, `assert` and `assertEqual`, run by vitest-peras
// through `perasc build --tests`, measured against v0 (TypeScript calling
// the exported functions, through vite-plugin-peras); v1 (`assert` and
// `assertEqual` only) was measured at commit d47168a and removed once not
// chosen: its figures are in the decision.
//
// 1. Ease: the lines of each module's tests, blank lines and comments left
//    out.
// 2. Bugs found: each module and three versions of it with a bug planted;
//    for each option, whether its suite fails, and what the failure says.
// 3. Cost: a production build of a module with its tests gives the module
//    without them, byte for byte.
// 5. Speed: the time from the command to the result of each suite, 11 rounds,
//    the median and its range.
//
// Usage: node scripts/bench-tests.mjs [--check] [--rounds N]
//        --check: one round, the suites checked to pass on the modules and
//        fail on the bugs, nothing written.
// Writes bench/tests/CURRENT.md, unless --check: RESULTS.md keeps the
// options as decision 0040 measured them.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";

const check = process.argv.includes("--check");
const at = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(at > 0 ? process.argv[at + 1] : 11);
const OPTIONS = ["v0", "v2"];
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "tests");
const work = join(root, "target", "bench-tests");
const perasc = join(root, "target", "release", "perasc");
const vitest = join(root, "node_modules", "vitest", "vitest.mjs");

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

const MODULES = [
  { name: "m1", ext: "peras", mode: "owned" },
  { name: "m2", ext: "apeiron", mode: "managed" },
  { name: "m3", ext: "peras", mode: "owned" },
];
const VERSIONS = ["", "-b1", "-b2", "-b3"];

/** Lines of code: blank lines and comments left out. @param {string} text */
const lines = (text) =>
  text.split("\n").filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;

/**
 * Sets a suite up: the module, its tests, a configuration of Vitest.
 * @param {{ name: string, ext: string }} m
 * @param {string} version
 * @param {string} option
 */
function suite(m, version, option) {
  const dir = join(work, `${m.name}${version}-${option}`);
  mkdirSync(dir, { recursive: true });
  const source = readFileSync(join(bench, "modules", `${m.name}${version}.${m.ext}`), "utf8");
  const file = `${m.name}.${m.ext}`;
  if (option === "v0") {
    writeFileSync(join(dir, file), source);
    const test = readFileSync(join(bench, "tests", `${m.name}.test.ts`), "utf8").replace(
      "./MODULE",
      `./${file}`,
    );
    writeFileSync(join(dir, `${m.name}.test.ts`), test);
    writeFileSync(
      join(dir, "vitest.config.ts"),
      `import peras from ${JSON.stringify(join(root, "packages", "vite-plugin-peras", "src", "index.ts"))};\nexport default { plugins: [peras({ perasc: ${JSON.stringify(perasc)} })], test: { include: ["*.test.ts"] } };\n`,
    );
  } else {
    const tests = readFileSync(join(bench, "tests", `${m.name}.${option}`), "utf8");
    writeFileSync(join(dir, file), source + tests);
    writeFileSync(
      join(dir, "vitest.config.ts"),
      `import perasTests from ${JSON.stringify(join(root, "packages", "vitest-peras", "src", "index.ts"))};\nexport default { plugins: [perasTests({ perasc: ${JSON.stringify(perasc)} })], test: { include: ["*.${m.ext}"] } };\n`,
    );
  }
  return dir;
}

/**
 * Runs a suite with Vitest: what passed, what failed and why, how long.
 * @param {string} dir
 */
function run(dir) {
  const out = join(dir, "results.json");
  const t0 = performance.now();
  spawnSync(
    process.execPath,
    [
      vitest,
      "run",
      "--root",
      dir,
      "--config",
      join(dir, "vitest.config.ts"),
      "--reporter=json",
      `--outputFile=${out}`,
    ],
    { cwd: root, encoding: "utf8", env: { ...process.env, PERASC: perasc } },
  );
  const ms = performance.now() - t0;
  if (!existsSync(out)) return { passed: 0, failed: -1, failures: ["no result"], ms };
  /** @type {unknown} */
  const parsed = JSON.parse(readFileSync(out, "utf8"));
  const r =
    /** @type {{ numPassedTests: number, numFailedTests: number, testResults: { message?: string, assertionResults: { status: string, failureMessages: string[] }[] }[] }} */ (
      parsed
    );
  const failures = r.testResults.flatMap((f) => [
    ...(f.message !== undefined && f.message !== "" ? [f.message] : []),
    ...f.assertionResults.flatMap((a) => a.failureMessages),
  ]);
  return { passed: r.numPassedTests, failed: r.numFailedTests, failures, ms };
}

/** The first line of a failure, as a person reads it. @param {string} text */
const said = (text) =>
  (text.split("\n").find((l) => l.trim() !== "") ?? "")
    .replace(/^(AssertionError|Error): /, "")
    .replaceAll("|", "\\|")
    .slice(0, 110);

const report = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; tests built with the debug allocator, as perasc builds by default.`,
  "",
  "### 1. Ease: lines of each module's tests (blank lines and comments left out)",
  "",
  `| Module | ${OPTIONS.join(" | ")} |`,
  `| --- |${OPTIONS.map(() => " ---: |").join("")}`,
];
for (const m of MODULES) {
  const cells = OPTIONS.map((option) =>
    String(
      lines(
        readFileSync(
          join(bench, "tests", option === "v0" ? `${m.name}.test.ts` : `${m.name}.${option}`),
          "utf8",
        ),
      ),
    ),
  );
  report.push(`| ${m.name} | ${cells.join(" | ")} |`);
}

report.push(
  "",
  "### 2. Bugs found: what each suite says",
  "",
  `| Version | ${OPTIONS.join(" | ")} |`,
  `| --- |${OPTIONS.map(() => " --- |").join("")}`,
);
/** @type {Record<string, number[]>} */
const times = {};
for (const m of MODULES) {
  for (const version of VERSIONS) {
    const cells = [];
    for (const option of OPTIONS) {
      const dir = suite(m, version, option);
      const r = run(dir);
      const key = `${m.name} ${option}`;
      if (version === "") (times[key] ??= []).push(r.ms);
      if (version === "" && r.failed !== 0)
        throw new Error(`${m.name}, ${option}: fails on the module: ${r.failures.join(" / ")}`);
      if (version !== "" && r.failed === 0) {
        cells.push(`passes: the bug is not seen (${String(r.passed)} tests)`);
        continue;
      }
      cells.push(
        version === "" ? `${String(r.passed)} pass` : `fails: ${said(r.failures[0] ?? "")}`,
      );
    }
    report.push(`| ${m.name}${version === "" ? "" : version} | ${cells.join(" | ")} |`);
  }
}

// 3. Cost: a production build with the tests blanked out is the module.
report.push(
  "",
  "### 3. Cost: a production build with its tests against the module without them",
  "",
  "| Module | v2 |",
  "| --- | --- |",
);
for (const m of MODULES) {
  const cells = [];
  for (const option of ["v2"]) {
    const dir = join(work, `cost-${m.name}-${option}`);
    mkdirSync(dir, { recursive: true });
    const module = join(bench, "modules", `${m.name}.${m.ext}`);
    const withTests = join(dir, `${m.name}.${m.ext}`);
    writeFileSync(
      withTests,
      readFileSync(module, "utf8") +
        readFileSync(join(bench, "tests", `${m.name}.${option}`), "utf8"),
    );
    const plain = join(dir, "plain");
    mkdirSync(plain, { recursive: true });
    copyFileSync(module, join(plain, `${m.name}.${m.ext}`));
    const shipped = m.ext === "peras" ? `${m.name}.wasm` : `${m.name}.js`;
    const build = (
      /** @type {string} */ file,
      /** @type {string} */ out,
      /** @type {string[]} */ extra,
    ) => {
      const r = spawnSync(perasc, [
        "build",
        file,
        "--out",
        out,
        "--allocator",
        "tlsf",
        "--color",
        "never",
        ...extra,
      ]);
      if (r.status !== 0) throw new Error(`${file}: ${String(r.stderr)}`);
      return readFileSync(join(out, shipped));
    };
    const a = build(withTests, join(dir, "a"), []);
    const b = build(join(plain, `${m.name}.${m.ext}`), join(dir, "b"), []);
    const t = build(withTests, join(dir, "t"), ["--tests"]);
    if (!a.equals(b))
      throw new Error(`${m.name}, ${option}: the production build changes with its tests`);
    cells.push(`the same ${String(b.length)} bytes; the test build ${String(t.length)}`);
  }
  report.push(`| ${m.name} | ${cells.join(" | ")} |`);
}

// 5. Speed.
for (let round = 1; round < ROUNDS; round++) {
  for (const m of MODULES) {
    for (const option of round % 2 === 0 ? OPTIONS : [...OPTIONS].reverse()) {
      (times[`${m.name} ${option}`] ??= []).push(run(join(work, `${m.name}-${option}`)).ms);
    }
  }
}
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((x, y) => x - y)[Math.floor(xs.length / 2)] ?? Number.NaN;
report.push(
  "",
  "### 5. Speed: from the command to the result of a suite",
  "",
  `| Module | ${OPTIONS.join(" | ")} |`,
  `| --- |${OPTIONS.map(() => " ---: |").join("")}`,
);
for (const m of MODULES) {
  const cells = OPTIONS.map((option) => {
    const xs = times[`${m.name} ${option}`] ?? [];
    return `${(median(xs) / 1000).toFixed(2)} s (${(Math.min(...xs) / 1000).toFixed(2)} to ${(Math.max(...xs) / 1000).toFixed(2)})`;
  });
  report.push(`| ${m.name} | ${cells.join(" | ")} |`);
}

console.log(report.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "CURRENT.md"),
    `# Tests written in Peras: the build of today against v0\n\nGenerated by \`node scripts/bench-tests.mjs\`, for [decision 0040](../../docs/en/decisions/0040-tests.md).\n\n${report.join("\n")}\n`,
  );
}
