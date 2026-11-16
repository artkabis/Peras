# 0027 : Les fermetures qui capturent des chaînes, des tableaux et des valeurs fonctions

_Version anglaise, qui fait foi : [../../en/decisions/0027-closures-over-the-heap.md](../../en/decisions/0027-closures-over-the-heap.md)._

_État : acceptée par le propriétaire le 2026-11-16, option F, après que ses options ont été mesurées selon un protocole enregistré avant tout prototype ; la première décision choisie en mesurant ses options, comme le propriétaire l’a demandé le même jour. Elle suit la [décision 0026](0026-closures.md), dont elle lève le refus `closure-heap`._

## Contexte

Ce que fait le code aujourd’hui, lu avant d’écrire ceci (constat M020).

```peras
fn label(prefix: String): (String) => String {
  return (s: String): String => `${prefix}${s}`; // E0900 : closure-heap
}
```

- **Un environnement tient des emplacements `i64`**, des nombres seulement, écrits quand la fermeture est faite et jamais après. Il est compté : une copie de la valeur fonction en ajoute un, la dernière libération le libère ([décision 0026](0026-closures.md)).
- **Un `const` ne peut pas changer, en profondeur** : `const xs = []; xs.push(1);` est `E0205`. Une chaîne ou un tableau `const` capturé ne peut pas changer tant que la fermeture vit, sauf s’il est déplacé ailleurs d’abord.
- **Les chaînes, tableaux, tables et ensembles ont un seul propriétaire** et se déplacent ([décision 0003](0003-runtime-proved-at-compile-time.md)) ; une copie s’écrit `copy()`. Rien n’est partagé sur le tas aujourd’hui, sauf les environnements.
- **La passe de propriété lit deux fois le corps d’une fonction fléchée**, comme le corps d’une boucle, et y refuse un déplacement.
- **Un module managed capture par référence**, comme JavaScript.

## Options

Chacune est construite en prototype quand c’est possible, et mesurée selon le protocole ci-dessous avant qu’une soit choisie.

- **A. Déplacer dans l’environnement.** Faire la fermeture y déplace ce qu’elle capture : la variable ne peut plus servir ensuite (`E0204`), sauf si le programme écrit `s.copy()` lui-même. L’environnement possède ses tampons et les libère avec son dernier détenteur. Aucune copie n’est faite en secret.
- **B. Copier dans l’environnement.** Faire la fermeture copie ce qu’elle capture, comme JavaScript le lirait, puisqu’un `const` ne peut pas changer : la variable reste utilisable, et le coût est une copie par fermeture faite.
- **C. Partager par compte.** Le tampon capturé est compté, comme un environnement : la fermeture et la variable tiennent le même. Il faut un compte sur chaque tampon, `Rc<T>`, le constat F310.
- **D. Prêter.** La fermeture emprunte ce qu’elle capture ; elle peut être appelée et passée plus bas, mais pas rendue, rangée, ni gardée au-delà de la portée de ce qu’elle emprunte. Ni copie ni compte ; une fermeture qui s’échappe est refusée.
- **E. Prêter, sinon déplacer.** D là où la fermeture ne s’échappe pas, A là où elle s’échappe.

## Protocole, enregistré avant tout prototype

Quatre grandeurs, comme le propriétaire l’a demandé : la vitesse, la charge du processeur, la mémoire et les octets, et la facilité d’écriture.

1. **Charges**, dans un module owned construit sur TLSF, exécuté dans Node, le module de chaque option construit du même source là où l’option l’accepte :
   - **W1, faite puis appelée une fois** : une fermeture qui capture une chaîne de 16 octets, 1 Kio et 64 Kio, faite, appelée une fois et abandonnée, 1 000 000, 100 000 et 2 000 fois.
   - **W2, appelée souvent** : une fermeture qui capture une chaîne de 1 Kio, appelée 20 000 000 de fois, chaque appel lisant un octet de celle-ci.
   - **W3, qui s’échappe** : `label(prefix)` rendue par une fonction, puis donnée à `map` sur 100 000 chaînes de 10 octets.
   - Le même source construit comme module managed, qui est du JavaScript, sert de référence.
