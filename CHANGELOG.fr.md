# Journal des modifications

_English version: [CHANGELOG.md](CHANGELOG.md)_

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; les versions suivent le [versionnage sémantique](https://semver.org/lang/fr/). Avant la 1.0, toute version peut casser le langage.

## Non publié

### Langage et compilateur

- **Un allocateur de production, TLSF** ([décision 0002](docs/fr/decisions/0002-tlsf-allocator.md)) : chaque allocation et libération en temps borné, classes de taille trouvées avec `i32.clz` et `i32.ctz`, blocs libérés fusionnés aussitôt avec leurs voisins, et fin du tas rendue. `vite build` le lie, le serveur de développement garde l’allocateur de débogage, et `perasc build --allocator tlsf` le choisit. Il est 2,7 fois plus rapide que l’allocateur de débogage sur un programme qui alloue 6 millions de blocs. Les deux allocateurs partagent les fonctions de tampons, écrites une seule fois.
- **Méthodes et interfaces.** `impl Point { fn norm(self): f64 { … } }` donne ses méthodes à une structure ou une énumération du module (`E0234` sinon).
  - `self`, `inout self` ou `sink self` vient en premier ; `p.shift(1.0)` n’a pas besoin de `&`, et `p` doit être un `let`. Une méthode sans `self` s’appelle sur son type, `Point.origin()`, et toute méthode peut s’appeler ainsi, son `self` en premier. `Self` nomme le type ; `impl<T> Stack<T>` donne ses méthodes à un type générique.
  - `interface Shape { fn area(self): f64; }` et `impl Shape for Circle { … }` : une implémentation fournit exactement les méthodes de l’interface (`E0235`), avec exactement leurs signatures (`E0236`).
  - Une interface est une contrainte, `fn total<T: Shape>(…)` : chaque instance de la fonction appelle la méthode de son type, résolue à la compilation, sans table à l’exécution. Utilisée comme type, elle est refusée (`E0237`).
- **Tuples** : `(i32, String)`, écrits `(1, "a")`, lus avec `.0`, `.1`, imbriqués `t.1.0`. Un tuple est une structure générique, gardée comme ses champs ; JavaScript voit un tableau, typé `[number, string]`.
- **Alias de types** : `type Meters = f64;` et `type Grid<T> = Array<Array<T>>;`, interchangeables avec ce qu’ils nomment ; un alias qui se contient est refusé (`E0232`).
- **Les constantes de module se compilent** : `const AREA: i32 = SIDE * SIDE;` est écrite là où elle sert. Sa valeur doit être connue à la compilation (`E0233`). Le `let` de module reste seulement vérifié.
- Une structure qui se contient à travers un tableau, `struct Tree { children: Array<Tree> }`, ne fait plus planter le compilateur par débordement de pile : elle est signalée comme pas encore compilée (`E0900`).
- Les **génériques**, sur les fonctions, les structures et les énumérations : `fn larger<T: Ord>(a: T, b: T): T`, `struct Pair<A, B>`, `enum Tree<T>`.
  - Six contraintes prédéfinies : `Eq`, `Ord`, `Numeric`, `Integer`, `Float` et `Copy`. Un corps générique est vérifié une fois, pour tous les types que ses paramètres peuvent représenter : une opération que ses contraintes ne permettent pas est refusée, avec une correction qui ajoute la contrainte (`E0228`) ; une utilisation avec un type qui n’en respecte pas une est refusée (`E0227`), de même qu’une contrainte inconnue (`E0226`).
  - Les arguments de type sont inférés des arguments et du type attendu, ou écrits à l’appel, `larger<i64>(a, b)` : les comparaisons enchaînées étant refusées, ce n’est jamais une comparaison. Quand rien ne les détermine, `E0229`.
  - Une valeur d’un paramètre de type se déplace, sauf si le paramètre est `Copy` ; `Ord` et `Numeric` l’impliquent.
  - Compilés par monomorphisation : une copie par jeu d’arguments de type utilisé par le module, nommée d’après eux (`larger__i64`), sans coût à l’exécution. Une fonction générique jamais appelée est vérifiée, pas compilée. Une fonction exportée ne peut pas être générique (`E0230`), et le code générique qui demanderait des copies pour des types toujours plus grands est refusé (`E0231`).
  - Les structures et énumérations génériques arrivent en JavaScript comme des interfaces et des unions nommées d’après leurs arguments, `Pair_i32_String`.
- `unwrapOr` avec une valeur de secours lue dans une variable contenant une chaîne ou un tableau ne produit plus de code refusé par le vérificateur : la valeur de secours est copiée.
- Les **structures** se compilent, comme des valeurs : chacune est remplacée par ses champs scalaires, structures imbriquées aplaties, si bien qu’elle ne touche jamais le tas.
  - Un paramètre structure devient un paramètre par champ.
  - Une fonction qui renvoie une structure renvoie un résultat WebAssembly par champ, et un tuple dans le backend Rust.
  - Les champs se lisent et s’écrivent, affectations composées comprises ; une structure se copie par affectation ; `p = Point(p.y, p.x)` échange.
  - Les appels qui renvoient une structure s’exécutent d’abord, dans des temporaires, sans changer l’ordre d’évaluation : à droite de `&&` et `||` seulement quand ils s’exécuteraient, et dans une condition de boucle à chaque tour.
  - Les fonctions exportées prennent et renvoient des objets JavaScript, déclarés comme interfaces TypeScript.
- **`Result<T, E>`** : `Ok(valeur)` ou `Err(erreur)`, `isOk`, `isErr`, `unwrapOr`, et `?`, qui renvoie l’erreur tout de suite ; la fonction doit renvoyer un résultat avec le même type d’erreur, ou une option pour `?` sur une option (`E0225`). L’erreur se déplace dans le retour, buffers compris.
- L’exhaustivité et les bras inatteignables sont maintenant vérifiés par l’algorithme de Maranget, si bien que les motifs imbriqués comptent : `Err(Empty)` et `Err(NotADigit(_))` couvrent ensemble `Err`. Un type du module peut prendre le nom d’un type intégré, comme `Result`.
- **Énumérations avec données, et `match`.** `enum Shape { Circle(f64), Rect(f64, f64), Empty }` ; les valeurs s’écrivent `Shape.Circle(1.0)`.
  - `match` décompose une valeur avec des motifs : `_`, noms, nombres et booléens, variantes avec leurs données, imbriqués ; les bras sont des valeurs ou des blocs, et `return`, `break`, `continue` y marchent.
  - Chaque `match` doit couvrir chaque valeur (`E0222`) ; une variante inconnue (`E0223`) et un bras qu’aucune valeur n’atteint (`E0224`) sont aussi des erreurs. Un `match` dont tous les bras renvoient termine la fonction.
  - Les options se filtrent aussi (`Some(x)`, `None`), et une option ou une énumération peut désormais contenir des chaînes et des tableaux : les données des variantes non prises sont des buffers vides, si bien que la valeur possède toujours les mêmes. `pop()` sur un tableau de chaînes marche désormais.
  - JavaScript voit `{ kind: "Circle", values: [1] }`, déclaré à TypeScript comme une union.
- Les **tableaux de chaînes et de tableaux** se compilent, ainsi que les structures qui en contiennent dans des tableaux : un buffer peut posséder des buffers, que le runtime libère et copie avec lui, à toute profondeur. JavaScript les passe et les reçoit en tableaux et objets imbriqués.
- E0204 raconte l’histoire de la valeur : où elle a reçu sa valeur, où elle a été déplacée, où elle est réutilisée.
- Les **tableaux de structures et d’options** se compilent, en colonnes : un buffer par scalaire d’un élément, si bien que `points[i].x` et une boucle sur un seul champ ne touchent qu’un buffer. L’indexation, `points[i].x = v`, `push`, `pop`, `copy`, `==` et `for…of` marchent colonne par colonne ; JavaScript voit des tableaux d’objets.
- **`inout`** marche sur tous les types : la fonction rend ses nombres en résultats supplémentaires, réécrits dans les variables de l’appelant ; tableaux et chaînes changent sur place. Depuis JavaScript, une structure passée `inout` est un objet dont les champs sont mis à jour. Une variable passée `inout` ne peut pas être lue ailleurs dans la même instruction (`E0207`), et deux champs différents d’une structure ne comptent plus comme des alias.
- `?` libère maintenant les tableaux et les chaînes que la fonction détient avant de renvoyer `None` ; ils fuyaient avant.
- Les **chaînes** se compilent, en UTF-8 : littéraux, `+`, `+=`, `==`, `!=`, `length` en octets et `copy()`. Un littéral est stocké une seule fois, en segment de données passif, et copié dans un nouveau buffer avec `memory.init` ; `+=` ajoute sur place. JavaScript passe et reçoit des chaînes, encodées et décodées en UTF-8. `==` compare aussi les tableaux, élément par élément.
- Les **tableaux** se compilent, de nombres et de booléens, sur le tas : littéraux, `length`, lecture et affectation par indice, `push`, `pop`, `copy`, `for…of`.
  - Chaque indice est vérifié ; un indice hors limites arrête le programme avec une `RangeError`, en anglais ou en français, avant de toucher la mémoire.
  - Un tableau est un en-tête de 12 octets, qui garde son adresse, et ses éléments ; `push` double la place quand elle manque. L’allocateur de débogage tient les deux blocs, si bien que chaque bug mémoire reste détecté.
  - JavaScript passe et reçoit des copies, en `number[]`, `bigint[]` ou `boolean[]` ; un paramètre `inout` est recopié.
- La **possession** est vérifiée : une valeur sur le tas a un seul propriétaire, se déplace à l’affectation, au `return` et vers `sink`, et est empruntée sinon.
  - Quatre nouvelles erreurs : une valeur utilisée après son déplacement ([E0204](docs/fr/errors/E0204.md)), `&` manquant ou en trop sur un argument `inout` ([E0206](docs/fr/errors/E0206.md)), deux arguments qui atteignent une variable que l’un modifie, ou un tableau modifié pendant que `for…of` le parcourt ([E0207](docs/fr/errors/E0207.md)), le déplacement de ce qui n’est qu’emprunté ([E0209](docs/fr/errors/E0209.md)). Chaque correction ajoute `.copy()`, `&` ou `sink` là où c’est possible.
  - Le lowering insère chaque libération : en fin de portée, à la réaffectation, avant `return`, `break` et `continue`, et là où des chemins se rejoignent après un déplacement sur l’un d’eux. Le vérificateur de libérations prouve chacune ; les programmes de test ne laissent aucun bloc vivant, dans l’interpréteur et en WebAssembly sur l’allocateur de débogage.
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
