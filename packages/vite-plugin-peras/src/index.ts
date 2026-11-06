import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import type { Plugin } from "vite";

/** Extension of owned modules, compiled to WebAssembly. */
export const OWNED_EXTENSION = ".peras";
/** Extension of managed modules, compiled to JavaScript. */
export const MANAGED_EXTENSION = ".apeiron";
/**
 * Lines of the loader written by `perasc build`, pinned by the tests of
 * `perasc-codegen-wasm`. The plugin keeps the imports (the debug allocator, for
 * a module that uses the heap) and the exports, and loads the module its own way
 * in between.
 */
export const IMPORTS_MARKER = "// perasc:imports";
export const LOAD_MARKER = "// perasc:load (needs `$imports`)";
export const EXPORTS_MARKER = "// perasc:exports (needs `$instance`)";

/** The mode of a Peras source module, set by its file extension. */
export type Mode = "owned" | "managed";

/** Returns the mode of a module id, or `null` if it is not a Peras source file. */
export function modeOf(id: string): Mode | null {
  const path = pathOf(id);
  if (path.endsWith(OWNED_EXTENSION)) return "owned";
  if (path.endsWith(MANAGED_EXTENSION)) return "managed";
  return null;
}

/** A module id without Vite's query string. */
function pathOf(id: string): string {
  return id.split("?", 1)[0] ?? id;
}

/** Options of the plugin. */
export interface PerasOptions {
  /** The `perasc` executable. Default: the `PERASC` environment variable, then `perasc` on the `PATH`. */
  perasc?: string;
  /** Language of the diagnostics. Default: chosen by `perasc` from the environment. */
  lang?: "en" | "fr";
  /**
   * Write `name.d.peras.ts` next to each `name.peras`, so that TypeScript knows its
   * exports (with `allowArbitraryExtensions`). Default: `true`.
   */
  dts?: boolean;
}

/** A compiled `.peras` module. */
export interface Compiled {
  /** The WebAssembly module. */
  wasm: Uint8Array;
  /** The JavaScript that defines `$imports`, the module's imports. */
  imports: string;
  /** The JavaScript that turns `$instance` into the module's exports. */
  exports: string;
  /** TypeScript declarations of the exports. */
  dts: string;
}

/**
 * Where a diagnostic points, line and column from 1. Vite shows the column as
 * given and passes it to the editor, which counts from 1, so it matches the
 * `file:line:column` of the excerpt below it.
 */
export interface Location {
  file: string;
  line: number;
  column: number;
}

/** A compilation that failed: the source has errors, or `perasc` could not run. */
export class PerasError extends Error {
  /** The first error of the source, when there is one. */
  readonly loc: Location | undefined;
  /** Every diagnostic, rendered by `perasc` with excerpt, cause and fix. */
  readonly frame: string | undefined;

  constructor(message: string, loc?: Location, frame?: string) {
    super(message);
    this.name = "PerasError";
    this.loc = loc;
    this.frame = frame;
  }
}

/** The part of `perasc --json` output the plugin reads. */
interface Report {
  errorCount: number;
  files: {
    path: string;
    diagnostics: {
      code: string;
      message: { text: string };
      labels: { primary: boolean; range: { start: { line: number; column: number } } }[];
    }[];
  }[];
}

interface Run {
  code: number;
  stdout: string;
  stderr: string;
}

/** Runs a program without a shell: a file name can never become a command. */
function run(program: string, args: string[]): Promise<Run> {
  return new Promise((resolve, reject) => {
    execFile(program, args, { maxBuffer: 16 * 1024 * 1024 }, (error, stdout, stderr) => {
      if (error === null) {
        resolve({ code: 0, stdout, stderr });
      } else if (typeof error.code === "number") {
        resolve({ code: error.code, stdout, stderr });
      } else if (error.code === "ENOENT") {
        reject(
          new PerasError(
            `vite-plugin-peras: \`${program}\` was not found. Build it with \`cargo build --release -p perasc\`, ` +
              "then set the `perasc` option or the PERASC environment variable to its path.",
          ),
        );
      } else {
        reject(new PerasError(`vite-plugin-peras: could not run \`${program}\`: ${error.message}`));
      }
    });
  });
}

/** The first error of a `--json` report, located for Vite. */
export function firstError(report: Report): { summary: string; loc: Location | undefined } {
  for (const file of report.files) {
    const diagnostic = file.diagnostics[0];
    if (diagnostic === undefined) continue;
    const label = diagnostic.labels.find((l) => l.primary) ?? diagnostic.labels[0];
    const start = label?.range.start;
    return {
      summary: `${diagnostic.code}: ${diagnostic.message.text}`,
      loc: start && { file: file.path, line: start.line, column: start.column },
    };
  }
  return { summary: `${String(report.errorCount)} error(s)`, loc: undefined };
}

