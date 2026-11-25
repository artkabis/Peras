// The figures the repository has recorded, read from its results files as
// they are written by the benchmarks' scripts: nothing is typed by hand, so
// that a decision which changes a result changes the page.

/** A ratio of times, Peras's over TypeScript's, with its range when known. */
export interface Ratio {
  median: number;
  low?: number;
  high?: number;
}

/** One program measured against TypeScript. */
export interface Measured {
  /** The program, as its results file names it. */
  label: string;
  /** Peras's time over TypeScript's. */
  peras: Ratio;
  /** The TypeScript timed against itself in the same rounds, when given. */
  control?: Ratio;
  /** Where it comes from, a path in the repository. */
  source: string;
  /** Before a decision, when the file gives it: the same program then. */
  before?: Ratio;
  /**
   * The TypeScript it is measured against: as it is usually written, or
   * tuned by hand (typed arrays, an expert's code). Usual when not said.
   */
  against?: "usual" | "tuned";
}

/** Where each toolchain first stops a bug: at compile time, at run time, never, or no bug. */
export type Stop = "compile" | "run" | "silent" | "none";

export interface Bug {
  name: string;
  kind: string;
  typescript: Stop;
  peras: Stop;
}

const number = (s: string): number => Number(s.replaceAll(",", ""));

/** `5.800 (2.945 to 7.082)` as a ratio; a lone number as its median. */
export function ratio(cell: string): Ratio | undefined {
  const m = /^\s*([\d.]+)(?:\s*\(([\d.]+) to ([\d.]+)\))?\s*$/.exec(cell);
  if (m === null) return undefined;
  const median = number(m[1] ?? "");
  if (m[2] === undefined || m[3] === undefined) return { median };
  return { median, low: number(m[2]), high: number(m[3]) };
}

/** The cells of a Markdown table's row, trimmed; `[]` for any other line. */
function cells(line: string): string[] {
  const t = line.trim();
  if (!t.startsWith("|") || !t.endsWith("|")) return [];
  return t
    .slice(1, -1)
    .split("|")
    .map((c) => c.trim());
}

/** The `Machine: …` line of a results file. */
export function machine(text: string): string {
  return /^Machine: (.*)$/m.exec(text)?.[1]?.trim() ?? "";
}

/** Decision 0046's four applications (`bench/next/RESULTS.md`). */
export function applications(text: string): Measured[] {
  const out: Measured[] = [];
  for (const line of text.split("\n")) {
    const c = cells(line);
    if (c.length !== 6 || !/^A\d/.test(c[0] ?? "")) continue;
    const peras = ratio(c[4] ?? "");
    const control = ratio(c[5] ?? "");
    if (peras === undefined) continue;
    out.push({
      label: c[0] ?? "",
      peras,
      ...(control === undefined ? {} : { control }),
      source: "bench/next/RESULTS.md",
    });
  }
  return out;
}

/**
 * Decision 0047's programs (`bench/frame/RESULTS.md`): each program's f3,
 * the option kept, against TypeScript, with f0, the frame before the
 * decision, as `before`, and the TypeScript against itself as the control.
 */
export function frames(text: string): Measured[] {
  const out: Measured[] = [];
  let label = "";
  let rows: Record<string, number> = {};
  const flush = () => {
    const f3 = rows.f3;
    const f0 = rows.f0;
    const again = rows["ts again"];
    if (label !== "" && f3 !== undefined) {
      out.push({
        label,
        peras: { median: f3 },
        ...(f0 === undefined ? {} : { before: { median: f0 } }),
        ...(again === undefined ? {} : { control: { median: again } }),
        source: "bench/frame/RESULTS.md",
      });
    }
  };
  for (const line of text.split("\n")) {
    const heading = /^###\s+(.*)$/.exec(line);
    if (heading !== null) {
      flush();
      label = heading[1]?.trim() ?? "";
      rows = {};
      continue;
    }
    const c = cells(line);
    // Version, same result, lines, time / f0, time / TypeScript, memory, bytes.
    if (c.length !== 7) continue;
    const r = ratio(c[4] ?? "");
    if (r !== undefined) rows[c[0] ?? ""] = r.median;
  }
  flush();
  return out;
}

function stop(cell: string): Stop {
  if (cell.startsWith("✓")) return "compile";
  if (cell.startsWith("◐")) return "run";
  if (cell.startsWith("✗")) return "silent";
  return "none";
}

/** Decision 0011's bugs of web applications (`bench/web-bugs/RESULTS.md`). */
export function bugs(text: string): Bug[] {
  const out: Bug[] = [];
  for (const line of text.split("\n")) {
    const c = cells(line);
    const m = /^(w\d+)-([\w-]+) \(([\w ]+)\)$/.exec(c[0] ?? "");
    if (c.length !== 3 || m === null) continue;
    out.push({
      // `w06-id-above-2-53`: a power of two as it is written, 2⁵³.
      name: `${m[1] ?? ""} ${(m[2] ?? "").replaceAll("-", " ")}`.replace(
        /\b2 (\d+)$/,
        (_, e: string) => `2${e.replace(/\d/g, (d) => "⁰¹²³⁴⁵⁶⁷⁸⁹"[Number(d)] ?? d)}`,
      ),
      kind: m[3] ?? "",
      typescript: stop(c[1] ?? ""),
      peras: stop(c[2] ?? ""),
    });
  }
  return out;
}

