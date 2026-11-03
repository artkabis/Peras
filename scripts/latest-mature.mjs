// Prints `name@version` for the newest stable release of each package that is old enough
// to pass pnpm's minimumReleaseAge policy (read from pnpm-workspace.yaml).
// Usage: node scripts/latest-mature.mjs vitest typescript@6.0 ...
// A `@<prefix>` suffix keeps the choice within that version line (e.g. 6.0 → 6.0.x).
// No dependencies: runs on a bare Node install.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");
const workspace = readFileSync(join(root, "pnpm-workspace.yaml"), "utf8");
const ageMatch = /^minimumReleaseAge:\s*(\d+)\s*$/m.exec(workspace);
if (!ageMatch?.[1]) {
  console.error("pnpm-workspace.yaml must set minimumReleaseAge (in minutes).");
  process.exit(1);
}
const cutoff = Date.now() - Number(ageMatch[1]) * 60_000;

/**
 * Compares two plain `major.minor.patch` versions.
 * @param {string} a
 * @param {string} b
 * @returns {number}
 */
function compareVersions(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/** @type {string[]} */
const picks = [];
for (const spec of process.argv.slice(2)) {
  const at = spec.lastIndexOf("@");
  const name = at > 0 ? spec.slice(0, at) : spec;
  const line = at > 0 ? spec.slice(at + 1) : "";
  const output = execFileSync("pnpm", ["view", name, "time", "--json"], { encoding: "utf8" });
  const parsed = /** @type {unknown} */ (JSON.parse(output));
  const times = /** @type {Record<string, string>} */ (parsed);
  const candidates = Object.entries(times)
    .filter(([version]) => /^\d+\.\d+\.\d+$/.test(version))
    .filter(([version]) => line === "" || version.startsWith(`${line}.`))
    .filter(([, published]) => Date.parse(published) <= cutoff)
    .map(([version]) => version)
    .sort(compareVersions);
  const best = candidates.at(-1);
  if (best === undefined) {
    console.error(`No mature stable version found for ${spec}.`);
    process.exit(1);
  }
  picks.push(`${name}@${best}`);
}
console.log(picks.join(" "));
