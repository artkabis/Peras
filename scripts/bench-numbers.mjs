// Decision 0030's protocol: numbers written as JavaScript writes them. Each
// option is a prototype the compiler builds when PERAS_PROTO_NUM names it:
// a, today's literals; b, a literal takes the type it is used as; c, b and
// integer variables converted to the float they are used as.
//
// 1. Ease of writing: the five programs of bench/numbers/ease, written as a
//    user would from JavaScript (P1 is the pond as first written): the
//    errors each option meets, the lines its fixed version changes.
// 2. What each option lets through: each program's result, built as an
//    owned module on both allocators and as a managed module, against the
//    same program written in JavaScript.
// 3. The pond as written, built by b and c, against the pond fixed, built by
//    a: bytes, and the time of a frame in one process, the order alternating
//    round by round, with the CPU time over the wall time.
//
// Usage: node scripts/bench-numbers.mjs [--check]
//        --check: every program builds and agrees with JavaScript as
//        registered; nothing timed or written.
// Writes bench/numbers/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";
import ts from "typescript";

const check = process.argv.includes("--check");
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "numbers");
const ease = join(bench, "ease");
const work = join(root, "target", "bench-numbers");
const perasc = join(root, "target", "release", "perasc");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

let builds = 0;
/**
 * Builds a source with an option, owned on an allocator or managed.
 * @param {string} source
 * @param {string} option
 * @param {"tlsf" | "debug" | "managed"} how
 * @returns {{ ok: true, js: string, wasm: string | undefined } | { ok: false, errors: string[] }}
 */
function build(source, option, how) {
  builds += 1;
  const dir = join(work, `${String(builds)}-${basename(source, ".peras")}-${option}-${how}`);
  mkdirSync(dir, { recursive: true });
  let file = source;
  if (how === "managed") {
    file = join(dir, `${basename(source, ".peras")}.apeiron`);
    // A managed module moves nothing, so it takes no `sink`.
    copyFileSync(source, file);
  }
  const args = ["build", file, "--out", dir, "--color", "never"];
  if (how !== "managed") args.push("--allocator", how);
  const r = spawnSync(perasc, args, {
    encoding: "utf8",
    env: { ...process.env, PERAS_PROTO_NUM: option },
  });
  if (r.status !== 0)
    return {
      ok: false,
      errors: r.stderr
        .split("\n")
        .filter((l) => l.startsWith("error["))
        .map((l) => l.slice(6, 11)),
    };
  const name = basename(file).replace(/\.(peras|apeiron)$/, "");
  const wasm = join(dir, `${name}.wasm`);
  return { ok: true, js: join(dir, `${name}.js`), wasm: existsSync(wasm) ? wasm : undefined };
}

/** @typedef {Record<string, (...args: unknown[]) => unknown> & { __perasHeap?: { live(): number, array(k: string, n: number): { view(): Int32Array, release(): void } } }} Loaded */
/** @param {string} js @returns {Promise<Loaded>} */
async function load(js) {
  /** @type {unknown} */
  const loaded = await import(pathToFileURL(js).href);
  return /** @type {Loaded} */ (loaded);
}

/** The lines two sources differ by, comments aside: removed plus added. */
function changed(/** @type {string} */ a, /** @type {string} */ b) {
  const code = (/** @type {string} */ f) =>
    readFileSync(f, "utf8")
      .split("\n")
      .filter((l) => !l.trimStart().startsWith("//"));
  const x = code(a);
  const y = code(b);
  return x.filter((l) => !y.includes(l)).length + y.filter((l) => !x.includes(l)).length;
}

