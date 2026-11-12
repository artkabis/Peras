// A/B of the sales dashboard's Peras version (bench/data/dashboard.peras)
// between two compilers, in one Node process: the same text summarized by
// each in turn, the order alternating, and the ratio of their times taken
// round by round. A machine whose speed drifts during the run moves both
// sides of a round alike, which a comparison of two separate profiles does
// not survive (decision 0017). Not a measure under a protocol: the check of
// one change.
// Usage: node scripts/ab-data.mjs <git ref before> [<git ref after>] [--rows N]
// The ref after is the working tree when it is left out. Each ref is built
// in a worktree of its own under target/ab-data, with its own target folder.
// Prints both medians and the median and range of the ratio before / after,
// after checking that both give the same summary.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { TextDecoder } from "node:util";

const root = join(import.meta.dirname, "..");
const work = join(root, "target", "ab-data");
const rowsAt = process.argv.indexOf("--rows");
const rows = rowsAt < 0 ? 1_000_000 : Number(process.argv[rowsAt + 1]);
const [beforeRef, afterRef] = process.argv
  .slice(2)
  .filter((a, i, all) => a !== "--rows" && all[i - 1] !== "--rows");
if (beforeRef === undefined)
  throw new Error("usage: node scripts/ab-data.mjs <ref before> [<ref after>]");

/** @param {string} command @param {string[]} args @param {string} [cwd] @returns {string} */
function run(command, args, cwd = root) {
  const done = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (done.status !== 0) throw new Error(`${command} ${args.join(" ")}: ${done.stderr}`);
  return done.stdout;
}

/** The perasc of a ref, built once, or of the working tree. @param {string | undefined} ref */
function perascOf(ref) {
  if (ref === undefined) {
    run("cargo", ["build", "--release", "-q", "-p", "perasc"]);
    return join(root, "target", "release", "perasc");
  }
  const sha = run("git", ["rev-parse", ref]).trim();
  const tree = join(work, "src", sha);
  if (!existsSync(tree)) run("git", ["worktree", "add", "--detach", tree, sha]);
  const target = join(work, "cargo", sha);
  const built = spawnSync("cargo", ["build", "--release", "-q", "-p", "perasc"], {
    cwd: tree,
    env: { ...process.env, CARGO_TARGET_DIR: target },
    encoding: "utf8",
  });
  if (built.status !== 0) throw new Error(built.stderr);
  return join(target, "release", "perasc");
}

/** @typedef {{ summarize: (text: string) => unknown }} Module */
/** @param {string} path @returns {Promise<unknown>} */
const load = (path) => import(pathToFileURL(path).href);

/** @type {Record<string, Module>} */
const sides = {};
for (const [name, ref] of [
  ["before", beforeRef],
  ["after", afterRef],
]) {
  const out = join(work, "build", name);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  run(perascOf(ref), [
    "build",
    join(root, "bench", "data", "dashboard.peras"),
    "--allocator",
    "tlsf",
    "--out",
    out,
  ]);
  sides[name ?? ""] = /** @type {Module} */ (await load(join(out, "dashboard.js")));
}
const before = sides.before;
const after = sides.after;
if (before === undefined || after === undefined) throw new Error("a side is missing");

const generator = /** @type {{ generate: (rows: number) => Uint8Array }} */ (
  await load(join(root, "bench", "data", "generate.ts"))
);
const text = new TextDecoder().decode(generator.generate(rows));
/** @param {unknown} summary */
const canonical = (summary) =>
  JSON.stringify(summary, (_, v) =>
    typeof v === "bigint" ? String(v) : /** @type {unknown} */ (v),
  );
if (canonical(before.summarize(text)) !== canonical(after.summarize(text)))
  throw new Error("the two compilers give different summaries");

/** @type {{ before: number[], after: number[] }} */
const times = { before: [], after: [] };
for (let round = 0; round < 23; round++) {
  /** @type {["before" | "after", Module][]} */
  const order =
    round % 2 === 0
      ? [
          ["after", after],
          ["before", before],
        ]
      : [
          ["before", before],
          ["after", after],
        ];
  for (const [name, module] of order) {
    const start = performance.now();
    module.summarize(text);
    if (round >= 3) times[name].push(performance.now() - start);
  }
}
const sorted = (/** @type {number[]} */ xs) => [...xs].sort((a, b) => a - b);
const median = (/** @type {number[]} */ xs) => sorted(xs)[xs.length >> 1] ?? 0;
const ratios = sorted(times.before.map((b, i) => b / (times.after[i] ?? b)));
console.log(
  `${rows.toLocaleString("en")} rows, 20 rounds; the same summary. Before ${median(times.before).toFixed(1)} ms, after ${median(times.after).toFixed(1)} ms; before / after, round by round: median ${median(ratios).toFixed(3)}, from ${(ratios[0] ?? 0).toFixed(3)} to ${(ratios.at(-1) ?? 0).toFixed(3)}.`,
);
