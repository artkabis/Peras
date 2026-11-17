// Decision 0031's protocol: parallel loops that can draw a frame. Each option
// is a prototype the compiler builds when PERAS_PROTO_PAR names it: p0,
// today's rules; p1, pure calls in a turn; p2, and parts.
//
// 1. Ease of writing: each program as first written on threads
//    (bench/parallel/as-written), built by each option; the version that
//    builds for each option, the lines it changes; P4, the dashboard by
//    chunks, refused by every option.
// 2. The same results: every version, on threads or not, gives the bytes the
//    TypeScript on one thread gives, in Chromium, the page cross-origin
//    isolated.
// 3. W1 the pond, W2 the garden, W3 the blur: 15 runs after 2, the versions
//    alternating, the ratio round by round to Peras on one thread; each
//    version alone in a fresh page for its CPU time, of every thread, over
//    its wall time, and the memory it holds.
// 4. The bytes each version ships.
// Usage: node scripts/bench-parallel.mjs [--check]
//        --check: the builds and the same results, at a small size; nothing
//        timed or written.
// Writes bench/parallel/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";
import ts from "typescript";
import { minify } from "vite";

const check = process.argv.includes("--check");
const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "parallel");
const work = join(root, "target", "bench-parallel");
const perasc = join(root, "target", "release", "perasc");
rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "ts"), { recursive: true });

/**
 * Builds a source; its errors when refused.
 * @param {string} source
 * @param {string} out
 * @param {string} option
 * @param {boolean} threads
 */
function build(source, out, option, threads) {
  const args = ["build", source, "--allocator", "tlsf", "--out", out, "--color", "never"];
  if (threads) args.push("--threads", "on");
  const r = spawnSync(perasc, args, {
    encoding: "utf8",
    env: { ...process.env, PERAS_PROTO_PAR: option },
  });
  const errors = [...r.stderr.matchAll(/^error(?:\[(E\d+)\])?: (.*)$/gm)].map(
    (m) => `${m[1] ?? "bug"}: ${m[2] ?? ""}`,
  );
  return { ok: r.status === 0, errors };
}

// The TypeScript, on one thread and with the rows it draws.
for (const [name, file] of [
  ["pond", join(root, "bench", "lanterns", "pond.ts")],
  ["pond-rows", join(dir, "pond-rows.ts")],
  ["garden", join(root, "bench", "garden", "garden.ts")],
  ["garden-rows", join(dir, "garden-rows.ts")],
  ["blur", join(dir, "blur.ts")],
]) {
  writeFileSync(
    join(work, "ts", `${name ?? ""}.js`),
    ts.transpileModule(readFileSync(file ?? "", "utf8"), {
      compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
    }).outputText,
  );
}
copyFileSync(join(dir, "worker.js"), join(work, "ts", "worker.js"));

// Peras on one thread.
for (const [name, file] of [
  ["pond", join(root, "bench", "lanterns", "pond.peras")],
  ["garden", join(root, "bench", "garden", "garden.peras")],
  ["blur", join(dir, "blur.peras")],
]) {
  const r = build(file ?? "", join(work, "seq", name ?? ""), "p0", false);
  if (!r.ok) throw new Error(`${String(name)}: ${r.errors.join("; ")}`);
}

// 1. Ease of writing. For each program: as written, and the version that
// builds for each option, or none.
/** @typedef {{ id: string, workload: string, natural: string, fixed: Record<string, string | undefined> }} Program */
/** @type {Program[]} */
const programs = [
  {
    id: "P1, the pond",
    workload: "pond",
    natural: join(dir, "as-written", "pond-bands.peras"),
    fixed: { p0: undefined, p1: undefined, p2: join(dir, "pond-bands.peras") },
  },
  {
    id: "P2, the garden",
    workload: "garden",
    natural: join(dir, "as-written", "garden-rows.peras"),
    fixed: {
      p0: undefined,
      p1: join(dir, "garden-pixels.peras"),
      p2: join(dir, "garden-rows.peras"),
    },
  },
  {
    id: "P3, the blur",
    workload: "blur",
    natural: join(dir, "as-written", "blur-rows.peras"),
    fixed: {
      p0: join(dir, "blur-pixels.peras"),
      p1: join(dir, "blur-pixels.peras"),
      p2: join(dir, "blur-rows.peras"),
    },
  },
  {
    id: "P4, the dashboard",
    workload: "",
    natural: join(root, "bench", "data", "dashboard-parallel.peras"),
    fixed: { p0: undefined, p1: undefined, p2: undefined },
  },
];
const options = ["p0", "p1", "p2"];
const code = (/** @type {string} */ f) =>
  readFileSync(f, "utf8")
    .split("\n")
    .filter((l) => l.trim() !== "" && !l.trimStart().startsWith("//"));
