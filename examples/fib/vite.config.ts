import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import peras from "vite-plugin-peras";

// In this repository, use the perasc built by `cargo build --release -p perasc`
// unless PERASC names another one.
const perasc =
  process.env.PERASC ?? fileURLToPath(new URL("../../target/release/perasc", import.meta.url));

export default defineConfig({
  plugins: [peras({ perasc })],
});
