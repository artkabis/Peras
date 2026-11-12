// Where the sales dashboard's Peras version spends its time, found by removal:
// each variant of bench/data/dashboard.peras leaves out or replaces one thing,
// and the time it saves is that thing's cost. Not a measure under a protocol:
// a profile, to choose what the compiler should do next (decision 0017). A
// variant changes what the dashboard computes; only its time is read.
// Usage: node scripts/profile-data.mjs [--calls]
// Prints, for 1,000,000 rows, the median of 9 rounds of each variant, the
// variants alternating, and what each saves. With --calls, instead: the
// dashboard built with the runtime beside it (--link off), and each call
// into the runtime counted, per row, on 100,000 rows.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { TextDecoder } from "node:util";

const root = join(import.meta.dirname, "..");
const work = join(root, "target", "profile-data");
const perasc = join(root, "target", "release", "perasc");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/** @param {string} path @returns {Promise<unknown>} */
const load = (path) => import(pathToFileURL(path).href);

if (process.argv.includes("--calls")) {
  const out = join(work, "calls");
  const built = spawnSync(
    perasc,
    [
      "build",
      join(root, "bench", "data", "dashboard.peras"),
      "--allocator",
      "tlsf",
      "--link",
      "off",
      "--out",
      out,
    ],
    { encoding: "utf8" },
  );
  if (built.status !== 0) throw new Error(built.stderr);
  // Each function the module imports from the runtime, wrapped to count.
  const loader = join(out, "dashboard.js");
  const text = readFileSync(loader, "utf8");
  const at = text.indexOf("// perasc:load");
  if (at < 0) throw new Error("the loader has changed: no `// perasc:load`");
  writeFileSync(
    loader,
    text.slice(0, at) +
      "export const $counts = {};\n" +
      "for (const [k, f] of Object.entries($imports.peras_rt)) if (typeof f === 'function') $imports.peras_rt[k] = (...a) => { $counts[k] = ($counts[k] ?? 0) + 1; return f(...a); };\n" +
      text.slice(at),
  );
  const counted =
    /** @type {{ summarize: (text: string) => { kept: number }, $counts: Record<string, number> }} */ (
      await load(loader)
    );
  const generate = /** @type {{ generate: (rows: number) => Uint8Array }} */ (
    await load(join(root, "bench", "data", "generate.ts"))
  );
  const rows = 100_000;
  const summary = counted.summarize(new TextDecoder().decode(generate.generate(rows)));
  console.log(`${String(rows)} rows, ${String(summary.kept)} kept; calls per row kept:`);
  for (const [name, n] of Object.entries(counted.$counts).sort((a, b) => b[1] - a[1]))
    console.log(`${name.padEnd(18)} ${(n / summary.kept).toFixed(2).padStart(6)}`);
  process.exit(0);
}

const source = readFileSync(join(root, "bench", "data", "dashboard.peras"), "utf8");

const pairBlock = `    const category = p % 12;
    const key = \`\${country}|\${category}\`;
    let pair = pairs.get(key).unwrapOr(Pair(country.copy(), category, 0, 0));
    pair.count += 1;
    pair.revenue += revenue;
    pairs.set(key, pair);
`;
const countryBlock = `    if (!countries.has(country)) {
      countries.set(country.copy(), users.length);
      users.push(Set.new());
    }
    users[countries.get(country).unwrapOr(0)].add(user);
`;

/** The lines parsed and filtered, nothing aggregated. @type {[string, string]} */
const parseOnly = [
  "    kept += 1;\n",
  "    kept += 1;\n    if (kept > 0) {\n      continue;\n    }\n",
];

