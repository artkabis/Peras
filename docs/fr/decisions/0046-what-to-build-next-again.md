# 0046 : Que construire ensuite, mesuré de nouveau

_Version anglaise : [../../en/decisions/0046-what-to-build-next-again.md](../../en/decisions/0046-what-to-build-next-again.md)._

_Statut : protocole enregistré le 2026-11-25, avant qu’aucun programme soit écrit, comme le demande la règle du projet ; programmes, mesure et classement consignés le même jour, la décision du propriétaire consignée le même jour. Demandée par le propriétaire le 2026-11-25 : décider de la suite par la mesure, comme l’a fait la [décision 0011](0011-what-to-build-next.md), plutôt que par l’ordre de la [feuille de route](../reference/roadmap.md)._

## Contexte

La dernière fois que ce qu’il faut construire a été classé par la mesure, c’était la décision 0011, il y a trente-quatre décisions : dix-sept bogues d’applications web et une étude de cas écrite depuis TypeScript. Depuis, les décisions ont suivi l’ordre de la feuille de route, et trois de suite ont gardé ce qui était là : les arènes (0043) et les pools (0044) ont été mesurés et pas gardés, et le premier verdict de la 0045 aussi. Entre-temps, la seule friction trouvée en écrivant des programmes comme un utilisateur, une option non restreinte à travers `&&` et `||` (constat F118), valait une décision à elle seule : huit erreurs sur cinq programmes, aucune depuis la 0045.

Les frictions connues et pas encore classées, chacune d’un constat : le JSON dans les modules owned (`E0900`), un `let` jamais restreint, les lectures d’un champ que les outils ne savent pas résoudre (F313), un test en échec qui ne peut montrer ni tableau ni structure (F227), aucune allocation dans un tour d’un `parallel for` (F301), des variables de module qui ne peuvent être ni chaînes ni tableaux (décision 0023), les paquets (phase 5). Et l’étape suivante de la feuille de route, la règle `@frame`.

## Méthode, enregistrée avant qu’aucun programme soit écrit

### Les programmes

Quatre applications, chacune écrite d’abord en TypeScript, la spécification, puis en Peras en la traduisant comme le ferait un développeur venu de TypeScript, sans la remodeler pour ce que Peras offre. Elles sont choisies pour couvrir les deux modes et les sortes de travail que nomme la feuille de route, et aucune n’est un programme que le dépôt a déjà :

- **A1, des notes** (un module managed) : une liste de notes gardée dans le stockage de la page en JSON, cherchée à mesure que l’utilisateur tape, triée par date, avec des étiquettes ; le DOM, les événements, les minuteurs, le stockage, le JSON.
- **A2, un livre de comptes** (un module owned appelé depuis un module managed) : un CSV de transactions lu, regroupé par mois et par catégorie, avec un solde courant et les plus grands mouvements ; chaînes, tables, tri, montants d’argent.
- **A3, une scène de particules** (un module owned) : une image de particules qui naissent, bougent, se heurtent sur une grille et meurent, dessinées dans un framebuffer ; tableaux de structures, le travail d’une image, ce que `@frame` régirait.
- **A4, un calcul d’itinéraire** (un module owned) : le plus court chemin entre les arrêts d’un petit réseau lu depuis du JSON, avec des options pour un arrêt manquant ; des structures qui pointent les unes vers les autres, des options, une file de priorité.

### Ce qui est compté, pour chaque programme

1. **Les erreurs rencontrées** : chaque exécution de `perasc check --json` sur la version Peras telle qu’écrite, puis après chaque correctif, jusqu’à ce qu’elle se construise ; chaque diagnostic consigné avec son code, son message et le correctif fait. Un correctif est le plus petit changement qu’un utilisateur ferait à partir du diagnostic.
2. **Ce qui ne peut pas s’écrire** : une partie du TypeScript que la version Peras ne peut pas exprimer du tout, consignée avec ce qui a été fait à la place (`E0900`, une méthode manquante, un type manquant).
3. **Les lignes** : de chaque version, lignes vides et commentaires à part, et les lignes que la version Peras change par rapport à une traduction littérale.
4. **Les résultats** : chaque version Peras donne ce que donne son TypeScript, sur les mêmes entrées, construite pour la production (`--allocator tlsf`, constat M029).
5. **Le temps**, en second : le temps de chaque version sur sa charge face à celui de son TypeScript, dans un seul processus, l’ordre alterné à chaque tour, 21 tours, avec un contrôle A/A, le TypeScript chronométré contre lui-même dans les mêmes tours (constat M042).

