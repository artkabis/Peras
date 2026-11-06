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
- **4 novembre 2026, d’où venait l’avance de Rust.** Mesuré par des variantes contrôlées de la fonction WebAssembly, puis confirmé dans le code source de V8 (`src/wasm/inlining-tree.h`, `module-decoder-impl.h`) :
  - **`fib` : une heuristique du moteur, pas LLVM.** La fonction `fib` exacte de Rust, placée seule dans un module, est aussi lente que la nôtre (40 ms sur `fib(35)`). Dans le module Rust, V8 inline `fib` en elle-même trois fois ; dans le nôtre, jamais. V8 dimensionne son budget d’inlining d’après la part des corps de fonction de moins de 50 octets : à la moitié ou plus, il suppose que la chaîne de compilation n’a pas inliné et autorise cinq fois la taille de la fonction ; à un quart ou moins, deux fois, trop peu pour un seul niveau. Le module Rust est rempli de petites fonctions d’allocateur et de glue ; le nôtre n’en a aucune. Peras le fait désormais lui-même, si bien que le gain ne dépend plus du moteur : le **déroulage de la récursion** (`crates/perasc-ir/src/inline.rs`) recopie le corps dans l’appel récursif restant sur trois niveaux, mesuré meilleur que deux et quatre, et la **rotation du test de sortie** (`opt.rs`, avec le backend) vérifie la condition d’une boucle une fois avant la boucle puis en bas, un seul branchement par tour. `fib(32)` passe de 9,4 ms à 4,0 ms contre 4,2 ms pour Rust ; sur `fib(30)` et `fib(35)`, 1,6 et 21,4 ms contre 1,7 et 22,6 ms.
  - **`checksum` : un branchement mal prédit.** `if (h < 0) h = -h` sur un hachage prend le branchement environ une fois sur deux, au hasard. LLVM l’écrit sans branchement. Peras le réécrit désormais en `abs`, que le backend écrit avec `select` (`opt.rs`, entiers seulement : sur les flottants, `-0.0` et NaN différeraient). `checksum` passe de 26 ms à 11 ms, le temps de Rust. Les autres changements de LLVM sur ce noyau (multiplication changée en addition, déroulage par quatre) ont été mesurés à moins de 5 %.
  - **`mandel` : un doublement, puis un invariant.** Le `mandel` de Rust seul dans un module reste plus rapide (12,8 ms contre 15,1) : le gain est donc dans son code. Chaque différence a été mesurée seule, médiane de neuf processus : réutiliser `x * x` et `y * y` n’apporte rien, V8 le fait déjà ; dérouler par deux n’apporte rien ; écrire `2.0 * x` sous la forme `x + x` apporte 1,1 ms, car l’addition ne dispute pas les unités de multiplication aux autres multiplications de la boucle ; sortir `cy` de la boucle intérieure et compter `px` en flottants apporte encore environ 0,3 ms. Peras fait désormais les deux premiers (`crates/perasc-ir/src/licm.rs`) : `2.0 * x` sur une variable flottante devient `x + x`, la même opération IEEE 754, `-0.0`, infinis et NaN compris, et la **sortie des invariants de boucle** sort une affectation d’une boucle quand sa valeur ne peut ni piéger ni changer, et que la variable n’est lue ni avant elle ni après la boucle. `mandel` passe de 15,0 ms à 13,6 ms, contre 13,3 ms pour Rust, mesuré de la même façon. Le compteur flottant est laissé de côté : il demande de prouver que `px` ne déborde jamais, pour une fraction de milliseconde.
  - Chaque passe garde tous les résultats, traps compris : le vérificateur d’IR tourne après chacune, un interpréteur d’IR compare code optimisé et non optimisé sur des entrées limites dans les tests unitaires, les flottants bit à bit (seul NaN égale n’importe quel NaN, comme en WebAssembly), y compris sur une IR écrite à la main qui lit des variables avant de les affecter, et le test différentiel en CI compare le backend direct au backend Rust, qui compile l’IR non optimisée.
