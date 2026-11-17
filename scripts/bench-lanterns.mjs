// The pond of a thousand lanterns (decision 0029), measured under its
// registered protocol: the same scene in TypeScript (bench/lanterns/pond.ts,
// the specification), Peras owned (pond.peras, for production and debug),
// Peras managed (the same source as a .apeiron module) and Rust (rust/,
// wasm-bindgen).
//   1. Same pixels: SHA-256 of each version's render at frames 0, 300, 600.
//   2. Bytes shipped, minified by Vite where it applies, gzip; lines of code;
//      the errors the first Peras version met (as-written/pond.peras); what
//      perasc says of the threads (threads.peras) and of the shared mesh
//      (shared-mesh.peras).
//   3. Time in Chromium: frame 600 at 1024 × 768 with 1,000 lanterns, 15 runs
//      after 2 of warm-up, the versions alternating, the ratios round by
//      round; then each version alone in a fresh page, its CPU time over its
//      wall time and the memory it holds.
//   4. The verdicts of H1 to H7, and a frame of the scene.
// Usage: node scripts/bench-lanterns.mjs [--check] [--no-rust]
//        --check: the versions give the same pixels at 256 × 192 with 120
//        lanterns, at frames 0 and 600; nothing else is measured or written.
//        Rust is left out where cargo or wasm-bindgen is missing.
// Writes bench/lanterns/RESULTS.md, results.json and frame-600.png, unless
// --check.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";
import ts from "typescript";
import { minify } from "vite";

const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "lanterns");
const check = process.argv.includes("--check");
const perasc = join(root, "target", "release", "perasc");
const work = join(root, "target", "bench-lanterns");
rmSync(work, { recursive: true, force: true });
for (const d of ["ts", "managed-src"]) mkdirSync(join(work, d), { recursive: true });

/** @param {string} command @param {string[]} args @param {object} [options] */
function sh(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 28, ...options });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  return result;
}
/**
 * What perasc says of a file it refuses: its error codes, and its first error.
 * @param {string[]} args
 * @param {Record<string, string>} [env]
 */
function refused(args, env = {}) {
  const result = spawnSync(perasc, args, { encoding: "utf8", env: { ...process.env, ...env } });
  if (result.status === 0) throw new Error(`perasc ${args.join(" ")}: accepted, expected refused`);
  const codes = [...result.stderr.matchAll(/^error\[(E\d+)\]: (.*)$/gm)];
  return { codes: codes.map((m) => m[1] ?? ""), first: codes[0]?.[2] ?? "" };
}
/** @param {string} command */
const has = (command) => spawnSync(command, ["--version"], { encoding: "utf8" }).status === 0;

