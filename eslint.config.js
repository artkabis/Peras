// ESLint flat config: strict, type-aware rules; any warning fails CI (--max-warnings 0).
import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  { ignores: ["target/**", "**/dist/**", "**/node_modules/**"] },
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: { allowDefaultProject: ["*.js", "vitest.config.ts", "scripts/*.mjs"] },
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
