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
- **Démarrage** : 8,8 Kio gzip pour les huit noyaux, contre 2,4 pour leur JavaScript, et l’import prend environ 1,7 fois plus longtemps (18 ms contre 11 ici). Un module n’emporte que les fonctions du runtime qu’il appelle, son runtime intégré élagué à celles-ci, et que les aides du chargeur dont ses fonctions se servent : 13,3 Kio avant pour ces noyaux (−34 %), et pour un petit module, une fonction sur un tableau ou un texte, 4,5 à 5,7 Kio contre 11,3 à 11,5 (−49 à −59 %). Le temps d’import n’a pas suivi les octets : ce qui reste, c’est compiler et instancier deux modules, le runtime et le programme, et une seconde requête, le `.wasm` demandé une fois le chargeur exécuté. Un seul module au lieu de deux, et une indication qui télécharge le `.wasm` avec le chargeur, sont les étapes suivantes.

Ce qu’apporte Peras, donc, c’est moins une vitesse que JavaScript ne peut pas atteindre que la vitesse de l’expert à partir du code habituel : `text.split(" ").filter(…).length` et un tableau de structures vont aussi vite, ou plus vite, que le JavaScript en lequel un expert les réécrit.

### Face à Rust et AssemblyScript

`node scripts/bench-browser.mjs --contenders` compile aussi les noyaux en Rust ([`browser/rust`](browser/rust/src/lib.rs), wasm-bindgen 0.2.130) et en AssemblyScript ([`browser/assemblyscript`](browser/assemblyscript/kernels.ts), 0.28.20), les deux autres façons d’écrire du WebAssembly pour le web, et les exécute dans la même page. Chacun est écrit comme on écrit habituellement ce langage : une structure ou une classe par corps, `split` et `filter`, le tri de la bibliothèque standard. Chacun est compilé pour la vitesse : Rust en release avec LTO et SIMD, AssemblyScript en `-O3` avec le SIMD permis. Leurs tableaux restent dans la mémoire de leur module, comme le fait un `PerasArray`. Chaque variante donne exactement le même résultat, vérifié dans la page ; la CI le vérifie à chaque pull request.

Trois exécutions, Chromium 153 ; le temps du concurrent divisé par celui de Peras, au-dessus de 1 quand Peras est plus rapide :

| Noyau     | Rust / Peras | AssemblyScript / Peras |
| --------- | -----------: | ---------------------: |
| particles |  1,49 – 1,52 |              4,8 – 4,9 |
| grayscale |  1,28 – 1,33 |              2,4 – 2,5 |
| sort      |  1,30 – 1,43 |              5,1 – 5,5 |
| words     |  1,44 – 1,48 |                    7,3 |
| nbody     |         0,98 |                    3,3 |
| hash64    |         1,00 |             1,0 – 1,04 |
| mandel    |  0,94 – 0,98 |            0,98 – 1,01 |
| calls     |  1,02 – 1,05 |             1,0 – 1,02 |

Téléchargement, gzip : Peras 8,8 Kio (13,1 avant que son runtime soit élagué à ce qu’un module appelle), Rust 16,1 Kio, AssemblyScript 6,1 Kio.

Ce que cela dit :

- **Là où le code est le même, la vitesse aussi.** n-body, le hachage, Mandelbrot et les appels donnent à Rust et à Peras le même code machine, à 6 % près : le WebAssembly de Peras y vaut celui de LLVM.
- **Là où Peras devance Rust, son compilateur en fait plus pour le code habituel**, ce n’est pas une meilleure génération de code : la boucle des particules, avec sa condition, tourne sur quatre voies sous un masque, le tri est un tri par base là où la bibliothèque standard de Rust compare, et le temps de Rust pour `words` comprend le passage du texte par wasm-bindgen, qui copie les caractères ASCII un par un avant `encodeInto`. Pourquoi grayscale est 1,3 fois plus rapide en Peras, les deux étant vectorisés, n’est pas encore analysé.
- **C’est du Rust idiomatique, pas celui d’un expert.** Un expert utiliserait les intrinsèques de `std::arch`, une crate de tri par base comme `radsort`, et passerait le texte en octets : ce Rust n’a pas été mesuré, et comblerait probablement les écarts ci-dessus. La lecture juste est « aussi rapide que Rust, à partir de code écrit comme du TypeScript », pas « plus rapide que Rust ».
- **AssemblyScript** est 2,4 à 7,3 fois plus lent là où il alloue ou parcourt des tableaux : son optimiseur ne vectorise pas, ses tableaux typés vérifient chaque indice, ses objets et ses chaînes passent par son ramasse-miettes. C’est le plus petit téléchargement. Un expert écrirait `unchecked()` et `StaticArray` ; pas mesuré non plus.

