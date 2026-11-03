// The strictest standard setup for TypeScript: ESLint's recommended rules plus
// typescript-eslint's strict, type-aware preset, on the strictest tsconfig.
import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(js.configs.recommended, tseslint.configs.strictTypeChecked, {
  languageOptions: {
    parserOptions: {
      project: "./tsconfig.json",
      tsconfigRootDir: import.meta.dirname,
    },
  },
});
