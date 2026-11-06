# Journal des modifications

_English version: [CHANGELOG.md](CHANGELOG.md)_

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; les versions suivent le [versionnage sémantique](https://semver.org/lang/fr/). Avant la 1.0, toute version peut casser le langage.

## Non publié

### Langage et compilateur

- Les **structures** se compilent, comme des valeurs : chacune est remplacée par ses champs scalaires, structures imbriquées aplaties, si bien qu’elle ne touche jamais le tas.
  - Un paramètre structure devient un paramètre par champ.
  - Une fonction qui renvoie une structure renvoie un résultat WebAssembly par champ, et un tuple dans le backend Rust.
  - Les champs se lisent et s’écrivent, affectations composées comprises ; une structure se copie par affectation ; `p = Point(p.y, p.x)` échange.
  - Les appels qui renvoient une structure s’exécutent d’abord, dans des temporaires, sans changer l’ordre d’évaluation : à droite de `&&` et `||` seulement quand ils s’exécuteraient, et dans une condition de boucle à chaque tour.
  - Les fonctions exportées prennent et renvoient des objets JavaScript, déclarés comme interfaces TypeScript.
- Les **options** se compilent, sans le tas elles aussi : une `Option<T>` est un `bool` suivi des scalaires de `T`, n’importe quel `T` compilé, structures comprises.
  - `Some`, `None`, `isSome`, `isNone`, `unwrapOr`, et `?`, qui renvoie `None` tout de suite depuis une fonction qui renvoie une option.
  - `checkedAdd`, `checkedSub` et `checkedMul` sur `i32` et `i64` donnent `None` en cas de dépassement ; ils sont testés contre ceux de Rust sur les valeurs où le dépassement commence et s’arrête.
  - `None` et `[]` prennent le type qu’attend leur contexte.
  - JavaScript voit la valeur ou `undefined`, déclarée `T | undefined`.
- Le chargeur JavaScript préfixe par `$` chaque nom qu’il déclare, ce par quoi un nom Peras ne peut pas commencer : une fonction ou un paramètre nommé `r`, `url` ou `runtime` ne casse plus le module.
- L’IR vérifie la possession : poignées `owned` et `borrowed`, libérations explicites, et un vérificateur qui prouve que chaque bloc est libéré exactement une fois sur chaque chemin. L’allocateur de débogage est intégré aux modules qui utilisent le tas ; un bug mémoire lève une `PerasMemoryError` qui nomme les deux sites, en anglais ou en français. Rien dans le langage n’alloue encore.

### Banc d’essai

- `overflows` exerce les options et l’arithmétique vérifiée ; les deux backends s’accordent avec la référence JavaScript.
- `cmandel` calcule l’ensemble de Mandelbrot avec `struct Complex`. Les deux backends s’accordent avec la référence JavaScript. Le backend direct prend 15 ms contre 13 ms pour le code `f64` simple et pour Rust ; JavaScript, qui alloue un objet par nombre complexe, prend 49 ms. L’optimiseur ne touche pas encore aux fonctions à plusieurs résultats.

## 0.0.1 — 4 novembre 2026

La fin de la phase 0 du [plan de démarrage](docs/fr/kickoff-plan.md) : une fonction `.peras` se compile en WebAssembly et s’exécute dans le navigateur via Vite, et chaque erreur affiche un extrait, une cause et une correction, en anglais et en français.

### Langage et compilateur

- Lexer, parser, vérification des noms et des types pour les fonctions, `let` et `const`, `struct`, les tableaux, `if`, `while`, `for` et `for…of`. Toutes les erreurs sont signalées en une passe, avec un code stable, une cause, une correction sous forme de diff et une page `perasc explain`.
- 46 codes d’erreur, chacun avec sa page en anglais et en français ; on vérifie que l’exemple de chaque page produit son code et que sa correction le fait disparaître.
- `perasc check`, `perasc build` et `perasc explain`, avec `--lang en|fr`, `--json` et `--color`.
- Génération de code pour les fonctions sur des nombres et des booléens :
  - un backend WebAssembly direct, par défaut, qui ne demande aucun autre outil ;
  - un backend Rust, utilisé en CI pour contrôler le premier ([décision 0001](docs/fr/decisions/0001-webassembly-backend.md)).
- Optimisations de l’IR, chacune mesurée face à Rust et vérifiée par un vérificateur d’IR et un test différentiel :
  - élimination de la récursion avec accumulateur ;
  - déroulage de la récursion ;
  - rotation du test de sortie ;
  - `abs` entier sans branchement ;
  - doublement écrit comme une addition ;
  - sortie des invariants de boucle.

  Sur les noyaux du banc d’essai, le backend direct est au niveau du backend Rust.

### Outillage

- `vite-plugin-peras` importe un fichier `.peras` comme un module. Il affiche les erreurs dans l’overlay de Vite, fait échouer `vite build` en cas d’erreur et écrit les déclarations TypeScript à côté de la source.
- `examples/fib` exécute `fib` dans le navigateur.

### Contrôles qualité

Chaque contrôle bloque la fusion :

- formatage ;
- lints sans aucun avertissement ;
- tests unitaires, tests d’interface et tests de bout en bout dans Chromium sans interface ;
- `cargo deny` et `pnpm audit` sans CVE connue ;
- accord des deux backends ;
- traductions complètes ;
- fuzzing nocturne du lexer, du parser et de toute la chaîne.

### Limites connues

- Chaînes, tableaux, structures, méthodes et `Option` sont vérifiés mais pas encore compilés : la génération de code les signale avec `E0900`.
- L’ownership, les modules `.apeiron` et la bibliothèque standard arrivent dans les phases suivantes.