/** The lines two sources differ by: removed plus added. */
const changed = (/** @type {string} */ a, /** @type {string} */ b) => {
  const x = code(a);
  const y = code(b);
  return x.filter((l) => !y.includes(l)).length + y.filter((l) => !x.includes(l)).length;
};
const lines = [
  `Machine: ${cpus()[0]?.model ?? "?"}, ${String(cpus().length)} cores; Node ${process.version}; Chromium.`,
  "",
  "### Ease of writing",
  "",
  "| Program | Option | As written | The version that builds | Lines changed |",
  "| --- | --- | --- | --- | ---: |",
];
/** @type {{ workload: string, option: string, name: string }[]} */
const threaded = [];
let n = 0;
for (const p of programs) {
  for (const o of options) {
    n += 1;
    const first = build(p.natural, join(work, "probe", String(n)), o, true);
    const fixed = p.fixed[o];
    let built = "none";
    let count = "-";
    if (fixed !== undefined) {
      const name = fixed.split("/").pop()?.replace(".peras", "") ?? "";
      const out = join(work, o, name);
      const r = build(fixed, out, o, true);
      if (!r.ok) throw new Error(`${p.id} ${o}: ${r.errors.join("; ")}`);
      built = `\`${name}.peras\``;
      count = String(changed(p.natural, fixed));
      if (!threaded.some((t) => t.workload === p.workload && t.option === o))
        threaded.push({ workload: p.workload, option: o, name });
    }
    const as = first.ok ? "builds" : first.errors.slice(0, 1).join("");
    lines.push(`| ${p.id} | ${o} | ${as} | ${built} | ${count} |`);
  }
}
lines.push("");

// 2 and 3. In Chromium.
const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
});
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  const headers = {
    "cross-origin-opener-policy": "same-origin",
    "cross-origin-embedder-policy": "require-corp",
  };
  if (path === "/") {
    response.writeHead(200, { ...headers, "content-type": "text/html" });
    response.end(
      '<!doctype html><meta charset="utf-8"><title>parallel</title><link rel="icon" href="data:,">',
    );
    return;
  }
  try {
    const file = join(work, path);
    response.writeHead(200, { ...headers, "content-type": types[extname(file)] ?? "text/plain" });
    response.end(readFileSync(file));
  } catch {
    response.writeHead(404);
    response.end();
  }
});
await new Promise((resolve) => {
  server.listen(0, "127.0.0.1", () => {
    resolve(undefined);
  });
});
const address = server.address();
const base = `http://127.0.0.1:${String(typeof address === "object" && address !== null ? address.port : 0)}/`;
const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
const WORKERS = 4;
// The workloads: their size, the buffers each version draws into, how each
// version is made in the page.
const sizes = check
  ? { pond: [256, 192, 120], garden: [128, 128], blur: [256, 256] }
  : { pond: [1024, 768, 1000], garden: [1024, 1024], blur: [2048, 2048] };
