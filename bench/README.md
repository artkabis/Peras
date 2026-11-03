# Benchmarks

_French translation: [README.fr.md](README.fr.md)._

## Bug-catching benchmark (`bench/bugs`)

Each case is a known bug, or a rule of rigour, written twice: in idiomatic TypeScript (`.ts`) and in equivalent Peras (`.peras`). Three tools are asked whether they reject it:

1. `tsc` with the strictest options (`bench/bugs/tsconfig.json`);
2. the same, plus ESLint's recommended rules and typescript-eslint's strict type-aware preset (`bench/bugs/eslint.config.js`);
3. `perasc check`.

Run it with `node scripts/bench-bugs.mjs`, after `pnpm install`; it writes [`bugs/RESULTS.md`](bugs/RESULTS.md).

What it measures, and what it does not:

- It measures whether well-known pitfalls of JavaScript and TypeScript are rejected before the code runs.
- The corpus was written by the Peras authors, around rules Peras enforces: it shows coverage of these pitfalls, not general superiority. Cases where TypeScript already does as well are kept on purpose.
- TypeScript has no `u8` or integer types, so case 08 uses a `Uint8ClampedArray`, and case 19 has no TypeScript equivalent at all.
- Performance is not measured yet: Peras does not generate code before day 7 of the kickoff plan.

`bench/bugs` holds deliberately wrong code: the repository's ESLint and Prettier ignore it.