- **4 novembre 2026, condition préalable 2 : l’allocateur de débogage** (`crates/perasc-runtime`). Écrit en WebAssembly textuel, assemblé à la compilation du crate, et gardé comme module à part jusqu’à ce que le backend direct l’intègre. Il remplit la mémoire neuve de `0xCD`, fait suivre chaque bloc d’une zone rouge de 16 octets vérifiée à la libération, empoisonne la mémoire libérée avec `0xDD` et la garde dans une quarantaine de 256 blocs, vérifiée contre les écritures quand un bloc en sort puis quand il est réutilisé. Il refuse une double libération, et la libération de tout ce qui n’est pas un bloc vivant, avant de toucher la mémoire, et liste les blocs encore vivants avec le site qui a alloué chacun. Un bug appelle `peras.fault(type, pointeur, site d’allocation, site de libération)` chez l’hôte, puis déclenche un trap ; les types sont mémoire épuisée, demande trop grande, débordement, libération invalide, double libération, utilisation après libération et fuite. Ses tests l’exécutent dans `wasmi`, commettent chaque bug exprès et comparent 5 000 opérations aléatoires à un modèle ; désactiver n’importe laquelle de ses vérifications fait échouer un test. Restent les conditions 1 (libérations explicites dans l’IR, avec un vérificateur) et 3 (le test différentiel sur les allocations vivantes) : rien n’alloue encore.
- **6 novembre 2026, condition préalable 1 : libérations explicites, et un vérificateur** (`crates/perasc-ir/src/own.rs`). L’IR a deux types de poignée : `owned`, un bloc du tas avec un seul propriétaire, et `borrowed`, un accès en lecture que seul un paramètre peut avoir, si bien qu’une référence n’est jamais stockée. Quatre opérations font changer les blocs de main : `alloc`, `move` (vers une variable, vers un argument `sink`, ou comme résultat), `borrow` (comme argument seulement) et `release`. Une poignée n’est jamais lue comme une valeur, comparée ni utilisée dans un calcul. Le vérificateur de libérations suit, pour chaque variable `owned`, si elle détient un bloc, le long de chaque chemin de l’IR structurée, et refuse :
  - la libération ou le déplacement d’une variable vide ;
  - une utilisation après libération ou déplacement ;
  - un bloc encore détenu au `return` ou à la fin d’une fonction `void` ;
  - un bloc écrasé, ou abandonné dès sa création ;
  - des chemins en désaccord là où ils se rejoignent, après un `if`, une boucle, un bloc, `&&` ou `||` ;
  - un tour de boucle qui ne rend pas les variables comme il les a trouvées.

  Un trap met fin à toute l’instance du module, mémoire comprise : il ne demande aucune libération, et la détection de fuites de l’allocateur de débogage ne tourne pas après un trap. Les tests le vérifient de trois façons :
  - des programmes écrits à la main : un programme accepté par sorte de chemin, et un programme refusé par sorte d’erreur ;
  - 1 000 programmes aléatoires qui libèrent exactement une fois, avec des `if` imbriqués, des boucles, `break`, `continue` et des `return` anticipés : tous acceptés, et exécutés par l’interpréteur d’IR, qui a désormais un tas, sur 16 entrées chacun, sans double libération, utilisation après libération ni fuite ;
  - chaque copie de ces programmes où une seule `release` est supprimée ou doublée est refusée.

  Désactiver n’importe laquelle des dix vérifications fait échouer un test. L’optimiseur laisse telles quelles les fonctions qui utilisent le tas tant qu’aucune passe n’est prouvée garder les libérations exactes, et le driver refuse l’IR qui utilise le tas tant que l’allocateur de débogage n’est pas intégré. L’abaissement n’en produit pas encore : la condition 3, le test différentiel sur les allocations vivantes, arrive avec le premier type qui alloue.

- **6 novembre 2026, l’allocateur de débogage intégré aux modules générés.** Un module qui utilise le tas importe `alloc(taille, site)` et `release(pointeur, site)` depuis `peras_rt`. Chaque allocation et chaque libération reçoit un numéro de site, associé à un nom comme `work (alloc 1)`. Le chargeur qu’écrit `perasc` embarque l’allocateur, l’instancie d’abord et y relie le module. Un bug mémoire lève une `PerasMemoryError` dont le message, dans la langue de `--lang`, nomme les deux sites, par exemple « le bloc alloué à twice (alloc 1) est libéré une seconde fois, à twice (release 2) ». Un export `__perasHeap` compte les blocs vivants et vérifie les fuites. Le plugin Vite garde les imports du chargeur et fait donc de même. Vérifié de trois façons, sur une IR écrite à la main puisqu’aucune construction n’alloue encore :
  - relié et exécuté dans `wasmi` : 1 000 allocations et libérations sans rien laisser ;
  - une double libération et une fuite sont détectées avec leurs sites ;
  - les mêmes modules importés dans Node par les tests de bout en bout, en anglais et en français.

  Le backend Rust refuse toujours l’IR qui utilise le tas : il n’a pas d’allocateur.
