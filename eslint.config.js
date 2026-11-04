// ESLint flat config: strict, type-aware rules; any warning fails CI (--max-warnings 0).
import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  // bench/bugs holds deliberately buggy code, linted by its own config in scripts/bench-bugs.mjs.
  // The e2e fixtures are tiny browser apps, one of them broken on purpose.
  {
    ignores: [
      "target/**",
      "**/dist/**",
      "**/node_modules/**",
      "bench/bugs/**",
      "packages/*/e2e/fixtures/**",
    ],
  },
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: {
          allowDefaultProject: [
            "*.js",
            "vitest.config.ts",
            "vitest.e2e.config.ts",
            "scripts/*.mjs",
          ],
        },
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    // Plain JS scripts run on Node: declare its globals (no extra dependency needed).
    files: ["**/*.js", "**/*.mjs"],
    languageOptions: {
      globals: { console: "readonly", process: "readonly" },
    },
  },
);
