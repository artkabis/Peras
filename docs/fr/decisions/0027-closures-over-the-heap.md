# 0027 : Les fermetures qui capturent des chaînes, des tableaux et des valeurs fonctions

_Version anglaise, qui fait foi : [../../en/decisions/0027-closures-over-the-heap.md](../../en/decisions/0027-closures-over-the-heap.md)._

_État : proposée le 2026-11-16, avec ses options et son protocole enregistrés avant qu’aucun prototype ne soit construit ni mesuré ; la première décision choisie en mesurant ses options, comme le propriétaire l’a demandé le même jour. Elle suit la [décision 0026](0026-closures.md), dont elle lève le refus `closure-heap`._

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

## Critères de l’étape qui suit le choix

Fixés une fois une option choisie, avec le propriétaire : les tests UI, de bout en bout sur les deux allocateurs, le test différentiel, le vérificateur de libération, les docs, comme pour la décision 0026.

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
