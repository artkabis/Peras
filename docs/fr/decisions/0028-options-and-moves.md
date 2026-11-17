# 0028 : Prendre une valeur ou partir, et quand un déplacement a lieu

_Version anglaise, qui fait foi : [../../en/decisions/0028-options-and-moves.md](../../en/decisions/0028-options-and-moves.md)._

_État : acceptée par le propriétaire le 2026-11-17, options 1b, 2e et 3b, après que ses options ont été mesurées selon un protocole enregistré avant qu’aucun prototype ne soit construit, comme la règle du projet le demande ([la décision 0027](0027-closures-over-the-heap.md) a été la première). Elle répond aux constats F101, F103, F104 et F105, et au bogue F012 trouvé en lisant le code pour elle._

## Contexte

Ce que fait le code aujourd’hui, lu et exécuté avant d’écrire ceci (constat M020).

- **F103, un déplacement a lieu là où son argument est écrit.** `m.set(key, m.get(key).unwrapOr(0) + 1)` est refusé (`E0204`) : `key` se déplace dans `set` par le premier argument, et le second le lit ensuite. La lecture du second argument se termine avant que `set` soit appelée ; JavaScript accepte la ligne, et c’est la ligne la plus courante d’un comptage.
- **F101, une branche qui part ne donne pas de type au `match`.** Dans `const row = match (parse(line)) { Some(r) => r, None => { continue; } };`, la branche `None` est un bloc qui ne finit jamais normalement, mais elle est typée `void`, si bien que le `match` est `void` et son usage refusé avec `` `void` has no field or method `qty` `` (`E0215`), qui ne dit pas pourquoi.
- **F105, aucun moyen de prendre une valeur ou de partir.** Il n’y a ni `unwrap` ni `let … else` : chaque valeur optionnelle demande un test, `isNone()`, puis `unwrapOr` avec une valeur de remplacement jamais utilisée.
- **F104, le repli de `unwrapOr` est évalué quand il n’est pas nécessaire**, dans un module owned : un repli qui fabrique une chaîne alloue à chaque appel. La décision 0016 a rendu paresseux un repli tranquille (F209), pas un repli qui a un effet.
- **F012, un bogue : les deux modes ne s’accordent pas sur `unwrapOr`.** Un module managed le compile en `x !== undefined ? x : repli`, qui n’évalue le repli que si nécessaire ; un module owned l’évalue toujours. Avec un repli qui change une variable de module, le même source donne `501, 202` dans un module owned et `500, 101` dans un module managed.

## Options

Trois questions, chacune tranchée par une de ses options. Chaque option est construite en prototype quand c’est possible, et mesurée selon le protocole ci-dessous avant qu’une soit choisie.

**Q1, quand un déplacement a lieu (F103).**

- **1a.** Comme aujourd’hui : refusé, avec la correction `key.copy()`.
- **1b. Un déplacement a lieu à l’appel**, une fois chaque argument évalué : un argument suivant peut lire la valeur, tant que ce qu’il lit n’est pas prêté au même appel. `m.set(key, m.get(key).unwrapOr(0) + 1)` se construit ; `f(key, key)` avec le second prêté reste refusé.
- **1c. Une copie faite pour le programme** : le compilateur insère `key.copy()` là où un argument suivant lit ce qui se déplace.

**Q2, prendre une valeur ou partir (F101, F105).**

- **2a.** Comme aujourd’hui : `isNone()`, une valeur de remplacement, `unwrapOr`.
- **2b. Une branche qui ne finit jamais normalement n’a pas de type** à elle (`never`) : `match (parse(line)) { Some(r) => r, None => { continue; } }` a le type de `r`. Quand chaque branche part, le message le dit.
- **2c. 2b, et `const … else`** : `const row = parse(line) else { continue; };` prend la valeur de `Some` ou `Ok`, ou exécute le bloc, qui doit partir.
- **2d. `unwrap()`**, qui arrête le programme avec une erreur quand il n’y a pas de valeur.

