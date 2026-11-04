import { defineConfig } from "vitest/config";

// End-to-end tests: they need the release perasc (`cargo build --release -p perasc`)
// and a Chromium (`pnpm exec playwright install chromium`, or PERAS_CHROMIUM).
export default defineConfig({
  test: {
    include: ["packages/*/e2e/**/*.test.ts"],
    testTimeout: 60_000,
    hookTimeout: 60_000,
  },
});
