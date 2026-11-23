import { execFile } from "node:child_process";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";
import type { Plugin } from "vite";

/**
 * Decision 0040, prototyped: runs the `test "name" { … }` blocks of `.peras`
 * and `.apeiron` files with Vitest. A test file is built for tests by
 * `perasc` (`PERAS_TEST_BUILD=1`), each block an exported function giving
 * `""` or the message of its first failure; this plugin turns the file into
 * a module of Vitest's tests, one a block, each failing with that message.
 */
export interface PerasTestOptions {
  /** The `perasc` executable. Default: the `PERASC` environment variable, then `perasc`. */
  perasc?: string;
  /** Which files are test files. Default: every `.peras` and `.apeiron` file. */
  include?: RegExp;
  /** The option of decision 0040, `v1` or `v2`. Default: `PERAS_PROTO_TEST`, then `v2`. */
  option?: string;
}

/** Builds `path` for tests into a new folder; gives the folder. */
function build(perasc: string, path: string, option: string): Promise<string> {
  return mkdtemp(join(tmpdir(), "peras-tests-")).then(
    (out) =>
      new Promise((done, fail) => {
        execFile(
          perasc,
          ["build", path, "--out", out, "--color", "never"],
          { env: { ...process.env, PERAS_TEST_BUILD: "1", PERAS_PROTO_TEST: option } },
          (error, _stdout, stderr) => {
            if (error) fail(new Error(`perasc could not build ${path} for tests:\n${stderr}`));
            else done(out);
          },
        );
      }),
  );
}

/** The plugin: a Peras test file becomes a module of Vitest's tests. */
export default function perasTests(options: PerasTestOptions = {}): Plugin {
  const perasc = options.perasc ?? process.env.PERASC ?? "perasc";
  const include = options.include ?? /\.(peras|apeiron)$/;
  const option = options.option ?? process.env.PERAS_PROTO_TEST ?? "v2";
  return {
    name: "vitest-peras",
    enforce: "pre",
    async transform(_code, id) {
      const path = id.split("?", 1)[0] ?? id;
      if (!include.test(path)) return null;
      const out = await build(perasc, path, option);
      const stem = basename(path).replace(/\.(peras|apeiron)$/, "");
      const listed: unknown = JSON.parse(await readFile(join(out, `${stem}.tests.json`), "utf8"));
      const names = Array.isArray(listed) ? listed.map(String) : [];
      const module = pathToFileURL(join(out, `${stem}.js`)).href;
      return {
        code: [
          `import { test } from "vitest";`,
          `const m = await import(${JSON.stringify(module)});`,
          `const names = ${JSON.stringify(names)};`,
          `names.forEach((name, i) => test(name, () => {`,
          `  const failure = m["__peras_test_" + String(i)]();`,
          `  if (failure !== "") throw new Error(failure);`,
          `}));`,
        ].join("\n"),
        map: null,
      };
    },
  };
}