**Q3, quand un repli est évalué (F104, F012).**

- **3a. Toujours, dans les deux modes** : le module managed l’évalue d’abord lui aussi.
- **3b. Seulement si nécessaire, dans les deux modes**, comme le font `??` et `? :` en JavaScript : le module owned évalue le repli dans la branche où il n’y a pas de valeur.
- **3c. Toujours, et `unwrapOrElse(() => …)`** pour un repli évalué seulement si nécessaire.

## Protocole, enregistré avant tout prototype

Quatre grandeurs, comme pour la décision 0027 : la vitesse, la charge du processeur, la mémoire et les octets, la facilité d’écriture.

1. **Facilité d’écriture** : six programmes, écrits comme un utilisateur les écrirait depuis JavaScript, puis corrigés jusqu’à ce qu’ils se construisent, pour chaque option de la question qu’ils touchent :
   - P1, des mots comptés dans une table, `m.set(w, m.get(w).unwrapOr(0) + 1)` ;
   - P2, des lignes lues en rangées, une mauvaise ligne sautée ;
   - P3, une fonction qui rend tôt quand une recherche ne trouve rien ;
   - P4, une recherche avec une chaîne comme repli ;
   - P5, une chaîne de trois étapes optionnelles, chacune dépendant de la précédente ;
   - P6, des rangées regroupées par une clé dans une table de structures.

   Pour chacun : les erreurs rencontrées avant qu’il se construise, avec leurs codes, les lignes changées par rapport au JavaScript, les notions que JavaScript ne demande pas. Chaque programme corrigé donne ce que donne le même source construit comme module managed.

2. **Charges**, modules owned sur TLSF dans Node, le module de chaque option dans un même processus, l’ordre alterné, 21 tours, la médiane des rapports à l’option qui garde le comportement d’aujourd’hui (1a, 2a, 3a), et leur étendue (constat M017) :
   - **W1**, P1 sur 1 000 000 de mots tirés parmi 1 000 : 1b contre 1c, qui copie la clé à chaque mot ;
   - **W2**, P2 sur 1 000 000 de lignes, une sur dix mauvaise : 2a, 2b, 2c et 2d (2d sur des lignes toutes bonnes) ;
   - **W3**, P4 sur 1 000 000 de recherches qui trouvent leur clé neuf fois sur dix, le repli une chaîne fabriquée à chaque appel : 3a, 3b et 3c.
3. **Charge du processeur** : le temps CPU sur le temps écoulé des tours de chaque module.
4. **Mémoire et octets** : la mémoire du module après chaque charge ; ses octets, bruts et compressés.
5. **Les modes s’accordent** : pour Q3, un repli qui a un effet donne le même résultat dans les deux modes pour chaque option gardée ; une option qui ne peut pas les accorder est écartée.
6. **Une option non construite** est rapportée comme telle, avec la raison, jamais classée sur une supposition (constat M001).

## Décision

**Les options 1b, 2e et 3b**, chacune la plus facile à écrire et aussi rapide que la plus rapide de sa question.

1. **Un déplacement a lieu à l’appel** (1b). Un argument qui déplace une variable la déplace une fois chaque argument évalué : un argument suivant peut la lire, tant qu’il ne la prête pas au même appel. `m.set(key, m.get(key).unwrapOr(0) + 1)` se construit ; `f(key, key)`, le second prêté, reste refusé.
2. **Une option est rétrécie par un test qui part** (2e). Après `if (x.isNone()) { … }` dont le bloc ne finit jamais normalement, un `const` ou un paramètre non `inout` `x` est sa valeur pour le reste du bloc : `x.qty`, `x * 2`, `f(x)`. Le lire lit la valeur que tient l’option, prêtée ou déplacée comme celle de toute variable.
3. **Un repli n’est évalué que s’il est nécessaire** (3b), dans les deux modes, comme le font `??` et `? :` en JavaScript : `unwrapOr` exécute son repli dans la branche où il n’y a pas de valeur, quoi que le repli fabrique. Cela corrige F012.
4. Les autres options restent constructibles derrière `PERAS_PROTO_MOVE`, `PERAS_PROTO_TAKE` et `PERAS_PROTO_FALLBACK`, pour le script de mesure seulement ; sans eux, une construction suit cette décision.