/** @param {string} workload @param {string[]} names */
const setup = (workload, names) => `
  const sizes = ${JSON.stringify(sizes)};
  const W = ${String(WORKERS)};
  const made = {};
  const names = ${JSON.stringify(names)};
  const shared = (kind, n) => new globalThis[kind](new SharedArrayBuffer(n * globalThis[kind].BYTES_PER_ELEMENT));
  const band = (h, k) => { const rows = Math.ceil(h / W); return [k * rows, Math.min(h, (k + 1) * rows) - 1]; };
  const pool = async (url, name, buffers, args, h) => {
    const ws = await Promise.all(Array.from({ length: W }, () => new Promise((ok) => { const w = new Worker("/ts/worker.js", { type: "module" }); ok(w); })));
    return async () => { await Promise.all(ws.map((w, k) => new Promise((ok) => { w.onmessage = () => ok(); const [top, bottom] = band(h, k); w.postMessage({ url, name, buffers: buffers.map((b) => [b.constructor.name, b.buffer]), args, top, bottom }); }))); };
  };
  const input = (n) => { const a = new Float64Array(n); for (let i = 0; i < n; i++) a[i] = ((i * 2654435761) >>> 0) / 4294967296; return a; };
  for (const name of names) {
    if ("${workload}" === "pond") {
      const [w, h, n] = sizes.pond;
      if (name === "TypeScript") { const m = await import("/ts/pond.js"); const b = [new Int32Array(w * h), new Float64Array(w * h), new Float64Array(3 * w * h)]; made[name] = { draw: () => m.render(...b, w, h, n, 600), out: () => b[0], mem: () => b.reduce((s, x) => s + x.byteLength, 0) }; }
      else if (name === "TypeScript, workers") { const b = [shared("Int32Array", w * h), shared("Float64Array", w * h), shared("Float64Array", 3 * w * h)]; const run = await pool("/ts/pond-rows.js", "render", b, [w, h, n, 600], h); made[name] = { draw: run, out: () => b[0], mem: () => b.reduce((s, x) => s + x.byteLength, 0) }; }
      else { const m = await import(name === "Peras" ? "/seq/pond/pond.js" : "/" + name + ".js"); const b = [m.__perasHeap.array("i32", w * h), m.__perasHeap.array("f64", w * h), m.__perasHeap.array("f64", 3 * w * h)]; made[name] = { draw: name === "Peras" ? () => m.render(...b, w, h, n, 600) : () => m.render(...b, w, h, n, 600, W), out: () => b[0].view(), mem: () => b[0].view().buffer.byteLength }; }
    } else if ("${workload}" === "garden") {
      const [w, h] = sizes.garden;
      if (name === "TypeScript") { const m = await import("/ts/garden.js"); const px = new Uint32Array(w * h); made[name] = { draw: () => m.render(px, w, h, 1, 0.5, 0.5), out: () => px, mem: () => px.byteLength }; }
      else if (name === "TypeScript, workers") { const px = shared("Uint32Array", w * h); const run = await pool("/ts/garden-rows.js", "render", [px], [w, h, 1, 0.5, 0.5], h); made[name] = { draw: run, out: () => px, mem: () => px.byteLength }; }
      else { const m = await import(name === "Peras" ? "/seq/garden/garden.js" : "/" + name + ".js"); const px = m.__perasHeap.array("i32", w * h); made[name] = { draw: () => m.render(px, w, h, 1, 0.5, 0.5), out: () => px.view(), mem: () => px.view().buffer.byteLength }; }
    } else {
      const [w, h] = sizes.blur;
      const src = input(w * h);
      if (name === "TypeScript") { const m = await import("/ts/blur.js"); const o = new Float64Array(w * h); made[name] = { draw: () => m.blur(src, o, w, h), out: () => o, mem: () => src.byteLength + o.byteLength }; }
      else if (name === "TypeScript, workers") { const i = shared("Float64Array", w * h); i.set(src); const o = shared("Float64Array", w * h); const run = await pool("/ts/blur.js", "blur", [i, o], [w, h], h); made[name] = { draw: run, out: () => o, mem: () => i.byteLength + o.byteLength }; }
      else { const m = await import(name === "Peras" ? "/seq/blur/blur.js" : "/" + name + ".js"); const i = m.__perasHeap.array("f64", src); const o = m.__perasHeap.array("f64", w * h); made[name] = { draw: () => m.blur(i, o, w, h), out: () => o.view(), mem: () => o.view().buffer.byteLength }; }
    }
  }
  const hash = async (a) => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", new Uint8Array(a.buffer, a.byteOffset, a.byteLength).slice()))).map((x) => x.toString(16).padStart(2, "0")).join("").slice(0, 16);
`;
const ROUNDS = check ? 1 : 15;
/** @type {Record<string, { names: string[], times: Record<string, number[]>, hashes: Record<string, string>, alone: Record<string, { load: number, mem: number }> }>} */
const results = {};
const cdp = await browser.newBrowserCDPSession();
/** The CPU time of every process of the browser, in seconds. */
const cpuTime = async () =>
  /** @type {{ processInfo: { cpuTime: number }[] }} */ (
    await cdp.send("SystemInfo.getProcessInfo")
  ).processInfo.reduce((s, p) => s + p.cpuTime, 0);
for (const workload of ["pond", "garden", "blur"]) {
  const names = [
    "TypeScript",
    "TypeScript, workers",
    "Peras",
    ...threaded
      .filter((t) => t.workload === workload)
      .map((t) => `${t.option}/${t.name}/${t.name}`),
  ];
  const page = await browser.newPage();
  await page.goto(base);
  const out =
    /** @type {{ times: Record<string, number[]>, hashes: Record<string, string>, isolated: boolean }} */ (
      await page.evaluate(`(async () => {${setup(workload, names)}
    const times = Object.fromEntries(names.map((k) => [k, []]));
    for (let run = 0; run < ${String(ROUNDS + 2)}; run++) {
      const order = run % 2 === 0 ? names : [...names].reverse();
      for (const k of order) {
        const t0 = performance.now();
        await made[k].draw();
        const ms = performance.now() - t0;
        if (run >= 2) times[k].push(ms);
      }
    }
    const hashes = {};
    for (const k of names) hashes[k] = await hash(made[k].out());
    return { times, hashes, isolated: crossOriginIsolated };
  })()`)
    );
  await page.close();
  if (!out.isolated) throw new Error("the page is not cross-origin isolated");
  if (new Set(Object.values(out.hashes)).size !== 1)
    throw new Error(
      `${workload}: the versions give different bytes: ${JSON.stringify(out.hashes)}`,
    );
  /** @type {Record<string, { load: number, mem: number }>} */
  const alone = {};
  if (!check) {
    for (const name of names) {
      const fresh = await browser.newPage();
      await fresh.goto(base);
      await fresh.evaluate(`(async () => {${setup(workload, [name])}
        globalThis.version = made[${JSON.stringify(name)}];
        await version.draw(); await version.draw();
      })()`);
      const c0 = await cpuTime();
      const wall = /** @type {number} */ (
        await fresh.evaluate(`(async () => {
          const t0 = performance.now();
          for (let i = 0; i < 5; i++) await version.draw();
          return performance.now() - t0;
        })()`)
      );
      const cpu = ((await cpuTime()) - c0) * 1000;
      alone[name] = {
        load: cpu / wall,
        mem: /** @type {number} */ (await fresh.evaluate("version.mem()")),
      };
      await fresh.close();
    }
  }
  results[workload] = { names, times: out.times, hashes: out.hashes, alone };
}
await browser.close();
server.close();
console.log(`same bytes in every version: ${Object.keys(results).join(", ")}`);
if (check) {
  console.log(lines.join("\n"));
  process.exit(0);
}

