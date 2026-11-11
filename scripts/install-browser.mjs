// Installs a browser of the pinned Playwright with its system packages, and
// tries again when the runner's network fails: the system packages come from
// the Ubuntu archives, which a runner sometimes cannot resolve for minutes
// (finding F107). A failure of Playwright itself still fails at the end.
// Usage: node scripts/install-browser.mjs chromium|webkit
// No dependencies: runs on a bare Node install.
import { spawnSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const browser = process.argv[2];
if (browser !== "chromium" && browser !== "webkit") {
  console.error("Usage: node scripts/install-browser.mjs chromium|webkit");
  process.exit(2);
}

const attempts = 5;
for (let attempt = 1; attempt <= attempts; attempt++) {
  const run = spawnSync("pnpm", ["exec", "playwright", "install", "--with-deps", browser], {
    stdio: "inherit",
  });
  if (run.status === 0) process.exit(0);
  if (attempt === attempts) break;
  const wait = 30 * attempt;
  console.log(
    `::warning::installing ${browser} failed (attempt ${String(attempt)} of ${String(attempts)}); again in ${String(wait)} s`,
  );
  await sleep(wait * 1000);
  // The package lists may name mirrors that changed while the network was down.
  spawnSync("sudo", ["apt-get", "update"], { stdio: "inherit" });
}
console.error(`installing ${browser} failed ${String(attempts)} times`);
process.exit(1);
