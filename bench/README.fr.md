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
- Il ne mesure pas la vitesse : le banc d’essai dans le navigateur, plus bas, s’en charge.

`bench/bugs` contient du code volontairement faux : l’ESLint et le Prettier du dépôt l’ignorent.

## Comparaison des backends (`bench/backends`)

`bench/backends/kernels.peras` contient quatre noyaux : récursion, boucles entières, boucles flottantes et arithmétique 64 bits. `node scripts/bench-backends.mjs` les compile avec les deux générateurs (WebAssembly direct, et Rust via `cargo` et `wasm-bindgen`), vérifie que chaque build renvoie les mêmes résultats qu’une référence JavaScript, puis mesure le temps de build, la taille du module (brute, gzip, après `wasm-opt -O3`) et le temps d’exécution dans Node. Un build direct avec `--no-optimize` montre ce qu’apportent les optimisations de l’IR ; `--check` ne fait que comparer les résultats, pour la CI. Il demande `rustup` et `wasm-bindgen` 0.2.130 pour le backend Rust, et écrit [`backends/RESULTS.md`](backends/RESULTS.md). La décision [0001](../docs/fr/decisions/0001-webassembly-backend.md) discute les résultats.

## Banc d’essai dans le navigateur (`bench/browser`)

`bench/browser/kernels.peras` contient huit noyaux qu’exécute une application web : des particules déplacées sur place, un filtre d’image, la gravité entre des structures, un hachage sur 64 bits, un tri, de l’arithmétique flottante, le découpage d’un texte, et un appel depuis JavaScript. `bench/browser/reference.js` contient les mêmes fonctions en JavaScript, deux fois là où cela compte : comme on écrit habituellement JavaScript (tableaux typés pour les nombres, objets pour les structures, `BigInt` pour les entiers 64 bits, `split` et `filter`), et comme un expert l’écrit pour la vitesse (une boucle sur les caractères, deux moitiés de 32 bits, des colonnes de `Float64Array`, un tri par base). Les particules, les pixels, Mandelbrot et les appels sont déjà écrits comme le ferait un expert.

`node scripts/bench-browser.mjs` compile les noyaux avec l’allocateur TLSF, les sert, et les exécute dans Chromium à côté de leurs jumeaux JavaScript. Dans la page, il vérifie d’abord que chaque variante donne exactement le même résultat ; puis il chronomètre chacune, en alternant : un appel à froid, 3 appels de chauffe, 15 appels mesurés. Il donne deux rapports : face à JavaScript tel qu’on l’écrit habituellement, et face au JavaScript le plus rapide. Il mesure aussi le téléchargement et l’import de chaque module dans un contexte de navigateur neuf. `--check` ne fait que comparer les résultats, sur de petites entrées, pour la CI. Il écrit [`browser/RESULTS.md`](browser/RESULTS.md).

Ce qu’il mesure, et ce qu’il ne mesure pas :

- Il mesure un moteur exécutant du WebAssembly face au même moteur exécutant du JavaScript, dans la même page, sur les mêmes entrées. Les temps varient beaucoup d’une machine à l’autre ; les rapports beaucoup moins.
- Les noyaux ont été choisis pour couvrir les deux côtés : ceux où Peras devrait gagner et ceux où il devrait perdre (appels très courts, arithmétique flottante seule). Tous figurent dans les résultats.
- Le JavaScript a été écrit par les auteurs de Peras. Les versions expertes ont été ajoutées après la question d’un lecteur extérieur : les chiffres tiendraient-ils face à du JavaScript écrit pour la vitesse ? Certains n’ont pas tenu. Un meilleur JavaScript est bienvenu : un jumeau plus rapide qui donne le même résultat remplace le nôtre, et les chiffres avec lui.
- Il ne mesure ni la mémoire, ni les pauses du ramasse-miettes sur une longue session, ni une application entière.

Ce que disent les résultats aujourd’hui, face au JavaScript **le plus rapide** (trois exécutions, Chromium 153, une machine cloud à 4 cœurs) ; face à JavaScript tel qu’on l’écrit habituellement entre parenthèses :

