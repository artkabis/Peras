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
      // Deliberately buggy too, with its own config in scripts/bench-web-bugs.mjs.
      "bench/web-bugs/**",
      // The case study's TypeScript, measured as written (scripts/bench-case-study.mjs).
      "bench/case-study/**",
      // The demo's TypeScript, measured as written (scripts/demo.mjs).
      "bench/demo/**",
      // The garden's reference, measured as written (scripts/bench-garden.mjs).
      "bench/garden/**",
      // The versions of decision 0014, measured as written (scripts/bench-data.mjs).
      "bench/data/**",
      "packages/*/e2e/fixtures/**",
      "website/.pages/**",
      "bench/browser/kernels.d.ts",
      "bench/browser/parallel.d.ts",
      // AssemblyScript: TypeScript's syntax, its own types (usize, changetype), checked by asc.
      "bench/browser/assemblyscript/**",
      // Prototypes run once to decide, then kept so the measure can be run again.
      "bench/prototypes/**",
    ],
  },
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: {
          allowDefaultProject: ["*.js", "vitest.config.ts", "vitest.e2e.config.ts"],
          // The scripts have a project of their own, scripts/tsconfig.json:
          // here only the root configs.
          maximumDefaultProjectFileMatchCount_THIS_WILL_SLOW_DOWN_LINTING: 16,
        },
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    // Plain JS scripts run on Node: declare its globals (no extra dependency needed).
    files: ["**/*.js", "**/*.mjs"],
    languageOptions: {
      globals: {
        console: "readonly",
        process: "readonly",
        URL: "readonly",
        performance: "readonly",
      },
    },
  },
  {
    // The browser benchmark's page code runs in a browser.
    files: ["bench/browser/*.js"],
    languageOptions: {
      globals: { document: "readonly", navigator: "readonly" },
    },
  },
);