### Web Workers

`node scripts/bench-workers.mjs` mesure ce que rapporte le découpage d’un noyau entre des Web Workers ([résultats](browser/WORKERS.md)). Un module compilé par `perasc` tourne tel quel dans un worker de module : chaque worker charge sa propre instance, avec sa propre mémoire. La page coupe les données en N parts, les transfère à N workers, chacun copie sa part dans la mémoire de son module, exécute le noyau, recopie le résultat et le transfère ; la page rassemble les parts. Tout est compté, face au noyau appelé directement sur le fil principal, ses tableaux déjà dans la mémoire du module. Chaque résultat découpé est vérifié égal au résultat direct, élément par élément ; la CI exécute la vérification.

Sur 4 cœurs, Chromium 153, sur 2 à 4 exécutions :

| Noyau              | Direct     | 4 workers  |  Accélération |
| ------------------ | ---------- | ---------- | ------------: |
| particles, 200 pas | 243–263 ms | 86–100 ms  | **2,6 – 2,9** |
| words              | 2,9–3,8 ms | 2,5–3,2 ms |     1,1 – 1,2 |
| particles, 10 pas  | 13–18 ms   | 18–27 ms   |   0,64 – 0,73 |
| grayscale          | 4,0–4,9 ms | 21–29 ms   |   0,17 – 0,19 |

Ce que cela dit :

- **Les workers paient là où le travail l’emporte sur le déplacement des données.** 200 pas sur un million de particules, vingt fois le travail de 10 pour les mêmes octets, vont 2,6 à 2,9 fois plus vite sur 4 cœurs. Un seul passage d’un filtre sur 16 Mo de pixels va 5 fois moins vite : couper, transférer, copier à l’aller et au retour coûtent environ 20 ms, le filtre 4.
- **Ce qui manque, c’est la mémoire partagée, et son plafond est mesuré.** Chaque worker a sa propre mémoire, les données entrent et sortent donc par copie. Une dernière façon, _4 workers, données résidentes_, garde chaque part dans la mémoire de son worker et ne chronomètre que le noyau et les réponses : le mieux que pourraient faire des threads WebAssembly sur un seul `SharedArrayBuffer`. Grayscale passe de 0,32–0,35 à 1,84–2,02 fois la vitesse directe, 10 pas de particules de 0,80–0,83 à 2,08–2,22, 200 pas de 2,7–3,0 à 3,0–3,3 (3 exécutions, un jour où cette machine faisait tourner les noyaux directs environ deux fois plus lentement). La [décision 0007](../docs/fr/decisions/0007-threads-over-shared-memory.md) les construit, découpés par possession ; une page qui les utilise doit être servie avec les en-têtes COOP et COEP.
- **`parallel for` approche ce plafond.** `bench/browser/parallel.peras` écrit les boucles de particles et de grayscale en `parallel for`, compilées avec `--threads on` ; la page est servie isolée entre origines. Sur 4 fils, particles va 1,95 à 2,23 fois plus vite que l’appel direct pour 10 pas, 2,68 à 2,81 pour 200, grayscale 1,70 à 1,87, sa somme de contrôle calculée par une seconde boucle sur un fil (2 exécutions). Chaque résultat est vérifié égal au résultat direct, élément par élément.
- **Rien de cela n’est propre à Peras** : JavaScript, Rust ou AssemblyScript dans des workers paient les mêmes copies.

### Ce qu’une revue indépendante demanderait encore

Ces points sont connus et pas encore faits ; chacun est une raison de lire les chiffres ci-dessus avec prudence.

- **Des concurrents experts** : Rust et AssemblyScript sont mesurés tels qu’on les écrit habituellement (ci-dessus) ; les versions de leurs experts ne le sont pas.
- **Toutes les mesures dans le dépôt** : les prototypes et sondes derrière les décisions 0004 et 0005, et les mesures des flottants en texte et d’`encodeInto`, ont été exécutés depuis un dossier de travail ; ils doivent être versionnés sous `bench/`, avec une commande chacun, comme l’est maintenant [`bench/prototypes`](prototypes/outer-simd/README.fr.md).
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
