// The cost of a call into another WebAssembly instance, as Peras's module
// calls its runtime, against a call inside the module and the work written
// inline (decision 0016). The modules come from
// crates/perasc-runtime/examples/call_cost.rs; they are timed in Chromium, the
// three alternating, median of 10 rounds after 2.
// Usage: node scripts/call-cost.mjs
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright";

const root = join(import.meta.dirname, "..");
const dir = join(root, "target", "call-cost");
const built = spawnSync(
  "cargo",
  ["run", "-q", "--release", "-p", "perasc-runtime", "--example", "call_cost", "--", dir],
  { cwd: root, stdio: "inherit" },
);
if (built.status !== 0) process.exit(1);

/** @type {Record<string, number[]>} */
const files = Object.fromEntries(
  ["callee", "import", "internal", "inline"].map((n) => [
    n,
    [...readFileSync(join(dir, `${n}.wasm`))],
  ]),
);

// The measure, evaluated in the page, where WebAssembly and TextEncoder are.
const measure = `async function measure(wasm) {
  const memory = new WebAssembly.Memory({ initial: 1 });
  const text = new TextEncoder().encode(
    "atrium,boiler,cellar,dock,garage,hall,lab,loft,office,roof,server,vault,héllo,日本".repeat(40),
  );
  new Uint8Array(memory.buffer).set(text);
    const load = async (name, imports) =>
    (await WebAssembly.instantiate(new Uint8Array(wasm[name] ?? []), imports)).instance.exports;
  const rt = await load("callee", { env: { memory } });
    const walk = {};
  for (const how of ["import", "internal", "inline"])
    walk[how] = (
      (await load(how, { env: { memory }, rt })).walk
    );
  const characters = [...new TextDecoder().decode(text)].length;
  const rounds = 4000;
    const times = { import: [], internal: [], inline: [] };
  const sums = new Set();
  for (let r = 0; r < 12; r++)
    for (const how of r % 2 === 0
      ? ["import", "internal", "inline"]
      : ["inline", "internal", "import"]) {
      const start = performance.now();
      sums.add(walk[how]?.(text.length, rounds));
      if (r >= 2) times[how]?.push(performance.now() - start);
    }
  const median = (xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1] ?? 0;
  return {
    same: sums.size === 1,
    ns: Object.fromEntries(
      Object.entries(times).map(([how, xs]) => [how, (median(xs) * 1e6) / (characters * rounds)]),
    ),
  };
}`;

const browser = await chromium.launch(
  process.env.PERAS_CHROMIUM === undefined ? {} : { executablePath: process.env.PERAS_CHROMIUM },
);
const page = await browser.newPage();
/** @typedef {{ same: boolean, ns: Record<string, number> }} Measured */
const inChromium = /** @type {Measured} */ (
  await page.evaluate(`(${measure})(${JSON.stringify(files)})`)
);
const version = browser.version();
await browser.close();
const ns = (/** @type {string} */ how) => (inChromium.ns[how] ?? 0).toFixed(2);
console.log(
  `Chromium ${version}: same result ${String(inChromium.same)}; per character, through an import ${ns("import")} ns, a call inside the module ${ns("internal")} ns, inline ${ns("inline")} ns`,
);