/** Each variant: its name, what it shows, and its replacements. @type {[string, string, [string, string][]][]} */
const variants = [
  ["full", "the dashboard as written", []],
  [
    "boundary",
    "the text passed in and a summary returned, nothing read",
    [
      [
        "  let header = true;\n",
        "  let header = true;\n  if (text.length > 0) {\n    return Summary(0, 0, 0, [], [], [], []);\n  }\n",
      ],
    ],
  ],
  [
    "lines-only",
    "only the walk over the lines",
    [["    lines += 1;\n", "    lines += 1;\n    if (lines > 0) {\n      continue;\n    }\n"]],
  ],
  [
    "parse-only",
    "the lines parsed and filtered, nothing aggregated",
    [["    kept += 1;\n", "    kept += 1;\n    if (kept > 0) {\n      continue;\n    }\n"]],
  ],
  // The reading alone, each variant on top of parse-only: what a variant
  // changes in which rows are kept changes nothing after the filter.
  [
    "parse-no-ints",
    "parse-only, the five integers their lengths, not parsed",
    [
      parseOnly,
      [
        "Row(f[0].parseI32()?, f[1].parseI32()?, f[2].copy(), f[3].parseI32()?, f[4].parseI32()?, f[5].parseI32()?)",
        "Row(f[0].length, f[1].length, f[2].copy(), f[3].length, f[4].length, f[5].length)",
      ],
    ],
  ],
  [
    "parse-no-check",
    "parse-only, without the check of the country's letters",
    [parseOnly, ["  if (f.length != 6 || !isCountry(f[2])) {", "  if (f.length != 6) {"]],
  ],
  [
    "parse-no-split",
    "parse-only, parse returns a row made of the line's length, no split",
    [
      parseOnly,
      ['  const f = line.split(",");\n', ""],
      [
        "  if (f.length != 6 || !isCountry(f[2])) {\n    return None;\n  }\n  return Some(Row(f[0].parseI32()?, f[1].parseI32()?, f[2].copy(), f[3].parseI32()?, f[4].parseI32()?, f[5].parseI32()?));",
        '  if (line.length == 0) {\n    return None;\n  }\n  return Some(Row(line.length, 1, "FR", 2, 3, 1));',
      ],
      ["fn isCountry(", "export fn isCountry("],
    ],
  ],
  [
    "no-country-check",
    "without the check of the country's letters",
    [["  if (f.length != 6 || !isCountry(f[2])) {", "  if (f.length != 6) {"]],
  ],
  ["no-pairs", "without the map of pairs", [[pairBlock, ""]]],
  [
    "pair-key-plain",
    "the pair's key the country itself, no template",
    [["    const key = `${country}|${category}`;\n", "    const key = country.copy();\n"]],
  ],
  [
    "pair-key-no-int",
    "the pair's key a template of the country alone, no integer formatted",
    [["    const key = `${country}|${category}`;\n", "    const key = `${country}|`;\n"]],
  ],
  [
    "pair-fallback-quiet",
    "the pair's fallback without a copy of the country",
    [["unwrapOr(Pair(country.copy(), category, 0, 0))", 'unwrapOr(Pair("", category, 0, 0))']],
  ],
  [
    "no-countries",
    "without the map of countries and the sets of users",
    [[countryBlock, "    kept += user * 0;\n"]],
  ],
  [
    "no-user-add",
    "the country looked up, no user added to its set",
    [
      [
        "    users[countries.get(country).unwrapOr(0)].add(user);\n",
        "    kept += countries.get(country).unwrapOr(0) * 0 + user * 0;\n",
      ],
    ],
  ],
  [
    "no-daily",
    "without the daily revenue",
    [["    daily[(t - start) / 86400] += revenue;\n", "    kept += t * 0;\n"]],
  ],
  [
    "no-products",
    "without the map of products",
    [["    products.set(p, products.get(p).unwrapOr(0) + revenue);\n", ""]],
  ],
  [
    "one-country-copy",
    "the row's country borrowed, not copied again",
    [
      ["    const country = row.country.copy();\n", ""],
      [pairBlock, pairBlock.replace(/\bcountry\b/g, "row.country")],
      [countryBlock, countryBlock.replace(/\bcountry\b/g, "row.country")],
    ],
  ],
];

for (const [name, , replacements] of variants) {
  let text = source;
  for (const [from, to] of replacements) {
    if (text.split(from).length !== 2)
      throw new Error(`${name}: the text to replace is not there once`);
    text = text.replace(from, to);
  }
  const file = join(work, `${name}.peras`);
  writeFileSync(file, text);
  const built = spawnSync(
    perasc,
    ["build", file, "--allocator", "tlsf", "--out", join(work, name)],
    {
      encoding: "utf8",
    },
  );
  if (built.status !== 0) throw new Error(`${name}: ${built.stderr}`);
}

/** @typedef {{ summarize: (text: string) => unknown }} Module */
const generator = /** @type {{ generate: (rows: number) => Uint8Array }} */ (
  await load(join(root, "bench", "data", "generate.ts"))
);
const text = new TextDecoder().decode(generator.generate(1_000_000));
/** @type {Record<string, Module>} */
const modules = {};
for (const [name] of variants)
  modules[name] = /** @type {Module} */ (await load(join(work, name, `${name}.js`)));
/** @type {Record<string, number[]>} */
const times = Object.fromEntries(variants.map(([name]) => [name, []]));
for (let round = 0; round < 11; round++) {
  const order = round % 2 === 0 ? variants : [...variants].reverse();
  for (const [name] of order) {
    const module = modules[name];
    if (module === undefined) continue;
    const start = performance.now();
    module.summarize(text);
    if (round >= 2) times[name]?.push(performance.now() - start);
  }
}
const median = (/** @type {number[]} */ xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1] ?? 0;
const full = median(times.full ?? []);
for (const [name, what] of variants) {
  const t = median(times[name] ?? []);
  const saved =
    name === "full"
      ? ""
      : `saves ${(full - t).toFixed(0).padStart(4)} ms (${(((full - t) / full) * 100).toFixed(0).padStart(3)} %)`;
  console.log(`${name.padEnd(20)} ${t.toFixed(0).padStart(5)} ms  ${saved}  ${what}`);
}
