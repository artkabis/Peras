// Web application bugs: each case in bench/web-bugs is a bug of a web
// application written twice, in TypeScript (`.ts`) and in a managed Peras
// module (`.apeiron`), as each is naturally written. For each, the script
// finds the earliest point where the toolchain stops it:
//   1. compile time: tsc with the strictest options, then typescript-eslint's
//      strict type-aware preset; perasc check;
//   2. run time: the case runs in Chromium, in a page of its own, and throws;
//   3. silent: it runs and gives a wrong result, or keeps listening or ticking.
// TypeScript is compiled with `transpileModule`, Peras with `perasc build`
// for production (no leak detector).
// Usage: node scripts/bench-web-bugs.mjs [--check] [--perasc <path>]
//        --check: fails if Peras lets more bugs through silently than
//        decision 0011 recorded, and writes nothing.
// Writes bench/web-bugs/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { chromium } from "playwright";
import ts from "typescript";

const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "web-bugs");
const check = process.argv.includes("--check");
/** Bugs Peras lets through silently, as decision 0011 recorded: no more. */
const silentAtMost = 2;
const at = process.argv.indexOf("--perasc");
const perasc = at > 0 ? (process.argv[at + 1] ?? "") : join(root, "target", "release", "perasc");
const work = join(root, "target", "bench-web-bugs");

/** @typedef {{ id: string, kind: string, expected: string }} Case */
const casesJson = /** @type {unknown} */ (
  JSON.parse(readFileSync(join(dir, "cases.json"), "utf8"))
);
const cases = /** @type {Case[]} */ (casesJson);

/**
 * @param {string} command
 * @param {string[]} args
 */
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8", maxBuffer: 1 << 26 });
  if (result.error) throw result.error;
  return result;
}

// 1. Compile time.
/** @type {Map<string, string>} */
const tsc = new Map();
for (const line of run("pnpm", [
  "exec",
  "tsc",
  "-p",
  "bench/web-bugs/tsconfig.json",
  "--pretty",
  "false",
]).stdout.split("\n")) {
  const match = /^bench\/web-bugs\/([\w-]+)\.ts\(\d+,\d+\): error (TS\d+): (.*)$/.exec(line);
  if (match?.[1] && match[2] && match[3] && !tsc.has(match[1]))
    tsc.set(match[1], `${match[2]}: ${match[3]}`);
}
/** @type {Map<string, string>} */
const eslint = new Map();
const eslintRun = run("pnpm", [
  "exec",
  "eslint",
  "--no-config-lookup",
  "-c",
  "bench/web-bugs/eslint.config.js",
  "-f",
  "json",
  ...cases.map((c) => `bench/web-bugs/${c.id}.ts`),
]);
/** @typedef {{ filePath: string, messages: { severity: number, ruleId: string | null, message: string }[] }} EslintFile */
const eslintJson = /** @type {unknown} */ (JSON.parse(eslintRun.stdout || "[]"));
for (const file of /** @type {EslintFile[]} */ (eslintJson)) {
  const id = file.filePath.split("/").pop()?.replace(/\.ts$/, "") ?? "";
  const first = file.messages.find((m) => m.severity === 2);
  if (first) eslint.set(id, `${first.ruleId ?? "parse"}: ${first.message}`);
}
/** @type {Map<string, string>} */
const peras = new Map();
const perasRun = run(perasc, [
  "check",
  ...cases.map((c) => `bench/web-bugs/${c.id}.apeiron`),
  "--json",
  "--lang",
  "en",
]);
/** @typedef {{ path: string, diagnostics: { code: string, message: { text: string } }[] }} PerasFile */
const perasJson = /** @type {unknown} */ (JSON.parse(perasRun.stdout));
for (const file of /** @type {{ files: PerasFile[] }} */ (perasJson).files) {
  const id =
    file.path
      .split("/")
      .pop()
      ?.replace(/\.apeiron$/, "") ?? "";
  const first = file.diagnostics[0];
  if (first) peras.set(id, `${first.code}: ${first.message.text}`);
}