// 1. The versions.
const tsSource = readFileSync(join(dir, "pond.ts"), "utf8");
writeFileSync(
  join(work, "ts", "pond.js"),
  ts.transpileModule(tsSource, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText,
);
const source = join(dir, "pond.peras");
sh(perasc, ["build", source, "--allocator", "tlsf", "--out", join(work, "peras")]);
sh(perasc, ["build", source, "--out", join(work, "debug")]);
copyFileSync(source, join(work, "managed-src", "pond.apeiron"));
// The study measured the managed module's debug build, which its protocol
// did not name (finding M029); named here, so that the run is the study's.
sh(perasc, [
  "build",
  join(work, "managed-src", "pond.apeiron"),
  "--allocator",
  "debug",
  "--out",
  join(work, "managed"),
]);
const withRust = !process.argv.includes("--no-rust") && has("cargo") && has("wasm-bindgen");
if (withRust) {
  const crate = join(dir, "rust");
  // With SIMD, as bench/garden/rust is built and as Peras builds by default.
  sh("cargo", ["build", "--release", "--locked", "--target", "wasm32-unknown-unknown", "-q"], {
    cwd: crate,
    env: { ...process.env, RUSTFLAGS: "-C target-feature=+simd128" },
  });
  sh("wasm-bindgen", [
    join(crate, "target", "wasm32-unknown-unknown", "release", "bench_lanterns_rust.wasm"),
    ...["--target", "web", "--out-dir", join(work, "rust")],
  ]);
}

/** @typedef {(w: number, h: number, n: number, frame: number) => Uint8Array} Render */
/** @typedef {{ render: (...a: unknown[]) => number, __perasHeap: { live: () => number, array: (k: string, n: number) => { view: () => Int32Array, release: () => void } } }} Owned */
/** @type {Record<string, Render>} */
const renders = {};
/** @type {Record<string, Owned>} */
const owned = {};
{
  /** @param {string} file */
  const load = async (file) => {
    /** @type {unknown} */
    const loaded = await import(pathToFileURL(file).href);
    return loaded;
  };
  for (const [name, file] of [
    ["typescript", join(work, "ts", "pond.js")],
    ["peras-managed", join(work, "managed", "pond.js")],
  ]) {
    const m = /** @type {{ render: (...a: unknown[]) => number }} */ (await load(file ?? ""));
    renders[name ?? ""] = (w, h, n, frame) => {
      const pixels = new Int32Array(w * h);
      m.render(pixels, new Float64Array(w * h), new Float64Array(3 * w * h), w, h, n, frame);
      return new Uint8Array(pixels.buffer);
    };
  }
  for (const [name, folder] of [
    ["peras", "peras"],
    ["peras-debug", "debug"],
  ]) {
    const p = /** @type {Owned} */ (await load(join(work, folder ?? "", "pond.js")));
    owned[name ?? ""] = p;
    renders[name ?? ""] = (w, h, n, frame) => {
      const [a, b, c] = [
        p.__perasHeap.array("i32", w * h),
        p.__perasHeap.array("f64", w * h),
        p.__perasHeap.array("f64", 3 * w * h),
      ];
      p.render(a, b, c, w, h, n, frame);
      const out = new Uint8Array(a.view().slice().buffer);
      for (const x of [a, b, c]) x.release();
      return out;
    };
  }
  if (withRust) {
    const r =
      /** @type {{ default: (o: { module_or_path: Buffer }) => Promise<{ memory: { buffer: ArrayBuffer } }>, Frame: new (n: number) => { ptr: () => number, free: () => void }, render: (...a: unknown[]) => number }} */ (
        await load(join(work, "rust", "bench_lanterns_rust.js"))
      );
    const wasm = await r.default({
      module_or_path: readFileSync(join(work, "rust", "bench_lanterns_rust_bg.wasm")),
    });
    renders.rust = (w, h, n, frame) => {
      const buffers = new r.Frame(w * h);
      r.render(buffers, w, h, n, frame);
      const out = new Uint8Array(wasm.memory.buffer, buffers.ptr(), 4 * w * h).slice();
      buffers.free();
      return out;
    };
  }
}

// 2. The same pixels, at every frame of the protocol, and nothing left alive.
const [width, height, count] = check ? [256, 192, 120] : [1024, 768, 1000];
const frames = check ? [0, 600] : [0, 300, 600];
/** @param {Uint8Array} p */
const sha = (p) => createHash("sha256").update(p).digest("hex");
/** @type {{ frame: number, hash: string }[]} */
const identity = [];
/** @type {Uint8Array | undefined} */
let last;
for (const frame of frames) {
  /** @type {Record<string, string>} */
  const hashes = {};
  for (const [name, render] of Object.entries(renders)) {
    const pixels = render(width, height, count, frame);
    hashes[name] = sha(pixels);
    if (name === "typescript") last = pixels;
  }
  if (new Set(Object.values(hashes)).size !== 1)
    throw new Error(
      `frame ${String(frame)}: the versions give different pixels: ${JSON.stringify(hashes)}`,
    );
  identity.push({ frame, hash: hashes.typescript ?? "" });
}
for (const [name, p] of Object.entries(owned))
  if (p.__perasHeap.live() !== 0)
    throw new Error(`${name}: ${String(p.__perasHeap.live())} blocks left alive`);
const versions = Object.keys(renders);
console.log(
  `same pixels: ${versions.join(", ")}, at frames ${frames.join(", ")} (${String(width)} × ${String(height)}, ${String(count)} lanterns); nothing left alive`,
);
if (check) process.exit(0);

// 3. Bytes, code, and what perasc refuses.
const gz = (/** @type {string | Uint8Array} */ x) => gzipSync(x, { level: 9 }).length;
const minified = async (/** @type {string} */ file) =>
  gz((await minify(file, readFileSync(file, "utf8"))).code);
/** @type {Record<string, number>} */
const shipped = {
  typescript: await minified(join(work, "ts", "pond.js")),
  peras:
    (await minified(join(work, "peras", "pond.js"))) +
    gz(readFileSync(join(work, "peras", "pond.wasm"))),
  "peras-managed": await minified(join(work, "managed", "pond.js")),
};
if (withRust)
  shipped.rust =
    (await minified(join(work, "rust", "bench_lanterns_rust.js"))) +
    gz(readFileSync(join(work, "rust", "bench_lanterns_rust_bg.wasm")));
const lines = (/** @type {string} */ s) =>
  s.split("\n").filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;
/** @type {Record<string, number>} */
const code = {
  typescript: lines(tsSource),
  peras: lines(readFileSync(source, "utf8")),
};
if (withRust) code.rust = lines(readFileSync(join(dir, "rust", "src", "lib.rs"), "utf8"));
// The first version as it met the compiler of decision 0029: decision 0030
// made it build, so its errors are counted with that compiler's literals.
const asWritten = refused(["check", join(dir, "as-written", "pond.peras")], {
  PERAS_PROTO_NUM: "a",
});
const threads = refused([
  "build",
  join(dir, "threads.peras"),
  "--threads",
  "on",
  "--out",
  join(work, "threads"),
]);
const sharedMesh = refused(["check", join(dir, "shared-mesh.peras")]);
/** @type {Record<string, number>} */
const errorCounts = {};
for (const c of asWritten.codes) errorCounts[c] = (errorCounts[c] ?? 0) + 1;

// A frame of the scene, as the versions draw it.
if (last !== undefined)
  sh(
    "ffmpeg",
    [
      ...["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"],
      ...["-s", `${String(width)}x${String(height)}`, "-i", "-"],
      join(dir, "frame-600.png"),
    ],
    { input: last },
  );

// 4. Time in Chromium.
const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
});
const page = `<!doctype html><meta charset="utf-8"><title>lanterns</title><link rel="icon" href="data:,">`;
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  if (path === "/") {
    response.writeHead(200, { "content-type": "text/html" });
    response.end(page);
    return;
  }
  try {
    const file = join(work, path);
    response.writeHead(200, { "content-type": types[extname(file)] ?? "text/plain" });
    response.end(readFileSync(file));
  } catch {
    response.writeHead(404);
    response.end();
  }
});
await new Promise((resolve) => {
  server.listen(0, () => {
    resolve(undefined);
  });
});
const address = server.address();
const port = typeof address === "object" && address !== null ? address.port : 0;
// The versions, made in the page: each gives a function drawing one frame,
// and the bytes it holds outside the JavaScript heap.
const setup = (/** @type {string[]} */ names) => `
  const w = ${String(width)}, h = ${String(height)}, n = ${String(count)}, frame = 600;
  const made = {};
  const names = ${JSON.stringify(names)};
  for (const name of names) {
    if (name === "typescript" || name === "peras-managed") {
      const m = await import(name === "typescript" ? "/ts/pond.js" : "/managed/pond.js");
      const px = new Int32Array(w * h), d = new Float64Array(w * h), l = new Float64Array(3 * w * h);
      made[name] = { draw: () => m.render(px, d, l, w, h, n, frame), wasm: () => px.byteLength + d.byteLength + l.byteLength };
    } else if (name === "rust") {
      const r = await import("/rust/bench_lanterns_rust.js");
      const wasm = await r.default("/rust/bench_lanterns_rust_bg.wasm");
      const f = new r.Frame(w * h);
      made[name] = { draw: () => r.render(f, w, h, n, frame), wasm: () => wasm.memory.buffer.byteLength };
    } else {
      const p = await import(name === "peras" ? "/peras/pond.js" : "/debug/pond.js");
      const a = p.__perasHeap.array("i32", w * h), d = p.__perasHeap.array("f64", w * h), l = p.__perasHeap.array("f64", 3 * w * h);
      made[name] = { draw: () => p.render(a, d, l, w, h, n, frame), wasm: () => a.view().buffer.byteLength };
    }
  }`;
