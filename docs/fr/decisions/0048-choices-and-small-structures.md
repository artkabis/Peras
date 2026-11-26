# 0048 : des choix sans branche, et les petites fonctions qui rendent une structure

_Original anglais : [../../en/decisions/0048-choices-and-small-structures.md](../../en/decisions/0048-choices-and-small-structures.md)._

_Statut : protocole enregistré le 2026-11-25, avant tout prototype, comme le demande la règle du projet, après que les planchers de ses options ont été mesurés (constat M041) ; s1 construite et mesurée à `b56cab7`, s2 et s3 mesurées par leur plancher ; proposition : c0 reste, aucune option ne remplissant les critères enregistrés ; au propriétaire de décider. Demandée par le propriétaire le 2026-11-25 : travailler le langage pour qu’il rattrape TypeScript là où il est en retard, et P2 est le programme que le propriétaire a mesuré sur un téléphone, Peras à 1,35 fois le temps de TypeScript._

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

### Le prototype s1 (2026-11-25)

À `ec62b13` : l’IR reçoit une sélection, ses trois opérandes tous évalués (`ExprKind::Select`), émise en `select` de WebAssembly, en choix en Rust, et en `v128.bitselect` sous un masque dans une boucle que le SIMD vectorise. Une passe, `branchless_choices` dans `crates/perasc-ir/src/opt.rs`, active seulement quand `PERAS_PROTO_CHOICE` nomme s1 ou s3, réécrit un `if` qui donne à une variable locale scalaire l’une de deux valeurs simples (ni appel, ni division, ni lecture de la mémoire, `is_simple`) en cette variable recevant une sélection, après `branchless_abs` ; `min`, `max` et `clamp` sont abaissés en un tel `if`, et la prennent donc aussi. Toute la suite de tests passe avec s1, sauf `other_sign_tests_keep_their_branch`, qui vérifie la forme même que s1 change. À `99788a9`, le générateur du test différentiel reçoit `min`, `max` et `clamp` sur des entiers et des flottants, NaN et le signe d’un zéro parmi eux, et un `if` qui donne à une variable locale l’une de deux valeurs, avec une branche ou deux : il passe 2000 graines avec s1, et échoue quand la sélection de s1 a ses deux valeurs échangées (constat M015).

### s2 et s3, mesurées par leur plancher (constat M041)

Le plancher de s2 est P2 avec `rgba`, sa seule petite fonction qui rend une structure sur le chemin du pixel, écrite à la main à sa place, au même résultat : 1,009 fois le temps de c0, 15 tours, face à un contrôle A/A de 1,01 à 1,02. Le temps que le profil trouvait dans `rgba`, `channel`, `word` et `Rgba8` ensemble, de 1,77 à 1,46 face à TypeScript, est réparti sur trois petites fonctions qui rendent un nombre et une qui rend une structure : retirer celle que s2 atteint n’en retire rien. s2 ne peut atteindre 0,95, elle n’est donc pas construite ; s3 est s1 et s2, dont aucune ne gagne, et n’est pas construite non plus.

### La mesure (2026-11-25)

À `b56cab7`, un script de cette session, non gardé dans le dépôt, puisque s1 quitte le compilateur avec lui ; ses chiffres sont ceux-ci : P2 (120 images), A3 (300 images) et le jardin (512 × 512 aux zooms 1, 4, 16 et 64), chacun construit pour la production (`--allocator tlsf`) sous c0, c0 de nouveau et s1, et TypeScript ; 21 tours dans un seul processus, l’ordre alterné à chaque tour ; le rapport à c0 pris tour par tour. Le temps CPU par `process.cpuUsage` sur les exécutions de chaque version. Chaque version donne le résultat de c0, deux fois sur une même instance. Intel Xeon à 2,10 GHz, 4 cœurs, Node 24.22.1.