2. **Vitesse** : le temps de chaque charge, le module de chaque option dans un même processus, l’ordre alterné à chaque tour, 21 tours, comparés par la médiane des rapports à ceux de l’option B, et leur étendue (constat M017).
3. **Charge du processeur** : le temps CPU du processus (`process.cpuUsage`, utilisateur et système) sur les 21 tours de chaque module, divisé par le temps écoulé : les cœurs gardés occupés, comme la décision 0012 mesure une page.
4. **Mémoire et octets** : le pic de la mémoire du module (`memory.buffer.byteLength`) et le plus de blocs vivants (`__perasHeap`) pendant chaque charge ; les octets du module, bruts et compressés par gzip.
5. **Facilité d’écriture** : cinq programmes, écrits pour chaque option comme un utilisateur les écrirait, en partant du JavaScript qu’il écrirait, puis corrigés jusqu’à ce qu’ils se construisent :
   - un fabricant d’étiquettes, `label(prefix)` rendue ;
   - un filtre par un tableau de mots capturé ;
   - une fermeture gardée dans une structure, avec un nom capturé ;
   - des rappels rangés dans un tableau, chacun capturant sa propre chaîne ;
   - une fermeture qui capture une valeur que la fonction utilise encore après.

   Pour chacun : les lignes changées par rapport au JavaScript, les notions qu’un utilisateur doit apprendre et que JavaScript ne demande pas, et les erreurs rencontrées avant que le programme se construise, avec leurs codes.

6. **Une option non construite** est rapportée comme telle, avec la raison, et classée sur rien de mesuré (constat M001).
7. **L’exactitude d’abord** : chaque prototype passe les tests de bout en bout de la décision 0026 et les résultats des nouvelles charges contre le module managed, sans rien laisser de vivant, avant que ses temps comptent.

## Décision

**L’option F**, trouvée en mesurant les autres : ce qu’une fermeture capture sur le tas est copié dans son environnement, qui le possède, sauf si la fermeture ne peut pas s’échapper, et alors il est prêté.

- Une fermeture ne peut pas s’échapper quand elle est donnée sur place comme argument, ou gardée dans un `const` qui est seulement appelé ou donné comme argument, et que rien de ce qu’elle capture ne se déplace tant qu’elle vit. La passe de propriété le décide, fonction par fonction, et l’abaissement prête ou copie.
- Une valeur fonction capturée suit la même règle : prêtée là où la fermeture ne peut pas s’échapper, tenue une fois de plus là où elle s’échappe, comme l’est une copie d’elle ([décision 0026](0026-closures.md)).
- Le programme s’écrit comme l’option B le demande : aucune notion nouvelle, aucune erreur nouvelle. Le refus `closure-heap` disparaît.
- L’interrupteur de prototype `PERAS_PROTO_CLOSURE` reste, pour le script de mesure seulement ; sans lui, une construction est l’option F.

## Critères de l’étape, enregistrés avant son code

1. **Tests UI** : `closures.peras` gagne une fermeture qui prête une chaîne, une qui la copie parce qu’elle s’échappe, une qui la copie parce que ce qu’elle capture se déplace ensuite, et une valeur fonction capturée ; leur IR est gardée. `E0900-closure.peras` disparaît, avec son refus.
2. **De bout en bout, sur les deux allocateurs**, contre le même source construit comme module managed : les cinq programmes du protocole, une fermeture qui capture une table, un ensemble, un tableau de chaînes et une structure qui en tient une, et une fermeture qui capture une fermeture. Les résultats sont les mêmes, et rien ne reste vivant.
3. **Prêter est sûr** : prêter ce que capture une fermeture qui s’échappe, exprès dans l’abaissement, fait échouer les tests de bout en bout (une fermeture qui lit un tampon déjà libéré) ; copier exprès là où elle prête ne change aucun résultat, et la mesure en montre le coût.
4. **Le test différentiel** gagne des fermetures qui capturent des chaînes, l’une s’échappant dans un tableau, l’autre seulement appelée, et passe 2 000 graines. Lire le mauvais emplacement de poignée, exprès dans le générateur WebAssembly, le fait échouer.
5. **Le compte tient** : un environnement qui possède des tampons les libère avec son dernier détenteur ; ne lui en faire posséder aucun, exprès, laisse des blocs vivants, que les tests prennent.
6. **Mesures**, rapportées, pas des conditions : `node scripts/bench-closures.mjs` de nouveau au commit de l’étape, F contre les prototypes ; l’A/B du tableau de bord contre le commit d’avant (`ab-data.mjs`).
7. **Docs**, en anglais et en français : la page du guide sur les fonctions, l’état, `E0900`, la référence de la CLI si elle change, la suite du constat F308, la feuille de route, le journal des changements, `CLAUDE.md`.

