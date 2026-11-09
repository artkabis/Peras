# 0004 : Du texte sans une allocation par morceau : des vues, puis la fusion, choisies par le compilateur

_Version anglaise, qui fait foi : [../../en/decisions/0004-strings-without-allocation.md](../../en/decisions/0004-strings-without-allocation.md)._

## Contexte

Le noyau `words` du [banc d’essai dans le navigateur](../../../bench/README.fr.md#banc-dessai-dans-le-navigateur-benchbrowser) compte les mots d’un texte d’un million de caractères :

```peras build
export fn words(text: String): i32 {
  return text.split(" ").filter((w) => w.length > 0).length;
}
```

C’était le noyau le plus lent face à JavaScript. Mesurées étape par étape dans Chromium, ses 31 ms se répartissaient ainsi :

| Étape                                      | Temps |
| ------------------------------------------ | ----- |
| l’entrée du texte dans le module, en UTF-8 | 4 ms  |
| `split`, environ 200 000 morceaux          | 14 ms |
| `filter`, qui copie chaque morceau gardé   | 13 ms |

La frontière n’était pas le coût : l’allocation l’était, un bloc et une libération par morceau. Deux changements depuis ont amené le noyau à 17 ms, deux fois le temps de JavaScript : `filter` garde sur place les éléments d’un tableau temporaire au lieu de les copier, et une chaîne courte tient en un bloc au lieu de deux. Il reste un bloc par morceau.

## Ce que d’autres ont fait

- **La fusion.** _Deforestation_ (Wadler, TCS 1990) et _Stream fusion: from lists to streams to nothing at all_ (Coutts, Leshchinskiy et Stewart, ICFP 2007) suppriment les structures intermédiaires d’une chaîne d’opérations. Rust le fait à la main : `text.split(' ').filter(|w| !w.is_empty()).count()` n’alloue rien, car `split` donne un itérateur paresseux.
- **Les vues.** Les morceaux que donne le `split` de Rust sont des `&str`, une adresse et une longueur dans le texte, dont le vérificateur d’emprunts prouve la durée de vie (_Oxide: The Essence of Rust_, Weiss et al., 2019). Les chaînes d’Umbra (Neumann et Freitag, CIDR 2020), puis DuckDB, Velox et le `StringView` d’Apache Arrow, gardent jusqu’à 12 octets en ligne, et sinon un préfixe et une adresse.
- **La réutilisation.** _Perceus_ (Reinking, Xie, de Moura et Leijen, PLDI 2021), _Counting Immutable Beans_ (Ullrich et de Moura, IFL 2019) et _FP²: Fully in-Place Functional Programming_ (Lorenzen, Leijen et Swierstra, ICFP 2023) réécrivent sur place une valeur que personne d’autre ne tient. `filter` sur un tableau temporaire en est un cas.
- **Les régions.** _Region-Based Memory Management_ (Tofte et Talpin, Information and Computation, 1997), Cyclone (Grossman et al., PLDI 2002) et les arènes de Rust allouent en avançant un pointeur et libèrent une région entière d’un coup.

En Rust, le développeur choisit : `String` ou `&str`, un `Vec` ou un itérateur, et il écrit les durées de vie. Le compilateur de Peras sait déjà qui possède chaque valeur, combien de temps chacune vit, et quels temporaires chaque instruction crée. Il peut choisir la représentation lui-même, sans rien écrire dans le programme.

## Mesures

Avant de décider, les deux voies ont été mesurées là où elles mèneraient, avec des fonctions écrites à la main dans le runtime d’une compilation à part, dans Chromium, entrée du texte comprise, 30 exécutions après 3 de chauffe, trois fois :

| Variante                                          | Temps          | Face à JavaScript        |
| ------------------------------------------------- | -------------- | ------------------------ |
| `words` aujourd’hui                               | 19 à 21 ms     | 2 fois plus lent         |
| **fusionné** : aucun tableau                      | 7,3 à 7,6 ms   | **1,4 fois plus rapide** |
| **vues** : un tableau de paires (début, longueur) | 7,8 à 8,0 ms   | **1,3 fois plus rapide** |
| JavaScript                                        | 10,4 à 10,8 ms | 1                        |
| dont l’entrée du texte                            | 4,1 à 4,4 ms   | —                        |

Chaque variante a trouvé les mêmes 195 889 mots, et n’a rien laissé en vie.

## Décision

1. **Les vues d’abord.** Un morceau de texte que donnent `split`, `slice` ou `trim`, et qui ne survit pas au texte dont il vient, est une vue : un début et une longueur dans ce texte, pas une copie. Un tableau de tels morceaux est un seul tampon de paires. Les vues arrivent à 0,5 ms de la fusion, et elles sont plus générales : le tableau existe, on peut donc l’indexer, le garder un temps, le passer à une fonction.
2. **Le compilateur choisit, le programme ne change pas.** Une vue ne s’écrit jamais dans un programme et ne se voit jamais depuis JavaScript. Le compilateur en utilise une là où il prouve à la fois :
   - que le texte dont elle vient vit aussi longtemps qu’elle, et ne change pas entre-temps ; les règles d’emprunt le donnent déjà, comme pour toute valeur prêtée ;
   - que la vue ne sort pas de cette durée de vie : elle n’est pas renvoyée, donnée à un paramètre `sink`, rangée dans une structure ou un tableau qui vit plus longtemps, ni passée à JavaScript.

   Partout ailleurs, le morceau est copié à cet endroit, comme aujourd’hui : la copie passe de l’endroit où le morceau est créé à celui où il s’échappe, et seulement pour les morceaux qui s’échappent.

3. **Puis la fusion.** Une chaîne dont le producteur est `split`, `map` ou `filter` et le consommateur `length`, `reduce`, `some`, `every`, `find`, `findIndex` ou `for…of` devient une seule boucle, sans tableau entre les deux. La fusion change l’ordre des appels des fonctions (`f` puis `g` sur chaque élément, au lieu de `f` sur tous puis `g` sur tous) : elle ne s’applique donc que lorsque cela ne peut pas se voir, c’est-à-dire quand chaque fonction de la chaîne sauf la dernière ne peut ni s’arrêter sur une erreur ni appeler quoi que ce soit, la condition que l’optimiseur utilise déjà pour déplacer du code ([`is_simple`](../../../crates/perasc-ir/src/opt.rs)). Sinon la chaîne reste telle qu’écrite.
4. **Les résultats sont toujours ceux du programme tel qu’écrit.** Comme pour les autres optimisations, des tests comparent les deux formes sur chaque entrée, et sont cassés exprès pour montrer qu’ils détectent une forme fausse.

## État

**Les vues de `split` sont faites.** Là où le tableau que crée `text.split(sep)` est lu aussitôt, par une méthode (`filter`, `map`, `length`, `join`, `some`…) ou un `for…of`, ses morceaux sont des vues :

- Un bloc contient un en-tête de 16 octets par morceau, dont les octets sont ceux du texte, et les poignées vers eux. Une vue est un en-tête de chaîne ordinaire : toute opération sur les chaînes le lit sans changement, et une copie de lui, du tableau (`copy`, `slice`, `concat`) ou d’un morceau, est une chaîne ordinaire.
- Quand un argument de cette méthode, ou le corps de cette boucle, nomme la variable dont vient le texte, qui pourrait alors le changer, le tableau est d’abord copié. Un `filter` dont le résultat sort de l’instruction donne aussi une copie.
- Un séparateur d’un octet, le cas courant, est découpé par deux boucles simples sur les octets.
- `push` n’appelle plus le runtime que quand le tampon est plein, pour tous les tableaux.

Les tests exécutent le WebAssembly sur l’allocateur de débogage, qui empoisonne les octets libérés, et comparent chaque résultat au `split` de Rust, y compris quand le texte change pendant que ses morceaux servent. Retirer l’une ou l’autre copie, ou la conversion d’un tableau copié, les fait échouer ; il leur a fallu des textes de plus de 256 octets pour le voir, car un texte plus court garde ses anciens octets dans le bloc de son en-tête quand il grandit.

`words` est passé de 17 à 11,2 ms, contre 10,3 pour JavaScript ; dans le banc d’essai, de 0,51 à 0,87 fois la vitesse de JavaScript. Du reste, 4,2 ms sont l’entrée du texte, 4,8 le découpage et 2,3 le filtre.

**Ensuite :** les vues pour `slice` et `trim`, puis la fusion.

**Le SIMD, une option de la compilation.** Chercher le séparateur seize octets à la fois avait d’abord été estimé à 1,5 ms ; mesuré, cela en a ôté 4. Comme le runtime est dans chaque module qui utilise des chaînes ou des tableaux, le SIMD est maintenant un choix de la compilation, `--simd on|off` (`simd` pour Vite), activé par défaut : il commande ensemble les boucles vectorielles et la recherche du runtime, si bien qu’un module ne demande le SIMD que quand sa compilation le dit, jamais parce que l’optimiseur a trouvé une boucle. Un module qui s’en sert vérifie, avant de se charger, que le moteur l’a, et le dit sinon. Mesuré avec `scripts/bench-ab.mjs`, `--simd off` contre activé, 4 tours :

| Noyau                 | Changement, à chaque tour          |
| --------------------- | ---------------------------------- |
| words                 | −37,3 %, −35,5 %, −35,8 %, −34,5 % |
| grayscale             | −23,7 %, −28,4 %, −30,8 %, −30,0 % |
| particles, zéro copie | −19,7 %, −20,1 %, −19,7 %, −19,3 % |
| les cinq autres       | entre −2,3 % et +1,8 %             |

Aucune allocation de plus ; +671 octets gzip ; temps de compilation +4,8 %, dans le bruit d’une compilation comparée à elle-même. Aucune estimation ne décide plus d’une exigence de la plateforme : elle est d’abord mesurée.

## Écarté pour l’instant

- **Des régions par instruction.** Les temporaires d’une instruction pourraient être alloués en avançant un pointeur et libérés d’un coup à sa fin ; Peras les connaît exactement, ce que Rust ne peut pas. Cela change le modèle mémoire, et les vues prennent l’essentiel de son gain sur cette charge : elle aura sa propre décision, avec la règle `@frame` de la [décision 0003](0003-runtime-proved-at-compile-time.md).
- **Les chaînes courtes en ligne dans leur poignée**, comme celles d’Umbra. Cela change la représentation de toutes les chaînes et leur passage à JavaScript, pour un gain que les vues donnent déjà ici.

## Conséquences

- Une fois les morceaux gratuits, l’entrée du texte dans le module représente plus de la moitié de ce qui reste (4 ms sur 7,5). L’étape suivante mesure les deux façons de la raccourcir : `TextEncoder.encodeInto`, qui écrit directement dans la mémoire du module, une passe au lieu de deux, et les fonctions de chaînes JavaScript intégrées à WebAssembly, qui laisseraient le module lire le texte sans le copier.
- Le reste est la recherche du séparateur, un octet à la fois. La chercher seize octets à la fois avec le SIMD de WebAssembly, comme le font [les boucles vectorielles](../guide/javascript.md#les-boucles-vectorielles), vient avec.
- La feuille de route de la phase 3 renvoie ici ; le README du banc d’essai dit quelle étape a apporté quoi, mesures à l’appui.