const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
const ROUNDS = 15;
const tab = await browser.newPage();
await tab.goto(`http://localhost:${String(port)}/`);
const timed = /** @type {Record<string, number[]>} */ (
  await tab.evaluate(
    `(async () => {${setup(versions)}
    const times = Object.fromEntries(names.map((k) => [k, []]));
    for (let run = 0; run < ${String(ROUNDS + 2)}; run++) {
      const order = run % 2 === 0 ? names : [...names].reverse();
      for (const k of order) {
        const t0 = performance.now();
        made[k].draw();
        const ms = performance.now() - t0;
        if (run >= 2) times[k].push(ms);
      }
    }
    return times;
  })()`,
  )
);
await tab.close();
// Each version alone in a fresh page: its CPU time over its wall time, by
// Chromium's TaskDuration, over 5 frames after 2; then the memory it holds,
// the JavaScript heap after a collection, and what the heap does not count:
// a module's WebAssembly memory, or the bytes of the typed arrays a
// JavaScript version draws into.
/** @type {Record<string, { load: number, heap: number, wasm: number }>} */
const alone = {};
for (const name of versions) {
  const fresh = await browser.newPage();
  await fresh.goto(`http://localhost:${String(port)}/`);
  const cdp = await fresh.context().newCDPSession(fresh);
  await cdp.send("Performance.enable");
  await fresh.evaluate(`(async () => {${setup([name])}
    globalThis.version = made[${JSON.stringify(name)}];
    version.draw(); version.draw();
  })()`);
  const metric = async (/** @type {string} */ m) =>
    /** @type {{ metrics: { name: string, value: number }[] }} */ (
      await cdp.send("Performance.getMetrics")
    ).metrics.find((x) => x.name === m)?.value ?? 0;
  const cpu0 = await metric("TaskDuration");
  const wall = /** @type {number} */ (
    await fresh.evaluate(`(() => {
      const t0 = performance.now();
      for (let i = 0; i < 5; i++) version.draw();
      return performance.now() - t0;
    })()`)
  );
  const cpu = ((await metric("TaskDuration")) - cpu0) * 1000;
  await cdp.send("HeapProfiler.collectGarbage");
  alone[name] = {
    load: cpu / wall,
    heap: await metric("JSHeapUsedSize"),
    wasm: /** @type {number} */ (await fresh.evaluate("version.wasm()")),
  };
  await fresh.close();
}
await browser.close();
server.close();

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
/** The ratios of `a`'s time over `b`'s, round by round. @param {string} a @param {string} b */
const ratios = (a, b) => (timed[a] ?? []).map((t, i) => t / (timed[b]?.[i] ?? Number.NaN));
/** @param {string} a @param {string} b */
const ratio = (a, b) => {
  const r = ratios(a, b);
  return { median: median(r), min: Math.min(...r), max: Math.max(...r) };
};
/** @type {Record<string, number>} */
const ms = Object.fromEntries(Object.entries(timed).map(([k, v]) => [k, median(v)]));