// The pond's reference, TypeScript, for P1.
const pondTs = join(work, "pond.js");
writeFileSync(
  pondTs,
  ts.transpileModule(readFileSync(join(root, "bench", "lanterns", "pond.ts"), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText,
);
const pondRef = /** @type {{ render: (...a: unknown[]) => number }} */ (await load(pondTs));
const [pw, ph, pn] = [256, 192, 120];
/** @param {(...a: unknown[]) => unknown} render @param {Loaded | undefined} m */
function pondHash(render, m) {
  if (m?.__perasHeap !== undefined) {
    const heap = m.__perasHeap;
    const [a, b, c] = [
      heap.array("i32", pw * ph),
      heap.array("f64", pw * ph),
      heap.array("f64", 3 * pw * ph),
    ];
    render(a, b, c, pw, ph, pn, 600);
    const out = createHash("sha256").update(new Uint8Array(a.view().slice().buffer)).digest("hex");
    for (const x of [a, b, c]) x.release();
    return out.slice(0, 12);
  }
  const px = new Int32Array(pw * ph);
  render(px, new Float64Array(pw * ph), new Float64Array(3 * pw * ph), pw, ph, pn, 600);
  return createHash("sha256").update(new Uint8Array(px.buffer)).digest("hex").slice(0, 12);
}

// Each program: its source as written, its entry, and how to run it on a
// module or on its JavaScript, giving a text to compare.
/** @typedef {{ id: string, natural: string, fixed: (o: string) => string, run: (m: Loaded) => string, js: () => string }} Program */
/** @type {Program[]} */
const programs = [
  {
    id: "P1",
    natural: join(root, "bench", "lanterns", "as-written", "pond.peras"),
    fixed: () => join(root, "bench", "lanterns", "pond.peras"),
    run: (m) => pondHash(/** @type {(...a: unknown[]) => unknown} */ (m.render), m),
    js: () => pondHash(pondRef.render, undefined),
  },
  {
    id: "P2",
    natural: join(ease, "p2.peras"),
    fixed: (o) => join(ease, "fixed", `p2-${o}.peras`),
    run: (m) => String(m.spread?.([1.5, 2.5, 4, 8, 13])),
    js: () => {
      const xs = [1.5, 2.5, 4, 8, 13];
      let sum = 0;
      for (const x of xs) sum += x;
      const mean = sum / xs.length;
      let squares = 0;
      for (const x of xs) squares += (x - mean) * (x - mean);
      return String(Math.sqrt(squares / (xs.length - 1)) + mean);
    },
  },
  {
    id: "P3",
    natural: join(ease, "p3.peras"),
    fixed: (o) => join(ease, "fixed", `p3-${o}.peras`),
    run: (m) => {
      const pos = [0.1, 0.95, 0.5];
      const speed = [0.3, 0.2, -1.4];
      const e = m.step?.(pos, speed, 0.5);
      return `${String(e)} ${pos.join(",")} ${speed.join(",")}`;
    },
    js: () => {
      const pos = [0.1, 0.95, 0.5];
      const speed = [0.3, 0.2, -1.4];
      let energy = 0;
      for (let i = 0; i < pos.length; i++) {
        speed[i] = (speed[i] ?? 0) * 0.99;
        pos[i] = (pos[i] ?? 0) + (speed[i] ?? 0) * 0.5;
        if ((pos[i] ?? 0) < 0) {
          pos[i] = -(pos[i] ?? 0);
          speed[i] = -(speed[i] ?? 0);
        } else if ((pos[i] ?? 0) > 1) {
          pos[i] = 2 - (pos[i] ?? 0);
          speed[i] = -(speed[i] ?? 0);
        }
        energy += ((speed[i] ?? 0) * (speed[i] ?? 0)) / 2;
      }
      return `${String(energy)} ${pos.join(",")} ${speed.join(",")}`;
    },
  },
  {
    id: "P4",
    natural: join(ease, "p4.peras"),
    fixed: (o) => join(ease, "fixed", `p4-${o}.peras`),
    run: (m) => String(m.taxed?.(3)),
    js: () => {
      // The literal of P4, as JavaScript reads it: beyond 2^53, it loses its
      // last digit, which an `i64` keeps.
      let total = Number("9007199254740993");
      for (let i = 0; i < 3; i++) total += Math.round(1999 * 1.2);
      return String(total);
    },
  },
  {
    id: "P5",
    natural: join(ease, "p5.peras"),
    fixed: (o) => join(ease, "fixed", `p5-${o}.peras`),
    run: (m) => String(m.gradient?.(5)),
    js: () => {
      const mix = (/** @type {number} */ a, /** @type {number} */ b, /** @type {number} */ t) =>
        a * (1 - t) + b * t;
      const stops = [0, 0.5, 1];
      let total = 0;
      for (let i = 0; i < 5; i++) {
        const t = i / (5 - 1);
        const r = mix(stops[0] ?? 0, stops[1] ?? 0, t);
        const g = mix(stops[1] ?? 0, stops[2] ?? 0, t);
        const b = mix(stops[2] ?? 0, 0, t);
        total += r * 255 + g * 255 + b * 255;
      }
      return String(total);
    },
  },
  {
    // Added after the protocol was registered: P5's total does not depend
    // on `t`, so it cannot show an integer division.
    id: "P5'",
    natural: join(ease, "p5t.peras"),
    fixed: (o) => join(ease, "fixed", `p5t-${o}.peras`),
    run: (m) => String(m.steps?.(5)),
    js: () => {
      let total = 0;
      for (let i = 0; i < 5; i++) total += i / (5 - 1);
      return String(total);
    },
  },
];

const options = ["a", "b", "c"];
const lines = [];
lines.push(
  `Machine: ${cpus()[0]?.model ?? "?"}, ${String(cpus().length)} cores; Node ${process.version}.`,
  "",
);

// 1 and 2. Ease of writing, and what each option lets through.
lines.push("### Ease of writing, and the results", "");
lines.push(
  "| Program | Option | Errors met as written | Lines changed to build | Owned, debug and TLSF | Managed | JavaScript as written | Agree with JavaScript |",
);
lines.push("| --- | --- | --- | ---: | --- | --- | --- | --- |");
/** @type {Record<string, { errors: number, lines: number, built: number, silent: number }>} */
const totals = Object.fromEntries(
  options.map((o) => [o, { errors: 0, lines: 0, built: 0, silent: 0 }]),
);
let disagreements = 0;
for (const p of programs) {
  const reference = p.js();
  for (const o of options) {
    const first = build(p.natural, o, "tlsf");
    const errors = first.ok ? [] : first.errors;
    const file = first.ok ? p.natural : p.fixed(o);
    if (!existsSync(file)) throw new Error(`${p.id} ${o}: no fixed version ${file}`);
    /** @type {string[]} */
    const results = [];
    for (const how of /** @type {const} */ (["debug", "tlsf", "managed"])) {
      const built = build(file, o, how);
      if (!built.ok)
        throw new Error(`${p.id} ${o} ${how}: does not build: ${built.errors.join(", ")}`);
      const m = await load(built.js);
      results.push(p.run(m));
      if ((m.__perasHeap?.live() ?? 0) !== 0)
        throw new Error(`${p.id} ${o} ${how}: blocks left alive`);
    }
    const [debug, tlsf, managed] = results;
    if (debug !== tlsf || tlsf !== managed) disagreements += 1;
    const agrees = tlsf === reference;
    const t = totals[o];
    if (t !== undefined) {
      t.errors += errors.length;
      t.lines += file === p.natural ? 0 : changed(p.natural, file);
      t.built += first.ok ? 1 : 0;
      t.silent += agrees ? 0 : 1;
    }
    /** @type {Record<string, number>} */
    const codes = {};
    for (const c of errors) codes[c] = (codes[c] ?? 0) + 1;
    const errorText =
      errors.length === 0
        ? "builds"
        : Object.entries(codes)
            .map(([c, k]) => `${c} × ${String(k)}`)
            .join(", ");
    lines.push(
      `| ${p.id} | ${o} | ${errorText} | ${String(file === p.natural ? 0 : changed(p.natural, file))} | \`${String(tlsf)}\`${debug === tlsf ? "" : `, debug \`${String(debug)}\``} | ${managed === tlsf ? "the same" : `\`${String(managed)}\``} | \`${reference}\` | ${agrees ? "yes" : "**no**"} |`,
    );
  }
}
lines.push("");
lines.push(
  "| Option | Programs that build as written | Errors met | Lines changed | Results that differ from JavaScript |",
);
lines.push("| --- | ---: | ---: | ---: | ---: |");
for (const o of options) {
  const t = totals[o];
  if (t === undefined) continue;
  lines.push(
    `| ${o} | ${String(t.built)} of ${String(programs.length)} | ${String(t.errors)} | ${String(t.lines)} | ${String(t.silent)} |`,
  );
}
lines.push("");
if (disagreements > 0)
  throw new Error(`${String(disagreements)} programs where the modes disagree`);
console.log(lines.join("\n"));
if (check) process.exit(0);

// 3. The pond: bytes and time.
const pondFixed = join(root, "bench", "lanterns", "pond.peras");
const pondNatural = join(root, "bench", "lanterns", "as-written", "pond.peras");
/** @type {{ label: string, built: { js: string, wasm: string | undefined } }[]} */
const ponds = [];
for (const [label, file, o] of [
  ["a, the pond fixed", pondFixed, "a"],
  ["b, the pond as written", pondNatural, "b"],
  ["c, the pond as written", pondNatural, "c"],
]) {
  const built = build(file ?? "", o ?? "a", "tlsf");
  if (!built.ok) throw new Error(`${String(label)}: does not build`);
  ponds.push({ label: label ?? "", built });
}
const [w, h, n] = [1024, 768, 1000];
const made = [];
for (const pond of ponds) {
  const m = await load(pond.built.js);
  const heap = m.__perasHeap;
  if (heap === undefined) throw new Error("no heap");
  const bufs = [heap.array("i32", w * h), heap.array("f64", w * h), heap.array("f64", 3 * w * h)];
  const render = /** @type {(...a: unknown[]) => number} */ (m.render);
  const wasm = readFileSync(pond.built.wasm ?? "");
  made.push({
    label: pond.label,
    draw: () => render(...bufs, w, h, n, 600),
    hash: () =>
      createHash("sha256")
        .update(new Uint8Array(bufs[0]?.view().slice().buffer ?? new ArrayBuffer(0)))
        .digest("hex"),
    bytes: {
      raw: wasm.length,
      gzip: gzipSync(wasm).length,
      sha: createHash("sha256").update(wasm).digest("hex").slice(0, 12),
    },
    times: /** @type {number[]} */ ([]),
    cpu: 0,
    wall: 0,
  });
}
const ROUNDS = 15;
for (let round = 0; round < ROUNDS + 2; round++) {
  const order = round % 2 === 0 ? made : [...made].reverse();
  for (const v of order) {
    const c0 = process.cpuUsage();
    const t0 = performance.now();
    v.draw();
    const ms = performance.now() - t0;
    const c = process.cpuUsage(c0);
    if (round >= 2) {
      v.times.push(ms);
      v.cpu += (c.user + c.system) / 1000;
      v.wall += ms;
    }
  }
}
const hashes = new Set(made.map((v) => v.hash()));
if (hashes.size !== 1) throw new Error("the ponds give different pixels");
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
const base = made[0];
lines.push(
  `### The pond, frame 600 at ${String(w)} × ${String(h)} with ${String(n)} lanterns, ${String(ROUNDS)} rounds after 2, the order alternating`,
  "",
);
lines.push(
  "| Build | Module, raw (gzip) | Same bytes as a | Median | / a, median (range) | CPU / wall |",
);
lines.push("| --- | ---: | --- | ---: | --- | ---: |");
for (const v of made) {
  const ratios = v.times.map((t, i) => t / (base?.times[i] ?? Number.NaN));
  lines.push(
    `| ${v.label} | ${String(v.bytes.raw)} (${String(v.bytes.gzip)}) | ${v.bytes.sha === base?.bytes.sha ? "yes" : "no"} | ${median(v.times).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${(v.cpu / v.wall).toFixed(2)} |`,
  );
}
lines.push("");
console.log(lines.slice(-7).join("\n"));
writeFileSync(
  join(bench, "RESULTS.md"),
  `# Numbers written as JavaScript writes them: the options measured\n\nGenerated by \`node scripts/bench-numbers.mjs\`, for [decision 0030](../../docs/en/decisions/0030-numeric-literals.md).\n\n${lines.join("\n")}`,
);
