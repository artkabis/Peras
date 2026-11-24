# 0039 : Durées et instants

_Version anglaise : [../../en/decisions/0039-durations-and-instants.md](../../en/decisions/0039-durations-and-instants.md)._

_État : protocole enregistré le 2026-11-18, avant tout prototype et toute mesure, comme le demande la règle du projet ; options mesurées au commit `370b4ce` ; t1 retenue par le propriétaire le 2026-11-23. Il répond à la ligne « temps » de la phase 3 de la [feuille de route](../reference/roadmap.md), durées et instants ; les dates du calendrier et les fuseaux horaires sont laissés à une décision suivante. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

Ce que fait Peras aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Le temps est un nombre de millisecondes.** Un module managed a `now(): f64`, les millisecondes depuis le début de la page, comme `performance.now()`, et `setTimeout(ms, f)` et `setInterval(ms, f)`, qui prennent un nombre ([référence du navigateur](../reference/browser.md)).
- **Rien ne distingue une durée d’un instant, ni une milliseconde d’une seconde** : `setTimeout(5, f)` voulu comme cinq secondes se construit et tourne après cinq millisecondes ; `now() + now()` se construit.
- **Un module owned n’a aucun temps** : il ne peut lire aucune horloge, et une durée y est le nombre que le programme dit qu’elle est.

## Options

- **t0. Comme aujourd’hui** : des nombres de millisecondes.
- **t1. Des types `Duration` et `Instant`**, chacun un nombre en dessous, sans coût : `Duration.ms(300)`, `Duration.seconds(5)`, `Duration.minutes(2)`, `d.toMs()`, `d.toSeconds()` ; `Instant.now()` dans un module managed ; `b - a` de deux instants est une `Duration`, un instant plus une durée un `Instant`, deux durées s’additionnent, une durée fois un nombre en est une ; `setTimeout` et `setInterval` prennent une `Duration`, un nombre à cette place une erreur dont le correctif écrit l’unité. Dans un module owned, des durées sans horloge.
- **t2. t1, avec des littéraux qui portent leur unité** : `300ms`, `5s`, `2min`, `1h` sont des `Duration`, comme `2.5` est un flottant.

## Protocole, enregistré avant tout prototype

Chaque option est prototypée derrière `PERAS_PROTO_TIME` ; le build sans elle garde le comportement d’aujourd’hui, t0. Chaque module mesuré est un build de production, nommé (constat M029).

1. **Bugs attrapés**, comptés comme la décision 0011 les compte : six programmes, chacun avec un bug de temps qu’une personne écrit, en TypeScript et en Peras sous chaque option ; si le bug se construit inaperçu, est refusé par un diagnostic qui le dit, ou ne peut pas s’écrire :
   - **B1**, cinq secondes écrites `setTimeout(5, …)` ;
   - **B2**, des secondes ajoutées à des millisecondes, `start + 30` pour trente secondes ;
   - **B3**, deux instants additionnés, `now() + start` ;
   - **B4**, une durée lue comme des secondes là où elle est en millisecondes, `elapsed / 60` pour des minutes ;
   - **B5**, un délai de `minutes * 60 * 100` pour des millisecondes, un zéro manquant ;
   - **B6**, un instant de `performance.now()` comparé à un de `Date.now()`, deux horloges.
2. **Facilité d’écriture**, quatre programmes écrits pour chaque option, comptés comme la décision 0028 les compte (lignes, notions à apprendre, erreurs rencontrées) :
   - **P1**, une recherche qui attend 300 ms après la dernière touche ;
   - **P2**, le temps qu’a pris un calcul, affiché en millisecondes avec une décimale ;
   - **P3**, un compte à rebours, « 2 h 05 min » à partir d’une durée ;
   - **P4**, dans un module owned, la durée totale d’une liste de lecture, ses morceaux en secondes.
