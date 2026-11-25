# 0048 : des choix sans branche, et les petites fonctions qui rendent une structure

_Original anglais : [../../en/decisions/0048-choices-and-small-structures.md](../../en/decisions/0048-choices-and-small-structures.md)._

_Statut : protocole enregistré le 2026-11-25, avant tout prototype, comme le demande la règle du projet, après que les planchers de ses options ont été mesurés (constat M041). Demandée par le propriétaire le 2026-11-25 : travailler le langage pour qu’il rattrape TypeScript là où il est en retard, et P2 est le programme que le propriétaire a mesuré sur un téléphone, Peras à 1,35 fois le temps de TypeScript._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Aucun choix n’est compilé sans branche.** `min`, `max` et `clamp` sont abaissés en comparaisons et branches (`crates/perasc-ir/src/lower/functional.rs`) ; un `if` qui donne à une variable l’une de deux valeurs est une branche ; l’IR n’a pas de `select`, et la seule réécriture sans branche est `branchless_abs` (`crates/perasc-ir/src/opt.rs`). WebAssembly a `select`, qui ne prend aucune branche.
- **Aucune fonction n’est insérée**, sauf une fonction récursive en elle-même (`crates/perasc-ir/src/inline.rs`). La décision 0036 a mesuré l’insertion des petites fonctions, i1 : elle a fait grossir 19 modules sur 173 de 1 à 3 % et ralenti le jardin de 5 %, et n’a pas été retenue. Elle laissait de côté une fonction qui rend une structure (`uses_many`), dont les champs reviennent comme plusieurs résultats.
- **Où va le temps de P2**, l’image pleine de la décision 0047 avec f3, profilé en retirant une chose à la fois en gardant le même résultat (constat M013), 15 tours dans un seul processus, face à TypeScript, contrôle A/A 1,01 à 1,02 : tel qu’adopté, 1,77 ; son `clamp` retiré, le plancher d’un choix sans branche à cet endroit, 1,59 à 1,69 selon la série ; le pixel écrit dans la boucle, sans `rgba`, `channel`, `word` ni `Rgba8`, le plancher de l’insertion de ces petites fonctions, 1,46. Avec un arrondi écrit avec une branche que le processeur prédit mal (le f1 de la décision 0047), le même arrondi sans branche passait de 3,18 à 1,87 (constat F128).

## Les options

- **c0, rien.** La référence.
- **s1, des choix sans branche** : `a.min(b)`, `a.max(b)` et `a.clamp(lo, hi)` abaissés en `select` de WebAssembly sur les mêmes comparaisons, pour que NaN et le signe d’un zéro donnent ce qu’ils donnent aujourd’hui ; et un `if` dont les branches ne font que donner à une variable locale l’une de deux valeurs, chacune peu coûteuse et incapable d’échouer (`is_simple`), réécrit en cette variable recevant un `select`.
- **s2, les petites fonctions qui rendent une structure, insérées** : une fonction dont le corps est un seul `return` d’une structure de nombres faite de ses paramètres, ou de nombres calculés à partir d’eux, d’au plus 40 nœuds, sans boucle ni tas, remplacée à chaque appel par ses champs, chaque argument évalué une fois et dans l’ordre ; la forme la plus étroite de l’i1 de la décision 0036, à l’endroit que son profil désigne.
- **s3, les deux**, s1 puis s2, mesurées à part et ensemble (constat M043).

## Les programmes

- **P2**, l’image pleine de la décision 0047 avec f3, `bench/frame/plasma-f3.peras`, et **A3** avec f3, `bench/frame/particles-f3.peras`, face à TypeScript.
- **Le jardin** (décision 0013), le programme que l’insertion de la décision 0036 ralentissait.
- **Chaque module du dépôt**, par `scripts/check-regression.mjs`.

## Ce qui est mesuré, enregistré maintenant

1. **L’exactitude** : le test différentiel (`crates/perasc-driver/tests/differential.rs`), son générateur étendu à `min`, `max`, `clamp`, à un `if` qui donne à une variable l’une de deux valeurs, et aux petites fonctions qui rendent une structure, passé avec `PERAS_DIFFERENTIAL_SEEDS=2000`, et échoué avec chaque option cassée exprès (constat M015) ; chaque résultat de P2, d’A3 et du jardin le même que celui de la référence.
2. **La vitesse** : chaque option face à c0 dans un seul processus, l’ordre alterné à chaque tour, 21 tours, avec un contrôle A/A, c0 face à lui-même (constat M042) ; P2, A3 et le jardin.
3. **La charge CPU** : le temps qu’un cœur est tenu occupé sur les images de P2, comme la décision 0012 le mesure.
4. **La mémoire et les octets** : la taille de chaque module, brute et compressée, sous chaque option, par `check-regression` ; la mémoire à son pic pour P2, A3 et le jardin.
5. **Le compilateur** : son temps (rapport médian au plus 1,05) et sa taille.
6. **La facilité** : aucune de ces options ne change ce qu’écrit un utilisateur ; dit, pas mesuré.

## Les critères, enregistrés maintenant

Une option est retenue quand : chaque résultat est le même ; elle passe le test différentiel et y échoue quand elle est cassée ; P2 ou A3 prend au plus 0,95 du temps de c0, la médiane du contrôle A/A entre 0,95 et 1,05 ; le jardin prend au plus 1,05 du temps de c0 ; aucun module ne grossit, sauf ceux dont elle mesure le temps plus court, chacun nommé avec ses chiffres (constat M038) ; le compilateur n’est pas plus lent que 1,05. Parmi les options retenues, celle qui en fait le moins ; s3 plutôt que s1 ou s2 seulement si elle prend au plus 0,95 du temps de la meilleure sur P2.

## Ce que ceci ne peut pas montrer, dit maintenant

Les programmes sont ceux du projet : un choix sans branche est plus lent qu’une branche que le processeur prédit bien, et un programme dont les choix sont prévisibles peut perdre avec s1 ; le jardin et les modules du dépôt sont là pour le trouver, pas pour prouver que cela ne peut arriver. Les moteurs insèrent et choisissent aussi, et autrement sur un autre processeur : P2 mesurait 1,35 sur un téléphone là où il mesurait 1,80 ici.

## Enregistrement

_À venir : les prototypes, les mesures, la proposition._
