import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// Strings cross into JavaScript as UTF-8 bytes, decoded back to strings.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const text = fileURLToPath(new URL("fixtures/strings/text.peras", import.meta.url));

interface User {
  name: string;
  age: number;
}

interface Text {
  greeting(name: string): string;
  is_empty(s: string): boolean;
  same(a: string, b: string): boolean;
  repeat(s: string, n: number): string;
  emoji(): string;
  rename(u: User, name: string): User;
  arrays_equal(a: readonly number[], b: readonly number[]): boolean;
  doubled(s: string): string;
  __perasHeap: { live(): number; checkLeaks(): void };
}

async function load(): Promise<{ m: Text; dts: string }> {
  const compiled = await compile(text, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Text, dts: compiled.dts };
}

describe("strings in JavaScript", () => {
  it("are joined, compared and returned, in UTF-8", async () => {
    const { m } = await load();
    expect(m.greeting("Élodie")).toBe("Hello, Élodie!");
    expect(m.is_empty("")).toBe(true);
    expect(m.same("abc", "abc")).toBe(true);
    expect(m.same("abc", "abd")).toBe(false);
    expect(m.repeat("ab", 3)).toBe("ababab");
    expect(m.emoji()).toBe("café 😀");
    expect(m.doubled("é!")).toBe("é!é!");
    expect(m.arrays_equal([1, 2], [1, 2])).toBe(true);
    const long = "x".repeat(100_000);
    expect(m.repeat(long, 3).length).toBe(300_000);
    m.__perasHeap.checkLeaks();
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("enter the module in UTF-8 whatever their characters, short or long", async () => {
    const { m } = await load();
    const encoder = new TextEncoder();
    // Written one byte per character first, then with more room: texts that
    // need it at once, near the end, never, and the lengths around INLINE.
    const texts = [
      "",
      "é",
      "ééé",
      "😀",
      "a😀b",
      "x".repeat(255) + "é",
      "é".repeat(300),
      "données ".repeat(40_000),
      "x".repeat(200_000) + "😀",
      "\ud800 lone \udfff",
    ];
    for (const t of texts) {
      // `doubled` gives `t + t`: every byte made it in, and out again.
      const back = m.doubled(t);
      const expected = new TextDecoder().decode(encoder.encode(t));
      expect(back.length, t.slice(0, 20)).toBe(expected.length * 2);
      expect(back === expected + expected, t.slice(0, 20)).toBe(true);
    }
    m.__perasHeap.checkLeaks();
    expect(m.__perasHeap.live()).toBe(0);
  });

  it("live in structures, as JavaScript strings", async () => {
    const { m, dts } = await load();
    expect(m.rename({ name: "a", age: 3 }, "Zoé")).toEqual({ name: "Zoé", age: 3 });
    expect(m.__perasHeap.live()).toBe(0);
    expect(dts).toContain("export interface User { name: string; age: number; }");
    expect(dts).toContain("export declare function greeting(name: string | Uint8Array): string;");
  });
});
