# 0034 : La division entière, telle qu’un utilisateur venu de JavaScript la rencontre

_Version anglaise : [../../en/decisions/0034-integer-division.md](../../en/decisions/0034-integer-division.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet. Il répond au constat F113, que la [décision 0030](0030-numeric-literals.md) a trouvé : `const t = i / (count - 1); total += t;` somme 1 là où JavaScript somme 2,5. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **`/` sur deux entiers est une division entière**, tronquée vers zéro, comme en Rust, Go, C et Swift (`BinOp::Div`, `crates/perasc-ir/src/lower.rs` ; `$div` dans un module managed). JavaScript n’a pas d’entier : son `/` donne une fraction, et une division entière s’écrit `Math.floor(a / b)` ou `Math.trunc`.
- **Une variable n’est jamais convertie** (décision 0030) : `total += t`, `t` un `i32` et `total` un `f64`, rencontre E0210 à la somme. Sa correction, `t.toF64()`, convertit le quotient déjà tronqué : le programme se construit et donne 1 là où JavaScript donne 2,5. Suivie telle qu’offerte, la correction du compilateur donne un nombre faux, en silence.
- **Chaque diagnostic bloque la construction** : il n’y a pas d’avertissement (`crates/perasc-diag/src/lib.rs`). Une option peut dire, ou refuser ; elle ne peut pas seulement conseiller.
- **Les programmes du dépôt divisent des entiers exprès** : un milieu `(lo + hi) / 2`, une ligne `p / width`. 280 lignes des bancs, des exemples et de la documentation contiennent `/` ; combien sont des divisions entières, et ce qu’une option qui change `/` casserait, les prototypes le comptent.

## Options

- **d0. Comme aujourd’hui.**
- **d1. L’erreur dit où.** Quand E0210 rencontre un entier là où un flottant est attendu, et que cet entier est une division d’entiers, écrite là ou tenue par un `const` qu’elle a initialisé, le message montre la division, dit qu’elle tronque, et sa correction est à la division : `i.toF64() / (count - 1).toF64()`. Rien d’autre ne change : `/` garde son sens.
- **d2. `/` comme celui de JavaScript.** `/` sur deux entiers donne un `f64` ; une division entière s’écrit `a.div(b)`.
- **d3. Ni l’un ni l’autre, refusé.** `/` sur deux entiers est une erreur qui offre les deux : `a.div(b)` pour le quotient entier, `a.toF64() / b.toF64()` pour la fraction.

## Protocole, enregistré avant tout prototype

Chaque option est prototypée derrière `PERAS_PROTO_DIV` ; le build sans elle garde le comportement d’aujourd’hui, d0.

1. **Facilité d’écriture, et les résultats que donnent les corrections** : cinq programmes, écrits comme un utilisateur les écrit à partir de JavaScript, puis corrigés en appliquant la correction qu’offre chaque diagnostic, telle que donnée, jusqu’à ce qu’ils se construisent :
   - Q1, les pas du dégradé sommés, `const t = i / (count - 1); total += t;` (`bench/numbers/ease/p5t.peras`), une fraction voulue ;
   - Q2, une moyenne rendue en `f64`, `return sum / count;`, une fraction voulue ;
   - Q3, le milieu d’une recherche dichotomique, `Math.floor((lo + hi) / 2)` en JavaScript, un entier voulu ;
   - Q4, la ligne et la colonne d’un pixel, `Math.floor(p / width)` et `p % width`, un entier voulu ;
   - Q5, un pourcentage affiché en nombre entier, `Math.round(done * 100 / total)`, une fraction arrondie.

   Pour chacun : les erreurs rencontrées, avec leurs codes ; les lignes changées ; et **si le programme ainsi corrigé donne le résultat de JavaScript**. Un résultat faux atteint en suivant la correction du compilateur lui-même compte contre l’option, quoi qu’elle gagne par ailleurs.

2. **Non-régression** (la règle du propriétaire du 2026-11-17) : chaque programme des bancs et des exemples construit sous chaque option contre le compilateur d’aujourd’hui, `scripts/check-regression.mjs` : combien ne se construisent plus, combien changent. Une option sous laquelle un programme du dépôt ne se construit plus, ou calcule autre chose, régresse : elle est écartée, sauf si la décision migre chacun de ces programmes, la migration comptée en lignes.
3. **Vitesse et charge du processeur** : deux boucles de 10 000 000 de tours, la version de chaque option telle que corrigée au point 1, modules owned sur TLSF dans Node, dans un même processus, l’ordre alternant, 21 tours, la médiane des rapports à d0 tour par tour et leur étendue (constat M017), le temps CPU sur le temps réel : W1, une ligne et une colonne par pixel, un entier voulu ; W2, la somme du dégradé, une fraction voulue.
4. **Octets** : le module de chaque version, brut et en gzip.
5. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-division.mjs` les lance ; `--check` vérifie que chaque programme se construit comme consigné et donne ce qu’il a donné, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._