3. **Justesse** : chaque programme donne le résultat du TypeScript ; un test de bout en bout dans les deux modes.
4. **Vitesse et charge du processeur**, dans Node, le TypeScript et le module de chaque option dans un même processus, l’ordre alternant, 21 tours, la médiane des rapports tour par tour et leur étendue (constat M017), le temps CPU sur le temps réel : **W1**, 1 000 000 durées additionnées et comparées, dans un module managed et dans un module owned, face aux nombres du TypeScript : une durée doit coûter ce que coûte un nombre.
5. **Octets** : chaque module, brut et en gzip, et chaque module du dépôt par `scripts/check-regression.mjs` sous chaque option, qui donne aussi le temps de compilation et la taille du compilateur.
6. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-time.mjs` lance les points 1, 2 et 4 ; `--check` vérifie les résultats de chaque option contre ceux du TypeScript, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._

### La mesure (2026-11-23)

Deux exécutions au commit 370b4ce, l’une après l’autre, rien d’autre ne tournant, chaque module construit pour la production ; chaque tableau dans [`bench/time/RESULTS.md`](../../../bench/time/RESULTS.md), avec la machine (un Intel Xeon à 2,10 GHz). Sous t2, un bug s’écrit comme sous t1 : c’est l’unité qui manque.

| Bug                                                     | TypeScript             | t0, aujourd’hui        | t1 et t2                                                       |
| ------------------------------------------------------- | ---------------------- | ---------------------- | -------------------------------------------------------------- |
| B1, cinq secondes écrites `setTimeout(5, …)`            | se construit, inaperçu | se construit, inaperçu | refusé, E0210 : un nombre n’est pas une `Duration`             |
| B2, `start + 30` pour trente secondes                   | se construit, inaperçu | se construit, inaperçu | refusé, E0210                                                  |
| B3, deux instants additionnés                           | se construit, inaperçu | se construit, inaperçu | refusé, E0221 : `+` ne peut pas prendre `Instant` et `Instant` |
| B4, des millisecondes lues comme des secondes           | se construit, inaperçu | se construit, inaperçu | refusé, E0210, là où les minutes rencontrent `5`               |
| B5, un zéro manquant, `Duration.ms(minutes * 60 * 100)` | se construit, inaperçu | se construit, inaperçu | se construit, inaperçu                                         |
| B6, un instant d’une autre horloge                      | se construit, inaperçu | se construit, inaperçu | refusé, E0210, mais son correctif est faux (constat F225)      |

- **t1 et t2 attrapent quatre bugs sur six**, TypeScript et le Peras d’aujourd’hui aucun. B5 n’est pas attrapé : écrit en millisecondes, le nombre est un nombre ; `Duration.minutes(minutes)` ne laisserait pas le zéro manquer, ce qu’aucun compilateur ne peut faire écrire à une personne. B6 est refusé, le nombre de l’autre horloge n’étant pas un `Instant`, mais le correctif proposé, `Duration.ms(created)`, en fait une durée, ce qui est faux : c’est un instant d’une autre horloge (constat F225).
- **Facilité** : chaque programme prend les mêmes lignes sous les trois options (P1 11, P2 13, P3 9, P4 11, W1 12), aucun n’a rencontré d’erreur en s’écrivant. Ce qui change est ce qui s’apprend : t1 ajoute deux types, quatre façons de faire une durée, quatre de la lire et `Instant.now()` ; t2 ajoute les quatre suffixes, `ms`, `s`, `min` et `h`, et écrit `300ms` là où t1 écrit `Duration.ms(300)`.
- **Vitesse, charge et octets** : un temps coûte un nombre. Les modules de W1 sous t1 et t2 sont ceux de t0, octet pour octet, dans les deux modes (506 octets en JavaScript, 170 en WebAssembly) ; W1 tourne à 0,90 à 0,92 du temps du TypeScript dans un module managed et 0,63 à 0,64 dans un module owned sous chaque option, le temps CPU sur le temps réel de 0,9 à 1,0.
- **Non-régression**, `scripts/check-regression.mjs 0158806` sous t1 et t2 : aucun module plus grand, plus petit ni changé ; temps de compilation de 1,012 à 1,024. Mais **trois programmes du dépôt ne se construisent plus**, chacun attendant un nombre de millisecondes : `orders.apeiron` et `orders-extern.apeiron` de l’étude de cas, et la fixture de bout en bout `browser/ui.apeiron`, avec six exemples de la documentation. Retenir t1 ou t2 change le langage : chaque `setTimeout(300, …)` écrit son unité. Les autres modules qui ne se construisent plus sont les programmes de cette décision pour t0.
- **Un module owned** a des durées et pas d’horloge : `Instant.now()` y est E0900, comme le protocole le disait.

### Proposition

**t1**, `Duration` et `Instant` : quatre des six bugs attrapés, sans coût en temps ni en octets, chaque programme aussi long qu’aujourd’hui. t2 attrape les mêmes bugs au même coût et écrit une durée en moins de caractères, `300ms`, pour quatre notions de plus et un changement de l’analyseur lexical ; elle peut s’ajouter plus tard à t1 sans rien changer de ce que t1 écrit, ce que t1 ne pourrait pas être après t2. Son prix, dans les deux cas, est la migration de chaque minuterie qui attend un nombre nu : trois programmes et six exemples ici, et ceux de chaque utilisateur. Le propriétaire décide.

### Décision et installation (2026-11-23)

Le propriétaire a retenu **t1**. `Duration` et `Instant` sont toujours là ; `PERAS_PROTO_TIME` a disparu, et avec elle les littéraux de t2, leurs chiffres ci-dessus et leur code dans git au commit 370b4ce. Chaque minuterie du dépôt qui attendait un nombre nu dit désormais son unité, `setTimeout(Duration.ms(300), …)` : les deux versions de l’étude de cas, la fixture de bout en bout, deux bugs web, six tests du vérificateur et du générateur, et les exemples du guide et de la référence du navigateur. Un nombre ôté d’un `Instant` est désormais `check-time-either`, dont le correctif nomme les deux lectures et ne modifie rien (constat F225). Un test du vérificateur tient les quatre erreurs, un du générateur les durées et instants calculés en millisecondes. `scripts/bench-time.mjs` mesure le build d’aujourd’hui face au TypeScript dans `bench/time/CURRENT.md` ; [`RESULTS.md`](../../../bench/time/RESULTS.md) garde les options telles que mesurées.

Non-régression, `scripts/check-regression.mjs d964ca4` : aucun module plus grand, plus petit ni changé ; temps de compilation 0,976. Les trois programmes migrés se construisent dans le JavaScript qu’ils donnaient avant, octet pour octet (vérifié contre le compilateur de d964ca4 sur leurs anciennes sources : un littéral entier attendu s’écrit `50`, non `50.0`) ; les autres modules qui ne se construisent que d’un côté sont les programmes de cette décision, et `b1-t1.apeiron`, le bug B1, refusé comme il doit l’être.
