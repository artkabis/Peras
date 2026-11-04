# 0001 : Comment les modules `.peras` deviennent du WebAssembly

_Version anglaise, qui fait foi : [../../en/decisions/0001-webassembly-backend.md](../../en/decisions/0001-webassembly-backend.md)._

**Statut :** acceptée, 4 novembre 2026, avec les amendements sur les garanties ci-dessous. Le plan de démarrage et la spec la suivent.

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

## Décision

1. **Le backend direct est celui par défaut**, en développement comme en production : aucun outil à installer en dehors de `perasc`, des builds en millisecondes, les modules les plus petits.
2. **Peras se dote de ses propres passes d’optimisation sur l’IR**, choisies et mesurées avec `bench/backends` : d’abord la transformation avec accumulateur qui explique `fib`, puis la cause de l’écart sur `checksum` une fois trouvée. L’IR sait ce que LLVM doit prouver, notamment que les arguments `inout` ne partagent jamais de mémoire.
3. **Le backend Rust reste, obligatoire en CI et optionnel pour les utilisateurs.** Derrière `--backend rust`, il sert de référence de performance tant que le backend direct ne l’égale pas. En CI, chaque programme du banc d’essai des backends passe par les deux backends, qui doivent donner les mêmes résultats : ce test différentiel remplace la règle du plan selon laquelle `rustc` ne doit jamais refuser un programme accepté par `perasc`. Les utilisateurs n’ont jamais besoin d’une toolchain Rust.
4. **Chaque module généré est validé** avec `wasmparser` dans les tests.

## Les garanties, et ce qui les protège sans `rustc`

Les garanties que Peras donne à ses utilisateurs viennent des vérifications de `perasc`, jamais de `rustc` : un utilisateur ne devait jamais voir une erreur de `rustc`. Ce que `rustc` apportait, c’était un second filet contre les bugs de `perasc` lui-même. Le sortir du chemin par défaut change ce filet, pas les garanties de l’utilisateur.

**Typage.** Chaque règle de typage est vérifiée par `perasc` avant toute génération de code, quel que soit le backend. Sans `rustc`, un bug du générateur qui produirait du code mal typé est encore attrapé par la validation WebAssembly (dans les tests, et par chaque moteur au chargement), mais WebAssembly n’a que quatre types de valeurs : un `bool` utilisé comme `i32`, un `u8` comme `i32`, ou une structure à la place d’une autre passeraient. Pour compenser :

- un **vérificateur d’IR** recontrôle les types de l’IR après l’abaissement et après chaque passe d’optimisation, comme le font LLVM et Cranelift ;
- le **test différentiel** contre le backend Rust attrape une compilation erronée qui change un résultat.

**Libération de la mémoire.** Le sous-ensemble pris en charge n’alloue pas encore : rien n’est libéré aujourd’hui. Avec le backend Rust, un `Array` serait devenu un `Vec` Rust : `rustc` aurait inséré chaque libération, sur chaque chemin, et son vérificateur d’emprunts aurait recontrôlé l’analyse d’ownership de Peras. Avec le backend direct, `perasc` insère lui-même les libérations, et un bug pourrait libérer deux fois, utiliser une mémoire libérée ou fuir. WebAssembly confine un tel bug à la mémoire du module : il ne peut atteindre ni la page ni le navigateur, mais il peut corrompre les données du module. Le modèle de Peras garde ce problème maîtrisable : un seul propriétaire, aucune référence stockée, `inout` exclusif, comme en Swift ou en Hylo.

**Conditions préalables.** Le backend direct ne compile rien qui alloue (structures, tableaux, chaînes) avant que ces trois éléments existent et tournent en CI :

1. **des libérations explicites dans l’IR**, avec un vérificateur qui prouve que chaque valeur possédée est libérée exactement une fois sur chaque chemin, `return`, `break` et arrêts compris ;
2. **un allocateur de débogage** qui marque la mémoire libérée, détecte les doubles libérations et vérifie que chaque allocation est rendue à la fin d’un programme ; la spec prévoit déjà ce détecteur de fuites pour le mode managed, et il s’applique aussi au mode owned ;
3. **le test différentiel étendu aux allocations** : les deux backends doivent s’accorder sur les résultats et sur le nombre d’allocations encore vivantes.

Le fuzzing, prévu au jour 10 du plan de démarrage, couvre les deux backends.

## Conséquences

- Les jours 8 et 9 du plan changent : le plugin Vite utilise le backend direct et n’a besoin d’aucune toolchain Rust ; seuls `--backend rust` et la CI pilotent `cargo`.
- La CI gagne un job backends : il installe `wasm-bindgen` 0.2.130 et lance `node scripts/bench-backends.mjs --check`, qui échoue dès que les deux backends divergent.
- Peras prend davantage à sa charge : son optimiseur, son allocateur (compact, comme la spec le prévoit déjà, et lié seulement quand un module alloue), la génération SIMD, et les informations de débogage et source maps pour WebAssembly, que LLVM fournirait sinon.
- Le banc d’essai est petit : quatre noyaux, une machine, Node seulement. Il doit grandir avec chaque optimisation, et tourner dans les navigateurs avant toute affirmation de performance.

## Suivi

- **4 novembre 2026, élimination de la récursion avec accumulateur** (`crates/perasc-ir/src/opt.rs`). Sur le backend direct, `fib(32)` passe de 16 ms à 9,4 ms, et `fib(35)` de 68 ms à 39 ms ; le backend Rust prend 4,2 ms sur `fib(32)`. Le vérificateur d’IR tourne après l’abaissement et après la passe, et le backend Rust compile l’IR non optimisée : le test différentiel en CI couvre donc aussi la passe ; un interpréteur d’IR compare les deux versions sur des centaines d’entrées dans les tests unitaires. L’écart restant n’est pas encore expliqué : il reste de 1,5 fois sur `fib(35)` avec le seul compilateur optimisant de V8, donc les paliers de compilation n’en sont pas la cause, et tester le cas de base avant la boucle, comme le fait LLVM, a été mesuré sans aucun gain, donc n’a pas été gardé.