## Critères de l’étape, enregistrés avant son code

1. **Tests UI** : les six programmes du protocole, tels qu’écrits, se vérifient sans erreur ; `f(key, key)` avec le second prêté, le rétrécissement d’un `let`, et le rétrécissement après un bloc qui ne part pas restent refusés, avec leurs codes.
2. **De bout en bout, sur les deux allocateurs**, contre le même source construit comme module managed : les six programmes, une option rétrécie qui tient une chaîne, lue puis déplacée, et rétrécie dans une boucle ; un déplacement à l’appel dans une table et dans une fonction ; un repli qui a un effet, un qui fabrique une chaîne, et un qui fabrique un temporaire. Les mêmes résultats, rien de vivant.
3. **Le test différentiel** gagne une option rétrécie, un repli paresseux qui a un effet sur une variable de module, et une clé déplacée à l’appel ; il passe 2 000 graines.
4. **Chaque règle cassée exprès est prise** : des déplacements appliqués à chaque argument évalué, dans le vérificateur de libération, refusent les programmes du critère 2 ; un nom rétréci qui lit l’option entière, dans l’abaissement, fait échouer les tests de bout en bout ; le repli exécuté quand il y a une valeur, dans le générateur, désaccorde les modes et les fait échouer.
5. **Mesures**, rapportées, pas des conditions : `node scripts/bench-options.mjs` de nouveau au commit de l’étape ; l’A/B du tableau de bord contre le commit d’avant (`ab-data.mjs`).
6. **Docs**, en anglais et en français : la page du guide sur les options, la référence des méthodes pour `unwrapOr`, l’état, les constats F012, F101, F103, F104 et F105, la feuille de route, le journal des changements, `CLAUDE.md`. Et ce que l’audit du 2026-11-17 a trouvé périmé : le paragraphe d’état du README, l’introduction du guide, deux liens cassés ; et une vérification de chaque lien des pages Markdown dans `pnpm run check`.

## Registre

_Chaque partie est consignée ici à son arrivée._

