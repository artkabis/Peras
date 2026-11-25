import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import peras from "vite-plugin-peras";

// In this repository, use the perasc built by `cargo build --release -p perasc`
// unless PERASC names another one.
const perasc =
  process.env.PERASC ?? fileURLToPath(new URL("../../target/release/perasc", import.meta.url));

export default defineConfig({
  // Relative URLs: the page works at any address, such as /Peras/observatory/.
  base: "./",
  // The page imports the benchmarks' own files, outside this folder; the
  // declarations the plugin writes stay out of bench/ (dts: false), and the
  // page declares what it imports in src/frame.d.ts.
  plugins: [peras({ perasc, dts: false })],
});