## Registre

_Chaque partie est consignée ici à son arrivée._

- **2026-11-16, les options mesurées** (`node scripts/bench-closures.mjs`, [résultats](../../../bench/closures/RESULTS.md) ; prototypes à `39743d8`, derrière `PERAS_PROTO_CLOSURE`). Un Intel Xeon à 2,10 GHz, 4 cœurs, Node 24, 21 tours, modules owned sur TLSF. Chaque prototype donne les résultats du module managed sur chaque charge qu’il accepte, et ne laisse rien de vivant.
  - **Vitesse**, la médiane des rapports à B, tour par tour :

    | Charge                      | A, déplacer | B, copier |    D, prêter | F, copier sauf prêt | managed |
    | --------------------------- | ----------: | --------: | -----------: | ------------------: | ------: |
    | W1, 16 o, 1 000 000 de fois |       1,031 |         1 |        0,357 |               0,354 |   0,299 |
    | W1, 1 Kio, 100 000 fois     |       1,006 |         1 |        0,205 |               0,202 |   0,075 |
    | W1, 64 Kio, 2 000 fois      |       1,011 |         1 |        0,106 |               0,105 |   0,004 |
    | W2, 20 000 000 d’appels     |       0,995 |         1 |        0,992 |               0,989 |   1,659 |
    | W3, qui s’échappe           |       0,986 |         1 | inexprimable |               1,002 |   0,816 |

    A coûte ce que coûte B : ses programmes copient là où ils réutiliseraient ce qu’ils ont capturé, et un déplacement n’économise rien là où la fermeture est faite une fois. Prêter économise la copie partout où une fermeture est faite souvent, de 3 à 10 fois. Appelées souvent, toutes se valent, et vont 1,66 fois plus vite que JavaScript.

  - **Charge du processeur** : le temps CPU sur le temps écoulé est entre 0,98 et 1,12 pour chaque option owned : un cœur, rien de plus. Aucune différence entre les options.
  - **Mémoire et octets** : la mémoire du module ne diffère que pour W1 à 64 Kio : 320 Kio pour A, 256 pour B, 192 pour D et F, les copies demandant de la place. Les modules diffèrent de 16 octets au plus (1 922 à 1 931 compressés), 3,7 fois ceux de JavaScript, le coût de l’allocateur.
  - **Facilité d’écriture**, cinq programmes écrits depuis JavaScript :

    |                                               |                    A, déplacer |       B, copier |                                D, prêter | F, copier sauf prêt |
    | --------------------------------------------- | -----------------------------: | --------------: | ---------------------------------------: | ------------------: |
    | Programmes qui se construisent tels qu’écrits |                        2 sur 5 |         4 sur 5 |                                  2 sur 5 |             4 sur 5 |
    | Erreurs rencontrées                           |                              4 |               1 |                                        4 |                   1 |
    | Lignes changées pour construire               |                             11 |               2 |                                       25 |                   2 |
    | Programmes réécrits sans fermeture            |                              0 |               0 |                                        3 |                   0 |
    | Notions que JavaScript ne demande pas         | `sink`, une capture se déplace | aucune nouvelle | une fermeture qui ne peut pas s’échapper |     aucune nouvelle |

    L’unique erreur de B et F, dans P3, est une règle que Peras a sans fermetures : un paramètre prêté mis dans une structure demande `copy()` (`E0209`).
