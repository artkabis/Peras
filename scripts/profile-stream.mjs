// Where the stream's Peras digest spends its time, found by removal: each
// variant of examples/stream/src/telemetry.peras leaves out or replaces one
// step, and the time it saves is that step's cost. Not a measure under a
// protocol: a profile, to choose what the compiler should do next (decision
// 0015, findings F207 to F210).
// Usage: node scripts/profile-stream.mjs
// Prints, for 1,000,000 rows in batches of 100,000, the median of 9 rounds of
// each variant, the variants alternating, and what each saves.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const root = join(import.meta.dirname, "..");
const work = join(root, "target", "profile-stream");
const perasc = join(root, "target", "release", "perasc");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
const source = readFileSync(join(root, "examples", "stream", "src", "telemetry.peras"), "utf8");

const zoneLoop = `  for (const c of s.split("")) {
    if (c.toUpperCase() == c) {
      return false;
    }
  }
  return true;`;
const fieldChecks = `  if (f.length != 6 || !isZone(f[2])) {
    return None;
  }
  const ts = f[0].parseI64()?;
  const sensor = f[1].parseI32()?;
  const temp = f[3].parseF64()?;
  const humidity = f[4].parseF64()?;
  const watts = f[5].parseF64()?;
  if (ts < 0 || temp < -60.0 || temp > 80.0 || humidity < 0.0 || humidity > 100.0 || watts < 0.0) {
    return None;
  }
  return Some(Reading(sensor, f[2].copy(), temp, humidity, watts));`;

/** Each variant: its name, what it shows, and its replacements. @type {[string, string, [string, string][]][]} */
const variants = [
  ["full", "the digest as written", []],
  [
    "lines-only",
    "only the walk over the lines",
    [["    lines += 1;\n", "    lines += 1;\n    if (lines > 0) {\n      continue;\n    }\n"]],
  ],
  [
    "no-parse",
    "without parse: a constant reading per line",
    [
      [
        `    const parsed = parse(line);
    if (parsed.isNone()) {
      broken += 1;
      continue;
    }
    const r = parsed.unwrapOr(Reading(0, "", 0.0, 0.0, 0.0));`,
        `    const r = Reading(line.length, "hall", 20.0, 50.0, 1.0);`,
      ],
      ["fn parse(", "export fn parse("],
    ],
  ],
  [
    "parse-no-split",
    "parse returns a constant reading, without split",
    [
      [
        fieldChecks,
        `  if (line.length == 0) {\n    return None;\n  }\n  return Some(Reading(1, "hall", 20.0, 50.0, 1.0));`,
      ],
      ['  const f = line.split(",");\n', ""],
      ["fn isZone(", "export fn isZone("],
    ],
  ],
  [
    "parse-split-only",
    "parse splits the line, then returns a constant reading",
    [
      [
        fieldChecks,
        `  if (f.length != 6) {\n    return None;\n  }\n  return Some(Reading(1, "hall", 20.0, 50.0, 1.0));`,
      ],
      ["fn isZone(", "export fn isZone("],
    ],
  ],
  [
    "no-zone-check",
    "without the check of the zone's letters",
    [["  if (f.length != 6 || !isZone(f[2])) {", "  if (f.length != 6) {"]],
  ],
  [
    "zone-walk-only",
    "the zone's characters walked, not compared",
    [
      [
        zoneLoop,
        `  let n = 0;\n  for (const c of s.split("")) {\n    n += c.length;\n  }\n  return n > 0;`,
      ],
    ],
  ],
  [
    "zone-whole",
    "the zone checked whole: two case changes compared",
    [[zoneLoop, "  return s.toLowerCase() == s && s.toUpperCase() != s;"]],
  ],
  [
    "no-ints",
    "without parseI64 and parseI32",
    [
      [
        "  const ts = f[0].parseI64()?;\n  const sensor = f[1].parseI32()?;",
        "  const ts = f[0].length.toI64();\n  const sensor = f[1].length;",
      ],
    ],
  ],
  [
    "no-floats",
    "without the three parseF64",
    [
      [
        "  const temp = f[3].parseF64()?;\n  const humidity = f[4].parseF64()?;\n  const watts = f[5].parseF64()?;",
        "  const temp = f[3].length.toF64();\n  const humidity = f[4].length.toF64();\n  const watts = f[5].length.toF64();",
      ],
    ],
  ],
  [
    "no-map",
    "without the map from zone to index",
    [
      ["  let index: Map<String, i32> = Map.new();\n", ""],
      [
        `    if (!index.has(r.zone)) {
      index.set(r.zone.copy(), zones.length);
      zones.push(Zone(r.zone.copy(), 0, 0, 0.0, r.temp, r.temp, 0.0, 0.0));
    }
    const at = index.get(r.zone).unwrapOr(0);`,
        `    if (zones.length == 0) {
      zones.push(Zone(r.zone.copy(), 0, 0, 0.0, r.temp, r.temp, 0.0, 0.0));
    }
    const at = r.sensor % zones.length;`,
      ],
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

/** @typedef {{ digest: (text: string) => unknown }} Module */
/** @typedef {{ batch: (first: number, rows: number) => string }} Generator */
/** @param {string} path @returns {Promise<unknown>} */
const load = (path) => import(pathToFileURL(path).href);
const generator = /** @type {Generator} */ (
  await load(join(root, "examples", "stream", "src", "generate.ts"))
);
const texts = Array.from({ length: 10 }, (_, b) => generator.batch(b * 100_000, 100_000));
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
    for (const text of texts) module.digest(text);
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
  console.log(`${name.padEnd(17)} ${t.toFixed(0).padStart(5)} ms  ${saved}  ${what}`);
}
