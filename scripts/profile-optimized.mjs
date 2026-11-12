// How the sales dashboard's TypeScript optimized by hand (version B of
// decision 0014) gets its speed, found by removal: each variant of
// bench/data/dashboard-optimized.ts replaces one of its techniques by what
// plain code does, and the time it adds is what that technique saves. Also
// times decoding the bytes into a string, which version C pays and B does
// not. Not a measure under a protocol: a profile (decision 0017).
// Usage: node scripts/profile-optimized.mjs
// Prints, for 1,000,000 rows, the median of 9 rounds of each variant.
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { TextDecoder } from "node:util";

const root = join(import.meta.dirname, "..");
const data = join(root, "bench", "data");
const work = join(root, "target", "profile-optimized");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
copyFileSync(join(data, "summary.ts"), join(work, "summary.ts"));
const source = readFileSync(join(data, "dashboard-optimized.ts"), "utf8");

const pairSlots = `    count[slot] = (count[slot] ?? 0) + 1;
    revenue[slot] = (revenue[slot] ?? 0) + r;
`;

/** Each variant: its name, what it shows, and its replacements. @type {[string, string, [string, string][]][]} */
const variants = [
  ["full", "B as written", []],
  [
    "users-in-sets",
    "every user in a Set per country, no bitmap",
    [["    if (user >= 0 && user < BITS) {", "    if (user < 0) {"]],
  ],
  [
    "products-in-map",
    "every product in a Map, no dense array",
    [["    if (product >= 0 && product < DENSE) {", "    if (product < 0) {"]],
  ],
  [
    "pairs-by-number",
    "the pairs in a Map keyed by the slot's number, no dense array",
    [
      [
        pairSlots,
        "    const e = byNumber.get(slot);\n    if (e === undefined) byNumber.set(slot, { n: 1, r });\n    else {\n      e.n++;\n      e.r += r;\n    }\n",
      ],
      [
        "  const field = new Float64Array(6);\n",
        "  const field = new Float64Array(6);\n  const byNumber = new Map<number, { n: number; r: number }>();\n  count[0] = byNumber.size;\n",
      ],
    ],
  ],
  [
    "pairs-by-text",
    "the pairs in a Map keyed by a string built per row, as plain code does",
    [
      [
        pairSlots,
        "    const key = `${String.fromCharCode(A + Math.floor(code / 26), A + (code % 26))}|${String(slot % 23)}`;\n    const e = byText.get(key);\n    if (e === undefined) byText.set(key, { n: 1, r });\n    else {\n      e.n++;\n      e.r += r;\n    }\n",
      ],
      [
        "  const field = new Float64Array(6);\n",
        "  const field = new Float64Array(6);\n  const byText = new Map<string, { n: number; r: number }>();\n  count[0] = byText.size;\n",
      ],
    ],
  ],
];
// All three at once: B's reading of the bytes, with plain code's maps and sets.
variants.push([
  "plain-structures",
  "B's reading of the bytes, with Sets, a Map of products and pairs keyed by strings",
  variants
    .slice(1, 3)
    .flatMap(([, , r]) => r)
    .concat(variants[4]?.[2] ?? []),
]);

for (const [name, , replacements] of variants) {
  let text = source;
  for (const [from, to] of replacements) {
    if (text.split(from).length !== 2)
      throw new Error(`${name}: the text to replace is not there once`);
    text = text.replace(from, to);
  }
  writeFileSync(join(work, `${name}.ts`), text);
}

/** @param {string} path @returns {Promise<unknown>} */
const load = (path) => import(pathToFileURL(path).href);
const generator = /** @type {{ generate: (rows: number) => Uint8Array }} */ (
  await load(join(data, "generate.ts"))
);
const bytes = generator.generate(1_000_000);
/** @type {Record<string, (bytes: Uint8Array) => unknown>} */
const fns = {};
for (const [name] of variants) {
  const module = /** @type {{ summarize: (bytes: Uint8Array) => unknown }} */ (
    await load(join(work, `${name}.ts`))
  );
  fns[name] = module.summarize;
}
const decoder = new TextDecoder();
fns.decode = (b) => decoder.decode(b).length;
const names = [...variants.map(([name]) => name), "decode"];
/** @type {Record<string, number[]>} */
const times = Object.fromEntries(names.map((name) => [name, []]));
for (let round = 0; round < 11; round++) {
  const order = round % 2 === 0 ? names : [...names].reverse();
  for (const name of order) {
    const fn = fns[name];
    if (fn === undefined) continue;
    const start = performance.now();
    fn(bytes);
    if (round >= 2) times[name]?.push(performance.now() - start);
  }
}
const median = (/** @type {number[]} */ xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1] ?? 0;
const full = median(times.full ?? []);
for (const [name, what] of [
  ...variants,
  ["decode", "TextDecoder alone: the bytes into the string C is given"],
]) {
  const t = median(times[name] ?? []);
  const added =
    name === "full" || name === "decode" ? "" : `adds ${(t - full).toFixed(0).padStart(4)} ms`;
  console.log(`${name.padEnd(17)} ${t.toFixed(0).padStart(5)} ms  ${added.padEnd(14)} ${what}`);
}