// 4. Bytes shipped, and the tables.
const gz = (/** @type {string | Uint8Array} */ x) => gzipSync(x, { level: 9 }).length;
const minified = async (/** @type {string} */ file) =>
  gz((await minify(file, readFileSync(file, "utf8"))).code);
/** @param {string} name */
/** What a version ships, counted alike for every version (finding M006). */
async function shipped(/** @type {string} */ name, /** @type {string} */ workload) {
  if (name === "TypeScript") return minified(join(work, "ts", `${workload}.js`));
  if (name === "TypeScript, workers")
    return (
      (await minified(join(work, "ts", workload === "blur" ? "blur.js" : `${workload}-rows.js`))) +
      (await minified(join(work, "ts", "worker.js")))
    );
  const js =
    name === "Peras" ? join(work, "seq", workload, `${workload}.js`) : join(work, `${name}.js`);
  if (!existsSync(js)) return undefined;
  return (await minified(js)) + gz(readFileSync(js.replace(/\.js$/, ".wasm")));
}
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
const kb = (/** @type {number} */ b) => `${(b / 1048576).toFixed(1)} MiB`;
for (const [workload, r] of Object.entries(results)) {
  const [w, h, n] = /** @type {Record<string, number[]>} */ (sizes)[workload] ?? [];
  lines.push(
    `### ${workload === "pond" ? `W1, the pond, frame 600 at ${String(w)} × ${String(h)} with ${String(n)} lanterns` : workload === "garden" ? `W2, the garden at ${String(w)} × ${String(h)}` : `W3, the blur of ${String(w)} × ${String(h)}`}`,
    "",
    `SHA-256 of every version's output: \`${Object.values(r.hashes)[0] ?? ""}\`. ${String(WORKERS)} threads.`,
    "",
    "| Version | Median | / Peras on one thread, median (range) | / TypeScript on one thread | CPU / wall, alone | Memory outside the heap | Shipped, gzip |",
    "| --- | ---: | --- | ---: | ---: | ---: | ---: |",
  );
  const seq = r.times.Peras ?? [];
  const tsOne = r.times.TypeScript ?? [];
  for (const name of r.names) {
    const t = r.times[name] ?? [];
    const toSeq = t.map((x, i) => x / (seq[i] ?? Number.NaN));
    const toTs = t.map((x, i) => x / (tsOne[i] ?? Number.NaN));
    const bytes = await shipped(name, workload);
    const a = r.alone[name];
    lines.push(
      `| ${name.includes("/") ? `Peras ${name.split("/")[0] ?? ""}, \`${name.split("/")[1] ?? ""}\`` : name} | ${median(t).toFixed(1)} ms | ${median(toSeq).toFixed(3)} (${Math.min(...toSeq).toFixed(3)} to ${Math.max(...toSeq).toFixed(3)}) | ${median(toTs).toFixed(3)} | ${(a?.load ?? 0).toFixed(2)} | ${kb(a?.mem ?? 0)} | ${bytes === undefined ? "-" : String(bytes)} |`,
    );
  }
  lines.push("");
}
console.log(lines.join("\n"));
writeFileSync(
  join(dir, "RESULTS.md"),
  `# Parallel loops that can draw a frame: the options measured\n\nRun 1, kept with its flaws, is in [run-1](run-1/RESULTS.md).\n\nGenerated by \`node scripts/bench-parallel.mjs\`, for [decision 0031](../../docs/en/decisions/0031-useful-parallel-loops.md).\n\n${lines.join("\n")}`,
);
