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
      name: `${m[1] ?? ""} ${(m[2] ?? "").replaceAll("-", " ")}`,
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
