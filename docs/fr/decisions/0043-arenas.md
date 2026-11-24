# 0043 : Les arènes, les valeurs temporaires d’un tour rendues d’un coup

_Version anglaise : [../../en/decisions/0043-arenas.md](../../en/decisions/0043-arenas.md)._

_État : protocole enregistré le 2026-11-24, avant tout prototype et toute mesure, comme le demande la règle du projet ; options mesurées au commit `d59e034` ; proposition : a0 reste, au propriétaire de décider. C’est le premier pas de la phase 6 de la [feuille de route](../reference/roadmap.md), « les allocateurs `Arena` et `Pool<T>` », dans l’ordre fixé par le propriétaire le 2026-11-24 : les arènes, puis `Pool<T>` (une décision à elle), puis `@frame`, puis l’étude de la mémoire chargée selon la proximité, qui a besoin des deux. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression ; si aucune ne l’est, la décision le dit et rien n’est installé._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Chaque bloc du tas a un seul propriétaire, et est libéré là où son propriétaire finit.** L’abaissement écrit chaque libération (`Stmt::Release`, `BufStmt::Release`), et le vérificateur des libérations (`crates/perasc-ir/src/own.rs`) prouve que chaque bloc est libéré exactement une fois sur chaque chemin, et que chaque tour de boucle laisse les variables comme il les a trouvées : un tour qui alloue libère, ou déplace vers l’extérieur. Une valeur temporaire d’un tour est donc déjà libérée dans ce tour, une libération par bloc.
- **Deux allocateurs.** L’allocateur de débogage (décision 0001), par défaut en ligne de commande et dans le serveur de développement de Vite, empoisonne, garde une quarantaine et nomme les sites de chaque bogue. TLSF (décision 0002), pour la production, prend un temps borné par opération ; depuis la décision 0016, il garde aussi les blocs libérés de 32 à 256 octets, au plus 64 par taille, pour la prochaine allocation de leur taille : allouer et libérer un tel bloc coûte quelques lectures et écritures. Un bloc plus grand cherche dans deux bitmaps, se découpe, et fusionne avec ses voisins quand il est libéré.
- **Un buffer est un en-tête et ses éléments.** Un tableau ou une chaîne est un en-tête de 16 octets qui garde son adresse ; ses éléments le suivent dans le même bloc quand ils sont peu nombreux et ne contiennent pas de buffers, et forment sinon un bloc à eux, réalloué, au moins doublé, quand il grandit (`crates/perasc-runtime/runtime/buffers.wat`). Un tableau de chaînes possède ses chaînes et les libère avec lui (`DEEP`). Certains blocs du runtime lui-même, ceux de `split` par exemple, sont alloués sans site (site 0).
- **Les sites d’allocation sont numérotés par le backend** (`crates/perasc-codegen-wasm`), pour les messages de l’allocateur de débogage ; l’IR ne les connaît pas.
- **Ce que prévoit la spécification** ([spécification](../spec.md#allocateurs)) : « `Arena` : allocation par avancement d’un pointeur, remise à zéro en une seule opération. Idéal pour les données temporaires d’une image », à côté de `Pool<T>(n)` et de l’allocateur général. Rien n’en est construit.
- **Ce qu’a mesuré la décision 0032** : un arbre de nœuds partagés est 6,8 fois plus lent qu’« une arène », trois nombres dans trois tableaux avec des indices. Cette arène-là est une disposition des données, dont traitera `Pool<T>` ; cette décision-ci traite du coût d’allouer et de libérer des valeurs temporaires.

La question n’est donc pas de savoir si les valeurs temporaires sont libérées : elles le sont, de façon déterministe, par construction. C’est de savoir si les rendre **d’un coup**, en reculant un pointeur, coûte nettement moins que de les rendre une à une par TLSF et ses blocs gardés, et à quel prix en mémoire, en octets et en notions à apprendre.

Ce que font les autres : `bumpalo` en Rust et `std::pmr::monotonic_buffer_resource` en C++ donnent une arène comme valeur, ses allocations liées à sa durée de vie par le système de types (Rust) ou par le programmeur (C++). Zig passe un allocateur à tout ce qui alloue, une arène parmi d’autres. Les moteurs de jeu remettent à zéro un allocateur d’image une fois par image. L’inférence de régions (Tofte et Talpin, 1997, dans le ML Kit) laisse le compilateur choisir la région de chaque allocation, sans syntaxe ; le ML Kit a ensuite ajouté un ramasse-miettes, car les régions inférées gardaient trop longtemps la mémoire de certains programmes.

## Options

- **a0. Comme aujourd’hui** : TLSF et ses blocs gardés, chaque valeur temporaire libérée là où son propriétaire finit. La référence.
- **a1. Un bloc `arena { … }`, écrit par le programmeur.** Chaque bloc alloué dedans, par le programme ou par le runtime pour lui, est pris en avançant un pointeur, et le tout est rendu à la fin du bloc, en une opération. Une valeur faite dedans qui survivrait au bloc, déplacée vers une variable déclarée dehors, renvoyée, poussée dans un tableau venu de dehors, est une erreur à la compilation, qui montre où elle sort. Une boucle dont les tours allouent s’écrit `for (…) { arena { … } }`.
- **a2. Des arènes choisies par le compilateur, sans syntaxe.** Le compilateur prend dans une arène chaque allocation dont il prouve la mort avant la fin du tour de sa boucle la plus intérieure ou de l’appel de sa fonction, et rend l’arène à la fin de ce tour ou au retour de cet appel ; une valeur qu’il ne peut pas prouver ainsi est allouée par TLSF, comme aujourd’hui. Rien à écrire, rien à apprendre ; rien de refusé.
- **a3. L’allocateur seul, sans arène** : TLSF garde les blocs libérés pour plus de tailles qu’aujourd’hui (les listes de la décision 0016, élargies), pour qu’une valeur temporaire de toute taille courante soit allouée et libérée en quelques lectures et écritures. Aucun changement du langage, aucun changement des modules.
- **Une arène comme valeur**, `const a = Arena.new()` passée à ce qui alloue, comme `bumpalo` ou Zig : non construite. Peras n’a pas de durées de vie et ne stocke pas de références (spécification), donc rien ne peut lier une valeur à l’arène d’où elle vient sans les ajouter, un changement du langage bien au-delà de cette décision ; elle est nommée pour qu’on ne la croie pas oubliée (constat M001).

Avec a1 et a2, les arènes n’existent que dans les modules construits avec TLSF. Une construction de développement garde l’allocateur de débogage, inchangé, avec tous les contrôles qu’il fait aujourd’hui : une arène cacherait l’usage après libération qu’il attrape.

## Protocole, enregistré avant tout prototype

a1, a2 et a3 sont prototypées derrière `PERAS_PROTO_ARENA=a1|a2|a3` ; sans elle, le compilateur est comme aujourd’hui, a0. Chaque module mesuré est construit pour la production, `--allocator tlsf` (constat M029), par une construction release de `perasc`.

### Avant toute mesure : les définitions testées

La règle de a1 (ce qui sort d’un bloc) et celle de a2 (ce qui est prouvé mort dans le tour ou l’appel) sont d’abord écrites comme une analyse qui ne fait que rapporter, lancée sur les charges ci-dessous et sur chaque banc et chaque exemple, quelles allocations chacune prendrait dans une arène (constat M003). Deux règles en désaccord avec ce protocole, ou une charge où une option ne prendrait rien, sont consignées, et le protocole amendé avant toute mesure, l’amendement daté.

### Les charges

Des modules possédés sur TLSF dans Node, le module de chaque option et celui de a0 dans un seul processus, l’ordre alterné tour à tour, 21 tours, la médiane des rapports à a0 et leur étendue (constat M017) :

- **W1, le tableau de bord de la décision 0014** (`bench/data/dashboard.peras`, tel qu’écrit), sur les données que génère `scripts/bench-data.mjs` : un `split` par ligne, ses champs lus puis abandonnés.
- **W2, les valeurs temporaires d’une image** (`bench/arena/frame.peras`, écrit pour cette décision comme un utilisateur l’écrirait depuis JavaScript) : 10 000 particules sur une grille ; à chaque image, pour chaque particule, le tableau des indices de ses voisines est construit, lu, puis abandonné ; 100 images.
- **W3, une ligne de texte par rangée** (`bench/arena/rows.peras`) : 1 000 000 de rangées, chacune écrite comme une chaîne à gabarit de cinq champs, ses octets sommés, puis abandonnée.
- **W4, des valeurs gardées** (`bench/arena/keep.peras`) : 1 000 000 de chaînes faites et gardées dans un tableau, puis sommées : aucune arène ne peut les prendre ; elle mesure ce qu’une option coûte là où elle ne peut pas aider.

### Les quatre grandeurs

1. **Vitesse** : le temps du travail de chaque charge, son module chargé une fois, comme ci-dessus.
2. **Charge du processeur** : le temps pendant lequel un cœur est occupé (`process.cpuUsage`), comme le mesure la décision 0012, sur les mêmes tours.
3. **Mémoire et octets** : la mémoire du module à la fin de chaque charge (`memory.buffer.byteLength`, le pic, puisque la mémoire ne rétrécit jamais), et la taille de chaque module, brute et gzip.
4. **Facilité d’écriture** : W2 et W3 écrites comme depuis JavaScript, puis corrigées jusqu’à ce qu’elles se construisent, pour chaque option : les erreurs rencontrées, avec leurs codes, les lignes changées, les notions que JavaScript ne demande pas.

Et, pour chaque option, `node scripts/check-regression.mjs` contre le commit de ce protocole : chaque module des bancs et des exemples identique ou plus petit, aucun plus grand ; le rapport médian du temps du compilateur au plus 1,05 ; sa taille rapportée.

### La justesse, avant que tout chiffre compte

Chaque option passe `cargo test --workspace` avec sa variable posée, les tests de bout en bout des deux allocateurs, et, pour a1 et a2, le test différentiel avec `PERAS_DIFFERENTIAL_SEEDS=2000`, étendu de tours et d’appels dont les valeurs meurent ou sortent ; puis une fois avec l’option cassée exprès (une allocation qui sort de son tour prise dans l’arène), pour voir un test échouer (constat M015).

### Les critères, enregistrés maintenant

- Une option qui échoue à `check-regression`, ou à un contrôle de justesse ci-dessus, est écartée.
- Une option n’est une amélioration que si son rapport médian est au plus 0,95 sur au moins une des charges W1 à W3, au plus 1,02 sur chaque charge, W4 comprise, son étendue prise en compte, et sa mémoire au plus 1,10 fois celle de a0 sur chaque charge.
- Parmi les améliorations, la plus rapide d’abord, par la médiane de ses rapports sur W1 à W3 ; à 0,03 l’une de l’autre, celle qui a le moins de notions et le moins d’erreurs en (4).
- Si aucune option n’est une amélioration, a0 reste, les prototypes sont retirés, leurs chiffres gardés ici, et `Arena` quitte la feuille de route jusqu’à ce qu’une charge en montre un coût à retirer.

## Journal

### Les définitions testées, et deux amendements (2026-11-24, avant toute mesure)

La règle de a2 est écrite comme une analyse qui ne fait que rapporter (`crates/perasc-ir/src/arena.rs`) : elle groupe les variables `owned` dont les blocs sont ceux d’une même valeur (une variable déplacée dans une autre, un buffer poussé dans un tableau de buffers, une clé déplacée dans une table), garde un groupe dont les blocs sont alloués dans la fonction par le runtime et dont aucun ne sort, et lui donne le tour de boucle le plus intérieur, ou l’appel, qui contient toutes ses mentions. Une instruction qui alloue ou fait grandir un tel groupe s’exécute dans l’arène, sauf si elle appelle une fonction, fait grandir un bloc non pris, ou se trouve dans une boucle, sous le foyer du groupe, qui rend sa place à chaque tour : TLSF la sert alors, comme aujourd’hui, et puisque chaque libération est gardée, un bloc peut venir de l’un ou de l’autre. La règle de a1 est la même, sa région le bloc écrit.

Lancée sur les 201 fichiers Peras des bancs, des exemples et de `bench/arena`, elle prendrait 603 de leurs 886 allocations. Sur les charges :

| Charge                   | Allocations prises                                                                  | Où                                                                                               |
| ------------------------ | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| W1, le tableau de bord   | `isCountry` 1 sur 1, `parse` 1 sur 3 (les champs du `split`), `summarize` 19 sur 34 | l’appel, et un tour de sa boucle sur les lignes                                                  |
| W2, l’image              | 5 sur 5                                                                             | l’appel (les positions), le tour d’une image (la grille), le tour d’une particule (ses voisines) |
| W3, les rangées          | 2 sur 2                                                                             | le tour d’une rangée                                                                             |
| W4, telle qu’enregistrée | 2 sur 2                                                                             | l’appel                                                                                          |
| W4, amendée (ci-dessous) | 0 sur 2                                                                             |                                                                                                  |

Deux amendements suivent, faits avant toute mesure :

1. **W4 ne mesurait rien de ce qu’elle devait.** Ses chaînes sont gardées dans un tableau jusqu’au retour de la fonction, puis abandonnées : elles meurent dans l’appel, et a2 les prend. W4 est désormais deux fonctions, `make(n)` qui renvoie le tableau de chaînes et `run(n)` qui somme ce que `make` donne : une valeur renvoyée sort de son appel, et rien n’est pris. La W4 enregistrée est gardée comme W4b, une valeur gardée tout un appel, que a2 prend.
2. **W3 ne peut pas sommer les octets d’une rangée** : Peras n’a pas encore `charCodeAt` (constat F309, `E0215` sur `byteAt`). Elle somme à la place la longueur de chaque rangée et la place de sa première virgule, ce qui lit la rangée sans allouer.

### Les prototypes (2026-11-24)

- **a2** (`crates/perasc-ir/src/arena.rs`, `PERAS_PROTO_ARENA=a2`) : l’analyse ci-dessus, puis une réécriture de l’IR : une marque là où commence chaque foyer, la place de l’arène rendue à la fin de chaque tour d’un foyer qui est une boucle, après la boucle, et à chaque retour ; `arena_flag(1)` et `arena_flag(0)` autour de chaque instruction que l’arène prend. TLSF avec une arène (`alloc_tlsf.wat`, entre `;; @proto a2` et `;; @endproto`) : un bloc de 256 Kio pris à TLSF la première fois, l’allocation en avançant un pointeur tant que le drapeau est levé, une libération dedans qui ne fait rien. Il n’est lié qu’à un module qui se sert d’une arène.
- **a1** (`PERAS_PROTO_ARENA=a1`) : `arena` devant un bloc, lu seulement sous la variable ; l’abaissement place le bloc entre deux marques ; la même passe, les blocs écrits comme seuls foyers ; une valeur faite dans l’un qui lui survit est refusée au bloc. **Écart** : le prototype la refuse avec `E0900`, dont le message ne nomme pas la valeur ; un code à lui viendrait avec son installation.
- **a3** (`PERAS_PROTO_ARENA=a3`) : les blocs de 257 octets à 4 Kio arrondis à 256 et gardés, comme la décision 0016 garde les petits, dans 15 listes de plus.

Chacune passe `cargo test --workspace` avec sa variable posée, et le test différentiel avec `PERAS_DIFFERENTIAL_SEEDS=2000`. Le test différentiel a d’abord manqué l’arène : il liait chaque module à TLSF tel qu’il est, donc les appels de l’arène ne faisaient rien. Il lie désormais le runtime du prototype, et il a gagné une grille qui grandit dans une boucle dont les tours ont leurs propres valeurs, écrite dans deux arènes imbriquées sous a1. Cassées exprès (une instruction sous une boucle qui rend sa place prise quand même), a2 et a1 le font chacune échouer (constat M015).

### La mesure (2026-11-24)

Au commit `d59e034`, `node scripts/bench-arena.mjs`, 21 tours, chaque option dans un seul processus, l’ordre alterné ([résultats](../../../bench/arena/RESULTS.md)) ; puis `node scripts/check-regression.mjs 1ff4de1 PERAS_PROTO_ARENA=…`, le commit du protocole, pour chaque option. Machine : Intel Xeon à 2,10 GHz, 4 cœurs ; Node 24.22.1.

Le temps de chaque charge, la médiane de ses rapports à a0 tour à tour :

|     | W1    | W2, les lucioles | W3        | W4    | W4b   |
| --- | ----- | ---------------- | --------- | ----- | ----- |
| a1  | 1,004 | **0,937**        | **0,940** | 1,008 | 1,011 |
| a2  | 0,971 | 1,163            | **0,935** | 1,001 | 0,983 |
| a3  | 1,046 | 1,017            | 1,035     | 1,019 | 1,027 |

La charge du processeur suit le temps : le temps processeur sur le temps écoulé vaut de 0,99 à 1,01 partout, un seul fil.

La mémoire à la fin de chaque charge, face à celle de a0 : a1 et a2 ajoutent les 256 Kio de l’arène là où elle sert : W3 passe de 64 Kio à 320 Kio, cinq fois ; W2 de 832 Kio à 768 Kio sous a1 (moins), 960 Kio sous a2 ; W1 à moins de 1 %. a3 ajoute jusqu’à 8 % (W2).

Les octets et le compilateur, par `check-regression` :

|     | modules plus grands                                  | temps de compilation, rapport médian | module de W3                              |
| --- | ---------------------------------------------------- | ------------------------------------ | ----------------------------------------- |
| a1  | 0 sur 250 (aucun programme du dépôt n’écrit d’arène) | 1,040                                | de 4 434 à 4 774 octets quand il en écrit |
| a2  | 61 sur 250                                           | 0,998                                | 4 778 octets                              |
| a3  | 91 sur 250                                           | 1,027                                | 4 495 octets                              |

La facilité d’écriture, W2 et W3 écrites comme depuis JavaScript : le langage d’aujourd’hui rencontre 10 `E0210` sur W2 (un `i32` dans un flottant, trois tours de corrections) et une `E0215` sur W3 (`byteAt`, F309), les mêmes sous chaque option ; a1 demande une notion de plus, un bloc et ce qui peut en sortir, et deux lignes par programme, et n’a rencontré aucune erreur à elle.

Face aux critères enregistrés :

- **a2 est écartée** : 1,163 sur W2, au-dessus de 1,02 ; et 61 modules plus grands.
- **a3 est écartée** : 1,046 sur W1 ; aucune charge à 0,95 ; et 91 modules plus grands.
- **a1 est écartée** : c’est la seule option qui gagne, 0,937 sur W2 et 0,940 sur W3, et elle n’agrandit aucun module du dépôt, mais sa mémoire sur W3 vaut cinq fois celle de a0, au-dessus de 1,10. Les 256 Kio de son arène sont un coût fixe qu’un petit programme paie en entier.

### Proposition

**a0 reste** : aucune option n’est une amélioration selon les critères enregistrés avant la mesure. Comme le dit le protocole, les prototypes sont retirés, leurs chiffres gardés ici et leur code dans git au commit `d59e034`, et `Arena` quitte la feuille de route jusqu’à ce qu’une charge en montre un coût à retirer.

Ce que la mesure apprend, pour une décision ultérieure plutôt que celle-ci : le gain d’une arène écrite est réel, environ 6 % sur les valeurs temporaires, et ce qui l’a fait échouer est la taille de son bloc, pas sa règle ; une arène qui commence petite et grandit, ou une arène dimensionnée d’après les allocations du programme lui-même, serait une autre option, mesurée face aux mêmes critères. Les drapeaux de a2 autour de chaque instruction lui coûtent plus que ce que l’arène économise là où les instructions sont nombreuses et petites (W2). Le propriétaire décide.
