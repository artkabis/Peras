import type { Plugin } from "vite";

/** Extension of owned modules, compiled to WebAssembly. */
export const OWNED_EXTENSION = ".peras";
/** Extension of managed modules, compiled to JavaScript. */
export const MANAGED_EXTENSION = ".apeiron";

/** The mode of a Peras source module, set by its file extension. */
export type Mode = "owned" | "managed";

/** Returns the mode of a module id, or `null` if it is not a Peras source file. */
export function modeOf(id: string): Mode | null {
  const path = id.split("?", 1)[0] ?? id;
  if (path.endsWith(OWNED_EXTENSION)) return "owned";
  if (path.endsWith(MANAGED_EXTENSION)) return "managed";
  return null;
}

/**
 * Day 1: the plugin only recognises Peras sources.
 * Compilation through `perasc` arrives on day 9 of the kickoff plan.
 */
export default function peras(): Plugin {
  return {
    name: "vite-plugin-peras",
    enforce: "pre",
    load(id) {
      const mode = modeOf(id);
      if (mode === null) return null;
      this.error(
        `vite-plugin-peras: compiling ${mode} modules is not implemented yet (${id}).`,
      );
    },
  };
}