- **Entiers 64 bits** : 5,4 fois plus rapide (20 fois face à `BigInt`). WebAssembly a des entiers 64 bits ; JavaScript doit les couper en deux.
- **Nombres dans des tableaux** (particules, pixels) : 2,1 à 2,4 et 1,6 à 1,8 fois plus rapide, quand le tableau est un `PerasArray` ; 1,1 à 1,3 quand un tableau ordinaire est copié à l’aller et au retour. Les vérifications de bornes prouvées inutiles et les boucles SIMD font la différence ([bounds.rs](../crates/perasc-ir/src/bounds.rs), [simd.rs](../crates/perasc-codegen-wasm/src/simd.rs)).
- **Tri** : 1,8 à 1,9 fois plus rapide qu’un tri par base écrit en JavaScript (3,8 à 4,2 fois plus que `Float64Array.prototype.sort` dans V8, 17 fois plus que `array.sort((a, b) => a - b)`). Dans WebKit, dont le tri natif est rapide, Peras était plus lent que lui (0,88).
- **Texte** : 1,6 à 1,7 fois plus rapide qu’une boucle sur les caractères (3,2 à 3,3 fois plus que `split` et `filter`). Les étapes sont dans la [décision 0004](../docs/fr/decisions/0004-strings-without-allocation.md).
- **Structures** (n-body) : **la même vitesse** (0,97 à 1,01) qu’un JavaScript qui range les champs en colonnes de `Float64Array`, ce que fait Peras ; 1,5 fois plus rapide qu’un objet par corps.
- **Arithmétique flottante seule** (Mandelbrot) : la même vitesse (0,98 à 1,07).
- **Appels** : un appel dans le module coûte 1,4 à 1,7 fois un appel JavaScript, que le moteur intègre. Le travail doit franchir la frontière par gros morceaux.
- **Démarrage** : 13 Kio gzip de plus à télécharger, et l’import prend environ deux fois plus longtemps (19 ms contre 9,5), surtout l’allocateur intégré au chargeur. C’est payé une fois par module.

Ce qu’apporte Peras, donc, c’est moins une vitesse que JavaScript ne peut pas atteindre que la vitesse de l’expert à partir du code habituel : `text.split(" ").filter(…).length` et un tableau de structures vont aussi vite, ou plus vite, que le JavaScript en lequel un expert les réécrit.

### Ce qu’une revue indépendante demanderait encore

Ces points sont connus et pas encore faits ; chacun est une raison de lire les chiffres ci-dessus avec prudence.

- **D’autres concurrents** : les mêmes noyaux en Rust compilé en WebAssembly, et en AssemblyScript, les alternatives à un nouveau langage.
- **Toutes les mesures dans le dépôt** : les prototypes et sondes derrière les décisions 0004 et 0005, et les mesures des flottants en texte et d’`encodeInto`, ont été exécutés depuis un dossier de travail ; ils doivent être versionnés sous `bench/`, avec une commande chacun.
- **Statistiques** : des intervalles de confiance (bootstrap sur les exécutions) plutôt que des médianes seules ; plus d’exécutions ; les niveaux d’optimisation du moteur indiqués.
- **Machines** : macOS avec le Safari d’Apple, Windows, un téléphone ARM ; le chronomètre de WebKit compte des millisecondes entières, trop grossier pour des noyaux de 2 ou 3 ms.
- **Une application entière**, mesurée du chargement à la dernière image.

Des idées qui vont à l’encontre de ce qui a été fait jusqu’ici, à mesurer avant de les garder :

