import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Template literals give the same text as JavaScript's, on both allocators,
// and leave nothing alive.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/templates/templates.peras", import.meta.url));

interface Templates {
  greet(name: string, count: bigint, done: boolean): string;
  small(k: number, n: bigint): string;
  nested(name: string, k: number): string;
  escaped(name: string): string;
  joined(n: number): string;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(allocator: "debug" | "tlsf"): Promise<Templates> {
  const compiled = await compile(source, { perasc, allocator });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return (await import(url)) as Templates;
}

const NAMES = ["", "Ada", "été", "日本語", "🦀 crab", "a`b${c}", "x".repeat(300)];
const COUNTS = [0n, -1n, 42n, 9223372036854775807n, -9223372036854775808n];

describe("template literals", () => {
  for (const allocator of ["debug", "tlsf"] as const) {
    it(`write their values as JavaScript does, on ${allocator}`, async () => {
      const m = await load(allocator);
      for (const name of NAMES) {
        for (const count of COUNTS) {
          for (const done of [true, false]) {
            expect(m.greet(name, count, done)).toBe(
              `${name}: ${String(count)} items, done ${String(done)}`,
            );
          }
        }
        for (const k of [0, 1, 7]) {
          const word = k === 0 ? "none" : k === 1 ? "one" : "many";
          expect(m.nested(name, k)).toBe(`<${name}=${String(k)}> ${word}`);
        }
        expect(m.escaped(name)).toBe(`\`${name}\` costs \${price}
next line`);
      }
      for (const k of [0, -1, 2147483647, -2147483648])
        for (const n of [0n, 255n]) expect(m.small(k, n)).toBe(`[${String(k)}|${String(n)}]`);
      for (const n of [0, 1, 50]) {
        let out = "";
        for (let i = 0; i < n; i++) out = `${out}${String(i)};`;
        expect(m.joined(n)).toBe(out);
      }
      m.__perasHeap.checkLeaks();
      expect(m.__perasHeap.live()).toBe(0);
    });
  }
});