/** How many bugs each toolchain stops where. */
export function tally(list: readonly Bug[], who: "typescript" | "peras"): Record<Stop, number> {
  const t: Record<Stop, number> = { compile: 0, run: 0, silent: 0, none: 0 };
  for (const b of list) t[b[who]] += 1;
  return t;
}

/** `4.04 [4.00, 4.08]`: a speed-up, Peras's against TypeScript's, as a ratio of times. */
function speedup(cell: string): Ratio | undefined {
  const m = /([\d.]+)\s*\[([\d.]+),\s*([\d.]+)\]/.exec(cell);
  if (m === null) return undefined;
  // A speed-up of x is a time of 1 / x; its interval turns over.
  return {
    median: 1 / number(m[1] ?? ""),
    low: 1 / number(m[3] ?? ""),
    high: 1 / number(m[2] ?? ""),
  };
}

/**
 * Decision 0015's stream (`bench/stream/RESULTS.md`): its hypotheses H1 and
 * H2 give Peras's speed-up at 100,000 rows, the second of their two values,
 * against TypeScript as usually written and as optimized by hand.
 */
export function stream(text: string): Measured[] {
  const out: Measured[] = [];
  for (const line of text.split("\n")) {
    const c = cells(line);
    const against = c[0] === "H1" ? "usual" : c[0] === "H2" ? "tuned" : undefined;
    if (against === undefined || c.length !== 4) continue;
    const r = speedup((c[2] ?? "").split(";")[1] ?? "");
    if (r === undefined) continue;
    out.push({
      label: `stream, 100,000 rows`,
      against,
      peras: r,
      source: "bench/stream/RESULTS.md",
    });
  }
  return out;
}

/** The candle (`bench/candle/RESULTS.md`): the processor time of the garden's zoom. */
export function candle(text: string): Measured[] {
  const times: Record<string, number> = {};
  for (const line of text.split("\n")) {
    const c = cells(line);
    const m = /^([ABC])\./.exec(c[0] ?? "");
    const t = /([\d.]+) ms/.exec(c[1] ?? "");
    if (m !== null && t !== null) times[m[1] ?? ""] = number(t[1] ?? "");
  }
  const { A, B, C } = times;
  if (A === undefined || B === undefined || C === undefined) return [];
  return [
    {
      label: "the garden's zoom, processor time",
      against: "usual",
      peras: { median: C / A },
      source: "bench/candle/RESULTS.md",
    },
    {
      label: "the garden's zoom, processor time",
      against: "tuned",
      peras: { median: C / B },
      source: "bench/candle/RESULTS.md",
    },
  ];
}

/** The demo (`bench/demo/RESULTS.md`): 1,500,000 particles, a frame's median time. */
export function demo(text: string): Measured[] {
  const frame: Record<string, number> = {};
  for (const line of text.split("\n")) {
    const c = cells(line);
    const t = /^([\d.]+) ms/.exec(c[2] ?? "");
    if (t === null) continue;
    const who = (c[0] ?? "").startsWith("Peras")
      ? "peras"
      : (c[0] ?? "").includes("immutable")
        ? "usual"
        : "tuned";
    frame[who] = number(t[1] ?? "");
  }
  const p = frame.peras;
  if (p === undefined || frame.usual === undefined || frame.tuned === undefined) return [];
  return [
    {
      label: "1,500,000 particles, a frame",
      against: "usual",
      peras: { median: p / frame.usual },
      source: "bench/demo/RESULTS.md",
    },
    {
      label: "1,500,000 particles, a frame",
      against: "tuned",
      peras: { median: p / frame.tuned },
      source: "bench/demo/RESULTS.md",
    },
  ];
}

/**
 * The kernels in Chromium (`bench/browser/RESULTS.md`): each kernel's Peras
 * with no copy against the fastest JavaScript, its median time.
 */
export function kernels(text: string): Measured[] {
  const out: Measured[] = [];
  let kernel = "";
  for (const line of text.split("\n")) {
    const c = cells(line);
    if (c.length !== 8) continue;
    if ((c[0] ?? "") !== "") kernel = c[0] ?? "";
    if (!(c[2] ?? "").startsWith("Peras") || (c[2] ?? "").includes("copied")) continue;
    const x = Number(c[7]);
    if (!Number.isFinite(x) || x <= 0 || kernel === "Kernel") continue;
    out.push({
      label: `kernel: ${kernel}`,
      against: "tuned",
      peras: { median: 1 / x },
      source: "bench/browser/RESULTS.md",
    });
  }
  return out;
}