- **Écarts au protocole.**
  - **Une option a été ajoutée après mesure** : F, la copie de B évitée là où la fermeture ne peut pas s’échapper, choisie par l’analyse d’échappement de D au lieu d’être refusée. La première exécution, de A, B et D seules, est gardée dans [bench/closures/archive](../../../bench/closures/archive/run-1-without-f.md) ; ses chiffres concordent avec la seconde.
  - **C et E ne sont pas construites.** C demande un compte sur chaque tampon, `Rc<T>` (constat F310), un changement de chaque opération sur les tampons, pas un prototype. E est laissée pour F, qui n’en diffère que là où une fermeture s’échappe, en copiant là où E déplace ; ses chiffres ne sont pas déduits des autres.
  - **W2 lit la longueur et les deux premiers octets** (`startsWith`) plutôt qu’un octet à un indice : aucune méthode n’en lit un.
  - **La mémoire est celle du module après chaque charge**, qui ne fait que croître : le plus de blocs vivants à la fois demande un compteur que l’allocateur ne tient pas.
- **Ce que disent les mesures** : F s’écrit comme B, la plus facile, et s’exécute comme D, la plus rapide, là où D peut s’exécuter. C’est l’option à proposer au propriétaire, qui choisit.
- **2026-11-16, l’étape : l’option F arrive** (`572bfd9`).
  - La passe de propriété décide quelles fermetures ne peuvent pas s’échapper (`Analysis::escaping` tient celles qui le peuvent) ; l’abaissement prête ce que capturent les autres, et copie dans un environnement qui le possède ce que capturent celles qui s’échappent. Un environnement compte les tampons qu’il possède, ses premiers emplacements, et les libère avec son dernier détenteur ; un tampon qu’il prête est lu par `env.handle`. Une valeur fonction capturée suit la même règle. `closure-heap` et son test UI disparaissent.
  - Critères :
    1. `closures.peras` garde l’IR d’une fermeture qui prête un tableau, d’une qui copie une chaîne parce qu’elle s’échappe, d’une qui la copie parce qu’elle se déplace ensuite, et d’une fermeture qui capture une fermeture, prêtée.
    2. `closures.test.ts`, sur les deux allocateurs, contre le module managed : les cinq programmes du protocole, une table et un ensemble capturés (figés dans un `const` d’abord, constat F110), une structure qui tient une chaîne, une fermeture qui capture une fermeture. Les mêmes résultats, rien de vivant.
    3. Prêter ce que capture une fermeture qui s’échappe, exprès dans l’abaissement, fait échouer les deux exécutions de bout en bout : en debug une valeur lue dans un bloc libéré déclenche une vérification de débordement, sur TLSF les résultats diffèrent. Copier partout est l’option B, mesurée.
    4. Le test différentiel gagne deux fermetures sur deux chaînes chacune, l’une s’échappant dans un tableau ; il passe 2 000 graines. Lire la poignée de l’emplacement 0 pour chaque emplacement, exprès dans le générateur WebAssembly, fait échouer les deux tests différentiels à la graine 23. Avec une chaîne par fermeture, il passait : la mutation n’atteignait pas la règle (constat M024), et l’instruction a été changée.
    5. Un environnement à qui l’on dit qu’il ne possède rien, exprès dans le générateur, laisse 30 blocs vivants, que le test de bout en bout prend.
    6. Mesures à `572bfd9`, rien d’autre en cours : `bench-closures.mjs` donne à F 0,360, 0,204 et 0,104 fois le temps de B sur W1, 0,988 sur W2, 0,991 sur W3, comme le prototype. Le tableau de bord, `node scripts/ab-data.mjs 2a0a5aa`, 1 000 000 de lignes, 20 tours : 370,3 ms avant, 364,8 ms après, un rapport de 1,035, de 0,847 à 1,217, le même résumé.
    7. La page du guide sur les fonctions, l’état, `E0900`, les constats F308 et F110, la feuille de route, le journal des changements et `CLAUDE.md`, en anglais et en français. La référence de la CLI ne change pas.
