# 0039 : Durées et instants

_Version anglaise : [../../en/decisions/0039-durations-and-instants.md](../../en/decisions/0039-durations-and-instants.md)._

_État : protocole enregistré le 2026-11-18, avant tout prototype et toute mesure, comme le demande la règle du projet. Il répond à la ligne « temps » de la phase 3 de la [feuille de route](../reference/roadmap.md), durées et instants ; les dates du calendrier et les fuseaux horaires sont laissés à une décision suivante. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

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
