// Fails when a README falls behind the repository (finding M040): every
// decision of docs/en/decisions must be linked from README.md, and its
// French page from README.fr.md; every benchmark script, scripts/bench-*.mjs,
// must be named in bench/README.md and bench/README.fr.md.
// No dependencies: runs on a bare Node install.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");
/** @type {string[]} */
const problems = [];

/** @param {string} path */
const read = (path) => readFileSync(join(root, path), "utf8");

const decisions = readdirSync(join(root, "docs", "en", "decisions")).filter((f) =>
  /^\d{4}-.*\.md$/.test(f),
);
/** @type {[string, string][]} */
const readmes = [
  ["README.md", "en"],
  ["README.fr.md", "fr"],
];
for (const [readme, lang] of readmes) {
  const text = read(readme);
  for (const d of decisions)
    if (!text.includes(`docs/${lang}/decisions/${d}`))
      problems.push(`${readme} does not link docs/${lang}/decisions/${d}`);
}

const scripts = readdirSync(join(root, "scripts"))
  .filter((f) => /^bench-.*\.mjs$/.test(f))
  .map((f) => f.replace(/\.mjs$/, ""));
for (const readme of ["bench/README.md", "bench/README.fr.md"]) {
  const text = read(readme);
  for (const s of scripts)
    if (!new RegExp(`\\b${s}(\\.mjs)?\\b(?!-)`).test(text))
      problems.push(`${readme} does not name scripts/${s}.mjs`);
}

if (problems.length > 0) {
  for (const p of problems) console.error(p);
  process.exit(1);
}
console.log(
  `READMEs up to date: ${String(decisions.length)} decisions, ${String(scripts.length)} benchmarks.`,
);
