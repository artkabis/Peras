// Fails when English text has no French counterpart.
// English is the source of truth; every message key and every doc page must exist in French too.
// No dependencies: runs on a bare Node install.
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = join(import.meta.dirname, "..");
/** @type {string[]} */
const problems = [];

/**
 * Message ids declared in a Fluent file (`key = value` at the start of a line).
 * @param {string} file
 * @returns {Set<string>}
 */
function fluentKeys(file) {
  /** @type {Set<string>} */
  const keys = new Set();
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const match = /^([a-zA-Z][\w-]*)\s*=/.exec(line);
    if (match?.[1]) keys.add(match[1]);
  }
  return keys;
}

/**
 * All files under a directory, as paths relative to it.
 * @param {string} dir
 * @returns {string[]}
 */
function filesUnder(dir) {
  if (!existsSync(dir)) return [];
  /** @type {string[]} */
  const out = [];
  /** @param {string} current */
  const walk = (current) => {
    for (const name of readdirSync(current)) {
      const full = join(current, name);
      if (statSync(full).isDirectory()) walk(full);
      else out.push(relative(dir, full));
    }
  };
  walk(dir);
  return out.sort((a, b) => a.localeCompare(b));
}

// 1. Fluent messages: every crates/*/messages/en.ftl key must be in fr.ftl.
const cratesDir = join(root, "crates");
for (const crate of readdirSync(cratesDir)) {
  const en = join(cratesDir, crate, "messages", "en.ftl");
  if (!existsSync(en)) continue;
  const fr = join(cratesDir, crate, "messages", "fr.ftl");
  if (!existsSync(fr)) {
    problems.push(`${relative(root, fr)} is missing`);
    continue;
  }
  const frKeys = fluentKeys(fr);
  for (const key of fluentKeys(en)) {
    if (!frKeys.has(key)) problems.push(`${relative(root, fr)}: missing key \`${key}\``);
  }
}

// 2. Documentation: every page in docs/en must exist in docs/fr.
const frDocs = new Set(filesUnder(join(root, "docs", "fr")));
for (const page of filesUnder(join(root, "docs", "en"))) {
  if (!frDocs.has(page))
    problems.push(`docs/fr/${page} is missing (translation of docs/en/${page})`);
}

// 3. Root files with a French twin.
for (const [en, fr] of /** @type {[string, string][]} */ ([
  ["README.md", "README.fr.md"],
  ["CONTRIBUTING.md", "CONTRIBUTING.fr.md"],
])) {
  if (existsSync(join(root, en)) && !existsSync(join(root, fr))) {
    problems.push(`${fr} is missing (translation of ${en})`);
  }
}

if (problems.length > 0) {
  console.error(`Translation check failed (${String(problems.length)} problem(s)):`);
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}
console.log("Translation check passed: every English text has its French counterpart.");
