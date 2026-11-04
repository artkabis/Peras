# Bancs d’essai

_Version anglaise, qui fait foi : [README.md](README.md)._

## Détection de bugs (`bench/bugs`)

Chaque cas est un bug connu, ou une règle de rigueur, écrit deux fois : en TypeScript idiomatique (`.ts`) et en Peras équivalent (`.peras`). On demande à trois outils s’ils le refusent :

1. `tsc` avec les options les plus strictes (`bench/bugs/tsconfig.json`) ;
2. le même, plus les règles recommandées d’ESLint et le preset strict avec types de typescript-eslint (`bench/bugs/eslint.config.js`) ;
3. `perasc check`.

Lance-le avec `node scripts/bench-bugs.mjs`, après `pnpm install` ; il écrit [`bugs/RESULTS.md`](bugs/RESULTS.md).

Ce qu’il mesure, et ce qu’il ne mesure pas :

- Il mesure si des pièges bien connus de JavaScript et TypeScript sont refusés avant l’exécution du code.
- Le corpus a été écrit par les auteurs de Peras, autour de règles que Peras impose : il montre la couverture de ces pièges, pas une supériorité générale. Les cas où TypeScript fait déjà aussi bien sont gardés volontairement.
- TypeScript n’a ni `u8` ni types entiers : le cas 08 utilise un `Uint8ClampedArray`, et le cas 19 n’a aucun équivalent TypeScript.
- Les performances ne sont pas encore mesurées : Peras ne génère pas de code avant le jour 7 du plan de démarrage.

`bench/bugs` contient du code volontairement faux : l’ESLint et le Prettier du dépôt l’ignorent.

## Comparaison des backends (`bench/backends`)

`bench/backends/kernels.peras` contient quatre noyaux : récursion, boucles entières, boucles flottantes et arithmétique 64 bits. `node scripts/bench-backends.mjs` les compile avec les deux générateurs (WebAssembly direct, et Rust via `cargo` et `wasm-bindgen`), vérifie que chaque build renvoie les mêmes résultats qu’une référence JavaScript, puis mesure le temps de build, la taille du module (brute, gzip, après `wasm-opt -O3`) et le temps d’exécution dans Node. Un build direct avec `--no-optimize` montre ce qu’apportent les optimisations de l’IR ; `--check` ne fait que comparer les résultats, pour la CI. Il demande `rustup` et `wasm-bindgen` 0.2.130 pour le backend Rust, et écrit [`backends/RESULTS.md`](backends/RESULTS.md). La décision [0001](../docs/fr/decisions/0001-webassembly-backend.md) discute les résultats.