- **2026-11-17, les options mesurées** (`node scripts/bench-options.mjs`, [résultats](../../../bench/options/RESULTS.md) ; prototypes à `8857c01`, derrière `PERAS_PROTO_MOVE`, `PERAS_PROTO_TAKE` et `PERAS_PROTO_FALLBACK`). Un Intel Xeon à 2,30 GHz, 4 cœurs, Node 24, 21 tours, modules owned sur TLSF. Chaque prototype donne les résultats du module managed sur chaque charge et programme qu’il accepte, et ne laisse rien de vivant.
  - **Vitesse**, la médiane des rapports à l’option qui garde le comportement d’aujourd’hui :

    | Charge                                                | Options                                                                                 |
    | ----------------------------------------------------- | --------------------------------------------------------------------------------------- |
    | W1, mots comptés                                      | 1a copié par le programme 1 ; **1b à l’appel 0,981** ; 1c copié pour le programme 1,474 |
    | W2, lignes lues, une sur dix mauvaise                 | 2a 1 ; 2b 1,023 ; 2c 0,601 ; **2e rétréci 0,618**                                       |
    | W2, toutes les lignes bonnes                          | 2a 1 ; 2d `unwrap()` 0,576                                                              |
    | W3, une chaîne comme repli, trouvée neuf fois sur dix | 3a toujours 1 ; **3b si nécessaire 0,463** ; 3c `unwrapOrElse` 0,811                    |

    2a et 2b copient la valeur qu’ils prennent, de l’option que garde une variable ou de la branche qui la prête ; 2c, 2d et 2e non. La copie de 1c, insérée par le compilateur, coûte plus que la même copie écrite dans le source (1a) ; pourquoi n’a pas été cherché.

  - **Charge du processeur** : le temps CPU sur le temps écoulé est entre 0,98 et 1,00 partout : aucune option ne la change.
  - **Mémoire et octets** : la même mémoire pour chaque option d’une charge ; les modules diffèrent de 90 octets au plus (2 932 à 3 029 compressés).
  - **Facilité d’écriture**, six programmes écrits depuis JavaScript :

    |            | Programmes qui se construisent tels qu’écrits | Erreurs rencontrées | Lignes changées | Notions que JavaScript ne demande pas       |
    | ---------- | --------------------------------------------: | ------------------: | --------------: | ------------------------------------------- |
    | 1a         |                                       0 sur 2 |                   2 |               4 | `copy()` là où un argument suivant lit      |
    | **1b**     |                                       2 sur 2 |                   0 |               0 | aucune                                      |
    | 1c         |                                       2 sur 2 |                   0 |               0 | aucune, mais une copie invisible            |
    | 2a         |                                       0 sur 3 |                   5 |              11 | une valeur de remplacement jamais utilisée  |
    | 2b         |                                       0 sur 3 |                   5 |              49 | `match` avec une branche qui part, `copy()` |
    | 2c         |                                       0 sur 3 |                   5 |              24 | `const … else`                              |
    | 2d         |                                       0 sur 3 |                   5 |              11 | `unwrap()`, qui arrête le programme         |
    | **2e**     |                                       3 sur 3 |                   0 |               0 | aucune                                      |
    | 3a, 3b, 3c |                                       1 sur 1 |                   0 |               0 | aucune (3c offre `unwrapOrElse`)            |

  - **Les modes s’accordent** : aujourd’hui, un repli qui a un effet donne `501, 202` owned et `500, 101` managed (F012) ; 3a, 3b et 3c les accordent chacune.
- **Écarts au protocole.**
  - **Une option a été ajoutée après mesure** : 2e, le rétrécissement, comme TypeScript le fait : après `if (x.isNone())` dont le bloc part, un `const` ou un paramètre `x` est sa valeur pour le reste du bloc. Chaque programme de Q2 s’écrit ainsi en JavaScript, et aucune de 2a à 2d ne les acceptait.
  - **2c est prototypée pour `Option` et `Result` ; 2e pour `Option` et `isNone()` seulement.** 2d arrête le programme comme le fait un indice hors bornes, un message à écrire si elle est choisie.
