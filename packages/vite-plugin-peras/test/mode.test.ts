import { describe, expect, it } from "vitest";
import peras, { modeOf } from "../src/index.ts";

describe("modeOf", () => {
  it("reads the mode from the extension", () => {
    expect(modeOf("/src/dsp.peras")).toBe("owned");
    expect(modeOf("/src/ui.apeiron")).toBe("managed");
  });

  it("ignores Vite query strings", () => {
    expect(modeOf("/src/dsp.peras?import")).toBe("owned");
  });

  it("returns null for other files", () => {
    expect(modeOf("/src/main.ts")).toBeNull();
    expect(modeOf("/src/peras.json")).toBeNull();
  });
});

describe("plugin", () => {
  it("is named and runs before other plugins", () => {
    const plugin = peras();
    expect(plugin.name).toBe("vite-plugin-peras");
    expect(plugin.enforce).toBe("pre");
  });
});
