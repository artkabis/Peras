import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { compile, moduleCode } from "../src/index.ts";

// The functions of numbers WebAssembly has no instruction for come from
// JavaScript's Math: they give exactly what JavaScript gives.
const root = fileURLToPath(new URL("../../..", import.meta.url));
const perasc = process.env.PERASC ?? join(root, "target", "release", "perasc");
const source = fileURLToPath(new URL("fixtures/math/math.peras", import.meta.url));

interface Maths {
  polar(x: number, y: number): number;
  rounded(x: number): number;
  single(x: number): number;
}

async function load(): Promise<{ m: Maths; dts: string }> {
  const compiled = await compile(source, { perasc });
  const code = moduleCode(compiled);
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return { m: (await import(url)) as Maths, dts: compiled.dts };
}

describe("the functions of numbers", () => {
  it("give what Math gives", async () => {
    const { m } = await load();
    for (const [x, y] of [
      [1.5, 2],
      [-3, 0.25],
      [0.001, -7],
    ] as const) {
      const expected =
        Math.atan2(y, x) +
        Math.sqrt(x * x + y * y) +
        Math.pow(x, 2) +
        Math.log(Math.exp(x)) +
        Math.sin(x) ** 2 +
        Math.cos(x) ** 2 +
        Math.tan(x * 0.5);
      expect(m.polar(x, y)).toBe(expected);
    }
  });

  it("round halves up, as Math.round does", async () => {
    const { m } = await load();
    for (const x of [2.5, -2.5, 0.49999999999999994, -0.5, 3.7, -3.7]) {
      expect(m.rounded(x)).toBe(Math.round(x));
    }
  });

  it("compute an f32 as an f64, rounded back", async () => {
    const { m } = await load();
    expect(m.single(2)).toBe(Math.fround(Math.fround(8) + Math.fround(Math.sin(2)) + 2));
  });
});