- **Ce que disent les mesures** : 1b, 2e et 3b sont chacune la plus facile à écrire, sans erreur ni changement, et aussi rapide que l’option la plus rapide de sa question (1b et 1a, 2e et 2c, 3b seule). Ce sont les options à proposer au propriétaire, qui choisit.
- **2026-11-17, l’étape : 1b, 2e et 3b par défaut** (`PERAS_PROTO_MOVE`, `PERAS_PROTO_TAKE` et `PERAS_PROTO_FALLBACK` nomment désormais une autre option pour le script de mesure seulement, ou `before`, le compilateur d’avant cette décision). Ses critères :
  1. **Tests UI** : les six programmes tels qu’écrits se vérifient sans erreur (`tests/ui/check/options-as-written.peras`) ; `f(key, key)` avec le second prêté reste refusé (`E0204-moved-at-call.peras`) ; rétrécir un `let`, et après un bloc qui ne part pas, restent refusés (`E0221-narrowing.peras`). Atteint.
  2. **De bout en bout, sur les deux allocateurs** (`packages/vite-plugin-peras/e2e/options.test.ts`), contre la même source construite en module managed : les six programmes, une chaîne rétrécie lue puis déplacée, rétrécie dans une boucle, un déplacement à l’appel dans une table, un constructeur et une fonction, les trois replis. Les mêmes résultats, rien de vivant. Atteint.
  3. **Le test différentiel** gagne une chaîne et un nombre rétrécis, un repli paresseux à effet sur une variable de module, une clé déplacée à l’appel dans une table et un constructeur ; il passe 2 000 graines. Atteint.
  4. **Chaque règle cassée exprès est attrapée** : (a) les déplacements appliqués à chaque argument évalué, dans le vérificateur de libération, font refuser le programme de bout en bout (`movedAtCall` : « %27 is used after its block was released or moved ») ; (b) un nom rétréci qui lit l’option entière, dans l’abaissement, fait échouer les tests de bout en bout (le vérificateur refuse le module) ; (c) le repli exécuté d’abord, dans le générateur JavaScript, fait diverger les modes (`"5|one|5|0"` contre `"5|one|5|11"`). Atteint, après un écart ci-dessous.
  5. **Mesures**, à `ad1ff36`, la même machine que pour la mesure des options (un Intel Xeon à 2,30 GHz, 4 cœurs, Node 24), 21 tours :
     - `node scripts/bench-options.mjs` ([résultats](../../../bench/options/RESULTS.md)), chaque option contre le compilateur d’avant la décision : 1b 0,980, 2e 0,625, 3b 0,456, comme lors de la mesure des options (0,981, 0,618, 0,463) ; la charge du processeur 1,00 et la mémoire la même partout. Les modes s’accordent désormais sur un repli à effet, `500, 101` dans les deux ; avant l’étape, `501, 202` et `500, 101`. Le module managed est plus rapide que chaque module owned sur W2 et W3 (0,12 à 0,36 du temps) : les deux construisent et parcourent des chaînes, qu’un module owned copie dans sa mémoire ; consigné comme constat F220.
     - `node scripts/ab-data.mjs cbe3981`, le tableau de bord contre le commit d’avant l’étape, 20 tours : 0,996 tour par tour, de 0,936 à 1,151. Aucun changement : les replis du tableau de bord étaient déjà silencieux (F209).
  6. **Docs** : les pages du guide sur les options et la mémoire, la référence des méthodes, l’état, la feuille de route, le journal des modifications, les constats (F012, F103, F104, F105 fermés, F101 en partie), `CLAUDE.md` ; le paragraphe d’état du README et l’introduction du guide, périmés depuis la décision 0009 ; les deux liens cassés corrigés, et `scripts/check-links.mjs` dans `pnpm run check`, qui vérifie chaque lien des 325 pages Markdown, fichier et titre. Atteint.
- **Écarts aux critères.**
  - **La mutation (a) passait d’abord** (constat M027) : aucun programme des tests de bout en bout n’avait d’argument suivant qui lise sur place la valeur déplacée. Un constructeur, `Sized(text, text.length)`, et un appel de fonction ont été ajoutés ; le constructeur était refusé par le vérificateur de libération sans la mutation, un bogue de l’étape (F013), corrigé : les nombres qui suivent un déplacement dans un constructeur, `Some`, `Ok` ou `Err` sont calculés d’abord.
  - **Le rétrécissement cassait les programmes écrits avant lui** (F014), trouvé par les tests de bout en bout d’`examples/stream` : `parsed.unwrapOr(…)` après `if (parsed.isNone()) { continue; }`. Sur un nom rétréci, les méthodes d’`Option` lisent l’option entière ; la décision ne le disait pas.
  - **Le test de bout en bout construit son module managed sans `sink`**, qu’un module managed refuse (`E0243`) et qui n’y change rien, puisque rien ne s’y déplace.
  - **Les 2 000 graines du test différentiel ont tourné dans le build release** (66 s) ; ses graines par défaut dans le build debug que lance la CI (70 s, constat M022).