| Programme | Version       | temps / c0, médiane (étendue) | temps / TypeScript | ms CPU par exécution | CPU / mur | mémoire à la fin | octets, bruts et gzip |
| --------- | ------------- | ----------------------------- | -----------------: | -------------------: | --------: | ---------------: | --------------------- |
| P2        | c0            | 1,000                         |              1,755 |                459,9 |     0,988 |         2112 Kio | 7022, 3340            |
| P2        | c0 de nouveau | 1,021 (0,838 à 1,292)         |              1,804 |                471,8 |     0,990 |         2112 Kio | 7022, 3340            |
| P2        | s1            | **1,108** (0,876 à 1,324)     |              1,940 |                499,5 |     0,993 |         2112 Kio | 7022, 3339            |
| A3        | c0            | 1,000                         |              2,028 |                 91,6 |     0,994 |          640 Kio | 12111, 5125           |
| A3        | c0 de nouveau | 0,987 (0,878 à 1,220)         |              2,002 |                 91,3 |     0,996 |          640 Kio | 12111, 5125           |
| A3        | s1            | **1,003** (0,853 à 1,550)     |              2,062 |                 96,6 |     0,988 |          640 Kio | 12111, 5124           |
| jardin    | c0            | 1,000                         |              0,925 |               2175,9 |     0,994 |         1088 Kio | 6166, 3078            |
| jardin    | c0 de nouveau | 0,981 (0,911 à 1,032)         |              0,906 |               2128,5 |     0,995 |         1088 Kio | 6166, 3078            |
| jardin    | s1            | **0,981** (0,869 à 1,076)     |              0,925 |               2135,9 |     0,994 |         1088 Kio | 6166, 3082            |

`node scripts/check-regression.mjs HEAD PERAS_PROTO_CHOICE=s1`, face à `b56cab7` : sur 307 modules, 7 plus gros de 14 à 18 octets (l’étang, tel qu’écrit et tel que gardé, sous les deux allocateurs, ses bandes, et W1 de la décision 0032 sous les deux), 19 plus petits d’environ 5 octets, 50 changés à taille égale ; le compilateur à 1,000 fois son temps (0,969 à 1,109), son binaire de même taille, la passe y étant qu’elle soit active ou non.

À côté du protocole, comme le demande F128 : le f1 de la décision 0047, dont l’arrondi est la branche que le processeur prédit mal, prend 0,991 fois le temps de c0 sous s1 (0,923 à 1,241, 15 tours, A/A 0,975) : sa condition, `d > 0.5 || (d == 0.5 && f % 2 == 1)`, contient `||`, `&&` et un reste flottant, un appel du runtime, que s1 laisse tels quels. L’arrondi que F128 a trouvé a sa propre réponse, `roundEven()`, le f3 de la décision 0047.

### Proposition

Selon les critères enregistrés, **aucune option n’est retenue, et c0 reste** :

- **s1** donne chaque résultat le même et passe le test différentiel, y échouant quand elle est cassée ; mais elle prend 1,108 fois le temps de c0 sur P2 et 1,003 sur A3, là où 0,95 était demandé, les contrôles A/A à 1,021 et 0,987, entre 0,95 et 1,05 ; le jardin 0,981, comme son contrôle A/A. Sept modules grossissent jusqu’à 18 octets, dont elle ne mesure aucun plus rapide. Une sélection paie ses deux côtés à chaque fois ; les branches de ces programmes sont bien prédites, ce que ce protocole disait pouvoir arriver.
- **s2** ne peut atteindre 0,95 par son plancher, 1,009 ; **s3** combine deux options qui ne gagnent rien.

Ce qui reste de l’écart de P2 à TypeScript n’est ni dans ses choix ni dans une petite fonction qui rend une structure : retirer le `clamp` donnait 1,59 à 1,69, et écrire tout le pixel dans la boucle 1,46, le travail des trois fonctions qui rendent un nombre ensemble, que seule une insertion générale atteint, et la décision 0036 l’a mesurée ralentissant le jardin de 5 %. Une fois que le propriétaire a décidé, le code de s1 et `PERAS_PROTO_CHOICE` sont retirés du compilateur, le code gardé dans git à `ec62b13`, l’extension du test différentiel gardée. Le constat F128 reste ouvert avec ce que ceci a mesuré : une conversion des conditions jointes par `&&` et `||` serait une autre décision, sans programme du projet, pour l’instant, qu’elle rendrait plus rapide.
