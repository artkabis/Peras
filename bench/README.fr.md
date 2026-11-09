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

`bench/browser/kernels.peras` contient huit noyaux qu’exécute une application web : des particules déplacées sur place, un filtre d’image, la gravité entre des structures, un hachage sur 64 bits, un tri, de l’arithmétique flottante, le découpage d’un texte, et un appel depuis JavaScript. `bench/browser/reference.js` contient les mêmes fonctions en JavaScript, écrites comme les écrirait un développeur soucieux de vitesse : tableaux typés pour les nombres, objets simples pour les structures, `BigInt` pour les entiers 64 bits.

`node scripts/bench-browser.mjs` compile les noyaux avec l’allocateur TLSF, les sert, et les exécute dans Chromium à côté de leurs jumeaux JavaScript. Dans la page, il vérifie d’abord que chaque variante donne exactement le même résultat ; puis il chronomètre chacune, en alternant : un appel à froid, 3 appels de chauffe, 15 appels mesurés. Il mesure aussi le téléchargement et l’import de chaque module dans un contexte de navigateur neuf. `--check` ne fait que comparer les résultats, sur de petites entrées, pour la CI. Il écrit [`browser/RESULTS.md`](browser/RESULTS.md).

Ce qu’il mesure, et ce qu’il ne mesure pas :

- Il mesure V8 exécutant du WebAssembly face à V8 exécutant du JavaScript, dans la même page, sur les mêmes entrées. Les temps varient beaucoup d’une machine à l’autre ; les rapports beaucoup moins (moins de 10 % entre deux exécutions).
- Les noyaux ont été choisis pour couvrir les deux côtés : ceux où Peras devrait gagner et ceux où il devrait perdre (texte, appels très courts). Tous figurent dans les résultats.
- Le JavaScript est honnête sans être héroïque : ni WebAssembly écrit à la main, ni SIMD, ni Web Workers, d’un côté comme de l’autre.
- Il ne mesure ni la mémoire, ni les pauses du ramasse-miettes sur une longue session, ni une application entière.

Ce que disent les résultats, aujourd’hui :

- **Nombres dans des tableaux** (particules, pixels) : Peras est 1,8 à 2,2 fois plus rapide quand le tableau est un `PerasArray`. Une boucle comptée abandonne les contrôles d’indice qu’elle prouve (`crates/perasc-ir/src/bounds.rs`), ce qui a fait passer les particules de 1,2 à 1,4 ; ses tours s’exécutent ensuite plusieurs à la fois avec le SIMD de WebAssembly, branches comprises (`crates/perasc-codegen-wasm/src/simd.rs`), ce qui a porté les particules à 1,8 et le filtre de gris de 1,2 à 2,2. Passer un tableau ordinaire le copie à l’aller et au retour, ce qui coûte plus que le travail : c’est le zéro copie qui fait le gain.
- **Arithmétique flottante seule** (Mandelbrot) : la même vitesse (0,98 à 1,07 selon l’exécution). V8 compile déjà très bien ce JavaScript, et Peras vérifie chaque indice et chaque opération entière.
- **Structures** (n corps) : 1,5 à 1,8 fois plus rapide. Un tableau de structures est rangé par colonnes en mémoire linéaire, là où JavaScript a un objet par élément.
- **Entiers 64 bits** : environ 20 fois plus rapide. JavaScript n’a que `BigInt`, qui alloue.
- **Tri** : 4,2 fois plus rapide que `Float64Array.prototype.sort`, que V8 exécute en code natif, et 17 fois plus rapide que l’habituel `array.sort((a, b) => a - b)`. Le runtime trie les nombres par leurs bits, un tri par base en quelques passes, là où un tri par comparaison demande une vingtaine de comparaisons par nombre ; c’était un tri par tas, 2,3 fois plus lent que V8, auparavant.
- **Texte** : 3,4 fois plus lent. La chaîne est encodée en UTF-8 pour entrer dans le module à chaque appel, et chaque morceau de `split` est un nouveau bloc. Le texte doit rester du côté où il est utilisé.
- **Appels** : un appel vers le module coûte environ 1,4 fois un appel JavaScript, que V8 met en ligne. Le travail doit traverser la frontière par gros morceaux.
- **Démarrage** : environ 10 Kio gzip de plus à télécharger, et l’import prend environ deux fois plus de temps (17 ms contre 9), surtout à cause de l’allocateur inclus dans le chargeur. Il se paie une fois par module.
