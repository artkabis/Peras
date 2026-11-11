// Installs a browser of the pinned Playwright, then its system packages.
//
// The browser comes from Playwright's own download host; it must install, or
// the job fails. The system packages come from the Ubuntu archives, which a
// runner sometimes cannot resolve for half an hour (finding F107); most are
// on the runner already, and those that were missing in that outage were
// fonts the tests do not need. So they are tried twice, for at most four
// minutes each, and a failure is a warning: if a library the browser needs
// were truly missing, the tests that launch it would fail, naming it.
// Usage: node scripts/install-browser.mjs chromium|webkit
// No dependencies: runs on a bare Node install.
import { spawnSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const browser = process.argv[2];
if (browser !== "chromium" && browser !== "webkit") {
  console.error("Usage: node scripts/install-browser.mjs chromium|webkit");
  process.exit(2);
}

/**
 * Runs pnpm exec playwright with `args`, at most `minutes`; true when it succeeded.
 * @param {string[]} args
 * @param {number} minutes
 * @returns {boolean}
 */
function playwright(args, minutes) {
  const run = spawnSync("pnpm", ["exec", "playwright", ...args], {
    stdio: "inherit",
    timeout: minutes * 60_000,
  });
  return run.status === 0;
}

for (let attempt = 1; ; attempt++) {
  if (playwright(["install", browser], 10)) break;
  if (attempt === 3) {
    console.error(`downloading ${browser} failed 3 times`);
    process.exit(1);
  }
  console.log(
    `::warning::downloading ${browser} failed (attempt ${String(attempt)}); again in 30 s`,
  );
  await sleep(30_000);
}

for (let attempt = 1; attempt <= 2; attempt++) {
  if (playwright(["install-deps", browser], 4)) process.exit(0);
  if (attempt === 1) {
    console.log("::warning::the system packages did not install; again in 30 s");
    await sleep(30_000);
    spawnSync("sudo", ["apt-get", "update"], { stdio: "inherit", timeout: 120_000 });
  }
}
console.log(
  `::warning::the system packages of ${browser} did not install (the runner's network, F107); going on with those the runner has`,
);