- **Compiler en JavaScript, exprès.** Une petite fonction appelée en boucle depuis JavaScript coûte plus à la frontière que son travail (les appels ci-dessus). Le compilateur pourrait l’émettre en JavaScript, comme le fera le mode géré, et garder WebAssembly pour le reste : le choix fait fonction par fonction, sur mesure.
- **Laisser l’hôte trier là où il trie plus vite.** Le tri natif de WebKit bat celui de Peras ; un module pourrait chronométrer les deux une fois au chargement sur un petit tableau et garder le plus rapide, comme le chargeur sonde déjà le SIMD.
- **Les flottants en texte par deux chemins.** Un flottant de valeur entière, le cas courant, peut être écrit par le chemin des entiers du runtime ; seuls les autres ont besoin du `String()` de l’hôte ([méthodes](../docs/fr/reference/methods.md)).
- **Des tables de deux dispositions.** Les tables suisses pour celles dont l’ordre ne se voit pas, l’ordre d’insertion pour les autres ([décision 0005](../docs/fr/decisions/0005-maps-and-sets.md)).

## Banc A/B (`scripts/bench-ab.mjs`)

Deux compilations de Peras, comparées sur les noyaux de `bench/browser` dans une même page Chromium, pour décider d’un changement sur des mesures :

```sh
node scripts/bench-ab.mjs --a ref:HEAD --b current           # un commit face à l’arbre de travail
node scripts/bench-ab.mjs --a current --a-flags "--simd off" --b current   # une option
```

A et B sont chacun `current` (l’arbre de travail), `ref:<référence git>` (compilé dans un worktree sous `target/bench-ab`) ou `bin:<chemin>`, avec leurs propres options de perasc. Chacun des 4 tours est une page neuve où A, B et JavaScript alternent, 30 appels mesurés après 3 de chauffe ; A passe en premier aux tours impairs et B aux tours pairs, pour qu’aucun ne prenne tous les premiers appels, les plus froids. Chaque variante doit donner le même résultat et ne rien laisser en vie. Le runtime de chaque compilation compte les allocations d’un appel de chaque variante. Puis la taille, le temps d’import dans un contexte neuf et le temps de compilation de chaque compilation. Le rapport va dans `target/bench-ab/REPORT.md` ; `--check` vérifie seulement que les deux compilations concordent, pour la CI.

Le rapport applique ces seuils :

| Indicateur                  | Seuil                                                                                       |
| --------------------------- | ------------------------------------------------------------------------------------------- |
| La médiane d’un noyau       | un gain compte à 10 % ou plus à **chaque** tour, une régression à 3 % ou plus à chaque tour |
| Allocations par appel       | ne doivent pas augmenter                                                                    |
| Fuites, doubles libérations | aucune, vérifié par la page                                                                 |
| Téléchargement              | au plus +1 Kio gzip pour un changement                                                      |
| Temps de compilation        | au plus +10 %                                                                               |

Ils ont été calibrés en comparant une compilation à elle-même : chaque noyau est resté entre −2,9 % et +1,8 %, le temps de compilation dans +4,8 %. Sur cette machine, deux exécutions séparées du banc dans le navigateur diffèrent jusqu’à 10 % ; la même page le garde sous 3 %.

## D’autres moteurs, et de vrais appareils

Les deux scripts du navigateur prennent `--browser chromium|firefox|webkit`, Chromium par défaut. WebKit est le moteur de Safari, JavaScriptCore, tel que Playwright le compile pour Linux : proche de Safari, pas la compilation d’Apple. Les résultats d’un autre moteur vont dans `target/bench-browser/RESULTS-<moteur>.md` ; le `RESULTS.md` commité reste celui de Chromium. La CI lance les deux bancs dans WebKit et met leurs rapports dans le résumé de l’exécution.

Pour mesurer un vrai Safari, sur un Mac ou un iPhone, `--serve` compile la page et la sert au lieu de piloter un navigateur ; la page affiche elle-même ses résultats :

```sh
node scripts/bench-ab.mjs --a current --a-flags "--simd off" --b current --serve --host 0.0.0.0 --port 8080
# puis ouvrir http://<cette machine>:8080/index.html?first=A, et ?first=B, sur l’appareil
```

`--host 0.0.0.0` sert le dossier au réseau local, pour un téléphone ; arrêter le script ensuite. Tant que les mesures d’un moteur ne sont pas là, rien n’est affirmé à son sujet : les chiffres de ce README sont ceux de Chromium.