// 2. Run time, in Chromium: every case that compiles, in a page of its own.
rmSync(work, { recursive: true, force: true });
for (const side of ["ts", "peras"]) {
  mkdirSync(join(work, side), { recursive: true });
  copyFileSync(join(dir, "lib.js"), join(work, side, "lib.js"));
}
for (const c of cases) {
  const source = readFileSync(join(dir, `${c.id}.ts`), "utf8");
  const js = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  writeFileSync(join(work, "ts", `${c.id}.js`), js);
  if (!peras.has(c.id)) {
    const built = run(perasc, [
      "build",
      `bench/web-bugs/${c.id}.apeiron`,
      "--out",
      join(work, "peras"),
      "--imports",
      "source",
      "--allocator",
      "tlsf",
    ]);
    if (built.status !== 0) throw new Error(`perasc build ${c.id}\n${built.stderr}`);
  }
}
const page = `<!doctype html><meta charset="utf-8"><title>case</title><link rel="icon" href="data:,">
<main><button id="save">Save</button><span id="total"></span></main>`;
const server = createServer((request, response) => {
  const path = decodeURIComponent((request.url ?? "/").split("?")[0] ?? "/");
  if (path === "/") {
    response.writeHead(200, { "content-type": "text/html" });
    response.end(page);
    return;
  }
  try {
    const body = readFileSync(join(work, path));
    response.writeHead(200, {
      "content-type": extname(path) === ".js" ? "text/javascript" : "text/plain",
    });
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
const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });

/**
 * What a case's `main()` gives in a fresh page, or what it throws.
 * @param {string} side
 * @param {string} id
 * @returns {Promise<{ value?: string, error?: string }>}
 */
async function runCase(side, id) {
  const tab = await browser.newPage();
  await tab.goto(`http://localhost:${String(port)}/`);
  const outcome = await tab.evaluate(async (url) => {
    try {
      /** @type {unknown} */
      const loaded = await import(url);
      const m = /** @type {{ main(): unknown }} */ (loaded);
      return { value: String(await m.main()) };
    } catch (e) {
      return {
        error: `${e instanceof Error ? e.name : "Error"}: ${e instanceof Error ? e.message : String(e)}`,
      };
    }
  }, `/${side}/${id}.js`);
  await tab.close();
  return outcome;
}

/** @typedef {{ stage: "compile" | "run" | "silent" | "correct", detail: string }} Verdict */
/** @type {Map<string, { ts: Verdict, peras: Verdict }>} */
const verdicts = new Map();
for (const c of cases) {
  /** @type {Verdict} */
  let tsVerdict;
  const tsError = tsc.get(c.id) ?? eslint.get(c.id);
  if (tsError !== undefined) {
    tsVerdict = { stage: "compile", detail: tsError };
  } else {
    const o = await runCase("ts", c.id);
    tsVerdict =
      o.error !== undefined
        ? { stage: "run", detail: o.error }
        : o.value === c.expected
          ? { stage: "correct", detail: o.value }
          : {
              stage: "silent",
              detail: `gives ${JSON.stringify(o.value)}, expected ${JSON.stringify(c.expected)}`,
            };
  }
  /** @type {Verdict} */
  let perasVerdict;
  const perasError = peras.get(c.id);
  if (perasError !== undefined) {
    perasVerdict = { stage: "compile", detail: perasError };
  } else {
    const o = await runCase("peras", c.id);
    perasVerdict =
      o.error !== undefined
        ? { stage: "run", detail: o.error }
        : o.value === c.expected
          ? { stage: "correct", detail: o.value }
          : {
              stage: "silent",
              detail: `gives ${JSON.stringify(o.value)}, expected ${JSON.stringify(c.expected)}`,
            };
  }
  verdicts.set(c.id, { ts: tsVerdict, peras: perasVerdict });
}
await browser.close();
server.close();

/** @param {Verdict} v */
const cell = (v) => {
  const mark = { compile: "✓ compile", run: "◐ run time", silent: "✗ silent", correct: "· no bug" }[
    v.stage
  ];
  const detail = v.detail.length > 90 ? `${v.detail.slice(0, 89)}…` : v.detail;
  return `${mark}: ${detail.replaceAll("|", "\\|")}`;
};
/** @param {"ts" | "peras"} side @param {string} stage */
const count = (side, stage) => [...verdicts.values()].filter((v) => v[side].stage === stage).length;
const lines = cases.map((c) => {
  const v = verdicts.get(c.id);
  return v === undefined ? "" : `| ${c.id} (${c.kind}) | ${cell(v.ts)} | ${cell(v.peras)} |`;
});
const totals = ["compile", "run", "silent"].map(
  (s) => `| ${s} | ${String(count("ts", s))} | ${String(count("peras", s))} |`,
);
const report = [
  "# Web application bugs — results",
  "",
  "Generated by `node scripts/bench-web-bugs.mjs`. Each case is a bug of a web application, written as it is naturally written in TypeScript (`.ts`) and in a managed Peras module (`.apeiron`). For each, the earliest point where the toolchain stops it: ✓ at compile time (tsc strictest, then typescript-eslint strict type-checked; perasc check), ◐ at run time, by an error, in Chromium, ✗ silent: a wrong result, or a listener or a timer still running.",
  "",
  "| Case | TypeScript | Peras (managed) |",
  "| --- | --- | --- |",
  ...lines,
  "",
  "| Stopped | TypeScript | Peras |",
  "| --- | --: | --: |",
  ...totals,
  "",
].join("\n");
if (check) {
  const silent = count("peras", "silent");
  if (silent > silentAtMost) {
    console.log(report);
    throw new Error(
      `Peras lets ${String(silent)} bugs through silently, more than ${String(silentAtMost)}.`,
    );
  }
  console.log(
    `Peras stops ${String(cases.length - silent)} of ${String(cases.length)} web bugs; TypeScript ${String(cases.length - count("ts", "silent"))}.`,
  );
} else {
  writeFileSync(join(dir, "RESULTS.md"), report);
  console.log(report);
}
