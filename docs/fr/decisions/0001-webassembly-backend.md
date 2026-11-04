# 0001 : Comment les modules `.peras` deviennent du WebAssembly

_Version anglaise, qui fait foi : [../../en/decisions/0001-webassembly-backend.md](../../en/decisions/0001-webassembly-backend.md)._

**Statut :** proposée, 4 novembre 2026. Le plan de démarrage garde son backend Rust tant que cette décision n’est pas acceptée.

## Contexte

Le plan de démarrage génère du Rust depuis l’IR, le compile avec `cargo` pour `wasm32-unknown-unknown`, puis lance `wasm-bindgen`. Cela réutilise l’optimiseur de LLVM et fait de `rustc` un second filet de sécurité, mais chaque utilisateur de Peras doit installer `rustup`, `cargo` et un `wasm-bindgen` de la bonne version : un gros frein pour des développeurs venus de TypeScript.

Pour décider sur des mesures plutôt que sur des opinions, le jour 7 a construit les deux backends sur la même IR :

- **direct** : `perasc-codegen-wasm` encode lui-même le `.wasm` avec `wasm-encoder`, et écrit le chargeur JavaScript et les déclarations `.d.ts` à partir des types qu’il connaît déjà ;
- **rust** : `perasc-codegen-rs` écrit une crate sans `unsafe`, compilée par `cargo` et `wasm-bindgen`.

Les deux donnent à l’arithmétique entière la même sémantique circulaire, s’arrêtent sur les mêmes divisions et saturent les mêmes conversions.

## Mesures

Tirées de [`bench/backends/RESULTS.md`](../../../bench/backends/RESULTS.md), produit par `node scripts/bench-backends.mjs` sur un Xeon 4 cœurs à 2,1 GHz, Node 26.13, Rust 1.99.1. Les noyaux sont `fib` (récursion), `primes` (boucles entières), `mandel` (boucles `f64`) et `checksum` (arithmétique `i64`).

|                                   | direct   | rust                                        |
| --------------------------------- | -------- | ------------------------------------------- |
| Premier build                     | 6,7 ms   | 9,2 s                                       |
| Build après une modification      | 4,7 ms   | 505 ms                                      |
| Outils nécessaires                | `perasc` | `perasc`, `rustup`, `cargo`, `wasm-bindgen` |
| Taille du module, brute           | 615 o    | 19 232 o                                    |
| Taille après `wasm-opt -O3`, gzip | 369 o    | 1 286 o                                     |
| `fib(32)`                         | 17 ms    | 4,2 ms                                      |
| `primes(1 000 000)`               | 132 ms   | 131 ms                                      |
| `mandel(600)`                     | 16 ms    | 16 ms                                       |
| `checksum(5 000 000, 7)`          | 26 ms    | 11 ms                                       |

Les temps varient d’environ 10 % d’un passage à l’autre sur cette machine virtuelle ; les tailles, non. Chaque build a donné les mêmes résultats qu’une référence JavaScript sur 19 entrées. Pour comparaison, le JavaScript seul a pris 20 ms sur `fib(32)` et 164 ms sur `checksum`, qui exige `BigInt` puisque JavaScript n’a pas d’entiers 64 bits.

D’où viennent les écarts :

- Le module Rust embarque `dlmalloc`, l’allocateur de Rust, alors que ces fonctions n’allouent jamais : c’est l’essentiel de sa taille.
- Sur `fib`, LLVM transforme l’un des deux appels récursifs en boucle avec accumulateur ; le module désassemblé le montre. `wasm-opt` ne le fait pas, et ne comble donc pas l’écart.
- Sur `checksum`, la cause n’a pas encore été analysée.
- Sur les boucles sans ces motifs (`primes`, `mandel`), le compilateur optimisant de V8 met les deux à égalité.

## Décision (proposée)

1. **Le backend direct devient celui par défaut**, en développement comme en production : aucun outil à installer en dehors de `perasc`, des builds en millisecondes, les modules les plus petits.
2. **Peras se dote de ses propres passes d’optimisation sur l’IR**, choisies et mesurées avec `bench/backends` : d’abord la transformation avec accumulateur qui explique `fib`, puis la cause de l’écart sur `checksum` une fois trouvée. L’IR sait ce que LLVM doit prouver, notamment que les arguments `inout` ne partagent jamais de mémoire.
3. **Le backend Rust reste, comme backend secondaire**, derrière `--backend rust` : une référence de performance tant que le backend direct ne l’égale pas, et la seconde moitié d’un test différentiel, où les deux backends doivent s’accorder sur chaque programme et chaque entrée. Cela remplace la règle du plan selon laquelle `rustc` ne doit jamais refuser un programme accepté par `perasc`.
4. **Chaque module généré est validé** avec `wasmparser` dans les tests.

## Conséquences

- Les jours 8 et 9 du plan changent : le plugin Vite utilise le backend direct et n’a besoin d’aucune toolchain Rust ; piloter `cargo` devient optionnel.
- Peras prend davantage à sa charge : son optimiseur, son allocateur (compact, comme la spec le prévoit déjà, et lié seulement quand un module alloue), la génération SIMD, et les informations de débogage et source maps pour WebAssembly, que LLVM fournirait sinon.
- Le banc d’essai est petit : quatre noyaux, une machine, Node seulement. Il doit grandir avec chaque optimisation, et tourner dans les navigateurs avant toute affirmation de performance.
