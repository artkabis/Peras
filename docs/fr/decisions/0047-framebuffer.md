# 0047 : un framebuffer de pixels

_Original anglais : [../../en/decisions/0047-framebuffer.md](../../en/decisions/0047-framebuffer.md)._

_Statut : protocole enregistré le 2026-11-25, avant tout prototype, comme le demande la règle du projet ; les planchers de deux options mesurés d’abord (constat M041). Classée première, à égalité, par la [décision 0046](0046-what-to-build-next-again.md), dont la scène de particules, A3, ne pouvait pas écrire de framebuffer ; demandée par le propriétaire le 2026-11-25 comme première étape d’un lot avec les paquets, `create-peras` et `@frame`._

## Contexte

Ce que fait Peras aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Une image est un `Array<u8>`, quatre octets par pixel**, écrits un octet à la fois avec une vérification de son indice chacun (`crates/perasc-ir`, la décision 0037 pour les modules managed), effacée par une boucle d’écritures d’octets : Peras n’a pas de `fill` (décision 0046, rang 9). A3 écrit l’arrondi de `Uint8ClampedArray` à la main, `clamp8`, onze lignes.
- **Le canvas peut déjà lire la mémoire d’un module sans copie** : un tableau de nombres gardé dans le module est un `PerasArray`, dont `view()` est un tableau typé sur ses éléments (`crates/perasc-codegen-wasm/src/lib.rs`, le chargeur) ; un `Uint8ClampedArray` sur les mêmes octets fait une `ImageData`. Rien ne le nomme : un utilisateur écrit les quatre lignes, ou copie l’image.
- **Où va le temps d’A3**, profilé en retirant une chose à la fois en gardant le même résultat (constat M013), 21 tours dans un seul processus, face à son TypeScript : A3 tel qu’écrit 2,79 ; ses pixels en un `u32` chacun, écrits une fois et effacés comme 64 000 mots, le plancher d’un framebuffer, 2,10 ; sa grille sans la copie de la liste que donne le `get` d’une table, 2,07 ; les deux, 1,59. Le contrôle A/A, 0,97. Deux coûts d’environ 40 ms chacun sur 172 : les octets de l’image, ceux de cette décision ; les copies de la grille, constat F125, pas ceux de cette décision.

## Les options

- **f0, rien** : un `Array<u8>`, comme l’écrit A3. La référence.
- **f1, un guide** : l’image en `Array<u32>`, un mot par pixel, `r | g << 8 | b << 16 | a << 24`, effacée par une boucle ; une page du guide et les quatre lignes qui la donnent à un canvas. Rien dans le compilateur.
- **f2, un type écrit en Peras** : `Framebuffer` et `Rgba8` dans un fichier `.peras` qu’un programme importe (décision 0022), Peras n’ayant pas encore de bibliothèque écrite en Peras : la première chose que porteraient les paquets de la décision suivante. Une structure sur un `Array<u32>` avec `width`, `height`, `set(x, y, color)`, `get(x, y)`, `clear(color)`, et `Rgba8.clamped(r, g, b, a)` qui arrondit comme `Uint8ClampedArray` ; et, dans le chargeur, `toImageData()` sur la poignée, une vue et aucune copie. Le compilateur inchangé, sauf le `fill` d’un tableau, dont `clear` a besoin et que la décision 0046 a classé.
- **f3, un type que le compilateur connaît** : la surface de f2, son `set` abaissé en une écriture avec sa vérification, son `clear` en `memory.fill` ou en une boucle d’écritures larges, les bornes d’une image vérifiées une fois par appel là où le compilateur prouve les coordonnées (les invariants de la décision 0036). Construite seulement si f2 manque les critères de ce que le plancher de f3 regagnerait.

Le framebuffer d’un module managed, un `Uint8ClampedArray` lui-même, a la même surface ; il est mesuré pour f2 et f3, pas comme une option à part.

## Les programmes

- **A3**, la scène de particules de la décision 0046, inchangée sauf son image : 300 images de 320 sur 200, la même ligne que son TypeScript.
- **P2, une image pleine** : chaque pixel de 640 sur 400 calculé à chaque image (un plasma de `sin` et `cos`, 120 images), où l’image est tout le travail ; écrit d’abord en TypeScript, comme les programmes de la décision 0046.
- **P3, l’image vers un canvas** : P2 dessiné dans un canvas sous Chromium (`PERAS_CHROMIUM`), chaque image donnée par une vue ou par une copie, le temps de `putImageData` et celui de l’image mesurés à part.

## Ce qui est mesuré, enregistré maintenant

1. **Les résultats** : chaque version donne la ligne de son TypeScript, deux fois sur une même instance, construite pour la production (`--allocator tlsf`, constat M029).
2. **La vitesse** : le temps de chaque programme face à son TypeScript et face à f0, dans un seul processus, l’ordre alterné à chaque tour, 21 tours ; le contrôle A/A, le TypeScript face à lui-même et f0 face à lui-même, dans les mêmes tours (constat M042).
3. **La charge CPU** : le temps qu’un cœur est tenu occupé sur les 120 images de P3, comme la décision 0012 le mesure.
4. **La mémoire et les octets** : la mémoire du module à son pic, et ses octets, pour chaque option ; une borne de 1,10 fois ou une page de 64 Kio de plus (constat M037).
5. **La facilité** : A3 et P2 écrits pour chaque option : les lignes, les notions qu’un utilisateur apprend, les erreurs rencontrées, consignées exécution par exécution comme dans la décision 0046.
6. **Le compilateur** : `check-regression` pour f2 et f3, chaque module identique ou plus petit sauf ceux qui utilisent ce que l’option apporte (constat M038).

## Les critères, enregistrés maintenant

Une option est retenue quand, face à f0, sur A3 et P2 : son temps est au plus 0,85 fois celui de f0, la médiane du contrôle A/A entre 0,95 et 1,05 ; sa mémoire dans la borne ; elle écrit moins de lignes et ne rencontre pas plus d’erreurs ; P3 donne une image au canvas sans copie. Parmi les options retenues, celle qui demande le moins au compilateur ; f3 plutôt que f2 seulement si son temps est au plus 0,90 fois celui de f2. Si aucune n’est retenue, f0 reste, et le guide dit ce que f1 a mesuré.

## Ce que ceci ne peut pas montrer, dit maintenant

Les programmes sont ceux du projet, et ce que vaut un framebuffer dépend de ce que dessine un utilisateur : A3 dessine quelques pixels par image, P2 tous, les deux extrêmes. Le texte, les images et les traits dessinés dans l’image ne sont pas mesurés : ce sont des décisions ultérieures, que porterait le type de celle-ci. Les types SIMD (phase 6) ne sont pas une option ici ; les écritures larges de f3 sont celles du compilateur, pas de l’utilisateur.

## Enregistrement

### La spécification de P2 (2026-11-25, avant tout Peras)

`bench/frame/plasma.ts`, vérifié au plus strict de TypeScript (`bench/frame/tsconfig.json`) : un plasma de 640 sur 400, un sinus par colonne et par ligne calculé à chaque image, une palette de 256 couleurs, une ombre par ligne dont les produits s’arrondissent comme ceux d’un `Uint8ClampedArray` ; 120 images donnent `120|135900051`, en 289 ms sur cette machine. Les sinus sont des tables d’une ligne et d’une colonne, comme s’écrit un plasma, pour que l’image, et non les sinus, soit le travail. À partir de ce commit, il n’est plus changé.

_À venir : les prototypes, les mesures, la proposition._