// 5. The verdicts, by the thresholds of the protocol.
const verdict = (/** @type {boolean} */ holds) => (holds ? "holds" : "**refuted**");
const speedTs = ratio("typescript", "peras");
const speedRust = withRust ? ratio("rust", "peras") : undefined;
const managed = ratio("peras-managed", "typescript");
const debug = ratio("peras-debug", "peras");
const f = (/** @type {number} */ x) => x.toFixed(2);
const range = (/** @type {{ median: number, min: number, max: number }} */ r) =>
  `${f(r.median)} (${f(r.min)} to ${f(r.max)})`;
const hypotheses = [
  {
    id: "H1",
    verdict: verdict(speedTs.median >= 1.3),
    detail: `Peras at ${range(speedTs)} times TypeScript's speed; threshold 1.3`,
  },
  {
    id: "H2",
    verdict:
      speedRust === undefined ? "undetermined (Rust not built)" : verdict(speedRust.median >= 0.85),
    detail:
      speedRust === undefined
        ? "cargo or wasm-bindgen missing"
        : `Peras at ${range(speedRust)} times Rust's speed; threshold 0.85`,
  },
  {
    id: "H3",
    verdict: verdict(managed.median <= 1.15),
    detail: `the managed module at ${range(managed)} times TypeScript's time; threshold 1.15`,
  },
  {
    id: "H4",
    verdict: "undetermined",
    detail: `\`parallel for\` cannot express the frame: ${threads.codes.join(", ")}, "${threads.first}" (finding F301)`,
  },
  {
    id: "H5",
    verdict:
      shipped.rust === undefined
        ? "undetermined (Rust not built)"
        : verdict((shipped.peras ?? Infinity) <= shipped.rust),
    detail: `Peras ships ${String(shipped.peras)} bytes gzip, Rust ${String(shipped.rust)}`,
  },
  {
    id: "H6",
    verdict: verdict((code.peras ?? Infinity) <= 1.25 * (code.typescript ?? 0)),
    detail: `Peras ${String(code.peras)} lines, ${f((code.peras ?? 0) / (code.typescript ?? 1))} times TypeScript's ${String(code.typescript)}; threshold 1.25. Met before it built: ${String(asWritten.codes.length)} errors (${Object.entries(
      errorCounts,
    )
      .map(([c, k]) => `${c} × ${String(k)}`)
      .join(", ")})`,
  },
  {
    id: "H7",
    verdict: verdict(debug.median <= 1.5),
    detail: `debug at ${range(debug)} times production's time; threshold 1.5`,
  },
];