### Le classement

Chaque erreur, chaque chose qui ne peut pas s’écrire et chaque ligne changée est rattachée à une cause : un constat existant, une étape de la feuille de route, ou un constat nouveau. Une cause est classée par les programmes qu’elle bloque (une partie qui ne peut pas s’écrire), puis par les erreurs qu’elle cause, puis par les lignes qu’elle ajoute. La liste classée est l’ordre des prochaines décisions ; une cause qu’aucun programme ne rencontre n’est pas classée, et le dit.

### Ce que ceci ne peut pas montrer, dit maintenant

Les programmes sont écrits par les auteurs de Peras, qui savent ce qu’il refuse : ils couvrent les applications que les auteurs connaissent, pas toutes les applications, comme la décision 0011 le disait de son corpus. Pour le borner : chaque version TypeScript est écrite et committée avant sa version Peras, et pas changée ensuite ; chaque correctif est celui que propose le diagnostic, ou le plus petit qu’un utilisateur ferait, consigné comme tel.

## Journal

### Les spécifications (2026-11-25, avant tout Peras)

Les quatre programmes en TypeScript, `bench/next/{notes,ledger,particles,routes}.ts`, vérifiés au plus strict de TypeScript (`bench/next/tsconfig.json` : `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), chacun avec un `run` qui fait sa propre entrée et donne une ligne de texte à comparer : A1 pour 2 000 notes donne `2000|139|1986|books:800,…` ; A2 pour 20 000 lignes, douze mois et un plus bas solde de -382 191,29 ; A3 après 300 images, 1 792 particules et une empreinte des pixels, 694335550 ; A4 sur une grille de 60 par 60 arrêts, `298:119|330:121|missing nowhere`. Écrit avant ce commit, le générateur du livre de comptes a été corrigé deux fois, avant toute ligne de Peras : ses bits de poids faible laissaient cinq mois vides, et un salaire de 2 500 empêchait le solde de jamais passer sous zéro ; il prend les bits de poids fort du générateur, et un salaire de 300. À partir de ce commit, ils ne changent plus.

### Les versions Peras (2026-11-25)

Chaque TypeScript traduit comme enregistré, `bench/next/{notes.apeiron,ledger.peras,particles.peras,routes.peras}`, chaque exécution de `perasc check --json` gardée avec sa source et la correction faite, dans [`bench/next/ease`](../../../bench/next/ease). Chacune se construit pour la production et donne la ligne de son TypeScript, deux fois sur une même instance (`node scripts/bench-next.mjs`).

| Programme                        | Exécutions | Erreurs rencontrées | Ne peut s’écrire                                                                               | Bogues de perasc trouvés |
| -------------------------------- | ---------: | ------------------: | ---------------------------------------------------------------------------------------------- | ------------------------ |
| A1, des notes                    |          4 |                  23 | –                                                                                              | F123                     |
| A2, un registre de comptes       |          6 |                  25 | –                                                                                              | –                        |
| A3, une scène de particules      |          7 |                  38 | un champ de `inout self` qui reçoit un nouveau tableau (F121) ; un framebuffer d’octets bornés | F121, F122               |
| A4, un calculateur d’itinéraires |         10 |                  29 | du JSON lu dans un module possédé (E0900, F120)                                                | F119, F120               |

115 erreurs en tout. Le premier commit d’A3 disait 36 et celui d’A4 21 ; les comptes ci-dessus sont lus dans les exécutions elles-mêmes, et ces deux-là étaient faux.

Deux écarts au protocole, dits ici. **Les déclarations ont été écrites dans les formes de Peras**, `struct`, `impl`, `enum` et `while (true)` pour les interfaces, classes, unions et `for (;;)` de TypeScript, comme le guide l’enseigne d’abord : un développeur qui ne l’a pas lu rencontrerait plus d’erreurs, le compte est donc un plancher. **Quatre résultats différaient** de ceux de TypeScript une fois une version construite, sans aucun diagnostic : trois sont des nombres (A2, un produit au-delà de 2⁵³ que JavaScript arrondit et qu’un `i64` n’arrondit pas ; A3, une division entière dont le quotient rencontrait un entier, et une somme qui déborde un `i32` en production), un est un élément lu dans un `let` et changé comme une copie, où menait la correction d’E0205 (F122, son message corrigé). Ils sont comptés avec leur cause ci-dessous, comme des résultats changés en silence, pesés au-dessus de toute erreur.

### Le classement

Chaque erreur attribuée à une cause, d’après les exécutions ; le score comme enregistré, par les programmes qu’une cause bloque, puis par ses erreurs.

| Rang | Cause                                                                                                              | Programmes bloqués |         Erreurs (A1, A2, A3, A4) | Résultats changés en silence | Où elle en est                                                                                   |
| ---: | ------------------------------------------------------------------------------------------------------------------ | ------------------ | -------------------------------: | ---------------------------: | ------------------------------------------------------------------------------------------------ |
|    1 | Du JSON dans un module possédé                                                                                     | A4                 | 0 par `check`, E0900 par `build` |                            – | constat F120, ouvert                                                                             |
|    1 | Un champ de `inout self` qui reçoit une valeur entière                                                             | A3                 | 0 par `check`, E0900 par `build` |                            – | constat F121, l’erreur posée, la fonction ouverte                                                |
|    1 | Un framebuffer, des octets bornés comme `Uint8ClampedArray`                                                        | A3                 |                   1 (0, 0, 1, 0) |                1 (l’arrondi) | feuille de route, phase 6, `Framebuffer<Rgba8>`                                                  |
|    4 | Un nombre de JavaScript utilisé comme entier et comme flottant                                                     | –                  |                 31 (1, 7, 22, 1) |                            3 | décisions 0030, 0034 ; nouveau                                                                   |
|    5 | Une valeur sortie d’un élément, d’un champ ou d’une liaison                                                        | –                  |                  17 (0, 7, 2, 8) |                            – | voulu (décision 0025) ; sa correction est proposée                                               |
|    6 | De la syntaxe de TypeScript absente : décomposition, `...`, `?:`, `?.`                                             | –                  |                  14 (4, 7, 0, 3) |                            – | nouveau                                                                                          |
|    7 | Le navigateur typé avec les noms de Peras                                                                          | –                  |                 14 (14, 0, 0, 0) |                            – | décision 0009                                                                                    |
|    8 | Un élément lu dans une variable est une copie                                                                      | –                  |                 10 (0, 0, 10, 0) |                            1 | F122 ; les projections de Hylo, dans [la recherche](../research.md#changer-un-élément-sur-place) |
|    9 | Des globales et méthodes de JavaScript absentes : `Number`, `String`, `padStart`, `Infinity`, `Map.values`, `fill` | –                  |                   8 (0, 4, 2, 2) |                            – | nouveau                                                                                          |
|   10 | Les paramètres d’une fléchée et ses résultats vides non typés d’après leur usage                                   | –                  |                   7 (0, 0, 0, 7) |                            – | nouveau                                                                                          |
|   11 | Un `let` non restreint                                                                                             | –                  |                   5 (0, 0, 1, 4) |                            – | décision 0045, dit alors                                                                         |
|   12 | Pas de tri par un comparateur                                                                                      | –                  |                   3 (3, 0, 0, 0) |                            – | décision 0011, `sortBy`                                                                          |
|   13 | Une branche de `match` qui affecte                                                                                 | –                  |                   2 (0, 0, 0, 2) |                            – | nouveau                                                                                          |
|    – | Voulu : `pop()` et `fromJson` donnent une option et un résultat                                                    | –                  |                   2 (0, 0, 0, 2) |                            – | pas une friction                                                                                 |
|    – | Un bogue de perasc, F123                                                                                           | –                  |                   1 (1, 0, 0, 0) |                            – | corrigé                                                                                          |

Non classés, parce qu’aucun programme ne les a rencontrés, comme enregistré : les paquets (chaque programme tient en un fichier), `@frame` (la trame d’A3 est une boucle sur ses particules, écrite sans lui et juste), l’allocation dans un tour d’un `parallel for` (F301), les variables de module chaînes ou tableaux, la lecture d’un champ par les outils (F313), un test en échec qui montre un tableau (F227).

### La mesure (2026-11-25)

`node scripts/bench-next.mjs`, 21 tours dans un seul processus, chaque version Peras construite pour la production ; [`bench/next/RESULTS.md`](../../../bench/next/RESULTS.md).

| Programme                            | Lignes, TypeScript | Lignes, Peras | Peras / TypeScript, médiane (étendue) | A/A, médiane (étendue) |
| ------------------------------------ | -----------------: | ------------: | ------------------------------------- | ---------------------- |
| A1, des notes, 2 000                 |                 93 |           114 | 5,80 (2,95 à 7,08)                    | 1,02 (0,93 à 5,07)     |
| A2, un registre, 20 000              |                 65 |            92 | 1,59 (1,40 à 1,76)                    | 1,04 (0,90 à 1,60)     |
| A3, une scène de particules, 300     |                 87 |           120 | 3,28 (2,53 à 3,67)                    | 1,04 (0,76 à 1,38)     |
| A4, un calculateur d’itinéraires, 60 |                118 |           166 | 0,66 (0,40 à 0,85)                    | 1,14 (0,64 à 2,23)     |

Les médianes du contrôle A/A sont entre 1,02 et 1,14, ses étendues jusqu’à 5,07 : une médiane dans cette bande ne décide rien (constat M042), et aucune des quatre n’y est. La première exécution de cette mesure donnait à A1 186 fois le temps de TypeScript : `copy()` sérialisait la valeur dans un module managé, un bogue de perasc trouvé ici, F124, corrigé avant cette exécution. Ce qui reste du temps d’A1 est la copie elle-même : `add` copie les étiquettes de chaque note là où la décomposition de TypeScript les partage, la sémantique de valeur [du guide](../guide/collections.md#un-élément-est-une-valeur). Le temps d’A3 n’est pas encore expliqué ; il est consigné comme une question de la décision du framebuffer, dont c’est le programme.

Les versions Peras sont de 23 à 42 % plus longues que leur TypeScript, pour les raisons que nomme le classement : des conversions, des boucles écrites là où TypeScript a une méthode ou une décomposition, un bornage et un compactage écrits à la main.

## Proposition (2026-11-25, pour le propriétaire)

Le classement, comme enregistré, donne l’ordre :

1. **Les trois causes qui bloquent un programme**, à égalité : `Framebuffer<Rgba8>` (phase 6, A3), le JSON dans les modules possédés (F120, A4), un champ de `inout self` qui reçoit une valeur entière (F121, A3). Chacune est une décision à part, mesurée ; le framebuffer d’abord, puisqu’il est aussi l’étape suivante de la feuille de route que le propriétaire a demandée, et que le temps d’A3 est sa question.
2. **Un nombre de JavaScript utilisé comme entier et comme flottant** : 31 erreurs sur quatre programmes et trois résultats changés en silence, de loin la plus grande cause ; une décision dont les options sont mesurées sur ces quatre programmes.
3. Puis les causes par leurs erreurs : les déplacements hors des éléments et des champs (17), la syntaxe de TypeScript absente (14), les noms du navigateur (14), un élément lu comme une copie (10).

Les paquets et `@frame`, que le propriétaire a demandé de construire avec le framebuffer en un lot, ne sont pas classés par cette mesure, comme enregistré : aucun programme ne les a rencontrés. Leur ordre reste au propriétaire ; cet enregistrement dit seulement que quatre applications écrites depuis TypeScript n’en ont pas eu besoin.

## La décision du propriétaire (2026-11-25)

L’ordre du classement, avec un changement : les copies qu’aucune lecture ne voit ensuite passent avant les paquets, comme le plus grand coût des applications mesurées (5,8 fois le temps de TypeScript pour A1, un quart de celui d’A3, constat F125). Le lot, dans l’ordre : la [décision 0047](0047-framebuffer.md), le framebuffer ; puis les copies, mesurées sur A1 et A3 ; puis les paquets et `create-peras` ; puis `@frame` ; puis une application qui valide le lot, avec la comparaison face à Rust et AssemblyScript mesurée de nouveau. Le JSON dans les modules possédés (F120) et un champ de `inout self` qui reçoit une valeur entière (F121) suivent le lot.