/** Compiles one `.peras` file with `perasc build`, the direct backend. */
export async function compile(file: string, options: PerasOptions = {}): Promise<Compiled> {
  const perasc = options.perasc ?? process.env.PERASC ?? "perasc";
  const lang = options.lang === undefined ? [] : ["--lang", options.lang];
  const out = await mkdtemp(join(tmpdir(), "peras-"));
  try {
    const built = await run(perasc, ["build", file, "--out", out, "--json", ...lang]);
    if (built.code === 1) {
      const { summary, loc } = firstError(JSON.parse(built.stdout) as Report);
      // The same errors, rendered as in a terminal, for the overlay.
      const rendered = await run(perasc, [
        "build",
        file,
        "--out",
        out,
        "--color",
        "never",
        ...lang,
      ]);
      throw new PerasError(summary, loc, rendered.stderr.trimEnd());
    }
    if (built.code !== 0) {
      const kind = built.code === 3 ? "a compiler bug" : "a failure";
      throw new PerasError(
        `vite-plugin-peras: \`perasc build\` stopped on ${kind}:\n${built.stderr}`,
      );
    }
    const stem = basename(file, OWNED_EXTENSION);
    const [wasm, loader, dts] = await Promise.all([
      readFile(join(out, `${stem}.wasm`)),
      readFile(join(out, `${stem}.js`), "utf8"),
      readFile(join(out, `${stem}.d.ts`), "utf8"),
    ]);
    return { wasm, ...splitLoader(loader), dts };
  } finally {
    await rm(out, { recursive: true, force: true });
  }
}

/** The imports and the exports of a loader written by `perasc build`. */
export function splitLoader(loader: string): { imports: string; exports: string } {
  const imports = loader.indexOf(IMPORTS_MARKER);
  const load = loader.indexOf(LOAD_MARKER);
  const exports = loader.indexOf(EXPORTS_MARKER);
  if (imports < 0 || load < imports || exports < load) {
    throw new PerasError("vite-plugin-peras: this perasc writes loaders it does not understand.");
  }
  return { imports: loader.slice(imports, load), exports: loader.slice(exports) };
}

/**
 * The JavaScript module Vite serves for a `.peras` file. `wasmUrl` is an
 * expression giving the URL of the `.wasm` file; without it, the module is
 * inlined, which the dev server uses.
 */
export function moduleCode(compiled: Compiled, wasmUrl?: string): string {
  const load =
    wasmUrl === undefined
      ? `const $bytes = Uint8Array.from(atob(${JSON.stringify(Buffer.from(compiled.wasm).toString("base64"))}), (c) => c.charCodeAt(0));
const { instance: $instance } = await WebAssembly.instantiate($bytes, $imports);`
      : `const $response = await fetch(${wasmUrl});
if (!$response.ok) throw new Error(\`Peras: cannot load \${$response.url} (\${$response.status})\`);
// Compiled while it downloads, when the server labels it as WebAssembly.
const { instance: $instance } = $response.headers.get("content-type")?.startsWith("application/wasm")
  ? await WebAssembly.instantiateStreaming($response, $imports)
  : await WebAssembly.instantiate(await $response.arrayBuffer(), $imports);`;
  return `${compiled.imports}${load}\n${compiled.exports}`;
}

/** Writes `name.d.peras.ts` next to `name.peras`, only when it changes. */
async function writeDeclarations(file: string, dts: string): Promise<void> {
  const target = join(dirname(file), `${basename(file, OWNED_EXTENSION)}.d.peras.ts`);
  const text = dts.replace(
    "// Generated by perasc.",
    "// Generated by vite-plugin-peras from the .peras file. Do not edit.",
  );
  const current = await readFile(target, "utf8").catch(() => null);
  if (current !== text) await writeFile(target, text);
}

/**
 * Compiles imported `.peras` modules to WebAssembly with the direct backend of
 * `perasc`; no Rust toolchain is needed. An error in the source is shown in the
 * overlay of the dev server and fails `vite build`.
 */
export default function peras(options: PerasOptions = {}): Plugin {
  let building = false;
  return {
    name: "vite-plugin-peras",
    enforce: "pre",
    configResolved(config) {
      building = config.command === "build";
    },
    async load(id) {
      const mode = modeOf(id);
      if (mode === null) return null;
      const file = pathOf(id);
      if (mode === "managed") {
        this.error(`vite-plugin-peras: managed modules (.apeiron) are not compiled yet (${file}).`);
      }
      let compiled: Compiled;
      try {
        compiled = await compile(file, options);
      } catch (error) {
        if (!(error instanceof PerasError)) throw error;
        // Vite shows `frame` under the message in its overlay.
        return this.error({
          message: error.message,
          id: file,
          ...(error.loc && { loc: error.loc }),
          ...(error.frame !== undefined && { frame: error.frame }),
        });
      }
      if (options.dts ?? true) await writeDeclarations(file, compiled.dts);
      if (!building) return moduleCode(compiled);
      const reference = this.emitFile({
        type: "asset",
        name: `${basename(file, OWNED_EXTENSION)}.wasm`,
        source: compiled.wasm,
      });
      return moduleCode(compiled, `import.meta.ROLLUP_FILE_URL_${reference}`);
    },
  };
}