const kb = (/** @type {number} */ b) => `${(b / 1024).toFixed(1)} KiB`;
const machine = `${cpus()[0]?.model ?? "?"}, ${String(cpus().length)} cores; Node ${process.version}; Chromium`;
const results = {
  machine,
  size: { width, height, lanterns: count, frame: 600 },
  identity,
  ms,
  ratios: { speedTs, speedRust, managed, debug },
  alone,
  shipped,
  code,
  asWritten: errorCounts,
  threads,
  sharedMesh,
  hypotheses,
};
const md = [
  "# The pond of a thousand lanterns: results",
  "",
  `Generated by \`node scripts/bench-lanterns.mjs\`, under the protocol of [decision 0029](../../docs/en/decisions/0029-pond-of-lanterns.md). Machine: ${machine}.`,
  "",
  "![Frame 600 of the pond, 1,000 lanterns](frame-600.png)",
  "",
  "Run 1, kept with its flaws, is in [run-1](run-1/RESULTS.md).",
  "",
  "## Hypotheses",
  "",
  "| | Verdict | Measured |",
  "| --- | --- | --- |",
  ...hypotheses.map((h) => `| ${h.id} | ${h.verdict} | ${h.detail} |`),
  "",
  "## Same pixels",
  "",
  `${versions.join(", ")} give the same bytes at ${String(width)} × ${String(height)} with ${String(count)} lanterns, nothing left alive in Peras's modules:`,
  "",
  "| Frame | SHA-256 |",
  "| ---: | --- |",
  ...identity.map((i) => `| ${String(i.frame)} | \`${i.hash}\` |`),
  "",
  `## Time, frame 600, ${String(ROUNDS)} runs after 2 of warm-up, the versions alternating`,
  "",
  "| Version | Median | CPU / wall, alone | JavaScript heap | Outside the heap | Shipped, gzip | Lines |",
  "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  ...versions.map(
    (v) =>
      `| ${v} | ${(ms[v] ?? 0).toFixed(1)} ms | ${f(alone[v]?.load ?? 0)} | ${kb(alone[v]?.heap ?? 0)} | ${kb(alone[v]?.wasm ?? 0)} | ${shipped[v] === undefined ? "-" : String(shipped[v])} | ${code[v] === undefined ? "-" : String(code[v])} |`,
  ),
  "",
  "Ratios round by round, median (range): " +
    [
      `TypeScript's time over Peras's ${range(speedTs)}`,
      speedRust === undefined ? "" : `Rust's over Peras's ${range(speedRust)}`,
      `managed's over TypeScript's ${range(managed)}`,
      `debug's over production's ${range(debug)}`,
    ]
      .filter((s) => s !== "")
      .join("; ") +
    ".",
  "",
  "## What perasc refused",
  "",
  `- **The first Peras version, as written** (\`as-written/pond.peras\`): ${String(asWritten.codes.length)} errors, ${Object.entries(
    errorCounts,
  )
    .map(([c, k]) => `${c} × ${String(k)}`)
    .join(", ")}; the first: "${asWritten.first}".`,
  `- **The threads** (\`threads.peras\`): ${threads.codes.join(", ")}, "${threads.first}".`,
  `- **The shared mesh** (\`shared-mesh.peras\`): ${sharedMesh.codes.join(", ")}, "${sharedMesh.first}".`,
  "",
].join("\n");
writeFileSync(join(dir, "RESULTS.md"), md);
writeFileSync(join(dir, "results.json"), `${JSON.stringify(results, null, 2)}\n`);
console.log(md);
