# 0031 : Des boucles parallèles qui peuvent dessiner une image

_Version anglaise : [../../en/decisions/0031-useful-parallel-loops.md](../../en/decisions/0031-useful-parallel-loops.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet (commit `3a1ba4a`) ; options mesurées à `29b742f` ; option p2 acceptée par le propriétaire le 2026-11-17. Il répond au constat F301, que la [décision 0029](0029-pond-of-lanterns.md) a rencontré de nouveau : le bassin ne peut pas être dessiné par un `parallel for`, et sur cette image seuls les threads peuvent faire passer Peras devant JavaScript (son profil, constat F222)._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Un `parallel for` est une tâche** ([décision 0007](0007-threads-over-shared-memory.md), `crates/perasc-ir/src/lower/parallel.rs`) : son corps devient une fonction, `f__par0(start, end, captures…)`, que les workers du chargeur exécutent sur des plages de tours, sur une seule mémoire partagée. Chaque worker instancie le module sans runtime : une tâche n’alloue jamais.
- **Ce qu’un tour ne peut pas faire** (`check`, `E0241`) : affecter une variable déclarée hors de la boucle ; écrire un élément à un autre indice que celui de son compteur (`Index`) ; lire, à un autre indice, un tableau que la boucle écrit (`Reads`) ; appeler une fonction (`Calls`) ; créer, déplacer ou libérer un tampon (`Heap`) ; lire ou écrire une variable de module, chaque thread ayant les siennes (`Global`) ; quitter la boucle.
- **Ce dont le bassin a besoin** : son ciel appelle `hash`, `unit` et `tri`, qui n’allouent rien et ne touchent aucune variable de module ; une lanterne remplit des pixels sur tout l’écran, si bien qu’un tour découpé par lanternes écrirait n’importe où, mais un tour découpé par bandes de lignes n’écrit que sa bande, et relit la profondeur qu’il y a écrite. Découpé par lignes, chaque pixel voit les mêmes opérations dans le même ordre que sur un thread : l’image reste la même, au bit près.
- **JavaScript a des threads lui aussi** : des Web Workers sur un `SharedArrayBuffer`. Un Peras sur threads est comparé à un TypeScript sur workers, pas seulement au TypeScript sur un thread.

## Options

- **p0. Comme aujourd’hui** : l’image ne peut pas s’écrire en `parallel for` ; le bassin tourne sur un thread.
- **p1. Des appels purs dans un tour** : un tour peut appeler une fonction qui, avec chaque fonction qu’elle appelle, ne crée, ne déplace ni ne libère aucun tampon, ne lit ni n’écrit aucune variable de module, n’écrit par aucun paramètre, et n’appelle aucune valeur fonction. Le vérificateur le prouve à partir du graphe d’appels ; un appel qui n’est pas pur reste refusé, sa raison nommée.
- **p2. p1, et des parts** : dans un tour compté par `b`, `xs.part(b, k)` est les éléments `b * k` à `(b + 1) * k` de `xs`, que le tour peut lire et écrire ; les parts de deux tours ne se chevauchent jamais, par construction. Une part est une vue : rien n’est copié, et elle ne peut pas quitter le tour. Le tour du bassin prend la part de sa bande dans `light`, `depth` et `pixels`.
- **p3. p1, et des plages prouvées** : aucune notion nouvelle ; le vérificateur prouve que chaque indice qu’un tour écrit, ou lit dans un tableau qu’il écrit, tombe dans une plage à lui, `[b * k, (b + 1) * k)`, à partir des bornes de ses boucles. Construite seulement si elle peut l’être dans cette décision ; sinon dite non construite, avec pourquoi (constat M001).

## Protocole, enregistré avant tout prototype

Quatre quantités, comme pour les décisions 0027, 0028 et 0030. Chaque option est prototypée derrière `PERAS_PROTO_PAR` ; le build sans elle garde p0.

1. **Facilité d’écriture** : quatre programmes, écrits comme un utilisateur les écrit à partir du TypeScript sur un thread, puis corrigés jusqu’à ce qu’ils se construisent, pour chaque option :
   - P1, le bassin, image 600 découpée en bandes de lignes ;
   - P2, le jardin de la décision 0013, découpé en lignes ;
   - P3, un flou d’image, chaque ligne de la sortie à partir de trois lignes de l’entrée ;
   - P4, le tableau de bord de la décision 0014 découpé en morceaux de lignes, qui alloue et somme : attendu refusé par chaque option, le refus rapporté.

   Pour chacun : les erreurs rencontrées avant qu’il se construise, avec leurs codes, les lignes changées par rapport au programme sur un thread, les notions que JavaScript ne demande pas.

2. **Les mêmes résultats** : chaque programme construit, sur threads, donne les octets qu’il donne sur un thread et que donne le TypeScript, vérifiés par un hash.
3. **Charges**, dans Chromium, la page isolée de toute autre origine, 15 exécutions après 2, les versions alternant, le rapport tour par tour à Peras sur un thread (constat M017) : W1, le bassin, image 600 en 1024 × 768 avec 1 000 lanternes ; W2, le jardin en 1024 × 1024 ; W3, le flou d’une image de 2048 × 2048. Chacune contre le TypeScript sur un thread et le TypeScript sur autant de workers, découpé de la même façon, sa mémoire un `SharedArrayBuffer`.
4. **Charge du processeur** : le temps CPU de chaque thread sur le temps réel, comme le mesure la décision 0012 : les threads finissent plus tôt et occupent plus de cœurs, et les deux sont rapportés.
5. **Mémoire et octets** : la mémoire de chaque module et des workers ; les octets du module et de son chargeur, bruts et gzip.
6. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-parallel.mjs` les lance ; `--check` vérifie que chaque programme construit donne ses octets sur un thread, et tourne avec chaque vérification du projet.

## Décision

**Option p2 : des appels purs, et des parts.**

1. Un tour de `parallel for` peut appeler une fonction qui, avec chaque fonction qu’elle appelle, ne crée, ne déplace ni ne libère aucun tampon, n’écrit aucun élément, ne lit ni n’écrit aucune variable de module, et n’appelle aucune valeur fonction ; le vérificateur le prouve à partir du graphe d’appels, et nomme la raison d’un appel qu’il refuse.
2. Dans un tour compté par `b`, `let band = xs.part(b, k)` est les éléments `b * k` à `(b + 1) * k` de `xs`, que le tour peut lire et écrire par `band` ; `k` ne doit pas dépendre du tour. Les parts de deux tours ne se chevauchent jamais. Une part est une vue : rien n’est copié, et elle ne peut pas quitter le tour. Un tableau écrit par des parts n’est écrit et lu que par elles dans le tour.
3. Le chargeur découpe une boucle dès deux tours quand son tour contient une boucle ou un appel, un tour lourd ; une boucle dont le tour ne fait ni l’un ni l’autre attend toujours 256 tours, réveiller les workers coûtant plus que de tels tours.
4. Les options p0 et p1 restent constructibles derrière `PERAS_PROTO_PAR`, pour le script de mesure seulement.

## Critères de l’étape, enregistrés avant son code

1. **Tests de construction** : une page de boucles qui se construisent, un appel pur, un appel rendant une structure, des parts dans deux tableaux, une part lue là où elle est écrite ; et refusées par `E0241` avec leur raison : une part qui n’est pas prise au compteur, une part dont le tour calcule la taille, un tableau écrit par une part et au compteur, un appel à une fonction qui écrit une variable de module, un appel à une qui alloue, un appel par une valeur fonction, une part utilisée hors d’un tour.
2. **Les mêmes résultats** : les tests de l’interpréteur sur `parallel for` gagnent un appel pur et des parts, non optimisés et optimisés, contre la boucle sur un thread ; `node scripts/bench-parallel.mjs --check` donne les octets du TypeScript sur quatre threads pour le bassin, le jardin et le flou.
3. **Chaque règle cassée exprès est attrapée** : une part acceptée à un autre indice que le compteur, et un appel accepté à une fonction qui écrit une variable de module, font chacun échouer un test de construction.
4. **Mesures**, rapportées, pas des conditions : `node scripts/bench-parallel.mjs` au commit de l’étape ; `node scripts/bench-workers.mjs`, dont les boucles d’un élément par tour gardent le seuil de 256.
5. **Docs**, en anglais et en français : la page du guide sur les threads, la référence des méthodes pour `part`, la page d’`E0241`, l’état, les constats F301 et F015, la feuille de route, le journal des modifications, `CLAUDE.md`.

## Registre

_Chaque partie est consignée ici à son arrivée._

- **2026-11-17, les options mesurées** (`node scripts/bench-parallel.mjs`, [résultats](../../../bench/parallel/RESULTS.md) ; prototypes à `10f7efe`, derrière `PERAS_PROTO_PAR` ; exécution 2 à `29b742f`). Un Intel Xeon à 2,30 GHz, 4 cœurs, Chromium sans écran, la page isolée de toute autre origine, 4 threads. Chaque version, sur threads ou non, donne les octets du TypeScript, vérifiés par SHA-256.
  - **Facilité d’écriture** : chaque programme tel qu’écrit d’abord sur threads est refusé par chaque option (`E0241`) ; la version qui se construit :

    | Programme           | p0, aujourd’hui              | p1, appels purs              | p2, et des parts            |
    | ------------------- | ---------------------------- | ---------------------------- | --------------------------- |
    | P1, le bassin       | aucune                       | aucune                       | en bandes, 102 lignes       |
    | P2, le jardin       | aucune : il appelle          | un tour par pixel, 11 lignes | un tour par ligne, 3 lignes |
    | P3, le flou         | un tour par pixel, 29 lignes | la même                      | un tour par ligne, 8 lignes |
    | P4, tableau de bord | aucune : il alloue et somme  | aucune                       | aucune                      |

    p1 ne peut pas dessiner le bassin : sans parts, un tour n’écrit que son propre indice, et un pixel devrait regarder les 88 000 triangles. Le flou se construit avec p0 une fois que son tour ne crée plus de tableau, `for (const row of [up, y, down])` écrit en trois lignes.

  - **Vitesse**, la médiane des rapports tour par tour, 15 exécutions :

    | Charge                                     | TypeScript, 1 thread | TypeScript, 4 workers |  Peras, 1 thread | Peras sur threads                             |
    | ------------------------------------------ | -------------------: | --------------------: | ---------------: | --------------------------------------------- |
    | W1, le bassin, 1024 × 768, 1 000 lanternes |              54,3 ms |    27,0 ms (0,497 TS) |  52,5 ms (0,934) | p2 : 25,5 ms, **0,424 du TypeScript**         |
    | W2, le jardin, 1024 × 1024                 |             766,6 ms |   313,2 ms (0,406 TS) | 862,4 ms (1,128) | p1 : 251,5 ms, 0,325 ; p2 : 259,0 ms, 0,337   |
    | W3, le flou, 2048 × 2048                   |              48,3 ms |    13,8 ms (0,290 TS) |  93,8 ms (1,946) | p0 et p1 : 7,3 ms, 0,151 ; p2 : 4,3 ms, 0,090 |

  - **Charge du processeur**, le temps CPU de chaque thread sur le temps réel : 1,0 sur un thread, 2,6 à 3,8 sur quatre, les workers de TypeScript et Peras de même : les threads finissent plus tôt en occupant plus de cœurs.
  - **Mémoire et octets** : les mêmes tampons pour chaque version (27, 4 et 64 Mio) ; Peras sur threads livre 6,5 à 10,9 Ko en gzip, son chargeur portant la réserve de workers, contre 4,3 à 8,2 Ko sur un thread ; TypeScript 0,2 à 2,1 Ko.
- **Écarts au protocole.**
  - **Le chargeur découpe une boucle dès deux tours** en p1 et p2, là où il attendait 256 : un tour peut être une bande de lignes. Sans cela, les 4 bandes du bassin tournaient sur la page seule (77 ms, vu avant l’exécution).
  - **Le nombre de bandes**, que le protocole laissait ouvert, est 4, les cœurs, choisi après des essais informels de 4 à 48 bandes ; le bassin à une ligne par tour n’a pas été essayé.
  - **Le flou sur threads ne crée pas de tableau par pixel**, là où le TypeScript et Peras sur un thread le font : Peras sur un thread paie une allocation par pixel (constat F223), si bien que son rapport à Peras sur threads exagère ce que donnent les threads ; face au TypeScript, que V8 garde d’allouer, la comparaison tient.
  - **L’exécution 1 est gardée avec deux défauts** dans [run-1](../../../bench/parallel/run-1/RESULTS.md) : les octets des versions TypeScript laissaient de côté leur module, et la mémoire du TypeScript sur un thread ses tableaux typés (constat M006) ; corrigés pour l’exécution 2, les temps semblables.
  - **Un bogue trouvé et corrigé dans le prototype** (constat F015) : un appel rendant plusieurs résultats dans un tour gardait ses variables de résultat hors de la tâche.
- **Ce que disent les mesures** : seule p2 dessine le bassin, et Peras sur threads y prend 0,424 du temps du TypeScript, 2,4 fois plus vite, et un peu moins que le TypeScript sur quatre workers (0,497). Sur le jardin, p1 et p2 se valent, 3 fois la vitesse du TypeScript et 1,2 fois celle de ses workers. Sur le flou, p2 est la plus rapide, 11 fois la vitesse du TypeScript. p2 est aussi la plus facile là où une image se découpe en lignes : 3 et 8 lignes changées. p2, qui contient les appels purs de p1, est l’option à proposer au propriétaire, qui choisit.
- **2026-11-17, l’étape : l’option p2 par défaut** (`b7ca6ac` ; `PERAS_PROTO_PAR` nomme une autre option pour le script de mesure seulement, `p0` ou `before` les règles d’avant cette décision). Ses critères :
  1. **Tests de construction** : `tests/ui/build/parallel-pure-and-parts.peras` se construit ; `E0241-parallel-parts.peras` est refusé une fois par boucle, avec sa raison : `impure` nommant `bump`, et `mark`, qui écrit une variable de module sans la lire ; `part` pour une part en `b + 1`, d’une taille que le tour calcule, et hors d’un tour ; `index` pour un tableau écrit par une part et au compteur. Les tests de l’interpréteur refusent les mêmes, et un appel par une valeur fonction. Atteint.
  2. **Les mêmes résultats** : `pure_calls_and_parts_give_what_one_thread_gives`, non optimisé et optimisé, contre la boucle écrite à la main sur un thread, rien de vivant ; `node scripts/bench-parallel.mjs --check`, les octets du TypeScript sur quatre threads. Atteint.
  3. **Chaque règle cassée exprès est attrapée** : une part acceptée à un autre indice que le compteur fait échouer les deux tests ; un appel accepté vers une fonction qui écrit une variable de module n’en faisait d’abord échouer aucun, `bump` lisant la variable qu’elle écrit, ce que la règle sur les lectures refuse déjà (constat M024) ; avec `mark`, qui ne fait qu’écrire, il fait échouer les deux. Atteint, après cet écart.
  4. **Mesures**, à `b7ca6ac` : `node scripts/bench-parallel.mjs` ([résultats](../../../bench/parallel/RESULTS.md)) redonne le bassin sur quatre threads à 0,417 du temps du TypeScript (ses workers 0,441), le jardin à 0,323 (0,381), le flou à 0,098 (0,306) ; `node scripts/bench-workers.mjs`, ses boucles d’un élément par tour : leur module est le même, au bit près, construit par les règles d’avant cette décision et d’après, chaque boucle découpée à partir de 256 tours ; leurs accélérations, 2,12, 2,52 et 1,60, contre 2,18, 2,74 et 1,89 relevées un autre jour, sur une autre machine (constat M016).
  5. **Docs** : la section du guide sur `parallel for`, la référence des méthodes pour `part`, la page et les messages d’`E0241`, l’état, les constats F301 fait en partie et F015 fermé, la feuille de route, le journal des modifications, `CLAUDE.md`. Atteint.
