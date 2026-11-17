# 0035 : `Math` tel que JavaScript l’écrit, et arrondir un entier

_Version anglaise : [../../en/decisions/0035-math-as-javascript-writes-it.md](../../en/decisions/0035-math-as-javascript-writes-it.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet. Il répond aux constats F116 et F117, que la [décision 0034](0034-integer-division.md) a rencontrés, et à ce que cette décision a laissé de F113 : un pourcentage arrondi, `Math.round(done * 100 / total)`, donne 66 % là où JavaScript donne 67 %. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **`Math` n’est défini dans aucun mode** : `Math.floor(x)`, `Math.max(a, b)`, `Math.PI` donnent `` `Math` is not defined `` (E0200), dont la correction, « déclare-le, ou vérifie son orthographe », n’offre rien (F116). Peras les écrit en méthodes : `abs`, `min`, `max`, `clamp` sur chaque nombre ; `sqrt`, `sin`, `cos`, `tan`, `floor`, `ceil`, `round`, `exp`, `log`, `pow`, `atan2`, `hypot` sur les flottants (`crates/perasc-check/src/builtins.rs`). Il n’a ni `PI`, ni `E`, ni `trunc`.
- **Un entier n’a ni `floor`, ni `round`, ni `ceil`** : `(p / width).floor()` donne E0215, sans correction (F117). Un entier est déjà entier, mais une division entière est tronquée : `round` sur l’une d’elles arrondit une fraction dans le mauvais sens, `Math.round(200 / 3)` 66 là où JavaScript donne 67.
- **Chaque diagnostic bloque la construction** : il n’y a pas d’avertissement.

## Options

Deux questions, mesurées ensemble, puisque l’une mène à l’autre : `Math.round(a / b)` écrit autrement devient `(a / b).round()`.

- **Question A, `Math` :**
  - **a0.** Comme aujourd’hui.
  - **a1. L’erreur offre la méthode** : `Math.f(x)` donne E0200 avec la correction `x.f()`, `Math.f(a, b)` avec `a.f(b)`, `Math.PI` avec sa valeur écrite, `3.141592653589793`.
  - **a2. `Math` accepté tel qu’écrit** : `Math.f(x)` est `x.f()` dans un module owned, le `Math.f(x)` de JavaScript dans un module managed ; `Math.PI` et `Math.E` sont ces nombres.
- **Question B, `floor`, `round`, `ceil` ou `trunc` sur un entier :**
  - **b0.** Comme aujourd’hui.
  - **b1. L’erreur offre d’enlever l’appel**, l’entier étant déjà entier.
  - **b2. Accepté** : l’appel donne l’entier.
  - **b3. Distingué** : sur une division entière, l’erreur montre la division et convertit ses opérandes, comme l’option d1 de la décision 0034, pour que la fraction soit arrondie, `((done * 100).toF64() / total.toF64()).round()` ; sur tout autre entier, la correction enlève l’appel.

## Protocole, enregistré avant tout prototype

Chaque option est prototypée derrière `PERAS_PROTO_MATH` (a0 à a2) et `PERAS_PROTO_WHOLE` (b0 à b3) ; le build sans elles garde le comportement d’aujourd’hui, a0 et b0. Les paires mesurées : a0 b0, et chacune de a1 et a2 avec chacune de b1, b2 et b3.

1. **Facilité d’écriture, et les résultats que donnent les corrections** : six programmes, écrits comme un utilisateur les écrit à partir de JavaScript, puis corrigés en appliquant les modifications qu’offrent leurs diagnostics, telles que données, rien d’autre, jusqu’à ce qu’ils se construisent ; un programme dont les diagnostics n’offrent aucune modification est bloqué :
   - R1, une distance et un angle, `Math.sqrt(dx * dx + dy * dy)`, `Math.atan2(dy, dx) * 180 / Math.PI` ;
   - R2, une composante de couleur bornée, `Math.min(Math.max(v, 0), 255)`, sur des entiers ;
   - R3, le milieu d’une recherche dichotomique, `Math.floor((lo + hi) / 2)` (Q3 de la décision 0034) ;
   - R4, la ligne d’un pixel, `Math.floor(p / width)` (Q4) ;
   - R5, un pourcentage, `Math.round(done * 100 / total)` (Q5) ;
   - R6, un prix arrondi au centime, `Math.round(price * 100) / 100`, sur un flottant.

   Pour chaque paire : les erreurs rencontrées, avec leurs codes ; les lignes changées ; si le programme se construit ; et s’il donne le résultat de JavaScript. Un résultat faux atteint en suivant les corrections du compilateur lui-même compte contre la paire, quoi qu’elle gagne par ailleurs.

2. **Non-régression** (la règle du propriétaire) : `scripts/check-regression.mjs` sous chaque option contre le compilateur d’aujourd’hui : les programmes qui ne se construisent plus, ou qui changent.
3. **Vitesse et charge du processeur** : R4 et R1 tels que chaque paire les a corrigés, sur 10 000 000 de tours, modules owned sur TLSF dans Node, dans un même processus, l’ordre alternant, 21 tours, la médiane des rapports à la première paire qui les construit, tour par tour, et leur étendue (constat M017), le temps CPU sur le temps réel. Une paire qui laisse un programme bloqué n’en a pas de version : mesurée comme non construite (constat M001).
4. **Octets** : le module de chaque version, brut et en gzip.

`node scripts/bench-math.mjs` les lance ; `--check` vérifie que chaque programme se construit et donne ce qu’il a donné, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._
